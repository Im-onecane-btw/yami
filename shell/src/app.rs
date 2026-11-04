// Ядро: Wayland (layer-shell, ввод, кадры), поверхности шелла, цикл событий, маршрутизация
// кликов в действия. Состояние, которое рисуется, — в State; ресурсы (шрифты, иконки) — в Res.

use crate::config::{self, Settings};
use crate::gfx::{Canvas, R};
use crate::icons::{self, Icons};
use crate::niri::{self, Niri};
use crate::sys::Sys;
use crate::theme::{Theme, CLEAR};
use crate::ui::{Act, Cur, Field, Ui, Zone};
use crate::{audio, dbus, font::Fonts, launcher, media, menu, net, notif, popups};

use calloop::{
    channel::{self, Sender},
    timer::{TimeoutAction, Timer},
    EventLoop, LoopHandle,
};
use calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, Region},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer, delegate_registry, delegate_seat,
    delegate_session_lock, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{cursor_shape::CursorShapeManager, PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    session_lock::{SessionLock, SessionLockHandler, SessionLockState, SessionLockSurface, SessionLockSurfaceConfigure},
    shell::{
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols::ext::background_effect::v1::client::{ext_background_effect_manager_v1, ext_background_effect_surface_v1};
use wayland_protocols::ext::idle_notify::v1::client::{ext_idle_notification_v1, ext_idle_notifier_v1};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::{Shape, WpCursorShapeDeviceV1};
use wayland_protocols::wp::fractional_scale::v1::client::{wp_fractional_scale_manager_v1 as frac_mgr, wp_fractional_scale_v1 as frac};
use wayland_protocols::wp::keyboard_shortcuts_inhibit::zv1::client::{zwp_keyboard_shortcuts_inhibit_manager_v1 as ksi_mgr, zwp_keyboard_shortcuts_inhibitor_v1 as ksi};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};

pub const BAR_H: i32 = 32;
pub const TITLE_H: i32 = 26;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Modal {
    Launcher,
    Power,
    Settings,
    /// пароль для polkit (auth.rs)
    Auth,
    /// что показать при показе экрана (screen.rs)
    Screen,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SKind {
    Wall,
    Bar,
    Dim,
    Catcher,
    Modal,
    Popup,
    Menu,
    Notifs,
    Osd,
    Widget(u32),
    Title(u64),
    EditBar,
    Snap,
    /// шторка для трансляций и значок «скрыто» (curtain.rs)
    Curtain,
    CurtainBadge,
    Dock,
    DockEdge,
}

pub struct Surf {
    pub kind: SKind,
    pub layer: LayerSurface,
    pub output: String,
    pub w: i32,
    pub h: i32,
    pub configured: bool,
    pub pool: Option<SlotPool>,
    pub dirty: bool,
    pub frame_pending: bool,
    pub anim: bool,
    pub zones: Vec<Zone>,
    pub closing: bool,
    pub born: Instant,
    pub viewport: Option<wp_viewport::WpViewport>,
    pub margin: (i32, i32),
    /// где поверхность сейчас у композитора (подтверждено wl_display.sync): события
    /// курсора приходят относительно именно этого места, а не только что запрошенного
    pub applied: (i32, i32),
    pub input: Option<Vec<R>>,
    pub input_set: Option<Vec<R>>,
    /// обои: буфер отдан и больше не нужен
    pub release_pool: bool,
    /// ещё ни разу не рисовалась: анимация появления считается от первого кадра
    pub fresh: bool,
    /// размытие фона через ext-background-effect: объект и отданный композитору регион
    pub fx: Option<ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1>,
    pub blur_set: Vec<R>,
    /// док: у какого края стоит
    pub anchor_side: String,
    /// физических пикселей на логический (wp_fractional_scale): буфер рисуется в нём,
    /// логический холст увеличивается ближайшим соседом — на HiDPI пиксели чёткие
    pub scale: f64,
    pub frac: Option<frac::WpFractionalScaleV1>,
}

pub struct LockSurf {
    pub s: SessionLockSurface,
    /// монитор: по нему видно, у какого вывода экрана блокировки ещё нет
    pub wl: wl_output::WlOutput,
    pub output: String,
    pub w: i32,
    pub h: i32,
    pub pool: Option<SlotPool>,
    pub zones: Vec<Zone>,
    pub dirty: bool,
    pub scale: f64,
    pub frac: Option<frac::WpFractionalScaleV1>,
    pub vp: Option<wp_viewport::WpViewport>,
}

pub enum Msg {
    Niri(serde_json::Value),
    Ipc(String),
    Audio(audio::AudioState),
    Viz(Vec<f32>),
    Media(media::MediaState),
    Notif(notif::Incoming),
    NotifClose(u32),
    Tray(dbus::TrayEv),
    TrayMenu(String, Vec<menu::MItem>),
    Net(net::NetState),
    Bt(net::BtState),
    Pam(bool),
    Auth(crate::auth::Ev),
    Prof(crate::profile::Prof),
    /// портал спрашивает, что показать: типы источников и сокет для ответа
    ScreenPick(u32, std::os::unix::net::UnixStream),
    Clip(Vec<String>),
    Art(String, Option<crate::gfx::Img>),
    Theme,
    /// кадры шторки готовы: номер включения, картинки по мониторам
    Curtain(u64, HashMap<String, crate::gfx::Img>),
    /// обои готовы (wall.rs): номер задания, картинка, акцент из неё
    Wall(u64, Option<crate::gfx::Img>, Option<u32>),
    Snap(HashMap<String, crate::gfx::Img>, Vec<(String, R, String)>, crate::snap::Mode),
    /// проверка обновлений (update.rs)
    Update(crate::update::Info),
    SnapDone { path: Option<std::path::PathBuf>, copied: bool, err: Option<String>, thumb: Option<crate::gfx::Img> },
}

pub struct Res {
    pub fonts: Fonts,
    pub icons: Icons,
    /// семейство, из которого собраны fonts (fontShell)
    pub font: String,
}

pub struct Popup {
    pub kind: &'static str,
}

pub struct State {
    pub set: Settings,
    pub t: Theme,
    pub niri: Niri,
    pub sys: Sys,
    pub apps: Vec<icons::App>,
    pub modal: Option<Modal>,
    pub popup: Option<Popup>,
    pub menu: Option<menu::MenuState>,
    pub bar_hidden: bool,
    pub launcher: launcher::LState,
    pub power_cur: usize,
    pub stab: &'static str,
    pub hex: Field,
    pub field_focus: Option<&'static str>,
    /// запросы polkit: первый — в окне, остальные ждут
    pub auth: Vec<crate::auth::Req>,
    pub prof: crate::profile::Prof,
    pub disp: crate::displays::Disp,
    pub bar_ed: crate::settings::BarEd,
    pub screen_pick: Option<crate::screen::Pick>,
    pub sel_widget: u32,
    pub editing: bool,
    pub notifs: notif::Notifs,
    pub osd: Option<(&'static str, Instant)>,
    /// шторка для трансляций опущена
    pub curtain: Option<crate::curtain::Curtain>,
    pub audio: audio::AudioState,
    pub media: media::MediaState,
    pub art: HashMap<String, Option<std::rc::Rc<crate::gfx::Img>>>,
    pub viz: Vec<f32>,
    pub net: net::NetState,
    pub bt: net::BtState,
    pub wifi_pass: Field,
    pub wifi_target: String,
    /// о низком заряде уже сказали: 1 — на 10%, 2 — на 5%; сбрасывается на зарядке
    pub bat_warn: u8,
    pub tray: Vec<dbus::TrayItem>,
    pub locked: bool,
    pub lock_field: Field,
    pub lock_status: String,
    pub lock_fails: u32,
    pub lock_shake: Option<Instant>,
    pub cal_month: i32,
    pub blink: bool,
    pub wall: Option<std::rc::Rc<crate::gfx::Img>>,
    pub wall_stamp: u64,
    pub started: Instant,
    pub rss: f32,
    pub titles: HashMap<u64, crate::titles::TitleGeo>,
    pub wall_field: Field,
    pub keys: crate::keys::KeysUi,
    pub game: crate::games::GameMode,
    pub game_field: Field,
    pub snap: Option<crate::snap::Snap>,
    pub snap_countdown: Option<(Instant, u32)>,
    pub shot_files: HashMap<u32, std::path::PathBuf>,
    pub shot_field: Field,
    pub float_field: Field,
    pub dock_hover: bool,
    pub dock_leave: Option<Instant>,
    pub dock_bounce: Vec<(String, Instant)>,
    /// наведение в доке для анимации: (сейчас, до этого, когда сменилось)
    pub dock_hov: std::cell::Cell<(Option<usize>, Option<usize>, Instant)>,
    pub trash_full: bool,
    pub upd: crate::update::Upd,
}

pub struct Press {
    pub sid: u32,
    pub act: Act,
    pub button: u32,
    pub x: f64,
    pub y: f64,
    pub zone: R,
    /// абсолютная позиция на экране в момент нажатия (для перетаскивания)
    pub ax: f64,
    pub ay: f64,
    pub moved: bool,
    pub start: (i32, i32),
    /// где стояла поверхность при нажатии: пока кнопка зажата, композитор (smithay
    /// ClickGrab) считает координаты курсора от этого места, даже если поверхность уехала
    pub origin: (i32, i32),
}

pub struct App {
    pub conn: Connection,
    pub qh: QueueHandle<App>,
    pub handle: LoopHandle<'static, App>,
    pub tx: Sender<Msg>,
    registry: RegistryState,
    seat: SeatState,
    pub outputs: OutputState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    shm: Shm,
    cursor_mgr: Option<CursorShapeManager>,
    cursor_dev: Option<WpCursorShapeDeviceV1>,
    cursor_now: Option<Shape>,
    viewporter: Option<wp_viewporter::WpViewporter>,
    fx_mgr: Option<ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1>,
    /// запись сочетаний во вкладке «клавиши»: niri отдаёт шеллу все клавиши
    ksi_mgr: Option<ksi_mgr::ZwpKeyboardShortcutsInhibitManagerV1>,
    ksi: Option<ksi::ZwpKeyboardShortcutsInhibitorV1>,
    frac_mgr: Option<frac_mgr::WpFractionalScaleManagerV1>,
    /// логический холст перед увеличением (HiDPI); большой — освобождается сразу
    scratch: Vec<u32>,
    lock_state: SessionLockState,
    pub lock: Option<SessionLock>,
    pub lock_surfs: Vec<LockSurf>,
    idle: Option<ext_idle_notifier_v1::ExtIdleNotifierV1>,
    idle_notes: Vec<ext_idle_notification_v1::ExtIdleNotificationV1>,
    idle_cfg: (u32, u32),
    pointer: Option<wl_pointer::WlPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    enter_serial: u32,
    pub surfs: HashMap<u32, Surf>,
    next_id: u32,
    /// курсор: (поверхность, x, y); для экрана блокировки sid = u32::MAX - n
    pub ptr: Option<(u32, f64, f64)>,
    pub press: Option<Press>,
    last_click: Option<(Act, Instant)>,
    pub kb_surf: Option<u32>,
    pub mods: Modifiers,
    pub res: Res,
    pub st: State,
    pub set_saved: String,
    set_mtime: Option<std::time::SystemTime>,
    last_sample: Instant,
    last_min: u32,
    palette_written: String,
    /// обои: ключ того, из чего собрана текущая картинка, и номер последнего задания
    pub wall_key: String,
    pub wall_job: u64,
    /// номер последнего включения шторки (поздний кадр от прошлого — мимо)
    pub curtain_wanted: u64,
    pub quit: bool,
    /// выйти и запустить себя заново (тот же путь: после обновления — новый бинарь)
    pub restart: bool,
    pub tray_anchor: Option<(String, i32)>,
    pub tdrag: Option<(u64, (f64, f64))>,
}

const LOCK_BASE: u32 = u32::MAX - 64;

impl State {
    pub fn new(set: Settings, niri: Niri) -> State {
        State {
            t: Theme::new(set.accent_rgb(), set.tint, set.glass, set.lift, set.inner),
            niri,
            sys: Sys::new(),
            apps: icons::load_apps(),
            modal: None,
            popup: None,
            menu: None,
            bar_hidden: false,
            launcher: launcher::LState::default(),
            power_cur: 0,
            stab: "color",
            hex: Field::default(),
            field_focus: None,
            auth: Vec::new(),
            prof: Default::default(),
            disp: Default::default(),
            bar_ed: Default::default(),
            screen_pick: None,
            sel_widget: 0,
            editing: false,
            notifs: notif::Notifs::default(),
            osd: None,
            curtain: None,
            audio: audio::AudioState::default(),
            media: media::MediaState::default(),
            art: HashMap::new(),
            viz: vec![],
            net: net::NetState::default(),
            bt: net::BtState::default(),
            wifi_pass: Field::default(),
            wifi_target: String::new(),
            bat_warn: 0,
            tray: vec![],
            locked: false,
            lock_field: Field::default(),
            lock_status: String::new(),
            lock_fails: 0,
            lock_shake: None,
            cal_month: 0,
            blink: true,
            wall: None,
            wall_stamp: 0,
            started: Instant::now(),
            rss: 0.0,
            titles: HashMap::new(),
            wall_field: Field::default(),
            keys: Default::default(),
            game: Default::default(),
            game_field: Field::default(),
            snap: None,
            snap_countdown: None,
            shot_files: HashMap::new(),
            shot_field: Field::default(),
            float_field: Field::default(),
            dock_hover: false,
            dock_leave: None,
            dock_bounce: vec![],
            dock_hov: std::cell::Cell::new((None, None, Instant::now())),
            trash_full: false,
            upd: crate::update::Upd::load(),
            set,
        }
    }
}

pub fn run() {
    let conn = Connection::connect_to_env().expect("no Wayland display");
    let (globals, mut queue) = registry_queue_init::<App>(&conn).expect("wayland registry");
    let qh = queue.handle();
    let mut ev: EventLoop<'static, App> = EventLoop::try_new().expect("event loop");
    let handle = ev.handle();

    let (tx, rx) = channel::channel::<Msg>();
    handle
        .insert_source(rx, |e, _, app: &mut App| {
            if let channel::Event::Msg(m) = e {
                app.on_msg(m);
            }
        })
        .expect("message channel");

    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("layer-shell");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm");
    let cursor_mgr = CursorShapeManager::bind(&globals, &qh).ok();
    let viewporter: Option<wp_viewporter::WpViewporter> = globals.bind(&qh, 1..=1, ()).ok();
    let fx_mgr: Option<ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1> = globals.bind(&qh, 1..=1, ()).ok();
    let idle: Option<ext_idle_notifier_v1::ExtIdleNotifierV1> = globals.bind(&qh, 1..=1, ()).ok();
    let ksi_mgr: Option<ksi_mgr::ZwpKeyboardShortcutsInhibitManagerV1> = globals.bind(&qh, 1..=1, ()).ok();
    let frac_mgr: Option<frac_mgr::WpFractionalScaleManagerV1> = globals.bind(&qh, 1..=1, ()).ok();

    let set = Settings::load();
    crate::i18n::set(&set.lang);
    let mut icons = Icons::new(&set.icon_theme);
    icons.set_pixel(set.icon_pixel.then(|| Theme::new(set.accent_rgb(), set.tint, set.glass, set.lift, set.inner).icon_ramp()));
    let set_saved = set.to_json();
    let set_font = set.font_shell.clone();

    let mut niri = Niri::new();
    niri.stacking = niri.stacking || set.stacking;

    let st = State::new(set, niri);

    let mut app = App {
        conn: conn.clone(),
        qh: qh.clone(),
        handle: handle.clone(),
        tx: tx.clone(),
        registry: RegistryState::new(&globals),
        seat: SeatState::new(&globals, &qh),
        outputs: OutputState::new(&globals, &qh),
        compositor,
        layer_shell,
        shm,
        cursor_mgr,
        cursor_dev: None,
        cursor_now: None,
        viewporter,
        fx_mgr,
        ksi_mgr,
        ksi: None,
        frac_mgr,
        scratch: vec![],
        lock_state: SessionLockState::new(&globals, &qh),
        lock: None,
        lock_surfs: vec![],
        idle,
        idle_notes: vec![],
        idle_cfg: (0, 0),
        pointer: None,
        keyboard: None,
        enter_serial: 0,
        surfs: HashMap::new(),
        next_id: 1,
        ptr: None,
        press: None,
        last_click: None,
        kb_surf: None,
        mods: Modifiers::default(),
        res: Res { fonts: Fonts::with(&set_font), icons, font: set_font },
        st,
        set_saved,
        set_mtime: std::fs::metadata(config::settings_path()).and_then(|m| m.modified()).ok(),
        last_sample: Instant::now(),
        last_min: 99,
        palette_written: String::new(),
        wall_key: String::new(),
        wall_job: 0,
        curtain_wanted: 0,
        quit: false,
        restart: false,
        tray_anchor: None,
        tdrag: None,
    };

    // нити: niri, IPC, D-Bus (уведомления, трей), звук, музыка
    {
        let tx = tx.clone();
        niri::spawn_stream(move |v| {
            let _ = tx.send(Msg::Niri(v));
        });
    }
    crate::ipc::serve(tx.clone());
    dbus::spawn(tx.clone(), app.st.set.agent_notif, app.st.set.agent_tray);
    crate::auth::spawn(tx.clone(), app.st.set.agent_polkit);
    crate::agents::write_portals(&app.st.set);
    crate::profile::refresh(None, tx.clone());
    // копия theme.py рядом с состоянием — сразу свежая, а не после следующей смены палитры
    let _ = theme_script();
    audio::spawn(tx.clone());
    media::spawn(tx.clone());
    net::spawn_bt(tx.clone());
    app.write_palette();
    // обои: при старте палитра обычно та же, theme.py не запускается — готовим картинку сами
    app.sync_wall();
    app.write_fx();
    crate::keys::write(&app.st.set);
    app.setup_idle();
    // «стол» при входе включится, когда придёт список окон (см. on_msg)

    handle
        .insert_source(Timer::from_duration(Duration::from_millis(300)), |_, _, app: &mut App| {
            app.tick();
            TimeoutAction::ToDuration(app.tick_interval())
        })
        .expect("timer");

    // мониторы, сиденье: первые события — сразу, до цикла
    let _ = queue.roundtrip(&mut app);
    let _ = queue.roundtrip(&mut app);
    WaylandSource::new(conn.clone(), queue).insert(handle.clone()).expect("wayland source");
    app.flush();
    loop {
        if ev.dispatch(Some(Duration::from_secs(5)), &mut app).is_err() {
            break;
        }
        app.flush();
        if app.quit || app.restart {
            break;
        }
    }
    if app.restart {
        app.save_now();
        // exec вместо выхода: петля автозапуска niri после чистого выхода не перезапускает
        use std::os::unix::process::CommandExt;
        let args: Vec<String> = std::env::args().skip(1).collect();
        drop(app);
        let e = std::process::Command::new(yami_px::paths::self_exe()).args(args).exec();
        eprintln!("restart: {e}");
        std::process::exit(1);
    }
}

impl App {
    // ── поверхности ─────────────────────────────────────────────────────────
    pub fn output_obj(&self, name: &str) -> Option<wl_output::WlOutput> {
        self.outputs.outputs().find(|o| self.outputs.info(o).and_then(|i| i.name.clone()).as_deref() == Some(name))
    }
    pub fn output_names(&self) -> Vec<String> {
        self.outputs.outputs().filter_map(|o| self.outputs.info(&o).and_then(|i| i.name.clone())).collect()
    }
    pub fn output_size(&self, name: &str) -> (i32, i32) {
        self.outputs
            .outputs()
            .filter_map(|o| self.outputs.info(&o))
            .find(|i| i.name.as_deref() == Some(name))
            .and_then(|i| i.logical_size)
            .unwrap_or((1920, 1080))
    }
    /// монитор, на котором сейчас работает человек
    pub fn focus_out(&self) -> String {
        let f = self.st.niri.focused_output();
        if !f.is_empty() && self.output_names().contains(&f) {
            return f;
        }
        self.output_names().into_iter().next().unwrap_or_default()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn mk(&mut self, kind: SKind, output: &str, layer: Layer, ns: &str, anchor: Anchor, size: (i32, i32), margin: (i32, i32, i32, i32), excl: i32, kb: KeyboardInteractivity) -> u32 {
        // панель поверх опущенной шторки: шторку — выше неё (см. curtain_raise)
        let over_curtain = layer == Layer::Overlay && !matches!(kind, SKind::Curtain | SKind::CurtainBadge) && self.st.curtain.is_some();
        let surface = self.compositor.create_surface(&self.qh);
        let out = self.output_obj(output);
        let ls = self.layer_shell.create_layer_surface(&self.qh, surface, layer, Some(ns), out.as_ref());
        ls.set_anchor(anchor);
        ls.set_size(size.0.max(0) as u32, size.1.max(0) as u32);
        ls.set_margin(margin.0, margin.1, margin.2, margin.3);
        ls.set_exclusive_zone(excl);
        ls.set_keyboard_interactivity(kb);
        // viewport у всех: Dim/Catcher растягивают буфер 1×1, остальные — буфер в физических пикселях
        let viewport = self.viewporter.as_ref().map(|v| v.get_viewport(ls.wl_surface(), &self.qh, ()));
        let id = self.next_id;
        self.next_id += 1;
        let frac = self.frac_mgr.as_ref().map(|m| m.get_fractional_scale(ls.wl_surface(), &self.qh, id));
        ls.commit();
        self.surfs.insert(
            id,
            Surf {
                kind,
                layer: ls,
                output: output.to_string(),
                w: size.0,
                h: size.1,
                configured: false,
                pool: None,
                dirty: true,
                frame_pending: false,
                anim: false,
                zones: vec![],
                closing: false,
                born: Instant::now(),
                viewport,
                margin: (margin.3, margin.0),
                applied: (margin.3, margin.0),
                input: None,
                input_set: None,
                release_pool: false,
                fresh: true,
                fx: None,
                blur_set: vec![],
                anchor_side: String::new(),
                scale: 1.0,
                frac,
            },
        );
        if over_curtain {
            self.curtain_raise();
        }
        id
    }

    pub fn find(&self, k: &SKind) -> Vec<u32> {
        self.surfs.iter().filter(|(_, s)| &s.kind == k && !s.closing).map(|(i, _)| *i).collect()
    }
    pub fn find_on(&self, k: &SKind, out: &str) -> Option<u32> {
        self.surfs.iter().find(|(_, s)| &s.kind == k && s.output == out && !s.closing).map(|(i, _)| *i)
    }
    pub fn dirty(&mut self, k: &SKind) {
        for s in self.surfs.values_mut() {
            if &s.kind == k {
                s.dirty = true;
            }
        }
    }
    pub fn dirty_where(&mut self, f: impl Fn(&SKind) -> bool) {
        for s in self.surfs.values_mut() {
            if f(&s.kind) {
                s.dirty = true;
            }
        }
    }
    /// всё, что рисует состояние (кроме обоев)
    pub fn dirty_all(&mut self) {
        self.dirty_where(|k| *k != SKind::Wall);
        for l in self.lock_surfs.iter_mut() {
            l.dirty = true;
        }
    }
    pub fn destroy(&mut self, id: u32) {
        if let Some(s) = self.surfs.remove(&id) {
            if let Some(fx) = s.fx {
                fx.destroy();
            }
            if let Some(f) = s.frac {
                f.destroy();
            }
            if let Some(v) = s.viewport {
                v.destroy();
            }
            if self.ptr.map(|p| p.0) == Some(id) {
                self.ptr = None;
            }
            if self.kb_surf == Some(id) {
                self.kb_surf = None;
            }
            if self.press.as_ref().map(|p| p.sid) == Some(id) {
                self.press = None;
            }
        }
    }
    /// закрыть с анимацией (или сразу, если анимации выключены)
    pub fn close(&mut self, id: u32) {
        let anim = self.st.set.animations;
        if let Some(s) = self.surfs.get_mut(&id) {
            if !anim || !s.configured {
                self.destroy(id);
                return;
            }
            if !s.closing {
                s.closing = true;
                s.born = Instant::now();
                s.dirty = true;
                s.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
                // клики сквозь закрывающееся
                let r = Region::new(&self.compositor).ok();
                if let Some(r) = r {
                    s.layer.wl_surface().set_input_region(Some(r.wl_region()));
                }
                s.layer.commit();
            }
        }
    }
    pub fn close_kind(&mut self, k: &SKind) {
        for id in self.find(k) {
            self.close(id);
        }
    }

    pub fn ensure_output_surfaces(&mut self) {
        let names = self.output_names();
        // мониторы, которых больше нет
        let gone: Vec<u32> = self.surfs.iter().filter(|(_, s)| !names.contains(&s.output)).map(|(i, _)| *i).collect();
        for id in gone {
            self.destroy(id);
        }
        for n in names {
            if self.find_on(&SKind::Wall, &n).is_none() {
                self.mk(SKind::Wall, &n, Layer::Background, "yami-wall", Anchor::all(), (0, 0), (0, 0, 0, 0), -1, KeyboardInteractivity::None);
                // монитор появился: теперь известен его размер (обои режутся под него)
                self.sync_wall();
                // и шторку на него, если она опущена
                self.curtain_show();
            }
            if self.find_on(&SKind::Bar, &n).is_none() && !self.st.bar_hidden {
                self.mk(SKind::Bar, &n, Layer::Top, "yami-bar", Anchor::TOP | Anchor::LEFT | Anchor::RIGHT, (0, BAR_H), (0, 0, 0, 0), BAR_H, KeyboardInteractivity::None);
            }
        }
        self.sync_widgets();
        self.sync_dock();
        self.sync_lock_surfs();
    }

    pub fn toggle_bar(&mut self) {
        self.st.bar_hidden = !self.st.bar_hidden;
        if self.st.bar_hidden {
            for id in self.find(&SKind::Bar) {
                self.destroy(id);
            }
        }
        self.ensure_output_surfaces();
    }

    // ── рисование ───────────────────────────────────────────────────────────
    pub fn flush(&mut self) {
        // окно на весь экран (игра): что под ним, не рисуем — останется грязным до выхода из него
        let covered = if self.st.game.fullscreen { Some(self.focus_out()) } else { None };
        let under = |s: &Surf| covered.as_deref() == Some(s.output.as_str()) && matches!(s.kind, SKind::Bar | SKind::Widget(_) | SKind::Dock | SKind::Wall | SKind::Title(_));
        let ids: Vec<u32> = self.surfs.iter().filter(|(_, s)| s.dirty && s.configured && !s.frame_pending && !under(s)).map(|(i, _)| *i).collect();
        for id in ids {
            self.draw(id);
        }
        for i in 0..self.lock_surfs.len() {
            if self.lock_surfs[i].dirty {
                self.draw_lock(i);
            }
        }
        self.save_settings_if_changed();
        let _ = self.conn.flush();
    }

    fn draw(&mut self, id: u32) {
        let Some(mut s) = self.surfs.remove(&id) else { return };
        s.dirty = false;
        if s.fresh {
            s.fresh = false;
            s.born = Instant::now();
        }
        // обои: пиксели нужны один раз
        let scaled = matches!(s.kind, SKind::Dim | SKind::Catcher);
        let (bw, bh) = if scaled { (1, 1) } else { (s.w, s.h) };
        if bw <= 0 || bh <= 0 {
            self.surfs.insert(id, s);
            return;
        }
        // HiDPI: буфер в физических пикселях, холст — логический, потом увеличение
        // оверлей скриншота — застывший экран, не пиксель-арт: его растягивает композитор
        let (pw, ph) = if scaled || s.viewport.is_none() || s.scale == 1.0 || s.kind == SKind::Snap { (bw, bh) } else { crate::gfx::phys(bw, bh, s.scale) };
        let up = (pw, ph) != (bw, bh);
        let need = (pw * ph * 4) as usize;
        if s.pool.as_ref().map(|p| p.len() < need).unwrap_or(true) {
            s.pool = SlotPool::new(need * 2, &self.shm).ok();
        }
        let Some(pool) = s.pool.as_mut() else {
            self.surfs.insert(id, s);
            return;
        };
        let Ok((buffer, bytes)) = pool.create_buffer(pw, ph, pw * 4, wl_shm::Format::Argb8888) else {
            self.surfs.insert(id, s);
            return;
        };
        let (out_ptr, out_len) = (bytes.as_mut_ptr() as *mut u32, bytes.len() / 4);
        let mut scratch = std::mem::take(&mut self.scratch);
        let buf: &mut [u32] = if up {
            scratch.clear();
            scratch.resize((bw * bh) as usize, 0);
            &mut scratch
        } else {
            unsafe { std::slice::from_raw_parts_mut(out_ptr, out_len) }
        };
        let ptr = self.ptr.filter(|p| p.0 == id).map(|p| (p.1, p.2));
        let down = self.press.as_ref().map(|p| p.sid == id && p.button == 0x110).unwrap_or(false);
        let mut done_closing = false;
        let blur: Vec<R>;
        {
            let mut ui = Ui {
                cv: Canvas::new(buf, bw, bh),
                f: &mut self.res.fonts,
                ic: &mut self.res.icons,
                t: self.st.t,
                zones: vec![],
                ptr,
                down,
                anim: false,
                blink: self.st.blink,
                tr: crate::i18n::tr,
            };
            ui.cv.clear(CLEAR);
            let st = &self.st;
            // появление: пиксельное проявление по Байеру + плавное всплытие
            let (k, finished) = match s.kind {
                SKind::Modal | SKind::Popup | SKind::Menu | SKind::Dim | SKind::Catcher | SKind::Osd | SKind::EditBar | SKind::Widget(_) | SKind::Snap | SKind::Dock | SKind::CurtainBadge => {
                    let dur = match s.kind {
                        SKind::Snap => 200.0,
                        SKind::Dock => 200.0,
                        SKind::Widget(_) => 320.0,
                        SKind::Menu => 140.0,
                        SKind::Popup | SKind::Osd | SKind::CurtainBadge => 190.0,
                        _ => 230.0,
                    };
                    let (k, fin) = reveal_of(st, &s, dur);
                    (k, fin)
                }
                _ => (1.0, true),
            };
            if !finished {
                ui.anim = true;
            }
            if s.closing && finished {
                done_closing = true;
            }
            let rise = match s.kind {
                SKind::Modal | SKind::Popup => 12,
                SKind::Widget(_) => 16,
                SKind::Osd | SKind::EditBar | SKind::Dock | SKind::CurtainBadge => 8,
                SKind::Menu => 4,
                _ => 0,
            };
            ui.cv.opacity = (0.35 + 0.65 * k).min(1.0);
            ui.cv.oy = rise - ((1.0 - k) * rise as f32).round() as i32;
            let (w, h) = (s.w, s.h - rise);
            match &s.kind {
                SKind::Wall => crate::wall::draw(&mut ui, st, w, h),
                SKind::Bar => crate::bar::draw(&mut ui, st, &s.output, w),
                SKind::Dim => {
                    // затемнение цветом фона темы, а не чёрным: стол остаётся виден
                    let a = if matches!(st.modal, Some(crate::app::Modal::Settings)) { st.set.dim * 0.7 } else { st.set.dim };
                    ui.cv.opacity = 1.0;
                    ui.cv.oy = 0;
                    ui.cv.clear(st.t.bg.alpha(a * k));
                }
                SKind::Catcher => {}
                SKind::Modal => match st.modal {
                    Some(Modal::Launcher) => crate::launcher::draw(&mut ui, st, w, h),
                    Some(Modal::Power) => crate::power::draw(&mut ui, st, w, h),
                    Some(Modal::Settings) => crate::settings::draw(&mut ui, st, w, h),
                    Some(Modal::Auth) => crate::auth::draw(&mut ui, st, w, h),
                    Some(Modal::Screen) => crate::screen::draw(&mut ui, st, w, h),
                    None => {}
                },
                SKind::Popup => crate::popups::draw(&mut ui, st, w, h),
                SKind::Menu => crate::menu::draw(&mut ui, st, w, h),
                SKind::Notifs => crate::notif::draw(&mut ui, st, w, h),
                SKind::Osd => crate::notif::draw_osd(&mut ui, st, w, h),
                SKind::Widget(wid) => crate::widgets::draw(&mut ui, st, *wid, w, h),
                SKind::Title(win) => crate::titles::draw(&mut ui, st, *win, w, h),
                SKind::EditBar => crate::widgets::draw_editbar(&mut ui, st, w, h),
                SKind::Snap => crate::snap::draw(&mut ui, st, &s.output, w, h),
                SKind::Dock => crate::dock::draw(&mut ui, st, w, h),
                SKind::DockEdge => {}
                SKind::Curtain => crate::curtain::draw(&mut ui, st, &s.output, w, h),
                SKind::CurtainBadge => crate::curtain::draw_badge(&mut ui, w, h),
            }
            if k < 1.0 && !matches!(s.kind, SKind::Dim | SKind::Catcher) {
                ui.cv.dissolve(k, 2);
            }
            // размытие — когда панель проявилась целиком, иначе сквозь клетки видно пятно
            blur = if !st.set.blur_shell || !finished || s.closing {
                vec![]
            } else if let SKind::Title(_) = s.kind {
                s.input.clone().unwrap_or_else(|| vec![R::new(0, 0, s.w, s.h)])
            } else {
                std::mem::take(&mut ui.cv.blur)
            };
            s.zones = std::mem::take(&mut ui.zones);
            s.anim = ui.anim;
        }
        if up {
            let dst = unsafe { std::slice::from_raw_parts_mut(out_ptr, out_len) };
            crate::gfx::upscale(&scratch, bw, bh, dst, pw, ph);
        }
        // большой холст (обои, скриншот) не держим
        self.scratch = if scratch.capacity() > 1 << 20 { Vec::new() } else { scratch };
        let surf = s.layer.wl_surface().clone();
        if let Some(vp) = &s.viewport {
            vp.set_destination(s.w.max(1), s.h.max(1));
        }
        surf.damage_buffer(0, 0, pw, ph);
        // поверхность ввода (заголовки окон: только видимые куски)
        if s.input != s.input_set {
            if let Ok(r) = Region::new(&self.compositor) {
                if let Some(rects) = &s.input {
                    for q in rects {
                        r.add(q.x, q.y, q.w, q.h);
                    }
                    surf.set_input_region(Some(r.wl_region()));
                } else {
                    surf.set_input_region(None);
                }
            }
            s.input_set = s.input.clone();
        }
        if blur != s.blur_set {
            if let Some(mgr) = &self.fx_mgr {
                let fx = s.fx.get_or_insert_with(|| mgr.get_background_effect(&surf, &self.qh, ()));
                if blur.is_empty() {
                    fx.set_blur_region(None);
                } else if let Ok(r) = Region::new(&self.compositor) {
                    for q in &blur {
                        r.add(q.x, q.y, q.w, q.h);
                    }
                    fx.set_blur_region(Some(r.wl_region()));
                }
            }
            s.blur_set = blur;
        }
        let wants_frame = s.anim || s.kind == SKind::Wall;
        if wants_frame {
            surf.frame(&self.qh, surf.clone());
            s.frame_pending = true;
        }
        let _ = buffer.attach_to(&surf);
        s.layer.commit();
        // обои и шторка рисуются один раз: буфер у композитора, свой не держим
        if matches!(s.kind, SKind::Wall | SKind::Curtain) {
            s.release_pool = true;
        }
        if done_closing {
            return;
        }
        let curtain = (s.kind == SKind::Curtain).then(|| s.output.clone());
        self.surfs.insert(id, s);
        if let Some(out) = curtain {
            self.curtain_drawn(id, &out);
        }
    }

    // ── блокировка ──────────────────────────────────────────────────────────
    pub fn lock_now(&mut self) {
        if self.lock.is_some() || self.st.locked {
            return;
        }
        self.close_all();
        self.st.locked = true;
        self.st.lock_field.wipe();
        self.st.lock_status.clear();
        match self.lock_state.lock(&self.qh) {
            Ok(l) => {
                self.lock = Some(l);
                self.sync_lock_surfs();
            }
            Err(_) => self.st.locked = false,
        }
    }

    /// экран блокировки на каждом мониторе, и на подключённом уже под блокировкой. Номера
    /// поверхностей — индексы в lock_surfs, поэтому ушедшие мониторы не вынимаются до разблокировки
    fn sync_lock_surfs(&mut self) {
        let Some(l) = self.lock.as_ref() else { return };
        for o in self.outputs.outputs().collect::<Vec<_>>() {
            if self.lock_surfs.iter().any(|ls| ls.wl == o) || self.lock_surfs.len() >= 64 {
                continue;
            }
            let name = self.outputs.info(&o).and_then(|i| i.name.clone()).unwrap_or_default();
            let surface = self.compositor.create_surface(&self.qh);
            let sid = LOCK_BASE + self.lock_surfs.len() as u32;
            let frac = self.frac_mgr.as_ref().map(|m| m.get_fractional_scale(&surface, &self.qh, sid));
            let vp = self.viewporter.as_ref().map(|v| v.get_viewport(&surface, &self.qh, ()));
            let ls = l.create_lock_surface(surface, &o, &self.qh);
            self.lock_surfs.push(LockSurf { s: ls, wl: o, output: name, w: 0, h: 0, pool: None, zones: vec![], dirty: false, scale: 1.0, frac, vp });
        }
    }
    pub fn unlock(&mut self) {
        if let Some(l) = self.lock.take() {
            l.unlock();
        }
        self.drop_lock_surfs();
        self.st.locked = false;
        self.st.lock_field.wipe();
        let _ = self.conn.roundtrip();
        self.dirty_all();
    }

    /// поверхности блокировки — со своими масштабом и viewport
    fn drop_lock_surfs(&mut self) {
        for l in self.lock_surfs.drain(..) {
            if let Some(f) = l.frac {
                f.destroy();
            }
            if let Some(v) = l.vp {
                v.destroy();
            }
        }
    }

    fn draw_lock(&mut self, i: usize) {
        let main = self.lock_surfs[i].output == self.focus_out() || self.lock_surfs.len() == 1;
        let ls = &mut self.lock_surfs[i];
        ls.dirty = false;
        let (w, h) = (ls.w, ls.h);
        if w <= 0 || h <= 0 {
            return;
        }
        let (pw, ph) = if ls.vp.is_none() || ls.scale == 1.0 { (w, h) } else { crate::gfx::phys(w, h, ls.scale) };
        let up = (pw, ph) != (w, h);
        let need = (pw * ph * 4) as usize;
        if ls.pool.as_ref().map(|p| p.len() < need).unwrap_or(true) {
            ls.pool = SlotPool::new(need * 2, &self.shm).ok();
        }
        let Some(pool) = ls.pool.as_mut() else { return };
        let Ok((buffer, bytes)) = pool.create_buffer(pw, ph, pw * 4, wl_shm::Format::Argb8888) else { return };
        let (out_ptr, out_len) = (bytes.as_mut_ptr() as *mut u32, bytes.len() / 4);
        let mut scratch: Vec<u32> = if up { vec![0; (w * h) as usize] } else { vec![] };
        let buf: &mut [u32] = if up { &mut scratch } else { unsafe { std::slice::from_raw_parts_mut(out_ptr, out_len) } };
        let sid = LOCK_BASE + i as u32;
        let ptr = self.ptr.filter(|p| p.0 == sid).map(|p| (p.1, p.2));
        let mut ui = Ui {
            cv: Canvas::new(buf, w, h),
            f: &mut self.res.fonts,
            ic: &mut self.res.icons,
            t: self.st.t,
            zones: vec![],
            ptr,
            down: false,
            anim: false,
            blink: self.st.blink,
            tr: crate::i18n::tr,
        };
        crate::lock::draw(&mut ui, &self.st, w, h, main);
        let anim = ui.anim;
        ls.zones = std::mem::take(&mut ui.zones);
        drop(ui);
        if up {
            crate::gfx::upscale(&scratch, w, h, unsafe { std::slice::from_raw_parts_mut(out_ptr, out_len) }, pw, ph);
        }
        let surf = ls.s.wl_surface().clone();
        if let Some(vp) = &ls.vp {
            vp.set_destination(w, h);
        }
        surf.damage_buffer(0, 0, pw, ph);
        if anim {
            surf.frame(&self.qh, surf.clone());
        }
        let _ = buffer.attach_to(&surf);
        surf.commit();
    }

    // ── тик: часы, датчики, файл настроек ───────────────────────────────────
    fn tick_interval(&self) -> Duration {
        let fast = !self.st.notifs.popups.is_empty() || self.st.osd.is_some() || self.st.field_focus.is_some() || self.st.locked || self.st.modal == Some(Modal::Launcher);
        let fast = fast || self.st.upd.busy.is_some() || self.st.snap.is_some() || self.st.snap_countdown.is_some() || self.st.dock_leave.is_some() || !self.st.dock_bounce.is_empty();
        Duration::from_millis(if fast { 100 } else { 1000 })
    }

    /// батарея садится: одно уведомление на 10% и одно на 5%, пока не поставят на зарядку
    fn bat_check(&mut self) {
        let Some(b) = self.st.sys.bat else { return };
        if b.charging || b.plugged {
            self.st.bat_warn = 0;
            return;
        }
        let lvl = if b.pct <= 5 { 2 } else if b.pct <= 10 { 1 } else { 0 };
        if lvl > self.st.bat_warn {
            self.st.bat_warn = lvl;
            let left = b.mins.map(|m| crate::tf!("осталось {} мин", "{} min left", m)).unwrap_or_default();
            self.toast_note(&crate::tf!("батарея {}%", "battery {}%", b.pct), if left.is_empty() { "подключи зарядку" } else { &left });
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        // мигание курсора ввода — раз в полсекунды
        let blink = (self.st.started.elapsed().as_millis() / 500) % 2 == 0;
        if blink != self.st.blink {
            self.st.blink = blink;
            if self.st.field_focus.is_some() || self.st.locked || self.st.modal == Some(Modal::Launcher) {
                self.dirty(&SKind::Modal);
                self.dirty_where(|k| matches!(k, SKind::Widget(_) | SKind::Popup));
                for l in self.lock_surfs.iter_mut() {
                    l.dirty = true;
                }
            }
        }
        if now.duration_since(self.last_sample) >= Duration::from_millis(990) {
            self.last_sample = now;
            self.st.sys.want_gpu = self.uses("gpu");
            self.st.sys.sample();
            self.bat_check();
            self.st.rss = crate::sys::self_rss_mb();
            let lt = local_now();
            let sec_needed = self.st.set.clock_seconds || self.st.set.widgets.iter().any(|w| w.on && (w.kind == "clock" || w.kind == "graph" || w.kind == "net"));
            if sec_needed || lt.min != self.last_min {
                self.last_min = lt.min;
                self.dirty_where(|k| matches!(k, SKind::Bar | SKind::Widget(_) | SKind::Popup));
                for l in self.lock_surfs.iter_mut() {
                    l.dirty = true;
                }
            } else {
                self.dirty(&SKind::Bar);
            }
            self.check_settings_file();
            if self.st.started.elapsed().as_secs() % 60 == 0 {
                self.update_tick();
            }
            // профиль могли сменить снаружи (powerprofilesctl, другой апплет)
            if self.st.started.elapsed().as_secs() % 15 == 0 && self.bar_has("profile") {
                crate::profile::refresh(None, self.tx.clone());
            }
        }
        // уведомления и всплывашка громкости гаснут сами
        self.update_log_tick();
        if self.notif_tick() {
            self.sync_notifs();
        }
        self.snap_tick();
        self.dock_tick();
        self.disp_tick();
        // отсчёт таймера скриншота держит табличку
        if let Some((t0, d)) = self.st.snap_countdown {
            if t0.elapsed().as_secs() < d as u64 {
                self.st.osd = Some(("snap", Instant::now()));
                self.dirty(&SKind::Osd);
            }
        }
        if let Some((_, t)) = self.st.osd {
            if t.elapsed() > Duration::from_millis(1500) {
                self.st.osd = None;
                self.close_kind(&SKind::Osd);
            }
        }
    }

    /// какой-то модуль или виджет показывает этот источник
    pub fn bar_has(&self, m: &str) -> bool {
        let s = &self.st.set;
        s.bar_left.iter().chain(&s.bar_center).chain(&s.bar_right).any(|x| x == m)
    }

    fn uses(&self, src: &str) -> bool {
        let s = &self.st.set;
        s.widgets.iter().any(|w| w.on && w.kind == "graph" && w.source == src) || (src == "gpu" && self.st.popup.as_ref().map(|p| p.kind == "sys").unwrap_or(false))
    }

    // ── настройки ───────────────────────────────────────────────────────────
    pub fn settings_changed(&mut self) {
        let s = &self.st.set;
        crate::i18n::set(&s.lang);
        if s.font_shell != self.res.font {
            self.res.font = s.font_shell.clone();
            self.res.fonts = Fonts::with(&self.res.font);
        }
        self.st.t = Theme::new(s.accent_rgb(), s.tint, s.glass, s.lift, s.inner);
        self.res.icons.set_pixel(s.icon_pixel.then(|| self.st.t.icon_ramp()));
        self.write_palette();
        self.write_fx();
        crate::agents::write_portals(&self.st.set);
        self.sync_wall();
        crate::keys::write(&self.st.set);
        self.setup_idle();
        self.sync_widgets();
        self.sync_dock();
        self.dirty_all();
    }

    /// записать настройки сейчас (перед перезапуском), даже посреди перетаскивания
    pub fn save_now(&mut self) {
        let j = self.st.set.to_json();
        if j != self.set_saved {
            let _ = config::write_atomic(&config::settings_path(), &j);
            self.set_saved = j;
        }
    }

    fn save_settings_if_changed(&mut self) {
        if self.press.is_some() {
            return;
        }
        let j = self.st.set.to_json();
        if j != self.set_saved {
            let _ = config::write_atomic(&config::settings_path(), &j);
            self.set_saved = j;
            self.set_mtime = std::fs::metadata(config::settings_path()).and_then(|m| m.modified()).ok();
        }
    }

    fn check_settings_file(&mut self) {
        let m = std::fs::metadata(config::settings_path()).and_then(|m| m.modified()).ok();
        if m != self.set_mtime {
            self.set_mtime = m;
            let s = Settings::load();
            if s != self.st.set {
                self.st.set = s;
                self.set_saved = self.st.set.to_json();
                self.settings_changed();
            }
        }
    }

    /// палитра для остальных программ; tools/theme.py перекрашивает их и рисует обои
    pub fn write_palette(&mut self) {
        let j = self.st.t.palette_json();
        if j == self.palette_written {
            return;
        }
        self.palette_written = j.clone();
        let p = config::state_dir().join("palette.json");
        let changed = std::fs::read_to_string(&p).map(|t| t != j).unwrap_or(true);
        let _ = config::write_atomic(&p, &j);
        if changed || !config::state_dir().join("wallpaper.png").exists() {
            let tx = self.tx.clone();
            std::thread::spawn(move || {
                let script = crate::app::theme_script();
                let _ = std::process::Command::new("python3").arg("-I").arg(&script).arg(&p).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
                let _ = tx.send(Msg::Theme);
            });
        }
    }

    /// курсор сменили в настройках: theme.py --cursor (niri перечитает свой конфиг сам)
    pub fn apply_cursor(&mut self) {
        // настройки должны быть на диске до запуска скрипта
        let j = self.st.set.to_json();
        let _ = config::write_atomic(&config::settings_path(), &j);
        self.set_saved = j;
        std::thread::spawn(|| {
            let _ = std::process::Command::new("python3").arg("-I").arg(crate::app::theme_script()).arg("--cursor").stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
        });
    }

    /// размер шрифта программ сменили (или экраны): theme.py --fonts — GTK, Qt, kitty, браузер
    pub fn apply_fonts(&mut self) {
        let j = self.st.set.to_json();
        let _ = config::write_atomic(&config::settings_path(), &j);
        self.set_saved = j;
        std::thread::spawn(|| {
            let _ = std::process::Command::new("python3").arg("-I").arg(crate::app::theme_script()).arg("--fonts").stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
        });
    }

    pub fn write_fx(&mut self) {
        let _ = config::write_atomic(&config::config_dir().join("niri/yami-fx.kdl"), &self.st.set.fx_kdl(self.st.niri.level, self.st.curtain.is_some()));
    }

    // ── простой: замок и гашение экрана ─────────────────────────────────────
    fn setup_idle(&mut self) {
        let cfg = (self.st.set.lock_after, self.st.set.screen_off_after);
        if cfg == self.idle_cfg && !self.idle_notes.is_empty() {
            return;
        }
        self.idle_cfg = cfg;
        for n in self.idle_notes.drain(..) {
            n.destroy();
        }
        let (Some(idle), Some(seat)) = (self.idle.clone(), self.seat.seats().next()) else { return };
        if cfg.0 > 0 {
            self.idle_notes.push(idle.get_idle_notification(cfg.0 * 60_000, &seat, &self.qh, 0));
        }
        if cfg.1 > 0 {
            self.idle_notes.push(idle.get_idle_notification(cfg.1 * 60_000, &seat, &self.qh, 1));
        }
    }

    // ── сообщения нитей ─────────────────────────────────────────────────────
    fn on_msg(&mut self, m: Msg) {
        match m {
            Msg::Niri(v) => {
                let layout_only = v.get("WindowLayoutsChanged").is_some();
                let had_windows = !self.st.niri.wins.is_empty();
                let evs = self.st.niri.handle(v);
                for e in evs {
                    match e {
                        niri::Ev::KbSwitched => self.show_osd("layout"),
                        niri::Ev::NewWindow(id) => self.on_new_window(id),
                        niri::Ev::Changed => {}
                    }
                }
                // «стол» при входе: окна пришли — делаем их плавающими
                if !had_windows && !self.st.niri.wins.is_empty() && self.st.niri.stacking {
                    let on = self.st.niri.stacking;
                    self.st.niri.set_stacking(on);
                }
                self.game_sync();
                self.sync_titles();
                if !layout_only {
                    self.sync_dock();
                    self.dirty(&SKind::Dock);
                    self.dirty_where(|k| matches!(k, SKind::Bar | SKind::Menu | SKind::Popup | SKind::Widget(_)));
                }
            }
            Msg::Ipc(cmd) => self.ipc(&cmd),
            Msg::Audio(a) => {
                let touched = self.st.audio.ready && (a.volume != self.st.audio.volume || a.muted != self.st.audio.muted);
                self.st.audio = a;
                if touched && self.st.started.elapsed() > Duration::from_millis(2500) {
                    self.show_osd("volume");
                }
                self.dirty_where(|k| matches!(k, SKind::Bar | SKind::Popup | SKind::Osd));
            }
            Msg::Viz(v) => {
                self.st.viz = v;
                self.dirty_where(|k| matches!(k, SKind::Popup | SKind::Widget(_) | SKind::Bar));
            }
            Msg::Media(m) => {
                if !m.art.is_empty() && !self.st.art.contains_key(&m.art) {
                    // обложки прошлых треков не копим: за долгий вечер с плеером их были бы тысячи
                    if self.st.art.len() >= 8 {
                        let cur = self.st.media.art.clone();
                        self.st.art.retain(|k, _| *k == cur);
                    }
                    self.st.art.insert(m.art.clone(), None);
                    media::fetch_art(m.art.clone(), self.st.set.media_art_net, self.tx.clone());
                }
                self.st.media = m;
                media::viz_wanted(self.media_visible() && self.st.media.playing());
                self.dirty_where(|k| matches!(k, SKind::Bar | SKind::Popup | SKind::Widget(_)));
            }
            Msg::Art(url, img) => {
                self.st.art.insert(url, img.map(std::rc::Rc::new));
                self.dirty_where(|k| matches!(k, SKind::Popup | SKind::Widget(_)));
            }
            Msg::Notif(n) => self.on_notif(n),
            Msg::Update(i) => self.on_update(i),
            Msg::Snap(imgs, wins, mode) => self.on_snap_ready(imgs, wins, mode),
            Msg::SnapDone { path, copied, err, thumb } => self.on_snap_done(path, copied, err, thumb),
            Msg::NotifClose(id) => {
                self.st.notifs.remove(id, false);
                self.sync_notifs();
            }
            Msg::Tray(e) => {
                dbus::apply(&mut self.st.tray, e);
                self.dirty(&SKind::Bar);
            }
            Msg::TrayMenu(svc, items) => self.open_tray_menu(&svc, items),
            Msg::Net(n) => {
                // сети не хватило пароля — поле под ней, курсор в нём
                if !n.ask.is_empty() && self.st.popup.as_ref().is_some_and(|p| p.kind == "net") {
                    self.st.wifi_target = n.ask.clone();
                    self.st.wifi_pass.clear();
                    self.st.field_focus = Some("wifi");
                }
                self.st.net = n;
                self.resize_popup();
                self.dirty(&SKind::Popup);
            }
            Msg::Bt(mut b) => {
                // сигналы BlueZ не знают, что идёт сопряжение: его состояние держим до ответа
                if !b.op {
                    b.msg = std::mem::take(&mut self.st.bt.msg);
                    b.busy = std::mem::take(&mut self.st.bt.busy);
                }
                self.st.bt = b;
                self.resize_popup();
                self.dirty_where(|k| matches!(k, SKind::Popup | SKind::Bar));
            }
            Msg::Pam(ok) => self.on_pam(ok),
            Msg::Auth(e) => self.on_auth(e),
            Msg::Prof(p) => self.on_prof(p),
            Msg::ScreenPick(types, s) => self.screen_ask(types, s),
            Msg::Clip(lines) => {
                self.st.launcher.clip_lines = lines;
                self.st.launcher.refresh(&self.st.apps);
                self.dirty(&SKind::Modal);
            }
            Msg::Wall(n, img, accent) => self.wall_ready(n, img, accent),
            Msg::Curtain(n, imgs) => self.curtain_ready(n, imgs),
            Msg::Theme => {
                self.sync_wall();
                self.res.icons.drop_cache();
            }
        }
    }

    pub fn media_visible(&self) -> bool {
        self.st.popup.as_ref().map(|p| p.kind == "sound").unwrap_or(false)
            || self.st.set.widgets.iter().any(|w| w.on && w.kind == "media")
            || self.st.set.bar_left.iter().chain(&self.st.set.bar_center).chain(&self.st.set.bar_right).any(|m| m == "media")
    }

    fn on_new_window(&mut self, id: u64) {
        // в режиме «стол»: под заголовок нужно место сверху
        self.fit_title(id);
    }

    pub fn show_osd(&mut self, kind: &'static str) {
        if self.st.locked {
            return;
        }
        let fresh = self.st.osd.is_none();
        self.st.osd = Some((kind, Instant::now()));
        if fresh || self.find(&SKind::Osd).is_empty() {
            let out = self.focus_out();
            let id = self.mk(SKind::Osd, &out, Layer::Overlay, "yami-osd", Anchor::BOTTOM, (308, 64 + 8), (0, 0, 64, 0), -1, KeyboardInteractivity::None);
            if let Some(s) = self.surfs.get(&id) {
                if let Ok(r) = Region::new(&self.compositor) {
                    s.layer.wl_surface().set_input_region(Some(r.wl_region()));
                }
            }
        }
        self.dirty(&SKind::Osd);
    }

    // ── команды (IPC) ───────────────────────────────────────────────────────
    pub fn ipc(&mut self, cmd: &str) {
        match cmd.trim() {
            "launcher" => self.toggle_modal(Modal::Launcher, false),
            "clipboard" => self.toggle_modal(Modal::Launcher, true),
            "power" => self.toggle_modal(Modal::Power, false),
            "settings" => self.toggle_modal(Modal::Settings, false),
            "lock" => self.lock_now(),
            "bar" => self.toggle_bar(),
            "stacking" => {
                let on = !self.st.niri.stacking;
                self.st.niri.set_stacking(on);
                self.st.set.stacking = on;
                self.sync_titles();
                self.dirty_all();
            }
            "minimize" => {
                if let Some(id) = self.st.niri.focused_win().map(|w| w.id) {
                    self.st.niri.minimize(id);
                }
            }
            "restore" => self.st.niri.restore_last(),
            "edit" => {
                self.close_all();
                self.set_editing(!self.st.editing);
            }
            "reload" => {
                self.st.set = Settings::load();
                self.st.apps = icons::load_apps();
                self.settings_changed();
            }
            "quit" => self.quit = true,
            // «жив ли шелл» для install.sh и deploy.sh: ответ сокета, ничего не делает
            "ping" => {}
            "restart" => self.restart = true,
            c if self.update_ipc(c) => {}
            c if c.starts_with("settings ") => self.open_settings_tab(c[9..].trim()),
            c if c == "screenshot" || c.starts_with("screenshot ") => {
                let mode = match c.split_whitespace().nth(1) {
                    Some("window") => crate::snap::Mode::Window,
                    Some("screen") => crate::snap::Mode::Screen,
                    _ => crate::snap::Mode::Area,
                };
                let delay = c.split_whitespace().nth(2).and_then(|d| d.parse().ok()).unwrap_or(0);
                if self.st.snap.is_some() {
                    self.snap_close();
                } else {
                    self.snap_start(mode, delay);
                }
            }
            // следующий профиль питания (для хоткея)
            "profile" => self.prof_step(1),
            c if c.starts_with("popup ") => {
                let kind: &'static str = match &c[6..] {
                    "cal" => "cal",
                    "sound" => "sound",
                    "net" => "net",
                    "bt" => "bt",
                    "profile" => "profile",
                    _ => "sys",
                };
                let out = self.focus_out();
                let (ow, _) = self.output_size(&out);
                self.open_popup(kind, &out, ow - 260);
            }
            // свои обои: путь; без пути — пиксельные из палитры
            c if c == "wallpaper" || c.starts_with("wallpaper ") => self.set_wall(&c["wallpaper".len()..]),
            c if c == "curtain" || c.starts_with("curtain ") => self.curtain_cmd(&c["curtain".len()..]),
            _ => {}
        }
    }

    /// настройки на вкладке (`yami open settings:keys`, карточка обновления)
    pub fn open_settings_tab(&mut self, tab: &str) {
        let Some(t) = crate::settings::TABS.iter().find(|x| x.0 == tab).map(|x| x.0) else { return };
        self.st.stab = t;
        if self.st.modal != Some(Modal::Settings) {
            self.toggle_modal(Modal::Settings, false);
        }
        self.dirty(&SKind::Modal);
    }

    pub fn close_all(&mut self) {
        self.close_modal();
        self.close_popup();
        self.close_menu();
    }

    // ── модальные панели: лаунчер, питание, настройки ───────────────────────
    pub fn toggle_modal(&mut self, m: Modal, clip: bool) {
        // окно пароля не уступает место другим панелям
        if self.st.locked || matches!(self.st.modal, Some(Modal::Auth | Modal::Screen)) {
            return;
        }
        let same = self.st.modal == Some(m) && (m != Modal::Launcher || self.st.launcher.clip == clip);
        if self.st.modal.is_some() && !same && m == Modal::Launcher && self.st.modal == Some(Modal::Launcher) {
            // переключение программы ↔ буфер без закрытия
            self.st.launcher.set_mode(clip, &self.st.apps);
            if clip {
                launcher::load_clip(self.tx.clone());
            }
            self.dirty(&SKind::Modal);
            return;
        }
        self.close_all();
        if same {
            return;
        }
        let out = self.focus_out();
        let (w, h) = match m {
            Modal::Launcher => launcher::SIZE,
            Modal::Power => crate::power::SIZE,
            Modal::Settings => crate::settings::SIZE,
            Modal::Auth => crate::auth::SIZE,
            Modal::Screen => (0, 0),
        };
        self.st.modal = Some(m);
        match m {
            Modal::Launcher => {
                self.st.apps = icons::load_apps();
                self.st.launcher.open(clip, &self.st.apps);
                if clip {
                    launcher::load_clip(self.tx.clone());
                }
            }
            Modal::Power => self.st.power_cur = 0,
            Modal::Auth | Modal::Screen => {}
            Modal::Settings => {
                self.st.hex.set(&self.st.set.accent.clone());
                if self.st.sel_widget == 0 {
                    self.st.sel_widget = self.st.set.widgets.first().map(|w| w.id).unwrap_or(0);
                }
            }
        }
        self.mk(SKind::Dim, &out, Layer::Overlay, "yami-dim", Anchor::all(), (0, 0), (0, 0, 0, 0), -1, KeyboardInteractivity::None);
        self.mk(SKind::Modal, &out, Layer::Overlay, match m {
            Modal::Launcher => "yami-launcher",
            Modal::Power => "yami-power",
            Modal::Settings => "yami-settings",
            Modal::Auth => "yami-auth",
            Modal::Screen => "yami-screen",
        }, Anchor::empty(), (w, h + 12), (0, 0, 0, 0), -1, KeyboardInteractivity::Exclusive);
    }

    /// все клавиши — шеллу (запись сочетания): niri не ловит свои бинды, пока фокус у настроек
    pub fn inhibit_shortcuts(&mut self, on: bool) {
        if let Some(i) = self.ksi.take() {
            i.destroy();
        }
        if !on {
            return;
        }
        let surf = self.surfs.values().find(|s| s.kind == SKind::Modal).map(|s| s.layer.wl_surface().clone());
        let seat = self.seat.seats().next();
        if let (Some(mgr), Some(surf), Some(seat)) = (&self.ksi_mgr, surf, seat) {
            self.ksi = Some(mgr.inhibit_shortcuts(&surf, &seat, &self.qh, ()));
        }
    }

    pub fn close_modal(&mut self) {
        if self.st.modal.is_none() {
            return;
        }
        self.keys_capture(None);
        if self.st.modal == Some(Modal::Auth) {
            self.auth_cancel();
        }
        // окно выбора экрана закрыли мимо кнопок — порталу «отмена»
        if self.st.modal == Some(Modal::Screen) {
            if let Some(p) = self.st.screen_pick.take() {
                use std::io::Write;
                let _ = (&p.stream).write_all(b"cancel\n");
            }
        }
        self.st.modal = None;
        self.st.field_focus = None;
        self.close_kind(&SKind::Modal);
        self.close_kind(&SKind::Dim);
    }

    // ── всплывашки бара ─────────────────────────────────────────────────────
    pub fn open_popup(&mut self, kind: &'static str, out: &str, ax: i32) {
        let same = self.st.popup.as_ref().map(|p| p.kind == kind).unwrap_or(false);
        self.close_popup();
        self.close_menu();
        if same {
            return;
        }
        self.close_modal();
        self.st.popup = Some(Popup { kind });
        match kind {
            "net" => net::refresh_wifi(self.tx.clone()),
            "bt" => net::refresh_bt(self.tx.clone()),
            "profile" => crate::profile::refresh(None, self.tx.clone()),
            "cal" => self.st.cal_month = 0,
            _ => {}
        }
        media::viz_wanted(self.media_visible() && self.st.media.playing());
        let (w, h) = popups::size(kind, &self.st);
        let (ow, _) = self.output_size(out);
        let x = (ax - w / 2).clamp(6, (ow - w - 6).max(6));
        self.mk_catcher(out);
        self.mk(SKind::Popup, out, Layer::Top, "yami-popup", Anchor::TOP | Anchor::LEFT, (w, h + 12), (if self.st.bar_hidden { 6 } else { 4 }, 0, 0, x), -1, KeyboardInteractivity::OnDemand);
    }

    pub fn resize_popup(&mut self) {
        let Some(p) = &self.st.popup else { return };
        let (w, h) = popups::size(p.kind, &self.st);
        for s in self.surfs.values_mut() {
            if s.kind == SKind::Popup && !s.closing && (s.w != w || s.h != h + 12) {
                s.layer.set_size(w as u32, (h + 12) as u32);
                s.layer.commit();
            }
        }
    }

    pub fn close_popup(&mut self) {
        if self.st.popup.take().is_some() {
            self.st.field_focus = None;
            self.close_kind(&SKind::Popup);
            if self.st.menu.is_none() {
                self.close_kind(&SKind::Catcher);
            }
            media::viz_wanted(self.media_visible() && self.st.media.playing());
        }
    }

    /// прозрачный ловец кликов мимо всплывашки (бар остаётся живым)
    pub fn mk_catcher(&mut self, out: &str) {
        if self.find(&SKind::Catcher).is_empty() {
            let id = self.mk(SKind::Catcher, out, Layer::Top, "yami-catch", Anchor::all(), (0, 0), (0, 0, 0, 0), -1, KeyboardInteractivity::None);
            let (w, h) = self.output_size(out);
            let top = if self.st.bar_hidden { 0 } else { BAR_H };
            if let Some(s) = self.surfs.get_mut(&id) {
                s.input = Some(vec![R::new(0, top, w, h - top)]);
            }
        }
    }

    // ── меню ────────────────────────────────────────────────────────────────
    pub fn open_menu(&mut self, m: menu::MenuState) {
        self.close_menu();
        let out = m.output.clone();
        let (w, h) = menu::size(&m, &mut self.res.fonts);
        let (ow, oh) = self.output_size(&out);
        let x = m.x.clamp(4, (ow - w - 8).max(4));
        let y = m.y.clamp(4, (oh - h - 8).max(4));
        self.st.menu = Some(m);
        self.mk_catcher(&out);
        self.mk(SKind::Menu, &out, Layer::Overlay, "yami-menu", Anchor::TOP | Anchor::LEFT, (w, h + 4), (y, 0, 0, x), -1, KeyboardInteractivity::OnDemand);
    }
    pub fn close_menu(&mut self) {
        if self.st.menu.take().is_some() {
            self.close_kind(&SKind::Menu);
            if self.st.popup.is_none() {
                self.close_kind(&SKind::Catcher);
            }
        }
    }

    // ── ввод ────────────────────────────────────────────────────────────────
    fn zone_at(&self, sid: u32, x: f64, y: f64) -> Option<Zone> {
        let zones = if sid >= LOCK_BASE {
            self.lock_surfs.get((sid - LOCK_BASE) as usize).map(|l| &l.zones)
        } else {
            self.surfs.get(&sid).map(|s| &s.zones)
        }?;
        zones.iter().rev().find(|z| z.r.contains(x, y)).cloned()
    }

    fn set_cursor(&mut self, c: Cur) {
        let shape = match c {
            Cur::Default => Shape::Default,
            Cur::Pointer => Shape::Pointer,
            Cur::Grab => Shape::Grabbing,
            Cur::Text => Shape::Text,
        };
        if self.cursor_now == Some(shape) {
            return;
        }
        self.cursor_now = Some(shape);
        if let Some(d) = &self.cursor_dev {
            d.set_shape(self.enter_serial, shape);
        }
    }

    fn surf_of(&self, s: &wl_surface::WlSurface) -> Option<u32> {
        if let Some((i, _)) = self.surfs.iter().find(|(_, x)| x.layer.wl_surface() == s) {
            return Some(*i);
        }
        self.lock_surfs.iter().position(|l| l.s.wl_surface() == s).map(|i| LOCK_BASE + i as u32)
    }

    fn mark(&mut self, sid: u32) {
        if sid >= LOCK_BASE {
            if let Some(l) = self.lock_surfs.get_mut((sid - LOCK_BASE) as usize) {
                l.dirty = true;
            }
        } else if let Some(s) = self.surfs.get_mut(&sid) {
            s.dirty = true;
        }
    }

    pub fn surf_kind(&self, sid: u32) -> Option<SKind> {
        self.surfs.get(&sid).map(|s| s.kind.clone())
    }

    fn on_pointer(&mut self, e: &PointerEvent) {
        let Some(sid) = self.surf_of(&e.surface) else { return };
        let (x, y) = e.position;
        // скриншот: протяжка, рисование и лупа — мимо обычных зон
        if self.surf_kind(sid) == Some(SKind::Snap) {
            let on_btn = self.zone_at(sid, x, y).map(|z| matches!(z.act, Act::SnapBtn(_))).unwrap_or(false);
            let took = match e.kind {
                PointerEventKind::Motion { .. } => self.snap_pointer(sid, 0, x, y, 0, on_btn),
                PointerEventKind::Press { button, .. } => self.snap_pointer(sid, 1, x, y, button, on_btn),
                PointerEventKind::Release { button, .. } if self.press.is_none() => self.snap_pointer(sid, 2, x, y, button, on_btn),
                PointerEventKind::Axis { vertical, .. } => {
                    let d = if vertical.value120 != 0 { vertical.value120.signum() } else { vertical.discrete.signum() };
                    if d != 0 {
                        self.snap_wheel(d);
                    }
                    true
                }
                _ => false,
            };
            if took {
                self.ptr = Some((sid, x, y));
                return;
            }
        }
        match e.kind {
            PointerEventKind::Enter { serial } => {
                if matches!(self.surf_kind(sid), Some(SKind::Dock | SKind::DockEdge)) {
                    self.dock_hover(true);
                }
                self.enter_serial = serial;
                self.cursor_now = None;
                self.ptr = Some((sid, x, y));
                let c = self.zone_at(sid, x, y).map(|z| z.cur).unwrap_or(Cur::Default);
                self.set_cursor(c);
                self.mark(sid);
                if let Some(SKind::Notifs) = self.surf_kind(sid) {
                    self.st.notifs.hover = true;
                }
            }
            PointerEventKind::Leave { .. } => {
                if self.surf_kind(sid) == Some(SKind::Dock) {
                    self.dock_hover(false);
                }
                if self.ptr.map(|p| p.0) == Some(sid) {
                    self.ptr = None;
                }
                self.mark(sid);
                if let Some(SKind::Notifs) = self.surf_kind(sid) {
                    self.st.notifs.hover = false;
                    self.st.notifs.resume();
                }
            }
            PointerEventKind::Motion { .. } => {
                let before = self.ptr.and_then(|p| if p.0 == sid { self.zone_at(sid, p.1, p.2) } else { None });
                self.ptr = Some((sid, x, y));
                if self.press.is_some() {
                    self.on_drag(x, y);
                    return;
                }
                let after = self.zone_at(sid, x, y);
                let c = after.as_ref().map(|z| z.cur).unwrap_or(Cur::Default);
                self.set_cursor(c);
                if before.as_ref().map(|z| (&z.act, z.r)) != after.as_ref().map(|z| (&z.act, z.r)) {
                    self.mark(sid);
                    if let Some(z) = &after {
                        self.on_hover(&z.act);
                    }
                }
            }
            PointerEventKind::Press { button, .. } => {
                self.ptr = Some((sid, x, y));
                let z = self.zone_at(sid, x, y);
                let (sx, sy) = self.surfs.get(&sid).map(|s| s.applied).unwrap_or((0, 0));
                if let Some(z) = z {
                    // тащим от того места, где поверхность реально стоит
                    let start = (sx, sy);
                    self.press = Some(Press { sid, act: z.act.clone(), button, x, y, zone: z.r, ax: x + sx as f64, ay: y + sy as f64, moved: false, start, origin: (sx, sy) });
                    if z.act.is_drag() && button == 0x110 {
                        self.on_drag(x, y);
                    }
                    if z.act.is_drag() {
                        self.set_cursor(Cur::Grab);
                    }
                } else {
                    // клик мимо панели — по затемнению или ловцу: закрыть
                    match self.surf_kind(sid) {
                        Some(SKind::Dim) => self.close_modal(),
                        Some(SKind::Catcher) => {
                            self.close_popup();
                            self.close_menu();
                        }
                        _ => {}
                    }
                }
                self.mark(sid);
            }
            PointerEventKind::Release { button, .. } => {
                let p = self.press.take();
                self.mark(sid);
                if let Some(p) = p {
                    if p.act.is_drag() {
                        self.on_drag_end(&p);
                        if p.moved {
                            return;
                        }
                    }
                    let z = self.zone_at(sid, x, y);
                    if z.map(|z| z.act == p.act).unwrap_or(false) && p.button == button {
                        let double = matches!(&self.last_click, Some((a, t)) if *a == p.act && t.elapsed() < Duration::from_millis(350));
                        self.last_click = Some((p.act.clone(), Instant::now()));
                        self.on_click(p.act, button, sid, p.zone, double);
                    }
                }
            }
            PointerEventKind::Axis { vertical, .. } => {
                let d = if vertical.discrete != 0 { vertical.discrete as f64 } else if vertical.value120 != 0 { vertical.value120 as f64 / 120.0 } else { vertical.absolute / 15.0 };
                if d.abs() < 0.01 {
                    return;
                }
                if let Some(z) = self.zone_at(sid, x, y) {
                    self.on_wheel(z.act, if d > 0.0 { 1 } else { -1 });
                }
            }
        }
    }

    fn on_drag(&mut self, x: f64, y: f64) {
        let Some(p) = self.press.as_mut() else { return };
        if (x - p.x).abs() + (y - p.y).abs() > 3.0 {
            p.moved = true;
        }
        let sid = p.sid;
        let act = p.act.clone();
        let zone = p.zone;
        let (sx, sy) = p.origin;
        let ax = x + sx as f64;
        let ay = y + sy as f64;
        let (dx, dy) = (ax - p.ax, ay - p.ay);
        let start = p.start;
        let frac = ((x - zone.x as f64) / zone.w.max(1) as f64).clamp(0.0, 1.0) as f32;
        match act {
            Act::SSlider(key, from, to, seg) => {
                let f = (frac * seg as f32).ceil() / seg as f32;
                settings_set_f32(&mut self.st.set, key, from + f * (to - from));
                self.settings_changed();
            }
            Act::VolSlider => audio::set_volume(frac as f64 * 1.0),
            Act::MicSlider => audio::set_mic(frac as f64),
            Act::AppVol(i) => audio::set_stream(i, frac as f64),
            Act::TDrag(win) => {
                if p.moved {
                    self.title_drag(win, sid, start, dx, dy);
                }
                return;
            }
            Act::SModDrag(..) | Act::SModNew(_) => {
                if p.moved {
                    let press = (p.x, p.y);
                    self.bar_drag(&act, zone, press, x, y);
                }
                return;
            }
            Act::DispDrag(i) => {
                if p.moved {
                    self.disp_drag(i, dx, dy);
                }
                return;
            }
            Act::WDrag(id) => {
                if p.moved {
                    let snap = |v: f64| ((v / 2.0).round() * 2.0) as i32;
                    let nx = snap(start.0 as f64 + dx);
                    let ny = snap(start.1 as f64 + dy);
                    let (ow, oh) = self.surfs.get(&sid).map(|s| self.output_size(&s.output)).unwrap_or((1920, 1080));
                    let (ww, wh) = self.surfs.get(&sid).map(|s| (s.w, s.h)).unwrap_or((0, 0));
                    let nx = nx.clamp(0, (ow - ww).max(0));
                    let ny = ny.clamp(BAR_H, (oh - wh).max(BAR_H));
                    if let Some(w) = self.st.set.widget_mut(id) {
                        w.x = nx;
                        w.y = ny;
                    }
                    // двигаем только эту поверхность, без перерисовки и без записи настроек
                    self.move_surface(sid, nx, ny);
                }
                return;
            }
            _ => {}
        }
        self.mark(sid);
    }

    /// сдвинуть поверхность; положение считается применённым, когда композитор ответит на sync
    pub fn move_surface(&mut self, sid: u32, x: i32, y: i32) {
        let Some(s) = self.surfs.get_mut(&sid) else { return };
        if s.margin == (x, y) {
            return;
        }
        s.layer.set_margin(y, 0, 0, x);
        s.margin = (x, y);
        s.layer.commit();
        self.conn.display().sync(&self.qh, (sid, x, y));
    }

    fn on_drag_end(&mut self, p: &Press) {
        if let Act::TDrag(win) = p.act {
            self.title_drag_end(win);
        }
        if let Act::DispDrag(_) = p.act {
            self.disp_drag_end();
        }
        if matches!(p.act, Act::SModDrag(..) | Act::SModNew(_)) {
            self.bar_drop();
        }
        if let Act::SSlider(..) = p.act {
            // ползунок отпустили: теперь пересчитать обои (если это был их ползунок)
            self.sync_wall();
        }
    }

    fn on_hover(&mut self, a: &Act) {
        match a {
            Act::LRun(i) => {
                if self.st.launcher.cur != *i {
                    self.st.launcher.cur = *i;
                }
            }
            Act::PowerDo(i) => self.st.power_cur = *i,
            Act::Menu(i) => {
                if let Some(m) = self.st.menu.as_mut() {
                    m.cur = Some(*i);
                }
            }
            _ => {}
        }
    }

    // ── клавиатура ──────────────────────────────────────────────────────────
    fn on_key(&mut self, e: &KeyEvent) {
        if self.st.locked {
            self.lock_key(e);
            return;
        }
        let Some(sid) = self.kb_surf else { return };
        match self.surf_kind(sid) {
            Some(SKind::Modal) => match self.st.modal {
                Some(Modal::Launcher) => self.launcher_key(e),
                Some(Modal::Power) => self.power_key(e),
                Some(Modal::Settings) => self.settings_key(e),
                Some(Modal::Auth) => self.auth_key(e),
                Some(Modal::Screen) => self.screen_key(e),
                None => {}
            },
            Some(SKind::Popup) => self.popup_key(e),
            Some(SKind::Menu) => self.menu_key(e),
            Some(SKind::Widget(_)) => self.widget_key(e),
            Some(SKind::Snap) => {
                self.snap_key(e);
                return;
            }
            Some(SKind::EditBar) => {
                if matches!(e.keysym, Keysym::Escape | Keysym::Return) {
                    self.set_editing(false);
                }
            }
            _ => {}
        }
        self.mark(sid);
    }

    /// общая правка поля: true — клавиша поглощена
    pub fn edit_field(f: &mut Field, e: &KeyEvent, mods: &Modifiers) -> bool {
        match e.keysym {
            Keysym::BackSpace => f.backspace(mods.ctrl),
            Keysym::Delete => f.delete(),
            Keysym::Left => f.left(),
            Keysym::Right => f.right(),
            Keysym::Home => f.home(),
            Keysym::End => f.end(),
            _ => {
                if mods.ctrl {
                    if e.keysym == Keysym::u {
                        f.clear();
                        return true;
                    }
                    if e.keysym == Keysym::v {
                        if let Some(t) = yami_px::ui::paste() {
                            f.insert(&t);
                        }
                        return true;
                    }
                    return false;
                }
                match &e.utf8 {
                    Some(s) if !s.is_empty() && !s.chars().any(|c| c.is_control()) => f.insert(s),
                    _ => return false,
                }
            }
        }
        true
    }

    // ── запуск программ ─────────────────────────────────────────────────────
    pub fn spawn(argv: &[String]) {
        if argv.is_empty() {
            return;
        }
        // setsid -f: процесс отвязывается от шелла и не становится зомби
        let _ = std::process::Command::new("setsid")
            .arg("-f")
            .args(argv)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    pub fn sh(cmd: &str) {
        Self::spawn(&["sh".to_string(), "-c".to_string(), cmd.to_string()]);
    }
}

pub fn reveal_of(st: &State, s: &Surf, dur_ms: f32) -> (f32, bool) {
    if !st.set.animations {
        return (if s.closing { 0.0 } else { 1.0 }, true);
    }
    // закрытие быстрее открытия
    let dur = dur_ms * st.set.anim_speed.max(0.1) * if s.closing { 0.7 } else { 1.0 };
    let t = (s.born.elapsed().as_secs_f32() * 1000.0 / dur).min(1.0);
    // открытие: быстро и мягко садится (ease-out cubic); закрытие: разгоняется (ease-in)
    let k = if s.closing { 1.0 - t * t } else { 1.0 - (1.0 - t).powi(3) };
    (k, t >= 1.0)
}

pub fn settings_set_f32(s: &mut Settings, key: &str, v: f32) {
    match key {
        "tint" => s.tint = (v * 10.0).round() / 10.0,
        "glass" => s.glass = (v * 100.0).round() / 100.0,
        "lift" => s.lift = (v * 100.0).round() / 100.0,
        "inner" => s.inner = (v * 100.0).round() / 100.0,
        "dim" => s.dim = (v * 100.0).round() / 100.0,
        "anim_speed" => s.anim_speed = (v * 10.0).round() / 10.0,
        "float_w" => s.float_w = (v * 20.0).round() / 20.0,
        "dock_size" => s.dock_size = ((v / 4.0).round() * 4.0) as i32,
        "float_h" => s.float_h = (v * 20.0).round() / 20.0,
        "lock_after" => s.lock_after = v.round() as u32,
        "screen_off_after" => s.screen_off_after = v.round() as u32,
        "notif_timeout" => s.notif_timeout = v.round().max(2.0) as u32,
        "task_width" => s.task_width = (v / 10.0).round() as i32 * 10,
        "wall_pixel" => s.wall_pixel = v.round().max(1.0) as i32,
        "wall_dim" => s.wall_dim = (v * 20.0).round() / 20.0,
        "curtain_cell" => s.curtain_cell = ((v / 4.0).round() * 4.0) as i32,
        _ => {}
    }
}

pub fn settings_get_f32(s: &Settings, key: &str) -> f32 {
    match key {
        "tint" => s.tint,
        "glass" => s.glass,
        "lift" => s.lift,
        "inner" => s.inner,
        "dim" => s.dim,
        "anim_speed" => s.anim_speed,
        "float_w" => s.float_w,
        "dock_size" => s.dock_size as f32,
        "float_h" => s.float_h,
        "lock_after" => s.lock_after as f32,
        "screen_off_after" => s.screen_off_after as f32,
        "notif_timeout" => s.notif_timeout as f32,
        "task_width" => s.task_width as f32,
        "wall_pixel" => s.wall_pixel as f32,
        "wall_dim" => s.wall_dim,
        "curtain_cell" => s.curtain_cell as f32,
        _ => 0.0,
    }
}

pub fn theme_script() -> std::path::PathBuf {
    // tools/theme.py вшит в шелл и пишется рядом с состоянием
    let p = config::state_dir().join("theme.py");
    let src = include_str!("../../tools/theme.py");
    if std::fs::read_to_string(&p).map(|t| t != src).unwrap_or(true) {
        let _ = std::fs::write(&p, src);
    }
    p
}

// ── время без chrono ────────────────────────────────────────────────────────
#[derive(Clone, Copy, Debug)]
pub struct Tm {
    pub year: i32,
    pub mon: u32,
    pub day: u32,
    pub hour: u32,
    pub min: u32,
    pub sec: u32,
    pub wday: u32,
}

pub fn local_now() -> Tm {
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    local_tm(now)
}
pub fn local_tm(t: i64) -> Tm {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&t, &mut tm) };
    Tm { year: tm.tm_year + 1900, mon: tm.tm_mon as u32 + 1, day: tm.tm_mday as u32, hour: tm.tm_hour as u32, min: tm.tm_min as u32, sec: tm.tm_sec as u32, wday: tm.tm_wday as u32 }
}

pub const RU_DAY: [&str; 7] = ["вс", "пн", "вт", "ср", "чт", "пт", "сб"];
pub const RU_DAY_FULL: [&str; 7] = ["воскресенье", "понедельник", "вторник", "среда", "четверг", "пятница", "суббота"];
pub const RU_MON: [&str; 12] = ["января", "февраля", "марта", "апреля", "мая", "июня", "июля", "августа", "сентября", "октября", "ноября", "декабря"];
pub const RU_MON_NOM: [&str; 12] = ["январь", "февраль", "март", "апрель", "май", "июнь", "июль", "август", "сентябрь", "октябрь", "ноябрь", "декабрь"];

// ── обработчики Wayland ─────────────────────────────────────────────────────
impl CompositorHandler for App {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, surface: &wl_surface::WlSurface, _: u32) {
        let Some(sid) = self.surf_of(surface) else { return };
        if sid >= LOCK_BASE {
            self.mark(sid);
            return;
        }
        if let Some(s) = self.surfs.get_mut(&sid) {
            s.frame_pending = false;
            if s.release_pool {
                // обои отрисованы и показаны — память буфера больше не нужна
                s.release_pool = false;
                s.pool = None;
                return;
            }
            if s.anim {
                s.dirty = true;
            }
        }
    }
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.ensure_output_surfaces();
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.ensure_output_surfaces();
    }
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.ensure_output_surfaces();
    }
}

