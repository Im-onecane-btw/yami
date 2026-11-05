// Ядро: Wayland (layer-shell, ввод, кадры), поверхности шелла, цикл событий, маршрутизация
// кликов в действия. Состояние, которое рисуется, — в State; ресурсы (шрифты, иконки) — в Res.

use crate::config::{self, Settings};
use crate::gfx::{Canvas, R};
use crate::icons::{self, Icons};
use crate::niri::{self, Niri};
use crate::sys::Sys;
use crate::theme::{Theme, CLEAR};
use crate::ui::{Act, Field, Ui, Zone};
use crate::{audio, dbus, font::Fonts, launcher, media, menu, net, notif};

use calloop::{
    channel::{self, Sender},
    timer::{TimeoutAction, Timer},
    EventLoop, LoopHandle,
};
use calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::{
    compositor::{CompositorState, Region},
    output::OutputState,
    registry::RegistryState,
    seat::{
        keyboard::Modifiers,
        pointer::cursor_shape::CursorShapeManager, SeatState,
    },
    session_lock::{SessionLock, SessionLockState, SessionLockSurface},
    shell::{
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell, LayerSurface},
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm},
};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_shm},
    Connection, QueueHandle,
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
    /// ответ из уведомления (reply.rs)
    Reply,
    /// выбор обоев (walls.rs)
    Walls,
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
    /// ручка размера в правом нижнем углу плавающего окна (titles.rs)
    Grip(u64),
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
    /// отпечаток последнего отданного кадра (0 — не было): тот же кадр второй раз не отдаётся —
    /// бар перерисовывается раз в секунду, а меняется обычно раз в минуту
    pub drawn: u64,
}

