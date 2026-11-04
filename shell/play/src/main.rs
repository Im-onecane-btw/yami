// yami-play — видео и музыка в пиксельном стиле шелла yami. Своё окно и интерфейс, а всё, что
// касается форматов, — у libmpv (загружается при запуске): любые контейнеры и кодеки, что знает
// ffmpeg, аппаратное декодирование, субтитры, дорожки, потоки по ссылке. Помнит, где остановился.
//
//   yami-play [ФАЙЛ|ПАПКА|URL…]
//   yami-play shot [-o ПАПКА] [--lang en]    снимки окна в PNG без Wayland и без mpv

mod app;
mod i18n;
mod mpv;
mod play;

pub use yami_px::{font, gfx, icons, look, paths, theme, ui};

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        Some("--version") => {
            println!("yami-play {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Some("-h") | Some("--help") => {
            println!("yami-play [FILE|DIR|URL…] | shot [-o DIR] [--lang en]\n\nkeys: space pause · ←/→ 5 s (shift 1 s, ctrl 30 s) · ↑/↓ volume · f fullscreen · m mute\n      n/p next/previous · [ ] speed · a audio · s subtitles · l repeat · x pixel scaling\n      c save frame · , . frame step · 1…9 jump to 10…90 % · i details · q quit");
            return;
        }
        Some("shot") => std::process::exit(shot(&args[1..])),
        _ => {}
    }
    let look = look::Look::load();
    i18n::set(&look.lang);
    let items: Vec<String> = args.into_iter().filter(|a| !a.starts_with('-') || a == "-").collect();
    // ссылки (http, rtsp…) — как есть; file:// и пути — файлами
    let urls: Vec<PathBuf> = items.iter().filter(|a| a.contains("://") && !a.starts_with("file://")).map(PathBuf::from).collect();
    let paths: Vec<PathBuf> = items.iter().filter(|a| !a.contains("://") || a.starts_with("file://")).map(|a| icons::uri_path(a).unwrap_or_else(|| PathBuf::from(a))).map(|p| if p.is_relative() { std::env::current_dir().unwrap_or_default().join(p) } else { p }).collect();
    // один файл — вся его папка по порядку; папка — её видео и музыка; несколько — как дали
    let (list, idx) = if !urls.is_empty() {
        let mut l = urls;
        l.extend(paths);
        (l, 0)
    } else {
        match paths.len() {
            0 => (vec![], 0),
            1 if paths[0].is_dir() => (play::siblings(&paths[0]), 0),
            1 => {
                let l = play::siblings(&paths[0]);
                match l.iter().position(|x| x == &paths[0]) {
                    Some(i) => (l, i),
                    None => (vec![paths[0].clone()], 0),
                }
            }
            _ => (paths, 0),
        }
    };
    std::process::exit(app::run(play::P::new(look, list, idx)));
}

/// offscreen-снимки: кадр-заглушка, панели, сведения, музыка без обложки
fn shot(args: &[String]) -> i32 {
    let mut out = PathBuf::from("shots");
    let mut lang = String::new();
    let mut mode: Option<bool> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" => out = it.next().map(PathBuf::from).unwrap_or(out),
            "--lang" => lang = it.next().cloned().unwrap_or_default(),
            "--theme" => mode = it.next().map(|m| m == "light"),
            _ => {}
        }
    }
    let mut look = look::Look::load_mode(mode);
    look.animations = false;
    if !lang.is_empty() {
        look.lang = lang.clone();
    }
    i18n::set(&look.lang);
    i18n::audit(lang == "en");
    let _ = std::fs::create_dir_all(&out);
    let mut fonts = font::Fonts::with(look.font("view"));
    let mut ic = icons::Icons::new("");
    let mut bad = 0;
    let t = look.theme;
    for target in ["video", "paused", "info", "music", "error"] {
        let list = vec![PathBuf::from("/home/me/Videos/Pixel Days — 02.mkv"), PathBuf::from("/home/me/Videos/Pixel Days — 03.mkv")];
        let mut p = play::P::new(look.clone(), list, 0);
        p.born = std::time::Instant::now() - std::time::Duration::from_secs(5);
        p.w = 1000;
        p.h = 600;
        p.loaded = true;
        p.video = target != "music" && target != "error";
        p.dur = 1504.0;
        p.pos = 612.0;
        p.cache = 40.0;
        p.vw = 1920;
        p.vh = 1080;
        p.vcodec = "hevc".into();
        p.acodec = "opus".into();
        p.hwdec = "nvdec-copy".into();
        p.fps = 23.976;
        p.format = "mkv".into();
        p.size = 734_003_200;
        p.tracks = (4, 0);
        p.sid = "1".into();
        p.ptr = Some((420.0, 553.0));
        match target {
            "paused" => p.paused = true,
            "info" => p.info = true,
            "music" => {
                p.title = "Night Drive".into();
                p.artist = "yami".into();
                p.vcodec.clear();
            }
            "error" => p.err = Some("unrecognized file format".into()),
            _ => {}
        }
        let (w, h) = (p.w, p.h);
        let mut buf = vec![0u32; (w * h) as usize];
        // «кадр»: закат из полос палитры
        if p.video {
            for y in 0..h {
                let k = y as f32 / h as f32;
                let c = t.bg.lerp(t.red, k * 0.8).lerp(t.ember, (k - 0.6).max(0.0));
                buf[(y * w) as usize..((y + 1) * w) as usize].fill(c.0 | 0xff00_0000);
            }
        }
        {
            let mut ui = play::Ui { cv: gfx::Canvas::new(&mut buf, w, h), f: &mut fonts, ic: &mut ic, t, zones: vec![], ptr: p.ptr, down: false, anim: false, blink: true, tr: i18n::tr };
            play::draw(&mut ui, &p, w, h, !p.video);
        }
        let mut rgba = Vec::with_capacity(buf.len() * 4);
        for px in &buf {
            rgba.extend_from_slice(&[(px >> 16) as u8, (px >> 8) as u8, *px as u8, 255]);
        }
        let path = out.join(format!("{target}.png"));
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
