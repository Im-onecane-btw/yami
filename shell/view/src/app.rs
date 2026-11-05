// Окно просмотрщика: xdg-toplevel, shm, мышь и клавиатура; декодирование — в нити.

use crate::view::{Act, Fx, Ui, V};
use crate::font::Fonts;
use crate::gfx::Canvas;
use crate::icons::Icons;
use calloop::channel::{self, Sender};
use calloop::timer::{TimeoutAction, Timer};
use calloop::{EventLoop, LoopHandle};
use calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, Region},
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
use std::path::PathBuf;
use std::time::{Duration, Instant};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols::ext::background_effect::v1::client::{ext_background_effect_manager_v1, ext_background_effect_surface_v1};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::{Shape, WpCursorShapeDeviceV1};
use wayland_protocols::wp::fractional_scale::v1::client::{wp_fractional_scale_manager_v1 as frac_mgr, wp_fractional_scale_v1 as frac};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};

pub enum Ev {
    Loaded(PathBuf, Result<crate::pic::Pic, String>),
    Saved(Result<PathBuf, String>),
}

enum Drag {
    Pan(f64, f64, f32, f32),
    Crop,
}

pub struct App {
    conn: Connection,
    qh: QueueHandle<App>,
    handle: LoopHandle<'static, App>,
    tx: Sender<Ev>,
    load_tx: std::sync::mpsc::Sender<PathBuf>,
    registry: RegistryState,
    seat: SeatState,
    outputs: OutputState,
    compositor: CompositorState,
    shm: Shm,
    window: Window,
    cursor_mgr: Option<CursorShapeManager>,
    cursor_dev: Option<WpCursorShapeDeviceV1>,
    cursor_now: Option<Shape>,
    enter_serial: u32,
    fx_mgr: Option<ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1>,
    fx: Option<ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1>,
    /// HiDPI: буфер в физических пикселях (wp_fractional_scale + viewport), холст — логический
    scale: f64,
    hidpi: Option<(frac::WpFractionalScaleV1, wp_viewport::WpViewport)>,
    scratch: Vec<u32>,
    blur_on: bool,
    pointer: Option<wl_pointer::WlPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    mods: Modifiers,
    drag: Option<Drag>,
    press_act: Option<Act>,
    last_click: Option<Instant>,
    pool: Option<SlotPool>,
    configured: bool,
    dirty: bool,
    frame_pending: bool,
    anim: bool,
    zones: Vec<yami_px::ui::Zone<Act>>,
    fonts: Fonts,
    icons: Icons,
    v: V,
    quit: bool,
    title: String,
    ui_was: bool,
}

