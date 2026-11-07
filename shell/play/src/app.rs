// Окно плеера: xdg-toplevel, shm-буфер XRGB в физических пикселях. mpv рисует кадр прямо в
// буфер, интерфейс ложится сверху (при HiDPI — логическим холстом с увеличением клетками).
// mpv будит цикл событий из своих нитей (calloop ping); новые кадры — по кадрам композитора.
// Пока видео идёт, экран не гаснет (idle-inhibit), курсор прячется вместе с панелями.

use crate::font::Fonts;
use crate::gfx::Canvas;
use crate::icons::Icons;
use crate::mpv::{Event, Mpv};
use crate::play::{Act, Fx, Ui, P};
use calloop::timer::{TimeoutAction, Timer};
use calloop::{EventLoop, LoopHandle};
use calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_output, delegate_pointer, delegate_registry, delegate_seat, delegate_shm, delegate_xdg_shell, delegate_xdg_window,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{cursor_shape::CursorShapeManager, PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        xdg::{
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
            XdgShell,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use std::time::{Duration, Instant};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::{Shape, WpCursorShapeDeviceV1};
use wayland_protocols::wp::fractional_scale::v1::client::{wp_fractional_scale_manager_v1 as frac_mgr, wp_fractional_scale_v1 as frac};
use wayland_protocols::wp::idle_inhibit::zv1::client::{zwp_idle_inhibit_manager_v1 as idle_mgr, zwp_idle_inhibitor_v1 as idle};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};

enum Drag {
    Seek(i32, i32),
    Vol(i32, i32),
}

pub struct App {
    conn: Connection,
    qh: QueueHandle<App>,
    handle: LoopHandle<'static, App>,
    registry: RegistryState,
    seat: SeatState,
    outputs: OutputState,
    shm: Shm,
    window: Window,
    mpv: Mpv,
    cursor_mgr: Option<CursorShapeManager>,
    cursor_dev: Option<WpCursorShapeDeviceV1>,
    cursor_now: Option<Shape>,
    cursor_hidden: bool,
    enter_serial: u32,
    idle_mgr: Option<idle_mgr::ZwpIdleInhibitManagerV1>,
    inhibitor: Option<idle::ZwpIdleInhibitorV1>,
    scale: f64,
    hidpi: Option<(frac::WpFractionalScaleV1, wp_viewport::WpViewport)>,
    scratch: Vec<u32>,
    pointer: Option<wl_pointer::WlPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    mods: Modifiers,
    drag: Option<Drag>,
    press_act: Option<Act>,
    last_click: Option<Instant>,
    pool: Option<SlotPool>,
    configured: bool,
    dirty: bool,
    /// холст интерфейса (HiDPI) весь прозрачный: чистить перед кадром не нужно
    scratch_clean: bool,
    frame_pending: bool,
    anim: bool,
    zones: Vec<yami_px::ui::Zone<Act>>,
    fonts: Fonts,
    icons: Icons,
    p: P,
    quit: bool,
    title: String,
    ui_was: bool,
}

/// папка «изображения» из user-dirs.dirs: туда — сохранённые кадры
fn pictures() -> std::path::PathBuf {
    crate::paths::pictures()
}

pub fn run(mut p: P) -> i32 {
    let Ok(conn) = Connection::connect_to_env() else {
        eprintln!("yami-play: no Wayland display");
        return 1;
    };
    let Ok((globals, mut queue)) = registry_queue_init::<App>(&conn) else { return 1 };
    let qh = queue.handle();
    let mut ev: EventLoop<'static, App> = EventLoop::try_new().expect("event loop");
    let handle = ev.handle();
    // mpv будит цикл из своих нитей
    let (ping, ping_src) = calloop::ping::make_ping().expect("ping");
    handle.insert_source(ping_src, |_, _, app: &mut App| app.pump()).expect("ping source");
    let state = crate::paths::state_dir().join("play");
    let _ = std::fs::create_dir_all(&state);
    // позиции «продолжить с того же места» mpv не чистит сам: год не открывали — забыть
    let wl = state.join("watch_later");
    std::thread::spawn(move || crate::paths::tidy(&wl, 365, 64 << 20));
    let pics = pictures();
    let opts: Vec<(&str, String)> = vec![
        ("config", "no".into()),
        ("vo", "libmpv".into()),
        ("hwdec", "auto-copy-safe".into()),
        ("terminal", "no".into()),
        ("input-default-bindings", "no".into()),
        ("input-vo-keyboard", "no".into()),
        ("osc", "no".into()),
        ("osd-level", "0".into()),
        ("idle", "yes".into()),
        ("keep-open", "yes".into()),
        ("audio-client-name", "yami-play".into()),
        ("volume-max", "130".into()),
        ("sub-auto", "fuzzy".into()),
        // программный рендер: билинейный масштаб заметно дешевле lanczos, пиксельный режим — point
        ("zimg-scaler", "bilinear".into()),
        ("zimg-dither", "no".into()),
        ("sws-scaler", "bilinear".into()),
        ("save-position-on-quit", "yes".into()),
        ("resume-playback", "yes".into()),
        ("watch-later-dir", state.join("watch_later").to_string_lossy().to_string()),
        ("watch-later-options", "start,aid,sid".into()),
        ("screenshot-dir", pics.to_string_lossy().to_string()),
        ("screenshot-template", "yami-play-%F-%P".into()),
        ("screenshot-format", "png".into()),
    ];
    // свои опции mpv поверх наших: YAMI_PLAY_OPTS="hwdec=no,zimg-scaler=spline36"
    let extra = std::env::var("YAMI_PLAY_OPTS").unwrap_or_default();
    let mut o: Vec<(&str, &str)> = opts.iter().map(|(k, v)| (*k, v.as_str())).collect();
    o.extend(extra.split(',').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.trim(), v.trim())));
    let mpv = match Mpv::new(&o, Box::new(move || ping.ping())) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("yami-play: {e}");
            // без libmpv играть нечем: скажем, какой пакет поставить
            let msg = crate::tf!("нет {} — поставь пакет mpv (libmpv)", "no {}: install the mpv package (libmpv)", "{} ausente: instale o pacote mpv (libmpv)", e);
            let _ = std::process::Command::new("notify-send").args(["-a", "yami-play", "yami-play", &msg]).status();
            return 2;
        }
    };
    for k in [
        ("time-pos", 'd'),
        ("duration", 'd'),
        ("demuxer-cache-time", 'd'),
        ("pause", 'b'),
        ("paused-for-cache", 'b'),
        ("volume", 'd'),
        ("mute", 'b'),
        ("speed", 'd'),
        ("loop-file", 's'),
        ("media-title", 's'),
        ("metadata/by-key/artist", 's'),
        ("vo-configured", 'b'),
        ("dwidth", 'i'),
        ("dheight", 'i'),
        ("aid", 's'),
        ("sid", 's'),
        ("playlist-pos", 'i'),
        ("video-codec", 's'),
        ("audio-codec-name", 's'),
        ("hwdec-current", 's'),
        ("container-fps", 'd'),
        ("file-format", 's'),
        ("file-size", 'i'),
        ("track-list/count", 'i'),
        ("osd-dimensions/mt", 'd'),
        ("osd-dimensions/mb", 'd'),
        ("osd-dimensions/ml", 'd'),
        ("osd-dimensions/mr", 'd'),
    ] {
        mpv.observe(k.0, k.1);
    }
    for f in &p.list {
        mpv.cmd(&["loadfile", &f.to_string_lossy(), "append"]);
    }
    if !p.list.is_empty() {
        mpv.cmd(&["playlist-play-index", &p.idx.to_string()]);
    } else {
        p.err = Some(crate::i18n::tr("нет файлов").to_string());
    }
    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor");
    let xdg = XdgShell::bind(&globals, &qh).expect("xdg_shell");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm");
    let surface = compositor.create_surface(&qh);
    let window = xdg.create_window(surface, WindowDecorations::RequestServer, &qh);
    window.set_app_id("yami-play");
    window.set_title(p.win_title());
    window.set_min_size(Some((420, 260)));
    window.commit();
    let hidpi = {
        let fm: Option<frac_mgr::WpFractionalScaleManagerV1> = globals.bind(&qh, 1..=1, ()).ok();
        let vpr: Option<wp_viewporter::WpViewporter> = globals.bind(&qh, 1..=1, ()).ok();
        match (fm, vpr) {
            (Some(fm), Some(vpr)) => Some((fm.get_fractional_scale(window.wl_surface(), &qh, ()), vpr.get_viewport(window.wl_surface(), &qh, ()))),
            _ => None,
        }
    };
    let font = p.look.font("view").to_string();
    let mut app = App {
        conn: conn.clone(),
        qh: qh.clone(),
        handle: handle.clone(),
        registry: RegistryState::new(&globals),
        seat: SeatState::new(&globals, &qh),
        outputs: OutputState::new(&globals, &qh),
        shm,
        window,
        mpv,
        cursor_mgr: CursorShapeManager::bind(&globals, &qh).ok(),
        cursor_dev: None,
        cursor_now: None,
        cursor_hidden: false,
        enter_serial: 0,
        idle_mgr: globals.bind(&qh, 1..=1, ()).ok(),
        inhibitor: None,
        scale: 1.0,
        hidpi,
        scratch: vec![],
        pointer: None,
        keyboard: None,
        mods: Modifiers::default(),
        drag: None,
        press_act: None,
        last_click: None,
        pool: None,
        configured: false,
        dirty: true,
        frame_pending: false,
        scratch_clean: false,
        anim: false,
        zones: vec![],
        fonts: Fonts::with(&font),
        icons: Icons::new(""),
        p,
        quit: false,
        title: String::new(),
        ui_was: true,
    };
    handle
        .insert_source(Timer::from_duration(Duration::from_millis(100)), |_, _, app: &mut App| {
            app.tick();
            // прятать нечего — раз в секунду (кадры и время идут событиями mpv, не от тика)
            TimeoutAction::ToDuration(Duration::from_millis(if app.p.animating() { 40 } else if app.p.hide_soon() { 250 } else { 1000 }))
        })
        .expect("timer");
    let _ = queue.roundtrip(&mut app);
    WaylandSource::new(conn.clone(), queue).insert(handle.clone()).expect("wayland source");
    while !app.quit {
        if ev.dispatch(Some(Duration::from_secs(1)), &mut app).is_err() {
            break;
        }
        app.flush();
    }
    0
}

