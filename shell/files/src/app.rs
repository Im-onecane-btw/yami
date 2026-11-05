// Окно файлов: xdg-toplevel, shm-буфер, ввод, буфер обмена и перетаскивание (wl_data_device).
// Рисует draw.rs по состоянию Fm; сюда приходят только события и «эффекты» Fm.

use crate::act::{Act, Ui};
use crate::draw;
use crate::fm::{Dlg, Fm, Focus, Fx, Msg, Thumb};
use crate::font::Fonts;
use crate::gfx::{Canvas, R};
use crate::icons::Icons;
use crate::keys::Key;
use crate::ui::Cur;
use calloop::channel::{self, Sender};
use calloop::timer::{TimeoutAction, Timer};
use calloop::{EventLoop, LoopHandle};
use calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::{
    compositor::{CompositorState, Region},
    data_device_manager::{
        data_device::DataDevice,
        data_source::{CopyPasteSource, DragSource},
        DataDeviceManagerState,
    },
    output::OutputState,
    registry::RegistryState,
    seat::{
        keyboard::{KeyEvent, Keysym, Modifiers},
        pointer::cursor_shape::CursorShapeManager, SeatState,
    },
    shell::{
        xdg::{
            window::{Window, WindowDecorations},
            XdgShell,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm},
};
use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, QueueHandle,
};
use wayland_protocols::ext::background_effect::v1::client::{ext_background_effect_manager_v1, ext_background_effect_surface_v1};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::{Shape, WpCursorShapeDeviceV1};
use wayland_protocols::wp::fractional_scale::v1::client::{wp_fractional_scale_manager_v1 as frac_mgr, wp_fractional_scale_v1 as frac};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};

pub enum Ev {
    Fm(Msg),
    /// прочитали из буфера обмена / перетаскивания: пути и «вырезано»
    Paste(PathBuf, Vec<PathBuf>, bool),
    Dropped { x: i32, y: i32, dst: PathBuf, paths: Vec<PathBuf>, ours: bool },
}

pub(crate) struct Press {
    pub(crate) act: Act,
    pub(crate) button: u32,
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) moved: bool,
    pub(crate) serial: u32,
    /// выделение до начала рамки (ctrl+рамка добавляет)
    pub(crate) base: std::collections::HashSet<PathBuf>,
    pub(crate) ctrl: bool,
    pub(crate) scroll0: f32,
}

pub struct App {
    pub(crate) conn: Connection,
    pub(crate) qh: QueueHandle<App>,
    pub(crate) handle: LoopHandle<'static, App>,
    pub(crate) tx: Sender<Ev>,
    pub(crate) registry: RegistryState,
    pub(crate) seat: SeatState,
    pub(crate) outputs: OutputState,
    pub(crate) compositor: CompositorState,
    pub(crate) shm: Shm,
    pub(crate) window: Window,
    pub(crate) cursor_mgr: Option<CursorShapeManager>,
    pub(crate) cursor_dev: Option<WpCursorShapeDeviceV1>,
    pub(crate) cursor_now: Option<Shape>,
    pub(crate) fx_mgr: Option<ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1>,
    pub(crate) fx: Option<ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1>,
    /// HiDPI: буфер в физических пикселях (wp_fractional_scale + viewport), холст — логический
    pub(crate) scale: f64,
    pub(crate) hidpi: Option<(frac::WpFractionalScaleV1, wp_viewport::WpViewport)>,
    pub(crate) scratch: Vec<u32>,
    pub(crate) blur_set: Vec<R>,
    pub(crate) ddm: Option<DataDeviceManagerState>,
    pub(crate) ddev: Option<DataDevice>,
    pub(crate) seat_obj: Option<wl_seat::WlSeat>,
    pub(crate) clip_src: Option<CopyPasteSource>,
    pub(crate) clip_data: (Vec<PathBuf>, bool),
    pub(crate) drag_src: Option<DragSource>,
    pub(crate) drag_icon: Option<(wl_surface::WlSurface, SlotPool)>,
    pub(crate) drag_paths: Vec<PathBuf>,
    pub(crate) pointer: Option<wl_pointer::WlPointer>,
    pub(crate) keyboard: Option<wl_keyboard::WlKeyboard>,
    pub(crate) mods: Modifiers,
    pub(crate) ptr: Option<(f64, f64)>,
    pub(crate) press: Option<Press>,
    pub(crate) last_click: Option<(Act, Instant)>,
    pub(crate) serial: u32,
    pub(crate) enter_serial: u32,
    pub(crate) kb_serial: u32,
    pub(crate) pool: Option<SlotPool>,
    pub(crate) w: i32,
    pub(crate) h: i32,
    pub(crate) configured: bool,
    pub(crate) dirty: bool,
    pub(crate) frame_pending: bool,
    pub(crate) anim: bool,
    /// прошлый шаг плавной прокрутки
    pub(crate) scroll_t: Instant,
    pub(crate) zones: Vec<Zone<Act>>,
    pub(crate) fonts: Fonts,
    pub(crate) icons: Icons,
    pub fm: Fm,
    pub(crate) quit: bool,
    pub(crate) blink: bool,
    pub(crate) blink_at: Instant,
    pub(crate) title: String,
}

