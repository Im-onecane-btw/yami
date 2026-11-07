// Состояние просмотрщика и его отрисовка. Окно (app.rs) передаёт сюда ввод, забирает эффекты.
// Картинка: «вписать» или свой масштаб с центром (cx, cy) в координатах картинки. Больше 100 % —
// честные пиксели без сглаживания, меньше — сглаженная уменьшенная копия.

use crate::gfx::{BoxStyle, Canvas, Img, C, R};
use crate::pic::{self, Pic};
use crate::ui::{Cur, Tone};
use crate::font::PX;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Block,
    Bg,
    Btn(&'static str),
}

impl yami_px::ui::Action for Act {
    fn block() -> Self {
        Act::Block
    }
}
pub type Ui<'a, 'b> = yami_px::ui::Ui<'a, 'b, Act>;

#[derive(Clone, Debug, PartialEq)]
pub enum Fx {
    Close,
    Fullscreen(bool),
    Load(PathBuf),
    Copy(Vec<u8>),
    SaveAs(PathBuf, String),
}

pub struct V {
    pub look: crate::look::Look,
    pub list: Vec<PathBuf>,
    pub idx: usize,
    pub cache: HashMap<PathBuf, Rc<Pic>>,
    pub pending: Vec<PathBuf>,
    pub errs: HashMap<PathBuf, String>,
    pub frame: usize,
    pub frame_at: Instant,
    /// предумноженные кадры текущей картинки (по запросу)
    pub px: RefCell<Vec<Option<Rc<Img>>>>,
    /// сглаженная уменьшенная копия: размер → картинка
    pub small: RefCell<Option<(i32, i32, usize, Rc<Img>)>>,
    pub fit: bool,
    pub zoom: f32,
    pub cx: f32,
    pub cy: f32,
    pub w: i32,
    pub h: i32,
    pub active_at: Instant,
    pub crop: Option<(f32, f32, f32, f32)>,
    pub cropping: bool,
    pub undo: Vec<Rc<Pic>>,
    pub info: bool,
    pub bg: u8,
    pub full: bool,
    pub slides: Option<Instant>,
    pub toast: Option<(String, Instant)>,
    pub switched: Instant,
    pub born: Instant,
    pub fx: Vec<Fx>,
    pub ptr: Option<(f64, f64)>,
    pub over_ui: bool,
}

impl V {
    pub fn new(look: crate::look::Look, list: Vec<PathBuf>, idx: usize) -> V {
        let now = Instant::now();
        V { look, list, idx, cache: HashMap::new(), pending: vec![], errs: HashMap::new(), frame: 0, frame_at: now, px: RefCell::new(vec![]), small: RefCell::new(None), fit: true, zoom: 1.0, cx: 0.0, cy: 0.0, w: 1100, h: 760, active_at: now, crop: None, cropping: false, undo: vec![], info: false, bg: 0, full: false, slides: None, toast: None, switched: now, born: now, fx: vec![], ptr: None, over_ui: false }
    }

