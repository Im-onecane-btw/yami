// Снимки панелей без Wayland: рисует в память и пишет PNG. Чтобы проверить вид и перевод,
// не трогая экран пользователя (и не открывая ничего поверх его игры).
//
//   yami-shell shot [ЦЕЛЬ…] [--lang ru|en] [--theme dark|light] [--defaults] [--transparent] [--frame PNG] [-o ПАПКА]
//
// Без цели — все панели. При --lang en печатает надписи, оставшиеся русскими, и выходит с 1.
// Окна, программы и уведомление — мок-овые (снимок не зависит от машины), датчики — настоящие.

use crate::app::{Modal, Popup, Res, State, BAR_H};
use crate::config::{Settings, Widget};
use crate::font::Fonts;
use crate::gfx::Canvas;
use crate::icons::Icons;
use crate::niri::{Niri, Win, Ws};
use crate::gfx::C;
use crate::ui::Ui;
use std::path::{Path, PathBuf};
use std::time::Instant;

const TARGETS: &[&str] = &[
    "bar",
    "bar:pill",
    "bar:float",
    "bar:compact",
    "settings:color",
    "settings:bar",
    "settings:bar-look",
    "settings:wall-skwd",
    "settings:widgets",
    "settings:look",
    "settings:wall",
    "settings:system",
    "settings:about",
    "launcher",
    "launcher:calc",
    "launcher:find",
    "launcher:win",
    "launcher:cmd",
    "launcher:all",
    "reply",
    "clipboard",
    "power",
    "popup:cal",
    "popup:sound",
    "popup:media",
    "popup:net",
    "popup:wifi",
    "popup:bt",
    "popup:sys",
    "popup:profile",
    "popup:tray",
    "menu:widget",
    "widget:clock",
    "widget:graph",
    "widget:net",
    "widget:media",
    "widget:cal",
    "notif",
    "notif:shot",
    "osd",
    "lock",
    "curtain",
    "curtain:badge",
    "osd:curtain",
    "auth",
    "screen-pick",
    "screenshot",
    "screenshot:pick",
    "settings:shots",
    "settings:windows",
    "settings:dock",
    "settings:screens",
    "settings:bar-drag",
    "settings:keys",
    "settings:games",
    "settings:fonts",
    "settings:keys-rec",
    "settings:keys-custom",
    "settings:keys-layout",
    "settings:agents",
    "settings:updates",
    "settings:updates-new",
    "settings:updates-busy",
    "settings:updates-pick",
    "settings:updates-need",
    "dock",
];

/// надписи, которые и в английском интерфейсе пишутся по-русски
const NATIVE: &[&str] = &["Русский"]; // i18n-ok

/// --transparent: без подложки, с честной альфой (картинки для README кладутся на обои)
static CLEAR: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// --frame FILE: настоящий кадр экрана под оверлеем скриншота (tools/readme-shots.sh)
static FRAME: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

