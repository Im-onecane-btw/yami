// Окно файлов: xdg-toplevel, shm-буфер, ввод, буфер обмена и перетаскивание (wl_data_device).
// Рисует draw.rs по состоянию Fm; сюда приходят только события и «эффекты» Fm.

use crate::act::{Act, Ui};
use crate::draw;
use crate::fm::{Cmd, Dlg, Fm, Focus, Fx, Msg, Thumb};
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
    compositor::{CompositorHandler, CompositorState, Region},
    data_device_manager::{
        data_device::{DataDevice, DataDeviceHandler},
        data_offer::{DataOfferHandler, DragOffer},
        data_source::{CopyPasteSource, DataSourceHandler, DragSource},
        DataDeviceManagerState, WritePipe,
    },
    delegate_compositor, delegate_data_device, delegate_keyboard, delegate_output, delegate_pointer, delegate_registry, delegate_seat, delegate_shm, delegate_xdg_shell, delegate_xdg_window,
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
            window::{DecorationMode, Window, WindowConfigure, WindowDecorations, WindowHandler},
            XdgShell,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_data_device::WlDataDevice, wl_data_device_manager::DndAction, wl_data_source::WlDataSource, wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, Dispatch, QueueHandle,
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

struct Press {
    act: Act,
    button: u32,
    x: f64,
    y: f64,
    moved: bool,
    serial: u32,
    /// выделение до начала рамки (ctrl+рамка добавляет)
    base: std::collections::HashSet<PathBuf>,
    ctrl: bool,
    scroll0: f32,
}

