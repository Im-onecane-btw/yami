// Пиксельная графика в shm-буфер: только целые пиксели, без сглаживания.
// Буфер — ARGB8888 с предумноженной альфой (как ждёт wl_shm), цвет C — обычный 0xAARRGGBB.

use crate::font::{Fonts, Face};
use crate::theme::Theme;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct C(pub u32);

impl C {
    pub const fn hex(rgb: u32) -> C {
        C(0xff000000 | rgb)
    }
    pub fn from_f(r: f32, g: f32, b: f32) -> C {
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
        C(0xff000000 | q(r) << 16 | q(g) << 8 | q(b))
    }
    pub fn a(self) -> u32 {
        self.0 >> 24
    }
    pub fn rgbf(self) -> (f32, f32, f32) {
        (((self.0 >> 16) & 255) as f32 / 255.0, ((self.0 >> 8) & 255) as f32 / 255.0, (self.0 & 255) as f32 / 255.0)
    }
    pub fn alpha(self, a: f32) -> C {
        C(self.0 & 0xffffff | ((a.clamp(0.0, 1.0) * 255.0).round() as u32) << 24)
    }
    /// умножить прозрачность
    pub fn fade(self, k: f32) -> C {
        C(self.0 & 0xffffff | ((self.a() as f32 * k.clamp(0.0, 1.0)).round() as u32) << 24)
    }
    pub fn lerp(self, o: C, t: f32) -> C {
        let t = t.clamp(0.0, 1.0);
        let ch = |s: u32| {
            let a = ((self.0 >> s) & 255) as f32;
            let b = ((o.0 >> s) & 255) as f32;
            ((a + (b - a) * t).round() as u32) << s
        };
        C(ch(24) | ch(16) | ch(8) | ch(0))
    }
    pub fn hex_str(self) -> String {
        format!("#{:06x}", self.0 & 0xffffff)
    }
    fn premul(self) -> u32 {
        let a = self.a();
        if a == 255 {
            return self.0;
        }
        let m = |s: u32| (((self.0 >> s) & 255) * a + 127) / 255;
        a << 24 | m(16) << 16 | m(8) << 8 | m(0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct R {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl R {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> R {
        R { x, y, w, h }
    }
    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x as f64 && py >= self.y as f64 && px < (self.x + self.w) as f64 && py < (self.y + self.h) as f64
    }
    pub fn inset(&self, d: i32) -> R {
        R::new(self.x + d, self.y + d, (self.w - 2 * d).max(0), (self.h - 2 * d).max(0))
    }
    pub fn inter(&self, o: &R) -> R {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = (self.x + self.w).min(o.x + o.w);
        let y1 = (self.y + self.h).min(o.y + o.h);
        R::new(x0, y0, (x1 - x0).max(0), (y1 - y0).max(0))
    }
    pub fn right(&self) -> i32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }
    pub fn cy(&self) -> i32 {
        self.y + self.h / 2
    }
    pub fn empty(&self) -> bool {
        self.w <= 0 || self.h <= 0
    }
}

/// Готовая картинка: предумноженные ARGB-пиксели
pub struct Img {
    pub w: i32,
    pub h: i32,
    pub px: Vec<u32>,
}

pub struct Canvas<'a> {
    pub buf: &'a mut [u32],
    pub w: i32,
    pub h: i32,
    pub ox: i32,
    pub oy: i32,
    clip: R,
    clips: Vec<R>,
    /// общая прозрачность (ступенчатое появление панелей)
    pub opacity: f32,
    /// где под поверхностью размывать фон: только сами стеклянные панели, без полей под тенью
    pub blur: Vec<R>,
}

impl<'a> Canvas<'a> {
    pub fn new(buf: &'a mut [u32], w: i32, h: i32) -> Canvas<'a> {
        Canvas { buf, w, h, ox: 0, oy: 0, clip: R::new(0, 0, w, h), clips: vec![], opacity: 1.0, blur: vec![] }
    }

    /// проявление клетками по матрице Байера 4×4: при k клетка видна, если её порог ниже k
    pub fn dissolve(&mut self, k: f32, cell: i32) {
        const B: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
        if k >= 1.0 {
            return;
        }
        let lvl = (k * 16.0).floor() as i32;
        let cell = cell.max(1);
        for y in 0..self.h {
            let by = ((y / cell) & 3) as usize;
            let row = (y * self.w) as usize;
            for x in 0..self.w {
                if B[by][((x / cell) & 3) as usize] as i32 >= lvl {
                    self.buf[row + x as usize] = 0;
                }
            }
        }
    }

    /// то же проявление, но только внутри r (появление отдельных значков, меню внутри окна)
    pub fn dissolve_rect(&mut self, r: R, k: f32, cell: i32) {
        const B: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
        if k >= 1.0 {
            return;
        }
        let r = R::new(r.x + self.ox, r.y + self.oy, r.w, r.h).inter(&R::new(0, 0, self.w, self.h));
        let lvl = (k * 16.0).floor() as i32;
        let cell = cell.max(1);
        for y in r.y..r.bottom() {
            let by = ((y / cell) & 3) as usize;
            let row = (y * self.w) as usize;
            for x in r.x..r.right() {
                if B[by][((x / cell) & 3) as usize] as i32 >= lvl {
                    self.buf[row + x as usize] = 0;
                }
            }
        }
    }

    /// пунктир по контуру («бегущие муравьи»): phase сдвигает штрихи
    pub fn ants(&mut self, r: R, c: C, dash: i32, phase: i32) {
        let d = dash.max(1);
        let on = |i: i32| ((i + phase).rem_euclid(2 * d)) < d;
        for i in 0..r.w {
            if on(i) {
                self.fill(R::new(r.x + i, r.y, 1, 1), c);
            }
            if on(i + r.h) {
                self.fill(R::new(r.right() - 1 - i, r.bottom() - 1, 1, 1), c);
            }
        }
        for j in 0..r.h {
            if on(j + r.w) {
                self.fill(R::new(r.right() - 1, r.y + j, 1, 1), c);
            }
            if on(j) {
                self.fill(R::new(r.x, r.bottom() - 1 - j, 1, 1), c);
            }
        }
    }

    pub fn clear(&mut self, c: C) {
        let v = c.premul();
        self.buf.fill(v);
    }

    pub fn push_clip(&mut self, r: R) {
        self.clips.push(self.clip);
        let r = R::new(r.x + self.ox, r.y + self.oy, r.w, r.h);
        self.clip = self.clip.inter(&r);
    }
    pub fn pop_clip(&mut self) {
        if let Some(c) = self.clips.pop() {
            self.clip = c;
        }
    }

    #[inline]
    fn blend_at(&mut self, i: usize, src: u32) {
        let sa = src >> 24;
        if sa == 255 {
            self.buf[i] = src;
            return;
        }
        if sa == 0 {
            return;
        }
        let d = self.buf[i];
        let inv = 255 - sa;
        let ch = |s: u32| {
            let sv = (src >> s) & 255;
            let dv = (d >> s) & 255;
            ((sv + (dv * inv + 127) / 255).min(255)) << s
        };
        self.buf[i] = ch(24) | ch(16) | ch(8) | ch(0);
    }

    fn eff(&self, c: C) -> u32 {
        if self.opacity >= 1.0 {
            c.premul()
        } else {
            c.fade(self.opacity).premul()
        }
    }

    pub fn fill(&mut self, r: R, c: C) {
        if c.a() == 0 {
            return;
        }
        let r = R::new(r.x + self.ox, r.y + self.oy, r.w, r.h).inter(&self.clip);
        if r.empty() {
            return;
        }
        let src = self.eff(c);
        let w = self.w as usize;
        for y in r.y..r.y + r.h {
            let row = y as usize * w;
            if src >> 24 == 255 {
                self.buf[row + r.x as usize..row + (r.x + r.w) as usize].fill(src);
            } else {
                for x in r.x..r.x + r.w {
                    self.blend_at(row + x as usize, src);
                }
            }
        }
    }

    /// заменить пиксели (без смешивания) — «дырка» в прозрачность
    pub fn set(&mut self, r: R, c: C) {
        let r = R::new(r.x + self.ox, r.y + self.oy, r.w, r.h).inter(&self.clip);
        if r.empty() {
            return;
        }
        let src = c.premul();
        let w = self.w as usize;
        for y in r.y..r.y + r.h {
            let row = y as usize * w;
            self.buf[row + r.x as usize..row + (r.x + r.w) as usize].fill(src);
        }
    }

    /// пиксель с готовой (уже предумноженной) альфой — для текста и картинок
    #[inline]
    fn put(&mut self, x: i32, y: i32, src: u32) {
        let x = x + self.ox;
        let y = y + self.oy;
        if x < self.clip.x || y < self.clip.y || x >= self.clip.x + self.clip.w || y >= self.clip.y + self.clip.h {
            return;
        }
        let i = y as usize * self.w as usize + x as usize;
        self.blend_at(i, src);
    }

    pub fn outline(&mut self, r: R, c: C, s: i32) {
        self.fill(R::new(r.x, r.y, r.w, s), c);
        self.fill(R::new(r.x, r.bottom() - s, r.w, s), c);
        self.fill(R::new(r.x, r.y + s, s, r.h - 2 * s), c);
        self.fill(R::new(r.right() - s, r.y + s, s, r.h - 2 * s), c);
    }

    /// прямоугольник со срезанными углами (px.rs notched_fill)
    pub fn notched(&mut self, r: R, n: i32, c: C) {
        if r.w <= 2 * n || r.h <= 2 * n {
            self.fill(r, c);
            return;
        }
        self.fill(R::new(r.x + n, r.y, r.w - 2 * n, r.h), c);
        self.fill(R::new(r.x, r.y + n, n, r.h - 2 * n), c);
        self.fill(R::new(r.right() - n, r.y + n, n, r.h - 2 * n), c);
    }

    /// размыть фон под r (углы с фаской n остаются чистыми)
    pub fn blur_rect(&mut self, r: R, n: i32) {
        let r = R::new(r.x + self.ox, r.y + self.oy, r.w, r.h);
        if self.blur.iter().any(|b| b.x <= r.x && b.y <= r.y && b.right() >= r.right() && b.bottom() >= r.bottom()) {
            return;
        }
        if n > 0 {
            self.blur.push(R::new(r.x, r.y + n, r.w, r.h - 2 * n));
            self.blur.push(R::new(r.x + n, r.y, r.w - 2 * n, r.h));
        } else {
            self.blur.push(r);
        }
    }

    pub fn pxbox(&mut self, r: R, st: &BoxStyle) {
        let n = st.notch;
        // стеклянная панель с тенью — верхний уровень, под ней размытие
        if st.shadow > 0 && st.fill.a() < 255 {
            self.blur_rect(r, n);
        }
        if st.shadow > 0 && !st.pressed {
            self.notched(R::new(r.x + st.shadow, r.y + st.shadow, r.w, r.h), n, C(0xb4000000));
        }
        self.notched(r, n, st.fill);
        if st.bevel {
            let hi = st.fill.lerp(C(0xffffffff), if st.pressed { 0.0 } else { 0.22 });
            let lo = st.fill.lerp(C(0xff000000), if st.pressed { 0.1 } else { 0.45 });
            self.fill(R::new(r.x + n, r.y, r.w - 2 * n, 1), hi);
            self.fill(R::new(r.x, r.y + n, 1, r.h - 2 * n), hi);
            self.fill(R::new(r.x + n, r.bottom() - 1, r.w - 2 * n, 1), lo);
            self.fill(R::new(r.right() - 1, r.y + n, 1, r.h - 2 * n), lo);
        }
        let s = st.stroke;
        if s > 0 && st.outline.a() > 0 {
            let o = st.outline;
            self.fill(R::new(r.x + n, r.y, r.w - 2 * n, s), o);
            self.fill(R::new(r.x + n, r.bottom() - s, r.w - 2 * n, s), o);
            self.fill(R::new(r.x, r.y + n, s, r.h - 2 * n), o);
            self.fill(R::new(r.right() - s, r.y + n, s, r.h - 2 * n), o);
            if n > s {
                let k = n - s;
                self.fill(R::new(r.x + s, r.y + s, k, k), o);
                self.fill(R::new(r.right() - n, r.y + s, k, k), o);
                self.fill(R::new(r.x + s, r.bottom() - n, k, k), o);
                self.fill(R::new(r.right() - n, r.bottom() - n, k, k), o);
            }
        }
    }

    /// иконка 7×7 из битмапа клетками cell
    pub fn icon(&mut self, name: &str, x: i32, y: i32, cell: i32, c: C) {
        if let Some(rows) = icon7(name) {
            for (j, row) in rows.iter().enumerate() {
                for (i, ch) in row.bytes().enumerate() {
                    if ch == b'#' {
                        self.fill(R::new(x + i as i32 * cell, y + j as i32 * cell, cell, cell), c);
                    }
                }
            }
        }
    }
    /// иконка по центру прямоугольника
    pub fn icon_in(&mut self, name: &str, r: R, cell: i32, c: C) {
        let s = 7 * cell;
        self.icon(name, r.x + (r.w - s) / 2, r.y + (r.h - s) / 2, cell, c);
    }

    pub fn image(&mut self, img: &Img, x: i32, y: i32, k: f32) {
        let k = k * self.opacity;
        for j in 0..img.h {
            for i in 0..img.w {
                let p = img.px[(j * img.w + i) as usize];
                if p >> 24 == 0 {
                    continue;
                }
                let p = if k >= 1.0 { p } else { scale_premul(p, k) };
                self.put(x + i, y + j, p);
            }
        }
    }

    /// непрозрачная картинка 1:1 — построчным копированием (замороженный экран)
    pub fn blit(&mut self, img: &Img, x: i32, y: i32) {
        let x = x + self.ox;
        let y = y + self.oy;
        let c = self.clip.inter(&R::new(x, y, img.w, img.h));
        if c.empty() {
            return;
        }
        let w = self.w as usize;
        for row in c.y..c.bottom() {
            let sy = (row - y) as usize;
            let sx = (c.x - x) as usize;
            let src = &img.px[sy * img.w as usize + sx..sy * img.w as usize + sx + c.w as usize];
            let d = row as usize * w + c.x as usize;
            self.buf[d..d + c.w as usize].copy_from_slice(src);
        }
    }

    /// картинка, растянутая без сглаживания в прямоугольник (обои, обложки)
    pub fn image_scaled(&mut self, img: &Img, r: R, k: f32) {
        if img.w == 0 || img.h == 0 || r.empty() {
            return;
        }
        let k = k * self.opacity;
        for j in 0..r.h {
            let sy = (j as i64 * img.h as i64 / r.h as i64) as i32;
            for i in 0..r.w {
                let sx = (i as i64 * img.w as i64 / r.w as i64) as i32;
                let p = img.px[(sy * img.w + sx) as usize];
                if p >> 24 == 0 {
                    continue;
                }
                let p = if k >= 1.0 { p } else { scale_premul(p, k) };
                self.put(r.x + i, r.y + j, p);
            }
        }
    }

    // ── текст ───────────────────────────────────────────────────────────────
    /// строка от левого края; y — верх строки высотой face.line(). Возвращает ширину.
    pub fn text(&mut self, fonts: &mut Fonts, face: Face, x: i32, y: i32, s: &str, c: C) -> i32 {
        let col = self.eff(c);
        let mut pen = x;
        let base = y + fonts.ascent(face);
        for ch in s.chars() {
            if ch == '\n' {
                continue;
            }
            let g = fonts.glyph(face, ch);
            if let Some(g) = g {
                let gx = pen + g.left;
                let gy = base + g.top;
                for j in 0..g.h {
                    for i in 0..g.w {
                        let cov = g.cov[(j * g.w + i) as usize];
                        if cov == 0 {
                            continue;
                        }
                        let p = if cov == 255 { col } else { scale_premul(col, cov as f32 / 255.0) };
                        self.put(gx + i, gy + j, p);
                    }
                }
                pen += g.adv;
            }
        }
        pen - x
    }

    /// текст, отцентрированный по вертикали в r; обрезается многоточием по ширине r
    pub fn text_in(&mut self, fonts: &mut Fonts, face: Face, r: R, s: &str, c: C) -> i32 {
        let y = r.y + (r.h - fonts.line(face)) / 2;
        if fonts.width(face, s) <= r.w {
            self.text(fonts, face, r.x, y, s, c)
        } else {
            let e = fonts.elide(face, s, r.w);
            self.text(fonts, face, r.x, y, &e, c)
        }
    }
    pub fn text_center(&mut self, fonts: &mut Fonts, face: Face, r: R, s: &str, c: C) -> i32 {
        let w = fonts.width(face, s).min(r.w);
        self.text_in(fonts, face, R::new(r.x + (r.w - w) / 2, r.y, w, r.h), s, c)
    }
    pub fn text_right(&mut self, fonts: &mut Fonts, face: Face, r: R, s: &str, c: C) -> i32 {
        let w = fonts.width(face, s).min(r.w);
        self.text_in(fonts, face, R::new(r.right() - w, r.y, w, r.h), s, c)
    }
    /// перенос по словам; возвращает высоту
    pub fn text_wrap(&mut self, fonts: &mut Fonts, face: Face, r: R, s: &str, c: C, max_lines: usize) -> i32 {
        let lines = fonts.wrap(face, s, r.w, max_lines);
        let lh = fonts.line(face) + 2;
        for (i, l) in lines.iter().enumerate() {
            self.text(fonts, face, r.x, r.y + i as i32 * lh, l, c);
        }
        lines.len() as i32 * lh
    }

    // ── составные элементы ───────────────────────────────────────────────
    /// шапка окна: матовая полоса и линия снизу
    pub fn header(&mut self, t: &Theme, r: R) {
        self.blur_rect(r, 0);
        self.fill(r, t.glass_panel());
        self.fill(R::new(r.x, r.bottom() - 1, r.w, 1), t.border);
    }

    /// сегментная шкала с блеском сверху; пустые клетки видны
    pub fn gauge(&mut self, r: R, segments: i32, value: f32, c: C, empty: C, gap: i32) {
        let cw = r.w / segments.max(1);
        let v = value.clamp(0.0, 1.0);
        for i in 0..segments {
            let x = r.x + i * cw;
            let cell = R::new(x, r.y, cw - gap, r.h);
            self.fill(cell, empty);
            let amount = (v * segments as f32 - i as f32).clamp(0.0, 1.0);
            let fw = (cell.w as f32 * amount).round() as i32;
            if fw > 0 {
                self.fill(R::new(x, r.y, fw, r.h), c);
                self.fill(R::new(x, r.y, fw, 1), c.lerp(C(0xffffffff), 0.35));
            }
        }
    }

    /// пунктирный разделитель бара
    pub fn sep(&mut self, x: i32, cy: i32, c: C) {
        for i in 0..4 {
            self.fill(R::new(x, cy - 8 + i * 4, 2, 2), c);
        }
    }
}

#[inline]
pub fn scale_premul(p: u32, k: f32) -> u32 {
    let m = |s: u32| ((((p >> s) & 255) as f32 * k).round() as u32).min(255) << s;
    m(24) | m(16) | m(8) | m(0)
}

#[derive(Clone, Copy)]
pub struct BoxStyle {
    pub fill: C,
    pub outline: C,
    pub notch: i32,
    pub stroke: i32,
    pub shadow: i32,
    pub bevel: bool,
    pub pressed: bool,
}

impl BoxStyle {
    pub fn new(fill: C, outline: C) -> BoxStyle {
        BoxStyle { fill, outline, notch: 2, stroke: 1, shadow: 0, bevel: false, pressed: false }
    }
    pub fn shadow(mut self, s: i32) -> Self {
        self.shadow = s;
        self
    }
    pub fn bevel(mut self, b: bool) -> Self {
        self.bevel = b;
        self
    }
    pub fn pressed(mut self, b: bool) -> Self {
        self.pressed = b;
        self
    }
}

/// иконки 7×7 (px.rs Icon)
pub fn icon7(name: &str) -> Option<&'static [&'static str; 7]> {
    Some(match name {
        "keys" => &[".......", "#######", "#.#.#.#", "#######", "##...##", "#######", "......."],
        "copy" => &["####...", "#..#...", "#.####.", "#.#..#.", "###..#.", "..#..#.", "..####."],
        "send" => &["#......", "###....", "#####..", "#######", "#####..", "###....", "#......"],
        "stop" => &[".......", ".#####.", ".#####.", ".#####.", ".#####.", ".#####.", "......."],
        "plus" => &["...#...", "...#...", "...#...", "#######", "...#...", "...#...", "...#..."],
        "gear" => &["#.###.#", ".#####.", "##...##", "##...##", "##...##", ".#####.", "#.###.#"],
        "close" => &["#.....#", ".#...#.", "..#.#..", "...#...", "..#.#..", ".#...#.", "#.....#"],
        "max" => &["#######", "#######", "#.....#", "#.....#", "#.....#", "#.....#", "#######"],
        "restore" => &["..#####", "..#...#", "#####.#", "#####.#", "#...###", "#...#..", "#####.."],
        "min" => &[".......", ".......", ".......", ".......", ".......", "#######", "#######"],
        "search" => &[".###...", "#...#..", "#...#..", "#...#..", ".####..", "....##.", ".....##"],
        "down" => &["...#...", "...#...", "...#...", "#..#..#", ".#.#.#.", "..###..", "...#..."],
        "up" => &["...#...", "..###..", ".#.#.#.", "#..#..#", "...#...", "...#...", "...#..."],
        "left" => &["...#...", "..#....", ".#.....", "#######", ".#.....", "..#....", "...#..."],
        "right" => &["...#...", "....#..", ".....#.", "#######", ".....#.", "....#..", "...#..."],
        "check" => &["......#", ".....##", "#...##.", "##.##..", ".###...", "..#....", "......."],
        "folder" => &["###....", "#..####", "#.....#", "#.....#", "#.....#", "#######", "......."],
        "trash" => &["..###..", "#######", ".#.#.#.", ".#.#.#.", ".#.#.#.", ".#.#.#.", "..###.."],
        "pin" => &["..###..", ".#####.", ".#####.", "..###..", "...#...", "...#...", "...#..."],
        "drop" => &["...#...", "..###..", ".#####.", ".##.##.", "##.####", "#######", ".#####."],
        "image" => &["#######", "#.....#", "#.#...#", "#...#.#", "#..####", "#.#####", "#######"],
        "power" => &["...#...", ".#.#.#.", "#..#..#", "#.....#", "#.....#", ".#...#.", "..###.."],
        "user" => &["..###..", "..###..", "..###..", ".......", ".#####.", "#######", "#######"],
        "bolt" => &["....##.", "...##..", "..##...", ".#####.", "...##..", "..##...", ".##...."],
        "leaf" => &["....###", "..#####", ".###..#", ".##..##", "#.#.##.", "#..##..", "#......"],
        "gauge" => &[".......", "..###..", ".#...#.", "#..#..#", "#.#...#", "#.....#", "#######"],
        "lock" => &["..###..", ".#...#.", ".#...#.", "#######", "###.###", "###.###", "#######"],
        "speaker" => &["...#...", "..##.#.", "####..#", "####..#", "####..#", "..##.#.", "...#..."],
        "mute" => &["...#...", "..##...", "####.#.", "####..#", "####.#.", "..##...", "...#..."],
        "mic" => &["..###..", "..###..", "..###..", "#.###.#", ".#####.", "...#...", "..###.."],
        "float" => &["####...", "#..#...", "#.#####", "###...#", "..#...#", "..#...#", "..#####"],
        "tile" => &["###.###", "#.#.#.#", "#.#.#.#", "#.#.#.#", "#.#.#.#", "#.#.#.#", "###.###"],
        "moon" => &["..###..", ".##....", "##.....", "##.....", "##.....", ".##...#", "..####."],
        "reboot" => &["..####.", ".#...##", "#...###", "#......", "#.....#", ".#...#.", "..###.."],
        "exit" => &["####...", "#......", "#...#..", "#.#####", "#...#..", "#......", "####..."],
        "top" => &["#######", ".......", "...#...", "..###..", ".#.#.#.", "...#...", "...#..."],
        "shade" => &["#######", "#######", ".......", ".......", ".......", ".......", "......."],
        "sticky" => &["..####.", "..#..#.", "####.#.", "#..###.", "#..#...", "####...", "......."],
        "wifi" => &[".#####.", "#.....#", "..###..", ".#...#.", "...#...", "..#.#..", "...#..."],
        "net" => &["...#...", "..###..", ".#.#.#.", "...#...", "...#...", "...#...", "#######"],
        "bt" => &["...##..", "#..#.#.", ".#.#.#.", "..##...", ".#.#.#.", "#..#.#.", "...##.."],
        "play" => &[".#.....", ".##....", ".###...", ".####..", ".###...", ".##....", ".#....."],
        "pause" => &[".##.##.", ".##.##.", ".##.##.", ".##.##.", ".##.##.", ".##.##.", ".##.##."],
        "next" => &["#...#.#", "##..#.#", "###.#.#", "####..#", "###.#.#", "##..#.#", "#...#.#"],
        "prev" => &["#.#...#", "#.#..##", "#.#.###", "#..####", "#.#.###", "#.#..##", "#.#...#"],
        "bell" => &["...#...", "..###..", ".#####.", ".#####.", ".#####.", "#######", "...#..."],
        "bellx" => &["#..#...", ".####..", ".##.##.", ".###.#.", ".####.#", "#######", "...#..."],
        "cpu" => &[".#.#.#.", "#######", ".#...#.", "##.#.##", ".#...#.", "#######", ".#.#.#."],
        "temp" => &["..##...", "..#.#..", "..##...", "..#.#..", ".####..", "######.", ".####.."],
        "ram" => &["#######", "#.#.#.#", "#.#.#.#", "#######", ".#.#.#.", ".#.#.#.", "......."],
        "clock" => &["..###..", ".#.#.#.", "#..#..#", "#..##.#", "#.....#", ".#...#.", "..###.."],
        "cal" => &["#.#.#.#", "#######", "#.....#", "#.#.#.#", "#.....#", "#.#.#.#", "#######"],
        "grid" => &["##.##..", "##.##..", ".......", "##.##..", "##.##..", ".......", "......."],
        "drag" => &["#.#.#.#", ".......", "#.#.#.#", ".......", "#.#.#.#", ".......", "#.#.#.#"],
        "eye" => &[".......", "..###..", ".#...#.", "#..#..#", ".#...#.", "..###..", "......."],
        "music" => &["..####.", "..#..#.", "..#..#.", "..#..#.", "###.###", "###.###", "......."],
        "about" => &[".#####.", "#.....#", "#.#.#.#", "#.....#", ".#.#.#.", "..###..", "......."],
        "chart" => &["......#", ".....##", "...#.##", "..##.##", "#.##.##", "#.##.##", "#######"],
        "menu" => &["#######", ".......", "#######", ".......", "#######", ".......", "......."],
        "dots" => &[".......", ".......", ".......", "#..#..#", ".......", ".......", "......."],
        // скриншотер
        "camera" => &[".......", "..##...", "#######", "#..#..#", "#.#.#.#", "#..#..#", "#######"],
        "cursor" => &["#......", "##.....", "###....", "####...", "#####..", "##.....", "#.#...."],
        "pen" => &[".....##", "....###", "...###.", "..###..", ".###...", "##.....", "#......"],
        "rect" => &[".......", "#######", "#.....#", "#.....#", "#.....#", "#######", "......."],
        "arrow" => &["...####", ".....##", "....#.#", "...#..#", "..#....", ".#.....", "#......"],
        "marker" => &[".......", ".......", "#######", "#######", "#######", ".......", "......."],
        "mosaic" => &["##..##.", "##..##.", "..##..#", "..##..#", "##..##.", "##..##.", "..##..#"],
        "count" => &[".#####.", "##.#.##", "#..#..#", "##.#.##", "#..#..#", "##...##", ".#####."],
        "text" => &["#######", "#..#..#", "...#...", "...#...", "...#...", "...#...", "..###.."],
        "window" => &["#######", "#######", "#.....#", "#.....#", "#.....#", "#.....#", "#######"],
        "screen" => &["#######", "#.....#", "#.....#", "#.....#", "#######", "...#...", "..###.."],
        "timer" => &["..###..", "...#...", ".#####.", "#..#..#", "#..##.#", "#.....#", ".#####."],
        // просмотрщик
        "rotl" => &[".####..", "#....#.", "#.....#", "###...#", ".#....#", "......#", "..####."],
        "rotr" => &["..####.", ".#....#", "#.....#", "#...###", "#....#.", "#......", ".####.."],
        "fliph" => &["...#...", "#..#..#", "##.#.##", "####.##", "##.#.##", "#..#..#", "...#..."],
        "fit" => &["##...##", "#.....#", "..###..", "..###..", "..###..", "#.....#", "##...##"],
        "one" => &["...#...", "..##...", "...#...", "...#...", "...#...", "...#...", "..###.."],
        "full" => &["###.###", "#.....#", "#.....#", ".......", "#.....#", "#.....#", "###.###"],
        "zoomin" => &[".###...", "#.#.#..", "#####..", "#.#.#..", ".####..", "....##.", ".....##"],
        "zoomout" => &[".###...", "#...#..", "#####..", "#...#..", ".####..", "....##.", ".....##"],
        "crop" => &[".#.....", ".######", ".#...#.", ".#...#.", ".#...#.", "######.", ".....#."],
        "save" => &["######.", "#....##", "#.....#", "#######", "#.###.#", "#.###.#", "#######"],
        "slides" => &["#######", "#.....#", "#.#...#", "#.##..#", "#.#...#", "#.....#", "#######"],
        "redo" => &["...#...", "....#..", "#######", "....#..", "...#...", ".......", "......."],
        _ => return None,
    })
}

/// Размер буфера в физических пикселях для логического w×h при масштабе вывода `scale`
/// (1.0, 1.25, 2.0 — из wp_fractional_scale, preferred_scale / 120).
pub fn phys(w: i32, h: i32, scale: f64) -> (i32, i32) {
    (((w as f64) * scale).round().max(1.0) as i32, ((h as f64) * scale).round().max(1.0) as i32)
}

/// Пиксельно-точное увеличение (ближайший сосед) логического холста src (w×h) в буфер dst
/// (pw×ph): на 4K с масштабом 2 каждый пиксель шелла — ровно 2×2, без мыла от
/// билинейного растяжения композитором. Дробный масштаб даёт чуть неровные клетки, но чётко.
pub fn upscale(src: &[u32], w: i32, h: i32, dst: &mut [u32], pw: i32, ph: i32) {
    let (w, h, pw, ph) = (w as usize, h as usize, pw as usize, ph as usize);
    if w == 0 || h == 0 || src.len() < w * h || dst.len() < pw * ph {
        return;
    }
    if pw % w == 0 && ph % h == 0 && pw / w == ph / h {
        // целый масштаб: строку растянуть один раз, потом копировать
        let k = pw / w;
        for y in 0..h {
            let row = &src[y * w..y * w + w];
            let first = y * k * pw;
            {
                let out = &mut dst[first..first + pw];
                for (x, &p) in row.iter().enumerate() {
                    out[x * k..x * k + k].fill(p);
                }
            }
            for r in 1..k {
                dst.copy_within(first..first + pw, first + r * pw);
            }
        }
        return;
    }
    let xs: Vec<usize> = (0..pw).map(|x| (x * w / pw).min(w - 1)).collect();
    let mut last = usize::MAX;
    for y in 0..ph {
        let sy = (y * h / ph).min(h - 1);
        let out = y * pw;
        if sy == last {
            dst.copy_within(out - pw..out, out);
            continue;
        }
        last = sy;
        let row = &src[sy * w..sy * w + w];
        for (x, &sx) in xs.iter().enumerate() {
            dst[out + x] = row[sx];
        }
    }
}
