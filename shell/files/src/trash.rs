// Корзина по спецификации freedesktop: ~/.local/share/Trash для домашнего диска,
// <точка монтирования>/.Trash-<uid> для остальных. Общая с Dolphin, Nautilus, gio trash.

use crate::fsx::Entry;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

fn uid() -> u32 {
    unsafe { libc::getuid() }
}

pub fn home_trash() -> PathBuf {
    crate::paths::data_dir().join("Trash")
}

/// точка монтирования, на которой лежит p
fn topdir(p: &Path) -> Option<PathBuf> {
    let dev = std::fs::symlink_metadata(p).ok()?.dev();
    let mut cur = p.to_path_buf();
    loop {
        let parent = cur.parent()?.to_path_buf();
        if std::fs::metadata(&parent).ok()?.dev() != dev {
            return Some(cur);
        }
        if parent == Path::new("/") {
            return Some(parent);
        }
        cur = parent;
    }
}

fn trash_for(p: &Path) -> Option<(PathBuf, Option<PathBuf>)> {
    let ht = home_trash();
    let _ = std::fs::create_dir_all(&ht);
    let hdev = std::fs::metadata(crate::paths::data_dir()).ok()?.dev();
    let pdev = std::fs::symlink_metadata(p).ok()?.dev();
    if hdev == pdev {
        return Some((ht, None));
    }
    let top = topdir(p)?;
    let t = top.join(format!(".Trash-{}", uid()));
    if std::fs::create_dir_all(t.join("files")).is_ok() && std::fs::create_dir_all(t.join("info")).is_ok() {
        return Some((t, Some(top)));
    }
    None
}

fn enc(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

fn dec(s: &str) -> PathBuf {
    crate::icons::uri_path(&format!("file://{}", if s.starts_with('/') { s.to_string() } else { format!("/{s}") })).unwrap_or_else(|| PathBuf::from(s))
}

fn stamp() -> String {
    let t = crate::fsx::local_tm(crate::fsx::now());
    let mut s: libc::tm = unsafe { std::mem::zeroed() };
    let now = crate::fsx::now() as libc::time_t;
    unsafe { libc::localtime_r(&now, &mut s) };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", t.year, t.mon, t.day, t.hour, t.min, s.tm_sec)
}

fn parse_stamp(s: &str) -> i64 {
    let n: Vec<i32> = s.split(|c: char| !c.is_ascii_digit()).filter(|x| !x.is_empty()).filter_map(|x| x.parse().ok()).collect();
    if n.len() < 6 {
        return 0;
    }
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    tm.tm_year = n[0] - 1900;
    tm.tm_mon = n[1] - 1;
    tm.tm_mday = n[2];
    tm.tm_hour = n[3];
    tm.tm_min = n[4];
    tm.tm_sec = n[5];
    tm.tm_isdst = -1;
    unsafe { libc::mktime(&mut tm) as i64 }
}

/// в корзину; Ok(путь в корзине)
pub fn put(p: &Path) -> Result<PathBuf, String> {
    let (t, top) = trash_for(p).ok_or_else(|| crate::i18n::tr("корзина недоступна").to_string())?;
    let files = t.join("files");
    let info = t.join("info");
    let _ = std::fs::create_dir_all(&files);
    let _ = std::fs::create_dir_all(&info);
    let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
    // имя в корзине: занятое — «имя (2)»; .trashinfo создаётся первым и атомарно (O_EXCL)
    let mut target = files.join(&name);
    let mut n = 1;
    let infofile = loop {
        let tn = target.file_name().unwrap().to_string_lossy().to_string();
        let ip = info.join(format!("{tn}.trashinfo"));
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&ip) {
            Ok(mut f) => {
                use std::io::Write;
                let shown = match &top {
                    Some(top) => p.strip_prefix(top).map(|r| r.to_string_lossy().to_string()).unwrap_or_else(|_| p.to_string_lossy().to_string()),
                    None => p.to_string_lossy().to_string(),
                };
                let _ = write!(f, "[Trash Info]\nPath={}\nDeletionDate={}\n", enc(&shown), stamp());
                break ip;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                n += 1;
                let (stem, ext) = match name.rfind('.') {
                    Some(i) if i > 0 => (&name[..i], &name[i..]),
                    _ => (name.as_str(), ""),
                };
                target = files.join(format!("{stem} ({n}){ext}"));
            }
            Err(e) => return Err(crate::fsx::err_text(&e)),
        }
    };
    if let Err(e) = std::fs::rename(p, &target) {
        let _ = std::fs::remove_file(&infofile);
        return Err(crate::fsx::err_text(&e));
    }
    Ok(target)
}

