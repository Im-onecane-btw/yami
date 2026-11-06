// Отрисовка окна файлов. Каждый кадр — заново, как в шелле: зоны щелчков записываются по ходу.
// Раскладка: панель инструментов (назад/вперёд/вверх, путь крошками, поиск, вид), вкладки,
// места слева, одна или две панели с файлами, сведения справа, строка состояния.

use crate::act::{Act, Ui};
use crate::fm::{item_rect, Fm, Geo, Menu, Mode, Thumb, View};
use crate::font::{Face, Fonts, PX};
use crate::fsx::{self, Sort};
use crate::gfx::{BoxStyle, C, R};
use crate::mime::{self, Kind};
use crate::picons::{self, Special};
use crate::places::DevKind;
use crate::ui::{Cur, Tone};
use std::time::Instant;

pub const TOOL_H: i32 = 44;
pub const TAB_H: i32 = 28;
pub const STATUS_H: i32 = 26;
pub const SIDE_W: i32 = 200;
pub const INFO_W: i32 = 270;
pub const PICK_H: i32 = 58;
pub const ROW_H: i32 = 24;
pub const HEAD_H: i32 = 24;
pub const MENU_W: i32 = 236;
pub const MROW: i32 = 24;

pub struct Lay {
    pub tool: R,
    pub tabs: Option<R>,
    pub side: Option<R>,
    pub panes: Vec<R>,
    pub info: Option<R>,
    pub status: R,
    pub pick: Option<R>,
}

pub fn lay(fm: &Fm, w: i32, h: i32) -> Lay {
    let tool = R::new(0, 0, w, TOOL_H);
    let mut y = TOOL_H;
    let tabs = if fm.tabs.len() > 1 {
        let r = R::new(0, y, w, TAB_H);
        y += TAB_H;
        Some(r)
    } else {
        None
    };
    let pick = fm.pick.as_ref().map(|_| R::new(0, h - PICK_H, w, PICK_H));
    let bottom = h - STATUS_H - pick.map(|p| p.h).unwrap_or(0);
    let status = R::new(0, bottom, w, STATUS_H);
    let side_on = fm.cfg.sidebar && w > 560;
    let side = side_on.then(|| R::new(0, y, SIDE_W, bottom - y));
    let info_on = fm.cfg.info && w > 900;
    let info = info_on.then(|| R::new(w - INFO_W, y, INFO_W, bottom - y));
    let x0 = side.map(|s| s.right()).unwrap_or(0);
    let x1 = info.map(|i| i.x).unwrap_or(w);
    let n = fm.tabv().panes.len() as i32;
    let pw = (x1 - x0) / n.max(1);
    let panes = (0..n).map(|i| R::new(x0 + i * pw, y, if i == n - 1 { x1 - x0 - i * pw } else { pw }, bottom - y)).collect();
    Lay { tool, tabs, side, panes, info, status, pick }
}

pub(crate) fn ease(born: Instant, ms: f32, fm: &Fm) -> f32 {
    let d = fm.look.dur(ms);
    if d <= 0.0 {
        return 1.0;
    }
    let t = (born.elapsed().as_secs_f32() * 1000.0 / d).min(1.0);
    1.0 - (1.0 - t).powi(3)
}

/// текст без перевода (имена файлов, пути)
pub(crate) fn raw(ui: &mut Ui, face: Face, r: R, s: &str, c: C) -> i32 {
    ui.cv.text_in(ui.f, face, r, s, c)
}
pub(crate) fn raw_center(ui: &mut Ui, face: Face, r: R, s: &str, c: C) -> i32 {
    ui.cv.text_center(ui.f, face, r, s, c)
}

/// имя в две строки: перенос по словам, а длинные слова — по буквам; лишнее — многоточием
pub fn label_lines(f: &mut Fonts, face: Face, s: &str, w: i32, max: usize) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut lines = vec![];
    let mut start = 0;
    while start < chars.len() && lines.len() < max {
        let last = lines.len() + 1 == max;
        let rest: String = chars[start..].iter().collect();
        if f.width(face, &rest) <= w {
            lines.push(rest);
            break;
        }
        if last {
            lines.push(f.elide(face, &rest, w));
            break;
        }
        // самый длинный кусок, что влезает
        let mut end = start + 1;
        while end < chars.len() && f.width(face, &chars[start..=end].iter().collect::<String>()) <= w {
            end += 1;
        }
        // перенос после пробела, точки, дефиса, подчёркивания — если он не слишком рано
        let brk = (start + 1..end).rev().find(|&i| matches!(chars[i - 1], ' ' | '-' | '_' | '.')).filter(|&i| i > start + (end - start) / 2).unwrap_or(end);
        lines.push(chars[start..brk].iter().collect::<String>().trim_end().to_string());
        start = brk;
        while start < chars.len() && chars[start] == ' ' {
            start += 1;
        }
    }
    lines
}

pub fn geo(fm: &Fm, area: R, f: &mut Fonts) -> Geo {
    match fm.mode() {
        Mode::Grid => {
            let u = fm.cfg.icon_u();
            let icon = 16 * u;
            let lh = f.line(PX) + 1;
            let cw0 = (icon + 48).max(96);
            let cols = ((area.w - 16) / cw0).max(1);
            let cw = (area.w - 16) / cols;
            Geo { area, cols, cw, ch: icon + 14 + 2 * lh + 8, top: 8 }
        }
        Mode::List => Geo { area, cols: 1, cw: area.w, ch: ROW_H, top: HEAD_H },
    }
}

pub fn content_h(fm: &Fm, v: &View, g: &Geo) -> i32 {
    let n = v.items.len() as i32;
    match fm.mode() {
        Mode::Grid => g.top + ((n + g.cols - 1) / g.cols.max(1)) * g.ch + 12,
        Mode::List => g.top + n * g.ch + 8,
    }
}