    pub fn path(&self) -> Option<&PathBuf> {
        self.list.get(self.idx)
    }
    pub fn pic(&self) -> Option<&Rc<Pic>> {
        self.path().and_then(|p| self.cache.get(p))
    }
    pub fn toast(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), Instant::now()));
    }
    pub fn title(&self) -> String {
        let n = self.path().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "yami-view".into());
        let e = if self.pic().map(|p| p.edited).unwrap_or(false) { " •" } else { "" };
        format!("{n}{e} — yami-view")
    }

    /// текущая и соседние — всё, что держим декодированным
    pub fn keep(&self) -> Vec<PathBuf> {
        let n = self.list.len();
        if n == 0 {
            return vec![];
        }
        let mut k: Vec<PathBuf> = [self.idx, (self.idx + 1) % n, (self.idx + n - 1) % n].iter().map(|&i| self.list[i].clone()).collect();
        k.dedup();
        k
    }

    /// загрузить текущую и соседние; лишнее из кэша выкинуть
    pub fn want(&mut self) {
        let keep = self.keep();
        if keep.is_empty() {
            return;
        }
        self.cache.retain(|p, pic| keep.contains(p) || pic.edited);
        for p in &keep {
            if !self.cache.contains_key(p) && !self.pending.contains(p) && !self.errs.contains_key(p) {
                self.pending.push(p.clone());
                self.fx.push(Fx::Load(p.clone()));
            }
        }
    }

    /// нить декодирования пропустила заявку: картинка уже не нужна
    pub fn on_skipped(&mut self, p: PathBuf) {
        self.pending.retain(|x| x != &p);
    }

    pub fn on_loaded(&mut self, p: PathBuf, r: Result<Pic, String>) {
        self.pending.retain(|x| x != &p);
        let cur = self.path() == Some(&p);
        match r {
            // долистали дальше, пока она декодировалась: не держать (96 МБ на фото 24 Мп)
            Ok(_) if !self.keep().contains(&p) => return,
            Ok(pic) => {
                self.cache.insert(p, Rc::new(pic));
            }
            Err(e) => {
                self.errs.insert(p, e);
            }
        }
        if cur {
            self.reset_view();
        }
    }

    pub fn reset_view(&mut self) {
        self.fit = true;
        self.frame = 0;
        self.frame_at = Instant::now();
        *self.px.borrow_mut() = vec![];
        *self.small.borrow_mut() = None;
        self.switched = Instant::now();
        self.crop = None;
        self.cropping = false;
        if let Some(p) = self.pic() {
            let (w, h) = (p.w() as f32, p.h() as f32);
            self.cx = w / 2.0;
            self.cy = h / 2.0;
        }
    }

    pub fn go(&mut self, d: i32) {
        let n = self.list.len() as i32;
        if n == 0 {
            return;
        }
        if self.pic().map(|p| p.edited).unwrap_or(false) {
            self.toast(crate::i18n::tr("правка не сохранена: ctrl+s — сохранить, ctrl+z — отменить"));
        }
        self.idx = ((self.idx as i32 + d).rem_euclid(n)) as usize;
        self.undo.clear();
        self.reset_view();
        self.want();
    }

    pub fn fit_zoom(&self) -> f32 {
        let Some(p) = self.pic() else { return 1.0 };
        let (w, h) = (p.w().max(1) as f32, p.h().max(1) as f32);
        let k = ((self.w as f32 - 16.0) / w).min((self.h as f32 - 16.0) / h);
        // маленькое не растягиваем, пиксель-арт — целыми кратностями
        if k >= 1.0 {
            let ki = k.floor();
            if w < 400.0 && h < 400.0 {
                return ki.min(8.0);
            }
            return 1.0;
        }
        k
    }
    pub fn z(&self) -> f32 {
        if self.fit {
            self.fit_zoom()
        } else {
            self.zoom
        }
    }

    /// масштаб вокруг экранной точки (x, y)
    pub fn zoom_at(&mut self, k: f32, x: f64, y: f64) {
        let Some(p) = self.pic() else { return };
        let (iw, ih) = (p.w() as f32, p.h() as f32);
        let z0 = self.z();
        if self.fit {
            self.cx = iw / 2.0;
            self.cy = ih / 2.0;
        }
        let z1 = (z0 * k).clamp(0.02, 64.0);
        // точка под курсором остаётся на месте
        let ix = self.cx + (x as f32 - self.w as f32 / 2.0) / z0;
        let iy = self.cy + (y as f32 - self.h as f32 / 2.0) / z0;
        self.cx = ix - (x as f32 - self.w as f32 / 2.0) / z1;
        self.cy = iy - (y as f32 - self.h as f32 / 2.0) / z1;
        // около 100 % — прилипнуть
        self.zoom = if (z1 - 1.0).abs() < 0.04 { 1.0 } else { z1 };
        self.fit = false;
        self.clamp_center();
    }

    pub fn set_zoom(&mut self, z: f32) {
        let (x, y) = (self.w as f64 / 2.0, self.h as f64 / 2.0);
        let k = z / self.z();
        self.zoom_at(k, x, y);
    }

    pub fn clamp_center(&mut self) {
        let Some(p) = self.pic() else { return };
        let (iw, ih) = (p.w() as f32, p.h() as f32);
        let z = self.z();
        let (hw, hh) = (self.w as f32 / 2.0 / z, self.h as f32 / 2.0 / z);
        self.cx = if iw <= 2.0 * hw { iw / 2.0 } else { self.cx.clamp(hw, iw - hw) };
        self.cy = if ih <= 2.0 * hh { ih / 2.0 } else { self.cy.clamp(hh, ih - hh) };
    }

    /// где картинка на экране
    pub fn dst(&self) -> Option<(f32, f32, f32)> {
        let p = self.pic()?;
        let z = self.z();
        let (iw, ih) = (p.w() as f32, p.h() as f32);
        let (cx, cy) = if self.fit { (iw / 2.0, ih / 2.0) } else { (self.cx, self.cy) };
        Some((self.w as f32 / 2.0 - cx * z, self.h as f32 / 2.0 - cy * z, z))
    }
    pub fn to_img(&self, x: f64, y: f64) -> Option<(f32, f32)> {
        let (ox, oy, z) = self.dst()?;
        Some(((x as f32 - ox) / z, (y as f32 - oy) / z))
    }

    pub fn ui_visible(&self) -> bool {
        self.ui_held() || self.ui_recent()
    }

    /// панели держит не время: обрезка, мышь над ними, картинки нет
    fn ui_held(&self) -> bool {
        self.cropping || self.over_ui || self.pic().is_none()
    }

    fn ui_recent(&self) -> bool {
        self.active_at.elapsed() < Duration::from_millis(1800)
    }

    /// панели видны, но вот-вот спрячутся сами: тику надо успеть (иначе он раз в секунду)
    pub fn hide_soon(&self) -> bool {
        !self.ui_held() && self.ui_recent()
    }

    // ── правка ─────────────────────────────────────────────────────────────
    pub fn edit(&mut self, f: impl Fn(&image::RgbaImage) -> image::RgbaImage) {
        let Some(p) = self.pic().cloned() else { return };
        let frames = p.frames.iter().map(|(img, d)| (f(img), *d)).collect();
        let np = Pic { path: p.path.clone(), frames, bytes: p.bytes, format: p.format.clone(), edited: true };
        self.undo.push(p.clone());
        if self.undo.len() > 4 {
            self.undo.remove(0);
        }
        self.cache.insert(p.path.clone(), Rc::new(np));
        let fit = self.fit;
        self.reset_view();
        self.fit = fit || true;
    }

    pub fn undo(&mut self) {
        if let Some(p) = self.undo.pop() {
            self.cache.insert(p.path.clone(), p);
            self.reset_view();
        } else {
            self.toast(crate::i18n::tr("нечего отменять"));
        }
    }

    pub fn apply_crop(&mut self) {
        let Some((x0, y0, x1, y1)) = self.crop else {
            self.cropping = false;
            return;
        };
        let (ax, bx) = (x0.min(x1).max(0.0) as u32, x0.max(x1) as u32);
        let (ay, by) = (y0.min(y1).max(0.0) as u32, y0.max(y1) as u32);
        if bx <= ax + 1 || by <= ay + 1 {
            self.toast(crate::i18n::tr("выдели область мышью"));
            return;
        }
        self.edit(move |img| {
            let bx = bx.min(img.width());
            let by = by.min(img.height());
            image::imageops::crop_imm(img, ax, ay, bx - ax, by - ay).to_image()
        });
        self.cropping = false;
        self.crop = None;
    }

    pub fn png_bytes(&self) -> Option<Vec<u8>> {
        use image::ImageEncoder as _;
        let p = self.pic()?;
        let f = &p.frames.get(self.frame.min(p.frames.len() - 1))?.0;
        let mut out = vec![];
        image::codecs::png::PngEncoder::new(&mut out).write_image(f.as_raw(), f.width(), f.height(), image::ExtendedColorType::Rgba8).ok()?;
        Some(out)
    }

    pub fn save(&mut self) {
        let Some(p) = self.pic().cloned() else { return };
        if !p.edited {
            self.toast(crate::i18n::tr("изменений нет"));
            return;
        }
        if p.frames.len() > 1 {
            self.toast(crate::i18n::tr("анимацию не перезаписываю: ctrl+shift+s — сохранить как png"));
            return;
        }
        match pic::save(&p, &p.path) {
            Ok(()) => {
                let np = Pic { path: p.path.clone(), frames: p.frames.clone(), bytes: std::fs::metadata(&p.path).map(|m| m.len()).unwrap_or(p.bytes), format: p.format.clone(), edited: false };
                self.cache.insert(p.path.clone(), Rc::new(np));
                self.undo.clear();
                self.toast(crate::i18n::tr("сохранено"));
            }
            Err(e) => self.toast(e),
        }
    }

    pub fn trash(&mut self) {
        let Some(p) = self.path().cloned() else { return };
        match pic::trash(&p) {
            Ok(()) => {
                self.cache.remove(&p);
                self.list.remove(self.idx);
                if self.list.is_empty() {
                    self.fx.push(Fx::Close);
                    return;
                }
                if self.idx >= self.list.len() {
                    self.idx = 0;
                }
                self.reset_view();
                self.want();
                self.toast(crate::i18n::tr("в корзине"));
            }
            Err(e) => self.toast(e),
        }
    }

    pub fn wallpaper(&mut self) {
        let Some(p) = self.path().cloned() else { return };
        if crate::look::set_wallpaper(&p) {
            self.toast(crate::i18n::tr("обои поменяются через секунду"));
        }
    }

    pub fn btn(&mut self, a: &str) {
        let (cx, cy) = (self.w as f64 / 2.0, self.h as f64 / 2.0);
        match a {
            "prev" => self.go(-1),
            "next" => self.go(1),
            "zin" => self.zoom_at(1.25, cx, cy),
            "zout" => self.zoom_at(0.8, cx, cy),
            "fit" => self.fit = true,
            "one" => self.set_zoom(1.0),
            "rotl" => self.edit(image::imageops::rotate270),
            "rotr" => self.edit(image::imageops::rotate90),
            "fliph" => self.edit(image::imageops::flip_horizontal),
            "flipv" => self.edit(image::imageops::flip_vertical),
            "crop" => {
                self.cropping = !self.cropping;
                self.crop = None;
            }
            "crop_ok" => self.apply_crop(),
            "crop_no" => {
                self.cropping = false;
                self.crop = None;
            }
            "copy" => {
                if let Some(b) = self.png_bytes() {
                    self.fx.push(Fx::Copy(b));
                    self.toast(crate::i18n::tr("скопировано"));
                }
            }
            "save" => self.save(),
            "saveas" => {
                if let Some(p) = self.path().cloned() {
                    let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).filter(|e| ["png", "jpg", "jpeg", "webp", "bmp"].contains(&e.as_str())).unwrap_or_else(|| "png".into());
                    let ext = if self.pic().map(|p| p.frames.len() > 1).unwrap_or(false) { "png".into() } else { ext };
                    self.fx.push(Fx::SaveAs(p.parent().map(|d| d.to_path_buf()).unwrap_or_default(), format!("{stem}-edit.{ext}")));
                }
            }
            "wall" => self.wallpaper(),
            "folder" => {
                if let Some(p) = self.path() {
                    spawn(&["yami-files".into(), p.to_string_lossy().to_string()]);
                }
            }
            "trash" => self.trash(),
            "info" => self.info = !self.info,
            "slides" => {
                self.slides = if self.slides.is_some() { None } else { Some(Instant::now()) };
                self.toast(if self.slides.is_some() { crate::i18n::tr("слайд-шоу: каждые 4 с") } else { crate::i18n::tr("слайд-шоу выключено") });
            }
            "full" => {
                self.full = !self.full;
                self.fx.push(Fx::Fullscreen(self.full));
            }
            "bg" => self.bg = (self.bg + 1) % 3,
            "close" => self.fx.push(Fx::Close),
            _ => {}
        }
        if !self.fit {
            self.clamp_center();
        }
    }

    /// кадр анимации сменился? true — нужна перерисовка
    pub fn tick(&mut self) -> bool {
        let mut dirty = false;
        if let Some(p) = self.pic() {
            if p.frames.len() > 1 {
                let d = p.frames[self.frame.min(p.frames.len() - 1)].1;
                if self.frame_at.elapsed().as_millis() as u32 >= d {
                    self.frame = (self.frame + 1) % p.frames.len();
                    self.frame_at = Instant::now();
                    dirty = true;
                }
            }
        }
        if let Some(t) = self.slides {
            if t.elapsed().as_secs() >= 4 && self.list.len() > 1 {
                self.go(1);
                self.slides = Some(Instant::now());
                dirty = true;
            }
        }
        dirty
    }

    pub fn animating(&self) -> bool {
        self.pic().map(|p| p.frames.len() > 1).unwrap_or(false) || self.slides.is_some()
    }

    fn frame_img(&self) -> Option<Rc<Img>> {
        let p = self.pic()?;
        let i = self.frame.min(p.frames.len() - 1);
        let mut px = self.px.borrow_mut();
        if px.len() != p.frames.len() {
            *px = vec![None; p.frames.len()];
        }
        if px[i].is_none() {
            px[i] = Some(Rc::new(pic::to_img(&p.frames[i].0)));
        }
        px[i].clone()
    }
}

