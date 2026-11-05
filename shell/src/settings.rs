// Настройки: вкладки слева, содержимое справа. Всё применяется сразу
// и пишется в ~/.config/yami-shell.json (его можно править и руками — шелл подхватит).

use crate::app::{App, Modal, SKind, State};
use crate::config::Widget;
use crate::font::{self, PX};
use crate::gfx::{BoxStyle, C, R};
use crate::theme::{parse_hex, PRESETS, WHITE};
use crate::ui::{Act, Cur, Tone, Ui};
pub use crate::bartab::{BDrag, BarEd};
use crate::bartab::tab_bar;
use crate::widgets::{self, name_of, ANIMS, COLORS, KINDS, SIZES, SOURCES, STYLES};
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Keysym};

pub const SIZE: (i32, i32) = (780, 560);

/// вкладки по смыслу: как выглядит (цвет, обои, вид, шрифты), стол (бар, виджеты, док, окна,
/// экраны), что делает (клавиши, игры, скриншоты), система (система, службы, обновления, о шелле)
pub const TABS: &[(&str, &str, &str)] = &[
    ("color", "цвет", "drop"),
    ("wall", "обои", "image"),
    ("look", "вид", "eye"),
    ("fonts", "шрифты", "text"),
    ("bar", "бар", "menu"),
    ("widgets", "виджеты", "grid"),
    ("dock", "док", "tile"),
    ("windows", "окна", "float"),
    ("screens", "экраны", "screen"),
    ("keys", "клавиши", "keys"),
    ("games", "игры", "play"),
    ("shots", "скриншоты", "camera"),
    ("system", "система", "gear"),
    ("agents", "службы", "bell"),
    ("updates", "обновления", "down"),
    ("about", "о шелле", "about"),
];

/// слова для поиска настроек из лаунчера («vrr» → экраны): по-русски и по-английски
pub const FIND: &[(&str, &str)] = &[
    ("color", "палитра акцент цвет тема светлая тёмная стекло прозрачность palette accent colour color theme light dark glass"), // i18n-ok
    ("wall", "обои картинка фон wallpaper background image"), // i18n-ok
    ("look", "анимации эффекты появление развёртка размытие значки иконки курсор animation effects reveal scanlines blur icons cursor"), // i18n-ok
    ("fonts", "шрифт размер терминал fastfetch браузер fonts size terminal browser"), // i18n-ok
    ("bar", "бар модули часы разделитель плавающий плашка ширина центр bar modules clock separator floating pill width center"), // i18n-ok
    ("widgets", "виджеты график часы заметка widgets graph clock note"), // i18n-ok
    ("dock", "док закреплённые dock pinned"), // i18n-ok
    ("windows", "окна отступы плавающие стол заголовки gaps floating stacking windows titlebars"), // i18n-ok
    ("keys", "клавиши сочетания хоткеи раскладка keys shortcuts hotkeys layout keyboard"), // i18n-ok
    ("games", "игры steam proton разрывы gamemode ntsync tearing games shader"), // i18n-ok
    ("shots", "скриншоты снимок лупа screenshot loupe"), // i18n-ok
    ("screens", "экраны монитор разрешение частота масштаб vrr hz герц поворот display monitor resolution refresh scale rotation"), // i18n-ok
    ("system", "система язык сон блокировка ожидание system language idle sleep lock"), // i18n-ok
    ("agents", "службы уведомления трей polkit порталы обложки services notifications tray portals"), // i18n-ok
    ("updates", "обновления версия откат updates version rollback"), // i18n-ok
    ("about", "о шелле версия память about version memory"), // i18n-ok
];

pub fn draw(ui: &mut Ui, st: &State, w: i32, h: i32) {
    let t = ui.t;
    let fr = R::new(0, 0, w - 6, h - 6);
    ui.frame(fr, false);
    ui.header(R::new(1, 1, fr.w - 2, 30), "gear", "настройки", "yami-shell");
    ui.ghost(R::new(fr.right() - 28, 5, 24, 22), "close", Act::CloseModal);
    // вкладки
    let side = R::new(1, 31, 160, fr.h - 32);
    ui.cv.fill(side, t.panel.alpha(0.5));
    ui.cv.fill(R::new(side.right(), side.y, 1, side.h), t.border);
    for (i, (id, name, icon)) in TABS.iter().enumerate() {
        let r = R::new(side.x + 8, side.y + 8 + i as i32 * 30, side.w - 16, 26);
        ui.button(r, name, icon, Tone::Ghost, st.stab == *id, true, Act::STab(id));
        if st.stab == *id {
            ui.cv.fill(R::new(r.x - 8, r.y, 3, r.h), t.red);
        }
    }
    ui.text_in(PX, R::new(side.x + 12, side.bottom() - 26, side.w - 20, 20), &crate::tf!("{:.0} МБ памяти", "{:.0} MB of memory", st.rss), t.text_faint);
    let body = R::new(side.right() + 20, 48, fr.right() - side.right() - 40, fr.h - 64);
    ui.cv.push_clip(R::new(body.x - 4, body.y - 4, body.w + 8, body.h + 8));
    match st.stab {
        "color" => tab_color(ui, st, body),
        "bar" => tab_bar(ui, st, body),
        "widgets" => tab_widgets(ui, st, body),
        "look" => tab_look(ui, st, body),
        "wall" => crate::wall::tab(ui, st, body),
        "system" => tab_system(ui, st, body),
        "shots" => tab_shots(ui, st, body),
        "windows" => tab_windows(ui, st, body),
        "dock" => tab_dock(ui, st, body),
        "screens" => crate::displays::tab(ui, st, body),
        "keys" => crate::keys::tab(ui, st, body),
        "games" => crate::games::tab(ui, st, body),
        "fonts" => crate::fonttab::tab(ui, st, body),
        "agents" => crate::agents::tab(ui, st, body),
        "updates" => crate::update::tab(ui, st, body),
        _ => tab_about(ui, st, body),
    }
    ui.cv.pop_clip();
}

pub fn title(ui: &mut Ui, x: i32, y: i32, w: i32, s: &str) -> i32 {
    let t = ui.t;
    ui.cv.fill(R::new(x, y + 7, 6, 6), t.red);
    ui.text_in(PX, R::new(x + 14, y, w - 14, 20), s, t.text);
    ui.cv.fill(R::new(x, y + 24, w, 1), t.border);
    y + 34
}