pub fn max_scroll(fm: &Fm, v: &View) -> f32 {
    let g = v.geo.get();
    (content_h(fm, v, &g) - g.area.h).max(0) as f32
}

// ═══ окно ═══════════════════════════════════════════════════════════════════
pub fn draw(ui: &mut Ui, fm: &Fm, w: i32, h: i32) {
    let t = ui.t;
    // фон: стекло темы, но плотнее панелей шелла — файлы должны читаться
    ui.cv.clear(t.bg.alpha(t.glass.max(0.86)));
    if fm.look.blur {
        ui.cv.blur_rect(R::new(0, 0, w, h), 0);
    }
    let l = lay(fm, w, h);
    if let Some(s) = l.side {
        sidebar(ui, fm, s);
    }
    for (i, r) in l.panes.iter().enumerate() {
        pane(ui, fm, i, *r);
    }
    if let Some(r) = l.info {
        info(ui, fm, r);
    }
    toolbar(ui, fm, l.tool);
    if let Some(r) = l.tabs {
        tabs(ui, fm, r);
    }
    status(ui, fm, l.status);
    if let Some(r) = l.pick {
        pick_bar(ui, fm, r);
    }
    toast(ui, fm, w, l.status.y);
    if let Some((d, born)) = &fm.dlg {
        crate::dialogs::dialog(ui, fm, d, *born, w, h);
    }
    if let Some(m) = &fm.menu {
        menu(ui, fm, m, w, h);
    }
    // проявление окна целиком при открытии
    let k = ease(fm.born, 260.0, fm);
    if k < 1.0 {
        ui.cv.dissolve(k, 2);
        ui.anim = true;
    }
}

// ── панель инструментов ─────────────────────────────────────────────────────
pub(crate) fn toolbar(ui: &mut Ui, fm: &Fm, r: R) {
    let t = ui.t;
    ui.cv.fill(r, t.panel.alpha(0.94));
    ui.cv.fill(R::new(r.x, r.bottom() - 1, r.w, 1), t.border);
    ui.zone(r, Act::Drag, Cur::Default);
    let v = fm.v();
    let by = r.y + 7;
    let bs = 30;
    let mut x = r.x + 8;
    if let Some(p) = &fm.pick {
        ui.cv.icon(if p.save { "down" } else { "folder" }, x + 2, r.cy() - 7, 2, t.red);
        x += 22;
        let tw = ui.cv.text_in(ui.f, PX, R::new(x, r.y, 220, r.h), &p.title, t.text);
        x += tw + 14;
    }
    let can_back = !v.back.is_empty() || v.trash || v.search.is_some();
    ui.button(R::new(x, by, bs, bs), "", "left", Tone::Ghost, false, can_back, Act::Back);
    x += bs + 2;
    ui.button(R::new(x, by, bs, bs), "", "right", Tone::Ghost, false, !v.fwd.is_empty(), Act::Fwd);
    x += bs + 2;
    ui.button(R::new(x, by, bs, bs), "", "up", Tone::Ghost, false, v.dir.parent().is_some() || v.trash, Act::Up);
    x += bs + 8;
    // справа налево: меню, сведения, разделение, вид, поиск
    let mut rx = r.right() - 8;
    let rb = |ui: &mut Ui, rx: &mut i32, icon: &str, sel: bool, act: Act| {
        *rx -= bs;
        ui.button(R::new(*rx, by, bs, bs), "", icon, Tone::Ghost, sel, true, act);
        *rx -= 2;
    };
    if fm.pick.is_some() {
        rb(ui, &mut rx, "close", false, Act::PickCancel);
    }
    rb(ui, &mut rx, "menu", fm.menu.as_ref().map(|m| m.title == "≡").unwrap_or(false), Act::MenuBtn);
    if fm.pick.is_none() {
        rb(ui, &mut rx, "about", fm.cfg.info, Act::InfoBtn);
        rb(ui, &mut rx, "tile", fm.tabv().panes.len() > 1, Act::SplitBtn);
    }
    rx -= 4;
    rb(ui, &mut rx, "lines_", false, Act::ModeBtn(Mode::List));
    let list_x = rx + 2;
    rb(ui, &mut rx, "grid", false, Act::ModeBtn(Mode::Grid));
    let grid_x = rx + 2;
    // список рисуем своим глифом: три полосы с точками
    for k in 0..3 {
        let yy = by + 9 + k * 5;
        ui.cv.fill(R::new(list_x + 8, yy, 2, 2), t.text_dim);
        ui.cv.fill(R::new(list_x + 12, yy, 10, 2), t.text_dim);
    }
    // отметить текущий вид: черта под кнопкой
    let sel_x = if fm.mode() == Mode::Grid { grid_x } else { list_x };
    ui.cv.fill(R::new(sel_x + 6, by + bs + 1, bs - 12, 2), t.red);
    rx -= 6;
    // поиск: поле фильтра
    let fw = if fm.focus == crate::fm::Focus::Filter || !fm.filter_f.text.is_empty() { 220 } else { 30 };
    let fr = R::new(rx - fw, by, fw, bs);
    if fw > 30 {
        ui.cv.pxbox(fr, &BoxStyle::new(t.code_bg, if fm.focus == crate::fm::Focus::Filter { t.red } else { t.border }));
        ui.cv.icon("search", fr.x + 8, fr.cy() - 7, 2, t.text_faint);
        let inner = R::new(fr.x + 28, fr.y, fr.w - 54, fr.h);
        field_text(ui, inner, &fm.filter_f, "фильтр по имени", fm.focus == crate::fm::Focus::Filter);
        ui.zone(fr, Act::FilterField, Cur::Text);
        if !fm.filter_f.text.is_empty() {
            ui.button(R::new(fr.right() - 24, fr.y + 5, 20, 20), "", "close", Tone::Ghost, false, true, Act::FilterClear);
        }
    } else {
        ui.button(fr, "", "search", Tone::Ghost, false, true, Act::SearchBtn);
    }
    rx = fr.x - 8;
    // путь
    let pr = R::new(x, by, (rx - x).max(60), bs);
    path_bar(ui, fm, pr);
}