pub fn spawn(argv: &[String]) {
    let _ = std::process::Command::new("setsid").arg("-f").args(argv).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
}

fn ease(born: Instant, ms: f32, v: &V) -> f32 {
    let d = v.look.dur(ms);
    if d <= 0.0 {
        return 1.0;
    }
    let t = (born.elapsed().as_secs_f32() * 1000.0 / d).min(1.0);
    1.0 - (1.0 - t).powi(3)
}

/// картинка с масштабом z без сглаживания; только видимая часть, альфа — поверх фона
fn blit_zoom(cv: &mut Canvas, img: &Img, ox: f32, oy: f32, z: f32) {
    let (w, h) = (cv.w, cv.h);
    let x0 = ox.max(0.0) as i32;
    let y0 = oy.max(0.0) as i32;
    let x1 = (ox + img.w as f32 * z).min(w as f32).ceil() as i32;
    let y1 = (oy + img.h as f32 * z).min(h as f32).ceil() as i32;
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    // столбцы источника считаем один раз
    let cols: Vec<i32> = (x0..x1).map(|x| (((x as f32 + 0.5 - ox) / z) as i32).clamp(0, img.w - 1)).collect();
    for y in y0..y1 {
        let sy = (((y as f32 + 0.5 - oy) / z) as i32).clamp(0, img.h - 1);
        let srow = (sy * img.w) as usize;
        let drow = (y * w) as usize;
        for (k, &sx) in cols.iter().enumerate() {
            let s = img.px[srow + sx as usize];
            let d = &mut cv.buf[drow + (x0 as usize + k)];
            let a = s >> 24;
            if a == 255 {
                *d = s;
            } else if a > 0 {
                let inv = 255 - a;
                let ch = |sh: u32| (((s >> sh) & 255) + (((*d >> sh) & 255) * inv + 127) / 255).min(255) << sh;
                *d = ch(24) | ch(16) | ch(8) | ch(0);
            }
        }
    }
}

