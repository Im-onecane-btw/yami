// Скриншоты: экран замораживается (grim), поверх — оверлей на каждом мониторе в пиксельном
// стиле шелла: вуаль со строками развёртки, выделение «бегущими муравьями» с ручками, лупа
// с пиксельной сеткой и пипеткой, панель инструментов (перо, рамка, стрелка, маркер,
// мозаика, номерки), палитра из темы. Enter — готово: в буфер и/или в папку, уведомление
// с миниатюрой. Режимы: область, окно (по раскладке niri), экран. Таймер — 3/5/10 с.
//
//   yami-shell msg screenshot [area|window|screen]   (Print, Alt+Print, Ctrl+Print)

use crate::app::{App, Msg, SKind, State};
use crate::font::PX;
use crate::gfx::{BoxStyle, Canvas, Img, C, R};
use crate::ui::{Act, Cur, Tone, Ui};
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Keysym};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Area,
    Window,
    Screen,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Select,
    Pen,
    Rect,
    Arrow,
    Marker,
    Mosaic,
    Count,
}

pub const TOOLS: &[(Tool, &str, &str, &str)] = &[
    (Tool::Select, "cursor", "t:select", "выделение"),
    (Tool::Pen, "pen", "t:pen", "перо"),
    (Tool::Rect, "rect", "t:rect", "рамка"),
    (Tool::Arrow, "arrow", "t:arrow", "стрелка"),
    (Tool::Marker, "marker", "t:marker", "маркер"),
    (Tool::Mosaic, "mosaic", "t:mosaic", "мозаика"),
    (Tool::Count, "count", "t:count", "номерок"),
];
const COLOR_ACTS: [&str; 6] = ["c:0", "c:1", "c:2", "c:3", "c:4", "c:5"];

#[derive(Clone, Debug)]
pub struct Mark {
    pub tool: Tool,
    pub pts: Vec<(i32, i32)>,
    pub color: C,
    pub size: i32,
    pub n: u32,
}

#[derive(Clone, Copy, Debug)]
enum Drag {
    New(i32, i32),
    Move(i32, i32, R),
    /// ручка: 0..8 по часовой с левого верхнего угла, исходный прямоугольник
    Handle(u8, R),
    Draw,
}

pub struct Snap {
    pub imgs: HashMap<String, Rc<Img>>,
    pub mode: Mode,
    pub sel: Option<(String, R)>,
    drag: Option<Drag>,
    pub tool: Tool,
    pub color: usize,
    pub size: i32,
    pub marks: Vec<Mark>,
    pub redo: Vec<Mark>,
    pub ptr: Option<(String, i32, i32)>,
    pub flash: Option<Instant>,
    /// окна на видимых рабочих столах: монитор, рамка, заголовок
    pub wins: Vec<(String, R, String)>,
    pub busy: bool,
    pub hint: Option<(String, Instant)>,
}

impl Snap {
    pub fn colors(t: &crate::theme::Theme) -> [C; 6] {
        [t.red, C::hex(0xffffff), C::hex(0x0b0909), t.ok, t.warn, C::hex(0x3a9fd0)]
    }
    fn cur_sel(&self, out: &str) -> Option<R> {
        self.sel.as_ref().filter(|s| s.0 == out).map(|s| s.1)
    }
}

impl Snap {
    /// для `yami-shell shot`: «рабочий стол» из полос, выделение и пара меток
    /// снимок для `shot`: шахматка с «окном» или настоящий кадр (`shot --frame`, картинки README);
    /// разметка задана для 1280×720 и масштабируется под кадр
    pub fn mock(w: i32, h: i32, edit: bool, t: &crate::theme::Theme, frame: Option<Vec<u32>>) -> Snap {
        let px = frame.unwrap_or_else(|| {
            let mut px = vec![0u32; (w * h) as usize];
            for y in 0..h {
                for x in 0..w {
                    let c = if y < 32 { t.panel } else if (x / 160 + y / 120) % 2 == 0 { t.surface_hi } else { t.bg.lerp(t.red, 0.12) };
                    px[(y * w + x) as usize] = c.0 | 0xff000000;
                }
            }
            for x in 200..1000 {
                for y in 160..520 {
                    px[(y * w + x) as usize] = if y < 190 { t.blood.0 } else { (t.code_bg.0 & 0xffffff) | 0xff000000 } | 0xff000000;
                }
            }
            px
        });
        let k = |v: i32| v * w / 1280;
        let p = |x: i32, y: i32| (k(x), k(y));
        let mut imgs = HashMap::new();
        imgs.insert(String::new(), Rc::new(Img { w, h, px }));
        let colors = Snap::colors(t);
        let marks = if edit {
            vec![
                Mark { tool: Tool::Rect, pts: vec![p(330, 250), p(620, 380)], color: colors[0], size: 4, n: 0 },
                Mark { tool: Tool::Arrow, pts: vec![p(820, 470), p(640, 390)], color: colors[1], size: 4, n: 0 },
                Mark { tool: Tool::Marker, pts: vec![p(260, 420), p(560, 440)], color: colors[4], size: 14, n: 0 },
                Mark { tool: Tool::Count, pts: vec![p(700, 260)], color: colors[0], size: 4, n: 1 },
                Mark { tool: Tool::Mosaic, pts: vec![p(760, 230), p(900, 330)], color: colors[0], size: 4, n: 0 },
            ]
        } else {
            vec![]
        };
        Snap {
            imgs,
            mode: Mode::Area,
            sel: if edit { Some((String::new(), R::new(k(200), k(160), k(800), k(360)))) } else { None },
            drag: None,
            tool: if edit { Tool::Arrow } else { Tool::Select },
            color: 0,
            size: 4,
            marks,
            redo: vec![],
            ptr: Some((String::new(), k(if edit { 1100 } else { 640 }), k(if edit { 600 } else { 400 }))),
            flash: None,
            wins: vec![],
            busy: false,
            hint: None,
        }
    }
}

