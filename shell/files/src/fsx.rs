// Файлы на диске: запись каталога, сортировка, размеры и даты по-человечески.

use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub dir: bool,
    pub link: bool,
    /// ссылка в никуда
    pub broken: bool,
    pub size: u64,
    pub mtime: i64,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub hidden: bool,
    pub mime: &'static str,
    /// для корзины: откуда удалён и когда
    pub orig: Option<PathBuf>,
    pub deleted: i64,
    /// у результатов поиска: каталог, где нашлось
    pub place: Option<PathBuf>,
}

impl Entry {
    pub fn exec(&self) -> bool {
        !self.dir && self.mode & 0o111 != 0
    }
    pub fn from_path(p: &Path) -> Option<Entry> {
        let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| p.to_string_lossy().to_string());
        let lm = std::fs::symlink_metadata(p).ok()?;
        let link = lm.file_type().is_symlink();
        let (m, broken) = if link {
            match std::fs::metadata(p) {
                Ok(m) => (m, false),
                Err(_) => (lm.clone(), true),
            }
        } else {
            (lm, false)
        };
        let dir = m.is_dir();
        let mime = if dir { "inode/directory" } else { crate::mime::guess(&name, p, m.permissions().mode()) };
        Some(Entry {
            hidden: name.starts_with('.'),
            name,
            path: p.to_path_buf(),
            dir,
            link,
            broken,
            size: if dir { 0 } else { m.len() },
            mtime: m.mtime(),
            mode: m.permissions().mode(),
            uid: m.uid(),
            gid: m.gid(),
            mime,
            orig: None,
            deleted: 0,
            place: None,
        })
    }
}

/// содержимое каталога (без «.» и «..»); Err — нет доступа или не каталог
pub fn list(dir: &Path) -> Result<Vec<Entry>, String> {
    let rd = std::fs::read_dir(dir).map_err(|e| err_text(&e))?;
    let mut v = Vec::with_capacity(64);
    for e in rd.flatten() {
        if let Some(en) = Entry::from_path(&e.path()) {
            v.push(en);
        }
    }
    Ok(v)
}

pub fn err_text(e: &std::io::Error) -> String {
    crate::i18n::tr(&err_ru(e)).to_string()
}

fn err_ru(e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => "нет доступа".into(),
        std::io::ErrorKind::NotFound => "не найдено".into(),
        std::io::ErrorKind::AlreadyExists => "уже существует".into(),
        _ => {
            if e.raw_os_error() == Some(libc::ENOTDIR) {
                "это не папка".into()
            } else if e.raw_os_error() == Some(libc::ENOSPC) {
                "нет места на диске".into()
            } else if e.raw_os_error() == Some(libc::EROFS) {
                "только для чтения".into()
            } else {
                e.to_string()
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sort {
    Name,
    Size,
    Time,
    Kind,
}

impl Sort {
    pub fn id(self) -> &'static str {
        match self {
            Sort::Name => "name",
            Sort::Size => "size",
            Sort::Time => "time",
            Sort::Kind => "type",
        }
    }
    pub fn from_id(s: &str) -> Sort {
        match s {
            "size" => Sort::Size,
            "time" => Sort::Time,
            "type" => Sort::Kind,
            _ => Sort::Name,
        }
    }
}

/// «естественное» сравнение: file2 < file10 (общее с шеллом, yami-px)
pub use crate::paths::natural;

pub fn sort(v: &mut [Entry], by: Sort, desc: bool, dirs_first: bool) {
    v.sort_by(|a, b| {
        if dirs_first && a.dir != b.dir {
            return b.dir.cmp(&a.dir);
        }
        let o = match by {
            Sort::Name => natural(&a.name, &b.name),
            Sort::Size => a.size.cmp(&b.size).then_with(|| natural(&a.name, &b.name)),
            Sort::Time => a.mtime.max(a.deleted).cmp(&b.mtime.max(b.deleted)).then_with(|| natural(&a.name, &b.name)),
            Sort::Kind => ext(&a.name).cmp(&ext(&b.name)).then(a.mime.cmp(b.mime)).then_with(|| natural(&a.name, &b.name)),
        };
        if desc {
            o.reverse()
        } else {
            o
        }
    });
}

pub fn ext(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, e)) if !stem.is_empty() => e.to_lowercase(),
        _ => String::new(),
    }
}

/// имя без расширения: для выделения при переименовании («отчёт» в «отчёт.pdf»)
pub fn stem_len(name: &str, dir: bool) -> usize {
    if dir {
        return name.chars().count();
    }
    let lower = name.to_lowercase();
    for two in [".tar.gz", ".tar.xz", ".tar.zst", ".tar.bz2"] {
        if lower.ends_with(two) && name.len() > two.len() {
            return name[..name.len() - two.len()].chars().count();
        }
    }
    match name.rfind('.') {
        Some(i) if i > 0 => name[..i].chars().count(),
        _ => name.chars().count(),
    }
}