pub fn draw(ui: &mut Ui, v: &V, w: i32, h: i32) {
    let t = ui.t;
    // фон: тема, шахматка (для прозрачных), чёрный
    let alpha = v.pic().map(|p| p.has_alpha()).unwrap_or(false);
    match v.bg {
        1 => ui.cv.clear(C::hex(0x000000)),
        2 => ui.cv.clear(t.surface_hi),
        _ => ui.cv.clear(t.bg.alpha(t.glass.max(0.9))),
    }
    ui.zone(R::new(0, 0, w, h), Act::Bg, Cur::Default);
    if let (Some(pic), Some((ox, oy, z))) = (v.pic(), v.dst()) {
        let (dw, dh) = ((pic.w() as f32 * z).round() as i32, (pic.h() as f32 * z).round() as i32);
        let r = R::new(ox.round() as i32, oy.round() as i32, dw, dh);
        if alpha && v.bg != 1 {
            // шахматка под прозрачностью
            let c = r.inter(&R::new(0, 0, w, h));
            let mut y = c.y - (c.y - r.y).rem_euclid(8);
            while y < c.bottom() {
                let mut x = c.x - (c.x - r.x).rem_euclid(8);
                while x < c.right() {
                    let odd = ((x - r.x) / 8 + (y - r.y) / 8) % 2 == 0;
                    ui.cv.fill(R::new(x, y, 8, 8).inter(&c), if odd { t.surface } else { t.surface_hi });
                    x += 8;
                }
                y += 8;
            }
        }
        if z < 0.999 {
            // сглаженная копия нужного размера — прямо из декодированной; полная предумноженная
            // (для увеличения) здесь не нужна: ещё одна картинка во весь размер в памяти
            let n = v.frame;
            if !v.px.borrow().is_empty() {
                v.px.borrow_mut().clear();
            }
            let mut sm = v.small.borrow_mut();
            let ok = sm.as_ref().map(|s| s.0 == dw && s.1 == dh && s.2 == n).unwrap_or(false);
            if !ok {
                let f = &pic.frames[n.min(pic.frames.len() - 1)].0;
                *sm = Some((dw, dh, n, Rc::new(pic::scaled(f, dw.max(1) as u32, dh.max(1) as u32))));
            }
            if let Some((_, _, _, s)) = sm.as_ref() {
                if alpha {
                    ui.cv.image(s, r.x, r.y, 1.0);
                } else {
                    ui.cv.blit(s, r.x, r.y);
                }
            }
        } else if let Some(img) = v.frame_img() {
            // крупный план: предумноженная копия кадра, её и рисуем клетками. Копия под окно
            // (не больше окна) остаётся: обратно во «вписать» — без нового масштабирования
            if (z - 1.0).abs() < 0.001 && !alpha {
                ui.cv.blit(&img, r.x, r.y);
            } else {
                blit_zoom(&mut ui.cv, &img, ox, oy, z);
            }
        }
        // смена картинки — проявление клетками
        let k = ease(v.switched, 170.0, v);
        if k < 1.0 {
            ui.cv.dissolve_rect(r.inter(&R::new(0, 0, w, h)), k, 3);
            ui.anim = true;
        }
        if v.cropping {
            crop_overlay(ui, v, w, h, ox, oy, z);
        }
    } else {
        // загрузка или ошибка
        let msg = match v.path() {
            None => crate::i18n::tr("нет картинок").to_string(),
            Some(p) => match v.errs.get(p) {
                Some(e) => crate::tf!("не открывается: {}", "cannot open: {}", "não foi possível abrir: {}", e),
                None => {
                    ui.anim = true;
                    let dots = ".".repeat(1 + (v.born.elapsed().as_millis() / 250 % 3) as usize);
                    format!("{}{dots}", crate::i18n::tr("открываю"))
                }
            },
        };
        ui.cv.icon("image", w / 2 - 14, h / 2 - 40, 4, t.border.lerp(t.red, 0.4));
        ui.cv.text_center(ui.f, PX, R::new(0, h / 2 + 4, w, 20), &msg, t.text_faint);
    }
    if v.ui_visible() {
        top(ui, v, w);
        bar(ui, v, w, h);
    }
    if v.info {
        info(ui, v, w);
    }
    if let Some((s, born)) = &v.toast {
        let el = born.elapsed().as_millis();
        if el < 2600 {
            let tw = ui.w(PX, s).min(w - 60) + 36;
            let r = R::new((w - tw) / 2, h - 110, tw, 32);
            ui.cv.pxbox(r, &BoxStyle::new(t.panel.alpha(0.97), t.red).shadow(3));
            ui.cv.fill(R::new(r.x + 12, r.cy() - 3, 6, 6), t.red);
            ui.text_in(PX, R::new(r.x + 26, r.y, r.w - 32, r.h), s, t.text);
            ui.anim = true;
        }
    }
    let k = ease(v.born, 240.0, v);
    if k < 1.0 {
        ui.cv.dissolve(k, 2);
        ui.anim = true;
    }
}