/// grim -t ppm: P6, ширина, высота, 255, RGB
fn parse_ppm(b: &[u8]) -> Option<Img> {
    let mut fields = vec![];
    let mut i = 0;
    while fields.len() < 4 && i < b.len() {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if b.get(i) == Some(&b'#') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        let s = i;
        while i < b.len() && !b[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(String::from_utf8_lossy(&b[s..i]).to_string());
    }
    i += 1;
    if fields.first().map(|f| f != "P6").unwrap_or(true) {
        return None;
    }
    let w: i32 = fields.get(1)?.parse().ok()?;
    let h: i32 = fields.get(2)?.parse().ok()?;
    let data = b.get(i..i + (w * h * 3) as usize)?;
    let px = data.chunks_exact(3).map(|p| 0xff000000 | (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32).collect();
    Some(Img { w, h, px })
}

pub fn grab(out: &str) -> Option<Img> {
    let o = std::process::Command::new("grim").args(["-o", out, "-t", "ppm", "-"]).output().ok()?;
    if !o.status.success() {
        return None;
    }
    parse_ppm(&o.stdout)
}

/// окна видимых рабочих столов в координатах их мониторов
fn niri_windows() -> Vec<(String, R, String)> {
    let run = |what: &str| -> serde_json::Value {
        std::process::Command::new("niri").args(["msg", "-j", what]).output().ok().and_then(|o| serde_json::from_slice(&o.stdout).ok()).unwrap_or(serde_json::Value::Null)
    };
    let ws = run("workspaces");
    let wins = run("windows");
    let mut active: HashMap<u64, String> = HashMap::new();
    for w in ws.as_array().into_iter().flatten() {
        if w["is_active"].as_bool().unwrap_or(false) {
            if let (Some(id), Some(out)) = (w["id"].as_u64(), w["output"].as_str()) {
                active.insert(id, out.to_string());
            }
        }
    }
    let mut v = vec![];
    for w in wins.as_array().into_iter().flatten() {
        let Some(out) = w["workspace_id"].as_u64().and_then(|i| active.get(&i)) else { continue };
        let l = &w["layout"];
        let (Some(px), Some(py)) = (l["tile_pos_in_workspace_view"][0].as_f64(), l["tile_pos_in_workspace_view"][1].as_f64()) else { continue };
        let ox = l["window_offset_in_tile"][0].as_f64().unwrap_or(0.0);
        let oy = l["window_offset_in_tile"][1].as_f64().unwrap_or(0.0);
        let ww = l["window_size"][0].as_f64().unwrap_or(0.0);
        let wh = l["window_size"][1].as_f64().unwrap_or(0.0);
        let r = R::new((px + ox).round() as i32, (py + oy).round() as i32, ww as i32, wh as i32);
        // сверху — фокусное и плавающие
        let title = w["title"].as_str().unwrap_or("").to_string();
        if w["is_floating"].as_bool().unwrap_or(false) || w["is_focused"].as_bool().unwrap_or(false) {
            v.insert(0, (out.clone(), r, title));
        } else {
            v.push((out.clone(), r, title));
        }
    }
    v
}

fn norm(x0: i32, y0: i32, x1: i32, y1: i32) -> R {
    R::new(x0.min(x1), y0.min(y1), (x1 - x0).abs() + 1, (y1 - y0).abs() + 1)
}

fn handles(r: R) -> [R; 8] {
    let s = 8;
    let (x0, x1, xm) = (r.x - s / 2, r.right() - s / 2, r.x + r.w / 2 - s / 2);
    let (y0, y1, ym) = (r.y - s / 2, r.bottom() - s / 2, r.y + r.h / 2 - s / 2);
    [R::new(x0, y0, s, s), R::new(xm, y0, s, s), R::new(x1, y0, s, s), R::new(x1, ym, s, s), R::new(x1, y1, s, s), R::new(xm, y1, s, s), R::new(x0, y1, s, s), R::new(x0, ym, s, s)]
}

// ── рисование меток (и на экране, и в файле) ───────────────────────────────
/// квадратная кисть size по линии Брезенхэма
fn line(cv: &mut Canvas, a: (i32, i32), b: (i32, i32), size: i32, c: C) {
    let (mut x, mut y) = a;
    let dx = (b.0 - x).abs();
    let dy = -(b.1 - y).abs();
    let sx = if x < b.0 { 1 } else { -1 };
    let sy = if y < b.1 { 1 } else { -1 };
    let mut err = dx + dy;
    let h = size / 2;
    loop {
        cv.set(R::new(x - h, y - h, size, size), c);
        if x == b.0 && y == b.1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// метка m; координаты экрана → буфер: (p - o) * k
pub fn paint(cv: &mut Canvas, fonts: &mut crate::font::Fonts, m: &Mark, base: &Img, o: (i32, i32), k: i32, on_accent: C) {
    let tr = |p: (i32, i32)| ((p.0 - o.0) * k, (p.1 - o.1) * k);
    let size = (m.size * k).max(1);
    match m.tool {
        Tool::Pen => {
            for w in m.pts.windows(2) {
                line(cv, tr(w[0]), tr(w[1]), size, m.color);
            }
            if m.pts.len() == 1 {
                line(cv, tr(m.pts[0]), tr(m.pts[0]), size, m.color);
            }
        }
        Tool::Rect | Tool::Marker | Tool::Mosaic => {
            let (Some(a), Some(b)) = (m.pts.first(), m.pts.last()) else { return };
            let (a, b) = (tr(*a), tr(*b));
            let r = norm(a.0, a.1, b.0, b.1);
            match m.tool {
                Tool::Rect => cv.outline(r, m.color, size),
                Tool::Marker => cv.fill(r, m.color.alpha(0.38)),
                _ => {
                    // мозаика: средний цвет клетки исходника (база — в своём масштабе: экран × k)
                    let cell = 10 * k;
                    let mut y = r.y;
                    while y < r.bottom() {
                        let mut x = r.x;
                        while x < r.right() {
                            let cw = cell.min(r.right() - x);
                            let ch = cell.min(r.bottom() - y);
                            // точка исходника: буфер → экран·k → база
                            let bx = (x + o.0 * k).clamp(0, base.w - 1);
                            let by = (y + o.1 * k).clamp(0, base.h - 1);
                            let (mut rs, mut gs, mut bs, mut n) = (0u32, 0u32, 0u32, 0u32);
                            let mut yy = by;
                            while yy < (by + ch).min(base.h) {
                                let mut xx = bx;
                                while xx < (bx + cw).min(base.w) {
                                    let p = base.px[(yy * base.w + xx) as usize];
                                    rs += (p >> 16) & 255;
                                    gs += (p >> 8) & 255;
                                    bs += p & 255;
                                    n += 1;
                                    xx += 2;
                                }
                                yy += 2;
                            }
                            let n = n.max(1);
                            cv.set(R::new(x, y, cw, ch), C(0xff000000 | (rs / n) << 16 | (gs / n) << 8 | bs / n));
                            x += cell;
                        }
                        y += cell;
                    }
                }
            }
        }
        Tool::Arrow => {
            let (Some(a), Some(b)) = (m.pts.first(), m.pts.last()) else { return };
            let (a, b) = (tr(*a), tr(*b));
            line(cv, a, b, size, m.color);
            let ang = ((b.1 - a.1) as f32).atan2((b.0 - a.0) as f32);
            let len = (size as f32 * 4.0 + 10.0 * k as f32).min((((b.0 - a.0).pow(2) + (b.1 - a.1).pow(2)) as f32).sqrt());
            for d in [2.6f32, -2.6] {
                let e = (b.0 + (len * (ang + d).cos()) as i32, b.1 + (len * (ang + d).sin()) as i32);
                line(cv, b, e, size, m.color);
            }
        }
        Tool::Count => {
            let Some(p) = m.pts.first() else { return };
            let p = tr(*p);
            let s = 26 * k;
            let r = R::new(p.0 - s / 2, p.1 - s / 2, s, s);
            cv.pxbox(r, &BoxStyle::new(m.color, C::hex(0x0b0909)).shadow(2 * k));
            let face = crate::font::px(16 * k as u16);
            let txt = m.n.to_string();
            let tc = if crate::theme::lightness(m.color) > 0.6 { C::hex(0x0b0909) } else { on_accent };
            cv.text_center(fonts, face, r, &txt, tc);
        }
        Tool::Select => {}
    }
}

// ── отрисовка оверлея ──────────────────────────────────────────────────────
pub fn draw(ui: &mut Ui, st: &State, out: &str, w: i32, h: i32) {
    let t = ui.t;
    let Some(sn) = &st.snap else { return };
    let Some(img) = sn.imgs.get(out) else { return };
    if img.w == w && img.h == h {
        ui.cv.blit(img, 0, 0);
    } else {
        ui.cv.image_scaled(img, R::new(0, 0, w, h), 1.0);
    }
    if let Some(f) = sn.flash {
        // затвор: белая вспышка, гаснет клетками
        let k = 1.0 - (f.elapsed().as_millis() as f32 / 260.0).min(1.0);
        ui.cv.fill(R::new(0, 0, w, h), C(0xffffffff).alpha(0.85 * k));
        ui.anim = true;
        return;
    }
    let sel = sn.cur_sel(out);
    let veil = t.bg.alpha(0.52);
    match sel {
        Some(r) => {
            ui.cv.fill(R::new(0, 0, w, r.y), veil);
            ui.cv.fill(R::new(0, r.bottom(), w, h - r.bottom()), veil);
            ui.cv.fill(R::new(0, r.y, r.x, r.h), veil);
            ui.cv.fill(R::new(r.right(), r.y, w - r.right(), r.h), veil);
        }
        None => ui.cv.fill(R::new(0, 0, w, h), veil),
    }
    // строки развёртки по вуали
    for y in (0..h).step_by(4) {
        match sel {
            Some(r) if y >= r.y && y < r.bottom() => {
                ui.cv.fill(R::new(0, y, r.x, 1), C(0xff000000).alpha(0.16));
                ui.cv.fill(R::new(r.right(), y, w - r.right(), 1), C(0xff000000).alpha(0.16));
            }
            _ => ui.cv.fill(R::new(0, y, w, 1), C(0xff000000).alpha(0.16)),
        }
    }
    let ph = (st.started.elapsed().as_millis() / 70) as i32;
    // режим «окно»: окно под курсором
    if sn.mode == Mode::Window && sn.drag.is_none() {
        if let Some((_, px, py)) = sn.ptr.as_ref().filter(|p| p.0 == out) {
            if let Some((_, r, title)) = sn.wins.iter().find(|(o, r, _)| o == out && r.contains(*px as f64, *py as f64)) {
                if sel.map(|s| s != *r).unwrap_or(true) {
                    ui.cv.fill(*r, t.red.alpha(0.12));
                    ui.cv.ants(*r, t.ember, 4, ph);
                    let tw = ui.f.width(PX, title).min(r.w - 20) + 20;
                    let tr = R::new(r.x + 8, r.y + 8, tw, 24);
                    ui.cv.pxbox(tr, &BoxStyle::new(t.panel.alpha(0.95), t.red).shadow(2));
                    ui.cv.text_in(ui.f, PX, R::new(tr.x + 10, tr.y, tr.w - 14, tr.h), title, t.text);
                }
            }
        }
    }
    // метки
    if let Some(r) = sel {
        ui.cv.push_clip(r);
        for m in &sn.marks {
            paint(&mut ui.cv, ui.f, m, img, (0, 0), 1, t.on_accent);
        }
        ui.cv.pop_clip();
        ui.cv.ants(R::new(r.x - 1, r.y - 1, r.w + 2, r.h + 2), t.ember, 4, ph);
        ui.anim = true;
        if sn.tool == Tool::Select || sn.drag.is_some() {
            for hr in handles(r) {
                ui.cv.fill(hr, C::hex(0x0b0909));
                ui.cv.fill(hr.inset(1), t.ember);
            }
        }
        // размер
        let s = format!("{} × {}", r.w, r.h);
        let sw = ui.f.width(PX, &s) + 16;
        let ly = if r.y > 30 { r.y - 28 } else { r.y + 6 };
        let lr = R::new(r.x, ly, sw, 22);
        ui.cv.pxbox(lr, &BoxStyle::new(t.panel.alpha(0.95), t.border));
        ui.cv.text_center(ui.f, PX, lr, &s, t.text);
        toolbar(ui, sn, r, w, h);
    } else {
        top_bar(ui, sn, w);
    }
    // перекрестье и лупа
    if let Some((_, px, py)) = sn.ptr.as_ref().filter(|p| p.0 == out) {
        let (px, py) = (*px, *py);
        let on_bar = ui.zones.iter().any(|z| matches!(z.act, Act::SnapBtn(_)) && z.r.contains(px as f64, py as f64));
        if sel.is_none() && sn.mode == Mode::Area {
            for x in (0..w).step_by(6) {
                ui.cv.fill(R::new(x, py, 3, 1), t.ember.alpha(0.55));
            }
            for y in (0..h).step_by(6) {
                ui.cv.fill(R::new(px, y, 1, 3), t.ember.alpha(0.55));
            }
        }
        if st.set.shot_loupe && !on_bar && sn.drag.map(|d| !matches!(d, Drag::Move(..) | Drag::Draw)).unwrap_or(true) {
            loupe(ui, img, px, py, w, h);
        }
    }
    if let Some((s, t0)) = &sn.hint {
        if t0.elapsed().as_millis() < 1600 {
            let sw = ui.w(PX, s) + 32;
            let r = R::new((w - sw) / 2, h - 80, sw, 30);
            ui.cv.pxbox(r, &BoxStyle::new(t.panel.alpha(0.96), t.red).shadow(3));
            ui.text_center(PX, r, s, t.text);
            ui.anim = true;
        }
    }
    ui.zone(R::new(0, 0, w, h), Act::Block, Cur::Default);
    // зоны кнопок должны быть поверх «фона»: переставим фон в начало
    if let Some(bg) = ui.zones.pop() {
        ui.zones.insert(0, bg);
    }
}

fn loupe(ui: &mut Ui, img: &Img, px: i32, py: i32, w: i32, h: i32) {
    let t = ui.t;
    let n = 15; // клеток по стороне
    let z = 8; // размер клетки
    let side = n * z;
    let mut x = px + 28;
    let mut y = py + 28;
    if x + side + 8 > w {
        x = px - 28 - side;
    }
    if y + side + 56 > h {
        y = py - 28 - side - 46;
    }
    let fr = R::new(x - 4, y - 4, side + 8, side + 52);
    ui.cv.pxbox(fr, &BoxStyle::new(t.panel.alpha(0.98), t.red).shadow(4));
    // масштаб снимка к экрану (HiDPI)
    let k = img.w as f32 / w.max(1) as f32;
    let (cx, cy) = ((px as f32 * k) as i32, (py as f32 * k) as i32);
    for j in 0..n {
        for i in 0..n {
            let sx = cx + i - n / 2;
            let sy = cy + j - n / 2;
            let c = if sx >= 0 && sy >= 0 && sx < img.w && sy < img.h { C(img.px[(sy * img.w + sx) as usize] | 0xff000000) } else { C::hex(0x000000) };
            ui.cv.set(R::new(x + i * z, y + j * z, z, z), c);
        }
    }
    // сетка и центральная клетка
    for i in 1..n {
        ui.cv.fill(R::new(x + i * z, y, 1, side), C(0xff000000).alpha(0.22));
        ui.cv.fill(R::new(x, y + i * z, side, 1), C(0xff000000).alpha(0.22));
    }
    let c0 = R::new(x + (n / 2) * z, y + (n / 2) * z, z, z);
    ui.cv.outline(R::new(c0.x - 1, c0.y - 1, z + 2, z + 2), C::hex(0x0b0909), 1);
    ui.cv.outline(c0, C(0xffffffff), 1);
    let col = if cx >= 0 && cy >= 0 && cx < img.w && cy < img.h { img.px[(cy * img.w + cx) as usize] & 0xffffff } else { 0 };
    ui.cv.fill(R::new(x, y + side + 8, 14, 14), C::hex(col));
    ui.cv.outline(R::new(x, y + side + 8, 14, 14), t.border, 1);
    ui.cv.text_in(ui.f, PX, R::new(x + 20, y + side + 4, side - 20, 22), &format!("#{col:06x}"), t.text);
    ui.cv.text_in(ui.f, PX, R::new(x, y + side + 24, side, 20), &format!("{px}, {py}"), t.text_faint);
}

fn top_bar(ui: &mut Ui, sn: &Snap, w: i32) {
    let t = ui.t;
    let modes = [(Mode::Area, "область", "crop_", "m:area"), (Mode::Window, "окно", "window", "m:window"), (Mode::Screen, "экран", "screen", "m:screen")];
    let bw = 112;
    let tw = bw * 3 + 8 * 2 + 16 + 44;
    let r = R::new((w - tw) / 2, 18, tw, 44);
    ui.cv.pxbox(r, &BoxStyle::new(t.panel.alpha(0.96), t.border.lerp(t.red, 0.5)).shadow(4));
    ui.zone(r, Act::SnapBtn("none"), Cur::Default);
    for (i, (m, label, icon, act)) in modes.iter().enumerate() {
        let br = R::new(r.x + 8 + i as i32 * (bw + 8), r.y + 7, bw, 30);
        let icon = if *icon == "crop_" { "rect" } else { icon };
        ui.button(br, label, icon, Tone::Ghost, sn.mode == *m, true, Act::SnapBtn(act));
    }
    ui.button(R::new(r.right() - 40, r.y + 7, 32, 30), "", "timer", Tone::Ghost, false, true, Act::SnapBtn("delay"));
    let hint = match sn.mode {
        Mode::Area => "тяни — выделить · enter — весь экран · esc — отмена",
        Mode::Window => "щёлкни по окну · enter — окно под курсором · esc — отмена",
        Mode::Screen => "щёлкни или enter — весь монитор · esc — отмена",
    };
    let hw = ui.w(PX, hint) + 24;
    let hr = R::new((w - hw) / 2, r.bottom() + 8, hw, 24);
    ui.cv.pxbox(hr, &BoxStyle::new(t.bg.alpha(0.85), C(0)));
    ui.text_center(PX, hr, hint, t.text_dim);
}

fn toolbar(ui: &mut Ui, sn: &Snap, sel: R, w: i32, h: i32) {
    let t = ui.t;
    let bs = 30;
    let colors = Snap::colors(&t);
    let n_tools = TOOLS.len() as i32;
    let tw = 8 + n_tools * (bs + 2) + 10 + 6 * 22 + 10 + 3 * (bs + 2) + 10 + 5 * (bs + 2) + 4;
    let th = 44;
    let mut x = sel.x + (sel.w - tw) / 2;
    x = x.clamp(6, (w - tw - 6).max(6));
    let mut y = sel.bottom() + 10;
    if y + th > h - 6 {
        y = sel.y - th - 34;
        if y < 6 {
            y = sel.bottom() - th - 10;
        }
    }
    let r = R::new(x, y, tw, th);
    ui.cv.pxbox(r, &BoxStyle::new(t.panel.alpha(0.97), t.border.lerp(t.red, 0.5)).shadow(4));
    ui.zone(r, Act::SnapBtn("none"), Cur::Default);
    let by = r.y + 7;
    let mut bx = r.x + 8;
    for (tool, icon, act, _) in TOOLS {
        ui.button(R::new(bx, by, bs, bs), "", icon, Tone::Ghost, sn.tool == *tool, true, Act::SnapBtn(act));
        bx += bs + 2;
    }
    bx += 4;
    ui.cv.fill(R::new(bx, by + 4, 1, bs - 8), t.border);
    bx += 6;
    for (i, c) in colors.iter().enumerate() {
        let cr = R::new(bx + 2, by + 6, 18, 18);
        let hov = ui.hover(cr);
        if sn.color == i {
            ui.cv.outline(R::new(cr.x - 3, cr.y - 3, cr.w + 6, cr.h + 6), t.ember, 2);
        } else if hov {
            ui.cv.outline(R::new(cr.x - 2, cr.y - 2, cr.w + 4, cr.h + 4), t.border, 1);
        }
        ui.cv.fill(cr, *c);
        ui.cv.outline(cr, C::hex(0x0b0909), 1);
        ui.zone(R::new(cr.x - 2, cr.y - 4, 22, 26), Act::SnapBtn(COLOR_ACTS[i]), Cur::Pointer);
        bx += 22;
    }
    bx += 4;
    ui.cv.fill(R::new(bx, by + 4, 1, bs - 8), t.border);
    bx += 6;
    // толщина: три точки разного размера
    for (k, sz) in [(0, 2), (1, 4), (2, 8)] {
        let br = R::new(bx, by, bs, bs);
        let on = sn.size == [2, 4, 8][k];
        ui.button(br, "", "", Tone::Ghost, on, true, Act::SnapBtn(["s:2", "s:4", "s:8"][k]));
        let d = sz.max(2);
        ui.cv.fill(R::new(br.x + (bs - d) / 2, br.y + (bs - d) / 2, d, d), if on { C(0xffffffff) } else { t.text_dim });
        bx += bs + 2;
    }
    bx += 4;
    ui.cv.fill(R::new(bx, by + 4, 1, bs - 8), t.border);
    bx += 6;
    let acts: [(&str, &str, bool); 5] = [("left", "undo", !sn.marks.is_empty()), ("copy", "copy", true), ("down", "save", true), ("folder", "saveas", true), ("close", "close", true)];
    for (icon, act, on) in acts {
        let tone = if act == "save" { Tone::Primary } else { Tone::Ghost };
        ui.button(R::new(bx, by, bs, bs), "", icon, tone, false, on && !sn.busy, Act::SnapBtn(act));
        bx += bs + 2;
    }
    // подсказка к кнопке под курсором
    if let Some((_, px, py)) = &sn.ptr {
        let tip = ui.zones.iter().rev().find(|z| z.r.contains(*px as f64, *py as f64)).and_then(|z| match &z.act {
            Act::SnapBtn(a) => tip_of(a),
            _ => None,
        });
        if let Some(tip) = tip {
            let tw2 = ui.w(PX, tip) + 18;
            let tr = R::new((*px - tw2 / 2).clamp(4, w - tw2 - 4), if r.y > 40 { r.y - 28 } else { r.bottom() + 6 }, tw2, 22);
            ui.cv.pxbox(tr, &BoxStyle::new(t.panel, t.red));
            ui.text_center(PX, tr, tip, t.text);
        }
    }
}

fn tip_of(a: &str) -> Option<&'static str> {
    Some(match a {
        "t:select" => "выделение · 1",
        "t:pen" => "перо · 2",
        "t:rect" => "рамка · 3",
        "t:arrow" => "стрелка · 4",
        "t:marker" => "маркер · 5",
        "t:mosaic" => "мозаика · 6",
        "t:count" => "номерок · 7",
        "undo" => "отменить · ctrl+z",
        "copy" => "в буфер · ctrl+c",
        "save" => "сохранить · enter",
        "saveas" => "сохранить как… · ctrl+s",
        "close" => "отмена · esc",
        "s:2" | "s:4" | "s:8" => "толщина · колесо",
        _ => return None,
    })
}

// ── поведение ──────────────────────────────────────────────────────────────
pub fn shots_dir(set: &crate::config::Settings) -> PathBuf {
    let d = set.shot_dir.trim();
    if !d.is_empty() {
        if let Some(r) = d.strip_prefix("~/") {
            return crate::config::home().join(r);
        }
        return PathBuf::from(d);
    }
    // XDG_PICTURES_DIR/Screenshots
    let home = crate::config::home();
    let pics = std::fs::read_to_string(crate::config::config_dir().join("user-dirs.dirs"))
        .ok()
        .and_then(|t| t.lines().find_map(|l| l.strip_prefix("XDG_PICTURES_DIR=").map(|v| PathBuf::from(v.trim_matches('"').replace("$HOME", &home.to_string_lossy())))))
        .unwrap_or_else(|| home.join("Pictures"));
    pics.join("Screenshots")
}

fn copy_text(s: &str) {
    use std::io::Write;
    if let Ok(mut ch) = std::process::Command::new("wl-copy").stdin(std::process::Stdio::piped()).spawn() {
        if let Some(i) = ch.stdin.as_mut() {
            let _ = i.write_all(s.as_bytes());
        }
        drop(ch.stdin.take());
        let _ = ch.wait();
    }
}

fn file_name() -> String {
    let tm = crate::app::local_now();
    let mut now: libc::tm = unsafe { std::mem::zeroed() };
    let t = unsafe { libc::time(std::ptr::null_mut()) };
    unsafe { libc::localtime_r(&t, &mut now) };
    format!("yami-{:04}-{:02}-{:02}_{:02}-{:02}-{:02}.png", now.tm_year + 1900, tm.mon, tm.day, tm.hour, tm.min, now.tm_sec)
}

impl App {
    /// начать: таймер (если есть) → снимок всех мониторов в нити → оверлей
    pub fn snap_start(&mut self, mode: Mode, delay: u32) {
        if self.st.locked || self.st.snap.is_some() {
            return;
        }
        self.close_all();
        let outs = self.output_names();
        let tx = self.tx.clone();
        if delay > 0 {
            self.st.snap_countdown = Some((Instant::now(), delay));
            self.show_osd("snap");
        }
        std::thread::spawn(move || {
            if delay > 0 {
                std::thread::sleep(Duration::from_secs(delay as u64));
            } else {
                // меню и всплывашки успевают исчезнуть
                std::thread::sleep(Duration::from_millis(60));
            }
            let imgs: HashMap<String, Img> = outs.iter().filter_map(|o| grab(o).map(|i| (o.clone(), i))).collect();
            let wins = niri_windows();
            let _ = tx.send(Msg::Snap(imgs, wins, mode));
        });
    }

    pub fn on_snap_ready(&mut self, imgs: HashMap<String, Img>, wins: Vec<(String, R, String)>, mode: Mode) {
        self.st.snap_countdown = None;
        if imgs.is_empty() {
            self.toast_note("скриншот не вышел", "нужен grim (пакет grim)");
            return;
        }
        let outs: Vec<String> = imgs.keys().cloned().collect();
        let mut sn = Snap { imgs: imgs.into_iter().map(|(k, v)| (k, Rc::new(v))).collect(), mode, sel: None, drag: None, tool: Tool::Select, color: 0, size: 4, marks: vec![], redo: vec![], ptr: None, flash: None, wins, busy: false, hint: None };
        let focus = self.focus_out();
        if mode == Mode::Screen {
            let (w, h) = self.output_size(&focus);
            sn.sel = Some((focus.clone(), R::new(0, 0, w, h)));
        }
        self.st.snap = Some(sn);
        for o in outs {
            let kb = if o == focus { KeyboardInteractivity::Exclusive } else { KeyboardInteractivity::None };
            self.mk(SKind::Snap, &o, Layer::Overlay, "yami-screenshot", Anchor::all(), (0, 0), (0, 0, 0, 0), -1, kb);
        }
    }

    pub fn snap_close(&mut self) {
        self.st.snap = None;
        for id in self.find(&SKind::Snap) {
            self.destroy(id);
        }
        self.sync_dock();
    }

    fn snap_hint(&mut self, s: &str) {
        if let Some(sn) = &mut self.st.snap {
            sn.hint = Some((s.to_string(), Instant::now()));
        }
    }

    pub fn toast_note(&mut self, summary: &str, body: &str) {
        let n = crate::dbus::Incoming { id: 9_000_000 + (self.st.started.elapsed().as_millis() % 1_000_000) as u32, app: "yami".into(), icon: String::new(), summary: crate::i18n::tr(summary).into(), body: crate::i18n::tr(body).into(), actions: vec![], urgency: 1, image: None, timeout: -1 };
        self.on_notif(n);
    }

    /// мышь над оверлеем; true — событие наше
    pub fn snap_pointer(&mut self, sid: u32, kind: u8, x: f64, y: f64, button: u32, on_btn: bool) -> bool {
        let Some(out) = self.surfs.get(&sid).map(|s| s.output.clone()) else { return false };
        let (w, h) = self.surfs.get(&sid).map(|s| (s.w, s.h)).unwrap_or((0, 0));
        let colors = Snap::colors(&self.st.t);
        let Some(sn) = &mut self.st.snap else { return false };
        if sn.flash.is_some() || sn.busy {
            return true;
        }
        let (xi, yi) = ((x as i32).clamp(0, w - 1), (y as i32).clamp(0, h - 1));
        sn.ptr = Some((out.clone(), xi, yi));
        self.dirty(&SKind::Snap);
        let Some(sn) = &mut self.st.snap else { return false };
        match kind {
            // движение
            0 => {
                let Some(d) = sn.drag else { return false };
                match d {
                    Drag::New(x0, y0) => sn.sel = Some((out, norm(x0, y0, xi, yi))),
                    Drag::Move(dx, dy, r0) => {
                        let nx = (xi - dx).clamp(0, w - r0.w);
                        let ny = (yi - dy).clamp(0, h - r0.h);
                        sn.sel = Some((out, R::new(nx, ny, r0.w, r0.h)));
                    }
                    Drag::Handle(k, r0) => {
                        let (mut x0, mut y0, mut x1, mut y1) = (r0.x, r0.y, r0.right() - 1, r0.bottom() - 1);
                        match k {
                            0 => (x0, y0) = (xi, yi),
                            1 => y0 = yi,
                            2 => (x1, y0) = (xi, yi),
                            3 => x1 = xi,
                            4 => (x1, y1) = (xi, yi),
                            5 => y1 = yi,
                            6 => (x0, y1) = (xi, yi),
                            _ => x0 = xi,
                        }
                        sn.sel = Some((out, norm(x0, y0, x1, y1)));
                    }
                    Drag::Draw => {
                        if let Some(m) = sn.marks.last_mut() {
                            if m.tool == Tool::Pen {
                                if m.pts.last() != Some(&(xi, yi)) {
                                    m.pts.push((xi, yi));
                                }
                            } else if m.pts.len() >= 2 {
                                m.pts[1] = (xi, yi);
                            } else {
                                m.pts.push((xi, yi));
                            }
                        }
                    }
                }
                true
            }
            // нажатие
            1 => {
                if on_btn {
                    return false;
                }
                if button == 0x111 {
                    // ПКМ: снять выделение / выйти
                    if sn.sel.is_some() && sn.mode != Mode::Screen {
                        sn.sel = None;
                        sn.marks.clear();
                    } else {
                        self.snap_close();
                    }
                    return true;
                }
                let sel = sn.cur_sel(&out);
                if let Some(r) = sel {
                    if let Some(k) = handles(r).iter().position(|hr| hr.inset(-3).contains(x, y)) {
                        sn.drag = Some(Drag::Handle(k as u8, r));
                        return true;
                    }
                    if r.contains(x, y) {
                        if sn.tool == Tool::Select {
                            sn.drag = Some(Drag::Move(xi - r.x, yi - r.y, r));
                        } else {
                            let n = sn.marks.iter().filter(|m| m.tool == Tool::Count).count() as u32 + 1;
                            sn.marks.push(Mark { tool: sn.tool, pts: vec![(xi, yi)], color: colors[sn.color], size: if sn.tool == Tool::Marker { 14 } else { sn.size }, n });
                            sn.redo.clear();
                            sn.drag = if sn.tool == Tool::Count { None } else { Some(Drag::Draw) };
                        }
                        return true;
                    }
                }
                match sn.mode {
                    Mode::Screen => {
                        sn.sel = Some((out.clone(), R::new(0, 0, w, h)));
                        sn.marks.clear();
                    }
                    _ => {
                        sn.marks.clear();
                        sn.sel = None;
                        sn.drag = Some(Drag::New(xi, yi));
                    }
                }
                true
            }
            // отпускание
            _ => {
                let Some(d) = sn.drag.take() else { return false };
                if let Drag::New(x0, y0) = d {
                    // щелчок без протяжки: окно под курсором (в любом режиме), иначе — экран
                    if (x0 - xi).abs() < 4 && (y0 - yi).abs() < 4 {
                        let win = sn.wins.iter().find(|(o, r, _)| *o == out && r.contains(x, y)).map(|(_, r, _)| r.inter(&R::new(0, 0, w, h)));
                        sn.sel = match (sn.mode, win) {
                            (Mode::Window, Some(r)) | (Mode::Area, Some(r)) => Some((out, r)),
                            _ => Some((out, R::new(0, 0, w, h))),
                        };
                    }
                }
                if let Drag::Draw = d {
                    // нулевая рамка/стрелка — убрать
                    if let Some(m) = sn.marks.last() {
                        if m.tool != Tool::Pen && m.tool != Tool::Count && m.pts.len() < 2 {
                            sn.marks.pop();
                        }
                    }
                }
                true
            }
        }
    }

    pub fn snap_wheel(&mut self, d: i32) {
        if let Some(sn) = &mut self.st.snap {
            let sizes = [2, 4, 8];
            let i = sizes.iter().position(|&s| s == sn.size).unwrap_or(1) as i32;
            sn.size = sizes[(i - d).clamp(0, 2) as usize];
            self.dirty(&SKind::Snap);
        }
    }

    pub fn snap_btn(&mut self, a: &str) {
        let Some(sn) = &mut self.st.snap else { return };
        if let Some(t) = TOOLS.iter().find(|t| t.2 == a) {
            sn.tool = t.0;
        } else if let Some(c) = a.strip_prefix("c:") {
            sn.color = c.parse().unwrap_or(0);
            if sn.tool == Tool::Select || sn.tool == Tool::Mosaic {
                sn.tool = Tool::Pen;
            }
        } else if let Some(s) = a.strip_prefix("s:") {
            sn.size = s.parse().unwrap_or(4);
        } else {
            match a {
                "m:area" => sn.mode = Mode::Area,
                "m:window" => sn.mode = Mode::Window,
                "m:screen" => sn.mode = Mode::Screen,
                "undo" => {
                    if let Some(m) = sn.marks.pop() {
                        sn.redo.push(m);
                    }
                }
                "delay" => {
                    let d = match self.st.set.shot_delay {
                        0 => 3,
                        3 => 5,
                        5 => 10,
                        _ => 3,
                    };
                    self.st.set.shot_delay = d;
                    self.settings_changed();
                    let mode = self.st.snap.as_ref().map(|s| s.mode).unwrap_or(Mode::Area);
                    self.snap_close();
                    self.snap_start(mode, d);
                    return;
                }
                "copy" => self.snap_finish(true, false, false),
                "save" => {
                    let (c, s) = (self.st.set.shot_copy, self.st.set.shot_save);
                    self.snap_finish(c, s || !c, false);
                }
                "saveas" => self.snap_finish(false, false, true),
                "close" => self.snap_close(),
                _ => {}
            }
        }
        self.dirty(&SKind::Snap);
    }

    pub fn snap_key(&mut self, e: &KeyEvent) {
        let (ctrl, shift) = (self.mods.ctrl, self.mods.shift);
        let ptr_w = self.st.snap.as_ref().and_then(|s| s.ptr.as_ref()).map(|p| self.output_size(&p.0).0).unwrap_or(1);
        let Some(sn) = &mut self.st.snap else { return };
        let raw = e.raw_code;
        match e.keysym {
            Keysym::Escape => {
                if sn.sel.is_some() && sn.mode != Mode::Screen && !sn.marks.is_empty() {
                    sn.marks.clear();
                } else if sn.sel.is_some() && sn.mode == Mode::Area {
                    sn.sel = None;
                } else {
                    self.snap_close();
                    return;
                }
            }
            Keysym::Return | Keysym::KP_Enter | Keysym::space => {
                if sn.sel.is_none() {
                    // окно под курсором или весь монитор
                    let p = sn.ptr.clone();
                    let out = p.as_ref().map(|p| p.0.clone()).unwrap_or_else(|| sn.imgs.keys().next().cloned().unwrap_or_default());
                    let win = p.as_ref().and_then(|(o, x, y)| sn.wins.iter().find(|(wo, r, _)| wo == o && r.contains(*x as f64, *y as f64))).map(|w| w.1);
                    let full = sn.imgs.get(&out).map(|i| R::new(0, 0, i.w, i.h)).unwrap_or_default();
                    sn.sel = Some((out, if sn.mode == Mode::Window { win.unwrap_or(full) } else { full }));
                    if e.keysym == Keysym::space {
                        self.dirty(&SKind::Snap);
                        return;
                    }
                }
                let (c, s) = (self.st.set.shot_copy, self.st.set.shot_save);
                self.snap_finish(c, s || !c, false);
                return;
            }
            Keysym::Left | Keysym::Right | Keysym::Up | Keysym::Down => {
                if let Some((_, r)) = &mut sn.sel {
                    let d = if shift { 10 } else { 1 };
                    let (dx, dy) = match e.keysym {
                        Keysym::Left => (-d, 0),
                        Keysym::Right => (d, 0),
                        Keysym::Up => (0, -d),
                        _ => (0, d),
                    };
                    if ctrl {
                        r.w = (r.w + dx).max(4);
                        r.h = (r.h + dy).max(4);
                    } else {
                        r.x += dx;
                        r.y += dy;
                    }
                }
            }
            Keysym::Tab => {
                sn.mode = match sn.mode {
                    Mode::Area => Mode::Window,
                    Mode::Window => Mode::Screen,
                    Mode::Screen => Mode::Area,
                };
                let m = sn.mode;
                self.snap_hint(match m {
                    Mode::Area => "область",
                    Mode::Window => "окно",
                    Mode::Screen => "экран",
                });
            }
            _ if ctrl => match raw {
                44 => {
                    // ctrl+z / ctrl+shift+z
                    if shift {
                        if let Some(m) = sn.redo.pop() {
                            sn.marks.push(m);
                        }
                    } else if let Some(m) = sn.marks.pop() {
                        sn.redo.push(m);
                    }
                }
                21 => {
                    if let Some(m) = sn.redo.pop() {
                        sn.marks.push(m);
                    }
                }
                46 => {
                    if sn.sel.is_some() {
                        self.snap_finish(true, false, false);
                        return;
                    }
                    // без выделения — цвет под курсором
                    if let Some((o, x, y)) = sn.ptr.clone() {
                        if let Some(img) = sn.imgs.get(&o) {
                            let k = img.w as f32 / ptr_w.max(1) as f32;
                            let (cx, cy) = ((x as f32 * k) as i32, (y as f32 * k) as i32);
                            let c = img.px[(cy.clamp(0, img.h - 1) * img.w + cx.clamp(0, img.w - 1)) as usize] & 0xffffff;
                            copy_text(&format!("#{c:06x}"));
                            self.snap_hint(&crate::tf!("цвет #{:06x} скопирован", "colour #{:06x} copied", c));
                        }
                    }
                }
                31 => {
                    self.snap_finish(false, false, true);
                    return;
                }
                _ => {}
            },
            _ => {
                // 1–7: инструменты
                if (2..=8).contains(&raw) {
                    sn.tool = TOOLS[(raw - 2) as usize].0;
                } else if raw == 33 && sn.sel.is_none() {
                    // f — весь монитор под курсором
                    sn.mode = Mode::Screen;
                } else if raw == 17 {
                    sn.mode = Mode::Window;
                }
            }
        }
        self.dirty(&SKind::Snap);
    }

    /// собрать картинку: вырезать, нанести метки, закодировать в PNG
    fn snap_render(&mut self) -> Option<(Vec<u8>, Img)> {
        use image::ImageEncoder as _;
        let sn = self.st.snap.as_ref()?;
        let (out, r) = sn.sel.clone()?;
        let img = sn.imgs.get(&out)?.clone();
        let (ow, _) = self.output_size(&out);
        let k = (img.w / ow.max(1)).max(1);
        let r = r.inter(&R::new(0, 0, img.w / k, img.h / k));
        if r.empty() {
            return None;
        }
        let (w, h) = (r.w * k, r.h * k);
        let mut buf = vec![0u32; (w * h) as usize];
        {
            let mut cv = Canvas::new(&mut buf, w, h);
            cv.blit(&img, -r.x * k, -r.y * k);
            for m in &sn.marks {
                paint(&mut cv, &mut self.res.fonts, m, &img, (r.x, r.y), k, self.st.t.on_accent);
            }
        }
        let mut rgba = Vec::with_capacity(buf.len() * 4);
        for p in &buf {
            rgba.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, *p as u8, 255]);
        }
        let mut png = vec![];
        image::codecs::png::PngEncoder::new(&mut png).write_image(&rgba, w as u32, h as u32, image::ExtendedColorType::Rgba8).ok()?;
        // миниатюра для уведомления
        let thumb = image::RgbaImage::from_raw(w as u32, h as u32, rgba).map(|i| crate::icons::fit_in(i, crate::notif::IMG_W, crate::notif::IMG_H));
        Some((png, thumb?))
    }

    /// готово: copy — в буфер, save — в папку, ask — спросить место диалогом yami-files
    pub fn snap_finish(&mut self, copy: bool, save: bool, ask: bool) {
        let Some((png, thumb)) = self.snap_render() else { return };
        let dir = shots_dir(&self.st.set);
        let name = file_name();
        let tx = self.tx.clone();
        if let Some(sn) = &mut self.st.snap {
            sn.busy = ask;
            if !ask {
                sn.flash = Some(Instant::now());
            }
        }
        if ask {
            // диалог должен быть виден: убираем оверлей сразу
            self.snap_close();
        }
        self.dirty(&SKind::Snap);
        let notify = self.st.set.shot_notify;
        std::thread::spawn(move || {
            let mut path: Option<PathBuf> = None;
            let mut err = None;
            if ask {
                let _ = std::fs::create_dir_all(&dir);
                let o = std::process::Command::new("yami-files").args(["--pick", "save", "--name", &name, "--folder"]).arg(&dir).args(["--filter", "PNG|*.png"]).output();
                match o {
                    Ok(o) if o.status.success() => path = String::from_utf8_lossy(&o.stdout).lines().next().map(PathBuf::from),
                    _ => return,
                }
            } else if save {
                path = Some(dir.join(&name));
            }
            if let Some(p) = &path {
                if let Some(d) = p.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                if let Err(e) = std::fs::write(p, &png) {
                    err = Some(e.to_string());
                }
            }
            if copy {
                use std::io::Write;
                if let Ok(mut ch) = std::process::Command::new("wl-copy").args(["-t", "image/png"]).stdin(std::process::Stdio::piped()).spawn() {
                    if let Some(i) = ch.stdin.as_mut() {
                        let _ = i.write_all(&png);
                    }
                    drop(ch.stdin.take());
                    let _ = ch.wait();
                }
            }
            let _ = tx.send(Msg::SnapDone { path, copied: copy, err, thumb: if notify { Some(thumb) } else { None } });
        });
    }

    pub fn on_snap_done(&mut self, path: Option<PathBuf>, copied: bool, err: Option<String>, thumb: Option<Img>) {
        if let Some(e) = err {
            self.toast_note("скриншот не сохранился", &e);
            return;
        }
        let Some(thumb) = thumb else { return };
        let summary = match (&path, copied) {
            (Some(_), true) => crate::i18n::tr("скриншот сохранён и скопирован"),
            (Some(_), false) => crate::i18n::tr("скриншот сохранён"),
            _ => crate::i18n::tr("скриншот в буфере обмена"),
        };
        let body = path.as_ref().map(|p| p.display().to_string().replace(&crate::config::home().to_string_lossy().to_string(), "~")).unwrap_or_default();
        let id = 9_500_000 + (self.st.started.elapsed().as_millis() % 400_000) as u32;
        let actions = if path.is_some() { vec![("default".to_string(), crate::i18n::tr("открыть").to_string()), ("folder".to_string(), crate::i18n::tr("в папке").to_string())] } else { vec![] };
        if let Some(p) = path {
            // действия есть только у карточек в истории (≤ 50): старые пути не храним
            let live: Vec<u32> = self.st.notifs.history.iter().map(|n| n.id).collect();
            self.st.shot_files.retain(|k, _| live.contains(k));
            self.st.shot_files.insert(id, p);
        }
        let n = crate::dbus::Incoming { id, app: "yami-screenshot".into(), icon: String::new(), summary: summary.into(), body, actions, urgency: 1, image: Some(thumb), timeout: -1 };
        self.on_notif(n);
    }

    /// уведомление скриншота: открыть файл или его папку (true — обработано)
    pub fn shot_note_action(&mut self, id: u32, key: &str) -> bool {
        let Some(p) = self.st.shot_files.get(&id).cloned() else { return false };
        match key {
            "folder" => App::spawn(&["yami-files".into(), p.display().to_string()]),
            _ => App::spawn(&["xdg-open".into(), p.display().to_string()]),
        }
        true
    }

    /// раз в тик: вспышка догорела — закрыть оверлей
    pub fn snap_tick(&mut self) {
        let done = self.st.snap.as_ref().and_then(|s| s.flash).map(|f| f.elapsed().as_millis() > 280).unwrap_or(false);
        if done {
            self.snap_close();
        }
    }
}