pub fn run(args: &[String]) -> i32 {
    let mut lang = String::new();
    let mut out = PathBuf::from("shots");
    let mut defaults = false;
    let mut mode: Option<bool> = None;
    let mut targets: Vec<String> = vec![];
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--lang" => lang = it.next().cloned().unwrap_or_default(),
            "-o" | "--out" => out = it.next().map(PathBuf::from).unwrap_or(out),
            "--defaults" => defaults = true,
            "--theme" => mode = it.next().map(|m| m == "light"),
            "--transparent" => CLEAR.store(true, std::sync::atomic::Ordering::Relaxed),
            "--frame" => {
                let _ = FRAME.set(it.next().map(PathBuf::from).unwrap_or_default());
            }
            "-h" | "--help" | "list" => {
                println!("yami-shell shot [TARGET…] [--lang ru|en] [--defaults] [--transparent] [--frame PNG] [-o DIR]\ntargets: {}", TARGETS.join(" "));
                return 0;
            }
            t if TARGETS.contains(&t) => targets.push(t.to_string()),
            t => {
                eprintln!("shot: unknown target {t}; `yami-shell shot list`");
                return 2;
            }
        }
    }
    if targets.is_empty() {
        targets = TARGETS.iter().map(|s| s.to_string()).collect();
    }
    let mut set = if defaults { Settings::default() } else { Settings::load() };
    if !lang.is_empty() {
        set.lang = lang;
    }
    if let Some(l) = mode {
        set.light = l;
    }
    crate::i18n::set(&set.lang);
    let mut res = Res { icons: Icons::new(&set.icon_theme), fonts: Fonts::with(&set.font_shell), font: set.font_shell.clone() };
    res.icons.set_pixel(set.icon_pixel.then(|| crate::theme::Theme::new(set.accent_rgb(), set.tint, set.glass, set.lift, set.inner, set.light).icon_ramp()));
    let mut st = State::new(set, mock_niri());
    st.apps = mock_apps();
    mock_tray(&mut st);
    st.sys.sample();
    mock_sys(&mut st.sys);
    if std::fs::create_dir_all(&out).is_err() {
        eprintln!("shot: cannot create {}", out.display());
        return 2;
    }
    crate::i18n::audit(crate::i18n::en());
    let mut bad = 0;
    for t in &targets {
        let path = out.join(format!("{}.png", t.replace(':', "-")));
        shoot(&mut st, &mut res, t, &path);
        let missed: Vec<String> = crate::i18n::take_missed().into_iter().filter(|s| !NATIVE.contains(&s.as_str())).collect();
        if missed.is_empty() {
            println!("{}", path.display());
        } else {
            bad += missed.len();
            println!("{}  untranslated: {}", path.display(), missed.iter().map(|s| format!("«{s}»")).collect::<Vec<_>>().join(" "));
        }
    }
    if bad > 0 {
        eprintln!("shot: {bad} untranslated string(s): add them to PAIRS in i18n.rs or use tf!/tr()");
        return 1;
    }
    0
}

