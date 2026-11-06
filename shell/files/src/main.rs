// yami-files — файловый менеджер в пиксельном стиле шелла yami. Своё окно, свой рендер в shm,
// никаких тулкитов: ~3 МБ бинарь, десяток мегабайт памяти. Тот же бинарь — диалог выбора
// файлов для xdg-desktop-portal (браузеры, Telegram, всё на GTK/Qt через портал).
//
//   yami-files [ПАПКА…]                открыть окно (каждая папка — вкладка)
//   yami-files --pick open|save|dir    диалог: выбранные пути — в stdout по строке
//        [--multiple] [--title T] [--accept T] [--name ИМЯ] [--folder ПАПКА]
//        [--filter "Name|*.png;*.jpg|image/*"]…
//   yami-files --portal                бэкенд org.freedesktop.impl.portal.FileChooser и
//                                      org.freedesktop.FileManager1 («показать в папке»)
//   yami-files --props ФАЙЛ…           свойства файлов
//   yami-files shot [ЦЕЛЬ…] [--lang en] [-o ПАПКА]   снимки окна в PNG без Wayland

mod act;
mod app;
mod ask;
mod cfg;
mod cmds;
mod devices;
mod dialogs;
mod draw;
mod fm;
mod fsx;
mod i18n;
mod keys;
mod mouse;
mod menus;
mod mime;
mod open;
mod ops;
mod picons;
mod places;
mod portal;
mod remote;
mod screencast;
mod shot;
mod thumbs;
mod trash;
mod watch;
mod wayland;

pub use yami_px::{font, gfx, icons, look, paths, theme, ui};

use std::path::PathBuf;

fn pick_from_args(args: &[String]) -> (fm::Pick, Vec<PathBuf>) {
    let mut p = fm::Pick { save: false, dirs: false, multiple: false, title: String::new(), accept: String::new(), name: ui::Field::default(), filters: vec![], filter: 0, done: None, name_focus: false, overwrite: None };
    let mut dirs = vec![];
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "open" => {}
            "save" => p.save = true,
            "dir" => p.dirs = true,
            "--multiple" => p.multiple = true,
            "--title" => p.title = it.next().cloned().unwrap_or_default(),
            "--accept" => p.accept = it.next().cloned().unwrap_or_default(),
            "--name" => {
                let n = it.next().cloned().unwrap_or_default();
                p.name.set(&n);
                p.name.cur = fsx::stem_len(&n, false);
            }
            "--folder" => dirs.push(PathBuf::from(it.next().cloned().unwrap_or_default())),
            "--filter" => {
                let f = it.next().cloned().unwrap_or_default();
                let mut parts = f.split('|');
                let name = parts.next().unwrap_or("").to_string();
                let mut pats = vec![];
                for part in parts {
                    for x in part.split(';').filter(|x| !x.is_empty()) {
                        pats.push(if x.contains('/') && !x.starts_with('*') { fm::Pat::Mime(x.to_string()) } else { fm::Pat::Glob(x.to_string()) });
                    }
                }
                p.filters.push((name, pats));
            }
            "--current-filter" => p.filter = it.next().and_then(|s| s.parse().ok()).unwrap_or(0),
            _ => {}
        }
    }
    if p.title.is_empty() {
        p.title = i18n::tr(if p.save { "сохранить файл" } else if p.dirs { "выбрать папку" } else if p.multiple { "открыть файлы" } else { "открыть файл" }).to_string();
    }
    if p.accept.is_empty() {
        p.accept = i18n::tr(if p.save { "сохранить" } else if p.dirs { "выбрать" } else { "открыть" }).to_string();
    }
    p.name_focus = p.save;
    if p.filter >= p.filters.len() {
        p.filter = 0;
    }
    (p, dirs)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("--version") => {
            println!("yami-files {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Some("-h") | Some("--help") => {
            println!("yami-files [DIR…] | --pick open|save|dir [--multiple] [--title T] [--accept T] [--name N] [--folder DIR] [--filter 'Name|*.ext;mime/type']… | --portal | --portal-reload | shot [TARGET…] [--lang en] [-o DIR]");
            return;
        }
        Some("shot") => std::process::exit(shot::run(&args[2..])),
        Some("--portal") => std::process::exit(portal::serve()),
        Some("--portal-reload") => std::process::exit(portal::reload()),
        _ => {}
    }
    let look = look::Look::load();
    i18n::set(&look.lang);
    let cfg = cfg::Cfg::load();
    // кеш миниатюр (общий с выбором обоев) — раз в сутки, не мешая открытию окна
    std::thread::spawn(icons::thumbs_tidy);
    if args.get(1).map(|s| s == "--pick").unwrap_or(false) {
        let (pick, dirs) = pick_from_args(&args[2..]);
        let dirs: Vec<PathBuf> = dirs.into_iter().filter(|d| d.is_dir()).collect();
        // папка не задана: где выбирали в прошлый раз, для сохранения — загрузки
        let last = paths::state_dir().join("pick-dir");
        let dirs = if dirs.is_empty() {
            let remembered = std::fs::read_to_string(&last).ok().map(|s| PathBuf::from(s.trim())).filter(|d| d.is_dir());
            let fallback = if pick.save { places::xdg_dir("DOWNLOAD").filter(|d| d.is_dir()).unwrap_or_else(paths::home) } else { paths::home() };
            vec![remembered.unwrap_or(fallback)]
        } else {
            dirs
        };
        let fm = app::run(move |tx| {
            let mut f = fm::Fm::new(look, cfg, vec![], tx);
            f.pick = Some(pick);
            f.tabs.clear();
            f.open_tab(dirs[0].clone());
            f.refilter_all();
            f
        });
        match fm.pick.as_ref().and_then(|p| p.done.clone()) {
            Some(v) if !v.is_empty() => {
                let dir = if v[0].is_dir() && !fm.tabs.is_empty() && v.len() == 1 && v[0].parent().is_some() { v[0].clone() } else { v[0].parent().map(|d| d.to_path_buf()).unwrap_or_default() };
                let _ = std::fs::write(&last, dir.to_string_lossy().as_bytes());
                for p in v {
                    println!("{}", p.display());
                }
                std::process::exit(0);
            }
            _ => std::process::exit(1),
        }
    }
    let trash = args[1..].iter().any(|a| a.starts_with("trash:"));
    let props = args[1..].iter().any(|a| a == "--props");
    let dirs: Vec<PathBuf> = args[1..]
        .iter()
        .filter(|a| !a.starts_with('-') && !a.starts_with("trash:"))
        .map(|a| icons::uri_path(a).unwrap_or_else(|| PathBuf::from(a)))
        .map(|p| if p.is_relative() { std::env::current_dir().unwrap_or_default().join(p) } else { p })
        .collect();
    // файл вместо папки — открыть его папку и выделить его
    let mut select = vec![];
    let dirs: Vec<PathBuf> = dirs
        .into_iter()
        .filter_map(|p| {
            if p.is_dir() {
                Some(p)
            } else if p.exists() {
                select.push(p.clone());
                p.parent().map(|d| d.to_path_buf())
            } else {
                None
            }
        })
        .collect();
    app::run(move |tx| {
        let mut f = fm::Fm::new(look, cfg, dirs, tx);
        if !select.is_empty() {
            f.vm().pending_sel = select;
        }
        if trash {
            f.go_trash();
        }
        if props {
            let t = f.v().pending_sel.clone();
            if !t.is_empty() {
                f.open_props(t);
            }
        }
        f
    });
}