impl App {
    /// события mpv и новые кадры
    fn pump(&mut self) {
        while let Some(e) = self.mpv.event() {
            match e {
                Event::Prop(k, v) => {
                    let was_paused = self.p.paused;
                    self.p.prop(&k, v);
                    if k == "time-pos" && !self.p.ui_visible() {
                        // время на спрятанных панелях не рисуем
                        continue;
                    }
                    if k == "pause" && was_paused != self.p.paused {
                        self.p.active_at = Instant::now();
                    }
                    self.dirty = true;
                }
                Event::FileLoaded => {
                    self.p.loaded = true;
                    self.p.err = None;
                    self.dirty = true;
                }
                Event::VideoReconfig => self.dirty = true,
                Event::EndFile(err) => {
                    if let Some(e) = err {
                        self.p.toast(e.clone());
                        self.p.err = Some(e);
                    }
                    self.dirty = true;
                }
                Event::Shutdown => self.quit = true,
            }
        }
        if self.mpv.frame_ready() {
            self.dirty = true;
        }
        self.inhibit();
        self.effects();
    }

    fn tick(&mut self) {
        if self.p.look.stale() {
            let font = self.p.look.font("view").to_string();
            self.p.look = crate::look::Look::load();
            if self.p.look.font("view") != font {
                self.fonts = Fonts::with(self.p.look.font("view"));
            }
            crate::i18n::set(&self.p.look.lang);
            self.dirty = true;
        }
        let vis = self.p.ui_visible();
        if vis != self.ui_was || self.p.animating() {
            self.dirty = true;
        }
        self.ui_was = vis;
        // курсор прячется вместе с панелями
        if !vis && !self.cursor_hidden && self.p.ptr.is_some() {
            if let Some(ptr) = &self.pointer {
                ptr.set_cursor(self.enter_serial, None, 0, 0);
                self.cursor_hidden = true;
                self.cursor_now = None;
            }
        }
        self.flush();
    }

