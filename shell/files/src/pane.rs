// Панель с файлами (одна или две на вкладку, F3): сетка значков или список с колонками, рамка
// выделения, миниатюры, поле переименования на месте, полоса прокрутки. Остальное окно — draw.rs.

use crate::act::{Act, Ui};
use crate::draw::*;
use crate::fm::{item_rect, Fm, Mode, Thumb};
use crate::font::PX;
use crate::fsx::{self, Sort};
use crate::gfx::{BoxStyle, C, R};
use crate::mime::{self, Kind};
use crate::picons::{self, Special};
use crate::ui::Cur;
use std::time::Instant;

pub(crate) fn pane(ui: &mut Ui, fm: &Fm, pi: usize, r: R) {
    let t = ui.t;
    let v = &fm.tabv().panes[pi];
    let split = fm.tabv().panes.len() > 1;
    let active = fm.tabv().active == pi;
    let g = geo(fm, r, ui.f);
    v.geo.set(g);
    ui.zone(r, Act::Bg(pi), Cur::Default);
    if split && pi > 0 {
        ui.cv.fill(R::new(r.x, r.y, 1, r.h), t.border);
    }
    if split && active {
        ui.cv.fill(R::new(r.x + 1, r.y, r.w - 1, 2), t.red);
    }
    if !split || active {
    } else {
        ui.cv.fill(r, t.bg.alpha(0.18));
    }
    ui.cv.push_clip(r);
    let mode = fm.mode();
    let scroll = v.scroll.round() as i32;
    if mode == Mode::List {
        list_head(ui, fm, R::new(r.x, r.y, r.w, HEAD_H));
    }
    let body = if mode == Mode::List { R::new(r.x, r.y + HEAD_H, r.w, r.h - HEAD_H) } else { r };
    ui.cv.push_clip(body);
    let now = Instant::now();
    let cut: Vec<std::path::PathBuf> = match &fm.clip {
        Some((p, true)) => p.clone(),
        _ => vec![],
    };
    // видимый диапазон
    let (first, last) = match mode {
        Mode::Grid => {
            let r0 = ((scroll - g.top) / g.ch).max(0);
            let r1 = (scroll + r.h) / g.ch + 1;
            ((r0 * g.cols) as usize, ((r1 + 1) * g.cols) as usize)
        }
        Mode::List => (((scroll - g.top) / g.ch).max(0) as usize, ((scroll + r.h) / g.ch + 2) as usize),
    };
    let reveal_ms = fm.look.dur(150.0);
    for i in first..last.min(v.items.len()) {
        let e = &v.all[v.items[i]];
        let mut ir = item_rect(mode, &g, i);
        ir.y -= scroll;
        if ir.bottom() < body.y || ir.y > body.bottom() {
            continue;
        }
        let sel = v.sel.contains(&e.path);
        let is_cur = v.cur == Some(i) && active;
        let hov = ui.hover(ir) && fm.menu.is_none();
        let renaming = v.rename.as_ref().filter(|rn| rn.path == e.path);
        let is_cut = cut.contains(&e.path);
        let drop = matches!(fm.drop_hint, Some((p, Some(k))) if p == pi && k == i);
        match mode {
            Mode::Grid => grid_item(ui, fm, e, ir, sel, is_cur, hov, is_cut, drop, renaming),
            Mode::List => list_item(ui, fm, e, ir, i, sel, is_cur, hov, is_cut, drop, renaming),
        }
        // только что появившееся — мигает рамкой три раза
        if let Some(t0) = v.fresh.get(&e.path) {
            let ms = now.duration_since(*t0).as_millis() as i32;
            if ms < 1100 {
                if (ms / 180) % 2 == 0 {
                    ui.cv.outline(ir.inset(1), t.ember, 2);
                }
                ui.anim = true;
            }
        }
        ui.zone(ir.inter(&body), Act::Item(pi, i), Cur::Pointer);
        // появление: клетки проявляются волной сверху вниз
        if reveal_ms > 0.0 {
            let delay = (i.saturating_sub(first)).min(40) as f32 * 9.0;
            let el = v.born.elapsed().as_secs_f32() * 1000.0 - delay;
            let k = (el / reveal_ms).clamp(0.0, 1.0);
            if k < 1.0 {
                ui.cv.dissolve_rect(ir.inter(&body), k, 2);
                ui.anim = true;
            }
        }
    }
    // резиновая рамка
    if let Some((x0, y0, x1, y1)) = v.band {
        let br = R::new(x0.min(x1) as i32, y0.min(y1) as i32 - scroll, (x1 - x0).abs() as i32, (y1 - y0).abs() as i32);
        ui.cv.fill(br, t.selection.alpha(0.28));
        let ph = (fm.born.elapsed().as_millis() / 60) as i32;
        ui.cv.ants(br, t.ember, 3, ph);
        ui.anim = true;
    }
    ui.cv.pop_clip();
    // пусто, ошибка, загрузка
    let msg: Option<(&str, String)> = if let Some(e) = &v.err {
        Some(("close", e.clone()))
    } else if v.loading && v.all.is_empty() {
        let dots = ".".repeat(1 + (fm.born.elapsed().as_millis() / 250 % 3) as usize);
        ui.anim = true;
        Some(("clock", format!("{}{dots}", crate::i18n::tr("читаю"))))
    } else if v.items.is_empty() {
        Some(if v.trash {
            ("trash", crate::i18n::tr("корзина пуста").to_string())
        } else if !v.filter.is_empty() {
            ("search", crate::i18n::tr("ничего не нашлось").to_string())
        } else {
            ("folder", crate::i18n::tr("папка пуста").to_string())
        })
    } else {
        None
    };
    if let Some((icon, text)) = msg {
        let cy = body.y + body.h / 2 - 30;
        ui.cv.icon(icon, body.x + body.w / 2 - 14, cy, 4, t.border.lerp(t.red, 0.4));
        ui.cv.text_center(ui.f, PX, R::new(body.x, cy + 40, body.w, 20), &text, t.text_faint);
    }
    // полоса прокрутки
    let ch = content_h(fm, v, &g);
    if ch > r.h {
        let tr = R::new(r.right() - 7, body.y + 2, 5, body.h - 4);
        let th = ((tr.h as f32 * body.h as f32 / ch as f32) as i32).max(24);
        let ty = tr.y + ((tr.h - th) as f32 * (v.scroll / max_scroll(fm, v).max(1.0))) as i32;
        let hov = ui.hover(R::new(tr.x - 6, tr.y, tr.w + 6, tr.h));
        ui.cv.fill(tr, t.surface.alpha(0.6));
        ui.cv.fill(R::new(tr.x, ty, tr.w, th), if hov { t.red } else { t.dim });
        ui.zone(R::new(tr.x - 6, tr.y, tr.w + 7, tr.h), Act::Bar(pi), Cur::Pointer);
    }
    ui.cv.pop_clip();
}