fn shoot(st: &mut State, res: &mut Res, target: &str, path: &Path) {
    let (kind, arg) = target.split_once(':').unwrap_or((target, ""));
    // образцы формы бара: свои настройки — только на этот снимок
    if kind == "bar" && !arg.is_empty() {
        let (set, media) = (st.set.clone(), st.media.title.clone());
        let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        st.set.bar_float = true;
        st.set.bar_center_align = "center".into();
        if arg == "pill" {
            // как в issue: плашка 60 %, столы и длинный трек слева, часы ровно по центру
            st.set.bar_width_pc = 60;
            st.set.bar_left = v(&["workspaces", "sep", "media"]);
            st.set.bar_center = v(&["clock"]);
            st.set.bar_right = v(&["settings", "net", "vol", "temp"]);
            st.media.title = "a very long track title that pushes the clock".into();
        } else if arg == "compact" {
            // #11: трей стрелкой и короткая температура
            st.set.bar_width_pc = 100;
            st.set.tray_mode = "arrow".into();
            st.set.temp_style = "compact".into();
            st.set.bar_right = v(&["tray", "sep", "net", "cpu", "temp", "sep", "vol", "sep", "clock", "sep", "settings", "power"]);
        } else {
            st.set.bar_width_pc = 100;
        }
        shoot(st, res, "bar", path);
        st.set = set;
        st.media.title = media;
        return;
    }
    st.modal = None;
    st.popup = None;
    st.menu = None;
    st.osd = None;
    st.curtain = None;
    st.locked = false;
    if st.wall.is_none() && (matches!(kind, "bar" | "lock") || target == "settings:wall") {
        // обои — как у шелла, только без нити
        st.wall = crate::wall::prepare(&crate::wall::Job::new(&st.set, &st.t, 1920, 1080, crate::wall::source(&st))).0.map(std::rc::Rc::new);
    }
    let (w, h) = match kind {
        // бар как на мониторе 1920: с отступом и тенью, если он плашкой
        "bar" => {
            let (top, _, sh) = crate::bar::geo(&st.set, 1920);
            (1920, (top + sh + 6).max(BAR_H))
        }
        "settings" => {
            st.modal = Some(Modal::Settings);
            st.stab = crate::settings::TABS.iter().find(|t| t.0 == arg).map(|t| t.0).unwrap_or("color");
            if st.sel_widget == 0 {
                st.sel_widget = st.set.widgets.first().map(|w| w.id).unwrap_or(0);
            }
            // вкладка «бар», раздел «вид»: форма бара, отступы
            st.bar_ed.look = arg == "bar-look";
            // обои с skwd-wall: он стоит и включён, выбрано видео (здесь — его кадр)
            st.skwd_here = arg == "wall-skwd";
            if arg == "wall-skwd" {
                st.stab = "wall";
                st.set.wall_skwd = true;
                st.skwd.pick = Some(("/home/you/Videos/wallpapers/neon-rain.jpg".into(), "video".into()));
            } else if st.set.wall_skwd && st.skwd.pick.as_ref().is_some_and(|p| p.0.starts_with("/home/you")) {
                st.set.wall_skwd = false;
                st.skwd.pick = None;
            }
            if arg == "bar-look" {
                st.stab = "bar";
            }
            if arg == "bar-drag" {
                // фишка «часы» в руке над правым рядом, щель раскрыта
                st.stab = "bar";
                st.bar_ed.drag = Some(crate::settings::BDrag { from: None, name: "clock".into(), x: 430, y: 196, off: (20, 12), target: Some((2, 3)), at: Instant::now() - std::time::Duration::from_secs(1) });
            }
            if arg == "screens" {
                st.disp.outs = crate::displays::mock();
            }
            if arg == "widgets" {
                // все виды и источники в списке, чтобы --lang en видел каждую подпись
                let mut ws = vec![];
                for (i, (k, _)) in crate::widgets::KINDS.iter().enumerate() {
                    ws.push(crate::config::Widget::new(i as u32 + 1, k));
                }
                for (i, (src, _)) in crate::widgets::SOURCES.iter().enumerate() {
                    ws.push(crate::config::Widget { source: src.to_string(), ..crate::config::Widget::new(20 + i as u32, "graph") });
                }
                st.set.widgets = ws;
                st.sel_widget = 1;
            }
            if arg.starts_with("updates") {
                // образец, а не то, что стоит на этой машине
                st.stab = "updates";
                st.upd = crate::update::Upd::mock(arg);
            }
            if let Some(k) = arg.strip_prefix("keys-") {
                // запись сочетания в разгаре; свои команды с одной записью
                st.stab = "keys";
                if k == "rec" {
                    st.keys.capture = Some("launcher".into());
                } else if k == "layout" {
                    st.keys.cat = "layout";
                } else {
                    st.keys.cat = "custom";
                    st.set.custom_binds = vec![crate::keys::Custom { key: "Mod+Shift+B".into(), cmd: "kitty -e btop".into() }];
                    st.keys.new_key = "Mod+Alt+M".into();
                }
            }
            crate::settings::SIZE
        }
        "launcher" | "clipboard" => {
            st.modal = Some(Modal::Launcher);
            st.launcher.open(kind == "clipboard", &st.apps, &st.niri.wins);
            // launcher:calc, :find (окна и настройки), :cmd, :all (Ctrl+A)
            let q = match arg {
                "calc" => "2+2*3",
                "find" => "vrr",
                "win" => "wiki",
                "cmd" => "> btop",
                "all" => "firefox",
                _ => "",
            };
            if !q.is_empty() {
                st.launcher.field.set(q);
                if arg == "all" {
                    st.launcher.field.select_all();
                }
                st.launcher.refresh(&st.apps, &st.niri.wins);
            }
            crate::launcher::SIZE
        }
        "reply" => {
            st.modal = Some(Modal::Reply);
            st.reply = crate::reply::Reply { id: 1, app: "Telegram".into(), summary: "Ann: are we still on for tonight?".into(), field: Default::default() };
            st.reply.field.set("yes, at 8");
            crate::reply::SIZE
        }
        "power" => {
            st.modal = Some(Modal::Power);
            crate::power::SIZE
        }
        "popup" => {
            if arg == "profile" {
                st.prof = crate::profile::Prof { present: true, active: "performance".into(), list: vec!["power-saver".into(), "balanced".into(), "performance".into()], degraded: String::new() };
            }
            if arg == "net" || arg == "wifi" {
                mock_net(st, arg == "wifi");
            }
            if arg == "bt" {
                mock_bt(st);
            }
            if arg == "media" {
                // играет: эквалайзер виден; за миг снимка позиция не сдвинется
                let at = std::time::Instant::now();
                st.media = crate::media::MediaState { player: "mpv".into(), status: "Playing".into(), artist: "Ash Field".into(), title: "Pixel Rain".into(), art: String::new(), length: 245.0, pos: 83.0, at };
                st.viz = (0..32).map(|i| ((i * 7 % 11) as f32 / 11.0) * 0.8 + 0.1).collect();
            }
            let k = ["cal", "sound", "media", "net", "bt", "sys", "profile", "tray"].into_iter().find(|k| *k == arg).unwrap_or(if arg == "wifi" { "net" } else { "cal" });
            st.popup = Some(Popup { kind: k });
            crate::popups::size(k, st)
        }
        "menu" => {
            let w = Widget::new(900, "graph");
            st.menu = Some(crate::menu::MenuState::new("", 0, 0, &crate::tf!("виджет: {}", "widget: {}", crate::i18n::tr("график")), crate::widgets::widget_menu(&w)));
            crate::menu::size(st.menu.as_ref().unwrap(), &mut res.fonts)
        }
        "widget" => {
            st.set.widgets.retain(|w| w.id != 900);
            let w = Widget { on: true, ..Widget::new(900, arg) };
            let s = crate::widgets::size_of(&w);
            st.set.widgets.push(w);
            s
        }
        "notif" if arg != "shot" => {
            if st.notifs.history.is_empty() {
                st.notifs.history.push(mock_note());
                st.notifs.popups.push(1);
            }
            crate::notif::size(st, &mut res.fonts)
        }
        "notif" => {
            // превью скриншота: широкая, высокая и крошечная области
            st.notifs.history.clear();
            st.notifs.popups.clear();
            for (i, (w, h)) in [(1600u32, 300u32), (300, 1000), (40, 30)].into_iter().enumerate() {
                let img = image::RgbaImage::from_fn(w, h, |x, y| image::Rgba([(x * 255 / w) as u8, 40, (y * 255 / h) as u8, 255]));
                let mut n = mock_note();
                n.id = i as u32 + 1;
                n.app = "yami-screenshot".into();
                n.summary = "скриншот сохранён и скопирован".into();
                n.body = "~/Pictures/Screenshots/shot-1200.png".into();
                n.actions = vec![("default".into(), "открыть".into()), ("folder".into(), "в папке".into())];
                n.image = Some(std::rc::Rc::new(crate::icons::fit_in(img, crate::notif::IMG_W, crate::notif::IMG_H)));
                st.notifs.history.push(n);
                st.notifs.popups.push(i as u32 + 1);
            }
            crate::notif::size(st, &mut res.fonts)
        }
        "osd" => {
            st.osd = Some((if arg == "curtain" { "curtain" } else { "volume" }, Instant::now()));
            if arg == "curtain" {
                st.curtain = Some(Default::default());
            }
            (308, 64)
        }
        "curtain" if arg == "badge" => (crate::curtain::badge_w(&mut res.fonts), 34),
        "curtain" => {
            // то, что увидят зрители: кадр — обои, в стиле шторки (как с настоящим экраном)
            let mut c = crate::curtain::Curtain::default();
            if let Some(i) = crate::wall::prepare(&crate::wall::Job::new(&st.set, &st.t, 1280, 720, crate::wall::source(&st))).0 {
                c.imgs.insert("shot".into(), std::rc::Rc::new(crate::curtain::cover_of(&i, &st.set.curtain_style, st.set.curtain_cell / 2).unwrap_or(i)));
            }
            st.curtain = Some(c);
            (1280, 720)
        }
        "lock" => {
            st.locked = true;
            (1280, 720)
        }
        "screen-pick" => {
            st.modal = Some(Modal::Screen);
            let (a, _b) = std::os::unix::net::UnixStream::pair().expect("pair");
            let it = |reply: &str, monitor: bool, title: &str, sub: &str, icon: &str, size: (i32, i32)| crate::screen::Item { reply: reply.into(), monitor, title: title.into(), sub: sub.into(), icon: icon.into(), size };
            let p = crate::screen::Pick {
                stream: a,
                items: vec![
                    it("monitor DP-1", true, "DP-1", "2560×1440", "", (2560, 1440)),
                    it("monitor HDMI-A-1", true, "HDMI-A-1", "1080×1920", "", (1080, 1920)),
                    it("window 1", false, "~/projects/yami", "kitty", "kitty", (0, 0)),
                    it("window 2", false, "niri wiki — Firefox", "firefox", "firefox", (0, 0)),
                    it("window 3", false, "Steam", "steam", "steam", (0, 0)),
                ],
                sel: 0,
                scroll: 0,
            };
            let sz = crate::screen::size(&p);
            st.screen_pick = Some(p);
            sz
        }
        "auth" => {
            st.modal = Some(Modal::Auth);
            st.auth = vec![crate::auth::Req {
                cookie: "1".into(),
                message: "Authentication is required to change the system power profile".into(),
                action: "net.hadess.PowerProfiles.switch-profile".into(),
                user: "user".into(),
                prompt: "Password: ".into(),
                echo: false,
                info: String::new(),
                fails: 1,
                busy: false,
                field: crate::ui::Field { text: "hunter2".into(), cur: 7, all: false },
                shake: None,
            }];
            crate::auth::SIZE
        }
        "dock" => {
            st.set.dock_pins = vec!["org.kde.dolphin".into(), "kitty".into(), "firefox".into(), "steam".into()];
            crate::dock::size(st)
        }
        "screenshot" => {
            let (w, h, px) = frame().unwrap_or((1280, 720, None));
            st.snap = Some(crate::snap::Snap::mock(w, h, arg != "pick", &st.t, px));
            (w, h)
        }
        _ => (300, 200),
    };
    let mut buf = vec![0u32; (w * h) as usize];
    {
        let mut ui = Ui {
            cv: Canvas::new(&mut buf, w, h),
            f: &mut res.fonts,
            ic: &mut res.icons,
            t: st.t,
            zones: vec![],
            ptr: None,
            down: false,
            anim: false,
            blink: true,
            tr: crate::i18n::tr,
        };
        // подложка — «рабочий стол»: стекло панелей видно, но снимок непрозрачный
        if !CLEAR.load(std::sync::atomic::Ordering::Relaxed) {
            ui.cv.clear(if st.t.light { C::hex(0xc4bdb8) } else { C::hex(0x2a2626) });
        }
        let st = &*st;
        match kind {
            "bar" => {
                crate::wall::draw(&mut ui, st, w, h);
                let (top, side, _) = crate::bar::geo(&st.set, w);
                ui.cv.ox = side;
                ui.cv.oy = top;
                crate::bar::draw(&mut ui, st, "", w - 2 * side);
                ui.cv.ox = 0;
                ui.cv.oy = 0;
            }
            "settings" => crate::settings::draw(&mut ui, st, w, h),
            "launcher" | "clipboard" => crate::launcher::draw(&mut ui, st, w, h),
            "reply" => crate::reply::draw(&mut ui, st, w, h),
            "power" => crate::power::draw(&mut ui, st, w, h),
            "popup" => crate::popups::draw(&mut ui, st, w, h),
            "menu" => crate::menu::draw(&mut ui, st, w, h),
            "widget" => crate::widgets::draw(&mut ui, st, 900, w, h),
            "notif" => crate::notif::draw(&mut ui, st, w, h),
            "osd" => crate::notif::draw_osd(&mut ui, st, w, h),
            "lock" => crate::lock::draw(&mut ui, st, w, h, true),
            "curtain" if arg == "badge" => crate::curtain::draw_badge(&mut ui, w, h),
            "curtain" => crate::curtain::draw(&mut ui, st, "shot", w, h),
            "screenshot" => crate::snap::draw(&mut ui, st, "", w, h),
            "dock" => crate::dock::draw(&mut ui, st, w, h),
            "auth" => crate::auth::draw(&mut ui, st, w, h),
            "screen-pick" => crate::screen::draw(&mut ui, st, w, h),
            _ => {}
        }
    }
    save_png(&buf, w, h, path);
}