pub fn run(mut v: V) {
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
    // нить декодирования: последний запрос — первым
    let (load_tx, load_rx) = std::sync::mpsc::channel::<PathBuf>();
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            while let Ok(first) = load_rx.recv() {
                let mut q = vec![first];
                while let Ok(p) = load_rx.try_recv() {
                    q.push(p);
                }
                // сначала текущая (её просят первой), потом соседи
                for p in q {
                    let r = crate::pic::load(&p);
                    let _ = tx.send(Ev::Loaded(p, r));
                    yami_px::trim_heap();
                }
            }
        });
    }
    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor");
    let xdg = XdgShell::bind(&globals, &qh).expect("xdg_shell");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm");
    let surface = compositor.create_surface(&qh);
    let window = xdg.create_window(surface, WindowDecorations::RequestServer, &qh);
    window.set_app_id("yami-view");
    window.set_title(v.title());
    window.set_min_size(Some((360, 260)));
    window.commit();
    v.want();
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
        tx,
        load_tx,
        registry: RegistryState::new(&globals),
        seat: SeatState::new(&globals, &qh),
        outputs: OutputState::new(&globals, &qh),
        compositor,
        shm,
        window,
        cursor_mgr: CursorShapeManager::bind(&globals, &qh).ok(),
        cursor_dev: None,
        cursor_now: None,
        enter_serial: 0,
        fx_mgr: globals.bind(&qh, 1..=1, ()).ok(),
        fx: None,
        scale: 1.0,
        hidpi,
        scratch: vec![],
        blur_on: false,
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
        anim: false,
        zones: vec![],
        fonts: Fonts::with(v.look.font("view")),
        icons: Icons::new(""),
        v,
        quit: false,
        title: String::new(),
        ui_was: true,
    };
    app.effects();
    handle
        .insert_source(Timer::from_duration(Duration::from_millis(50)), |_, _, app: &mut App| {
            if app.v.tick() {
                app.dirty = true;
            }
            if app.v.look.stale() {
                let font = app.v.look.font("view").to_string();
                app.v.look = crate::look::Look::load();
                if app.v.look.font("view") != font {
                    app.fonts = Fonts::with(app.v.look.font("view"));
                }
                crate::i18n::set(&app.v.look.lang);
                app.dirty = true;
            }
            // панели прячутся сами
            if !app.v.ui_visible() && app.ui_was {
                app.dirty = true;
            }
            app.ui_was = app.v.ui_visible();
            app.flush();
            TimeoutAction::ToDuration(Duration::from_millis(if app.v.animating() { 20 } else { 250 }))
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
}

impl App {
    fn on_ev(&mut self, e: Ev) {
        match e {
            Ev::Loaded(p, r) => self.v.on_loaded(p, r),
            Ev::Saved(r) => match r {
                Ok(p) => {
                    // сохранили копию: показать её
                    self.v.list = crate::pic::siblings(&p);
                    self.v.idx = self.v.list.iter().position(|x| x == &p).unwrap_or(0);
                    self.v.reset_view();
                    self.v.want();
                    self.v.toast(crate::i18n::tr("сохранено"));
                }
                Err(e) => self.v.toast(e),
            },
        }
        self.effects();
        self.dirty = true;
    }

    fn effects(&mut self) {
        for f in std::mem::take(&mut self.v.fx) {
            match f {
                Fx::Close => self.quit = true,
                Fx::Fullscreen(on) => {
                    if on {
                        self.window.set_fullscreen(None);
                    } else {
                        self.window.unset_fullscreen();
                    }
                }
                Fx::Load(p) => {
                    let _ = self.load_tx.send(p);
                }
                Fx::Copy(bytes) => {
                    std::thread::spawn(move || {
                        use std::io::Write;
                        if let Ok(mut ch) = std::process::Command::new("wl-copy").args(["-t", "image/png"]).stdin(std::process::Stdio::piped()).spawn() {
                            if let Some(i) = ch.stdin.as_mut() {
                                let _ = i.write_all(&bytes);
                            }
                            drop(ch.stdin.take());
                            let _ = ch.wait();
                        }
                    });
                }
                Fx::SaveAs(dir, name) => {
                    let Some(pic) = self.v.pic().cloned() else { continue };
                    let tx = self.tx.clone();
                    // копия кадров — нить сохранения не держит Rc
                    let frames = vec![pic.frames[self.v.frame.min(pic.frames.len() - 1)].clone()];
                    let one = crate::pic::Pic { path: pic.path.clone(), frames, bytes: 0, format: pic.format.clone(), edited: true };
                    std::thread::spawn(move || {
                        let o = std::process::Command::new("yami-files").args(["--pick", "save", "--name", &name, "--folder"]).arg(&dir).args(["--filter", "PNG|*.png", "--filter", "JPEG|*.jpg;*.jpeg", "--filter", "WebP|*.webp"]).output();
                        let Ok(o) = o else {
                            let _ = tx.send(Ev::Saved(Err("yami-files?".into())));
                            return;
                        };
                        let Some(p) = String::from_utf8_lossy(&o.stdout).lines().next().map(PathBuf::from) else { return };
                        let r = crate::pic::save(&one, &p).map(|_| p);
                        let _ = tx.send(Ev::Saved(r));
                    });
                }
            }
        }
        let t = self.v.title();
        if t != self.title {
            self.window.set_title(t.clone());
            self.title = t;
        }
    }

    fn flush(&mut self) {
        if self.dirty && self.configured && !self.frame_pending {
            self.draw();
        }
        let _ = self.conn.flush();
    }

    fn draw(&mut self) {
        self.dirty = false;
        let (w, h) = (self.v.w, self.v.h);
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
        {
            let mut ui = Ui { cv: Canvas::new(buf, w, h), f: &mut self.fonts, ic: &mut self.icons, t: self.v.look.theme, zones: vec![], ptr: self.v.ptr, down: self.press_act.is_some(), anim: false, blink: true, tr: crate::i18n::tr };
            crate::view::draw(&mut ui, &self.v, w, h);
            self.zones = std::mem::take(&mut ui.zones);
            self.anim = ui.anim;
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
        // стекло под окном, пока фон темы полупрозрачный
        let want_blur = self.v.look.blur && self.v.bg == 0;
        if want_blur != self.blur_on {
            if let Some(mgr) = &self.fx_mgr {
                let fx = self.fx.get_or_insert_with(|| mgr.get_background_effect(&surf, &self.qh, ()));
                if want_blur {
                    if let Ok(r) = Region::new(&self.compositor) {
                        r.add(0, 0, i32::MAX / 2, i32::MAX / 2);
                        fx.set_blur_region(Some(r.wl_region()));
                    }
                } else {
                    fx.set_blur_region(None);
                }
            }
            self.blur_on = want_blur;
        }
        if self.anim {
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
        if self.cursor_now == Some(c) {
            return;
        }
        self.cursor_now = Some(c);
        if let Some(d) = &self.cursor_dev {
            d.set_shape(self.enter_serial, c);
        }
    }

    fn key(&mut self, e: &KeyEvent) {
        let (c, s) = (self.mods.ctrl, self.mods.shift);
        let v = &mut self.v;
        v.active_at = Instant::now();
        let (cx, cy) = (v.w as f64 / 2.0, v.h as f64 / 2.0);
        match e.keysym {
            Keysym::Escape => {
                if v.cropping {
                    v.btn("crop_no");
                } else if v.full {
                    v.btn("full");
                } else {
                    self.quit = true;
                }
            }
            Keysym::Return | Keysym::KP_Enter if v.cropping => v.apply_crop(),
            Keysym::Left | Keysym::Page_Up | Keysym::BackSpace => {
                if !v.fit && !c {
                    v.cx -= 60.0 / v.z();
                    v.clamp_center();
                } else {
                    v.go(-1);
                }
            }
            Keysym::Right | Keysym::Page_Down => {
                if !v.fit && !c {
                    v.cx += 60.0 / v.z();
                    v.clamp_center();
                } else {
                    v.go(1);
                }
            }
            Keysym::Up if !v.fit => {
                v.cy -= 60.0 / v.z();
                v.clamp_center();
            }
            Keysym::Down if !v.fit => {
                v.cy += 60.0 / v.z();
                v.clamp_center();
            }
            Keysym::Home => {
                v.idx = 0;
                v.reset_view();
                v.want();
            }
            Keysym::End => {
                v.idx = v.list.len().saturating_sub(1);
                v.reset_view();
                v.want();
            }
            Keysym::space => v.btn("slides"),
            Keysym::F11 => v.btn("full"),
            Keysym::Delete => v.btn("trash"),
            Keysym::plus | Keysym::equal | Keysym::KP_Add => v.zoom_at(1.25, cx, cy),
            Keysym::minus | Keysym::KP_Subtract => v.zoom_at(0.8, cx, cy),
            _ => match e.raw_code {
                // по физическим клавишам — и в русской раскладке
                11 => v.btn("fit"),                      // 0
                2 => v.btn("one"),                       // 1
                3 => v.set_zoom(2.0),                    // 2
                19 if s => v.btn("rotl"),                // shift+r
                19 => v.btn("rotr"),                     // r
                35 => v.btn("fliph"),                    // h
                47 if !c => v.btn("flipv"),              // v
                46 if c => v.btn("copy"),                // ctrl+c
                46 => v.btn("crop"),                     // c
                31 if c && s => v.btn("saveas"),         // ctrl+shift+s
                31 if c => v.btn("save"),                // ctrl+s
                44 if c => v.undo(),                     // ctrl+z
                33 => v.btn("full"),                     // f
                23 => v.btn("info"),                     // i
                48 => v.btn("bg"),                       // b
                17 => v.btn("wall"),                     // w
                18 => v.btn("folder"),                   // e
                16 if c => self.quit = true,             // ctrl+q
                _ => {}
            },
        }
        self.effects();
        self.dirty = true;
    }

    fn on_pointer(&mut self, e: &PointerEvent) {
        let (x, y) = e.position;
        match e.kind {
            PointerEventKind::Enter { serial } => {
                self.enter_serial = serial;
                self.cursor_now = None;
                self.v.ptr = Some((x, y));
                self.set_cursor(Shape::Default);
            }
            PointerEventKind::Leave { .. } => {
                self.v.ptr = None;
                self.v.over_ui = false;
                self.dirty = true;
            }
            PointerEventKind::Motion { .. } => {
                self.v.ptr = Some((x, y));
                self.v.active_at = Instant::now();
                let z = self.zone_at(x, y);
                self.v.over_ui = matches!(z.as_ref().map(|z| &z.act), Some(Act::Btn(_) | Act::Block));
                match &self.drag {
                    Some(Drag::Pan(px, py, cx0, cy0)) => {
                        let k = self.v.z();
                        self.v.cx = cx0 - ((x - px) as f32) / k;
                        self.v.cy = cy0 - ((y - py) as f32) / k;
                        self.v.clamp_center();
                    }
                    Some(Drag::Crop) => {
                        if let (Some((ix, iy)), Some(p)) = (self.v.to_img(x, y), self.v.pic()) {
                            let (w, h) = (p.w() as f32, p.h() as f32);
                            if let Some(c) = &mut self.v.crop {
                                c.2 = ix.clamp(0.0, w);
                                c.3 = iy.clamp(0.0, h);
                            }
                        }
                    }
                    None => {
                        let shape = match z.map(|z| z.act) {
                            Some(Act::Btn(_)) => Shape::Pointer,
                            Some(Act::Bg) if self.v.cropping => Shape::Crosshair,
                            Some(Act::Bg) if !self.v.fit => Shape::Grab,
                            _ => Shape::Default,
                        };
                        self.set_cursor(shape);
                    }
                }
                self.dirty = true;
            }
            PointerEventKind::Press { button, .. } => {
                let z = self.zone_at(x, y);
                match button {
                    0x113 => self.v.go(-1),
                    0x114 => self.v.go(1),
                    0x112 => self.v.btn(if self.v.fit { "one" } else { "fit" }),
                    0x110 => match z.as_ref().map(|z| z.act.clone()) {
                        Some(Act::Bg) => {
                            let dbl = self.last_click.map(|t| t.elapsed() < Duration::from_millis(350)).unwrap_or(false);
                            self.last_click = Some(Instant::now());
                            if self.v.cropping {
                                if let (Some((ix, iy)), Some(p)) = (self.v.to_img(x, y), self.v.pic()) {
                                    let (ix, iy) = (ix.clamp(0.0, p.w() as f32), iy.clamp(0.0, p.h() as f32));
                                    self.v.crop = Some((ix, iy, ix, iy));
                                    self.drag = Some(Drag::Crop);
                                }
                            } else if dbl {
                                self.v.btn("full");
                            } else {
                                if self.v.fit {
                                    // из «вписать» — перетаскивать нечего, но центр запомним
                                    if let Some((w, h)) = self.v.pic().map(|p| (p.w() as f32, p.h() as f32)) {
                                        self.v.cx = w / 2.0;
                                        self.v.cy = h / 2.0;
                                    }
                                }
                                self.drag = Some(Drag::Pan(x, y, self.v.cx, self.v.cy));
                                if !self.v.fit {
                                    self.set_cursor(Shape::Grabbing);
                                }
                            }
                        }
                        Some(a) => self.press_act = Some(a),
                        None => {}
                    },
                    _ => {}
                }
                self.effects();
                self.dirty = true;
            }
            PointerEventKind::Release { button, .. } => {
                if button == 0x110 {
                    self.drag = None;
                    if let Some(a) = self.press_act.take() {
                        if self.zone_at(x, y).map(|z| z.act == a).unwrap_or(false) {
                            if let Act::Btn(b) = a {
                                self.v.btn(b);
                            }
                        }
                    }
                }
                self.effects();
                self.dirty = true;
            }
            PointerEventKind::Axis { vertical, .. } => {
                let d = if vertical.value120 != 0 { vertical.value120 as f64 / 120.0 } else if vertical.discrete != 0 { vertical.discrete as f64 } else { vertical.absolute / 15.0 };
                if d.abs() < 0.01 {
                    return;
                }
                // колесо — масштаб к курсору (как в Gwenview/Loupe); листать — стрелками и кнопками
                self.v.zoom_at(if d < 0.0 { 1.2f32.powf(-d as f32) } else { (1.0 / 1.2f32).powf(d as f32) }, x, y);
                self.v.active_at = Instant::now();
                self.dirty = true;
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
            self.v.w = w.get() as i32;
            self.v.h = h.get() as i32;
        }
        self.v.full = c.is_fullscreen();
        if !self.v.fit {
            self.v.clamp_center();
        }
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
delegate_registry!(App);