pub(crate) fn field_text(ui: &mut Ui, inner: R, f: &crate::ui::Field, ph: &str, focused: bool) {
    let t = ui.t;
    let before: String = f.text.chars().take(f.cur).collect();
    let bw = ui.f.width(PX, &before);
    let shift = (bw - (inner.w - 10)).max(0);
    ui.cv.push_clip(inner);
    if f.text.is_empty() {
        if !focused {
            ui.text_in(PX, inner, ph, t.text_faint);
        } else {
            ui.text_in(PX, inner, ph, t.text_faint.fade(0.6));
        }
    } else {
        let y = inner.y + (inner.h - ui.f.line(PX)) / 2;
        ui.cv.text(ui.f, PX, inner.x - shift, y, &f.text, t.text);
    }
    if focused && ui.blink {
        let lh = ui.f.line(PX);
        ui.cv.fill(R::new(inner.x - shift + bw, inner.y + (inner.h - lh) / 2, 2, lh), t.ember);
    }
    ui.cv.pop_clip();
}

pub(crate) fn path_bar(ui: &mut Ui, fm: &Fm, r: R) {
    let t = ui.t;
    let v = fm.v();
    let editing = fm.focus == crate::fm::Focus::Path;
    ui.cv.pxbox(r, &BoxStyle::new(t.code_bg, if editing { t.red } else { t.border }));
    if editing {
        field_text(ui, R::new(r.x + 10, r.y, r.w - 20, r.h), &fm.path_f, "путь", true);
        ui.zone(r, Act::PathField, Cur::Text);
        return;
    }
    ui.zone(r, Act::PathBar, Cur::Text);
    if v.trash || v.search.is_some() {
        let icon = if v.trash { "trash" } else { "search" };
        ui.cv.icon(icon, r.x + 10, r.cy() - 7, 2, t.red);
        ui.text_in(PX, R::new(r.x + 32, r.y, r.w - 40, r.h), &v.title(), t.text);
        return;
    }
    // крошки: ⌂ / home / user / … ; не влезают — левые прячутся за «…»
    let home = crate::paths::home();
    let mut parts: Vec<(String, std::path::PathBuf)> = vec![];
    let mut acc = std::path::PathBuf::new();
    let rel = v.dir.strip_prefix(&home).ok();
    if let Some(rel) = rel {
        acc = home.clone();
        parts.push((crate::i18n::tr("дом").to_string(), acc.clone()));
        for c in rel.components() {
            acc.push(c);
            parts.push((c.as_os_str().to_string_lossy().to_string(), acc.clone()));
        }
    } else {
        for c in v.dir.components() {
            acc.push(c);
            let s = c.as_os_str().to_string_lossy().to_string();
            parts.push((if s == "/" { "/".into() } else { s }, acc.clone()));
        }
    }
    let sep_w = 14;
    let widths: Vec<i32> = parts.iter().map(|(s, _)| ui.f.width(PX, s) + 12).collect();
    let avail = r.w - 16 - if rel.is_some() { 18 } else { 0 };
    let mut first = 0;
    while first < parts.len() - 1 && widths[first..].iter().sum::<i32>() + sep_w * (parts.len() - first) as i32 > avail {
        first += 1;
    }
    let mut x = r.x + 6;
    if rel.is_some() {
        // глиф дома есть только в значках файлов — рисуем свой
        picons_glyph(ui, x + 2, r.cy() - 7, t.red);
        x += 18;
    }
    if first > 0 {
        ui.text_in(PX, R::new(x + 2, r.y, 20, r.h), "…", t.text_faint);
        x += 18;
    }
    for (i, (name, p)) in parts.iter().enumerate().skip(first) {
        let wdt = widths[i];
        let cr = R::new(x, r.y + 4, wdt, r.h - 8);
        let last = i == parts.len() - 1;
        let hov = ui.hover(cr);
        if hov {
            ui.cv.fill(cr, t.selection.alpha(0.6));
        }
        let c = if last { t.text } else if hov { t.text } else { t.text_dim };
        raw(ui, PX, R::new(x + 6, r.y, wdt - 8, r.h), name, c);
        if !last {
            ui.zone(cr, Act::Crumb(p.clone()), Cur::Pointer);
        }
        if !last && name != "/" {
            // разделитель — пиксельная косая
            let sx = x + wdt + 3;
            for k in 0..4 {
                ui.cv.fill(R::new(sx + 3 - k, r.cy() - 4 + k * 2, 2, 2), t.text_faint);
            }
        }
        x += wdt + if name == "/" { 2 } else { sep_w };
    }
}

pub(crate) fn picons_glyph(ui: &mut Ui, x: i32, y: i32, c: C) {
    const H: [&str; 7] = ["...#...", "..###..", ".#####.", "#######", ".#...#.", ".#.#.#.", ".#.#.#."];
    ui.cv.fill(R::new(x, y, 14, 14), C(0));
    for (j, row) in H.iter().enumerate() {
        for (i, ch) in row.bytes().enumerate() {
            if ch == b'#' {
                ui.cv.fill(R::new(x + i as i32 * 2, y + j as i32 * 2, 2, 2), c);
            }
        }
    }
}

