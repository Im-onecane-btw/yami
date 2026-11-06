// Обработчики Wayland окна файлов: кадры, мониторы, xdg-окно, seat, клавиатура, мышь, буфер
// обмена и перетаскивание (wl_data_device), дробный масштаб, viewporter, размытие.

use crate::app::*;
use crate::draw;
use smithay_client_toolkit::{
    compositor::CompositorHandler,
    data_device_manager::{
        data_device::DataDeviceHandler,
        data_offer::{DataOfferHandler, DragOffer},
        data_source::DataSourceHandler, WritePipe,
    },
    delegate_compositor, delegate_data_device, delegate_keyboard, delegate_output, delegate_pointer, delegate_registry, delegate_seat, delegate_shm, delegate_xdg_shell, delegate_xdg_window,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{PointerEvent, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::xdg::window::{DecorationMode, Window, WindowConfigure, WindowHandler},
    shm::{Shm, ShmHandler},
};
use std::io::{Read, Write};
use std::path::PathBuf;
use wayland_client::{
    protocol::{wl_data_device::WlDataDevice, wl_data_device_manager::DndAction, wl_data_source::WlDataSource, wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface},
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols::ext::background_effect::v1::client::{ext_background_effect_manager_v1, ext_background_effect_surface_v1};
use wayland_protocols::wp::fractional_scale::v1::client::{wp_fractional_scale_manager_v1 as frac_mgr, wp_fractional_scale_v1 as frac};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};

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
        self.maximized = c.is_maximized();
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
    pub(crate) fn dnd_motion(&mut self, x: f64, y: f64) {
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
                let Some(v) = self.fm.pane_m(pi) else { return };
                if y < (g.area.y + 24) as f64 {
                    v.target = (v.target - 14.0).max(0.0);
                } else if y > (g.area.bottom() - 24) as f64 {
                    v.target = (v.target + 14.0).min(ms);
                }
            }
        }
        self.dirty = true;
    }

    pub(crate) fn pending_finish(&mut self, off: DragOffer) {
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
