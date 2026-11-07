// Окно экрана входа: по поверхности layer-shell (overlay, на весь экран) на каждый монитор, поле
// входа — на том, где клавиатура. shm и HiDPI как в шелле: холст логический, увеличение
// ближайшим соседом. greetd — в своей нити (greetd.rs), события приходят сюда каналом.
// greet-session узнаёт, как идёт запуск, из data::status: «up» после первого кадра, «session» —
// как только greetd принял сеанс (до ухода и выхода niri). Уход кончается чёрным кадром, и niri
// закрывается, только когда этот кадр показан: между экраном входа и сеансом — ровный чёрный.

use crate::draw::{Act, Phase, Ui, G};
use crate::greetd::{Ask, Ev};
use calloop::channel::{self};
use calloop::timer::{TimeoutAction, Timer};
use calloop::{EventLoop, LoopHandle};
use calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer, delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{cursor_shape::CursorShapeManager, PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::{Shape, WpCursorShapeDeviceV1};
use wayland_protocols::wp::fractional_scale::v1::client::{wp_fractional_scale_manager_v1 as frac_mgr, wp_fractional_scale_v1 as frac};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use yami_px::font::Fonts;
use yami_px::gfx::Canvas;
use yami_px::icons::Icons;
use yami_px::ui::{Cur, Zone};

/// простой без ввода: недописанный пароль стирается, курсор поля перестаёт мигать (ни одной
/// перерисовки до ввода), мониторы гаснут через data::Opts::off_min
const IDLE_WIPE: Duration = Duration::from_secs(60);
const IDLE_CALM: Duration = Duration::from_secs(30);
/// демо (поверх своего сеанса) само закрывается
const DEMO_MAX: Duration = Duration::from_secs(180);

struct Out {
    id: u32,
    layer: LayerSurface,
    wl: wl_output::WlOutput,
    w: i32,
    h: i32,
    scale: f64,
    frac: Option<frac::WpFractionalScaleV1>,
    vp: Option<wp_viewport::WpViewport>,
    pool: Option<SlotPool>,
    zones: Vec<Zone<Act>>,
    configured: bool,
    dirty: bool,
    frame_pending: bool,
    anim: bool,
}

pub struct App {
    conn: Connection,
    qh: QueueHandle<App>,
    handle: LoopHandle<'static, App>,
    registry: RegistryState,
    seat: SeatState,
    outputs: OutputState,
    compositor: CompositorState,
    shm: Shm,
    layer_shell: LayerShell,
    frac_mgr: Option<frac_mgr::WpFractionalScaleManagerV1>,
    viewporter: Option<wp_viewporter::WpViewporter>,
    cursor_mgr: Option<CursorShapeManager>,
    cursor_dev: Option<WpCursorShapeDeviceV1>,
    cursor_now: Option<Shape>,
    enter_serial: u32,
    pointer: Option<wl_pointer::WlPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    mods: Modifiers,
    surfs: Vec<Out>,
    next_id: u32,
    /// поверхность с клавиатурой (на ней поле входа)
    kb_on: Option<u32>,
    /// поверхность под курсором и точка
    ptr: Option<(u32, f64, f64)>,
    press: Option<(u32, Act)>,
    scratch: Vec<u32>,
    fonts: Fonts,
    icons: Icons,
    g: G,
    ask: Sender<Ask>,
    minute: u32,
    last_input: Instant,
    screens_off: bool,
    /// старт процесса: время до первого кадра
    t0: Instant,
    /// первый кадр показан (через столько мс после старта)
    up: Option<u128>,
    /// уход доигран, чёрный кадр отправлен (когда); показан — можно закрывать niri
    leaving: Option<Instant>,
    left: bool,
    pub code: i32,
    quit: bool,
}

pub fn run(g: G, fonts: Fonts, t0: Instant) -> i32 {
    let Ok(conn) = Connection::connect_to_env() else {
        eprintln!("yami-greet: no Wayland display");
        return 1;
    };
    let Ok((globals, queue)) = registry_queue_init::<App>(&conn) else { return 1 };
    let qh = queue.handle();
    let Ok(mut ev) = EventLoop::<'static, App>::try_new() else { return 1 };
    let handle = ev.handle();
    let (tx, rx) = channel::channel::<Ev>();
    let _ = handle.insert_source(rx, |e, _, app: &mut App| {
        if let channel::Event::Msg(m) = e {
            app.on_ev(m);
        }
    });
    let (ask, ask_rx) = std::sync::mpsc::channel::<Ask>();
    let send = move |e: Ev| {
        let _ = tx.send(e);
    };
    if g.demo {
        crate::greetd::spawn_demo(ask_rx, send);
    } else {
        crate::greetd::spawn(ask_rx, send);
    }
    let (Ok(compositor), Ok(shm), Ok(layer_shell)) = (CompositorState::bind(&globals, &qh), Shm::bind(&globals, &qh), LayerShell::bind(&globals, &qh)) else {
        eprintln!("yami-greet: the compositor has no layer-shell");
        return 1;
    };
    let mut app = App {
        conn: conn.clone(),
        qh: qh.clone(),
        handle: handle.clone(),
        registry: RegistryState::new(&globals),
        seat: SeatState::new(&globals, &qh),
        outputs: OutputState::new(&globals, &qh),
        compositor,
        shm,
        layer_shell,
        frac_mgr: globals.bind(&qh, 1..=1, ()).ok(),
        viewporter: globals.bind(&qh, 1..=1, ()).ok(),
        cursor_mgr: CursorShapeManager::bind(&globals, &qh).ok(),
        cursor_dev: None,
        cursor_now: None,
        enter_serial: 0,
        pointer: None,
        keyboard: None,
        mods: Modifiers::default(),
        surfs: vec![],
        next_id: 1,
        kb_on: None,
        ptr: None,
        press: None,
        scratch: vec![],
        fonts,
        icons: Icons::new(""),
        g,
        ask,
        minute: yami_px::time::local_now().min,
        last_input: Instant::now(),
        screens_off: false,
        t0,
        up: None,
        leaving: None,
        left: false,
        // без запуска сеанса выход — сбой (niri пропал, greetd отвалился): greet-session их считает
        code: 2,
        quit: false,
    };
    let _ = handle.insert_source(Timer::from_duration(Duration::from_millis(530)), |_, _, app: &mut App| {
        app.tick();
        TimeoutAction::ToDuration(Duration::from_millis(530))
    });
    // ни одного монитора за 2 с: niri экрана входа погасил все (yami-outputs.kdl выключает тот,
    // что сейчас не подключён, а подключённый — off). Включить подключённый, иначе войти некуда
    if !app.g.demo {
        let _ = handle.insert_source(Timer::from_duration(Duration::from_secs(2)), |_, _, app: &mut App| {
            if app.surfs.is_empty() {
                eprintln!("yami-greet: no monitor is on, turning one on");
                std::thread::spawn(crate::niri::any_output_on);
            }
            TimeoutAction::Drop
        });
    }
    let _ = WaylandSource::new(conn.clone(), queue).insert(handle.clone());
    // мониторы приходят первым обходом реестра; поверхности — по new_output
    let _ = ev.dispatch(Some(Duration::from_millis(0)), &mut app);
    loop {
        if ev.dispatch(Some(Duration::from_secs(1)), &mut app).is_err() {
            break;
        }
        app.flush();
        if app.quit {
            break;
        }
    }
    app.code
}

impl App {
    fn main_id(&self) -> Option<u32> {
        self.kb_on.filter(|k| self.surfs.iter().any(|s| s.id == *k)).or_else(|| self.surfs.first().map(|s| s.id))
    }

    fn dirty_all(&mut self) {
        for s in self.surfs.iter_mut() {
            s.dirty = true;
        }
    }

    fn dirty_main(&mut self) {
        let m = self.main_id();
        for s in self.surfs.iter_mut().filter(|s| Some(s.id) == m) {
            s.dirty = true;
        }
    }

    fn add_output(&mut self, o: wl_output::WlOutput) {
        if self.surfs.iter().any(|s| s.wl == o) {
            return;
        }
        let surface = self.compositor.create_surface(&self.qh);
        let id = self.next_id;
        self.next_id += 1;
        let frac = self.frac_mgr.as_ref().map(|m| m.get_fractional_scale(&surface, &self.qh, id));
        let vp = self.viewporter.as_ref().map(|v| v.get_viewport(&surface, &self.qh, ()));
        let layer = self.layer_shell.create_layer_surface(&self.qh, surface, Layer::Overlay, Some("yami-greet"), Some(&o));
        layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        layer.commit();
        self.surfs.push(Out { id, layer, wl: o, w: 0, h: 0, scale: 1.0, frac, vp, pool: None, zones: vec![], configured: false, dirty: true, frame_pending: false, anim: false });
    }

    fn tick(&mut self) {
        let idle = self.last_input.elapsed();
        // мигает курсор поля, пока с экраном что-то делают; потом — ни одной перерисовки
        let calm = idle > IDLE_CALM || self.screens_off;
        if !(calm && self.g.blink) {
            self.g.blink = if calm { true } else { !self.g.blink };
            if !self.screens_off {
                self.dirty_main();
            }
        }
        let m = yami_px::time::local_now().min;
        if m != self.minute {
            self.minute = m;
            if !self.screens_off {
                self.dirty_all();
            }
        }
        if idle > IDLE_WIPE && !self.g.field.text.is_empty() && matches!(self.g.phase, Phase::Pass | Phase::Name) {
            self.g.field.wipe();
            self.dirty_main();
        }
        let off = self.g.opts.off_min;
        if off > 0 && idle > Duration::from_secs(off as u64 * 60) && !self.screens_off && !self.g.demo {
            self.screens_off = true;
            std::thread::spawn(crate::niri::monitors_off);
        }
        if self.g.demo && idle > DEMO_MAX {
            self.quit = true;
        }
        self.flush();
    }

    fn flush(&mut self) {
        for i in 0..self.surfs.len() {
            let s = &self.surfs[i];
            if s.dirty && s.configured && !s.frame_pending {
                self.draw(i);
            }
        }
        if self.g.out_done() {
            match self.leaving {
                // уход доигран: следующий кадр — чёрный (draw), ждём, когда его покажут
                None => {
                    self.leaving = Some(Instant::now());
                    self.dirty_all();
                    let _ = self.handle.insert_source(Timer::from_duration(Duration::from_millis(300)), |_, _, app: &mut App| {
                        app.flush();
                        TimeoutAction::Drop
                    });
                    self.flush();
                    return;
                }
                // показан (или монитор не отвечает 250 мс) — выходить
                Some(t) if self.left || t.elapsed() > Duration::from_millis(250) || self.surfs.is_empty() => self.finish(),
                _ => {}
            }
        }
        let _ = self.conn.flush();
    }

    /// сеанс запущен, уход доигран и показан: niri экрана входа — выйти (greetd запустит сеанс)
    fn finish(&mut self) {
        if self.quit {
            return;
        }
        self.code = 0;
        let _ = self.conn.flush();
        if !self.g.demo {
            crate::niri::quit();
        }
        self.quit = true;
    }

    fn ms(&self) -> u128 {
        self.t0.elapsed().as_millis()
    }

    fn draw(&mut self, i: usize) {
        let main = self.main_id() == Some(self.surfs[i].id);
        let s = &mut self.surfs[i];
        s.dirty = false;
        let (w, h) = (s.w, s.h);
        if w <= 0 || h <= 0 {
            return;
        }
        let (pw, ph) = if s.vp.is_none() || s.scale == 1.0 { (w, h) } else { yami_px::gfx::phys(w, h, s.scale) };
        let up = (pw, ph) != (w, h);
        let need = (pw * ph * 4) as usize;
        if s.pool.as_ref().map(|p| p.len() < need).unwrap_or(true) {
            s.pool = SlotPool::new(need * 2, &self.shm).ok();
        }
        let Some(pool) = s.pool.as_mut() else { return };
        let Ok((buffer, bytes)) = pool.create_buffer(pw, ph, pw * 4, wl_shm::Format::Argb8888) else { return };
        let (out_ptr, out_len) = (bytes.as_mut_ptr() as *mut u32, bytes.len() / 4);
        let mut scratch = std::mem::take(&mut self.scratch);
        let buf: &mut [u32] = if up {
            scratch.clear();
            scratch.resize((w * h) as usize, 0);
            &mut scratch
        } else {
            unsafe { std::slice::from_raw_parts_mut(out_ptr, out_len) }
        };
        let ptr = self.ptr.filter(|p| p.0 == s.id).map(|p| (p.1, p.2));
        let down = self.press.as_ref().is_some_and(|p| p.0 == s.id);
        {
            let mut ui = Ui { cv: Canvas::new(buf, w, h), f: &mut self.fonts, ic: &mut self.icons, t: self.g.look.theme, zones: vec![], ptr, down, anim: false, blink: self.g.blink, tr: crate::i18n::tr };
            crate::draw::draw(&mut ui, &self.g, w, h, main);
            s.zones = std::mem::take(&mut ui.zones);
            s.anim = ui.anim;
        }
        if up {
            yami_px::gfx::upscale(&scratch, w, h, unsafe { std::slice::from_raw_parts_mut(out_ptr, out_len) }, pw, ph);
        }
        self.scratch = scratch;
        let surf = s.layer.wl_surface().clone();
        if let Some(vp) = &s.vp {
            vp.set_destination(w, h);
        }
        surf.damage_buffer(0, 0, pw, ph);
        if s.anim {
            surf.frame(&self.qh, surf.clone());
            s.frame_pending = true;
        }
        let _ = buffer.attach_to(&surf);
        surf.commit();
        if self.up.is_none() {
            // первый кадр: greet-session больше не считает этот запуск несостоявшимся
            let ms = self.ms();
            self.up = Some(ms);
            eprintln!("yami-greet: first frame in {ms} ms ({w}x{h}, {} monitor(s))", self.surfs.len());
            if !self.g.demo {
                crate::data::status("up", ms);
            }
        }
    }

    fn on_ev(&mut self, e: Ev) {
        // greetd принял сеанс: greet-session узнаёт об этом сразу, до ухода и выхода niri
        if matches!(e, Ev::Started) && !self.g.demo {
            crate::data::status("session", self.up.unwrap_or(0));
        }
        let g = &mut self.g;
        let back = g.first_phase();
        g.bad = matches!(e, Ev::Error(_) | Ev::Lost(_) | Ev::Info { error: true, .. });
        match e {
            Ev::Prompt { secret, text } => {
                g.phase = Phase::Prompt { secret };
                g.prompt = text;
                g.status.clear();
            }
            Ev::Info { text, .. } => g.status = text,
            Ev::Failed => {
                g.fails += 1;
                g.status.clear();
                g.shake = Some(Instant::now());
                g.phase = back;
            }
            Ev::Started if g.gone.is_some() => {}
            Ev::Error(d) => {
                g.status = if d.trim().is_empty() { crate::i18n::tr("сеанс не запустился").to_string() } else { d };
                g.phase = back;
            }
            Ev::Started => {
                let session = g.sessions.get(g.session).map(|s| s.id.clone()).unwrap_or_default();
                crate::data::remember(&g.user_name(), &session);
                g.phase = Phase::Done;
                g.status.clear();
                g.gone = Some(Instant::now());
                self.dirty_all();
            }
            Ev::Lost(why) => {
                eprintln!("yami-greet: greetd: {why}");
                g.status = crate::i18n::tr("нет связи с greetd").to_string();
                g.phase = back;
                // без greetd войти нельзя: выйти через 3 с, greet-session решит, что дальше
                self.code = 1;
                let _ = self.handle.insert_source(Timer::from_duration(Duration::from_secs(3)), |_, _, app: &mut App| {
                    app.quit = true;
                    TimeoutAction::Drop
                });
            }
        }
        self.g.field.wipe();
        // имя вводили руками: после отказа оно уже в поле — Enter спросит пароль снова
        if self.g.phase == Phase::Name && !self.g.typed.is_empty() && self.g.gone.is_none() {
            let t = self.g.typed.clone();
            self.g.field.set(&t);
            self.g.field.text.reserve(1024);
        }
        self.dirty_all();
    }

    fn submit(&mut self) {
        let g = &mut self.g;
        let mut text = std::mem::replace(&mut g.field.text, String::with_capacity(1024));
        g.field.wipe();
        g.status.clear();
        g.bad = false;
        match g.phase {
            Phase::Pass | Phase::Name => {
                if g.phase == Phase::Name {
                    let name = text.trim().to_string();
                    yami_px::ui::wipe(&mut text);
                    if name.is_empty() || name.len() > 64 || !name.chars().all(|c| c.is_alphanumeric() || "._-@".contains(c)) {
                        return;
                    }
                    g.typed = name;
                } else if text.is_empty() {
                    return;
                }
                let (cmd, env) = match g.sessions.get(g.session) {
                    Some(s) => (s.cmd.clone(), s.env()),
                    None => (vec!["niri-session".to_string()], vec!["XDG_SESSION_TYPE=wayland".to_string()]),
                };
                let pass = if g.phase == Phase::Pass {
                    g.typed.clear();
                    Some(text)
                } else {
                    None
                };
                let user = g.user_name();
                g.phase = Phase::Busy;
                let _ = self.ask.send(Ask::Login { user, pass, cmd, env });
            }
            Phase::Prompt { .. } => {
                g.phase = Phase::Busy;
                let _ = self.ask.send(Ask::Answer(Some(text)));
            }
            _ => yami_px::ui::wipe(&mut text),
        }
    }

    fn act(&mut self, a: Act, back: bool) {
        let g = &mut self.g;
        let busy = matches!(g.phase, Phase::Busy | Phase::Done);
        match a {
            // по кругу: пользователи, потом «другой» (имя вводят)
            Act::UserPrev | Act::UserNext if !busy && g.can_pick() && matches!(g.phase, Phase::Pass | Phase::Name) => {
                let n = g.users.len() + 1;
                g.user = if (a == Act::UserPrev) != back { (g.user + n - 1) % n } else { (g.user + 1) % n };
                g.typed.clear();
                g.field.wipe();
                g.fails = 0;
                g.status.clear();
                g.phase = g.first_phase();
            }
            Act::Session if !busy && !g.sessions.is_empty() => {
                let n = g.sessions.len();
                g.session = if back { (g.session + n - 1) % n } else { (g.session + 1) % n };
            }
            Act::Layout => {
                std::thread::spawn(crate::niri::next_layout);
            }
            Act::Sleep | Act::Reboot | Act::PowerOff if g.power && g.opts.power => {
                let (cmd, msg) = match a {
                    Act::Sleep => ("suspend", "засыпаю…"),
                    Act::Reboot => ("reboot", "перезагружаю…"),
                    _ => ("poweroff", "выключаю…"),
                };
                if g.demo {
                    g.status = crate::i18n::tr("в демо не выключаю").to_string();
                } else {
                    g.status = crate::i18n::tr(msg).to_string();
                    // logind (и elogind): экран входа — активный сеанс, polkit пускает
                    let _ = std::process::Command::new("loginctl").arg(cmd).stdin(std::process::Stdio::null()).spawn().map(|mut c| std::thread::spawn(move || c.wait()));
                }
            }
            _ => {}
        }
        self.dirty_all();
    }

    fn wake(&mut self) {
        self.last_input = Instant::now();
        if self.screens_off {
            self.screens_off = false;
            self.dirty_all();
        }
    }

    fn key(&mut self, e: &KeyEvent) {
        self.wake();
        let g = &mut self.g;
        // демо поверх своего сеанса: Esc на пустом поле закрывает его
        if g.demo && e.keysym == Keysym::Escape && g.field.text.is_empty() && !matches!(g.phase, Phase::Prompt { .. }) {
            self.quit = true;
            return;
        }
        if matches!(g.phase, Phase::Busy | Phase::Done) {
            return;
        }
        let ctrl = self.mods.ctrl;
        match e.keysym {
            Keysym::Return | Keysym::KP_Enter => self.submit(),
            Keysym::Escape => {
                if matches!(g.phase, Phase::Prompt { .. }) {
                    let _ = self.ask.send(Ask::Cancel);
                    g.phase = g.first_phase();
                    g.status.clear();
                }
                g.field.wipe();
            }
            Keysym::Up if matches!(g.phase, Phase::Pass | Phase::Name) && g.can_pick() => return self.act(Act::UserPrev, false),
            Keysym::Down if matches!(g.phase, Phase::Pass | Phase::Name) && g.can_pick() => return self.act(Act::UserNext, false),
            Keysym::BackSpace => g.field.backspace(ctrl),
            Keysym::Delete => g.field.delete(),
            Keysym::Left => g.field.left(),
            Keysym::Right => g.field.right(),
            Keysym::Home => g.field.home(),
            Keysym::End => g.field.end(),
            // по физическим клавишам: и в русской раскладке
            _ if ctrl && e.raw_code == 30 => g.field.select_all(),
            _ if ctrl && e.raw_code == 22 => g.field.wipe(),
            _ if ctrl || self.mods.alt || self.mods.logo => {}
            _ => {
                if let Some(t) = e.utf8.as_deref().filter(|t| !t.is_empty()) {
                    if g.field.text.len() < 512 {
                        g.field.insert(t);
                        if g.fails > 0 {
                            g.status.clear();
                        }
                    }
                }
            }
        }
        self.g.blink = true;
        self.dirty_main();
    }

    fn zone_at(&self, id: u32, x: f64, y: f64) -> Option<Zone<Act>> {
        self.surfs.iter().find(|s| s.id == id)?.zones.iter().rev().find(|z| z.r.contains(x, y)).cloned()
    }

    fn set_cursor(&mut self, c: Shape) {
        if self.cursor_now == Some(c) {
            return;
        }
        self.cursor_now = Some(c);
        if let Some(d) = &self.cursor_dev {
            d.set_shape(self.enter_serial, c);
        }
    }

    fn surf_id(&self, s: &wl_surface::WlSurface) -> Option<u32> {
        self.surfs.iter().find(|o| o.layer.wl_surface() == s).map(|o| o.id)
    }

    fn on_pointer(&mut self, e: &PointerEvent) {
        let Some(id) = self.surf_id(&e.surface) else { return };
        let (x, y) = e.position;
        match e.kind {
            PointerEventKind::Enter { serial } => {
                self.enter_serial = serial;
                self.cursor_now = None;
                self.ptr = Some((id, x, y));
                self.set_cursor(Shape::Default);
            }
            PointerEventKind::Leave { .. } => {
                self.ptr = None;
                self.press = None;
            }
            PointerEventKind::Motion { .. } => {
                self.wake();
                self.ptr = Some((id, x, y));
                let shape = match self.zone_at(id, x, y).map(|z| z.cur) {
                    Some(Cur::Pointer) => Shape::Pointer,
                    Some(Cur::Text) => Shape::Text,
                    _ => Shape::Default,
                };
                self.set_cursor(shape);
            }
            PointerEventKind::Press { button, .. } => {
                self.wake();
                if button == 0x110 || button == 0x111 {
                    self.press = self.zone_at(id, x, y).map(|z| (id, z.act));
                }
            }
            PointerEventKind::Release { button, .. } => {
                if let Some((pid, a)) = self.press.take() {
                    if pid == id && self.zone_at(id, x, y).is_some_and(|z| z.act == a) {
                        self.act(a, button == 0x111);
                    }
                }
            }
            PointerEventKind::Axis { .. } => {}
        }
        for s in self.surfs.iter_mut().filter(|s| s.id == id) {
            s.dirty = true;
        }
    }
}

impl CompositorHandler for App {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, surf: &wl_surface::WlSurface, _: u32) {
        let leaving = self.leaving.is_some();
        for s in self.surfs.iter_mut().filter(|s| s.layer.wl_surface() == surf) {
            s.frame_pending = false;
            // чёрный кадр ухода показан: больше не рисовать, выходить
            if leaving && !s.dirty {
                self.left = true;
                continue;
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
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, o: wl_output::WlOutput) {
        self.add_output(o);
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, o: wl_output::WlOutput) {
        self.surfs.retain(|s| {
            if s.wl == o {
                if let Some(f) = &s.frac {
                    f.destroy();
                }
                if let Some(v) = &s.vp {
                    v.destroy();
                }
            }
            s.wl != o
        });
        self.dirty_all();
    }
}

impl LayerShellHandler for App {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, l: &LayerSurface) {
        self.surfs.retain(|s| &s.layer != l);
    }
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, l: &LayerSurface, c: LayerSurfaceConfigure, _: u32) {
        for s in self.surfs.iter_mut().filter(|s| &s.layer == l) {
            s.w = c.new_size.0 as i32;
            s.h = c.new_size.1 as i32;
            s.configured = true;
            s.dirty = true;
            s.frame_pending = false;
        }
        self.flush();
    }
}