/// отпечаток кадра: размер и пиксели (FNV по 64-битным словам — на бар ~30 мкс)
fn frame_hash(px: &[u32], dims: [i32; 4]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |v: u64| h = (h ^ v).wrapping_mul(0x0000_0100_0000_01b3);
    for d in dims {
        eat(d as u32 as u64);
    }
    let mut it = px.chunks_exact(2);
    for c in &mut it {
        eat(c[0] as u64 | (c[1] as u64) << 32);
    }
    for c in it.remainder() {
        eat(*c as u64);
    }
    h | 1
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
    /// строка модуля-скрипта бара (plugins.rs)
    Plugin(String, crate::plugins::Plug),
    /// файлы для «/» в лаунчере: запрос и найденное
    LFiles(String, Vec<std::path::PathBuf>),
    Art(String, Option<crate::gfx::Img>),
    Theme,
    /// кадры шторки готовы: номер включения, картинки по мониторам
    Curtain(u64, HashMap<String, crate::gfx::Img>),
    /// обои готовы (wall.rs): номер задания, картинка, акцент из неё
    Wall(u64, Option<crate::gfx::Img>, Option<u32>),
    /// папка выбора обоев прочитана (walls.rs): номер открытия, папка, картинки, сколько сверх MAX
    WallsListed(u64, std::path::PathBuf, Vec<std::path::PathBuf>, usize),
    /// миниатюра для выбора обоев (walls.rs): номер открытия, клетка, картинка
    WallThumb(u64, usize, Option<crate::gfx::Img>),
    /// ответ окна выбора yami-files из выбора обоев: папка ли, что выбрано, вернуться ли в настройки
    WallsAsked(bool, Option<String>, bool),
    /// мониторы, где обои рисует skwd-paper (skwd.rs, слои niri)
    SkwdLayers(Vec<String>),
    Snap(HashMap<String, crate::gfx::Img>, Vec<(String, R, String)>, crate::snap::Mode),
    /// распознанное со снимка: QR или текст, None — ничего не нашлось
    SnapText(bool, Option<String>),
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
    /// куда тянут полосу позиции (доля трека); перемотка — когда отпустят
    pub seek: Option<f32>,
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
    /// что есть для игр (проверяется при открытии вкладки «игры»)
    pub game_probe: crate::gameboost::Probe,
    pub night: crate::night::Night,
    /// модули бара из скриптов (~/.config/yami/modules)
    pub plugins: crate::plugins::Plugins,
    pub session: crate::session::Session,
    /// skwd-wall: что выбрано, его палитра, где рисует сам (skwd.rs)
    pub skwd: crate::skwd::Skwd,
    /// skwd-wall стоит на машине (проверяется при открытии настроек)
    pub skwd_here: bool,
    /// виджет без подложки стоит на светлых обоях (widgets.rs, widget_contrast)
    pub widget_bg: HashMap<u32, bool>,
    pub game_field: Field,
    /// окно ответа из уведомления
    pub reply: crate::reply::Reply,
    /// координаты для ночного света
    pub geo_field: Field,
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
    /// выбор обоев: папка, клетки, миниатюры видимых строк
    pub walls: crate::walls::Walls,
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
    pub(crate) registry: RegistryState,
    pub(crate) seat: SeatState,
    pub outputs: OutputState,
    pub(crate) compositor: CompositorState,
    pub(crate) layer_shell: LayerShell,
    pub(crate) shm: Shm,
    pub(crate) cursor_mgr: Option<CursorShapeManager>,
    pub(crate) cursor_dev: Option<WpCursorShapeDeviceV1>,
    pub(crate) cursor_now: Option<Shape>,
    pub(crate) viewporter: Option<wp_viewporter::WpViewporter>,
    pub(crate) fx_mgr: Option<ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1>,
    /// запись сочетаний во вкладке «клавиши»: niri отдаёт шеллу все клавиши
    pub(crate) ksi_mgr: Option<ksi_mgr::ZwpKeyboardShortcutsInhibitManagerV1>,
    pub(crate) ksi: Option<ksi::ZwpKeyboardShortcutsInhibitorV1>,
    pub(crate) frac_mgr: Option<frac_mgr::WpFractionalScaleManagerV1>,
    /// логический холст перед увеличением (HiDPI); большой — освобождается сразу
    pub(crate) scratch: Vec<u32>,
    pub(crate) lock_state: SessionLockState,
    pub lock: Option<SessionLock>,
    pub lock_surfs: Vec<LockSurf>,
    pub(crate) idle: Option<ext_idle_notifier_v1::ExtIdleNotifierV1>,
    pub(crate) idle_notes: Vec<ext_idle_notification_v1::ExtIdleNotificationV1>,
    pub(crate) idle_cfg: (u32, u32),
    pub(crate) pointer: Option<wl_pointer::WlPointer>,
    pub(crate) keyboard: Option<wl_keyboard::WlKeyboard>,
    pub(crate) enter_serial: u32,
    pub surfs: HashMap<u32, Surf>,
    pub(crate) next_id: u32,
    /// курсор: (поверхность, x, y); для экрана блокировки sid = u32::MAX - n
    pub ptr: Option<(u32, f64, f64)>,
    pub press: Option<Press>,
    pub(crate) last_click: Option<(Act, Instant)>,
    pub kb_surf: Option<u32>,
    pub mods: Modifiers,
    pub res: Res,
    pub st: State,
    pub set_saved: String,
    /// настройки на момент последней сверки с файлом: пока они те же, JSON не собирается
    /// (flush идёт на каждом проходе цикла — при эквалайзере 30 раз в секунду)
    pub(crate) set_seen: Settings,
    pub(crate) set_mtime: Option<std::time::SystemTime>,
    pub(crate) last_sample: Instant,
    pub(crate) last_min: u32,
    pub(crate) palette_written: String,
    /// обои: ключ того, из чего собрана текущая картинка, и номер последнего задания
    pub wall_key: String,
    pub wall_job: u64,
    /// номер последнего включения шторки (поздний кадр от прошлого — мимо)
    pub curtain_wanted: u64,
    pub quit: bool,
    /// выйти и запустить себя заново (тот же путь: после обновления — новый бинарь)
    pub restart: bool,
    /// где открыть меню программы трея: монитор, x, y
    pub tray_anchor: Option<(String, i32, i32)>,
    /// окно, которое тянут за заголовок или угол, и сколько сдвига (размера) niri уже отправлено
    pub tdrag: Option<(u64, (f64, f64))>,
    /// ползунок пиксельности сменил шрифт программ / курсор, theme.py ещё не звали
    pub look_pending: (bool, bool),
}

pub(crate) const LOCK_BASE: u32 = u32::MAX - 64;

impl State {
    pub fn new(set: Settings, niri: Niri) -> State {
        yami_px::gfx::set_reveal(yami_px::gfx::Reveal::from_id(&set.reveal), set.reveal_grain);
        State {
            t: Theme::new(set.accent_rgb(), set.tint, set.glass, set.lift, set.inner, set.light).apply(),
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
            seek: None,
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
            game_probe: Default::default(),
            night: Default::default(),
            plugins: Default::default(),
            session: Default::default(),
            skwd: Default::default(),
            skwd_here: false,
            widget_bg: HashMap::new(),
            game_field: Field::default(),
            reply: Default::default(),
            geo_field: Field::default(),
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
            walls: Default::default(),
            set,
        }
    }
}