impl LayerShellHandler for App {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        let id = self.surfs.iter().find(|(_, s)| &s.layer == layer).map(|(i, _)| *i);
        if let Some(id) = id {
            self.destroy(id);
        }
    }
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface, c: LayerSurfaceConfigure, _: u32) {
        if let Some(s) = self.surfs.values_mut().find(|s| &s.layer == layer) {
            let (w, h) = c.new_size;
            if w > 0 {
                s.w = w as i32;
            }
            if h > 0 {
                s.h = h as i32;
            }
            s.configured = true;
            s.dirty = true;
            s.frame_pending = false;
        }
    }
}

impl SessionLockHandler for App {
    fn locked(&mut self, _: &Connection, _: &QueueHandle<Self>, _: SessionLock) {}
    fn finished(&mut self, _: &Connection, _: &QueueHandle<Self>, _: SessionLock) {
        self.lock = None;
        self.drop_lock_surfs();
        self.st.locked = false;
    }
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, surface: SessionLockSurface, c: SessionLockSurfaceConfigure, _: u32) {
        if let Some(l) = self.lock_surfs.iter_mut().find(|l| l.s.wl_surface() == surface.wl_surface()) {
            l.w = c.new_size.0 as i32;
            l.h = c.new_size.1 as i32;
            l.dirty = true;
        }
    }
}