fn trash_dirs() -> Vec<(PathBuf, Option<PathBuf>)> {
    let mut v = vec![(home_trash(), None)];
    if let Ok(m) = std::fs::read_to_string("/proc/self/mounts") {
        for l in m.lines() {
            let Some(mp) = l.split(' ').nth(1) else { continue };
            let mp = PathBuf::from(mp.replace("\\040", " "));
            let t = mp.join(format!(".Trash-{}", uid()));
            if t.join("files").is_dir() && !v.iter().any(|(x, _)| *x == t) {
                v.push((t, Some(mp)));
            }
        }
    }
    v
}

pub fn list() -> Vec<Entry> {
    let mut out = vec![];
    for (t, top) in trash_dirs() {
        let Ok(rd) = std::fs::read_dir(t.join("files")) else { continue };
        for e in rd.flatten() {
            let Some(mut en) = Entry::from_path(&e.path()) else { continue };
            let info = t.join("info").join(format!("{}.trashinfo", en.name));
            if let Ok(txt) = std::fs::read_to_string(&info) {
                for l in txt.lines() {
                    if let Some(v) = l.strip_prefix("Path=") {
                        let p = dec(v);
                        en.orig = Some(match &top {
                            Some(top) if !v.starts_with('/') => top.join(p.strip_prefix("/").unwrap_or(&p)),
                            _ => p,
                        });
                    } else if let Some(v) = l.strip_prefix("DeletionDate=") {
                        en.deleted = parse_stamp(v);
                    }
                }
            }
            out.push(en);
        }
    }
    out
}

pub fn count() -> usize {
    trash_dirs().iter().map(|(t, _)| std::fs::read_dir(t.join("files")).map(|r| r.count()).unwrap_or(0)).sum()
}

fn info_of(p: &Path) -> PathBuf {
    let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    p.parent().and_then(|f| f.parent()).map(|t| t.join("info").join(format!("{name}.trashinfo"))).unwrap_or_default()
}

/// вернуть на место; занято — рядом под свободным именем
pub fn restore(e: &Entry) -> Result<PathBuf, String> {
    let orig = e.orig.clone().ok_or_else(|| crate::i18n::tr("неизвестно, откуда удалён").to_string())?;
    let dir = orig.parent().map(|p| p.to_path_buf()).unwrap_or_else(crate::paths::home);
    let _ = std::fs::create_dir_all(&dir);
    let dst = crate::fsx::free_name(&dir, &orig.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
    std::fs::rename(&e.path, &dst).map_err(|er| crate::fsx::err_text(&er))?;
    let _ = std::fs::remove_file(info_of(&e.path));
    Ok(dst)
}

pub fn forget(p: &Path) {
    let _ = std::fs::remove_file(info_of(p));
}

/// в корзине ли путь (файлы прямо в */Trash/files или .Trash-uid/files)
pub fn is_trashed(p: &Path) -> bool {
    p.parent().map(|f| f.file_name().map(|n| n == "files").unwrap_or(false) && f.parent().map(|t| t == home_trash() || t.file_name().map(|n| n.to_string_lossy().starts_with(".Trash")).unwrap_or(false)).unwrap_or(false)).unwrap_or(false)
}

pub fn all_files() -> Vec<PathBuf> {
    trash_dirs().iter().flat_map(|(t, _)| std::fs::read_dir(t.join("files")).map(|r| r.flatten().map(|e| e.path()).collect::<Vec<_>>()).unwrap_or_default()).collect()
}