// ── вкладки ─────────────────────────────────────────────────────────────────
pub(crate) fn tabs(ui: &mut Ui, fm: &Fm, r: R) {
    let t = ui.t;
    ui.cv.fill(r, t.panel.alpha(0.9));
    ui.cv.fill(R::new(r.x, r.bottom() - 1, r.w, 1), t.border);
    let n = fm.tabs.len() as i32;
    let tw = ((r.w - 40) / n).min(220);
    for (i, tab) in fm.tabs.iter().enumerate() {
        let x = r.x + 6 + i as i32 * tw;
        let tr = R::new(x, r.y + 3, tw - 4, r.h - 4);
        let cur = i == fm.tab;
        let hov = ui.hover(tr);
        let fill = if cur { t.surface_hi } else if hov { t.surface } else { C(0) };
        ui.cv.pxbox(tr, &BoxStyle::new(fill, if cur { t.red } else { C(0) }));
        if cur {
            ui.cv.fill(R::new(tr.x + 2, tr.y, tr.w - 4, 2), t.red);
        }
        let v = &tab.panes[tab.active];
        ui.zone(tr, Act::Tab(i), Cur::Pointer);
        let title = v.title();
        ui.cv.icon(if v.trash { "trash" } else { "folder" }, tr.x + 8, tr.cy() - 7, 2, if cur { t.red } else { t.text_faint });
        raw(ui, PX, R::new(tr.x + 28, tr.y, tr.w - 54, tr.h), &title, if cur { t.text } else { t.text_dim });
        if hov || cur {
            ui.button(R::new(tr.right() - 22, tr.y + 3, 18, tr.h - 6), "", "close", Tone::Ghost, false, true, Act::TabClose(i));
        }
    }
    let nx = r.x + 6 + n * tw;
    ui.button(R::new(nx, r.y + 3, 26, r.h - 6), "", "plus", Tone::Ghost, false, true, Act::TabNew);
}

// ── места ───────────────────────────────────────────────────────────────────
pub(crate) fn side_row(ui: &mut Ui, fm: &Fm, r: R, name: &str, sp: Special, active: bool, act: Act, translate: bool) {
    let t = ui.t;
    let hov = ui.hover(r);
    let drop = fm.drop_hint.is_some() && hov;
    if active {
        ui.cv.pxbox(r, &BoxStyle::new(t.blood.alpha(0.85), C(0)));
        ui.cv.fill(R::new(r.x, r.y + 3, 2, r.h - 6), t.ember);
    } else if drop {
        ui.cv.pxbox(r, &BoxStyle::new(t.selection, t.red));
    } else if hov {
        ui.cv.pxbox(r, &BoxStyle::new(t.surface_hi, C(0)));
    }
    picons::draw(&mut ui.cv, &t, Kind::Dir, sp, r.x + 8, r.y + (r.h - 16) / 2, 1, picons::Look { dim: false, link: false, selected: active });
    let tr = R::new(r.x + 32, r.y, r.w - 40, r.h);
    let c = if active { t.text_hi } else { t.text };
    if translate {
        ui.text_in(PX, tr, name, c);
    } else {
        raw(ui, PX, tr, name, c);
    }
    ui.zone(r, act, Cur::Pointer);
}

pub(crate) fn section(ui: &mut Ui, x: i32, y: i32, w: i32, s: &str) {
    let t = ui.t;
    ui.cv.fill(R::new(x, y + 7, 4, 4), t.red);
    ui.text_in(PX, R::new(x + 10, y, w - 10, 18), s, t.text_faint);
}

pub(crate) fn sidebar(ui: &mut Ui, fm: &Fm, r: R) {
    let t = ui.t;
    ui.cv.fill(r, t.panel.alpha(0.6));
    ui.cv.fill(R::new(r.right() - 1, r.y, 1, r.h), t.border);
    ui.zone(r, Act::Block, Cur::Default);
    let v = fm.v();
    let x = r.x + 8;
    let w = r.w - 16;
    let mut y = r.y + 10;
    let rh = 26;
    ui.cv.push_clip(r);
    section(ui, x, y, w, "места");
    y += 22;
    for p in &fm.places {
        let active = !v.trash && v.search.is_none() && v.dir == p.path;
        side_row(ui, fm, R::new(x, y, w, rh), &p.name, p.sp, active, Act::Place(p.path.clone(), false), true);
        y += rh;
    }
    for p in &fm.marks {
        let active = !v.trash && v.search.is_none() && v.dir == p.path;
        side_row(ui, fm, R::new(x, y, w, rh), &p.name, picons::Special::Star, active, Act::Place(p.path.clone(), true), false);
        y += rh;
    }
    let sp = if fm.trash_n > 0 { Special::TrashFull } else { Special::Trash };
    side_row(ui, fm, R::new(x, y, w, rh), "корзина", sp, v.trash, Act::PlaceTrash, true);
    if fm.trash_n > 0 {
        let n = fm.trash_n.to_string();
        let nw = ui.f.width(PX, &n) + 10;
        let br = R::new(x + w - nw - 4, y + 5, nw, rh - 10);
        ui.cv.pxbox(br, &BoxStyle::new(t.surface_hi, C(0)));
        raw_center(ui, PX, br, &n, t.text_dim);
    }
    y += rh + 12;
    let local: Vec<usize> = (0..fm.devs.len()).filter(|&i| fm.devs[i].kind != DevKind::Net).collect();
    if !local.is_empty() {
        section(ui, x, y, w, "устройства");
        y += 22;
        for i in local {
            y = dev_row(ui, fm, i, x, y, w, rh);
        }
    }
    if fm.cfg.network {
        y += 2;
        section(ui, x, y, w, "сеть");
        y += 22;
        for i in (0..fm.devs.len()).filter(|&i| fm.devs[i].kind == DevKind::Net) {
            y = dev_row(ui, fm, i, x, y, w, rh);
        }
        side_row(ui, fm, R::new(x, y, w, rh), "подключиться…", Special::Net, false, Act::Connect, true);
    }
    ui.cv.pop_clip();
}