pub(crate) fn tab_color(ui: &mut Ui, st: &State, b: R) {
    let t = ui.t;
    let s = &st.set;
    let mut y = title(ui, b.x, b.y, b.w, "палитра — весь райс перекрашивается сразу");
    let cw = (b.w - 6 * 8) / 7;
    for (i, p) in PRESETS.iter().enumerate() {
        let r = R::new(b.x + i as i32 * (cw + 8), y, cw, 64);
        let sel = s.accent.is_empty() && s.palette == p.id;
        let hov = ui.hover(r);
        ui.cv.pxbox(r, &BoxStyle::new(if sel { t.selection } else { t.surface }, if sel { t.ember } else if hov { t.red } else { t.border }).bevel(true).shadow(2));
        let sw = R::new(r.x + 8, r.y + 8, r.w - 16, 26);
        let c = C(0xff000000 | p.accent);
        ui.cv.pxbox(sw, &BoxStyle::new(c, c.lerp(WHITE, 0.3)).bevel(true));
        ui.text_center(PX, R::new(r.x, r.y + 38, r.w, 22), p.name, if sel { t.text_hi } else { t.text_dim });
        ui.zone(r, Act::SPreset(i), Cur::Pointer);
    }
    y += 80;
    y = title(ui, b.x, y, b.w, "свой цвет");
    let ok = parse_hex(&st.hex.text).is_some();
    let focused = st.field_focus == Some("hex");
    ui.field(R::new(b.x, y, 200, 30), &st.hex, "#rrggbb", focused, false, Act::SField("hex"));
    if let Some(h) = parse_hex(&st.hex.text) {
        let c = C(0xff000000 | h);
        ui.cv.pxbox(R::new(b.x + 210, y, 30, 30), &BoxStyle::new(c, t.border).bevel(true));
    }
    ui.button(R::new(b.x + 250, y + 2, 110, 26), "применить", "check", Tone::Primary, false, ok, Act::SCycle("hex_apply"));
    ui.text_in(PX, R::new(b.x + 370, y, b.w - 370, 30), if s.accent.is_empty() { "сейчас: пресет" } else { "сейчас: свой" }, t.text_faint);
    y += 46;
    y = title(ui, b.x, y, b.w, "оттенки");
    // по два ползунка в ряд: цвет фона слева, прозрачность справа
    let hw = (b.w - 24) / 2;
    let (lx, rx) = (b.x, b.x + hw + 24);
    let pct = |v: f32| format!("{:.0}%", v * 100.0);
    ui.slider(R::new(lx, y, hw, 40), "насыщенность фона", s.tint / 2.0, &format!("{:.1}", s.tint), 12, Act::SSlider("tint", 0.0, 2.0, 20));
    ui.slider(R::new(rx, y, hw, 40), "стекло панелей", (s.glass - 0.15) / 0.85, &pct(s.glass), 12, Act::SSlider("glass", 0.15, 1.0, 17));
    y += 50;
    ui.slider(R::new(lx, y, hw, 40), "яркость панелей", s.lift, &pct(s.lift), 12, Act::SSlider("lift", 0.0, 1.0, 20));
    ui.slider(R::new(rx, y, hw, 40), "карточки и графики", (s.inner - 0.2) / 0.8, &pct(s.inner), 12, Act::SSlider("inner", 0.2, 1.0, 16));
    y += 50;
    ui.slider(R::new(lx, y, hw, 40), "затемнение под меню", s.dim / 0.8, &pct(s.dim), 12, Act::SSlider("dim", 0.0, 0.8, 16));
    // тема: тёмная или светлая — та же палитра, весь райс вместе с шеллом
    ui.text_in(PX, R::new(rx, y, hw, 20), "тема", t.text);
    let bw = (hw - 8) / 2;
    ui.button(R::new(rx, y + 20, bw, 24), "тёмная", "moon", Tone::Plain, !s.light, true, Act::SCycle("theme_dark"));
    ui.button(R::new(rx + bw + 8, y + 20, bw, 24), "светлая", "sun", Tone::Plain, s.light, true, Act::SCycle("theme_light"));
    y += 54;
    // образец
    let r = R::new(b.x, y, b.w, (b.bottom() - y).min(70));
    if r.h > 30 {
        ui.cv.pxbox(r, &BoxStyle::new(t.glass_panel(), t.border));
        let mut x = r.x + 10;
        for c in [t.bg, t.panel, t.surface, t.surface_hi, t.border, t.dim, t.blood, t.red, t.glow, t.ember, t.text_dim, t.text] {
            ui.cv.pxbox(R::new(x, r.y + 10, 26, r.h - 20), &BoxStyle::new(c, t.border));
            x += 32;
        }
    }
}

/// число в пикселях: ‹ значение › — щелчок по стрелке или колесо меняют на шаг
pub fn num_btn(ui: &mut Ui, r: R, label: &str, v: i32, key: &'static str) {
    let t = ui.t;
    let hov = ui.hover(r);
    ui.cv.pxbox(r, &BoxStyle::new(if hov { t.surface_hi } else { t.surface }, if hov { t.red } else { t.border }).bevel(true));
    ui.text_in(PX, R::new(r.x + 8, r.y, r.w - 80, r.h), label, t.text_dim);
    ui.zone(r, Act::SNum(key, 1), Cur::Pointer);
    let (l, rr) = (R::new(r.right() - 72, r.y + 2, 16, r.h - 4), R::new(r.right() - 18, r.y + 2, 16, r.h - 4));
    for (b, icon) in [(l, "left"), (rr, "right")] {
        ui.cv.icon(icon, b.x + 1, b.cy() - 7, 2, if ui.hover(b) { t.ember } else { t.red });
    }
    ui.zone(l, Act::SNum(key, -1), Cur::Pointer);
    ui.zone(rr, Act::SNum(key, 1), Cur::Pointer);
    ui.cv.text_center(ui.f, PX, R::new(l.right(), r.y, rr.x - l.right(), r.h), &format!("{v}"), t.text);
}

/// кнопка выбора: подпись слева, значение справа; подписи — всё место, что значение не заняло
pub fn opt_btn(ui: &mut Ui, r: R, label: &str, val: &str, act: Act) {
    let t = ui.t;
    let hov = ui.hover(r);
    ui.cv.pxbox(r, &BoxStyle::new(if hov { t.surface_hi } else { t.surface }, if hov { t.red } else { t.border }).bevel(true));
    // значение не наезжает на подпись: длинное — с многоточием, подписи — не меньше трети
    let lw = ui.f.width(PX, (ui.tr)(label)).min(r.w / 3);
    let room = r.w - 32 - lw - 12;
    let tv = (ui.tr)(val).to_string();
    let v = if ui.f.width(PX, &tv) > room { ui.f.elide(PX, &tv, room) } else { tv };
    let vw = ui.f.width(PX, &v);
    ui.text_in(PX, R::new(r.x + 8, r.y, r.w - 32 - vw - 12, r.h), label, t.text_dim);
    ui.cv.text_right(ui.f, PX, R::new(r.x, r.y, r.w - 24, r.h), &v, t.text);
    ui.cv.icon("right", r.right() - 18, r.cy() - 7, 2, t.red);
    ui.zone(r, act, Cur::Pointer);
}