impl SeatHandler for App {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {
        self.setup_idle();
    }
    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Keyboard && self.keyboard.is_none() {
            let h = self.handle.clone();
            self.keyboard = self
                .seat
                .get_keyboard_with_repeat(qh, &seat, None, h, Box::new(|app: &mut App, _, e| app.on_key(&e)))
                .ok();
        }
        if cap == Capability::Pointer && self.pointer.is_none() {
            if let Ok(p) = self.seat.get_pointer(qh, &seat) {
                if let Some(m) = &self.cursor_mgr {
                    self.cursor_dev = Some(m.get_shape_device(&p, qh));
                }
                self.pointer = Some(p);
            }
        }
    }
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Keyboard {
            if let Some(k) = self.keyboard.take() {
                k.release();
            }
        }
        if cap == Capability::Pointer {
            if let Some(p) = self.pointer.take() {
                p.release();
            }
            self.cursor_dev = None;
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for App {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, surface: &wl_surface::WlSurface, _: u32, _: &[u32], _: &[Keysym]) {
        self.kb_surf = self.surf_of(surface);
    }
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, surface: &wl_surface::WlSurface, _: u32) {
        let sid = self.surf_of(surface);
        if self.kb_surf == sid {
            self.kb_surf = None;
        }
        if let Some(sid) = sid {
            if let Some(SKind::Widget(_)) = self.surf_kind(sid) {
                self.st.field_focus = None;
                self.mark(sid);
            }
        }
    }
    fn press_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, e: KeyEvent) {
        self.on_key(&e);
    }
    fn repeat_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, e: KeyEvent) {
        self.on_key(&e);
    }
    fn release_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, _: KeyEvent) {}
    fn update_modifiers(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, m: Modifiers, _: RawModifiers, _: u32) {
        self.mods = m;
    }
}