impl SeatHandler for App {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Keyboard && self.keyboard.is_none() {
            let h = self.handle.clone();
            self.keyboard = self.seat.get_keyboard_with_repeat(qh, &seat, None, h, Box::new(|app: &mut App, _, e| app.key(&e))).ok();
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
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, s: &wl_surface::WlSurface, _: u32, _: &[u32], _: &[Keysym]) {
        if let Some(id) = self.surf_id(s) {
            self.kb_on = Some(id);
            self.dirty_all();
        }
    }
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32) {}
    fn press_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, e: KeyEvent) {
        self.key(&e);
    }
    fn repeat_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, e: KeyEvent) {
        self.key(&e);
    }
    fn release_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, _: KeyEvent) {}
    fn update_modifiers(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, m: Modifiers, _: RawModifiers, layout: u32) {
        self.mods = m;
        if self.g.caps != m.caps_lock || self.g.layout != layout as usize {
            self.g.caps = m.caps_lock;
            self.g.layout = layout as usize;
            self.dirty_main();
        }
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

impl Dispatch<frac_mgr::WpFractionalScaleManagerV1, ()> for App {
    fn event(_: &mut Self, _: &frac_mgr::WpFractionalScaleManagerV1, _: frac_mgr::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<wp_viewporter::WpViewporter, ()> for App {
    fn event(_: &mut Self, _: &wp_viewporter::WpViewporter, _: wp_viewporter::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<wp_viewport::WpViewport, ()> for App {
    fn event(_: &mut Self, _: &wp_viewport::WpViewport, _: wp_viewport::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<frac::WpFractionalScaleV1, u32> for App {
    fn event(app: &mut Self, _: &frac::WpFractionalScaleV1, e: frac::Event, id: &u32, _: &Connection, _: &QueueHandle<Self>) {
        if let frac::Event::PreferredScale { scale } = e {
            let v = scale as f64 / 120.0;
            for s in app.surfs.iter_mut().filter(|s| s.id == *id && s.scale != v) {
                s.scale = v;
                s.dirty = true;
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
delegate_registry!(App);