/// кадр из --frame как непрозрачный ARGB; None — нет флага или файл не читается
fn frame() -> Option<(i32, i32, Option<Vec<u32>>)> {
    let img = image::open(FRAME.get()?).map_err(|e| eprintln!("shot: --frame: {e}")).ok()?.to_rgb8();
    let px = img.pixels().map(|p| 0xff000000 | (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32).collect();
    Some((img.width() as i32, img.height() as i32, Some(px)))
}

fn save_png(buf: &[u32], w: i32, h: i32, path: &Path) {
    let mut rgba = Vec::with_capacity(buf.len() * 4);
    for &p in buf {
        // холст — premultiplied ARGB; в PNG альфа прямая
        let a = (p >> 24) as u32;
        let un = |c: u32| if a == 0 { 0 } else { (c * 255 / a).min(255) as u8 };
        if CLEAR.load(std::sync::atomic::Ordering::Relaxed) {
            rgba.extend_from_slice(&[un((p >> 16) & 255), un((p >> 8) & 255), un(p & 255), a as u8]);
        } else {
            rgba.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8, 255]);
        }
    }
    if let Err(e) = image::save_buffer(path, &rgba, w as u32, h as u32, image::ColorType::Rgba8) {
        eprintln!("shot: {}: {e}", path.display());
    }
}