    /// экран не гаснет, пока идёт видео
    fn inhibit(&mut self) {
        let want = self.p.video && !self.p.paused && self.p.loaded;
        if want == self.inhibitor.is_some() {
            return;
        }
        match (&self.idle_mgr, want) {
            (Some(m), true) => self.inhibitor = Some(m.create_inhibitor(self.window.wl_surface(), &self.qh, ())),
            _ => {
                if let Some(i) = self.inhibitor.take() {
                    i.destroy();
                }
            }
        }
    }

    fn effects(&mut self) {
        for f in std::mem::take(&mut self.p.fx) {
            match f {
                Fx::Close => self.quit = true,
                Fx::Fullscreen(on) => {
                    if on {
                        self.window.set_fullscreen(None);
                    } else {
                        self.window.unset_fullscreen();
                    }
                }
                Fx::Cmd(a) => {
                    let a: Vec<&str> = a.iter().map(|s| s.as_str()).collect();
                    self.mpv.cmd(&a);
                }
                Fx::Set(k, v) => self.mpv.set(k, &v),
            }
        }
        let t = self.p.win_title();
        if t != self.title {
            self.window.set_title(t.clone());
            self.title = t;
        }
        self.dirty = true;
    }

    fn flush(&mut self) {
        if self.dirty && self.configured && !self.frame_pending {
            self.draw();
        }
        let _ = self.conn.flush();
    }