/// долгий процесс нити шелла (playerctl -F, pactl subscribe, parec) уходит вместе с ней: при
/// падении шелла и при перезапуске (exec в том же pid уничтожает нити, их дети получают сигнал).
/// Без этого каждый `yami restart` и каждое обновление оставляли по живой копии каждого
pub fn owned(cmd: &mut std::process::Command) -> &mut std::process::Command {
    use std::os::unix::process::CommandExt;
    // SAFETY: между fork и exec только prctl
    unsafe {
        cmd.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
            Ok(())
        })
    }
}

/// окно выбора yami-files (`--pick MODE`, папка, свои ключи): первая строка ответа или None
/// (отмена). Ждёт, пока окно открыто, — только из нити; окно закрывается вместе с нитью (owned)
pub fn files_pick(mode: &str, folder: &std::path::Path, args: &[&str]) -> Option<String> {
    let mut c = std::process::Command::new("yami-files");
    c.args(["--pick", mode]).args(args).arg("--folder").arg(folder);
    owned(&mut c).output().ok().filter(|o| o.status.success()).and_then(|o| String::from_utf8_lossy(&o.stdout).lines().next().map(str::to_string))
}

/// дети, доставшиеся от образа до перезапуска: их Child остались в старых нитях, ждать их некому.
/// Список берётся до первого запуска чего-либо (все дети на этот момент — чужие) и
/// подбирается в tick, иначе каждый перезапуск оставлял зомби
static INHERITED: std::sync::Mutex<Vec<i32>> = std::sync::Mutex::new(Vec::new());

fn note_inherited() {
    let me = std::process::id().to_string();
    let Ok(rd) = std::fs::read_dir("/proc") else { return };
    let kids: Vec<i32> = rd
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.parse::<i32>().ok())
        .filter(|p| {
            // «pid (comm) S ppid …»: comm может содержать пробелы и скобки — режем по последней «)»
            let Ok(s) = std::fs::read_to_string(format!("/proc/{p}/stat")) else { return false };
            s.rsplit_once(')').and_then(|(_, r)| r.split_whitespace().nth(1)) == Some(me.as_str())
        })
        .collect();
    if let Ok(mut v) = INHERITED.lock() {
        *v = kids;
    }
}

fn reap_inherited() {
    let Ok(mut v) = INHERITED.lock() else { return };
    // 0 — ещё жив; pid — подобран; -1 — уже не наш
    v.retain(|&p| unsafe { libc::waitpid(p, std::ptr::null_mut(), libc::WNOHANG) } == 0);
}

