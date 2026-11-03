// Типы файлов по shared-mime-info (globs2, subclasses) и чем их открывать (mimeapps.list,
// MimeType в .desktop). Без расширения — смотрим первые байты: ELF, #!, картинки, текст.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

struct Db {
    ext: HashMap<String, &'static str>,
    /// полные имена (Makefile) и шаблоны посложнее (*.tar.gz → по суффиксу)
    names: HashMap<String, &'static str>,
    suffix: Vec<(String, &'static str)>,
    parents: HashMap<&'static str, Vec<&'static str>>,
    comments: HashMap<&'static str, String>,
}

fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

fn data_dirs() -> Vec<PathBuf> {
    let mut v = vec![crate::paths::data_dir()];
    let xdg = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    v.extend(xdg.split(':').filter(|d| !d.is_empty()).map(PathBuf::from));
    v
}

fn db() -> &'static Db {
    static DB: OnceLock<Db> = OnceLock::new();
    DB.get_or_init(|| {
        let mut intern: HashMap<String, &'static str> = HashMap::new();
        let mut get = |m: &str| -> &'static str {
            if let Some(s) = intern.get(m) {
                return s;
            }
            let s = leak(m);
            intern.insert(m.to_string(), s);
            s
        };
        let mut db = Db { ext: HashMap::new(), names: HashMap::new(), suffix: vec![], parents: HashMap::new(), comments: HashMap::new() };
        let mut weights: HashMap<String, u32> = HashMap::new();
        for d in data_dirs().iter().rev() {
            if let Ok(t) = std::fs::read_to_string(d.join("mime/globs2")) {
                for l in t.lines() {
                    if l.starts_with('#') {
                        continue;
                    }
                    let mut it = l.splitn(4, ':');
                    let (Some(w), Some(m), Some(g)) = (it.next(), it.next(), it.next()) else { continue };
                    let w: u32 = w.parse().unwrap_or(50);
                    let m = get(m);
                    if let Some(e) = g.strip_prefix("*.").filter(|e| !e.contains(['*', '?', '['])) {
                        let key = e.to_lowercase();
                        if key.contains('.') {
                            db.suffix.push((format!(".{key}"), m));
                        } else if weights.get(&key).map(|&ow| w > ow).unwrap_or(true) {
                            weights.insert(key.clone(), w);
                            db.ext.insert(key, m);
                        }
                    } else if !g.contains(['*', '?', '[']) {
                        db.names.insert(g.to_string(), m);
                    }
                }
            }
            if let Ok(t) = std::fs::read_to_string(d.join("mime/subclasses")) {
                for l in t.lines() {
                    if let Some((a, b)) = l.split_once(' ') {
                        let (a, b) = (get(a), get(b));
                        db.parents.entry(a).or_default().push(b);
                    }
                }
            }
        }
        db.suffix.sort_by_key(|(s, _)| std::cmp::Reverse(s.len()));
        let _ = &mut db.comments;
        db
    })
}

/// тип по имени, а если не вышло — по содержимому
pub fn guess(name: &str, path: &Path, mode: u32) -> &'static str {
    let d = db();
    if let Some(m) = d.names.get(name) {
        return m;
    }
    let low = name.to_lowercase();
    for (s, m) in &d.suffix {
        if low.ends_with(s.as_str()) {
            return m;
        }
    }
    if let Some((_, e)) = low.rsplit_once('.') {
        if let Some(m) = d.ext.get(e) {
            return m;
        }
    }
    sniff(path, mode)
}

fn sniff(path: &Path, mode: u32) -> &'static str {
    use std::io::Read;
    let mut b = [0u8; 512];
    let n = std::fs::File::open(path).and_then(|mut f| f.read(&mut b)).unwrap_or(0);
    let b = &b[..n];
    if n == 0 {
        return if mode & 0o170000 == 0o100000 { "application/x-zerosize" } else { "application/octet-stream" };
    }
    if b.starts_with(b"\x7fELF") {
        return if b.len() > 17 && b[16] == 3 { "application/x-sharedlib" } else { "application/x-executable" };
    }
    if b.starts_with(b"#!") {
        let l = String::from_utf8_lossy(&b[..b.iter().position(|&c| c == b'\n').unwrap_or(n)]).to_string();
        return if l.contains("python") { "text/x-python" } else { "application/x-shellscript" };
    }
    if b.starts_with(b"\x89PNG") {
        return "image/png";
    }
    if b.starts_with(&[0xff, 0xd8, 0xff]) {
        return "image/jpeg";
    }
    if b.starts_with(b"%PDF") {
        return "application/pdf";
    }
    if b.starts_with(b"PK\x03\x04") {
        return "application/zip";
    }
    if b.starts_with(&[0x1f, 0x8b]) {
        return "application/gzip";
    }
    // текст: валидный UTF-8 без нулей
    if !b.contains(&0) && (std::str::from_utf8(b).is_ok() || std::str::from_utf8(&b[..n.saturating_sub(4)]).is_ok()) {
        return "text/plain";
    }
    "application/octet-stream"
}

