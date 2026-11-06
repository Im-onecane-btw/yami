// Лицо экрана блокировки и экрана входа: обои под вуалью со строками развёртки, большие часы с
// тенью, дата, 闇 в углу. Одно на двоих (шелл lock.rs и yami-greet), чтобы вход и блокировка
// были одним экраном.

use crate::font;
use crate::gfx::{Img, C, R};
use crate::time::{local_now, RU_DAY_FULL, RU_MON};
use crate::ui::{Action, Ui};

/// картинка на весь экран с обрезкой краёв (без сглаживания: клетки остаются клетками)
pub fn cover<A: Action>(ui: &mut Ui<A>, img: &Img, w: i32, h: i32) {
    let k = (w as f32 / img.w.max(1) as f32).max(h as f32 / img.h.max(1) as f32);
    let (dw, dh) = ((img.w as f32 * k).ceil() as i32, (img.h as f32 * k).ceil() as i32);
    ui.cv.image_scaled(img, R::new((w - dw) / 2, (h - dh) / 2, dw, dh), 1.0);
}

/// обои, вуаль к фону темы (в тёмной — затемнить, в светлой — высветлить) и строки развёртки
pub fn backdrop<A: Action>(ui: &mut Ui<A>, wall: Option<&Img>, w: i32, h: i32) {
    let t = ui.t;
    ui.cv.clear(t.bg);
    if let Some(img) = wall {
        cover(ui, img, w, h);
    }
    let (veil, line) = if t.light { (t.bg.alpha(0.62), t.text.alpha(0.07)) } else { (C(0xff000000).alpha(0.55), C(0xff000000).alpha(0.18)) };
    ui.cv.fill(R::new(0, 0, w, h), veil);
    for y in (0..h).step_by(4) {
        ui.cv.fill(R::new(0, y, w, 1), line);
    }
}

/// большие часы и дата по центру чуть выше середины; возвращает низ даты
pub fn clock<A: Action>(ui: &mut Ui<A>, w: i32, h: i32) -> i32 {
    let t = ui.t;
    let tm = local_now();
    let big = font::px(128);
    let s = format!("{:02}:{:02}", tm.hour, tm.min);
    let cw = ui.w(big, &s);
    let cy = h / 2 - 150;
    // тень-сдвиг на 4 px, как у панелей
    ui.text(big, (w - cw) / 2 + 4, cy + 4, &s, t.blood.fade(0.8));
    ui.text(big, (w - cw) / 2, cy, &s, t.text);
    let date = format!("{}, {} {}", (ui.tr)(RU_DAY_FULL[tm.wday as usize % 7]), tm.day, (ui.tr)(RU_MON[(tm.mon as usize).clamp(1, 12) - 1]));
    let dy = cy + ui.f.line(big) + 6;
    ui.text_center(font::PX24, R::new(0, dy, w, 30), &date, t.text_dim);
    dy + 30
}

/// 闇 в правом нижнем углу
pub fn kanji<A: Action>(ui: &mut Ui<A>, w: i32, h: i32) {
    let t = ui.t;
    ui.text(font::jp(32), w - 72, h - 64, "闇", t.red.fade(0.5));
}