pub fn run() {
    note_inherited();
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
    icons.set_pixel(set.icon_pixel.then(|| Theme::new(set.accent_rgb(), set.tint, set.glass, set.lift, set.inner, set.light).icon_ramp()));
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
        set_seen: Settings::default(),
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
        look_pending: (false, false),
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
    app.side_sync();
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
                drawn: 0,
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
            self.drop_press(id);
        }
    }

    /// поверхность ушла: нажатие на ней больше ни к чему, и перемотка, которую там тянули, не залипнет
    fn drop_press(&mut self, id: u32) {
        if self.press.as_ref().is_some_and(|p| p.sid == id) {
            self.press = None;
            self.st.seek = None;
        }
    }
    /// закрыть с анимацией (или сразу, если анимации выключены)
    pub fn close(&mut self, id: u32) {
        let anim = self.st.set.anim() && self.st.set.anim_close;
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
        // закрывается посреди перемотки (Esc): бегунок — назад к игре; само нажатие живёт до
        // отпускания (тянуть заголовок окна можно и через пересоздание его поверхности)
        if self.press.as_ref().is_some_and(|p| p.sid == id) {
            self.st.seek = None;
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
        // обои на этом мониторе рисует skwd-paper — свои убрать
        let skwd: Vec<u32> = self.surfs.iter().filter(|(_, s)| s.kind == SKind::Wall && self.skwd_draws(&s.output)).map(|(i, _)| *i).collect();
        for id in skwd {
            self.destroy(id);
        }
        for n in names {
            if self.find_on(&SKind::Wall, &n).is_none() && !self.skwd_draws(&n) {
                self.mk(SKind::Wall, &n, Layer::Background, "yami-wall", Anchor::all(), (0, 0), (0, 0, 0, 0), -1, KeyboardInteractivity::None);
                // монитор появился: теперь известен его размер (обои режутся под него)
                self.sync_wall();
                // и шторку на него, если она опущена
                self.curtain_show();
            }
            if self.find_on(&SKind::Bar, &n).is_none() && !self.st.bar_hidden {
                self.mk_bar(&n);
            }
        }
        // монитор сменил размер: ширина бара в процентах — от новой
        self.sync_bar();
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
        let under = |s: &Surf| covered.as_deref() == Some(s.output.as_str()) && matches!(s.kind, SKind::Bar | SKind::Widget(_) | SKind::Dock | SKind::Wall | SKind::Title(_) | SKind::Grip(_));
        // всплывашка перерисовывается — значит, её данные поменялись, и размер (строки сетей, устройств,
        // программ трея, время трека) мог тоже; resize_popup ничего не делает, если он тот же
        if self.surfs.values().any(|s| s.kind == SKind::Popup && s.dirty && !s.closing) {
            self.resize_popup();
        }
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

    pub(crate) fn draw(&mut self, id: u32) {
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
                    let a = if matches!(st.modal, Some(crate::app::Modal::Settings | crate::app::Modal::Walls)) { st.set.dim * 0.7 } else { st.set.dim };
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
                    Some(Modal::Reply) => crate::reply::draw(&mut ui, st, w, h),
                    Some(Modal::Walls) => crate::walls::draw(&mut ui, st, w, h),
                    None => {}
                },
                SKind::Popup => crate::popups::draw(&mut ui, st, w, h),
                SKind::Menu => crate::menu::draw(&mut ui, st, w, h),
                SKind::Notifs => crate::notif::draw(&mut ui, st, w, h),
                SKind::Osd => crate::notif::draw_osd(&mut ui, st, w, h),
                SKind::Widget(wid) => crate::widgets::draw(&mut ui, st, *wid, w, h),
                SKind::Title(win) => crate::titles::draw(&mut ui, st, *win, w, h),
                SKind::Grip(win) => crate::titles::draw_grip(&mut ui, st, *win, w, h),
                SKind::EditBar => crate::widgets::draw_editbar(&mut ui, st, w, h),
                SKind::Snap => crate::snap::draw(&mut ui, st, &s.output, w, h),
                SKind::Dock => crate::dock::draw(&mut ui, st, w, h),
                SKind::DockEdge => {}
                SKind::Curtain => crate::curtain::draw(&mut ui, st, &s.output, w, h),
                SKind::CurtainBadge => crate::curtain::draw_badge(&mut ui, w, h),
            }
            // развёртка — только панели: не обои, не оверлей скриншота, не шторка
            if st.set.scanlines && !matches!(s.kind, SKind::Dim | SKind::Catcher | SKind::Wall | SKind::Snap | SKind::Curtain | SKind::CurtainBadge | SKind::DockEdge | SKind::Title(_) | SKind::Grip(_)) {
                ui.cv.scanlines(if st.t.light { 0.06 } else { 0.22 });
            }
            if k < 1.0 && !matches!(s.kind, SKind::Dim | SKind::Catcher) {
                ui.cv.dissolve(k, 2);
            }
            // размытие — когда панель проявилась целиком, иначе сквозь клетки видно пятно
            blur = if !st.set.blur_on() || !finished || s.closing {
                vec![]
            } else if let SKind::Title(_) = s.kind {
                s.input.clone().unwrap_or_else(|| vec![R::new(0, 0, s.w, s.h)])
            } else {
                std::mem::take(&mut ui.cv.blur)
            };
            s.zones = std::mem::take(&mut ui.zones);
            s.anim = ui.anim;
        }
        // тот же кадр, что уже у композитора (часы без секунд, датчики не сдвинулись): не отдавать —
        // композитору нечего пересобирать. Обои, скриншот и шторка рисуются один раз и так
        let hash = if s.anim || done_closing || matches!(s.kind, SKind::Wall | SKind::Snap | SKind::Curtain) { 0 } else { frame_hash(buf, [s.w, s.h, pw, ph]) };
        if hash != 0 && hash == s.drawn && s.input == s.input_set && blur == s.blur_set {
            drop(buffer);
            self.scratch = if scratch.capacity() > 1 << 20 { Vec::new() } else { scratch };
            self.surfs.insert(id, s);
            return;
        }
        s.drawn = hash;
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

    // ── тик: часы, датчики, файл настроек ───────────────────────────────────
    pub(crate) fn tick_interval(&self) -> Duration {
        let fast = !self.st.notifs.popups.is_empty() || self.st.osd.is_some() || self.st.field_focus.is_some() || self.st.locked || self.st.modal == Some(Modal::Launcher);
        let fast = fast || self.st.upd.busy.is_some() || self.st.snap.is_some() || self.st.snap_countdown.is_some() || self.st.dock_leave.is_some() || !self.st.dock_bounce.is_empty();
        Duration::from_millis(if fast { 100 } else { 1000 })
    }

    /// батарея садится: одно уведомление на 10% и одно на 5%, пока не поставят на зарядку
    pub(crate) fn bat_check(&mut self) {
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

    pub(crate) fn tick(&mut self) {
        reap_inherited();
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
            self.need_tick();
            self.skwd_tick();
            // ночной свет упал сам — значок в баре гаснет
            let n = self.st.night.running();
            if n != self.st.night.on {
                self.st.night.on = n;
                self.dirty(&SKind::Bar);
            }
            if self.st.started.elapsed().as_secs() % 60 == 0 {
                self.update_tick();
            }
            if self.st.started.elapsed().as_secs() % 5 == 0 {
                self.session_tick();
            }
            // профиль могли сменить снаружи (powerprofilesctl, другой апплет)
            if self.st.started.elapsed().as_secs() % 15 == 0 && self.st.set.on_bar("profile") {
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

    pub(crate) fn uses(&self, src: &str) -> bool {
        let s = &self.st.set;
        s.widgets.iter().any(|w| w.on && w.kind == "graph" && w.source == src) || (src == "gpu" && self.st.popup_is("sys"))
    }

    // ── простой: замок и гашение экрана ─────────────────────────────────────
    pub(crate) fn setup_idle(&mut self) {
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
    pub(crate) fn on_msg(&mut self, m: Msg) {
        match m {
            Msg::Niri(v) => {
                let layout_only = v.get("WindowLayoutsChanged").is_some();
                let had_windows = !self.st.niri.wins.is_empty();
                // первый полный список окон: только теперь видно, что уже открыто
                let full_list = v.get("WindowsChanged").is_some();
                let evs = self.st.niri.handle(v);
                for e in evs {
                    match e {
                        niri::Ev::KbSwitched => self.show_osd("layout"),
                        niri::Ev::NewWindow(id) => self.on_new_window(id),
                        niri::Ev::Changed => {}
                    }
                }
                if !layout_only {
                    self.st.session.dirty = true;
                }
                if full_list {
                    self.session_restore();
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
                    // окна и столы видны только в баре и доке: виджеты и всплывашки их не рисуют,
                    // а события niri идут на каждую смену фокуса
                    self.dirty_where(|k| matches!(k, SKind::Bar | SKind::Dock));
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
                // ~30 раз в секунду: перерисовать только то, где эквалайзер (всплывашка с плеером,
                // виджет музыки, бар с модулем media), а не все бары и виджеты на всех мониторах
                self.st.viz = v;
                let (s, pop) = (&self.st.set, crate::popups::shows_media(&self.st));
                let bar = s.on_bar("media");
                let ws: Vec<u32> = s.widgets.iter().filter(|w| w.on && w.kind == "media").map(|w| w.id).collect();
                self.dirty_where(|k| match k {
                    SKind::Popup => pop,
                    SKind::Bar => bar,
                    SKind::Widget(id) => ws.contains(id),
                    _ => false,
                });
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
                if self.st.popup_is("tray") {
                    self.dirty(&SKind::Popup);
                }
            }
            Msg::TrayMenu(svc, items) => self.open_tray_menu(&svc, items),
            Msg::Net(n) => {
                // сети не хватило пароля — поле под ней, курсор в нём
                if !n.ask.is_empty() && self.st.popup_is("net") {
                    self.st.wifi_target = n.ask.clone();
                    self.st.wifi_pass.clear();
                    self.st.field_focus = Some("wifi");
                }
                self.st.net = n;
                self.dirty(&SKind::Popup);
            }
            Msg::Bt(mut b) => {
                // сигналы BlueZ не знают, что идёт сопряжение: его состояние держим до ответа
                if !b.op {
                    b.msg = std::mem::take(&mut self.st.bt.msg);
                    b.busy = std::mem::take(&mut self.st.bt.busy);
                }
                self.st.bt = b;
                self.dirty_where(|k| matches!(k, SKind::Popup | SKind::Bar));
            }
            Msg::Pam(ok) => self.on_pam(ok),
            Msg::Auth(e) => self.on_auth(e),
            Msg::Prof(p) => self.on_prof(p),
            Msg::ScreenPick(types, s) => self.screen_ask(types, s),
            Msg::Clip(lines) => {
                self.st.launcher.clip_lines = lines;
                self.st.launcher.refresh(&self.st.apps, &self.st.niri.wins);
                self.dirty(&SKind::Modal);
            }
            Msg::Plugin(id, p) => {
                if self.st.plugins.shown.get(&id) != Some(&p) {
                    self.st.plugins.shown.insert(id, p);
                    self.dirty(&SKind::Bar);
                }
            }
            Msg::SnapText(qr, text) => self.on_snap_text(qr, text),
            Msg::LFiles(q, found) => {
                self.st.launcher.files = (q, found);
                if self.st.modal == Some(Modal::Launcher) {
                    self.st.launcher.refresh(&self.st.apps, &self.st.niri.wins);
                    self.dirty(&SKind::Modal);
                }
            }
            Msg::Wall(n, img, accent) => self.wall_ready(n, img, accent),
            Msg::WallsListed(gen, dir, files, more) => self.walls_listed(gen, dir, files, more),
            Msg::WallThumb(gen, i, img) => self.walls_thumb(gen, i, img),
            Msg::WallsAsked(folder, got, back) => self.walls_asked(folder, got, back),
            Msg::SkwdLayers(drawn) => self.skwd_layers(drawn),
            Msg::Curtain(n, imgs) => self.curtain_ready(n, imgs),
            Msg::Theme => {
                self.sync_wall();
                self.res.icons.drop_cache();
            }
        }
    }

    pub fn media_visible(&self) -> bool {
        self.st.set.media_placed() || crate::popups::shows_media(&self.st)
    }

    pub(crate) fn on_new_window(&mut self, id: u64) {
        // в режиме «стол»: под заголовок нужно место сверху
        self.fit_title(id);
        self.session_place(id);
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
            "night" => self.night_toggle(),
            "clipboard" => self.toggle_modal(Modal::Launcher, true),
            "power" => self.toggle_modal(Modal::Power, false),
            "settings" => self.toggle_modal(Modal::Settings, false),
            "walls" => self.walls_open(false),
            "lock" => self.lock_now(),
            "bar" => self.toggle_bar(),
            "stacking" => {
                let on = !self.st.niri.stacking;
                self.st.niri.set_stacking(on);
                self.st.set.stacking = on;
                // фокус за мышью есть только в ленте (keys::kdl)
                crate::keys::write(&self.st.set);
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
                let kind = crate::popups::KINDS.iter().copied().find(|k| *k == &c[6..]).unwrap_or("sys");
                // из терминала или хоткея: под правым краем бара
                let out = self.focus_out();
                let (ow, _) = self.output_size(&out);
                let side = crate::bar::geo(&self.st.set, ow).1;
                self.open_popup(kind, &out, ow - side - 260);
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
        } else {
            self.tab_probe();
        }
        self.dirty(&SKind::Modal);
    }

    pub fn close_all(&mut self) {
        self.close_modal();
        self.close_popup();
        self.close_menu();
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
    if !st.set.anim() {
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
        "pixel_level" => crate::settings::pixel_level(s) as f32,
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

/// tools/theme.py, тихо и с пониженным приоритетом: на слабой машине перекраска (обои, курсор,
/// темы программ) идёт секунды и не должна отнимать процессор у стола
pub fn theme_py() -> std::process::Command {
    let mut c = std::process::Command::new("python3");
    c.arg("-I").arg(theme_script()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    gentle(&mut c);
    c
}

/// фоновая работа (theme.py, распознавание текста): nice 10 у процесса, шелл свой не меняет
pub fn gentle(cmd: &mut std::process::Command) -> &mut std::process::Command {
    use std::os::unix::process::CommandExt;
    // SAFETY: между fork и exec только setpriority
    unsafe {
        cmd.pre_exec(|| {
            libc::setpriority(libc::PRIO_PROCESS, 0, 10);
            Ok(())
        })
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