pub(crate) fn tab_widgets(ui: &mut Ui, st: &State, b: R) {
    let t = ui.t;
    let s = &st.set;
    let y0 = title(ui, b.x, b.y, b.w, "виджеты рабочего стола");
    // список слева
    let lw = 200;
    let mut y = y0;
    for w in &s.widgets {
        let r = R::new(b.x, y, lw, 28);
        let sel = st.sel_widget == w.id;
        ui.button(r, "", "", Tone::Ghost, sel, true, Act::SWidgetSel(w.id));
        let c = if w.on { t.ok } else { t.text_faint };
        ui.cv.fill(R::new(r.x + 8, r.cy() - 3, 6, 6), c);
        let extra = if w.kind == "graph" { format!(" · {}", crate::i18n::tr(name_of(SOURCES, &w.source))) } else { String::new() };
        ui.text_in(PX, R::new(r.x + 22, r.y, r.w - 26, r.h), &format!("{}{}", crate::i18n::tr(name_of(KINDS, &w.kind)), extra), if sel { t.text_hi } else if w.on { t.text } else { t.text_faint });
        y += 30;
    }
    y += 8;
    ui.text_in(PX, R::new(b.x, y, lw, 20), "добавить:", t.text_dim);
    y += 22;
    for (i, (k, n)) in KINDS.iter().enumerate() {
        let r = R::new(b.x + (i as i32 % 2) * (lw / 2 + 2), y + (i as i32 / 2) * 28, lw / 2 - 2, 24);
        ui.button(r, n, "", Tone::Plain, false, true, Act::SWidgetAdd(k));
    }
    // параметры справа
    let px = b.x + lw + 20;
    let pw = b.right() - px;
    let Some(w) = s.widgets.iter().find(|w| w.id == st.sel_widget) else {
        ui.text_in(PX, R::new(px, y0, pw, 20), "выбери виджет слева", t.text_faint);
        return;
    };
    let id = w.id;
    let mut y = y0;
    ui.toggle(R::new(px, y, pw, 26), "показывать", "", w.on, Act::SWidgetOpt(id, "on"));
    y += 34;
    let row = |ui: &mut Ui, y: &mut i32, label: &str, val: &str, key: &'static str| {
        opt_btn(ui, R::new(px, *y, pw, 26), label, val, Act::SWidgetOpt(id, key));
        *y += 30;
    };
    match w.kind.as_str() {
        "graph" => {
            row(ui, &mut y, "источник", name_of(SOURCES, &w.source), "source");
            row(ui, &mut y, "вид графика", name_of(STYLES, &w.style), "style");
            row(ui, &mut y, "анимация", name_of(ANIMS, &w.anim), "anim");
        }
        "net" => {
            row(ui, &mut y, "вид графика", name_of(STYLES, &w.style), "style");
            row(ui, &mut y, "анимация", name_of(ANIMS, &w.anim), "anim");
        }
        "media" => row(ui, &mut y, "эквалайзер", name_of(STYLES, &w.style), "style"),
        _ => {}
    }
    row(ui, &mut y, "размер", name_of(SIZES, &w.size), "size");
    row(ui, &mut y, "цвет", name_of(COLORS, &w.color), "color");
    y += 4;
    ui.toggle(R::new(px, y, pw / 2, 26), "подложка", "", w.frame, Act::SWidgetOpt(id, "frame"));
    if matches!(w.kind.as_str(), "graph" | "net" | "media") {
        ui.toggle(R::new(px + pw / 2, y, pw / 2, 26), "подпись", "", w.label, Act::SWidgetOpt(id, "label"));
    }
    if w.kind == "clock" {
        ui.toggle(R::new(px + pw / 2, y, pw / 4, 26), "секунды", "", w.seconds, Act::SWidgetOpt(id, "seconds"));
        ui.toggle(R::new(px + pw * 3 / 4, y, pw / 4, 26), "дата", "", w.date, Act::SWidgetOpt(id, "date"));
    }
    y += 40;
    // живое превью графика
    if matches!(w.kind.as_str(), "graph" | "net") {
        let r = R::new(px, y, pw, 70);
        ui.cv.pxbox(r, &BoxStyle::new(t.code_bg, t.border));
        let (ser, full) = if w.kind == "net" { (&st.sys.h_rx, st.sys.h_rx.max().max(65536.0)) } else { st.sys.series(&w.source) };
        let data: Vec<f32> = ser.tail(160).iter().map(|v| v / full).collect();
        widgets::graph(ui, r.inset(6), &data, &w.style, &w.anim, st.sys_phase(), widgets::color(&t, &w.color), None);
        y += 80;
    }
    let bw = (pw - 12) / 3;
    ui.button(R::new(px, y, bw, 28), "расставить", "drag", Tone::Primary, false, true, Act::SEdit);
    ui.button(R::new(px + bw + 6, y, bw, 28), "на место", "reboot", Tone::Plain, false, true, Act::SResetPos);
    ui.button(R::new(px + 2 * (bw + 6), y, bw, 28), "убрать", "trash", Tone::Danger, false, true, Act::SWidgetDel(id));
}

pub(crate) fn tab_look(ui: &mut Ui, st: &State, b: R) {
    let s = &st.set;
    let half = (b.w - 20) / 2;
    let mut y = title(ui, b.x, b.y, b.w, "анимации");
    ui.toggle(R::new(b.x, y, half, 40), "анимации", "ступенчатое появление панелей", s.animations, Act::SBool("animations"));
    ui.toggle(R::new(b.x + half + 20, y, half, 40), "анимация закрытия", "иначе — исчезают сразу", s.anim_close, Act::SBool("anim_close"));
    y += 50;
    ui.slider(R::new(b.x, y, b.w, 40), "скорость анимаций (меньше — быстрее)", (s.anim_speed - 0.3) / 1.7, &format!("×{:.1}", s.anim_speed), 17, Act::SSlider("anim_speed", 0.3, 2.0, 17));
    y += 50;
    // узор проявления и крупность клеток — во всех программах yami; развёртка — только шелл
    opt_btn(ui, R::new(b.x, y, half, 26), "появление", reveal_name(&s.reveal), Act::SCycle("reveal"));
    num_btn(ui, R::new(b.x + half + 20, y, half, 26), "размер клеток", s.num("reveal_grain"), "reveal_grain");
    y += 34;
    ui.toggle(R::new(b.x, y, b.w, 26), "строки развёртки на панелях шелла (как на старом мониторе)", "", s.scanlines, Act::SBool("scanlines"));
    y += 38;
    y = title(ui, b.x, y, b.w, "размытие (niri)");
    ui.toggle(R::new(b.x, y, half, 26), "под панелями шелла", "", s.blur_shell, Act::SBool("blur_shell"));
    ui.toggle(R::new(b.x + half + 20, y, half, 26), "под терминалами", "", s.blur_term, Act::SBool("blur_term"));
    y += 40;
    y = title(ui, b.x, y, b.w, "значки");
    let it = if s.icon_theme.is_empty() { "авто" } else { &s.icon_theme };
    opt_btn(ui, R::new(b.x, y, half, 26), "значки программ", it, Act::SCycle("icon_theme"));
    ui.toggle(R::new(b.x + half + 20, y, half, 26), "пиксельные, в цветах палитры", "", s.icon_pixel, Act::SBool("icon_pixel"));
    y += 40;
    y = title(ui, b.x, y, b.w, "курсор");
    cursor_block(ui, st, R::new(b.x, y, b.w, 58));
}

/// подпись узора проявления (gfx::Reveal)
pub(crate) fn reveal_name(id: &str) -> &'static str {
    match id {
        "scan" => "развёртка",
        "noise" => "шум",
        "wipe" => "диагональ",
        "blocks" => "блоки",
        _ => "дизеринг",
    }
}