type ZoneA = Zone<Act>;
use yami_px::ui::Zone;

pub fn run(fm_init: impl FnOnce(Option<calloop::channel::Sender<Msg>>) -> Fm) -> Fm {
    let conn = Connection::connect_to_env().expect("no Wayland display");
    let (globals, mut queue) = registry_queue_init::<App>(&conn).expect("wayland registry");
    let qh = queue.handle();
    let mut ev: EventLoop<'static, App> = EventLoop::try_new().expect("event loop");
    let handle = ev.handle();
    let (tx, rx) = channel::channel::<Ev>();
    handle
        .insert_source(rx, |e, _, app: &mut App| {
            if let channel::Event::Msg(m) = e {
                app.on_ev(m);
            }
        })
        .expect("channel");
    // сообщения Fm (нити) — через свой канал, переливаем в общий
    let (ftx, frx) = channel::channel::<Msg>();
    handle
        .insert_source(frx, |e, _, app: &mut App| {
            if let channel::Event::Msg(m) = e {
                app.on_ev(Ev::Fm(m));
            }
        })
        .expect("fm channel");
    let fm = fm_init(Some(ftx));

    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor");
    let xdg = XdgShell::bind(&globals, &qh).expect("xdg_shell");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm");
    let surface = compositor.create_surface(&qh);
    let window = xdg.create_window(surface, WindowDecorations::RequestServer, &qh);
    let pick = fm.pick.is_some();
    window.set_app_id(if pick { "yami-files-pick" } else { "yami-files" });
    window.set_title(fm.title());
    window.set_min_size(Some((480, 320)));
    window.commit();
    let fx_mgr: Option<ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1> = globals.bind(&qh, 1..=1, ()).ok();
    let (w, h) = if pick { (980, 620) } else { (fm.cfg.width.max(480), fm.cfg.height.max(320)) };
    let mut icons = Icons::new(&fm.look.icon_theme);
    icons.set_pixel(fm.look.icon_pixel.then(|| fm.look.theme.icon_ramp()));
    let hidpi = {
        let fm: Option<frac_mgr::WpFractionalScaleManagerV1> = globals.bind(&qh, 1..=1, ()).ok();
        let vpr: Option<wp_viewporter::WpViewporter> = globals.bind(&qh, 1..=1, ()).ok();
        match (fm, vpr) {
            (Some(fm), Some(vpr)) => Some((fm.get_fractional_scale(window.wl_surface(), &qh, ()), vpr.get_viewport(window.wl_surface(), &qh, ()))),
            _ => None,
        }
    };
    let mut app = App {
        conn: conn.clone(),
        qh: qh.clone(),
        handle: handle.clone(),
        tx: tx.clone(),
        registry: RegistryState::new(&globals),
        seat: SeatState::new(&globals, &qh),
        outputs: OutputState::new(&globals, &qh),
        compositor,
        shm,
        window,
        cursor_mgr: CursorShapeManager::bind(&globals, &qh).ok(),
        cursor_dev: None,
        cursor_now: None,
        fx_mgr,
        fx: None,
        scale: 1.0,
        hidpi,
        scratch: vec![],
        blur_set: vec![],
        ddm: DataDeviceManagerState::bind(&globals, &qh).ok(),
        ddev: None,
        seat_obj: None,
        clip_src: None,
        clip_data: (vec![], false),
        drag_src: None,
        drag_icon: None,
        drag_paths: vec![],
        pointer: None,
        keyboard: None,
        mods: Modifiers::default(),
        ptr: None,
        press: None,
        last_click: None,
        serial: 0,
        enter_serial: 0,
        kb_serial: 0,
        pool: None,
        w,
        h,
        configured: false,
        dirty: true,
        frame_pending: false,
        scroll_t: Instant::now(),
        anim: false,
        zones: vec![],
        fonts: Fonts::with(fm.look.font("files")),
        icons,
        fm,
        quit: false,
        blink: true,
        blink_at: Instant::now(),
        title: String::new(),
    };
    app.fm.w = w;
    app.fm.h = h;
    // миниатюры и слежка за папками — свои нити
    {
        let tx = tx.clone();
        app.fm.worker = Some(crate::thumbs::Thumbs::spawn(move |p, s, img| {
            let _ = tx.send(Ev::Fm(Msg::Thumb(p, s, img)));
        }));
    }
    let watch = {
        let tx = tx.clone();
        crate::watch::Watch::spawn(move |p| {
            let _ = tx.send(Ev::Fm(Msg::Changed(p)));
        })
    };
    handle
        .insert_source(Timer::from_duration(Duration::from_millis(100)), move |_, _, app: &mut App| {
            if app.fm.watch_dirty {
                app.fm.watch_dirty = false;
                if let Some(w) = &watch {
                    w.set(&app.fm.watched());
                }
            }
            app.tick();
            TimeoutAction::ToDuration(Duration::from_millis(if app.anim { 50 } else { 250 }))
        })
        .expect("timer");
    let _ = queue.roundtrip(&mut app);
    WaylandSource::new(conn.clone(), queue).insert(handle.clone()).expect("wayland source");
    loop {
        if ev.dispatch(Some(Duration::from_secs(2)), &mut app).is_err() {
            break;
        }
        app.flush();
        if app.quit {
            break;
        }
    }
    // размер окна запомнить (только у обычного окна)
    if app.fm.pick.is_none() {
        app.fm.cfg.width = app.w;
        app.fm.cfg.height = app.h;
    }
    app.fm.save_cfg();
    let _ = conn.flush();
    app.fm
}