/// строка устройства: значок, имя, заполненность или «не подключён», кнопка «извлечь»
pub(crate) fn dev_row(ui: &mut Ui, fm: &Fm, i: usize, x: i32, y: i32, w: i32, rh: i32) -> i32 {
    let t = ui.t;
    let v = fm.v();
    let d = &fm.devs[i];
    let rr = R::new(x, y, w, rh + 8);
    let root = d.mount.as_deref() == Some(std::path::Path::new("/"));
    let active = d.mount.as_ref().map(|m| !v.trash && v.dir.starts_with(m) && (m != std::path::Path::new("/") || !fm.devs.iter().any(|o| o.mount.as_ref().map(|om| om != m && v.dir.starts_with(om)).unwrap_or(false)) && !v.dir.starts_with(crate::paths::home()))).unwrap_or(false);
    let hov = ui.hover(rr);
    if active {
        ui.cv.pxbox(rr, &BoxStyle::new(t.blood.alpha(0.85), C(0)));
        ui.cv.fill(R::new(rr.x, rr.y + 3, 2, rr.h - 6), t.ember);
    } else if hov {
        ui.cv.pxbox(rr, &BoxStyle::new(t.surface_hi, C(0)));
    }
    let sp = match d.kind {
        DevKind::Phone => Special::Phone,
        DevKind::Camera => Special::Camera,
        DevKind::Net => Special::Net,
        DevKind::Disk if root => Special::Root,
        DevKind::Disk if d.removable => Special::Usb,
        DevKind::Disk => Special::Drive,
    };
    picons::draw(&mut ui.cv, &t, Kind::Other, sp, rr.x + 8, rr.y + 4, 1, picons::Look { dim: d.mount.is_none(), link: false, selected: active });
    let title = d.title();
    let tc = if d.mount.is_none() { t.text_dim } else if active { t.text_hi } else { t.text };
    let eject = d.mount.is_some() && !root && (d.removable || hov);
    raw(ui, PX, R::new(rr.x + 32, rr.y + 1, rr.w - 40 - if eject { 22 } else { 0 }, 18), &title, tc);
    match &d.mount {
        // заполненность — шкала из клеток (у сети и телефонов её может не быть)
        Some(m) => match fsx::disk_free(m).filter(|_| d.kind == DevKind::Disk || d.kind == DevKind::Phone) {
            Some((free, total)) if total > 0 => {
                let used = 1.0 - free as f32 / total as f32;
                let c = if used > 0.9 { t.warn } else { t.red };
                ui.cv.gauge(R::new(rr.x + 32, rr.y + 21, rr.w - 44, 6), 12, used, c, t.surface.lerp(t.border, 0.4), 1);
            }
            _ => {
                let s = if d.kind == DevKind::Net { d.uri.split("://").next().filter(|s| !s.is_empty()).unwrap_or(&d.fstype).to_string() } else { String::new() };
                raw(ui, PX, R::new(rr.x + 32, rr.y + 16, rr.w - 40, 16), &s, t.text_faint);
            }
        },
        None => {
            let s = if d.kind != DevKind::Disk && d.uri.is_empty() { "нужен gvfs-mtp" } else { "не подключён" };
            ui.text_in(PX, R::new(rr.x + 32, rr.y + 16, rr.w - 40, 16), s, t.text_faint);
        }
    }
    ui.zone(rr, Act::Dev(i), Cur::Pointer);
    if eject {
        ui.button(R::new(rr.right() - 24, rr.y + 4, 20, 20), "", "up", Tone::Ghost, false, true, Act::Eject(i));
    }
    y + rh + 10
}

// ── панель с файлами ────────────────────────────────────────────────────────
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
            used.set(fm.frame.get());
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

