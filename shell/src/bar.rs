// Верхний бар: тёмная матовая панель, тонкая линия снизу.
// Модули и их порядок — в настройках (bar_left / bar_center / bar_right).

use crate::app::{local_now, State, BAR_H, RU_DAY};
use crate::font::{self, PX};
use crate::gfx::{BoxStyle, R};
use crate::icons;
use crate::theme::{CLEAR, WHITE};
use crate::ui::{Act, Cur, Tone, Ui};

const H: i32 = BAR_H;

pub fn draw(ui: &mut Ui, st: &State, out: &str, w: i32) {
    let t = ui.t;
    ui.cv.blur_rect(R::new(0, 0, w, H), 0);
    ui.cv.fill(R::new(0, 0, w, H), t.glass_panel());
    ui.cv.fill(R::new(0, H - 1, w, 1), t.border);
    let set = &st.set;

    let gap = set.bar_gap.clamp(2, 24);
    let sw = set.bar_sep_w.clamp(2, 32);
    // разделитель рисуется только между двумя видимыми модулями: без пар и висячих по краям
    let row = |ui: &mut Ui, list: &mut dyn Iterator<Item = &String>, x0: i32, dir: i32| -> i32 {
        let mut x = x0;
        let (mut pending, mut any) = (false, false);
        for m in list {
            if m == "sep" {
                pending = any;
                continue;
            }
            let mw = module(ui, st, out, m, 0, true);
            if mw == 0 {
                continue;
            }
            if pending {
                let sx = if dir > 0 { x } else { x - sw };
                sep(ui, st, sx, sw);
                x += dir * (sw + gap);
                pending = false;
            }
            module(ui, st, out, m, if dir > 0 { x } else { x - mw }, false);
            x += dir * (mw + gap);
            any = true;
        }
        x
    };
    let left_end = row(ui, &mut set.bar_left.iter(), 6, 1);
    let right_start = row(ui, &mut set.bar_right.iter().rev(), w - 6, -1);

    // в середине — окна стола (и всё, что в bar_center)
    let mut cx = left_end + 4;
    let mut pending = false;
    for m in set.bar_center.iter() {
        if m == "sep" {
            pending = cx > left_end + 4;
            continue;
        }
        if pending {
            sep(ui, st, cx, sw);
            cx += sw + gap;
            pending = false;
        }
        if m == "tasks" {
            tasks(ui, st, out, R::new(cx, 0, right_start - cx - 8, H - 1));
            cx = right_start;
        } else {
            let mw = module(ui, st, out, m, cx, false);
            cx += mw + gap;
        }
    }
}

/// разделитель шириной w от x: точки, черта, клетки или просто место
fn sep(ui: &mut Ui, st: &State, x: i32, w: i32) {
    let t = ui.t;
    let c = match st.set.bar_sep_color.as_str() {
        "accent" => t.red,
        "dim" => t.text_faint,
        _ => t.border,
    };
    let cx = x + w / 2 - 1;
    match st.set.bar_sep.as_str() {
        "line" => ui.cv.fill(R::new(cx, 7, 1, H - 14), c),
        "blocks" => {
            for i in 0..3 {
                ui.cv.fill(R::new(cx, H / 2 - 7 + i * 5, 3, 3), c);
            }
        }
        "gap" => {}
        _ => ui.cv.sep(cx, H / 2, c),
    }
}