/// тип и все его предки (text/x-rust → text/plain), для подбора программ
pub fn ancestry(m: &'static str) -> Vec<&'static str> {
    let d = db();
    let mut out = vec![m];
    let mut i = 0;
    while i < out.len() && out.len() < 12 {
        if let Some(ps) = d.parents.get(out[i]) {
            for p in ps {
                if !out.contains(p) {
                    out.push(p);
                }
            }
        }
        i += 1;
    }
    if m.starts_with("text/") && !out.contains(&"text/plain") {
        out.push("text/plain");
    }
    if !out.contains(&"application/octet-stream") && !m.starts_with("inode/") {
        out.push("application/octet-stream");
    }
    out
}

/// группа для значка и подписи
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Dir,
    Image,
    Video,
    Audio,
    Text,
    Code,
    Doc,
    Pdf,
    Archive,
    Exec,
    Font,
    Disk,
    Package,
    Web,
    Other,
}

pub fn kind(m: &str) -> Kind {
    if m == "inode/directory" {
        return Kind::Dir;
    }
    let (top, sub) = m.split_once('/').unwrap_or((m, ""));
    match top {
        "image" => return Kind::Image,
        "video" => return Kind::Video,
        "audio" => return Kind::Audio,
        "font" => return Kind::Font,
        _ => {}
    }
    if sub == "pdf" || sub.contains("postscript") || sub.contains("djvu") || sub.contains("epub") {
        return Kind::Pdf;
    }
    if sub == "html" || sub == "xhtml+xml" {
        return Kind::Web;
    }
    if ["zip", "x-tar", "gzip", "x-7z-compressed", "vnd.rar", "x-rar", "zstd", "x-xz", "x-bzip2", "x-bzip", "x-compressed-tar", "x-xz-compressed-tar", "x-zstd-compressed-tar", "x-bzip2-compressed-tar", "x-lzma", "x-cpio", "java-archive", "x-lz4"]
        .contains(&sub)
    {
        return Kind::Archive;
    }
    if ["x-iso9660-image", "x-raw-disk-image", "x-cd-image", "x-qemu-disk", "x-apple-diskimage"].contains(&sub) {
        return Kind::Disk;
    }
    if ["vnd.debian.binary-package", "x-rpm", "x-alpm-package", "vnd.flatpak.ref", "vnd.appimage", "x-msi", "vnd.microsoft.portable-executable", "x-msdownload", "x-ms-dos-executable"].contains(&sub) {
        return Kind::Package;
    }
    if ["x-executable", "x-sharedlib", "x-pie-executable", "x-shellscript", "x-desktop"].contains(&sub) {
        return Kind::Exec;
    }
    if sub.contains("opendocument") || sub.contains("msword") || sub.contains("officedocument") || sub.contains("ms-excel") || sub.contains("ms-powerpoint") || sub == "rtf" {
        return Kind::Doc;
    }
    if top == "text" {
        if sub == "plain" || sub == "markdown" || sub == "x-log" {
            return Kind::Text;
        }
        return Kind::Code;
    }
    if ["json", "xml", "javascript", "x-yaml", "toml", "x-php", "sql", "x-ruby", "x-perl", "x-python", "x-sh"].contains(&sub) {
        return Kind::Code;
    }
    if sub == "x-zerosize" {
        return Kind::Text;
    }
    Kind::Other
}

/// «изображение · PNG»
pub fn label(m: &str, name: &str) -> String {
    let k = kind(m);
    let what = match k {
        Kind::Dir => "папка",
        Kind::Image => "изображение",
        Kind::Video => "видео",
        Kind::Audio => "аудио",
        Kind::Text => "текст",
        Kind::Code => "код",
        Kind::Doc => "документ",
        Kind::Pdf => "документ",
        Kind::Archive => "архив",
        Kind::Exec => "программа",
        Kind::Font => "шрифт",
        Kind::Disk => "образ диска",
        Kind::Package => "пакет",
        Kind::Web => "веб-страница",
        Kind::Other => "файл",
    };
    let what = crate::i18n::tr(what);
    let e = crate::fsx::ext(name);
    if k == Kind::Dir || e.is_empty() || e.len() > 6 {
        what.to_string()
    } else {
        format!("{what} · {}", e.to_uppercase())
    }
}