// ── сведения ────────────────────────────────────────────────────────────────
pub(crate) fn info(ui: &mut Ui, fm: &Fm, r: R) {
    let t = ui.t;
    ui.cv.fill(r, t.panel.alpha(0.6));
    ui.cv.fill(R::new(r.x, r.y, 1, r.h), t.border);
    ui.zone(r, Act::Block, Cur::Default);
    let v = fm.v();
    let sel = v.selected();
    let x = r.x + 14;
    let w = r.w - 28;
    let mut y = r.y + 14;
    if sel.len() > 1 {
        let total: u64 = sel.iter().map(|e| e.size).sum();
        picons::draw(&mut ui.cv, &t, Kind::Other, Special::None, x + (w - 96) / 2, y, 6, picons::Look { dim: false, link: false, selected: false });
        y += 110;
        ui.text_center(PX, R::new(x, y, w, 20), &crate::tf!("выбрано: {}", "selected: {}", sel.len()), t.text);
        y += 22;
        ui.cv.text_center(ui.f, PX, R::new(x, y, w, 20), &fsx::human(total), t.text_dim);
        return;
    }
    let e = sel.first().copied().or(v.cur_entry());
    let Some(e) = e else {
        ui.text_center(PX, R::new(x, r.y + r.h / 2 - 10, w, 20), "выбери файл", t.text_faint);
        return;
    };
    let big = 128;
    let shown_thumb = thumb_of(fm, e, 192);
    match shown_thumb {
        Some(img) => {
            let s = (w as f32 / img.w as f32).min(1.0);
            let (dw, dh) = ((img.w as f32 * s) as i32, (img.h as f32 * s) as i32);
            let tx = x + (w - dw) / 2;
            ui.cv.fill(R::new(tx - 3, y - 3, dw + 6, dh + 6), t.code_bg);
            ui.cv.image_scaled(&img, R::new(tx, y, dw, dh), 1.0);
            ui.cv.outline(R::new(tx - 3, y - 3, dw + 6, dh + 6), t.border, 1);
            y += dh + 14;
        }
        None => {
            let sp = if e.dir { crate::places::special_of(&e.path) } else { Special::None };
            picons::draw(&mut ui.cv, &t, mime::kind(e.mime), sp, x + (w - big) / 2, y, 8, picons::Look { dim: false, link: e.link, selected: false });
            y += big + 12;
        }
    }
    let lines = label_lines(ui.f, PX, &e.name, w, 3);
    for l in &lines {
        raw_center(ui, PX, R::new(x, y, w, 18), l, t.text);
        y += 18;
    }
    y += 8;
    ui.cv.fill(R::new(x, y, w, 1), t.border);
    y += 10;
    let mut kv = |ui: &mut Ui, k: &str, val: &str| {
        ui.text_in(PX, R::new(x, y, 90, 18), k, t.text_faint);
        raw(ui, PX, R::new(x + 92, y, w - 92, 18), val, t.text_dim);
        y += 20;
    };
    kv(ui, "тип", &mime::label(e.mime, &e.name));
    if !e.dir {
        kv(ui, "размер", &fsx::human(e.size));
    }
    kv(ui, "изменён", &fsx::when(e.mtime));
    if let Some(o) = &e.orig {
        kv(ui, "откуда", &o.parent().map(fsx::pretty).unwrap_or_default());
        kv(ui, "удалён", &fsx::when(e.deleted));
    }
    kv(ui, "права", &fsx::perm_str(e.mode));
    if e.link {
        if let Ok(tg) = std::fs::read_link(&e.path) {
            kv(ui, "ссылка на", &tg.display().to_string());
        }
    }
    // начало текстового файла — моноширинно не нужно: пиксельный шрифт и так ровный
    let k = mime::kind(e.mime);
    if matches!(k, Kind::Text | Kind::Code) && e.size < 2_000_000 {
        let mut pv = fm.preview.borrow_mut();
        if pv.as_ref().map(|p| p.0 != e.path).unwrap_or(true) {
            let lines: Vec<String> = std::fs::read(&e.path).ok().map(|b| String::from_utf8_lossy(&b[..b.len().min(4096)]).lines().take(40).map(|l| l.replace('\t', "  ")).collect()).unwrap_or_default();
            *pv = Some((e.path.clone(), lines));
        }
        if let Some((_, lines)) = pv.as_ref() {
            y += 8;
            let box_r = R::new(x - 4, y, w + 8, r.bottom() - y - 10);
            if box_r.h > 40 {
                ui.cv.pxbox(box_r, &BoxStyle::new(t.code_bg, t.border));
                ui.cv.push_clip(box_r.inset(4));
                let mut ly = box_r.y + 6;
                for l in lines {
                    if ly > box_r.bottom() {
                        break;
                    }
                    ui.cv.text(ui.f, PX, box_r.x + 8, ly, l, t.text_dim);
                    ly += 17;
                }
                ui.cv.pop_clip();
            }
        }
    }
}

// ── строка состояния ────────────────────────────────────────────────────────
pub(crate) fn status(ui: &mut Ui, fm: &Fm, r: R) {
    let t = ui.t;
    ui.cv.fill(r, t.panel.alpha(0.94));
    ui.cv.fill(R::new(r.x, r.y, r.w, 1), t.border);
    ui.zone(r, Act::Block, Cur::Default);
    let v = fm.v();
    let dirs = v.items.iter().filter(|&&k| v.all[k].dir).count();
    let files = v.items.len() - dirs;
    let sel = v.selected();
    let mut s = crate::tf!("папок {} · файлов {}", "{} folders · {} files", dirs, files);
    if !sel.is_empty() {
        let size: u64 = sel.iter().filter(|e| !e.dir).map(|e| e.size).sum();
        if sel.iter().any(|e| !e.dir) {
            s += &crate::tf!("  ·  выбрано {} ({})", "  ·  {} selected ({})", sel.len(), fsx::human(size));
        } else {
            s += &crate::tf!("  ·  выбрано {}", "  ·  {} selected", sel.len());
        }
    }
    ui.cv.fill(R::new(r.x + 10, r.cy() - 2, 4, 4), t.red);
    ui.cv.text_in(ui.f, PX, R::new(r.x + 20, r.y, r.w / 2, r.h), &s, t.text_dim);
    // справа: свободно на диске
    let mut rx = r.right() - 10;
    if let Some((free, total)) = v.free.filter(|_| !v.trash) {
        let gw = 90;
        let used = 1.0 - free as f32 / total.max(1) as f32;
        ui.cv.gauge(R::new(rx - gw, r.cy() - 4, gw, 8), 10, used, if used > 0.9 { t.warn } else { t.red }, t.surface.lerp(t.border, 0.4), 1);
        rx -= gw + 8;
        let fs = crate::tf!("свободно {}", "{} free", fsx::human(free));
        let fw = ui.f.width(PX, &fs);
        ui.cv.text_in(ui.f, PX, R::new(rx - fw, r.y, fw, r.h), &fs, t.text_faint);
        rx -= fw + 16;
    }
    // задачи: шкала, имя, отмена
    if let Some(j) = fm.jobs.first() {
        let jw = 330.min(rx - r.x - r.w / 2 + 120).max(160);
        let jx = rx - jw;
        let what = match j.kind {
            crate::ops::Kind::Copy => "копирую",
            crate::ops::Kind::Move => "переношу",
            crate::ops::Kind::Trash => "в корзину",
            crate::ops::Kind::Delete => "удаляю",
            crate::ops::Kind::Extract => "распаковываю",
            crate::ops::Kind::Compress => "сжимаю",
            crate::ops::Kind::Restore => "возвращаю",
            crate::ops::Kind::Link => "ссылки",
        };
        let lw = ui.text_in(PX, R::new(jx, r.y, 110, r.h), what, t.text);
        let gx = jx + lw + 8;
        let k = if j.total > 0 { j.done as f32 / j.total as f32 } else { ((fm.born.elapsed().as_millis() / 120) % 12) as f32 / 12.0 };
        ui.cv.gauge(R::new(gx, r.cy() - 4, jw - lw - 40, 8), 14, k, t.ember, t.surface.lerp(t.border, 0.4), 1);
        ui.button(R::new(jx + jw - 26, r.y + 3, 20, r.h - 6), "", "close", Tone::Ghost, false, true, Act::JobCancel(j.id));
        ui.anim = true;
    }
}

