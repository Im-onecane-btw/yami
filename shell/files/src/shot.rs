// Снимки окна файлов без Wayland: собирает папку-образец во временном каталоге (по файлу
// каждого типа, папки, картинка) и рисует нужное состояние в PNG. Как `yami-shell shot`.
//
//   yami-files shot [ЦЕЛЬ…] [--lang ru|en] [-o ПАПКА]

use crate::act::Ui;
use crate::fm::{Dlg, Fm, Mode};
use crate::font::Fonts;
use crate::gfx::{Canvas, C};
use crate::icons::Icons;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const TARGETS: &[&str] = &["grid", "list", "split", "info", "menu", "menu-bg", "main-menu", "props", "settings", "new-folder", "conflict", "open-with", "pick-save", "pick-open", "trash", "rename", "empty", "icons", "server", "places", "drag", "unlock"];

fn fixture(root: &Path) {
    let _ = std::fs::remove_dir_all(root);
    let mk = |p: &str, data: &[u8]| {
        let f = root.join(p);
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(f, data);
    };
    for d in ["проекты", "фото с моря", "Downloads", "музыка", ".config"] { // i18n-ok
        let _ = std::fs::create_dir_all(root.join(d));
    }
    mk("README.md", b"# yami\n\nPixel-art rice for niri.\n\n- shell\n- files\n- portal\n");
    mk("main.rs", b"fn main() {\n    println!(\"hi\");\n}\n");
    mk("notes.txt", "купить хлеба\nпочинить niri\n".as_bytes()); // i18n-ok
    mk("report-2026.pdf", b"%PDF-1.4\n");
    mk("song.flac", b"fLaC");
    mk("clip.mkv", b"\x1aE");
    mk("backup.tar.zst", b"\x28\xb5\x2f\xfd");
    mk("install.sh", b"#!/bin/sh\necho hi\n");
    mk("font.ttf", b"\0\x01\0\0");
    mk("arch.iso", b"CD001");
    mk("page.html", b"<html></html>");
    mk("table.ods", b"PK");
    mk(".hidden", b"x");
    mk("a-very-long-file-name-that-will-not-fit-into-one-line.txt", b"long");
    // картинка: маленький градиент в палитре
    let mut img = image::RgbaImage::new(64, 48);
    for (x, y, p) in img.enumerate_pixels_mut() {
        *p = image::Rgba([(x * 4) as u8, (y * 5) as u8, 120, 255]);
    }
    let _ = img.save(root.join("sunset.png"));
    let _ = std::os::unix::fs::symlink(root.join("проекты"), root.join("link-to-projects")); // i18n-ok
    let mut perm = std::fs::metadata(root.join("install.sh")).map(|m| m.permissions()).unwrap();
    use std::os::unix::fs::PermissionsExt;
    perm.set_mode(0o755);
    let _ = std::fs::set_permissions(root.join("install.sh"), perm);
}

