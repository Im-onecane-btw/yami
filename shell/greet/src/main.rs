// yami-greet — экран входа yami: приветствие greetd, которое рисует себя в niri тем же пиксельным
// лицом, что экран блокировки шелла. Вид (палитра, обои, язык, мониторы, раскладки) выкладывает
// шелл в /var/lib/yami-greet/look; пароль уходит только в greetd.
//
//   yami-greet                  экран входа (его запускает niri из greet-session)
//   yami-greet --demo           то же без greetd, пароль «yami» (попробовать в окне niri)
//   yami-greet kdl [--plain]    конфиг niri экрана входа в stdout
//   yami-greet shot [-o DIR] [--lang en]   снимки состояний в PNG без Wayland

mod app;
mod data;
mod draw;
mod greetd;
mod i18n;
mod kdl;
mod niri;

use draw::{Phase, G};
use std::path::{Path, PathBuf};
use std::time::Instant;
use yami_px::font::Fonts;
use yami_px::look::Look;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("--version") => {
            println!("yami-greet {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Some("-h") | Some("--help") => {
            println!("yami-greet [--demo] | kdl [--plain] | shot [-o DIR] [--lang en]");
            0
        }
        Some("kdl") => {
            let me = yami_px::paths::self_exe().to_string_lossy().to_string();
            print!("{}", kdl::config(&data::look_json(), args.iter().any(|a| a == "--plain"), &me));
            0
        }
        Some("shot") => shot(&args[1..]),
        _ => greet(args.iter().any(|a| a == "--demo")),
    };
    std::process::exit(code);
}

fn fonts(look: &Look) -> Fonts {
    let own = look.font("shell");
    if own.is_empty() {
        Fonts::bundled(Path::new(data::FONTS))
    } else {
        Fonts::with(own)
    }
}

/// состояние экрана входа из вида шелла, пользователей, сеансов и памяти о прошлом входе
fn state(v: &serde_json::Value, demo: bool) -> G {
    let look = Look::from_json(v, None);
    i18n::set(&look.lang);
    let users = data::users();
    let sessions = data::sessions();
    let (lu, ls) = data::last();
    let user = users.iter().position(|u| u.name == lu).unwrap_or(0);
    let session = sessions.iter().position(|s| s.id == ls).or_else(|| sessions.iter().position(|s| s.id.contains("niri"))).unwrap_or(0);
    let layouts: Vec<String> = v.get("kbLayout").and_then(|x| x.as_str()).unwrap_or("").split(',').map(|s| s.split('(').next().unwrap_or("").trim().to_string()).filter(|s| !s.is_empty()).collect();
    let phase = if users.is_empty() { Phase::Name } else { Phase::Pass };
    G {
        kanji: v.get("wallKanji").and_then(|x| x.as_bool()).unwrap_or(true),
        look,
        wall: None,
        users,
        user,
        typed: String::new(),
        sessions,
        session,
        layouts,
        layout: 0,
        caps: false,
        field: Default::default(),
        phase,
        prompt: String::new(),
        status: String::new(),
        bad: false,
        fails: 0,
        shake: None,
        host: data::hostname(),
        blink: true,
        born: Instant::now(),
        gone: None,
        demo,
    }
}

fn greet(demo: bool) -> i32 {
    // обои декодируются, пока niri отвечает на первые запросы
    let wall = std::thread::spawn(data::wall);
    let v = data::look_json();
    let mut g = state(&v, demo);
    if g.layouts.is_empty() {
        // вид не выложен: раскладки — у niri («English (US)» → «en»)
        if let Some((names, cur)) = niri::layouts() {
            g.layouts = names.iter().map(|n| n.chars().take(2).collect::<String>().to_lowercase()).collect();
            g.layout = cur;
        }
    }
    if g.sessions.is_empty() {
        g.status = i18n::tr("нет сеансов Wayland").to_string();
    }
    let f = fonts(&g.look);
    g.wall = wall.join().ok().flatten();
    g.born = Instant::now();
    yami_px::shed_code_later(10);
    app::run(g, f)
}

/// offscreen-снимки: ввод пароля, проверка, ошибка, вопрос PAM, ввод имени
fn shot(args: &[String]) -> i32 {
    let mut out = PathBuf::from("shots");
    let mut lang = String::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" => out = it.next().map(PathBuf::from).unwrap_or(out),
            "--lang" => lang = it.next().cloned().unwrap_or_default(),
            _ => {}
        }
    }
    let mut v = data::look_json();
    if v.is_null() {
        // вид не выложен (машина без экрана входа): вид шелла
        v = std::fs::read_to_string(yami_px::paths::shell_settings()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(serde_json::Value::Null);
    }
    if let Some(o) = v.as_object_mut() {
        o.insert("animations".into(), false.into());
        if !lang.is_empty() {
            o.insert("lang".into(), lang.clone().into());
        }
        if o.get("kbLayout").and_then(|x| x.as_str()).unwrap_or("").is_empty() {
            o.insert("kbLayout".into(), "us,ru".into());
        }
    }
    i18n::audit(lang == "en");
    let _ = std::fs::create_dir_all(&out);
    let mut bad = 0;
    let wall = data::wall();
    let mut fonts = fonts(&Look::from_json(&v, None));
    let mut ic = yami_px::icons::Icons::new("");
    for target in ["pass", "busy", "wrong", "prompt", "name", "second"] {
        let mut g = state(&v, false);
        g.wall = wall.clone();
        g.host = "yami".into();
        g.blink = true;
        g.users = vec![data::User { name: "yami".into(), real: "Yami".into() }, data::User { name: "guest".into(), real: String::new() }];
        g.user = 0;
        g.sessions = vec![data::Session { id: "niri".into(), name: "Niri".into(), cmd: vec!["niri-session".into()], desktops: "niri".into() }];
        g.session = 0;
        match target {
            "pass" => g.field.set("secret"),
            "busy" => g.phase = Phase::Busy,
            "wrong" => g.fails = 2,
            "prompt" => {
                g.phase = Phase::Prompt { secret: false };
                g.prompt = "Verification code:".into();
            }
            "name" => {
                g.users.clear();
                g.phase = Phase::Name;
                g.field.set("yami");
            }
            _ => {}
        }
        let (w, h) = (1280, 720);
        let mut buf = vec![0u32; (w * h) as usize];
        {
            let mut ui = draw::Ui { cv: yami_px::gfx::Canvas::new(&mut buf, w, h), f: &mut fonts, ic: &mut ic, t: g.look.theme, zones: vec![], ptr: None, down: false, anim: false, blink: true, tr: i18n::tr };
            draw::draw(&mut ui, &g, w, h, target != "second");
        }
        let rgba: Vec<u8> = buf.iter().flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8, 255]).collect();
        let path = out.join(format!("greet-{target}.png"));
        let _ = image::save_buffer(&path, &rgba, w as u32, h as u32, image::ColorType::Rgba8);
        println!("{}", path.display());
        if lang == "en" {
            for m in i18n::take_missed() {
                println!("  untranslated [{target}]: {m}");
                bad = 1;
            }
        }
    }
    bad
}