fn mock_niri() -> Niri {
    let mut n = Niri::default();
    for i in 1..=3u64 {
        n.ws.push(Ws { id: i, idx: i as u32, output: String::new(), active: i == 1, focused: i == 1, ..Default::default() });
    }
    for (id, app, title) in [(1u64, "kitty", "~/projects/yami"), (2, "firefox", "niri wiki — Firefox")] {
        n.wins.push(Win { id, app_id: app.into(), title: title.into(), ws: Some(1), focused: id == 1, ..Default::default() });
    }
    n.kb_names = vec!["English (US)".into()];
    n
}

/// сети образца; ask — сеть попросила пароль (поле под ней, подсказка)
fn mock_net(st: &mut State, ask: bool) {
    use crate::net::WifiNet;
    let net = |ssid: &str, signal: u8, sec: &str, active: bool, known: bool| WifiNet { ssid: ssid.into(), signal, secure: !sec.is_empty(), active, known, sec: sec.into(), uuid: if known { "u".into() } else { String::new() }, mgmt: if known { "wpa-psk".into() } else { String::new() } };
    st.net = crate::net::NetState { loaded: true, wifi_present: true, wifi_on: true, active: vec![("home".into(), "wi-fi".into(), "wlan0".into())], ..Default::default() };
    st.net.nets = vec![net("home", 82, "WPA2", !ask, true), net("cafe-guest", 64, "", false, false), net("neighbour 5G", 41, "WPA2 WPA3", false, false), net("office", 27, "WPA2 802.1X", false, false)];
    if ask {
        st.net.active.clear();
        st.net.msg = "нужен пароль".into();
        st.wifi_target = "home".into();
        st.wifi_pass.text = "secret".into();
        st.wifi_pass.cur = 6;
        st.field_focus = Some("wifi");
    }
}