/// модуль бара: рисует (или только меряет) с левого края x, возвращает ширину
fn module(ui: &mut Ui, st: &State, out: &str, m: &str, x: i32, measure: bool) -> i32 {
    let t = ui.t;
    let cy = (H - 1) / 2;
    match m {
        "launcher" => {
            if !measure {
                let r = R::new(x, 0, 26, H - 1);
                let hov = ui.hover(r) || st.modal == Some(crate::app::Modal::Launcher);
                for i in 0..4 {
                    let c = if i == 0 || hov { t.red } else { t.dim };
                    ui.cv.fill(R::new(x + 6 + (i % 2) * 8, cy - 7 + (i / 2) * 8, 6, 6), c);
                }
                ui.zone(r, Act::Launcher, Cur::Pointer);
            }
            26
        }
        "workspaces" => {
            let list = st.niri.bar_ws(out);
            let w = list.len() as i32 * 26 - 4;
            if measure {
                return w.max(0);
            }
            let mut wx = x;
            for ws in list {
                let r = R::new(wx, cy - 11, 22, 22);
                let busy = st.niri.has_windows(ws.id);
                let hov = ui.hover(r);
                let fill = if ws.active { t.red } else if hov { t.surface_hi } else if busy { t.surface } else { CLEAR };
                let outline = if ws.urgent || ws.active { t.ember } else if busy { t.border } else { t.border.fade(0.5) };
                // срочный стол мигает
                let blink = ws.urgent && !ws.active && (st.started.elapsed().as_millis() / 400) % 2 == 1;
                if !blink {
                    ui.cv.pxbox(r, &BoxStyle::new(fill, outline).bevel(ws.active || busy).shadow(if ws.active { 2 } else { 0 }));
                    let label = ws.name.as_ref().and_then(|n| n.chars().next()).map(|c| c.to_string()).unwrap_or_else(|| ws.idx.to_string());
                    let c = if ws.active { t.on_accent } else if busy { t.text } else { t.text_faint };
                    ui.text_center(PX, r, &label, c);
                }
                if ws.urgent && !ws.active {
                    ui.anim = true;
                }
                ui.zone(r, Act::Ws(ws.id), Cur::Pointer);
                wx += 26;
            }
            w
        }
        "mode" => {
            let label = if st.niri.stacking { "стол" } else { "лента" };
            let w = ui.w(PX, label) + 14 + 7 + 14;
            if !measure {
                let r = R::new(x, cy - 11, w, 22);
                ui.button(r, label, if st.niri.stacking { "float" } else { "tile" }, Tone::Plain, st.niri.stacking, true, Act::Mode);
            }
            w
        }
        "tray" => {
            let items: Vec<&crate::dbus::TrayItem> = st.tray.iter().filter(|i| i.status != "Passive").collect();
            let w = items.len() as i32 * 24 - 2;
            if measure || items.is_empty() {
                return w.max(0);
            }
            let mut ix = x;
            for it in items {
                let r = R::new(ix, cy - 11, 22, 22);
                if ui.hover(r) {
                    ui.cv.pxbox(r, &BoxStyle::new(t.surface_hi, t.red));
                }
                if let Some(img) = &it.pixmap {
                    ui.cv.image(img, ix + 3, cy - 8, 1.0);
                } else if !it.icon.is_empty() {
                    let name = it.icon.clone();
                    if let Some(img) = ui.ic.get(&name, 16).or_else(|| it.theme_path.as_ref().and_then(|p| icons::load_file(&std::path::Path::new(p).join(format!("{name}.png")), 16).map(std::rc::Rc::new))) {
                        ui.cv.image(&img, ix + 3, cy - 8, 1.0);
                    } else {
                        ui.cv.icon("dots", ix + 4, cy - 7, 2, t.text_dim);
                    }
                } else {
                    ui.cv.icon("dots", ix + 4, cy - 7, 2, t.text_dim);
                }
                ui.zone(r, Act::Tray(it.service.clone()), Cur::Pointer);
                ix += 24;
            }
            w
        }
        "net" => {
            let off = !st.sys.online;
            let w = if off { 6 + 6 + ui.w(PX, "нет сети") } else { 10 };
            if !measure {
                let r = R::new(x - 2, 0, w + 4, H - 1);
                let hov = ui.hover(r);
                ui.cv.fill(R::new(x + if off { 0 } else { 2 }, cy - 3, 6, 6), if off { t.ember } else if hov { WHITE } else { t.ok });
                if off {
                    ui.text_in(PX, R::new(x + 12, 0, w, H - 1), "нет сети", t.ember);
                }
                ui.zone(r, Act::Popup("net"), Cur::Pointer);
            }
            w
        }
        "netspeed" => {
            let s = format!("↓{} ↑{}", crate::sys::rate(st.sys.rx), crate::sys::rate(st.sys.tx));
            let w = ui.w(PX, "↓8888K ↑8888K");
            if !measure {
                let r = R::new(x, 0, w, H - 1);
                ui.text_in(PX, r, &s, if ui.hover(r) { t.text } else { t.text_dim });
                ui.zone(r, Act::Popup("net"), Cur::Pointer);
            }
            w
        }
        "bt" => {
            if !st.bt.present {
                return 0;
            }
            let n = st.bt.devices.iter().filter(|d| d.connected).count();
            let label = if n > 0 { n.to_string() } else { String::new() };
            let w = 14 + if n > 0 { 4 + ui.w(PX, &label) } else { 0 };
            if !measure {
                let r = R::new(x - 2, 0, w + 4, H - 1);
                let hov = ui.hover(r);
                let c = if !st.bt.powered { t.text_faint } else if hov { WHITE } else if n > 0 { t.red } else { t.text_dim };
                ui.cv.icon("bt", x, cy - 7, 2, c);
                if n > 0 {
                    ui.text_in(PX, R::new(x + 18, 0, 40, H - 1), &label, t.text);
                }
                ui.zone(r, Act::Popup("bt"), Cur::Pointer);
            }
            w
        }
        "profile" => {
            if !measure {
                let r = R::new(x - 2, 0, 18, H - 1);
                let hov = ui.hover(r);
                let c = if hov { WHITE } else if !st.prof.present { t.text_faint } else if st.prof.active == "performance" { t.red } else if st.prof.active == "power-saver" { t.ok } else { t.text_dim };
                ui.cv.icon(st.prof.icon(), x, cy - 7, 2, c);
                ui.zone(r, Act::Popup("profile"), Cur::Pointer);
            }
            14
        }
        "kb" => {
            let s = st.niri.kb_short();
            let s = if s.is_empty() { "??".to_string() } else { s };
            let w = ui.w(PX, &s) + 12;
            if !measure {
                ui.button(R::new(x, cy - 11, w, 22), &s, "", Tone::Plain, false, true, Act::Kb);
            }
            w
        }
        "cpu" | "ram" | "temp" => {
            let (label, v, txt, hot) = match m {
                "cpu" => ("цп", st.sys.cpu, format!("{}%", (st.sys.cpu * 100.0).round()), st.sys.cpu > 0.85),
                "ram" => ("озу", st.sys.mem, format!("{}%", (st.sys.mem * 100.0).round()), st.sys.mem > 0.85),
                _ => ("t°", (st.sys.temp / 100.0).min(1.0), format!("{}°", st.sys.temp.round()), st.sys.temp > 80.0),
            };
            let lw = ui.w(PX, label);
            let style = st.set.cpu_style.as_str();
            let gw = if style == "number" { 0 } else { 48 };
            let w = lw + 6 + gw + if gw > 0 { 6 } else { 0 } + 34;
            if measure {
                return w;
            }
            let r = R::new(x, 0, w, H - 1);
            let hov = ui.hover(r);
            ui.text_in(PX, R::new(x, 0, lw, H - 1), label, if hov { t.text_dim } else { t.text_faint });
            let gx = x + lw + 6;
            let col = if hot { t.ember } else { t.red };
            if style == "graph" {
                let src = match m {
                    "cpu" => "cpu",
                    "ram" => "ram",
                    _ => "temp",
                };
                let (ser, full) = st.sys.series(src);
                let pts = ser.tail(24);
                for (i, p) in pts.iter().enumerate() {
                    let hh = ((p / full).clamp(0.0, 1.0) * 14.0).round() as i32;
                    ui.cv.fill(R::new(gx + i as i32 * 2, cy + 7 - hh.max(1), 2, hh.max(1)), if i == pts.len() - 1 { t.ember } else { col.fade(0.8) });
                }
                ui.cv.fill(R::new(gx, cy + 8, 48, 1), t.border);
            } else if style == "gauge" {
                ui.cv.gauge(R::new(gx, cy - 5, 48, 10), 6, v, col, t.surface_hi.lerp(t.border, 0.45), 2);
            }
            let tx = gx + gw + if gw > 0 { 6 } else { 0 };
            ui.text_in(PX, R::new(tx, 0, 34, H - 1), &txt, if hot { t.ember } else { t.text_dim });
            ui.zone(r, Act::Popup("sys"), Cur::Pointer);
            w
        }
        "vol" => {
            let a = &st.audio;
            let s = if a.muted { "—".to_string() } else { format!("{}", (a.volume * 100.0).round()) };
            let w = 14 + 5 + ui.w(PX, "100");
            if !measure {
                let r = R::new(x, 0, w, H - 1);
                let hov = ui.hover(r);
                ui.cv.icon(if a.muted { "mute" } else { "speaker" }, x, cy - 7, 2, if a.muted { t.text_faint } else if hov { WHITE } else { t.text });
                ui.text_in(PX, R::new(x + 19, 0, w - 19, H - 1), &s, if a.muted { t.text_faint } else { t.text });
                ui.zone(r, Act::Popup("sound"), Cur::Pointer);
            }
            w
        }
        "media" => {
            let me = &st.media;
            if me.title.is_empty() {
                return 0;
            }
            let w = 200;
            if !measure {
                let r = R::new(x, 0, w, H - 1);
                let hov = ui.hover(r);
                // маленький эквалайзер: 5 столбиков
                for i in 0..5 {
                    let v = st.viz.get(i * 4 + 2).copied().unwrap_or(0.0);
                    let hh = if me.playing() { (2.0 + v * 12.0).round() as i32 } else { 2 };
                    ui.cv.fill(R::new(x + i as i32 * 3, cy + 7 - hh, 2, hh), t.red);
                }
                let label = if me.artist.is_empty() { me.title.clone() } else { format!("{} — {}", me.artist, me.title) };
                ui.text_in(PX, R::new(x + 20, 0, w - 20, H - 1), &label, if hov { WHITE } else { t.text_dim });
                ui.zone(r, Act::Popup("sound"), Cur::Pointer);
            }
            w
        }
        "clock" => {
            let tm = local_now();
            let time = if st.set.clock_seconds { format!("{:02}:{:02}:{:02}", tm.hour, tm.min, tm.sec) } else { format!("{:02}:{:02}", tm.hour, tm.min) };
            let date = format!("{} {:02}.{:02}", crate::i18n::tr(RU_DAY[tm.wday as usize]), tm.day, tm.mon);
            let tw = ui.w(PX, &time);
            let dw = if st.set.clock_date { ui.w(PX, &date) + 6 } else { 0 };
            let w = tw + dw;
            if !measure {
                let r = R::new(x - 4, 0, w + 8, H - 1);
                let hov = ui.hover(r) || st.popup.as_ref().map(|p| p.kind == "cal").unwrap_or(false);
                if hov {
                    ui.cv.fill(R::new(x - 4, H - 3, w + 8, 2), t.red);
                }
                ui.text_in(PX, R::new(x, 0, tw, H - 1), &time, if hov { WHITE } else { t.text });
                if st.set.clock_date {
                    ui.text_in(PX, R::new(x + tw + 6, 0, dw, H - 1), &date, t.text_dim);
                }
                ui.zone(r, Act::Popup("cal"), Cur::Pointer);
            }
            w
        }
        "notif" => {
            let n = st.notifs.history.len();
            let w = 14 + if n > 0 { 4 + ui.w(PX, &n.to_string()) } else { 0 };
            if !measure {
                let r = R::new(x - 2, 0, w + 4, H - 1);
                let hov = ui.hover(r);
                let c = if st.set.dnd { t.text_faint } else if hov { WHITE } else if n > 0 { t.red } else { t.text_dim };
                ui.cv.icon(if st.set.dnd { "bellx" } else { "bell" }, x, cy - 7, 2, c);
                if n > 0 {
                    ui.text_in(PX, R::new(x + 18, 0, 30, H - 1), &n.to_string(), t.text);
                }
                ui.zone(r, Act::Popup("cal"), Cur::Pointer);
            }
            w
        }
        "settings" => {
            if !measure {
                ui.button(R::new(x, cy - 11, 26, 22), "", "gear", Tone::Ghost, st.modal == Some(crate::app::Modal::Settings), true, Act::Settings);
            }
            26
        }
        "power" => {
            if !measure {
                ui.button(R::new(x, cy - 11, 26, 22), "", "power", Tone::Danger, st.modal == Some(crate::app::Modal::Power), true, Act::Power);
            }
            26
        }
        _ => 0,
    }
}