// ── чем открывать ──────────────────────────────────────────────────────────
fn mimeapps_files() -> Vec<PathBuf> {
    let mut v = vec![crate::paths::config_dir().join("mimeapps.list")];
    let cfg = std::env::var("XDG_CONFIG_DIRS").unwrap_or_else(|_| "/etc/xdg".into());
    v.extend(cfg.split(':').filter(|d| !d.is_empty()).map(|d| Path::new(d).join("mimeapps.list")));
    for d in data_dirs() {
        v.push(d.join("applications/mimeapps.list"));
    }
    v
}

fn read_list(section: &str) -> Vec<HashMap<String, Vec<String>>> {
    mimeapps_files()
        .iter()
        .map(|f| {
            let mut m: HashMap<String, Vec<String>> = HashMap::new();
            let Ok(t) = std::fs::read_to_string(f) else { return m };
            let mut inside = false;
            for l in t.lines() {
                let l = l.trim();
                if l.starts_with('[') {
                    inside = l == section;
                } else if inside {
                    if let Some((k, v)) = l.split_once('=') {
                        m.entry(k.trim().to_string()).or_default().extend(v.split(';').map(|x| x.trim().trim_end_matches(".desktop").to_string()).filter(|x| !x.is_empty()));
                    }
                }
            }
            m
        })
        .collect()
}

pub struct Apps {
    pub all: Vec<crate::icons::App>,
    defaults: Vec<HashMap<String, Vec<String>>>,
    added: Vec<HashMap<String, Vec<String>>>,
    removed: Vec<HashMap<String, Vec<String>>>,
}

impl Apps {
    pub fn load() -> Apps {
        Apps { all: crate::icons::load_apps(), defaults: read_list("[Default Applications]"), added: read_list("[Added Associations]"), removed: read_list("[Removed Associations]") }
    }

    fn by_id(&self, id: &str) -> Option<&crate::icons::App> {
        self.all.iter().find(|a| a.id == id)
    }

    /// программа по умолчанию для типа (с учётом предков)
    pub fn default_for(&self, m: &'static str) -> Option<&crate::icons::App> {
        for t in ancestry(m) {
            for d in &self.defaults {
                if let Some(ids) = d.get(t) {
                    if let Some(a) = ids.iter().find_map(|i| self.by_id(i)) {
                        return Some(a);
                    }
                }
            }
            if let Some(a) = self.for_type(t).into_iter().next() {
                return Some(a);
            }
        }
        None
    }

    fn for_type(&self, t: &str) -> Vec<&crate::icons::App> {
        let removed: Vec<&String> = self.removed.iter().filter_map(|m| m.get(t)).flatten().collect();
        let mut v: Vec<&crate::icons::App> = self.added.iter().filter_map(|m| m.get(t)).flatten().filter_map(|i| self.by_id(i)).collect();
        for a in self.all.iter().filter(|a| a.mime.iter().any(|x| x == t)) {
            if !v.iter().any(|b| b.id == a.id) {
                v.push(a);
            }
        }
        v.retain(|a| !removed.contains(&&a.id));
        v
    }

    /// все подходящие программы: сначала умолчание, потом по типу, потом по предкам
    pub fn candidates(&self, m: &'static str) -> Vec<&crate::icons::App> {
        let mut out: Vec<&crate::icons::App> = vec![];
        if let Some(d) = self.default_for(m) {
            out.push(d);
        }
        for t in ancestry(m) {
            for a in self.for_type(t) {
                if !out.iter().any(|b| b.id == a.id) {
                    out.push(a);
                }
            }
        }
        out
    }

    /// запомнить программу по умолчанию (~/.config/mimeapps.list)
    pub fn set_default(&mut self, m: &str, id: &str) {
        let p = crate::paths::config_dir().join("mimeapps.list");
        let text = std::fs::read_to_string(&p).unwrap_or_default();
        let mut out: Vec<String> = vec![];
        let mut inside = false;
        let mut done = false;
        let line = format!("{m}={id}.desktop;");
        for l in text.lines() {
            let t = l.trim();
            if t.starts_with('[') {
                if inside && !done {
                    out.push(line.clone());
                    done = true;
                }
                inside = t == "[Default Applications]";
            } else if inside && t.split_once('=').map(|(k, _)| k.trim() == m).unwrap_or(false) {
                if !done {
                    out.push(line.clone());
                    done = true;
                }
                continue;
            }
            out.push(l.to_string());
        }
        if !done {
            if !inside {
                if !out.is_empty() {
                    out.push(String::new());
                }
                out.push("[Default Applications]".into());
            }
            out.push(line);
        }
        let _ = crate::paths::write_atomic(&p, &(out.join("\n") + "\n"));
        self.defaults = read_list("[Default Applications]");
    }
}