    fn draw(&mut self) {
        self.dirty = false;
        let (w, h) = (self.p.w, self.p.h);
        let (pw, ph) = if self.hidpi.is_none() || self.scale == 1.0 { (w, h) } else { yami_px::gfx::phys(w, h, self.scale) };
        let up = (pw, ph) != (w, h);
        let need = (pw * ph * 4) as usize;
        if self.pool.as_ref().map(|p| p.len() < need).unwrap_or(true) {
            self.pool = SlotPool::new(need * 2, &self.shm).ok();
        }
        let Some(pool) = self.pool.as_mut() else { return };
        let Ok((buffer, bytes)) = pool.create_buffer(pw, ph, pw * 4, wl_shm::Format::Xrgb8888) else { return };
        let (out_ptr, out_len) = (bytes.as_mut_ptr() as *mut u32, bytes.len() / 4);
        // SAFETY: буфер пула живёт до attach ниже, других ссылок на него нет
        let out = unsafe { std::slice::from_raw_parts_mut(out_ptr, out_len) };
        let video = self.p.video && self.p.err.is_none();
        if video {
            self.mpv.render(out, pw, ph);
            // поля вокруг кадра — цветом темы (mpv в программном режиме льёт чёрный); в светлой
            // теме — её самым тёмным тоном: светлые поля вокруг кино слепят
            let [mt, mb, ml, mr] = self.p.margins.map(|m| m.max(0.0) as i32);
            let t = &self.p.look.theme;
            let c = if t.light { t.text.0 } else { t.bg.0 } | 0xff00_0000;
            let mut cv = Canvas::new(out, pw, ph);
            for r in [yami_px::gfx::R::new(0, 0, pw, mt), yami_px::gfx::R::new(0, ph - mb, pw, mb), yami_px::gfx::R::new(0, mt, ml, ph - mt - mb), yami_px::gfx::R::new(pw - mr, mt, mr, ph - mt - mb)] {
                if !r.empty() {
                    cv.set(r, yami_px::gfx::C(c));
                }
            }
        }
        let mut scratch = std::mem::take(&mut self.scratch);
        let drew;
        {
            let buf: &mut [u32] = if up {
                // холст интерфейса остался пустым с прошлого кадра (панели спрятаны): не чистить его
                // заново на каждом кадре видео
                if !self.scratch_clean || scratch.len() != (w * h) as usize {
                    scratch.clear();
                    scratch.resize((w * h) as usize, 0);
                }
                &mut scratch
            } else {
                out
            };
            let mut ui = Ui { cv: Canvas::new(buf, w, h), f: &mut self.fonts, ic: &mut self.icons, t: self.p.look.theme, zones: vec![], ptr: self.p.ptr, down: self.press_act.is_some(), anim: false, blink: true, tr: crate::i18n::tr };
            drew = crate::play::draw(&mut ui, &self.p, w, h, !video);
            self.zones = std::mem::take(&mut ui.zones);
            self.anim = ui.anim;
        }
        if up {
            // SAFETY: тот же буфер пула; прошлая ссылка `out` больше не используется
            let out = unsafe { std::slice::from_raw_parts_mut(out_ptr, out_len) };
            if !video {
                yami_px::gfx::upscale(&scratch, w, h, out, pw, ph);
            } else if drew {
                yami_px::gfx::overlay_up(&scratch, w, h, out, pw, ph);
            }
            // поверх видео пусто — кадр mpv уже в буфере, проходить по нему незачем
            self.scratch_clean = !drew;
        }
        self.scratch = scratch;
        let surf = self.window.wl_surface().clone();
        if let Some((_, vp)) = &self.hidpi {
            vp.set_destination(w, h);
        }
        surf.damage_buffer(0, 0, pw, ph);
        // кадры композитора задают темп, пока идёт видео или анимация
        if self.anim || (video && !self.p.paused) {
            surf.frame(&self.qh, surf.clone());
            self.frame_pending = true;
        }
        let _ = buffer.attach_to(&surf);
        surf.commit();
    }

