// Настройки: вкладки слева, содержимое справа. Всё применяется сразу
// и пишется в ~/.config/yami-shell.json (его можно править и руками — шелл подхватит).

use crate::app::{App, Modal, SKind, State};
use crate::font::PX;
use crate::gfx::{BoxStyle, R};
use crate::ui::{Act, Cur, Tone, Ui};
pub use crate::bartab::{BDrag, BarEd};
use crate::bartab::tab_bar;
use crate::settabs::{tab_about, tab_color, tab_dock, tab_look, tab_shots, tab_system, tab_widgets, tab_windows};
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
    ("color", "палитра акцент цвет тема темы шаблон светлая тёмная стекло прозрачность плотность palette accent colour color theme themes template light dark glass opacity"), // i18n-ok
    ("wall", "обои картинка фон папка миниатюры skwd skwd-wall видео wallpaper background image folder thumbnails picker video"), // i18n-ok
    ("look", "пиксельность пиксели лёгкий режим слабый тормозит виртуалка анимации эффекты появление развёртка размытие значки иконки курсор pixel pixelation lite slow lag vm animation effects reveal scanlines blur icons cursor"), // i18n-ok
    ("fonts", "шрифт размер терминал fastfetch браузер fonts size terminal browser"), // i18n-ok
    ("bar", "бар модули часы разделитель плавающий плашка ширина центр трей стрелка температура bar modules clock separator floating pill width center tray arrow temperature"), // i18n-ok
    ("widgets", "виджеты график часы заметка widgets graph clock note"), // i18n-ok
    ("dock", "док закреплённые dock pinned"), // i18n-ok
    ("windows", "окна отступы плавающие стол заголовки углы скругление рамка gaps floating stacking windows titlebars corners rounded radius border"), // i18n-ok
    ("keys", "клавиши сочетания хоткеи раскладка keys shortcuts hotkeys layout keyboard"), // i18n-ok
    ("games", "игры steam proton разрывы gamemode ntsync tearing games shader"), // i18n-ok
    ("shots", "скриншоты снимок лупа screenshot loupe"), // i18n-ok
    ("screens", "экраны монитор разрешение частота масштаб vrr hz герц поворот display monitor resolution refresh scale rotation"), // i18n-ok
    ("system", "система язык сон блокировка ожидание уведомления место osd громкость system language idle sleep lock notifications position osd volume"), // i18n-ok
    ("agents", "службы уведомления трей polkit порталы обложки services notifications tray portals"), // i18n-ok
    ("updates", "обновления версия откат updates version rollback"), // i18n-ok
    ("about", "о шелле версия память about version memory"), // i18n-ok
];

pub fn draw(ui: &mut Ui, st: &State, w: i32, h: i32) {
    let t = ui.t;
    let fr = R::new(0, 0, w - 6, h - 6);
    ui.frame(fr, false);
    ui.header(R::new(1, 1, fr.w - 2, 30), "gear", "настройки", "yami-shell");
    // за заголовок окно двигается (крестик зарегистрирован позже — он сверху)
    ui.zone(R::new(1, 1, fr.w - 2, 30), Act::MDrag, Cur::Default);
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
    /// что есть в системе для открытой вкладки: игры (gamemode, ntsync…), skwd-wall для обоев.
    /// При открытии вкладки и настроек — не в каждом кадре
    pub fn tab_probe(&mut self) {
        match self.st.stab {
            "games" => self.st.game_probe = crate::gameboost::probe(),
            "wall" => self.st.skwd_here = crate::skwd::installed(),
            _ => {}
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
                    } else if f == "weather_city" {
                        self.settings_click(&Act::SCycle("weather_apply"));
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
                        "weather_city" => &mut self.st.weather_field,
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