pub(crate) fn tab_system(ui: &mut Ui, st: &State, b: R) {
    let s = &st.set;
    let half = (b.w - 20) / 2;
    let (lx, rx) = (b.x, b.x + half + 20);
    let mut y = title(ui, b.x, b.y, b.w, "простой");
    let mins = |m: u32| if m == 0 { "никогда".to_string() } else { crate::tf!("{m} мин", "{m} min") };
    ui.slider(R::new(lx, y, half, 40), "заблокировать через", s.lock_after as f32 / 60.0, &mins(s.lock_after), 12, Act::SSlider("lock_after", 0.0, 60.0, 20));
    ui.slider(R::new(rx, y, half, 40), "погасить экран через", s.screen_off_after as f32 / 60.0, &mins(s.screen_off_after), 12, Act::SSlider("screen_off_after", 0.0, 60.0, 20));
    y += 52;
    y = title(ui, b.x, y, b.w, "уведомления");
    ui.toggle(R::new(lx, y, half, 40), "не беспокоить", "только срочные", s.dnd, Act::SBool("dnd"));
    ui.slider(R::new(rx, y, half, 40), "сколько висит", (s.notif_timeout as f32 - 2.0) / 28.0, &crate::tf!("{} с", "{} s", s.notif_timeout), 12, Act::SSlider("notif_timeout", 2.0, 30.0, 14));
    y += 52;
    // ночной свет: wlsunset; по координатам — от заката до восхода, иначе по часам
    y = title(ui, b.x, y, b.w, "ночной свет: по солнцу или по часам");
    let has = crate::doctor::which("wlsunset");
    ui.toggle(R::new(lx, y, half, 40), "теплее вечером", if has { "wlsunset" } else { "нужен wlsunset" }, s.night, Act::SBool("night"));
    num_btn(ui, R::new(rx, y + 6, half, 26), "температура, K", s.num("night_temp"), "night_temp");
    y += 46;
    // координаты есть — по закату и восходу, часы тогда не нужны
    let gf = st.field_focus == Some("night_geo");
    let by_sun = crate::night::geo(&s.night_geo).is_some();
    ui.field(R::new(lx, y, half, 26), &st.geo_field, if s.night_geo.is_empty() { "широта, долгота — по солнцу" } else { &s.night_geo }, gf, false, Act::SField("night_geo"));
    if by_sun {
        ui.text_in(PX, R::new(rx + 8, y, half - 8, 26), "по закату и восходу", ui.t.text_faint);
    } else {
        let qw = (half - 8) / 2;
        num_btn(ui, R::new(rx, y, qw, 26), "с", s.num("night_on_h"), "night_on_h");
        num_btn(ui, R::new(rx + qw + 8, y, qw, 26), "до", s.num("night_off_h"), "night_off_h");
    }
    y += 40;
    y = title(ui, b.x, y, b.w, "язык и вход");
    let lang = match s.lang.as_str() {
        "ru" => "Русский", // i18n-ok: name of the language
        "en" => "English",
        _ => "авто",
    };
    opt_btn(ui, R::new(lx, y, half, 26), "язык", lang, Act::SCycle("lang"));
    ui.toggle(R::new(rx, y, half, 26), "вернуть программы при входе", "", s.session_restore, Act::SBool("session_restore"));
    y += 40;
    y = title(ui, b.x, y, b.w, "быстро");
    let bw = (b.w - 18) / 4;
    for (i, (label, icon, act)) in [("заблокировать", "lock", Act::SCycle("do_lock")), ("микшер", "speaker", Act::Pavu), ("процессы", "cpu", Act::SysMon), ("перечитать", "reboot", Act::SCycle("reload"))].into_iter().enumerate() {
        ui.button(R::new(b.x + i as i32 * (bw + 6), y, bw, 28), label, icon, Tone::Plain, false, true, act);
    }
    y += 34;
    // для тех, кто правит руками: файл открывается в редакторе, справка — yami-shell config
    ui.button(R::new(b.x, y, bw * 2 + 6, 28), "файл настроек", "pen", Tone::Plain, false, true, Act::SCycle("open_config"));
    ui.text_in(PX, R::new(b.x + bw * 2 + 18, y, b.w - bw * 2 - 18, 28), "или: yami-shell config list", ui.t.text_faint);
}

pub(crate) fn tab_dock(ui: &mut Ui, st: &State, b: R) {
    let s = &st.set;
    let t = ui.t;
    let half = (b.w - 20) / 2;
    let mut y = title(ui, b.x, b.y, b.w, "док");
    ui.toggle(R::new(b.x, y, half, 40), "показывать док", "закреплённое и открытые окна", s.dock, Act::SBool("dock"));
    ui.toggle(R::new(b.x + half + 20, y, half, 40), "прыжок при запуске", "значок подскакивает ступенями", s.dock_bounce, Act::SBool("dock_bounce"));
    y += 46;
    ui.toggle(R::new(b.x, y, half, 40), "открытые программы", "не только закреплённые", s.dock_running, Act::SBool("dock_running"));
    ui.toggle(R::new(b.x + half + 20, y, half, 40), "подписи", "имя или заголовок окна", s.dock_labels, Act::SBool("dock_labels"));
    y += 46;
    ui.toggle(R::new(b.x, y, half, 40), "кнопка программ", "в начале дока", s.dock_launcher, Act::SBool("dock_launcher"));
    ui.toggle(R::new(b.x + half + 20, y, half, 40), "корзина", "в конце дока", s.dock_trash, Act::SBool("dock_trash"));
    y += 48;
    let side = match s.dock_side.as_str() {
        "top" => "сверху",
        "left" => "слева",
        "right" => "справа",
        _ => "снизу",
    };
    let hide = match s.dock_hide.as_str() {
        "never" => "никогда (занимает место)",
        "always" => "всегда — выезжает у края",
        _ => "умно — виден на пустом столе",
    };
    opt_btn(ui, R::new(b.x, y, b.w, 26), "край экрана", side, Act::SCycle("dock_side"));
    y += 32;
    opt_btn(ui, R::new(b.x, y, b.w, 26), "прятать", hide, Act::SCycle("dock_hide"));
    y += 32;
    let anim = match s.dock_anim.as_str() {
        "wave" => "волна — соседи тоже растут",
        "lift" => "подъём без роста",
        "none" => "нет",
        _ => "рост и подъём",
    };
    opt_btn(ui, R::new(b.x, y, b.w, 26), "при наведении", anim, Act::SCycle("dock_anim"));
    y += 34;
    ui.slider(R::new(b.x, y, b.w, 40), "размер значков", (s.dock_size as f32 - 24.0) / 40.0, &format!("{} px", s.dock_size), 10, Act::SSlider("dock_size", 24.0, 64.0, 10));
    y += 50;
    y = title(ui, b.x, y, b.w, "закреплённые");
    let mut x = b.x;
    for id in s.dock_pins.iter() {
        let Some(a) = st.apps.iter().find(|a| &a.id == id) else { continue };
        let r = R::new(x, y, 30, 30);
        if x + 30 > b.right() {
            break;
        }
        let hov = ui.hover(r);
        ui.cv.pxbox(r, &BoxStyle::new(if hov { t.surface_hi } else { t.surface }, if hov { t.red } else { t.border }));
        ui.app_icon(&a.icon, r.x + 3, r.y + 3, 24, 1.0);
        ui.zone(r, Act::DockPin(id.clone(), 0), Cur::Pointer);
        x += 36;
    }
    ui.text_in(PX, R::new(b.x, y + 36, b.w, 18), "щелчок — убрать · порядок и закрепление — ПКМ по значку в доке", t.text_faint);
}