    fn zone_at(&self, x: f64, y: f64) -> Option<yami_px::ui::Zone<Act>> {
        self.zones.iter().rev().find(|z| z.r.contains(x, y)).cloned()
    }

    fn set_cursor(&mut self, c: Shape) {
        if self.cursor_now == Some(c) && !self.cursor_hidden {
            return;
        }
        self.cursor_now = Some(c);
        self.cursor_hidden = false;
        if let Some(d) = &self.cursor_dev {
            d.set_shape(self.enter_serial, c);
        }
    }

    fn key(&mut self, e: &KeyEvent) {
        let (c, s) = (self.mods.ctrl, self.mods.shift);
        let p = &mut self.p;
        p.active_at = Instant::now();
        let step = if s { 1.0 } else if c { 30.0 } else { 5.0 };
        match e.keysym {
            Keysym::Escape => {
                if p.info {
                    p.info = false;
                } else if p.full {
                    p.btn("full");
                } else {
                    p.btn("close");
                }
            }
            Keysym::space | Keysym::XF86_AudioPlay | Keysym::XF86_AudioPause => p.btn("play"),
            Keysym::Left => p.seek_rel(-step),
            Keysym::Right => p.seek_rel(step),
            Keysym::Up => p.set_volume(p.volume + 5.0),
            Keysym::Down => p.set_volume(p.volume - 5.0),
            Keysym::Page_Up | Keysym::XF86_AudioPrev => p.go(-1),
            Keysym::Page_Down | Keysym::XF86_AudioNext => p.go(1),
            Keysym::Home => p.btn("start"),
            Keysym::Return | Keysym::KP_Enter | Keysym::F11 => p.btn("full"),
            Keysym::BackSpace => p.btn("speed"),
            Keysym::XF86_AudioMute => p.btn("mute"),
            _ => match e.raw_code {
                // по физическим клавишам — и в русской раскладке
                16 if c => p.btn("close"), // ctrl+q
                16 => p.btn("close"),      // q
                37 => p.btn("play"),       // k
                36 => p.seek_rel(-10.0),   // j
                38 => p.btn("loop"),       // l
                33 => p.btn("full"),       // f
                50 => p.btn("mute"),       // m
                49 => p.go(1),             // n
                25 => p.go(-1),            // p
                30 => p.btn("audio"),      // a
                31 => p.btn("subs"),       // s
                45 => p.btn("pixel"),      // x
                46 => p.btn("shot"),       // c
                23 => p.btn("info"),       // i
                18 => p.btn("folder"),     // e
                26 => p.btn("slower"),     // [
                27 => p.btn("faster"),     // ]
                51 => p.btn("frameback"),  // ,
                52 => p.btn("frame"),      // .
                // 1…9 и 0 — к 10 %…90 % и в начало
                2..=10 => p.seek_frac((e.raw_code - 1) as f64 / 10.0, true),
                11 => p.seek_frac(0.0, true),
                _ => {}
            },
        }
        self.effects();
    }