impl PointerHandler for App {
    fn pointer_frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_pointer::WlPointer, events: &[PointerEvent]) {
        for e in events {
            self.on_pointer(e);
        }
    }
}

impl ShmHandler for App {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

impl Dispatch<wayland_client::protocol::wl_callback::WlCallback, (u32, i32, i32)> for App {
    fn event(app: &mut Self, _: &wayland_client::protocol::wl_callback::WlCallback, e: wayland_client::protocol::wl_callback::Event, d: &(u32, i32, i32), _: &Connection, _: &QueueHandle<Self>) {
        if let wayland_client::protocol::wl_callback::Event::Done { .. } = e {
            if let Some(s) = app.surfs.get_mut(&d.0) {
                s.applied = (d.1, d.2);
            }
        }
    }
}

impl Dispatch<ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1, ()> for App {
    fn event(_: &mut Self, _: &ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1, _: ext_background_effect_manager_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1, ()> for App {
    fn event(_: &mut Self, _: &ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1, _: ext_background_effect_surface_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<frac_mgr::WpFractionalScaleManagerV1, ()> for App {
    fn event(_: &mut Self, _: &frac_mgr::WpFractionalScaleManagerV1, _: frac_mgr::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
/// масштаб вывода под поверхностью: id поверхности (LOCK_BASE+i — экран блокировки)
impl Dispatch<frac::WpFractionalScaleV1, u32> for App {
    fn event(app: &mut Self, _: &frac::WpFractionalScaleV1, e: frac::Event, id: &u32, _: &Connection, _: &QueueHandle<Self>) {
        let frac::Event::PreferredScale { scale } = e else { return };
        let v = scale as f64 / 120.0;
        if *id >= LOCK_BASE {
            if let Some(l) = app.lock_surfs.get_mut((*id - LOCK_BASE) as usize) {
                if l.scale != v {
                    l.scale = v;
                    l.dirty = true;
                }
            }
        } else if let Some(s) = app.surfs.get_mut(id) {
            if s.scale != v {
                s.scale = v;
                s.dirty = true;
            }
        }
    }
}

impl Dispatch<ksi_mgr::ZwpKeyboardShortcutsInhibitManagerV1, ()> for App {
    fn event(_: &mut Self, _: &ksi_mgr::ZwpKeyboardShortcutsInhibitManagerV1, _: ksi_mgr::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<ksi::ZwpKeyboardShortcutsInhibitorV1, ()> for App {
    fn event(_: &mut Self, _: &ksi::ZwpKeyboardShortcutsInhibitorV1, _: ksi::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<wp_viewporter::WpViewporter, ()> for App {
    fn event(_: &mut Self, _: &wp_viewporter::WpViewporter, _: wp_viewporter::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<wp_viewport::WpViewport, ()> for App {
    fn event(_: &mut Self, _: &wp_viewport::WpViewport, _: wp_viewport::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<ext_idle_notifier_v1::ExtIdleNotifierV1, ()> for App {
    fn event(_: &mut Self, _: &ext_idle_notifier_v1::ExtIdleNotifierV1, _: ext_idle_notifier_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<ext_idle_notification_v1::ExtIdleNotificationV1, u8> for App {
    fn event(app: &mut Self, _: &ext_idle_notification_v1::ExtIdleNotificationV1, e: ext_idle_notification_v1::Event, kind: &u8, _: &Connection, _: &QueueHandle<Self>) {
        if let ext_idle_notification_v1::Event::Idled = e {
            if *kind == 0 {
                app.lock_now();
            } else {
                app.st.niri.act(serde_json::json!({"PowerOffMonitors": {}}));
            }
        }
    }
}

delegate_compositor!(App);
delegate_output!(App);
delegate_shm!(App);
delegate_seat!(App);
delegate_keyboard!(App);
delegate_pointer!(App);
delegate_layer!(App);
delegate_session_lock!(App);
delegate_registry!(App);