pub struct App {
    conn: Connection,
    qh: QueueHandle<App>,
    #[allow(dead_code)]
    handle: LoopHandle<'static, App>,
    tx: Sender<Ev>,
    registry: RegistryState,
    seat: SeatState,
    outputs: OutputState,
    compositor: CompositorState,
    shm: Shm,
    window: Window,
    cursor_mgr: Option<CursorShapeManager>,
    cursor_dev: Option<WpCursorShapeDeviceV1>,
    cursor_now: Option<Shape>,
    fx_mgr: Option<ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1>,
    fx: Option<ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1>,
    /// HiDPI: буфер в физических пикселях (wp_fractional_scale + viewport), холст — логический
    scale: f64,
    hidpi: Option<(frac::WpFractionalScaleV1, wp_viewport::WpViewport)>,
    scratch: Vec<u32>,
    blur_set: Vec<R>,
    ddm: Option<DataDeviceManagerState>,
    ddev: Option<DataDevice>,
    seat_obj: Option<wl_seat::WlSeat>,
    clip_src: Option<CopyPasteSource>,
    clip_data: (Vec<PathBuf>, bool),
    drag_src: Option<DragSource>,
    drag_icon: Option<(wl_surface::WlSurface, SlotPool)>,
    drag_paths: Vec<PathBuf>,
    pointer: Option<wl_pointer::WlPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    mods: Modifiers,
    ptr: Option<(f64, f64)>,
    press: Option<Press>,
    last_click: Option<(Act, Instant)>,
    serial: u32,
    enter_serial: u32,
    kb_serial: u32,
    pool: Option<SlotPool>,
    w: i32,
    h: i32,
    configured: bool,
    dirty: bool,
    frame_pending: bool,
    anim: bool,
    /// прошлый шаг плавной прокрутки
    scroll_t: Instant,
    zones: Vec<Zone<Act>>,
    fonts: Fonts,
    icons: Icons,
    pub fm: Fm,
    quit: bool,
    blink: bool,
    blink_at: Instant,
    title: String,
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
    fn tick(&mut self) {
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
        let typing = self.fm.focus != Focus::Content || self.fm.dlg.as_ref().map(|d| matches!(d.0, Dlg::Name { .. } | Dlg::OpenWith { .. })).unwrap_or(false) || self.fm.v().rename.is_some() || self.fm.pick.as_ref().map(|p| p.name_focus).unwrap_or(false);
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

    fn on_ev(&mut self, e: Ev) {
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

    fn after_input(&mut self) {
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
    fn set_clip(&mut self, paths: Vec<PathBuf>, cut: bool) {
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

    fn paste(&mut self, dir: PathBuf) {
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
    fn flush(&mut self) {
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
    fn step_scroll(&mut self) -> bool {
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

    fn draw(&mut self) {
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

    fn zone_at(&self, x: f64, y: f64) -> Option<ZoneA> {
        self.zones.iter().rev().find(|z| z.r.contains(x, y)).cloned()
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

    fn key(&mut self, e: &KeyEvent) {
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

    // ── мышь ───────────────────────────────────────────────────────────────
    fn pane_at(&self, x: f64, y: f64) -> Option<usize> {
        let l = draw::lay(&self.fm, self.w, self.h);
        l.panes.iter().position(|r| r.contains(x, y))
    }

    fn on_pointer(&mut self, e: &PointerEvent) {
        let (x, y) = e.position;
        match e.kind {
            PointerEventKind::Enter { serial } => {
                self.enter_serial = serial;
                self.cursor_now = None;
                self.ptr = Some((x, y));
                self.dirty = true;
            }
            PointerEventKind::Leave { .. } => {
                self.ptr = None;
                self.dirty = true;
            }
            PointerEventKind::Motion { .. } => {
                let before = self.ptr.and_then(|p| self.zone_at(p.0, p.1));
                self.ptr = Some((x, y));
                if self.press.is_some() {
                    self.on_drag(x, y);
                    return;
                }
                let after = self.zone_at(x, y);
                self.set_cursor(after.as_ref().map(|z| z.cur).unwrap_or(Cur::Default));
                if before.as_ref().map(|z| (&z.act, z.r)) != after.as_ref().map(|z| (&z.act, z.r)) {
                    self.dirty = true;
                    if let Some(z) = after {
                        self.on_hover(&z.act);
                    }
                }
            }
            PointerEventKind::Press { button, serial, .. } => {
                self.serial = serial;
                self.ptr = Some((x, y));
                self.dirty = true;
                let z = self.zone_at(x, y);
                let Some(z) = z else { return };
                match button {
                    0x113 => {
                        self.fm.back();
                        self.after_input();
                        return;
                    }
                    0x114 => {
                        self.fm.forward();
                        self.after_input();
                        return;
                    }
                    _ => {}
                }
                let ctrl = self.mods.ctrl;
                let base = self.fm.v().sel.clone();
                let scroll0 = match &z.act {
                    Act::Bar(p) => self.fm.pane(*p).map(|v| v.scroll).unwrap_or(0.0),
                    _ => 0.0,
                };
                self.press = Some(Press { act: z.act.clone(), button, x, y, moved: false, serial, base, ctrl, scroll0 });
                self.on_press(&z.act, button, x, y);
                self.after_input();
            }
            PointerEventKind::Release { button, .. } => {
                let p = self.press.take();
                self.dirty = true;
                let Some(p) = p else { return };
                if p.button != button {
                    return;
                }
                if let Act::Bg(pi) = p.act {
                    if let Some(v) = self.fm.pane_m(pi) {
                        v.band = None;
                    }
                    if p.moved {
                        self.after_input();
                        return;
                    }
                }
                if p.moved && matches!(p.act, Act::Bar(_) | Act::SZoom | Act::Item(..) | Act::Drag) {
                    self.after_input();
                    return;
                }
                let z = self.zone_at(x, y);
                if z.as_ref().map(|z| z.act == p.act).unwrap_or(false) {
                    let double = matches!(&self.last_click, Some((a, t)) if *a == p.act && t.elapsed() < Duration::from_millis(400));
                    self.last_click = Some((p.act.clone(), Instant::now()));
                    self.on_click(p.act, button, x, y, double);
                }
                self.after_input();
            }
            PointerEventKind::Axis { vertical, .. } => {
                let d = if vertical.discrete != 0 { vertical.discrete as f64 } else if vertical.value120 != 0 { vertical.value120 as f64 / 120.0 } else { vertical.absolute / 15.0 };
                if d.abs() < 0.01 {
                    return;
                }
                self.on_wheel(x, y, d);
                self.after_input();
            }
        }
    }

    fn on_hover(&mut self, a: &Act) {
        let Some(m) = self.fm.menu.as_mut() else { return };
        match a {
            Act::MItem(i) => {
                m.hover = Some(*i);
                let has_sub = m.items.get(*i).map(|it| !it.sub.is_empty()).unwrap_or(false);
                if has_sub {
                    if m.sub.map(|s| s.0) != Some(*i) {
                        m.sub = Some((*i, Instant::now()));
                        m.sub_hover = None;
                    }
                } else {
                    m.sub = None;
                }
            }
            Act::MSub(j) => m.sub_hover = Some(*j),
            _ => {}
        }
    }

    fn on_press(&mut self, a: &Act, button: u32, x: f64, y: f64) {
        let fm = &mut self.fm;
        // меню открыто: щелчок мимо закрывает
        if fm.menu.is_some() && !matches!(a, Act::MItem(_) | Act::MSub(_) | Act::Block) {
            fm.menu = None;
            if matches!(a, Act::MenuBg) {
                return;
            }
        }
        if button == 0x111 {
            // ПКМ
            let (xi, yi) = (x as i32, y as i32);
            match a {
                Act::Item(p, i) => {
                    fm.set_active(*p);
                    let path = fm.v().item(*i).map(|e| e.path.clone());
                    if let Some(path) = path {
                        if !fm.v().sel.contains(&path) {
                            fm.select_only(*i);
                        }
                        fm.vm().cur = Some(*i);
                    }
                    let items = crate::menus::for_items(fm);
                    crate::menus::open(fm, items, xi, yi, "");
                }
                Act::Bg(p) => {
                    fm.set_active(*p);
                    fm.vm().sel.clear();
                    let items = crate::menus::for_bg(fm);
                    crate::menus::open(fm, items, xi, yi, "");
                }
                Act::Place(path, user) => {
                    let items = crate::menus::for_place(fm, path, *user);
                    // «в новой вкладке» берёт цель из выделения: подставим путь
                    let items = items.into_iter().map(|mut it| {
                        if it.cmd == Cmd::OpenTab {
                            it.cmd = Cmd::Go(path.clone());
                            it.label = "открыть".into();
                        }
                        it
                    }).collect::<Vec<_>>();
                    let mut items = items;
                    items.dedup_by(|a, b| a.label == b.label);
                    crate::menus::open(fm, items, xi, yi, &path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
                }
                Act::PlaceTrash => {
                    let items = crate::menus::for_trash_place(fm);
                    crate::menus::open(fm, items, xi, yi, "");
                }
                Act::Dev(i) => {
                    let items = crate::menus::for_dev(fm, *i);
                    crate::menus::open(fm, items, xi, yi, "");
                }
                _ => {}
            }
            return;
        }
        if button == 0x112 {
            if let Act::Item(p, i) = a {
                fm.set_active(*p);
                if let Some(e) = fm.v().item(*i).cloned() {
                    if e.dir {
                        let keep = fm.tab;
                        fm.open_tab(e.path);
                        fm.tab = keep;
                    }
                }
            }
            if let Act::Place(path, _) = a {
                let keep = fm.tab;
                fm.open_tab(path.clone());
                fm.tab = keep;
            }
            return;
        }
        if button != 0x110 {
            return;
        }
        // поле ввода теряет фокус, если щёлкнули мимо
        if !matches!(a, Act::PathField | Act::PathBar) && fm.focus == Focus::Path {
            fm.focus = Focus::Content;
        }
        if !matches!(a, Act::FilterField | Act::FilterClear) && fm.focus == Focus::Filter && fm.filter_f.text.is_empty() {
            fm.focus = Focus::Content;
        }
        if let Some(p) = &mut fm.pick {
            p.name_focus = matches!(a, Act::PickName);
        }
        if fm.v().rename.is_some() && !matches!(a, Act::Block) {
            fm.finish_rename();
        }
        match a {
            Act::Item(p, i) => {
                fm.set_active(*p);
                fm.focus = Focus::Content;
                let path = fm.v().item(*i).map(|e| e.path.clone());
                let (ctrl, shift) = (self.mods.ctrl, self.mods.shift);
                // уже выделенное без модификаторов — выделение сохраняем до отпускания (для перетаскивания)
                if path.as_ref().map(|p| fm.v().sel.contains(p)).unwrap_or(false) && !ctrl && !shift {
                    fm.vm().cur = Some(*i);
                } else {
                    fm.click_item(*i, ctrl, shift);
                }
            }
            Act::Bg(p) => {
                fm.set_active(*p);
                fm.focus = Focus::Content;
                if !self.mods.ctrl {
                    fm.vm().sel.clear();
                }
            }
            Act::Drag => {
                // двойной щелчок по пустой панели — развернуть; иначе тащим окно
            }
            Act::SZoom => self.slide_zoom(x),
            _ => {}
        }
    }

    fn slide_zoom(&mut self, x: f64) {
        let w = self.w;
        let r = R::new((w - 560) / 2 + 18, 0, (560 - 52) / 2, 10);
        let k = ((x - r.x as f64) / r.w as f64).clamp(0.0, 1.0);
        self.fm.cfg.zoom = (k * 4.0).round() as i32;
    }

    fn on_drag(&mut self, x: f64, y: f64) {
        let Some(p) = self.press.as_mut() else { return };
        if (x - p.x).abs() + (y - p.y).abs() > 5.0 {
            p.moved = true;
        }
        if !p.moved {
            return;
        }
        let act = p.act.clone();
        let (px, py, ctrl, scroll0) = (p.x, p.y, p.ctrl, p.scroll0);
        let serial = p.serial;
        match act {
            Act::Bg(pi) if p.button == 0x110 => {
                let base = p.base.clone();
                let Some(v) = self.fm.pane_m(pi) else { return };
                let s = v.scroll as f64;
                let start = match v.band {
                    Some((x0, y0, _, _)) => (x0, y0),
                    None => (px, py + s),
                };
                v.band = Some((start.0, start.1, x, y + s));
                self.fm.set_active(pi);
                let tab = self.fm.tab;
                let saved = self.fm.tabs[tab].active;
                self.fm.tabs[tab].active = pi;
                self.fm.band_select(ctrl, &base);
                self.fm.tabs[tab].active = saved;
                // у края — прокручиваем
                let g = self.fm.pane(pi).map(|v| v.geo.get()).unwrap_or_default();
                let ms = self.fm.pane(pi).map(|v| draw::max_scroll(&self.fm, v)).unwrap_or(0.0);
                if let Some(v) = self.fm.pane_m(pi) {
                    if y < (g.area.y + 10) as f64 {
                        v.target = (v.target - 18.0).max(0.0);
                    } else if y > (g.area.bottom() - 10) as f64 {
                        v.target = (v.target + 18.0).min(ms);
                    }
                }
                self.dirty = true;
            }
            Act::Bar(pi) => {
                let Some(v) = self.fm.pane(pi) else { return };
                let g = v.geo.get();
                let ch = draw::content_h(&self.fm, v, &g) as f32;
                let ms = draw::max_scroll(&self.fm, v);
                let k = ch / g.area.h.max(1) as f32;
                let v = self.fm.pane_m(pi).unwrap();
                v.target = (scroll0 + (y - py) as f32 * k).clamp(0.0, ms);
                v.scroll = v.target;
                self.dirty = true;
            }
            Act::SZoom => {
                self.slide_zoom(x);
                self.dirty = true;
            }
            Act::Drag => {
                self.press = None;
                if let Some(s) = &self.seat_obj {
                    self.window.move_(s, serial);
                }
            }
            Act::Item(pi, i) if self.drag_src.is_none() => {
                let paths: Vec<PathBuf> = {
                    let v = self.fm.pane(pi);
                    let mut s: Vec<PathBuf> = v.map(|v| v.selected().iter().map(|e| e.path.clone()).collect()).unwrap_or_default();
                    if s.is_empty() {
                        if let Some(e) = v.and_then(|v| v.item(i)) {
                            s.push(e.path.clone());
                        }
                    }
                    s
                };
                if !paths.is_empty() && !self.fm.pane(pi).map(|v| v.trash).unwrap_or(false) {
                    self.start_drag(paths, serial);
                }
            }
            _ => {}
        }
    }

    fn start_drag(&mut self, paths: Vec<PathBuf>, serial: u32) {
        let (Some(ddm), Some(dev)) = (&self.ddm, &self.ddev) else { return };
        let src = ddm.create_drag_and_drop_source(&self.qh, ["text/uri-list", "x-special/gnome-copied-files", "text/plain;charset=utf-8"], DndAction::Copy | DndAction::Move | DndAction::Ask);
        // значок перетаскивания: пиксельный файл и число
        let icon = self.compositor.create_surface(&self.qh);
        let (iw, ih) = (56, 56);
        if let Ok(mut pool) = SlotPool::new((iw * ih * 4) as usize, &self.shm) {
            if let Ok((buffer, bytes)) = pool.create_buffer(iw, ih, iw * 4, wl_shm::Format::Argb8888) {
                let buf: &mut [u32] = unsafe { std::slice::from_raw_parts_mut(bytes.as_mut_ptr() as *mut u32, bytes.len() / 4) };
                let mut cv = Canvas::new(buf, iw, ih);
                cv.clear(crate::theme::CLEAR);
                let t = self.fm.look.theme;
                let first = crate::fsx::Entry::from_path(&paths[0]);
                let kind = first.as_ref().map(|e| crate::mime::kind(e.mime)).unwrap_or(crate::mime::Kind::Other);
                if paths.len() > 1 {
                    crate::picons::draw(&mut cv, &t, kind, crate::picons::Special::None, 10, 2, 2, crate::picons::Look { dim: true, link: false, selected: false });
                }
                crate::picons::draw(&mut cv, &t, kind, crate::picons::Special::None, 4, 8, 2, crate::picons::Look { dim: false, link: false, selected: true });
                if paths.len() > 1 {
                    let n = paths.len().to_string();
                    let nw = self.fonts.width(crate::font::PX, &n) + 8;
                    let br = R::new(iw - nw - 2, ih - 20, nw, 18);
                    cv.pxbox(br, &yami_px::gfx::BoxStyle::new(t.red, crate::theme::CLEAR));
                    cv.text_center(&mut self.fonts, crate::font::PX, br, &n, t.on_accent);
                }
                let _ = buffer.attach_to(&icon);
                icon.damage_buffer(0, 0, iw, ih);
            }
            src.start_drag(dev, self.window.wl_surface(), Some(&icon), serial);
            icon.commit();
            self.drag_icon = Some((icon, pool));
        } else {
            src.start_drag(dev, self.window.wl_surface(), None, serial);
        }
        self.drag_src = Some(src);
        self.drag_paths = paths;
        self.press = None;
    }

    fn on_wheel(&mut self, x: f64, y: f64, d: f64) {
        if self.fm.menu.is_some() {
            return;
        }
        if self.mods.ctrl {
            self.fm.run(Cmd::Zoom(if d < 0.0 { 1 } else { -1 }));
            return;
        }
        if let Some((Dlg::OpenWith { scroll, .. }, _)) = &mut self.fm.dlg {
            *scroll = (*scroll + (d * 60.0) as i32).max(0);
            return;
        }
        if self.fm.dlg.is_some() {
            return;
        }
        let Some(pi) = self.pane_at(x, y) else { return };
        let Some(v) = self.fm.pane(pi) else { return };
        let g = v.geo.get();
        let ms = draw::max_scroll(&self.fm, v);
        let step = if self.fm.mode() == crate::fm::Mode::Grid { g.ch as f32 * 0.75 } else { g.ch as f32 * 3.0 };
        let v = self.fm.pane_m(pi).unwrap();
        v.target = (v.target + d as f32 * step).clamp(0.0, ms);
    }

    fn on_click(&mut self, a: Act, button: u32, x: f64, y: f64, double: bool) {
        if button != 0x110 {
            return;
        }
        let fm = &mut self.fm;
        match a {
            Act::Back => fm.back(),
            Act::Fwd => fm.forward(),
            Act::Up => fm.up(),
            Act::PathBar => fm.edit_path(),
            Act::Crumb(p) => fm.go(p),
            Act::Place(p, _) => {
                fm.go(p);
            }
            Act::PlaceTrash => fm.go_trash(),
            Act::Dev(i) => match fm.devs.get(i).and_then(|d| d.mount.clone()) {
                Some(m) => fm.go(m),
                None => fm.run(Cmd::Mount(i)),
            },
            Act::Eject(i) => fm.run(Cmd::Unmount(i)),
            Act::Item(p, i) => {
                fm.set_active(p);
                let single = fm.cfg.single_click && !self.mods.ctrl && !self.mods.shift;
                if double || single {
                    fm.activate(i);
                } else if !self.mods.ctrl && !self.mods.shift {
                    fm.select_only(i);
                }
            }
            Act::Bg(_) => {}
            Act::ColSort(s) => fm.run(Cmd::Sort(s)),
            Act::Tab(i) => {
                fm.tab = i;
                fm.watch_dirty = true;
            }
            Act::TabClose(i) => fm.close_tab(i),
            Act::TabNew => fm.run(Cmd::NewTab),
            Act::ModeBtn(m) => fm.run(Cmd::Mode(m)),
            Act::SplitBtn => fm.run(Cmd::Split),
            Act::InfoBtn => fm.run(Cmd::Info),
            Act::MenuBtn => {
                if fm.menu.take().is_none() {
                    let items = crate::menus::main_menu(fm);
                    crate::menus::open(fm, items, self.w - 10 - draw::MENU_W, draw::TOOL_H - 4, "≡");
                }
            }
            Act::SearchBtn | Act::FilterField => fm.focus = Focus::Filter,
            Act::FilterClear => {
                fm.filter_f.clear();
                fm.apply_filter();
            }
            Act::MItem(i) => fm.menu_pick(i, None),
            Act::MSub(j) => {
                if let Some(si) = fm.menu.as_ref().and_then(|m| m.sub.map(|s| s.0)) {
                    fm.menu_pick(si, Some(j));
                }
            }
            Act::MenuBg => fm.menu = None,
            Act::Dlg(k) => match k {
                "ok" => fm.dlg_ok(),
                "cancel" => fm.dlg_cancel(),
                "skip" => fm.conflict_answer(crate::ops::Resolve::Skip),
                "rename" => fm.conflict_answer(crate::ops::Resolve::Rename),
                "replace" => fm.conflict_answer(crate::ops::Resolve::Replace),
                "all" => {
                    if let Some((Dlg::Conflict { all, .. }, _)) = &mut fm.dlg {
                        *all = !*all;
                    }
                }
                "remember" => {
                    if let Some((Dlg::OpenWith { remember, .. }, _)) = &mut fm.dlg {
                        *remember = !*remember;
                    }
                }
                _ => {}
            },
            Act::DlgApp(n) => {
                if let Some((Dlg::OpenWith { sel, .. }, _)) = &mut fm.dlg {
                    *sel = n;
                }
                if double {
                    fm.dlg_ok();
                }
            }
            Act::DlgBit(b) => fm.chmod(b),
            Act::SBool(k) => {
                let c = &mut fm.cfg;
                match k {
                    "hidden" => c.hidden = !c.hidden,
                    "foldersFirst" => c.folders_first = !c.folders_first,
                    "thumbs" => c.thumbs = !c.thumbs,
                    "sidebar" => c.sidebar = !c.sidebar,
                    "info" => c.info = !c.info,
                    "singleClick" => c.single_click = !c.single_click,
                    "confirmTrash" => c.confirm_trash = !c.confirm_trash,
                    "split" => c.split = !c.split,
                    "desc" => c.desc = !c.desc,
                    _ => {}
                }
                fm.refilter_all();
            }
            Act::SCycle(k) => {
                match k {
                    "view" => {
                        let m = if fm.mode() == crate::fm::Mode::Grid { crate::fm::Mode::List } else { crate::fm::Mode::Grid };
                        fm.run(Cmd::Mode(m));
                    }
                    "sort" => {
                        let next = match fm.cfg.sort.as_str() {
                            "name" => "size",
                            "size" => "time",
                            "time" => "type",
                            _ => "name",
                        };
                        fm.cfg.sort = next.into();
                        fm.refilter_all();
                    }
                    _ => {}
                }
            }
            Act::PickOk => fm.pick_accept(),
            Act::PickCancel => fm.pick_cancel(),
            Act::PickName => {
                if let Some(p) = &mut fm.pick {
                    p.name_focus = true;
                }
            }
            Act::PickFilter => {
                if let Some(p) = &fm.pick {
                    let items: Vec<crate::fm::MItem> = p.filters.iter().enumerate().map(|(i, f)| crate::fm::MItem::new(&f.0, "", "", Cmd::PickFilter(i)).check(i == p.filter)).collect();
                    crate::menus::open(fm, items, x as i32, y as i32 - 10, "");
                }
            }
            Act::JobCancel(id) => {
                if let Some(j) = fm.jobs.iter().find(|j| j.id == id) {
                    j.stop.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
            Act::Toast => fm.toast = None,
            Act::Drag => {
                if double {
                    self.window.set_maximized();
                }
            }
            Act::Dim => {
                if matches!(fm.dlg, Some((Dlg::Settings | Dlg::Props { .. } | Dlg::Errors(_), _))) {
                    fm.dlg_cancel();
                }
            }
            Act::Block | Act::PathField | Act::DlgField | Act::SZoom | Act::Bar(_) => {}
        }
    }

    /// что под точкой (x, y) годится целью для броска: папка-элемент, место, сама панель
    fn drop_target(&self, x: f64, y: f64) -> Option<(PathBuf, Option<(usize, Option<usize>)>)> {
        let z = self.zone_at(x, y)?;
        match z.act {
            Act::Item(p, i) => {
                let v = self.fm.pane(p)?;
                let e = v.item(i)?;
                if e.dir {
                    Some((e.path.clone(), Some((p, Some(i)))))
                } else if v.trash {
                    None
                } else {
                    Some((v.dir.clone(), Some((p, None))))
                }
            }
            Act::Bg(p) => {
                let v = self.fm.pane(p)?;
                if v.search.is_some() {
                    return None;
                }
                Some((if v.trash { crate::trash::home_trash().join("files") } else { v.dir.clone() }, Some((p, None))))
            }
            Act::Place(path, _) => Some((path, Some((usize::MAX, None)))),
            Act::PlaceTrash => Some((crate::trash::home_trash().join("files"), Some((usize::MAX, None)))),
            Act::Crumb(path) => Some((path, None)),
            _ => None,
        }
    }
}

// ── обработчики Wayland ────────────────────────────────────────────────────
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
        if self.fm.pick.is_some() {
            self.fm.pick_cancel();
        }
        self.quit = true;
    }
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window, c: WindowConfigure, _: u32) {
        if let (Some(w), Some(h)) = c.new_size {
            self.w = w.get() as i32;
            self.h = h.get() as i32;
        }
        let _ = c.decoration_mode == DecorationMode::Server;
        self.fm.w = self.w;
        self.fm.h = self.h;
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
        if self.ddev.is_none() {
            if let Some(m) = &self.ddm {
                self.ddev = Some(m.get_data_device(qh, &seat));
            }
        }
        self.seat_obj = Some(seat.clone());
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
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, serial: u32, _: &[u32], _: &[Keysym]) {
        self.kb_serial = serial;
    }
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32) {}
    fn press_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, serial: u32, e: KeyEvent) {
        self.kb_serial = serial;
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

// ── буфер обмена и перетаскивание ──────────────────────────────────────────
impl DataDeviceHandler for App {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, dd: &WlDataDevice, x: f64, y: f64, _: &wl_surface::WlSurface) {
        let Some(dev) = &self.ddev else { return };
        if dev.inner() != dd {
            return;
        }
        let Some(offer) = dev.data().drag_offer() else { return };
        let ok = offer.with_mime_types(|m| m.iter().any(|t| t == "text/uri-list"));
        if ok {
            offer.accept_mime_type(offer.serial, Some("text/uri-list".into()));
            offer.set_actions(DndAction::Copy | DndAction::Move, DndAction::Copy);
        } else {
            offer.accept_mime_type(offer.serial, None);
        }
        self.dnd_motion(x, y);
    }
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {
        self.fm.drop_hint = None;
        self.dirty = true;
    }
    fn motion(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice, x: f64, y: f64) {
        self.dnd_motion(x, y);
    }
    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}
    fn drop_performed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {
        let Some(dev) = &self.ddev else { return };
        let Some(offer) = dev.data().drag_offer() else { return };
        let (x, y) = (offer.x, offer.y);
        self.fm.drop_hint = None;
        let Some((dst, _)) = self.drop_target(x, y) else {
            offer.finish();
            offer.destroy();
            return;
        };
        let ours = !self.drag_paths.is_empty();
        if ours {
            let paths = std::mem::take(&mut self.drag_paths);
            offer.finish();
            offer.destroy();
            let _ = self.tx.send(Ev::Dropped { x: x as i32, y: y as i32, dst, paths, ours: true });
            return;
        }
        let Ok(mut pipe) = offer.receive("text/uri-list".into()) else { return };
        let tx = self.tx.clone();
        let _ = self.conn.flush();
        let off = offer.clone();
        std::thread::spawn(move || {
            let mut s = String::new();
            let _ = pipe.read_to_string(&mut s);
            let paths: Vec<PathBuf> = s.lines().filter_map(crate::icons::uri_path).filter(|p| std::fs::symlink_metadata(p).is_ok()).collect();
            let _ = tx.send(Ev::Dropped { x: x as i32, y: y as i32, dst, paths, ours: false });
        });
        // закончить — после чтения; источнику важно знать, что бросок принят
        self.pending_finish(off);
    }
}

impl App {
    fn dnd_motion(&mut self, x: f64, y: f64) {
        let hint = self.drop_target(x, y).and_then(|t| t.1);
        let hint = hint.map(|(p, i)| (if p == usize::MAX { usize::MAX } else { p }, i));
        if hint != self.fm.drop_hint {
            self.fm.drop_hint = hint.or(Some((usize::MAX - 1, None)));
            self.dirty = true;
        }
        self.ptr = Some((x, y));
        // у краёв панели — прокрутка
        if let Some(pi) = self.pane_at(x, y) {
            if let Some(v) = self.fm.pane(pi) {
                let g = v.geo.get();
                let ms = draw::max_scroll(&self.fm, v);
                let v = self.fm.pane_m(pi).unwrap();
                if y < (g.area.y + 24) as f64 {
                    v.target = (v.target - 14.0).max(0.0);
                } else if y > (g.area.bottom() - 24) as f64 {
                    v.target = (v.target + 14.0).min(ms);
                }
            }
        }
        self.dirty = true;
    }

    fn pending_finish(&mut self, off: DragOffer) {
        // чтение идёт в нити; finish можно отправить сразу после receive — данные уже в трубе
        off.finish();
        off.destroy();
    }
}

impl DataOfferHandler for App {
    fn source_actions(&mut self, _: &Connection, _: &QueueHandle<Self>, offer: &mut DragOffer, _: DndAction) {
        offer.set_actions(DndAction::Copy | DndAction::Move, DndAction::Copy);
    }
    fn selected_action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &mut DragOffer, _: DndAction) {}
}

impl DataSourceHandler for App {
    fn accept_mime(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: Option<String>) {}
    fn send_request(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource, mime: String, mut fd: WritePipe) {
        let is_drag = self.drag_src.as_ref().map(|d| d.inner() == source).unwrap_or(false);
        let (paths, cut) = if is_drag { (self.drag_paths.clone(), false) } else { self.clip_data.clone() };
        let uris: Vec<String> = paths.iter().map(|p| crate::icons::file_uri(p)).collect();
        let body = match mime.as_str() {
            "x-special/gnome-copied-files" => format!("{}\n{}", if cut { "cut" } else { "copy" }, uris.join("\n")),
            "text/uri-list" => uris.join("\r\n") + "\r\n",
            "application/x-kde-cutselection" => if cut { "1".into() } else { "0".into() },
            _ => paths.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>().join("\n"),
        };
        let _ = fd.write_all(body.as_bytes());
    }
    fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource) {
        if self.drag_src.as_ref().map(|d| d.inner() == source).unwrap_or(false) {
            self.drag_src = None;
            self.drag_icon = None;
            self.drag_paths.clear();
        } else if self.clip_src.as_ref().map(|c| c.inner() == source).unwrap_or(false) {
            self.clip_src = None;
            self.clip_data = (vec![], false);
            self.fm.clip = None;
            self.dirty = true;
        }
    }
    fn dnd_dropped(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}
    fn dnd_finished(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {
        self.drag_src = None;
        self.drag_icon = None;
        // перенесли в другую программу — папку перечитает inotify
    }
    fn action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: DndAction) {}
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

impl Dispatch<ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1, ()> for App {
    fn event(_: &mut Self, _: &ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1, _: ext_background_effect_manager_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1, ()> for App {
    fn event(_: &mut Self, _: &ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1, _: ext_background_effect_surface_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

delegate_compositor!(App);
delegate_output!(App);
delegate_shm!(App);
delegate_seat!(App);
delegate_keyboard!(App);
delegate_pointer!(App);
delegate_xdg_shell!(App);
delegate_xdg_window!(App);
delegate_data_device!(App);
delegate_registry!(App);