/// окна стола: активное — в акценте с яркой полоской, свёрнутое — блёклое, со значком
fn tasks(ui: &mut Ui, st: &State, out: &str, area: R) {
    let t = ui.t;
    let Some(ws) = st.niri.active_ws(out) else { return };
    let mut list = st.niri.windows_on(ws.id);
    list.extend(st.niri.minimized());
    if list.is_empty() || area.w <= 0 {
        return;
    }
    let n = list.len() as i32;
    let iw = st.set.task_width.min((area.w + 4) / n - 4).max(24);
    ui.cv.push_clip(area);
    let mut x = area.x;
    for w in list {
        let r = R::new(x, area.cy() - 12, iw, 24);
        let hidden = st.niri.is_min(&w);
        let hov = ui.hover(r);
        let fill = if w.focused { t.selection } else if hov { t.surface_hi } else if hidden { CLEAR } else { t.surface };
        let outline = if w.focused || hov { t.red } else if hidden { t.border.fade(0.6) } else { t.border };
        ui.cv.pxbox(r, &BoxStyle::new(fill, outline).bevel(!hidden).shadow(if w.focused { 2 } else { 0 }));
        if w.focused {
            ui.cv.fill(R::new(r.x + 4, r.bottom() - 3, r.w - 8, 1), t.ember);
        }
        if w.urgent && !w.focused {
            ui.cv.outline(r, t.ember, 1);
        }
        let mut cx = r.x + 6;
        let icon = icons::icon_for(&st.apps, &w.app_id);
        ui.app_icon(&icon, cx, r.cy() - 8, 16, if hidden { 0.45 } else { 1.0 });
        cx += 22;
        if hidden {
            ui.cv.icon("min", cx, r.cy() - 7, 2, t.text_faint);
            cx += 20;
        }
        let title = if w.title.is_empty() { &w.app_id } else { &w.title };
        let c = if w.focused { WHITE } else if hidden { t.text_faint } else { t.text };
        ui.text_in(font::PX, R::new(cx, r.y, r.right() - cx - 6, r.h), title, c);
        ui.zone(r, Act::Task(w.id), Cur::Pointer);
        x += iw + 4;
    }
    ui.cv.pop_clip();
}