fn crop_overlay(ui: &mut Ui, v: &V, w: i32, h: i32, ox: f32, oy: f32, z: f32) {
    let t = ui.t;
    let veil = C(0xff000000).alpha(0.55);
    match v.crop {
        Some((x0, y0, x1, y1)) => {
            let sx = |x: f32| (ox + x * z).round() as i32;
            let sy = |y: f32| (oy + y * z).round() as i32;
            let r = R::new(sx(x0.min(x1)), sy(y0.min(y1)), sx(x0.max(x1)) - sx(x0.min(x1)), sy(y0.max(y1)) - sy(y0.min(y1)));
            ui.cv.fill(R::new(0, 0, w, r.y), veil);
            ui.cv.fill(R::new(0, r.bottom(), w, h - r.bottom()), veil);
            ui.cv.fill(R::new(0, r.y, r.x, r.h), veil);
            ui.cv.fill(R::new(r.right(), r.y, w - r.right(), r.h), veil);
            ui.cv.ants(r, t.ember, 4, (v.born.elapsed().as_millis() / 70) as i32);
            // сетка третей
            for k in 1..3 {
                ui.cv.fill(R::new(r.x + r.w * k / 3, r.y, 1, r.h), C(0xffffffff).alpha(0.25));
                ui.cv.fill(R::new(r.x, r.y + r.h * k / 3, r.w, 1), C(0xffffffff).alpha(0.25));
            }
            let s = format!("{} × {}", (x1 - x0).abs().round(), (y1 - y0).abs().round());
            let lw = ui.f.width(PX, &s) + 16;
            let lr = R::new(r.x, (r.y - 28).max(4), lw, 22);
            ui.cv.pxbox(lr, &BoxStyle::new(t.panel.alpha(0.95), t.border));
            ui.cv.text_center(ui.f, PX, lr, &s, t.text);
            ui.anim = true;
        }
        None => {
            ui.cv.fill(R::new(0, 0, w, h), veil.alpha(0.3));
        }
    }
}