pub(crate) fn toast(ui: &mut Ui, fm: &Fm, w: i32, bottom: i32) {
    let Some((s, born)) = &fm.toast else { return };
    let t = ui.t;
    let el = born.elapsed().as_millis() as f32;
    if el > 3200.0 {
        return;
    }
    ui.anim = true;
    let tw = ui.w(PX, s).min(w - 80) + 36;
    let r = R::new((w - tw) / 2, bottom - 46, tw, 32);
    ui.cv.pxbox(r, &BoxStyle::new(t.panel.alpha(0.97), t.red).shadow(3));
    ui.cv.fill(R::new(r.x + 12, r.cy() - 3, 6, 6), t.red);
    ui.text_in(PX, R::new(r.x + 26, r.y, r.w - 32, r.h), s, t.text);
    ui.zone(r, Act::Toast, Cur::Pointer);
    let k = if el < 2800.0 { ease(*born, 180.0, fm) } else { 1.0 - (el - 2800.0) / 400.0 };
    if k < 1.0 {
        ui.cv.dissolve_rect(R::new(r.x, r.y, r.w + 4, r.h + 4), k.max(0.0), 2);
    }
}

// ── выбор файлов: имя, фильтр, кнопки ───────────────────────────────────────
pub(crate) fn pick_bar(ui: &mut Ui, fm: &Fm, r: R) {
    let t = ui.t;
    let Some(p) = &fm.pick else { return };
    ui.cv.fill(r, t.panel.alpha(0.96));
    ui.cv.fill(R::new(r.x, r.y, r.w, 1), t.border);
    ui.zone(r, Act::Block, Cur::Default);
    let by = r.y + 13;
    let bh = 32;
    let mut rx = r.right() - 12;
    let ok_w = ui.w(PX, &p.accept).max(60) + 36;
    rx -= ok_w;
    ui.button(R::new(rx, by, ok_w, bh), &p.accept, "check", Tone::Primary, false, true, Act::PickOk);
    rx -= 8;
    let cw = ui.w(PX, "отмена") + 30;
    rx -= cw;
    ui.button(R::new(rx, by, cw, bh), "отмена", "", Tone::Plain, false, true, Act::PickCancel);
    rx -= 16;
    let mut x = r.x + 12;
    if !p.filters.is_empty() {
        let name = p.filters.get(p.filter).map(|f| f.0.clone()).unwrap_or_default();
        let fw = (ui.f.width(PX, &name) + 44).clamp(120, 260);
        rx -= fw;
        let fr = R::new(rx, by, fw, bh);
        let hov = ui.hover(fr);
        ui.cv.pxbox(fr, &BoxStyle::new(t.surface, if hov { t.red } else { t.border }).shadow(2).bevel(true));
        ui.cv.text_in(ui.f, PX, R::new(fr.x + 10, fr.y, fr.w - 34, fr.h), &name, t.text);
        ui.cv.icon("down", fr.right() - 22, fr.cy() - 7, 2, t.text_dim);
        ui.zone(fr, Act::PickFilter, Cur::Pointer);
        rx -= 10;
    }
    if p.save {
        ui.text_in(PX, R::new(x, by, 50, bh), "имя", t.text_dim);
        x += ui.w(PX, "имя") + 12;
        let nr = R::new(x, by, (rx - x).max(80), bh);
        ui.cv.pxbox(nr, &BoxStyle::new(t.code_bg, if p.name_focus { t.red } else { t.border }));
        field_text(ui, R::new(nr.x + 10, nr.y, nr.w - 20, nr.h), &p.name, "имя файла", p.name_focus);
        ui.zone(nr, Act::PickName, Cur::Text);
        if let Some(o) = &p.overwrite {
            let s = crate::tf!("«{}» уже есть — нажми ещё раз, чтобы заменить", "“{}” exists — press again to replace", o.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
            let sw = ui.f.width(PX, &s) + 24;
            let wr = R::new(nr.x, r.y - 34, sw, 28);
            ui.cv.pxbox(wr, &BoxStyle::new(t.panel, t.warn).shadow(3));
            ui.cv.text_in(ui.f, PX, R::new(wr.x + 12, wr.y, wr.w - 16, wr.h), &s, t.warn);
        }
    } else {
        let sel = fm.v().selected();
        let s = if sel.is_empty() {
            crate::i18n::tr(if p.dirs { "выбери папку (или открой её и нажми «выбрать»)" } else { "выбери файл двойным щелчком или кнопкой" }).to_string()
        } else {
            sel.iter().map(|e| e.name.clone()).collect::<Vec<_>>().join(", ")
        };
        ui.cv.text_in(ui.f, PX, R::new(x, by, rx - x, bh), &s, t.text_dim);
    }
}

// ── меню ────────────────────────────────────────────────────────────────────
pub fn menu_rect(m: &Menu, items: &[crate::fm::MItem], x: i32, y: i32, w: i32, h: i32, mw: i32) -> R {
    let mh = menu_h(items) + if m.title.is_empty() || m.title == "≡" { 0 } else { 26 };
    let mut mx = x;
    let mut my = y;
    if mx + mw > w - 4 {
        mx = (w - mw - 4).max(4);
    }
    if my + mh > h - 4 {
        my = (y - mh).max(4);
    }
    R::new(mx, my, mw, mh)
}

/// ширина меню по самому длинному пункту
pub(crate) fn menu_w(ui: &mut Ui, items: &[crate::fm::MItem]) -> i32 {
    let mut w = 0;
    for it in items.iter().filter(|i| !i.is_sep()) {
        let lw = ui.w(PX, &it.label);
        let kw = if it.key.is_empty() { if it.sub.is_empty() { 0 } else { 16 } } else { ui.f.width(PX, it.key) };
        w = w.max(lw + kw + 64);
    }
    w.clamp(170, 380)
}

pub fn menu_h(items: &[crate::fm::MItem]) -> i32 {
    items.iter().map(|i| if i.is_sep() { 9 } else { MROW }).sum::<i32>() + 8
}

pub(crate) fn menu_items(ui: &mut Ui, items: &[crate::fm::MItem], r: R, hover: Option<usize>, open_sub: Option<usize>, sub: bool) {
    let t = ui.t;
    let mut y = r.bottom() - menu_h(items) + 4;
    for (i, it) in items.iter().enumerate() {
        if it.is_sep() {
            ui.cv.fill(R::new(r.x + 8, y + 4, r.w - 16, 1), t.border);
            y += 9;
            continue;
        }
        let row = R::new(r.x + 4, y, r.w - 8, MROW);
        let hov = it.on && (ui.hover(row) || hover == Some(i) || open_sub == Some(i));
        if hov {
            ui.cv.pxbox(row, &BoxStyle::new(t.blood, C(0)));
            ui.cv.fill(R::new(row.x, row.y + 3, 2, row.h - 6), t.ember);
        }
        let fg = if !it.on { t.text_faint } else if hov { t.text_hi } else { t.text };
        if let Some(c) = it.check {
            let b = R::new(row.x + 8, row.cy() - 5, 10, 10);
            ui.cv.outline(b, if hov { fg } else { t.border.lerp(t.red, 0.5) }, 1);
            if c {
                ui.cv.fill(b.inset(2), t.ember);
            }
        } else if !it.icon.is_empty() {
            ui.cv.icon(it.icon, row.x + 7, row.cy() - 7, 2, if hov { fg } else { t.red });
        }
        let kw = if it.key.is_empty() { 20 } else { ui.f.width(PX, it.key) + 16 };
        ui.text_in(PX, R::new(row.x + 28, row.y, row.w - 28 - kw, row.h), &it.label, fg);
        if !it.sub.is_empty() {
            ui.cv.icon("right", row.right() - 16, row.cy() - 4, 1, fg);
        } else if !it.key.is_empty() {
            ui.cv.text_right(ui.f, PX, R::new(row.x, row.y, row.w - 8, row.h), it.key, if hov { t.text } else { t.text_faint });
        }
        if it.on {
            ui.zone(row, if sub { Act::MSub(i) } else { Act::MItem(i) }, Cur::Pointer);
        }
        y += MROW;
    }
}

pub(crate) fn menu(ui: &mut Ui, fm: &Fm, m: &Menu, w: i32, h: i32) {
    let t = ui.t;
    ui.zone(R::new(0, 0, w, h), Act::MenuBg, Cur::Default);
    let mw = menu_w(ui, &m.items);
    let r = menu_rect(m, &m.items, m.x, m.y, w, h, mw);
    let k = ease(m.born, 150.0, fm);
    let rise = ((1.0 - k) * 6.0) as i32;
    let r = R::new(r.x, r.y + rise, r.w, r.h);
    ui.cv.pxbox(r, &BoxStyle::new(t.panel.alpha(0.98), t.border.lerp(t.red, 0.5)).shadow(4));
    ui.zone(r, Act::Block, Cur::Default);
    if !m.title.is_empty() && m.title != "≡" {
        let hr = R::new(r.x + 1, r.y + 1, r.w - 2, 24);
        ui.cv.fill(R::new(hr.x + 8, hr.cy() - 2, 4, 4), t.red);
        ui.cv.text_in(ui.f, PX, R::new(hr.x + 18, hr.y, hr.w - 24, hr.h), &m.title, t.red);
        ui.cv.fill(R::new(hr.x + 6, hr.bottom(), hr.w - 12, 1), t.border);
    }
    menu_items(ui, &m.items, r, m.hover, m.sub.map(|s| s.0), false);
    if k < 1.0 {
        ui.cv.dissolve_rect(R::new(r.x, r.y, r.w + 4, r.h + 4), k, 2);
        ui.anim = true;
    }
    // подменю справа от пункта (не влезает — слева)
    if let Some((si, born)) = m.sub {
        let it = &m.items[si];
        if it.sub.is_empty() {
            return;
        }
        let mut y = r.bottom() - menu_h(&m.items) + 4;
        for j in 0..si {
            y += if m.items[j].is_sep() { 9 } else { MROW };
        }
        let sh = menu_h(&it.sub);
        let sw = menu_w(ui, &it.sub);
        let mut sx = r.right() - 4;
        if sx + sw > w - 4 {
            sx = r.x - sw + 4;
        }
        let sy = (y - 4).min(h - sh - 4).max(4);
        let sr = R::new(sx, sy, sw, sh);
        ui.cv.pxbox(sr, &BoxStyle::new(t.panel.alpha(0.98), t.border.lerp(t.red, 0.5)).shadow(4));
        ui.zone(sr, Act::Block, Cur::Default);
        menu_items(ui, &it.sub, sr, m.sub_hover, None, true);
        let k = ease(born, 120.0, fm);
        if k < 1.0 {
            ui.cv.dissolve_rect(R::new(sr.x, sr.y, sr.w + 4, sr.h + 4), k, 2);
            ui.anim = true;
        }
    }
}
