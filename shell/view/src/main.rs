// yami-view — просмотрщик картинок шелла yami: мгновенно открывается, листает папку, честные
// пиксели при увеличении, поворот/отражение/кадрирование, сохранение через диалог yami-files.
//
//   yami-view [ФАЙЛ|ПАПКА…]
//   yami-view shot [-o ПАПКА] [--lang en]    снимки окна в PNG без Wayland

mod app;
mod i18n;
mod pic;
mod view;

pub use yami_px::{font, gfx, icons, look, paths, theme, ui};

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        Some("--version") => {
            println!("yami-view {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Some("-h") | Some("--help") => {
            println!("yami-view [FILE|DIR…] | shot [-o DIR] [--lang en]");
            return;
        }
        Some("shot") => std::process::exit(shot(&args[1..])),
        _ => {}
    }
    let look = look::Look::load();
    i18n::set(&look.lang);
    let paths: Vec<PathBuf> = args.iter().filter(|a| !a.starts_with('-')).map(|a| icons::uri_path(a).unwrap_or_else(|| PathBuf::from(a))).map(|p| if p.is_relative() { std::env::current_dir().unwrap_or_default().join(p) } else { p }).collect();
    // несколько файлов — листаем их; один — его папку; папка — её картинки
    let (list, idx) = match paths.len() {
        0 => (pic::siblings(&std::env::current_dir().unwrap_or_default()), 0),
        1 => {
            let p = &paths[0];
            let l = pic::siblings(p);
            if p.is_dir() {
                (l, 0)
            } else {
                let i = l.iter().position(|x| x == p);
                match i {
                    Some(i) => (l, i),
                    None => (vec![p.clone()], 0),
                }
            }
        }
        _ => (paths.clone(), 0),
    };
    app::run(view::V::new(look, list, idx));
}

/// offscreen-снимки: шахматка-картинка, панели, кадрирование, сведения
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
    let mut look = look::Look::load();
    look.animations = false;
    if !lang.is_empty() {
        look.lang = lang.clone();
    }
    i18n::set(&look.lang);
    i18n::audit(lang == "en");
    let _ = std::fs::create_dir_all(&out);
    let dir = std::env::temp_dir().join(format!("yami-view-shot-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    // картинка: пиксель-арт 48×32 в палитре
    let t = look.theme;
    let mut img = image::RgbaImage::new(48, 32);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let c = if (x / 6 + y / 6) % 2 == 0 { t.red } else { t.surface_hi };
        let c = if (x as i32 - 24).pow(2) + (y as i32 - 16).pow(2) < 80 { t.ember } else { c };
        *p = image::Rgba([(c.0 >> 16) as u8, (c.0 >> 8) as u8, c.0 as u8, 255]);
    }
    let f = dir.join("pixel-art.png");
    let _ = img.save(&f);
    let _ = img.save(dir.join("second.png"));
    let mut fonts = font::Fonts::new();
    let mut ic = icons::Icons::new("");
    let mut bad = 0;
    for target in ["view", "zoom", "crop", "info"] {
        let mut v = view::V::new(look.clone(), pic::siblings(&f), 0);
        v.idx = v.list.iter().position(|x| x == &f).unwrap_or(0);
        if let Ok(p) = pic::load(&f) {
            v.cache.insert(f.clone(), std::rc::Rc::new(p));
        }
        v.w = 1000;
        v.h = 640;
        v.reset_view();
        v.ptr = Some((500.0, 600.0));
        match target {
            "zoom" => v.set_zoom(24.0),
            "crop" => {
                v.cropping = true;
                v.crop = Some((8.0, 6.0, 40.0, 26.0));
            }
            "info" => v.info = true,
            _ => {}
        }
        let (w, h) = (v.w, v.h);
        let mut buf = vec![0u32; (w * h) as usize];
        {
            let mut ui = view::Ui { cv: gfx::Canvas::new(&mut buf, w, h), f: &mut fonts, ic: &mut ic, t: look.theme, zones: vec![], ptr: v.ptr, down: false, anim: false, blink: true, tr: i18n::tr };
            view::draw(&mut ui, &v, w, h);
        }
        let mut rgba = Vec::with_capacity(buf.len() * 4);
        for p in &buf {
            rgba.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, *p as u8, 255]);
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
    let _ = std::fs::remove_dir_all(&dir);
    bad
}