pub(crate) fn tab_windows(ui: &mut Ui, st: &State, b: R) {
    let s = &st.set;
    let half = (b.w - 20) / 2;
    // отступы окон: niri gaps/struts в yami-fx.kdl (применяются сразу)
    let mut y = title(ui, b.x, b.y, b.w, "отступы окон: от краёв (сверху — от бара) и между");
    let cw = (b.w - 24) / 3;
    let col = |i: i32| b.x + i * (cw + 12);
    for (i, (k, label)) in [("win_left", "слева"), ("win_right", "справа"), ("win_gap", "между"), ("win_top", "сверху"), ("win_bottom", "снизу")].into_iter().enumerate() {
        let i = i as i32;
        num_btn(ui, R::new(col(i % 3), y + i / 3 * 32, cw, 26), label, s.num(k), k);
    }
    y += 66;
    y = title(ui, b.x, y, b.w, "плавающие окна");
    ui.toggle(R::new(b.x, y, half, 40), "фото и видео", "просмотр в Telegram, yami-view, mpv…", s.float_media, Act::SBool("float_media"));
    ui.toggle(R::new(b.x + half + 20, y, half, 40), "картинка в картинке", "в правом нижнем углу", s.float_pip, Act::SBool("float_pip"));
    y += 52;
    ui.slider(R::new(b.x, y, b.w, 40), "ширина", (s.float_w - 0.3) / 0.65, &format!("{:.0}%", s.float_w * 100.0), 13, Act::SSlider("float_w", 0.3, 0.95, 13));
    y += 46;
    ui.slider(R::new(b.x, y, b.w, 40), "высота", (s.float_h - 0.3) / 0.65, &format!("{:.0}%", s.float_h * 100.0), 13, Act::SSlider("float_h", 0.3, 0.95, 13));
    y += 56;
    y = title(ui, b.x, y, b.w, "свои программы");
    let focused = st.field_focus == Some("float_apps");
    ui.field(R::new(b.x, y, b.w - 130, 30), &st.float_field, "app-id через запятую: org.telegram.desktop, imv", focused, false, Act::SField("float_apps"));
    ui.button(R::new(b.right() - 120, y + 2, 120, 26), "применить", "check", Tone::Primary, false, true, Act::SCycle("float_apply"));
    y += 40;
    let t = ui.t;
    ui.text_in(PX, R::new(b.x, y, b.w, 20), "app-id окна: niri msg windows · правила пишутся в niri/yami-fx.kdl", t.text_faint);
    y += 28;
    y = title(ui, b.x, y, b.w, "режим «стол»");
    ui.toggle(R::new(b.x, y, half, 40), "окна плавают, как в DE", "Mod+Shift+Space — назад к ленте", s.stacking, Act::SBool("stacking"));
    ui.toggle(R::new(b.x + half + 20, y, half, 40), "заголовки окон", "полоса с кнопками над окном", s.titlebars, Act::SBool("titlebars"));
}

/// курсор: тема и размер слева, предпросмотр справа (та же стрелка, что рисует tools/theme.py)
fn cursor_block(ui: &mut Ui, st: &State, b: R) {
    let s = &st.set;
    let t = ui.t;
    let y = b.y;
    let kind = match s.cursor.as_str() {
        "system" => "системный",
        _ => "пиксельный yami — в цвет палитры",
    };
    let pw = 64;
    opt_btn(ui, R::new(b.x, y, b.w - pw - 12, 26), "тема", kind, Act::SCycle("cursor"));
    opt_btn(ui, R::new(b.x, y + 32, b.w - pw - 12, 26), "размер", &format!("{} px", s.cursor_size), Act::SCycle("cursor_size"));
    // предпросмотр: та же стрелка, что рисует tools/theme.py
    let pr = R::new(b.right() - pw, y, pw, 58);
    ui.cv.pxbox(pr, &BoxStyle::new(t.code_bg, t.border));
    if s.cursor == "yami" {
        const ARROW: [&str; 12] = ["o...........", "oo..........", "obo.........", "oaao........", "oaaao.......", "oaaaao......", "oaaaaao.....", "oaaaaaao....", "oaaaoooo....", "oaoao.......", "oo.oao......", "o...oo......"];
        let k = 4;
        let (ox, oy) = (pr.x + (pw - 12 * k) / 2 + 6, pr.y + 5);
        for (j, row) in ARROW.iter().enumerate() {
            for (i, ch) in row.bytes().enumerate() {
                let c = match ch {
                    b'o' => t.bg.lerp(C(0xff000000), 0.5),
                    b'a' => t.red,
                    b'b' => t.ember,
                    _ => continue,
                };
                ui.cv.fill(R::new(ox + i as i32 * k, oy + j as i32 * k, k, k), c);
            }
        }
    } else {
        ui.cv.icon("cursor", pr.x + (pw - 28) / 2, pr.y + 15, 4, t.text_dim);
    }
}

pub(crate) fn tab_shots(ui: &mut Ui, st: &State, b: R) {
    let s = &st.set;
    let half = (b.w - 20) / 2;
    let mut y = title(ui, b.x, b.y, b.w, "куда");
    ui.toggle(R::new(b.x, y, half, 40), "в буфер обмена", "сразу вставляй куда угодно", s.shot_copy, Act::SBool("shot_copy"));
    ui.toggle(R::new(b.x + half + 20, y, half, 40), "в папку", "файл png с датой в имени", s.shot_save, Act::SBool("shot_save"));
    y += 50;
    let focused = st.field_focus == Some("shot_dir");
    let shown = crate::snap::shots_dir(s).display().to_string().replace(&crate::config::home().to_string_lossy().to_string(), "~");
    ui.field(R::new(b.x, y, b.w - 130, 30), &st.shot_field, &shown, focused, false, Act::SField("shot_dir"));
    ui.button(R::new(b.right() - 120, y + 2, 120, 26), "применить", "check", Tone::Primary, false, true, Act::SCycle("shot_dir_apply"));
    y += 46;
    y = title(ui, b.x, y, b.w, "оверлей");
    ui.toggle(R::new(b.x, y, half, 40), "лупа", "пиксели под курсором, ctrl+c — цвет", s.shot_loupe, Act::SBool("shot_loupe"));
    ui.toggle(R::new(b.x + half + 20, y, half, 40), "уведомление", "с миниатюрой и кнопками", s.shot_notify, Act::SBool("shot_notify"));
    y += 50;
    let d = crate::tf!("{} с", "{} s", s.shot_delay);
    opt_btn(ui, R::new(b.x, y, b.w, 26), "таймер (кнопка ◷ в оверлее)", &d, Act::SCycle("shot_delay"));
    y += 40;
    // шторка: зрители трансляции видят её, ты — экран (curtain.rs)
    y = title(ui, b.x, y, b.w, "скрыть экран в трансляции");
    opt_btn(ui, R::new(b.x, y, b.w, 26), "шторка", crate::curtain::style_name(&s.curtain_style), Act::SCycle("curtain_style"));
    y += 34;
    ui.slider(R::new(b.x, y, b.w, 40), "размер пикселей шторки", (s.curtain_cell as f32 - 8.0) / 56.0, &format!("{} px", s.curtain_cell), 14, Act::SSlider("curtain_cell", 8.0, 64.0, 14));
    y += 46;
    let t = ui.t;
    if !st.niri.cast_only {
        ui.text_in(PX, R::new(b.x, y, b.w, 20), "этот niri не умеет шторку: окна в трансляции станут чёрными", t.warn);
        y += 24;
    }
    y = title(ui, b.x, y, b.w, "клавиши");
    for (k, v) in [("print", "область · окно · экран"), ("mod+shift+h", "скрыть экран в трансляции"), ("в оверлее", "1–7 инструменты · tab режим · enter готово · ctrl+s сохранить как")] {
        ui.text_in(PX, R::new(b.x, y, 130, 20), k, t.red);
        ui.text_in(PX, R::new(b.x + 140, y, b.w - 140, 20), v, t.text_dim);
        y += 22;
    }
}