pub(crate) fn thumb_of(fm: &Fm, e: &fsx::Entry, size: i32) -> Option<std::rc::Rc<crate::gfx::Img>> {
    if !fm.cfg.thumbs || e.dir {
        return None;
    }
    let k = mime::kind(e.mime);
    let video = k == Kind::Video;
    if !(k == Kind::Image && (e.mime == "image/png" || e.mime == "image/jpeg") || video) {
        return None;
    }
    if !video && e.size > fm.cfg.thumb_max_mb as u64 * 1024 * 1024 {
        return None;
    }
    match fm.thumbs.get(&(e.path.clone(), size)) {
        Some(Thumb::Ready(img, used)) => {
            used.set(fm.frame);
            Some(img.clone())
        }
        Some(_) => None,
        None => {
            fm.want.borrow_mut().push(crate::thumbs::Req { path: e.path.clone(), mtime: e.mtime, size, video });
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn grid_item(ui: &mut Ui, fm: &Fm, e: &fsx::Entry, r: R, sel: bool, cur: bool, hov: bool, cut: bool, drop: bool, rn: Option<&crate::fm::Rename>) {
    let t = ui.t;
    let u = fm.cfg.icon_u();
    let icon = 16 * u;
    let cell = r.inset(3);
    if sel || drop {
        ui.cv.pxbox(cell, &BoxStyle::new(t.selection.alpha(0.75), if drop { t.ember } else { t.red }));
    } else if hov {
        ui.cv.pxbox(cell, &BoxStyle::new(t.surface_hi.alpha(0.8), t.border));
    }
    if cur && !sel {
        ui.cv.ants(cell.inset(1), t.red, 2, 0);
    }
    let ix = r.x + (r.w - icon) / 2;
    let iy = r.y + 8;
    let dim = e.hidden || cut || e.broken;
    match thumb_of(fm, e, icon) {
        Some(img) => {
            let (dw, dh) = (img.w, img.h);
            let tx = r.x + (r.w - dw) / 2;
            let ty = iy + icon - dh;
            ui.cv.fill(R::new(tx - 2, ty - 2, dw + 4, dh + 4), t.code_bg);
            ui.cv.image(&img, tx, ty, if dim { 0.55 } else { 1.0 });
            ui.cv.outline(R::new(tx - 2, ty - 2, dw + 4, dh + 4), if sel { t.ember } else { t.border }, 1);
            if mime::kind(e.mime) == Kind::Video {
                ui.cv.icon("play", tx + dw / 2 - 7, ty + dh / 2 - 7, 2, C(0xffffffff).fade(0.9));
            }
        }
        None => {
            let sp = if e.dir { crate::places::special_of(&e.path) } else { Special::None };
            picons::draw(&mut ui.cv, &t, mime::kind(e.mime), sp, ix, iy, u, picons::Look { dim, link: e.link, selected: sel });
        }
    }
    let lr = R::new(r.x + 6, iy + icon + 6, r.w - 12, r.h - icon - 16);
    if let Some(rn) = rn {
        rename_field(ui, R::new(r.x + 2, lr.y - 2, r.w - 4, 24), rn);
        return;
    }
    let lh = ui.f.line(PX) + 1;
    let lines = label_lines(ui.f, PX, &e.name, lr.w, 2);
    let c = if sel { t.text_hi } else if dim { t.text_dim } else { t.text };
    for (k, l) in lines.iter().enumerate() {
        raw_center(ui, PX, R::new(lr.x, lr.y + k as i32 * lh, lr.w, lh), l, c);
    }
}

pub(crate) fn rename_field(ui: &mut Ui, r: R, rn: &crate::fm::Rename) {
    let t = ui.t;
    ui.cv.pxbox(r, &BoxStyle::new(t.code_bg, t.red).shadow(2));
    let inner = R::new(r.x + 6, r.y, r.w - 12, r.h);
    ui.cv.push_clip(inner);
    let text = &rn.f.text;
    let before: String = text.chars().take(rn.f.cur).collect();
    let bw = ui.f.width(PX, &before);
    let shift = (bw - (inner.w - 8)).max(0);
    let y = inner.y + (inner.h - ui.f.line(PX)) / 2;
    if rn.fresh {
        // имя без расширения — «выделено»: набор заменит его
        let stem: String = text.chars().take(rn.f.cur).collect();
        let sw = ui.f.width(PX, &stem);
        ui.cv.fill(R::new(inner.x - shift, y - 1, sw, ui.f.line(PX) + 2), t.blood);
    }
    ui.cv.text(ui.f, PX, inner.x - shift, y, text, t.text);
    if ui.blink {
        ui.cv.fill(R::new(inner.x - shift + bw, y, 2, ui.f.line(PX)), t.ember);
    }
    ui.cv.pop_clip();
    ui.zone(r, Act::Block, Cur::Text);
}

pub(crate) fn list_head(ui: &mut Ui, fm: &Fm, r: R) {
    let t = ui.t;
    ui.cv.fill(r, t.panel.alpha(0.7));
    ui.cv.fill(R::new(r.x, r.bottom() - 1, r.w, 1), t.border);
    let cols = list_cols(r);
    let sort = Sort::from_id(&fm.cfg.sort);
    for (label, x, w, s, right) in [("имя", cols.name.0, cols.name.1, Sort::Name, false), ("размер", cols.size.0, cols.size.1, Sort::Size, true), ("изменён", cols.time.0, cols.time.1, Sort::Time, false), ("тип", cols.kind.0, cols.kind.1, Sort::Kind, false)] {
        if w <= 0 {
            continue;
        }
        let cr = R::new(x, r.y, w, r.h);
        let hov = ui.hover(cr);
        let c = if sort == s { t.red } else if hov { t.text } else { t.text_dim };
        let lw = if right { ui.text_right(PX, R::new(cr.x, cr.y, cr.w - 22, cr.h), label, c) } else { ui.text_in(PX, R::new(cr.x + 4, cr.y, cr.w - 20, cr.h), label, c) };
        if sort == s {
            let ax = if right { cr.right() - 18 } else { cr.x + 10 + lw };
            ui.cv.icon(if fm.cfg.desc { "down" } else { "up" }, ax, cr.cy() - 4, 1, t.red);
        }
        ui.zone(cr, Act::ColSort(s), Cur::Pointer);
    }
}

pub struct Cols {
    pub name: (i32, i32),
    pub size: (i32, i32),
    pub time: (i32, i32),
    pub kind: (i32, i32),
}

pub fn list_cols(r: R) -> Cols {
    let w = r.w - 12;
    let kind_w = if w > 760 { 170 } else { 0 };
    let time_w = if w > 520 { 150 } else { 0 };
    let size_w = if w > 380 { 90 } else { 0 };
    let name_w = w - kind_w - time_w - size_w - 8;
    let x = r.x + 6;
    Cols { name: (x, name_w), size: (x + name_w, size_w), time: (x + name_w + size_w + 16, time_w), kind: (x + name_w + size_w + time_w + 16, kind_w) }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn list_item(ui: &mut Ui, _fm: &Fm, e: &fsx::Entry, r: R, i: usize, sel: bool, cur: bool, hov: bool, cut: bool, drop: bool, rn: Option<&crate::fm::Rename>) {
    let t = ui.t;
    let row = R::new(r.x + 4, r.y + 1, r.w - 14, r.h - 2);
    if sel || drop {
        ui.cv.pxbox(row, &BoxStyle::new(t.selection.alpha(0.8), if drop { t.ember } else { C(0) }));
    } else if hov {
        ui.cv.pxbox(row, &BoxStyle::new(t.surface_hi.alpha(0.8), C(0)));
    } else if i % 2 == 1 {
        ui.cv.fill(row, t.surface.alpha(0.25));
    }
    if cur {
        ui.cv.fill(R::new(row.x, row.y + 2, 2, row.h - 4), t.ember);
    }
    let cols = list_cols(r);
    let dim = e.hidden || cut || e.broken;
    let sp = if e.dir { crate::places::special_of(&e.path) } else { Special::None };
    picons::draw(&mut ui.cv, &t, mime::kind(e.mime), sp, cols.name.0 + 4, r.y + (r.h - 16) / 2, 1, picons::Look { dim, link: e.link, selected: sel });
    let c = if sel { t.text_hi } else if dim { t.text_dim } else { t.text };
    let nr = R::new(cols.name.0 + 28, r.y, cols.name.1 - 32, r.h);
    if let Some(rn) = rn {
        rename_field(ui, R::new(nr.x - 6, r.y, nr.w + 6, r.h), rn);
    } else {
        let shown = match (&e.place, &e.orig) {
            (Some(p), _) => format!("{}  ", e.name).to_string() + &fsx::pretty(p),
            _ => e.name.clone(),
        };
        let nw = raw(ui, PX, nr, &e.name, c);
        if shown.len() > e.name.len() {
            raw(ui, PX, R::new(nr.x + nw + 10, nr.y, nr.w - nw - 10, nr.h), &fsx::pretty(e.place.as_ref().unwrap()), t.text_faint);
        } else if let Some(o) = &e.orig {
            let dir = o.parent().map(fsx::pretty).unwrap_or_default();
            raw(ui, PX, R::new(nr.x + nw + 10, nr.y, nr.w - nw - 10, nr.h), &dir, t.text_faint);
        }
    }
    let dc = if sel { t.text } else { t.text_dim };
    if cols.size.1 > 0 {
        let s = if e.dir { String::new() } else { fsx::human(e.size) };
        raw(ui, PX, R::new(cols.size.0, r.y, cols.size.1 - 4, r.h), "", dc);
        ui.cv.text_right(ui.f, PX, R::new(cols.size.0, r.y, cols.size.1 - 4, r.h), &s, dc);
    }
    if cols.time.1 > 0 {
        let when = fsx::when(if e.deleted > 0 { e.deleted } else { e.mtime });
        raw(ui, PX, R::new(cols.time.0, r.y, cols.time.1, r.h), &when, dc);
    }
    if cols.kind.1 > 0 {
        raw(ui, PX, R::new(cols.kind.0, r.y, cols.kind.1, r.h), &mime::label(e.mime, &e.name), t.text_faint);
    }
}