    /// доля по горизонтали внутри зоны (у шкалы — без полей по 4 px)
    fn frac_in(x: f64, x0: i32, x1: i32) -> f64 {
        ((x - x0 as f64) / (x1 - x0).max(1) as f64).clamp(0.0, 1.0)
    }

    fn on_pointer(&mut self, e: &PointerEvent) {
        let (x, y) = e.position;
        match e.kind {
            PointerEventKind::Enter { serial } => {
                self.enter_serial = serial;
                self.cursor_now = None;
                self.p.ptr = Some((x, y));
                self.p.active_at = Instant::now();
                self.set_cursor(Shape::Default);
                self.dirty = true;
            }
            PointerEventKind::Leave { .. } => {
                self.p.ptr = None;
                self.p.over_ui = false;
                self.dirty = true;
            }
            PointerEventKind::Motion { .. } => {
                let moved = self.p.ptr.map(|(px, py)| (px - x).abs() + (py - y).abs() > 0.5).unwrap_or(true);
                self.p.ptr = Some((x, y));
                if moved {
                    self.p.active_at = Instant::now();
                }
                let z = self.zone_at(x, y);
                self.p.over_ui = matches!(z.as_ref().map(|z| &z.act), Some(Act::Btn(_) | Act::Block | Act::Seek | Act::Vol));
                match self.drag {
                    Some(Drag::Seek(x0, x1)) => {
                        let k = Self::frac_in(x, x0, x1);
                        self.p.seek_drag = Some(k);
                        self.p.seek_frac(k, false);
                        self.effects();
                    }
                    Some(Drag::Vol(x0, x1)) => {
                        let k = Self::frac_in(x, x0, x1);
                        self.p.set_volume((k * 100.0 / 10.0).ceil() * 10.0);
                        self.effects();
                    }
                    None => {
                        let shape = match z.map(|z| z.act) {
                            Some(Act::Btn(_) | Act::Seek | Act::Vol) => Shape::Pointer,
                            _ => Shape::Default,
                        };
                        if moved || !self.cursor_hidden {
                            self.set_cursor(shape);
                        }
                    }
                }
                self.dirty = true;
            }
            PointerEventKind::Press { button, .. } => {
                let z = self.zone_at(x, y);
                match button {
                    0x113 => self.p.go(-1),
                    0x114 => self.p.go(1),
                    0x112 => self.p.btn("mute"),
                    0x111 => self.p.btn("play"),
                    0x110 => match z.as_ref().map(|z| (z.act.clone(), z.r)) {
                        Some((Act::Bg, _)) => {
                            let dbl = self.last_click.map(|t| t.elapsed() < Duration::from_millis(350)).unwrap_or(false);
                            self.last_click = Some(Instant::now());
                            if dbl {
                                // второй щелчок вернул паузу как была и развернул окно
                                self.p.btn("play");
                                self.p.btn("full");
                                self.last_click = None;
                            } else if self.p.loaded {
                                self.p.btn("play");
                            }
                        }
                        Some((Act::Seek, r)) => {
                            let (x0, x1) = (r.x + 4, r.right() - 4);
                            let k = Self::frac_in(x, x0, x1);
                            self.p.seek_drag = Some(k);
                            self.p.seek_frac(k, false);
                            self.drag = Some(Drag::Seek(x0, x1));
                        }
                        Some((Act::Vol, r)) => {
                            let (x0, x1) = (r.x + 4, r.right() - 4);
                            self.p.set_volume((Self::frac_in(x, x0, x1) * 10.0).ceil() * 10.0);
                            self.drag = Some(Drag::Vol(x0, x1));
                        }
                        Some((a, _)) => self.press_act = Some(a),
                        None => {}
                    },
                    _ => {}
                }
                self.effects();
            }
            PointerEventKind::Release { button, .. } => {
                if button == 0x110 {
                    // отпустили шкалу — точная перемотка туда
                    if let (Some(Drag::Seek(..)), Some(k)) = (&self.drag, self.p.seek_drag) {
                        self.p.seek_frac(k, true);
                    }
                    self.p.seek_drag = None;
                    self.drag = None;
                    if let Some(a) = self.press_act.take() {
                        if self.zone_at(x, y).map(|z| z.act == a).unwrap_or(false) {
                            if let Act::Btn(b) = a {
                                self.p.btn(b);
                            }
                        }
                    }
                }
                self.effects();
            }
            PointerEventKind::Axis { vertical, horizontal, .. } => {
                let d = |a: &smithay_client_toolkit::seat::pointer::AxisScroll| if a.value120 != 0 { a.value120 as f64 / 120.0 } else if a.discrete != 0 { a.discrete as f64 } else { a.absolute / 15.0 };
                let (dv, dh) = (d(&vertical), d(&horizontal));
                self.p.active_at = Instant::now();
                let over_seek = matches!(self.zone_at(x, y).map(|z| z.act), Some(Act::Seek));
                if dh.abs() >= 0.5 || over_seek && dv.abs() >= 0.5 {
                    // вбок или над шкалой — перемотка
                    let k = if dh.abs() >= 0.5 { dh } else { dv };
                    self.p.seek_rel(if k > 0.0 { 5.0 } else { -5.0 });
                } else if dv.abs() >= 0.5 {
                    // колесо — громкость
                    self.p.set_volume(self.p.volume - dv.signum() * 5.0);
                }
                self.effects();
            }
        }
    }
}