/// три программы в трее: значки — клетки в тонах темы, одна зовёт (NeedsAttention)
fn mock_tray(st: &mut State) {
    let t = st.t;
    for (i, (name, status)) in [("Telegram", "Active"), ("Steam", "NeedsAttention"), ("nm-applet", "Active")].into_iter().enumerate() {
        let c = [t.red, t.ok, t.text_dim][i];
        let px = (0..256).map(|k| { let (x, y) = (k % 16, k / 16); if (2..14).contains(&x) && (2..14).contains(&y) && (x + y + i) % 5 != 0 { c.0 | 0xff000000 } else { 0 } }).collect();
        let pixmap = Some(crate::gfx::Img { w: 16, h: 16, px });
        crate::dbus::apply(&mut st.tray, crate::dbus::TrayEv::Upsert { service: format!(":1.{i}"), bus: format!(":1.{i}"), path: "/StatusNotifierItem".into(), title: name.into(), status: status.into(), icon: String::new(), theme_path: None, pixmap, menu: String::new(), is_menu: false });
    }
}

fn mock_bt(st: &mut State) {
    use crate::net::BtDev;
    let dev = |n: &str, icon: &str, c: bool, p: bool, b: Option<u8>| BtDev { path: format!("/org/bluez/hci0/{n}"), name: n.into(), icon: icon.into(), connected: c, paired: p, battery: b };
    st.bt = crate::net::BtState { present: true, powered: true, adapter: "/org/bluez/hci0".into(), devices: vec![dev("headphones", "audio-headset", true, true, Some(80)), dev("mouse", "input-mouse", false, true, None), dev("speaker", "audio-card", false, false, None)], msg: "AuthenticationFailed".into(), busy: "/org/bluez/hci0/speaker".into(), ..Default::default() };
}