pub(crate) fn tab_about(ui: &mut Ui, st: &State, b: R) {
    let t = ui.t;
    let mut y = b.y;
    let face = font::px(32);
    ui.text(face, b.x, y, "YAMI", t.red);
    y += ui.f.line(face) + 4;
    ui.text_in(PX, R::new(b.x, y, b.w, 20), &crate::tf!("yami-shell {} — лёгкий Wayland-шелл для niri на Rust", "yami-shell {} — a lightweight Wayland shell for niri, written in Rust", env!("CARGO_PKG_VERSION")), t.text_dim);
    y += 34;
    let up = st.started.elapsed().as_secs();
    let rows = [
        ("память шелла", crate::tf!("{:.1} МБ", "{:.1} MB", st.rss)),
        ("работает", crate::tf!("{} ч {} мин", "{} h {} min", up / 3600, up / 60 % 60)),
        ("niri", if st.niri.yami { "yami-niri (форк)".to_string() } else { "стоковый".to_string() }),
        ("окон", st.niri.wins.len().to_string()),
        ("уведомлений в истории", st.notifs.history.len().to_string()),
        ("настройки", "~/.config/yami-shell.json".to_string()),
        ("команды", "yami-shell msg launcher|power|settings|lock|…".to_string()),
    ];
    for (k, v) in rows {
        ui.text_in(PX, R::new(b.x, y, 200, 22), k, t.text_faint);
        ui.text_in(PX, R::new(b.x + 210, y, b.w - 210, 22), &v, t.text);
        y += 26;
    }
    y += 10;
    // шкала памяти: шелл против бюджета 64 МБ
    ui.cv.gauge(R::new(b.x, y, b.w, 12), 32, (st.rss / 64.0).min(1.0), t.red, t.surface.lerp(t.border, 0.3), 2);
}

/// установленные темы значков (с index.theme и не только курсоры)
pub(crate) fn icon_themes() -> Vec<String> {
    let mut v = vec![String::new()];
    let home = crate::config::home();
    for d in [home.join(".local/share/icons"), home.join(".icons"), "/usr/share/icons".into()] {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        let mut names: Vec<String> = rd
            .flatten()
            .filter(|e| {
                let p = e.path();
                p.join("index.theme").exists() && !p.join("cursors").exists() && e.file_name() != "default" && e.file_name() != "hicolor"
            })
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        for n in names {
            if !v.contains(&n) {
                v.push(n);
            }
        }
    }
    v
}

/// следующее значение из ряда (после последнего — первое)
pub fn step_in(list: &[i32], cur: i32) -> i32 {
    list.iter().copied().find(|v| *v > cur).unwrap_or(list[0])
}

pub(crate) fn cycle<'a>(list: &[(&'a str, &'a str)], cur: &str) -> &'a str {
    let i = list.iter().position(|x| x.0 == cur).map(|i| (i + 1) % list.len()).unwrap_or(0);
    list[i].0
}