impl CompositorHandler for App {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
        self.frame_pending = false;
        if self.anim {
            self.dirty = true;
        }
    }
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl WindowHandler for App {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        self.quit = true;
    }
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window, c: WindowConfigure, _: u32) {
        if let (Some(w), Some(h)) = c.new_size {
            self.p.w = w.get() as i32;
            self.p.h = h.get() as i32;
        }
        self.p.full = c.is_fullscreen();
        self.configured = true;
        self.dirty = true;
        self.frame_pending = false;
        self.draw();
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
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32, _: &[u32], _: &[Keysym]) {}
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32) {}
    fn press_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, e: KeyEvent) {
        self.key(&e);
    }
    fn repeat_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, e: KeyEvent) {
        self.key(&e);
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

impl Dispatch<frac_mgr::WpFractionalScaleManagerV1, ()> for App {
    fn event(_: &mut Self, _: &frac_mgr::WpFractionalScaleManagerV1, _: frac_mgr::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<wp_viewporter::WpViewporter, ()> for App {
    fn event(_: &mut Self, _: &wp_viewporter::WpViewporter, _: wp_viewporter::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<wp_viewport::WpViewport, ()> for App {
    fn event(_: &mut Self, _: &wp_viewport::WpViewport, _: wp_viewport::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<frac::WpFractionalScaleV1, ()> for App {
    fn event(app: &mut Self, _: &frac::WpFractionalScaleV1, e: frac::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let frac::Event::PreferredScale { scale } = e {
            let v = scale as f64 / 120.0;
            if v != app.scale {
                app.scale = v;
                app.dirty = true;
            }
        }
    }
}
impl Dispatch<idle_mgr::ZwpIdleInhibitManagerV1, ()> for App {
    fn event(_: &mut Self, _: &idle_mgr::ZwpIdleInhibitManagerV1, _: idle_mgr::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<idle::ZwpIdleInhibitorV1, ()> for App {
    fn event(_: &mut Self, _: &idle::ZwpIdleInhibitorV1, _: idle::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

delegate_compositor!(App);
delegate_output!(App);
delegate_shm!(App);
delegate_seat!(App);
delegate_keyboard!(App);
delegate_pointer!(App);
delegate_xdg_shell!(App);
delegate_xdg_window!(App);
delegate_registry!(App);