pub fn human(n: u64) -> String {
    let en = crate::i18n::translated();
    let units: [&str; 5] = if en { ["B", "KB", "MB", "GB", "TB"] } else { ["Б", "КБ", "МБ", "ГБ", "ТБ"] }; // i18n-ok
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < units.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} {}", units[0])
    } else if v < 10.0 {
        format!("{v:.1} {}", units[i])
    } else {
        format!("{v:.0} {}", units[i])
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Tm {
    pub year: i32,
    pub mon: i32,
    pub day: i32,
    pub hour: i32,
    pub min: i32,
    pub yday: i32,
}

pub fn local_tm(t: i64) -> Tm {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let tt = t as _;
    unsafe { libc::localtime_r(&tt, &mut tm) };
    Tm { year: tm.tm_year + 1900, mon: tm.tm_mon + 1, day: tm.tm_mday, hour: tm.tm_hour, min: tm.tm_min, yday: tm.tm_yday }
}

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// «сегодня 14:05», «вчера 09:12», «3 мар 2024»
pub fn when(t: i64) -> String {
    if t <= 0 {
        return String::new();
    }
    let a = local_tm(t);
    let n = local_tm(now());
    let hm = format!("{:02}:{:02}", a.hour, a.min);
    if a.year == n.year && a.yday == n.yday {
        return crate::tf!("сегодня {}", "today {}", "hoje {}", hm);
    }
    if (a.year == n.year && a.yday + 1 == n.yday) || (a.year + 1 == n.year && n.yday == 0 && a.mon == 12 && a.day == 31) {
        return crate::tf!("вчера {}", "yesterday {}", "ontem {}", hm);
    }
    let m = crate::i18n::month_short(a.mon);
    if a.year == n.year {
        format!("{} {} {}", a.day, m, hm)
    } else {
        format!("{} {} {}", a.day, m, a.year)
    }
}

pub fn full_time(t: i64) -> String {
    let a = local_tm(t);
    format!("{:02}.{:02}.{} {:02}:{:02}", a.day, a.mon, a.year, a.hour, a.min)
}

/// rwxr-xr-x
pub fn perm_str(mode: u32) -> String {
    let mut s = String::new();
    for sh in [6, 3, 0] {
        let b = (mode >> sh) & 7;
        s.push(if b & 4 != 0 { 'r' } else { '-' });
        s.push(if b & 2 != 0 { 'w' } else { '-' });
        s.push(if b & 1 != 0 { 'x' } else { '-' });
    }
    s
}

/// имя пользователя/группы из /etc/passwd, /etc/group
pub fn id_name(file: &str, id: u32) -> String {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|t| t.lines().find(|l| l.split(':').nth(2).and_then(|x| x.parse::<u32>().ok()) == Some(id)).map(|l| l.split(':').next().unwrap_or("").to_string()))
        .unwrap_or_else(|| id.to_string())
}

/// свободно и всего на файловой системе, где лежит p
pub fn disk_free(p: &Path) -> Option<(u64, u64)> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(p.as_os_str().as_bytes()).ok()?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    let bs = s.f_frsize as u64;
    Some((s.f_bavail as u64 * bs, s.f_blocks as u64 * bs))
}

/// свободное имя: «файл (2).txt», «папка (3)»
pub fn free_name(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    if std::fs::symlink_metadata(&p).is_err() {
        return p;
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    // уже «(n)» на конце — продолжаем счёт
    let base = match stem.rfind(" (") {
        Some(i) if stem.ends_with(')') && stem[i + 2..stem.len() - 1].parse::<u32>().is_ok() => &stem[..i],
        _ => stem,
    };
    for n in 2..10000 {
        let c = dir.join(format!("{base} ({n}){ext}"));
        if std::fs::symlink_metadata(&c).is_err() {
            return c;
        }
    }
    p
}

/// путь «~/…» для показа (общее с шеллом, yami-px)
pub use crate::paths::pretty;

/// размер каталога целиком (файлы, папки) — для свойств, в отдельной нити
pub fn du(p: &Path, stop: &std::sync::atomic::AtomicBool) -> (u64, u64, u64) {
    let (mut bytes, mut files, mut dirs) = (0u64, 0u64, 0u64);
    let mut stack = vec![p.to_path_buf()];
    while let Some(d) = stack.pop() {
        if stop.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let Ok(m) = e.metadata() else { continue };
            if m.is_dir() {
                dirs += 1;
                stack.push(e.path());
            } else {
                files += 1;
                bytes += m.len();
            }
        }
    }
    (bytes, files, dirs)
}