impl App {
    pub(crate) fn tick(&mut self) {
        // настройки шелла поменялись — новые цвета, язык
        if self.fm.look.stale() {
            let font = self.fm.look.font("files").to_string();
            self.fm.look = crate::look::Look::load();
            if self.fm.look.font("files") != font {
                self.fonts = Fonts::with(self.fm.look.font("files"));
            }
            crate::i18n::set(&self.fm.look.lang);
            self.icons.set_pixel(self.fm.look.icon_pixel.then(|| self.fm.look.theme.icon_ramp()));
            self.dirty = true;
        }
        // мигание курсора в полях
        let typing = self.fm.focus != Focus::Content || self.fm.dlg.as_ref().map(|d| matches!(d.0, Dlg::Name { .. } | Dlg::OpenWith { .. } | Dlg::Server { .. })).unwrap_or(false) || self.fm.v().rename.is_some() || self.fm.pick.as_ref().map(|p| p.name_focus).unwrap_or(false);
        if typing && self.blink_at.elapsed().as_millis() > 530 {
            self.blink = !self.blink;
            self.blink_at = Instant::now();
            self.dirty = true;
        } else if !typing && !self.blink {
            self.blink = true;
        }
        // отложенные перечитывания по inotify
        let due: Vec<PathBuf> = self.fm.changed.iter().filter(|(_, t)| t.elapsed().as_millis() > 120).map(|(p, _)| p.clone()).collect();
        for p in due {
            self.fm.changed.remove(&p);
            if p.ends_with("Trash/files") {
                self.fm.trash_n = crate::trash::count();
                let tabs: Vec<(usize, usize)> = self.fm.tabs.iter().enumerate().flat_map(|(ti, t)| t.panes.iter().enumerate().filter(|(_, v)| v.trash).map(move |(pi, _)| (ti, pi))).collect();
                for (t, pn) in tabs {
                    self.fm.load(t, pn);
                }
            }
            self.fm.reload_all_of(&p);
            self.dirty = true;
        }
        if self.fm.dev_tick.elapsed().as_secs() >= 4 {
            self.fm.refresh_devices();
        }
        if self.fm.toast.as_ref().map(|t| t.1.elapsed().as_millis() > 3300).unwrap_or(false) {
            self.fm.toast = None;
            self.dirty = true;
        }
        self.fm.save_cfg();
    }