fn top(ui: &mut Ui, v: &V, w: i32) {
    let t = ui.t;
    let r = R::new(0, 0, w, 34);
    ui.cv.fill(r, t.panel.alpha(0.9));
    ui.cv.fill(R::new(0, r.bottom() - 1, w, 1), t.border);
    ui.zone(r, Act::Block, Cur::Default);
    ui.cv.icon("image", 10, 10, 2, t.red);
    let name = v.path().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let edited = v.pic().map(|p| p.edited).unwrap_or(false);
    let nw = ui.cv.text_in(ui.f, PX, R::new(32, 0, w / 2, 34), &name, t.text);
    let mut x = 32 + nw + 10;
    if edited {
        ui.cv.fill(R::new(x, 14, 6, 6), t.ember);
        x += 14;
    }
    if v.list.len() > 1 {
        ui.cv.text_in(ui.f, PX, R::new(x, 0, 120, 34), &format!("{} / {}", v.idx + 1, v.list.len()), t.text_faint);
    }
    let mut s = String::new();
    if let Some(p) = v.pic() {
        s = format!("{} × {}  ·  {}  ·  {:.0}%", p.w(), p.h(), pic::human(p.bytes), v.z() * 100.0);
        if p.frames.len() > 1 {
            s = format!("{} · {}/{}", s, v.frame + 1, p.frames.len());
        }
    }
    let bs = 26;
    ui.button(R::new(w - bs - 6, 4, bs, bs), "", "close", Tone::Ghost, false, true, Act::Btn("close"));
    ui.cv.text_right(ui.f, PX, R::new(0, 0, w - bs - 18, 34), &s, t.text_dim);
}