/// датчики образца, а не этой машины: снимки одинаковы везде и не выдают железо
fn mock_sys(s: &mut crate::sys::Sys) {
    let wave = |i: usize, k: f32, b: f32| (b + 0.18 * ((i as f32 * k).sin() + 0.5 * (i as f32 * k * 2.7).cos())).clamp(0.02, 0.98);
    for i in 0..120 {
        s.h_cpu.push(wave(i, 0.21, 0.32));
        s.h_mem.push(wave(i, 0.05, 0.41));
        s.h_temp.push(wave(i, 0.08, 0.48) * 100.0);
        s.h_gpu.push(wave(i, 0.13, 0.41) * 100.0);
        s.h_rx.push(wave(i, 0.37, 0.3) * 2.0e6);
        s.h_tx.push(wave(i, 0.29, 0.15) * 4.0e5);
    }
    (s.cpu, s.mem, s.mem_used, s.mem_total, s.swap, s.temp, s.gpu_temp, s.disk) = (0.32, 0.41, 6.6, 16.0, 0.0, 48.0, 41.0, 0.37);
    s.iface = "wlan0".into();
    s.bat = Some(crate::sys::Bat { pct: 73, charging: true, plugged: false, mins: Some(48) });
    s.ip = "192.0.2.10".into();
    (s.rx, s.tx) = (6.1e5, 1.2e5);
}

fn mock_apps() -> Vec<crate::icons::App> {
    [("kitty", "kitty", "Terminal emulator", "kitty"), ("firefox", "Firefox", "Web browser", "firefox"), ("org.kde.dolphin", "Dolphin", "File manager", "system-file-manager"), ("steam", "Steam", "Game library", "steam")]
        .into_iter()
        .map(|(id, name, generic, icon)| crate::icons::App {
            id: id.into(),
            name: name.into(),
            generic: generic.into(),
            comment: String::new(),
            keywords: String::new(),
            icon: icon.into(),
            exec: id.into(),
            terminal: false,
            wm_class: id.into(),
            ..Default::default()
        })
        .collect()
}

fn mock_note() -> crate::notif::Note {
    crate::notif::Note {
        id: 1,
        app: "Steam".into(),
        icon: String::new(),
        summary: "Download complete".into(),
        body: "Cyberpunk 2077 is ready to play".into(),
        actions: vec![],
        critical: false,
        image: None,
        // уже показано целиком (не в начале появления)
        born: Instant::now() - std::time::Duration::from_secs(2),
        time: "12:00".into(),
        life: 7.0,
        left: 7.0,
    }
}