    pub(crate) fn on_ev(&mut self, e: Ev) {
        match e {
            Ev::Fm(m) => match m {
                Msg::Listed { view, gen, res } => self.fm.on_listed(view, gen, res),
                Msg::Changed(p) => {
                    self.fm.changed.insert(p, Instant::now());
                }
                Msg::Thumb(p, s, img) => {
                    self.fm.thumbs.insert((p, s), match img {
                        Some(i) => Thumb::Ready(std::rc::Rc::new(i)),
                        None => Thumb::None,
                    });
                }
                Msg::Op(e) => self.fm.on_op(e),
                Msg::Du { gen, v, done } => self.fm.on_du(gen, v, done),
                Msg::Devices(d) => {
                    if d != self.fm.devs {
                        self.fm.devs = d;
                    }
                }
                Msg::Remote { uri, res, auth } => self.fm.on_remote(uri, res, auth),
                Msg::Mounted { err, open } => {
                    if let Some(e) = err {
                        self.fm.toast(e);
                    } else if let Some(p) = open {
                        self.fm.go(p);
                    }
                }
            },
            Ev::Paste(dir, paths, cut) => {
                if paths.is_empty() {
                    self.fm.toast(crate::i18n::tr("в буфере нет файлов"));
                } else {
                    let kind = if cut { crate::ops::Kind::Move } else { crate::ops::Kind::Copy };
                    self.fm.job(kind, paths, dir, String::new());
                    if cut {
                        self.fm.clip = None;
                    }
                }
            }
            Ev::Dropped { x, y, dst, paths, ours } => {
                if paths.is_empty() {
                    return;
                }
                // в ту же папку, откуда тащили — ничего не делать
                if ours && paths.iter().all(|p| p.parent() == Some(dst.as_path())) {
                    return;
                }
                if dst.starts_with(crate::trash::home_trash()) || self.fm.v().trash && dst == self.fm.v().dir {
                    self.fm.job(crate::ops::Kind::Trash, paths, PathBuf::new(), String::new());
                    return;
                }
                let title = dst.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "/".into());
                crate::menus::open(&mut self.fm, crate::menus::for_drop(paths, dst), x, y, &title);
            }
        }
        self.after_input();
    }

    pub(crate) fn after_input(&mut self) {
        self.dirty = true;
        let fx: Vec<Fx> = std::mem::take(&mut self.fm.fx);
        for f in fx {
            match f {
                Fx::Close => self.quit = true,
                Fx::Title(_) => {}
                Fx::Clip(paths, cut) => self.set_clip(paths, cut),
                Fx::Paste(dir) => self.paste(dir),
            }
        }
        let t = self.fm.title();
        if t != self.title {
            self.window.set_title(t.clone());
            self.title = t;
        }
    }

    // ── буфер обмена ───────────────────────────────────────────────────────
    pub(crate) fn set_clip(&mut self, paths: Vec<PathBuf>, cut: bool) {
        let (Some(ddm), Some(dev)) = (&self.ddm, &self.ddev) else {
            // без data device — хотя бы списком путей через wl-copy
            crate::fm::copy_text(&paths.iter().map(|p| crate::icons::file_uri(p)).collect::<Vec<_>>().join("\n"));
            return;
        };
        let src = ddm.create_copy_paste_source(&self.qh, ["x-special/gnome-copied-files", "text/uri-list", "application/x-kde-cutselection", "text/plain;charset=utf-8", "text/plain", "UTF8_STRING"]);
        src.set_selection(dev, self.kb_serial.max(self.serial));
        self.clip_src = Some(src);
        self.clip_data = (paths, cut);
    }

    pub(crate) fn paste(&mut self, dir: PathBuf) {
        // своё — без круга через композитор
        if self.clip_src.is_some() && !self.clip_data.0.is_empty() {
            let (p, cut) = self.clip_data.clone();
            if cut {
                self.clip_src = None;
                self.clip_data = (vec![], false);
            }
            self.on_ev(Ev::Paste(dir, p, cut));
            return;
        }
        let Some(dev) = &self.ddev else { return };
        let Some(offer) = dev.data().selection_offer() else {
            self.fm.toast(crate::i18n::tr("буфер обмена пуст"));
            return;
        };
        let mimes: Vec<String> = offer.with_mime_types(|m| m.to_vec());
        let pick = ["x-special/gnome-copied-files", "text/uri-list", "text/plain;charset=utf-8", "text/plain"].into_iter().find(|m| mimes.iter().any(|x| x == m));
        let Some(mime) = pick else {
            self.fm.toast(crate::i18n::tr("в буфере нет файлов"));
            return;
        };
        let kde_cut = mimes.iter().any(|m| m == "application/x-kde-cutselection");
        let cut_pipe = if kde_cut { offer.receive("application/x-kde-cutselection".into()).ok() } else { None };
        let Ok(mut pipe) = offer.receive(mime.to_string()) else { return };
        let tx = self.tx.clone();
        let _ = self.conn.flush();
        std::thread::spawn(move || {
            let mut s = String::new();
            let _ = pipe.read_to_string(&mut s);
            let mut cut = false;
            if let Some(mut c) = cut_pipe {
                let mut v = String::new();
                let _ = c.read_to_string(&mut v);
                cut = v.trim() == "1";
            }
            let mut lines: Vec<&str> = s.lines().map(|l| l.trim()).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
            if let Some(first) = lines.first() {
                if *first == "cut" || *first == "copy" {
                    cut = *first == "cut";
                    lines.remove(0);
                }
            }
            let paths: Vec<PathBuf> = lines.iter().filter_map(|l| crate::icons::uri_path(l).or_else(|| if l.starts_with('/') { Some(PathBuf::from(l)) } else { None })).filter(|p| std::fs::symlink_metadata(p).is_ok()).collect();
            let _ = tx.send(Ev::Paste(dir, paths, cut));
        });
    }

    // ── отрисовка ──────────────────────────────────────────────────────────
    pub(crate) fn flush(&mut self) {
        if self.dirty && self.configured && !self.frame_pending {
            self.draw();
        }
        // заявки на миниатюры, собранные кадром
        let want: Vec<crate::thumbs::Req> = std::mem::take(&mut *self.fm.want.borrow_mut());
        for r in want {
            let key = (r.path.clone(), r.size);
            if self.fm.thumbs.contains_key(&key) {
                continue;
            }
            self.fm.thumbs.insert(key, Thumb::Wait);
            if let Some(w) = &self.fm.worker {
                w.ask(r);
            }
        }
        let _ = self.conn.flush();
    }

    /// плавная прокрутка колесом: шаг по времени кадра (frame callback), а не по таймеру —
    /// таймер тикает раз в 50–250 мс, и список ехал рывками. true — ещё едет
    pub(crate) fn step_scroll(&mut self) -> bool {
        let now = Instant::now();
        // первый кадр после покоя — как один кадр 60 Гц
        let dt = now.duration_since(self.scroll_t).as_secs_f32().min(1.0 / 30.0);
        self.scroll_t = now;
        let k = 1.0 - 0.7f32.powf(dt * 60.0);
        let mut moving = false;
        for t in self.fm.tabs.iter_mut() {
            for v in t.panes.iter_mut() {
                let d = v.target - v.scroll;
                if d.abs() > 0.5 {
                    v.scroll += d * k;
                    moving = true;
                } else if d != 0.0 {
                    v.scroll = v.target;
                }
            }
        }
        moving
    }

    pub(crate) fn draw(&mut self) {
        self.dirty = false;
        let scrolling = self.step_scroll();
        let (w, h) = (self.w, self.h);
        // HiDPI: буфер в физических пикселях, холст логический, увеличение ближайшим соседом
        let (pw, ph) = if self.hidpi.is_none() || self.scale == 1.0 { (w, h) } else { yami_px::gfx::phys(w, h, self.scale) };
        let up = (pw, ph) != (w, h);
        let need = (pw * ph * 4) as usize;
        if self.pool.as_ref().map(|p| p.len() < need).unwrap_or(true) {
            self.pool = SlotPool::new(need * 2, &self.shm).ok();
        }
        let Some(pool) = self.pool.as_mut() else { return };
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
        let down = self.press.as_ref().map(|p| p.button == 0x110).unwrap_or(false);
        let blur;
        {
            let mut ui = Ui { cv: Canvas::new(buf, w, h), f: &mut self.fonts, ic: &mut self.icons, t: self.fm.look.theme, zones: vec![], ptr: self.ptr, down, anim: false, blink: self.blink, tr: crate::i18n::tr };
            draw::draw(&mut ui, &self.fm, w, h);
            self.zones = std::mem::take(&mut ui.zones);
            self.anim = ui.anim || scrolling;
            blur = std::mem::take(&mut ui.cv.blur);
        }
        if up {
            yami_px::gfx::upscale(&scratch, w, h, unsafe { std::slice::from_raw_parts_mut(out_ptr, out_len) }, pw, ph);
        }
        self.scratch = scratch;
        let surf = self.window.wl_surface().clone();
        if let Some((_, vp)) = &self.hidpi {
            vp.set_destination(w, h);
        }
        surf.damage_buffer(0, 0, pw, ph);
        if blur != self.blur_set {
            if let Some(mgr) = &self.fx_mgr {
                let fx = self.fx.get_or_insert_with(|| mgr.get_background_effect(&surf, &self.qh, ()));
                if let Ok(r) = Region::new(&self.compositor) {
                    for q in &blur {
                        r.add(q.x, q.y, q.w, q.h);
                    }
                    fx.set_blur_region(if blur.is_empty() { None } else { Some(r.wl_region()) });
                }
            }
            self.blur_set = blur;
        }
        if self.anim {
            surf.frame(&self.qh, surf.clone());
            self.frame_pending = true;
        }
        let _ = buffer.attach_to(&surf);
        surf.commit();
    }

    pub(crate) fn zone_at(&self, x: f64, y: f64) -> Option<ZoneA> {
        self.zones.iter().rev().find(|z| z.r.contains(x, y)).cloned()
    }

    pub(crate) fn set_cursor(&mut self, c: Cur) {
        let shape = match c {
            Cur::Default => Shape::Default,
            Cur::Pointer => Shape::Pointer,
            Cur::Grab => Shape::Grabbing,
            Cur::Text => Shape::Text,
            Cur::Resize => Shape::SeResize,
        };
        if self.cursor_now == Some(shape) {
            return;
        }
        self.cursor_now = Some(shape);
        if let Some(d) = &self.cursor_dev {
            d.set_shape(self.enter_serial, shape);
        }
    }

    pub(crate) fn key(&mut self, e: &KeyEvent) {
        let k = Key { sym: e.keysym, raw: e.raw_code, utf8: e.utf8.clone(), ctrl: self.mods.ctrl, shift: self.mods.shift, alt: self.mods.alt };
        // меню по клавише Menu / shift+F10
        if (e.keysym == Keysym::Menu || e.keysym == Keysym::F10 && self.mods.shift) && self.fm.menu.is_none() {
            let items = crate::menus::for_items(&mut self.fm);
            let (x, y) = self.ptr.map(|p| (p.0 as i32, p.1 as i32)).unwrap_or((self.w / 2, self.h / 2));
            crate::menus::open(&mut self.fm, items, x, y, "");
            self.after_input();
            return;
        }
        self.fm.key(&k);
        self.blink = true;
        self.blink_at = Instant::now();
        self.after_input();
    }

}