const BAR: &[(&str, &str)] = &[
    ("left", "prev"),
    ("right", "next"),
    ("|", ""),
    ("zoomout", "zout"),
    ("fit", "fit"),
    ("one", "one"),
    ("zoomin", "zin"),
    ("|", ""),
    ("rotl", "rotl"),
    ("rotr", "rotr"),
    ("fliph", "fliph"),
    ("crop", "crop"),
    ("|", ""),
    ("copy", "copy"),
    ("save", "save"),
    ("down", "saveas"),
    ("|", ""),
    ("image", "wall"),
    ("folder", "folder"),
    ("trash", "trash"),
    ("|", ""),
    ("about", "info"),
    ("slides", "slides"),
    ("full", "full"),
];

fn tip(a: &str) -> &'static str {
    match a {
        "prev" => "назад · ←",
        "next" => "вперёд · →",
        "zout" => "мельче · −",
        "fit" => "вписать · 0",
        "one" => "100 % · 1",
        "zin" => "крупнее · +",
        "rotl" => "повернуть влево · shift+r",
        "rotr" => "повернуть вправо · r",
        "fliph" => "отразить · h",
        "crop" => "кадрировать · c",
        "copy" => "в буфер · ctrl+c",
        "save" => "сохранить · ctrl+s",
        "saveas" => "сохранить как… · ctrl+shift+s",
        "wall" => "сделать обоями · w",
        "folder" => "показать в папке · e",
        "trash" => "в корзину · del",
        "info" => "сведения · i",
        "slides" => "слайд-шоу · пробел",
        "full" => "во весь экран · f",
        _ => "",
    }
}