pub fn run(args: &[String]) -> i32 {
    let mut lang = String::new();
    let mut out = PathBuf::from("shots");
    let mut targets: Vec<String> = vec![];
    let mut mode: Option<bool> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--lang" => lang = it.next().cloned().unwrap_or_default(),
            "-o" | "--out" => out = it.next().map(PathBuf::from).unwrap_or(out),
            "--theme" => mode = it.next().map(|m| m == "light"),
            "-h" | "--help" | "list" => {
                println!("yami-files shot [TARGET…] [--lang ru|en] [--theme dark|light] [-o DIR]\ntargets: {}", TARGETS.join(" "));
                return 0;
            }
            t if TARGETS.contains(&t) => targets.push(t.to_string()),
            t => {
                eprintln!("shot: unknown target {t}");
                return 2;
            }
        }
    }
    if targets.is_empty() {
        targets = TARGETS.iter().map(|s| s.to_string()).collect();
    }
    let mut look = crate::look::Look::load_mode(mode);
    if !lang.is_empty() {
        look.lang = lang.clone();
    }
    look.animations = false;
    crate::i18n::set(&look.lang);
    crate::i18n::audit(lang == "en" || yami_px::i18n::normalize(&lang) == "pt-BR");
    let root = std::env::temp_dir().join(format!("yami-files-shot-{}", std::process::id()));
    fixture(&root);
    let _ = std::fs::create_dir_all(&out);
    let mut fonts = Fonts::new();
    let mut icons = Icons::new(&look.icon_theme);
    icons.set_pixel(look.icon_pixel.then(|| look.theme.icon_ramp()));
    let mut bad = 0;
    for t in &targets {
        let mut cfg = crate::cfg::Cfg { info: false, ..Default::default() };
        if t == "list" || t == "rename" || t == "trash" {
            cfg.view = "list".into();
        }
        if t == "info" {
            cfg.info = true;
        }
        let mut fm = Fm::new(look.clone(), cfg, vec![root.clone()], None);
        fm.born = Instant::now() - Duration::from_secs(5);
        {
            use crate::places::{Dev, DevKind};
            let d = |dev: &str, uri: &str, label: &str, size: u64, mount: Option<&str>, kind: DevKind| Dev { dev: dev.into(), uri: uri.into(), label: label.into(), size, mount: mount.map(Into::into), removable: kind != DevKind::Disk || dev == "/dev/sdb1", fstype: if kind == DevKind::Disk { "btrfs".into() } else { String::new() }, kind, ..Default::default() };
            fm.devs = vec![
                Dev { free: Some((612_000_000_000, 1_000_000_000_000)), ..d("/dev/nvme0n1p2", "", "", 1_000_000_000_000, Some("/"), DevKind::Disk) },
                d("/dev/sdb1", "", "FLASH", 32_000_000_000, None, DevKind::Disk),
                Dev { locked: true, fstype: "crypto_LUKS".into(), ..d("/dev/sdc1", "", "", 64_000_000_000, None, DevKind::Disk) },
                d("", "mtp://Google_Pixel_8/", "Pixel 8", 0, None, DevKind::Phone),
                d("", "smb://nas/media/", "", 0, None, DevKind::Net),
            ];
        }
        let (w, h) = (1060, 660);
        fm.w = w;
        fm.h = h;
        let pick_cur = |fm: &mut Fm, name: &str| {
            if let Some(i) = fm.v().items.iter().position(|&k| fm.v().all[k].name == name) {
                fm.select_only(i);
            }
        };
        match t.as_str() {
            "split" => {
                fm.toggle_split();
                let d = root.join("проекты"); // i18n-ok
                fm.go(d);
            }
            "info" => pick_cur(&mut fm, "README.md"),
            "menu" | "menu-bg" | "main-menu" => {
                if t == "menu" {
                    pick_cur(&mut fm, "sunset.png");
                }
                let items = match t.as_str() {
                    "menu" => crate::menus::for_items(&mut fm),
                    "menu-bg" => crate::menus::for_bg(&mut fm),
                    _ => crate::menus::main_menu(&mut fm),
                };
                crate::menus::open(&mut fm, items, if t == "main-menu" { w - 10 - crate::draw::MENU_W } else { 420 }, if t == "main-menu" { 40 } else { 150 }, if t == "main-menu" { "≡" } else { "" });
                if let Some(m) = &mut fm.menu {
                    m.born = Instant::now() - Duration::from_secs(5);
                    if t == "menu" {
                        m.hover = Some(2);
                        m.sub = Some((2, Instant::now() - Duration::from_secs(5)));
                    }
                }
            }
            "props" => {
                pick_cur(&mut fm, "install.sh");
                fm.run(crate::fm::Cmd::Props);
            }
            "settings" => fm.run(crate::fm::Cmd::Settings),
            "server" => {
                fm.server_dlg("sftp://me@nas.local/", String::new(), None);
                if let Some((crate::fm::Dlg::Server { err, .. }, _)) = &mut fm.dlg {
                    *err = crate::i18n::tr("неверный логин или пароль").to_string();
                }
            }
            "new-folder" => fm.run(crate::fm::Cmd::NewFolder),
            "places" | "drag" => {
                // скрытое место, своя закладка; меню по «изображениям» или место в руке
                fm.side.push(crate::places::Place { id: root.join("проекты").to_string_lossy().into(), name: "проекты".into(), path: root.join("проекты"), sp: crate::picons::Special::Star, user: true }); // i18n-ok
                if let Some(i) = fm.side.iter().position(|p| p.id == "videos") {
                    let p = fm.side.remove(i);
                    fm.side_hidden.push(p);
                }
                let at = fm.side.iter().position(|p| p.id == "pictures" || p.id == "downloads").unwrap_or(1);
                let top = 47 + 10 + 22;
                if t == "drag" {
                    fm.side_drag = Some((at, (top + 26 + 13) as f64));
                } else {
                    let items = crate::menus::for_place(&fm, at);
                    crate::menus::open(&mut fm, items, 140, top + 26 * at as i32 + 14, "");
                    if let Some(m) = &mut fm.menu {
                        m.born = Instant::now() - Duration::from_secs(5);
                    }
                }
            }
            "unlock" => {
                fm.unlock_dlg("/dev/sdc1", "BACKUP · 64G".into());
                if let Some((Dlg::Unlock { pass, err, .. }, _)) = &mut fm.dlg {
                    pass.set("secret");
                    *err = crate::i18n::tr("неверный пароль").to_string();
                }
            }
            "conflict" => {
                let (tx, _rx) = std::sync::mpsc::channel();
                fm.dlg = Some((Dlg::Conflict { src: root.join("notes.txt"), dst: root.join("README.md"), all: false, reply: tx }, Instant::now()));
            }
            "open-with" => {
                pick_cur(&mut fm, "notes.txt");
                fm.run(crate::fm::Cmd::OpenWithOther);
                fm.refresh_open_list();
            }
            "pick-save" | "pick-open" => {
                let save = t == "pick-save";
                let mut name = crate::ui::Field::default();
                name.set("screenshot.png");
                fm.pick = Some(crate::fm::Pick { save, dirs: false, multiple: !save, title: crate::i18n::tr(if save { "сохранить файл" } else { "открыть файлы" }).into(), accept: crate::i18n::tr(if save { "сохранить" } else { "открыть" }).into(), name, filters: vec![("картинки".into(), vec![crate::fm::Pat::Mime("image/*".into())]), ("все файлы".into(), vec![])], filter: if save { 1 } else { 0 }, done: None, name_focus: save, overwrite: None });
                fm.refilter_all();
                if !save {
                    pick_cur(&mut fm, "sunset.png");
                }
            }
            "trash" => {
                fm.trash_n = 3;
                let v = fm.vm();
                v.trash = true;
                v.all = ["old.txt", "draft.md", "фото.png"] // i18n-ok
                    .iter().map(|n| { let mut e = crate::fsx::Entry::from_path(&root.join("notes.txt")).unwrap(); e.name = n.to_string(); e.orig = Some(root.join(n)); e.deleted = crate::fsx::now() - 3600; e }).collect();
                let cfg = fm.cfg.clone();
                crate::fm::refilter(fm.vm(), &cfg, None);
            }
            "rename" => {
                pick_cur(&mut fm, "notes.txt");
                fm.start_rename();
            }
            "empty" => {
                let d = root.join("музыка"); // i18n-ok
                fm.go(d);
            }
            _ => {}
        }
        if t == "grid" || t == "icons" {
            pick_cur(&mut fm, "проекты"); // i18n-ok
        }
        if t == "icons" {
            fm.cfg.zoom = 4;
        }
        let _ = Mode::Grid;
        let mut buf = vec![0u32; (w * h) as usize];
        // два прохода: первый узнаёт раскладку (колонки, высоты), второй рисует
        for _ in 0..2 {
            let mut ui = Ui { cv: Canvas::new(&mut buf, w, h), f: &mut fonts, ic: &mut icons, t: look.theme, zones: vec![], ptr: None, down: false, anim: false, blink: true, tr: crate::i18n::tr };
            crate::draw::draw(&mut ui, &fm, w, h);
        }
        // миниатюры — синхронно, чтобы попали в снимок
        let want: Vec<crate::thumbs::Req> = std::mem::take(&mut *fm.want.borrow_mut());
        if !want.is_empty() {
            for r in want {
                let img = image::open(&r.path).ok().map(|i| crate::icons::from_rgba(&i.thumbnail(r.size as u32, r.size as u32).to_rgba8()));
                fm.put_thumb((r.path, r.size), img);
            }
            let mut ui = Ui { cv: Canvas::new(&mut buf, w, h), f: &mut fonts, ic: &mut icons, t: look.theme, zones: vec![], ptr: None, down: false, anim: false, blink: true, tr: crate::i18n::tr };
            crate::draw::draw(&mut ui, &fm, w, h);
        }
        let path = out.join(format!("{t}.png"));
        save_png(&buf, w, h, &path);
        println!("{}", path.display());
        if lang == "en" || yami_px::i18n::normalize(&lang) == "pt-BR" {
            let missed = crate::i18n::take_missed();
            for m in &missed {
                println!("  untranslated [{t}]: {m}");
            }
            if !missed.is_empty() {
                bad = 1;
            }
        }
    }
    let _ = std::fs::remove_dir_all(&root);
    bad
}

fn save_png(buf: &[u32], w: i32, h: i32, path: &Path) {
    // поверх «рабочего стола», как в снимках шелла
    let bg = C::hex(0x2a2626).0;
    let mut rgba = Vec::with_capacity(buf.len() * 4);
    for &p in buf {
        let a = p >> 24;
        let ch = |s: u32| {
            let src = (p >> s) & 255;
            let dst = (bg >> s) & 255;
            (src + dst * (255 - a) / 255).min(255) as u8
        };
        rgba.extend_from_slice(&[ch(16), ch(8), ch(0), 255]);
    }
    if let Err(e) = image::save_buffer(path, &rgba, w as u32, h as u32, image::ColorType::Rgba8) {
        eprintln!("shot: {}: {e}", path.display());
    }
}