impl App {
    /// клики внутри настроек; true — обработано
    pub fn settings_click(&mut self, a: &Act) -> bool {
        let s = &mut self.st.set;
        match a {
            Act::STab(id) => {
                self.st.stab = id;
                self.st.field_focus = None;
                self.keys_capture(None);
                if *id == "screens" {
                    self.disp_open();
                }
                if *id == "games" {
                    self.st.game_probe = crate::gameboost::probe();
                }
            }
            Act::SPreset(i) => {
                s.palette = PRESETS[*i].id.into();
                s.accent.clear();
                // цвет выбран руками: обои его больше не перекрашивают
                s.wall_accent = false;
                self.st.hex.set(&format!("#{:06x}", PRESETS[*i].accent));
                self.settings_changed();
            }
            Act::SNum(k, d) => {
                s.step_num(k, *d);
                self.settings_changed();
            }
            Act::SBool(k) => {
                match *k {
                    "animations" => s.animations = !s.animations,
                    "anim_close" => s.anim_close = !s.anim_close,
                    "scanlines" => s.scanlines = !s.scanlines,
                    "icon_pixel" => s.icon_pixel = !s.icon_pixel,
                    "stacking" => {
                        s.stacking = !s.stacking;
                        let on = s.stacking;
                        self.st.niri.set_stacking(on);
                        self.sync_titles();
                    }
                    "titlebars" => {
                        s.titlebars = !s.titlebars;
                        self.sync_titles();
                    }
                    "wall_kanji" => s.wall_kanji = !s.wall_kanji,
                    "bar_float" => s.bar_float = !s.bar_float,
                    "blur_shell" => s.blur_shell = !s.blur_shell,
                    "blur_term" => s.blur_term = !s.blur_term,
                    "clock_seconds" => s.clock_seconds = !s.clock_seconds,
                    "clock_date" => s.clock_date = !s.clock_date,
                    "dnd" => s.dnd = !s.dnd,
                    "shot_copy" => s.shot_copy = !s.shot_copy,
                    "float_media" => s.float_media = !s.float_media,
                    "dock" => s.dock = !s.dock,
                    "dock_bounce" => s.dock_bounce = !s.dock_bounce,
                    "dock_running" => s.dock_running = !s.dock_running,
                    "dock_labels" => s.dock_labels = !s.dock_labels,
                    "dock_launcher" => s.dock_launcher = !s.dock_launcher,
                    "dock_trash" => s.dock_trash = !s.dock_trash,
                    "float_pip" => s.float_pip = !s.float_pip,
                    "shot_save" => s.shot_save = !s.shot_save,
                    "shot_loupe" => s.shot_loupe = !s.shot_loupe,
                    "shot_notify" => s.shot_notify = !s.shot_notify,
                    "game_fullscreen" => s.game_fullscreen = !s.game_fullscreen,
                    "game_vrr" => s.game_vrr = !s.game_vrr,
                    "game_tearing" => s.game_tearing = !s.game_tearing,
                    "game_cache" => s.game_cache = !s.game_cache,
                    "game_dnd" => s.game_dnd = !s.game_dnd,
                    "game_profile" => s.game_profile = !s.game_profile,
                    "game_ntsync" => s.game_ntsync = !s.game_ntsync,
                    "game_gamemode" => s.game_gamemode = !s.game_gamemode,
                    "night" => s.night = !s.night,
                    "session_restore" => s.session_restore = !s.session_restore,
                    "update_check" => s.update_check = !s.update_check,
                    "fetch_full" => {
                        // theme.py читает настройки с диска: сохранить, потом перерисовать fastfetch
                        s.fetch_full = !s.fetch_full;
                        self.settings_changed();
                        self.save_now();
                        std::thread::spawn(|| {
                            let _ = std::process::Command::new("python3").arg("-I").arg(crate::app::theme_script()).arg("--fetch").stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
                        });
                        self.dirty(&SKind::Modal);
                        return true;
                    }
                    k if crate::agents::toggle(s, k) => {}
                    "browser_menus" | "browser_font" => {
                        if *k == "browser_menus" {
                            s.browser_menus = !s.browser_menus;
                        } else {
                            s.browser_font = !s.browser_font;
                        }
                        self.settings_changed();
                        self.apply_fonts();
                        self.dirty(&SKind::Modal);
                        return true;
                    }
                    _ => {}
                }
                self.settings_changed();
            }
            Act::SCycle(k) if k.starts_with("font_") && *k != "font_size" => {
                let k: &str = k;
                return self.font_cycle(k);
            }
            Act::SCycle(k) => match *k {
                "cpu_style" => {
                    s.cpu_style = cycle(&[("gauge", ""), ("graph", ""), ("number", "")], &s.cpu_style).into();
                    self.settings_changed();
                }
                "bar_page_mods" | "bar_page_look" => {
                    self.st.bar_ed.look = *k == "bar_page_look";
                    self.dirty(&SKind::Modal);
                }
                "bar_center_align" => {
                    s.bar_center_align = if s.bar_center_align == "after-left" { "center" } else { "after-left" }.into();
                    self.settings_changed();
                }
                "bar_sep" => {
                    s.bar_sep = cycle(&[("dots", ""), ("line", ""), ("blocks", ""), ("gap", "")], &s.bar_sep).into();
                    self.settings_changed();
                }
                "bar_sep_color" => {
                    s.bar_sep_color = cycle(&[("border", ""), ("accent", ""), ("dim", "")], &s.bar_sep_color).into();
                    self.settings_changed();
                }
                "icon_theme" => {
                    let list = icon_themes();
                    let i = list.iter().position(|x| *x == s.icon_theme).map(|i| (i + 1) % list.len()).unwrap_or(0);
                    s.icon_theme = list[i].clone();
                    self.res.icons = crate::icons::Icons::new(&self.st.set.icon_theme);
                    self.res.icons.set_pixel(self.st.set.icon_pixel.then(|| self.st.t.icon_ramp()));
                    self.settings_changed();
                }
                "reveal" => {
                    s.reveal = cycle(&[("dither", ""), ("scan", ""), ("noise", ""), ("wipe", ""), ("blocks", "")], &s.reveal).into();
                    self.settings_changed();
                }
                "dock_anim" => {
                    s.dock_anim = cycle(&[("grow", ""), ("wave", ""), ("lift", ""), ("none", "")], &s.dock_anim).into();
                    self.settings_changed();
                }
                "font_size" => {
                    s.font_size = match s.font_size {
                        0 => 16,
                        16 => 24,
                        24 => 32,
                        _ => 0,
                    };
                    self.settings_changed();
                    self.apply_fonts();
                }
                "hex_apply" => self.apply_hex(),
                "theme_dark" | "theme_light" => {
                    s.light = *k == "theme_light";
                    self.settings_changed();
                }
                "lang" => {
                    s.lang = match s.lang.as_str() {
                        "" => "en",
                        "en" => "ru",
                        _ => "",
                    }
                    .into();
                    self.settings_changed();
                }
                "wall_apply" => {
                    let p = self.st.wall_field.text.clone();
                    self.set_wall(&p);
                }
                "curtain_style" => {
                    s.curtain_style = crate::curtain::next_style(&s.curtain_style).into();
                    self.settings_changed();
                }
                "shot_delay" => {
                    s.shot_delay = match s.shot_delay {
                        3 => 5,
                        5 => 10,
                        _ => 3,
                    };
                    self.settings_changed();
                }
                "cursor" => {
                    s.cursor = match s.cursor.as_str() {
                        "system" => "yami",
                        _ => "system",
                    }
                    .into();
                    self.settings_changed();
                    self.apply_cursor();
                }
                "cursor_size" => {
                    s.cursor_size = match s.cursor_size {
                        24 => 32,
                        32 => 48,
                        48 => 64,
                        _ => 24,
                    };
                    self.settings_changed();
                    self.apply_cursor();
                }
                "dock_side" => {
                    s.dock_side = match s.dock_side.as_str() {
                        "bottom" => "left",
                        "left" => "right",
                        "right" => "top",
                        _ => "bottom",
                    }
                    .into();
                    // край сменился — пересоздать
                    for k in [SKind::Dock, SKind::DockEdge] {
                        for id in self.find(&k) {
                            self.destroy(id);
                        }
                    }
                    self.settings_changed();
                }
                "dock_hide" => {
                    let s = &mut self.st.set;
                    s.dock_hide = match s.dock_hide.as_str() {
                        "smart" => "always",
                        "always" => "never",
                        _ => "smart",
                    }
                    .into();
                    for k in [SKind::Dock, SKind::DockEdge] {
                        for id in self.find(&k) {
                            self.destroy(id);
                        }
                    }
                    self.settings_changed();
                }
                "kb_apply" => {
                    self.st.set.kb_layout = self.st.keys.lay.text.trim().to_string();
                    self.st.keys.lay.clear();
                    self.st.field_focus = None;
                    self.settings_changed();
                }
                "kb_switch" => {
                    let sw = crate::keys::SWITCHES;
                    let i = sw.iter().position(|x| x.0 == self.st.set.kb_switch).map(|i| (i + 1) % sw.len()).unwrap_or(0);
                    self.st.set.kb_switch = sw[i].0.to_string();
                    self.settings_changed();
                }
                "geo_apply" => {
                    let g = self.st.geo_field.text.trim().to_string();
                    // пусто — по часам; иначе только верные координаты
                    if g.is_empty() || crate::night::geo(&g).is_some() {
                        self.st.set.night_geo = g;
                        self.st.field_focus = None;
                        self.settings_changed();
                    }
                }
                "game_apply" => {
                    let p = self.st.game_field.text.trim().to_string();
                    self.st.set.game_apps = p;
                    self.st.field_focus = None;
                    self.settings_changed();
                }
                "float_apply" => {
                    let p = self.st.float_field.text.trim().to_string();
                    self.st.set.float_apps = p;
                    self.st.field_focus = None;
                    self.settings_changed();
                }
                "shot_dir_apply" => {
                    let p = self.st.shot_field.text.trim().to_string();
                    self.st.set.shot_dir = p;
                    self.st.field_focus = None;
                    self.settings_changed();
                }
                "do_lock" => {
                    self.close_modal();
                    self.lock_now();
                }
                "reload" => self.ipc("reload"),
                k if self.update_cycle(k) || self.agents_cycle(k) => {}
                "open_config" => {
                    // на диске должно быть то, что сейчас в настройках
                    let j = self.st.set.to_json();
                    let _ = crate::config::write_atomic(&crate::config::settings_path(), &j);
                    self.set_saved = j;
                    let _ = std::process::Command::new("xdg-open").arg(crate::config::settings_path()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().map(|mut c| std::thread::spawn(move || c.wait()));
                }
                _ => {}
            },
            Act::SModDel(si, i) => {
                let l = s.side_mut(*si);
                if *i < l.len() {
                    l.remove(*i);
                }
                self.settings_changed();
            }
            Act::SModNew(m) => {
                // клик (не перетаскивание): в привычное место
                let m = m.to_string();
                let m = &m;
                if m == "tasks" {
                    s.bar_center.push(m.clone());
                } else if matches!(m.as_str(), "launcher" | "workspaces" | "mode") {
                    s.bar_left.push(m.clone());
                } else {
                    // справа — перед настройками и питанием
                    let at = s.bar_right.iter().position(|x| x == "settings" || x == "power").unwrap_or(s.bar_right.len());
                    s.bar_right.insert(at, m.clone());
                }
                self.settings_changed();
            }
            Act::SWidgetSel(id) => self.st.sel_widget = *id,
            Act::SWidgetAdd(k) => {
                let id = s.next_widget_id();
                let mut w = Widget::new(id, k);
                if *k == "net" {
                    w.style = "line".into();
                }
                if *k == "media" {
                    w.style = "bars".into();
                }
                s.widgets.push(w);
                self.st.sel_widget = id;
                self.settings_changed();
            }
            Act::SWidgetDel(id) => {
                s.widgets.retain(|w| w.id != *id);
                self.st.sel_widget = self.st.set.widgets.first().map(|w| w.id).unwrap_or(0);
                self.settings_changed();
            }
            Act::SWidgetOpt(id, key) => {
                if let Some(w) = s.widget_mut(*id) {
                    let v = match *key {
                        "source" => cycle(SOURCES, &w.source).to_string(),
                        "style" => cycle(STYLES, &w.style).to_string(),
                        "anim" => cycle(ANIMS, &w.anim).to_string(),
                        "size" => cycle(SIZES, &w.size).to_string(),
                        "color" => cycle(COLORS, &w.color).to_string(),
                        _ => String::new(),
                    };
                    widgets::set_opt(w, key, &v);
                }
                self.settings_changed();
            }
            Act::SEdit => {
                self.close_all();
                self.set_editing(true);
            }
            Act::SResetPos => {
                let id = self.st.sel_widget;
                if let Some(w) = self.st.set.widget_mut(id) {
                    w.x = -1;
                    w.y = -1;
                }
                self.settings_changed();
            }
            Act::DockPin(id, _) => {
                let id = id.clone();
                self.st.set.dock_pins.retain(|p| *p != id);
                self.settings_changed();
            }
            Act::SField(k) => {
                self.st.field_focus = Some(k);
                if *k == "float_apps" && self.st.float_field.text.is_empty() {
                    let p = self.st.set.float_apps.clone();
                    self.st.float_field.set(&p);
                }
                if *k == "kb_layout" && self.st.keys.lay.text.is_empty() {
                    let p = self.st.set.kb_layout.clone();
                    self.st.keys.lay.set(&p);
                }
                if *k == "night_geo" && self.st.geo_field.text.is_empty() {
                    let p = self.st.set.night_geo.clone();
                    self.st.geo_field.set(&p);
                }
                if *k == "game_apps" && self.st.game_field.text.is_empty() {
                    let p = self.st.set.game_apps.clone();
                    self.st.game_field.set(&p);
                }
                if *k == "shot_dir" && self.st.shot_field.text.is_empty() {
                    let p = self.st.set.shot_dir.clone();
                    self.st.shot_field.set(&p);
                }
                if *k == "wall" && self.st.wall_field.text.is_empty() {
                    let p = self.st.set.wallpaper.clone();
                    self.st.wall_field.set(&p);
                }
            }
            _ => return self.wall_click(a) || self.keys_click(a) || self.update_click(a),
        }
        self.dirty(&SKind::Modal);
        true
    }

    fn apply_hex(&mut self) {
        let mut h = self.st.hex.text.trim().to_string();
        if !h.starts_with('#') {
            h.insert(0, '#');
        }
        if parse_hex(&h).is_some() {
            self.st.set.accent = h.to_lowercase();
            self.st.set.wall_accent = false;
            self.st.field_focus = None;
            self.settings_changed();
        }
    }

    pub fn settings_key(&mut self, e: &KeyEvent) {
        if self.keys_key(e) {
            return;
        }
        if let Some(f) = self.st.field_focus {
            match e.keysym {
                Keysym::Escape => self.st.field_focus = None,
                Keysym::Return | Keysym::KP_Enter => {
                    if f == "hex" {
                        self.apply_hex();
                    } else if f == "float_apps" {
                        self.settings_click(&Act::SCycle("float_apply"));
                    } else if f == "shot_dir" {
                        self.settings_click(&Act::SCycle("shot_dir_apply"));
                    } else if f == "game_apps" {
                        self.settings_click(&Act::SCycle("game_apply"));
                    } else if f == "night_geo" {
                        self.settings_click(&Act::SCycle("geo_apply"));
                    } else if f == "kb_layout" {
                        self.settings_click(&Act::SCycle("kb_apply"));
                    } else if f == "key_cmd" {
                        self.settings_click(&Act::KeyAdd);
                    } else {
                        self.settings_click(&Act::SCycle("wall_apply"));
                    }
                }
                _ => {
                    let mods = self.mods;
                    let field = match f {
                        "hex" => &mut self.st.hex,
                        "shot_dir" => &mut self.st.shot_field,
                        "float_apps" => &mut self.st.float_field,
                        "key_cmd" => &mut self.st.keys.cmd,
                        "game_apps" => &mut self.st.game_field,
                        "night_geo" => &mut self.st.geo_field,
                        "kb_layout" => &mut self.st.keys.lay,
                        _ => &mut self.st.wall_field,
                    };
                    App::edit_field(field, e, &mods);
                }
            }
            self.dirty(&SKind::Modal);
            return;
        }
        match e.keysym {
            Keysym::Escape => self.close_modal(),
            Keysym::Tab | Keysym::ISO_Left_Tab | Keysym::Down | Keysym::Up => {
                let back = e.keysym == Keysym::ISO_Left_Tab || e.keysym == Keysym::Up || self.mods.shift;
                let n = TABS.len();
                let i = TABS.iter().position(|x| x.0 == self.st.stab).unwrap_or(0);
                self.st.stab = TABS[if back { (i + n - 1) % n } else { (i + 1) % n }].0;
            }
            _ => {}
        }
        if self.st.modal == Some(Modal::Settings) {
            self.dirty(&SKind::Modal);
        }
    }
}