fn bar(ui: &mut Ui, v: &V, w: i32, h: i32) {
    let t = ui.t;
    let bs = 30;
    if v.cropping {
        let labels = [("обрезать", "check", "crop_ok", Tone::Primary), ("отмена", "close", "crop_no", Tone::Plain)];
        let tw: i32 = labels.iter().map(|l| ui.w(PX, l.0) + 50).sum::<i32>() + 24;
        let r = R::new((w - tw) / 2, h - 58, tw, 44);
        ui.cv.pxbox(r, &BoxStyle::new(t.panel.alpha(0.97), t.border.lerp(t.red, 0.5)).shadow(4));
        ui.zone(r, Act::Block, Cur::Default);
        let mut x = r.x + 8;
        for (l, ic, a, tone) in labels {
            let bw = ui.w(PX, l) + 46;
            ui.button(R::new(x, r.y + 7, bw, 30), l, ic, tone, false, true, Act::Btn(a));
            x += bw + 8;
        }
        let hint = crate::i18n::tr("тяни по картинке · enter — обрезать · esc — отмена");
        let hw = ui.f.width(PX, hint) + 20;
        let hr = R::new((w - hw) / 2, r.y - 30, hw, 22);
        ui.cv.pxbox(hr, &BoxStyle::new(t.bg.alpha(0.85), C(0)));
        ui.cv.text_center(ui.f, PX, hr, hint, t.text_dim);
        return;
    }
    let n_sep = BAR.iter().filter(|b| b.0 == "|").count() as i32;
    let n_btn = BAR.len() as i32 - n_sep;
    let tw = 12 + n_btn * (bs + 2) + n_sep * 11;
    let r = R::new((w - tw) / 2, h - 58, tw, 44);
    ui.cv.pxbox(r, &BoxStyle::new(t.panel.alpha(0.96), t.border.lerp(t.red, 0.5)).shadow(4));
    ui.zone(r, Act::Block, Cur::Default);
    let edited = v.pic().map(|p| p.edited).unwrap_or(false);
    let mut x = r.x + 6;
    for (icon, a) in BAR {
        if *icon == "|" {
            ui.cv.fill(R::new(x + 4, r.y + 12, 1, 20), t.border);
            x += 11;
            continue;
        }
        let sel = match *a {
            "info" => v.info,
            "slides" => v.slides.is_some(),
            "full" => v.full,
            "fit" => v.fit,
            _ => false,
        };
        let on = match *a {
            "save" => edited,
            "prev" | "next" => v.list.len() > 1,
            _ => v.pic().is_some(),
        };
        let tone = if *a == "save" && edited { Tone::Primary } else { Tone::Ghost };
        ui.button(R::new(x, r.y + 7, bs, bs), "", icon, tone, sel, on, Act::Btn(a));
        x += bs + 2;
    }
    // подсказка над кнопкой
    if let Some((px, py)) = v.ptr {
        let hit = ui.zones.iter().rev().find(|z| z.r.contains(px, py)).and_then(|z| if let Act::Btn(a) = &z.act { Some(*a) } else { None });
        if let Some(a) = hit {
            let s = tip(a);
            if !s.is_empty() {
                let sw = ui.w(PX, s) + 18;
                let tr = R::new(((px as i32) - sw / 2).clamp(4, w - sw - 4), r.y - 28, sw, 22);
                ui.cv.pxbox(tr, &BoxStyle::new(t.panel, t.red));
                ui.text_center(PX, tr, s, t.text);
            }
        }
    }
}

fn info(ui: &mut Ui, v: &V, w: i32) {
    let t = ui.t;
    let Some(p) = v.pic() else { return };
    let iw = 300;
    let r = R::new(w - iw - 12, 46, iw, 196);
    ui.cv.pxbox(r, &BoxStyle::new(t.panel.alpha(0.96), t.border).shadow(4));
    ui.zone(r, Act::Block, Cur::Default);
    ui.cv.fill(R::new(r.x + 12, r.y + 15, 6, 6), t.red);
    ui.text_in(PX, R::new(r.x + 24, r.y + 6, r.w - 30, 24), "сведения", t.text);
    let mtime = std::fs::metadata(&p.path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| {
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        let s = d.as_secs() as _;
        unsafe { libc::localtime_r(&s, &mut tm) };
        format!("{:02}.{:02}.{} {:02}:{:02}", tm.tm_mday, tm.tm_mon + 1, tm.tm_year + 1900, tm.tm_hour, tm.tm_min)
    }).unwrap_or_default();
    let dir = p.path.parent().map(|d| d.to_string_lossy().replace(&crate::paths::home().to_string_lossy().to_string(), "~")).unwrap_or_default();
    let rows: Vec<(&str, String)> = vec![
        ("размер", format!("{} × {}", p.w(), p.h())),
        ("файл", pic::human(p.bytes)),
        ("формат", if p.frames.len() > 1 { format!("{} · {}", p.format, crate::tf!("кадров {}", "{} frames", "{} quadros", p.frames.len())) } else { p.format.clone() }),
        ("изменён", mtime),
        ("папка", dir),
        ("прозрачность", if p.has_alpha() { crate::i18n::tr("есть").to_string() } else { crate::i18n::tr("нет").to_string() }),
    ];
    let mut y = r.y + 36;
    for (k, val) in rows {
        ui.text_in(PX, R::new(r.x + 14, y, 110, 20), k, t.text_faint);
        ui.cv.text_in(ui.f, PX, R::new(r.x + 124, y, r.w - 136, 20), &val, t.text_dim);
        y += 24;
    }
}
