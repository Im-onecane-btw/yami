// Что знает экран входа: пользователи (/etc/passwd в пределах UID_MIN..UID_MAX из
// /etc/login.defs, с настоящей оболочкой), сеансы Wayland (*.desktop в wayland-sessions), вид,
// который выложил шелл (look_dir: look.json + wall.png), и кто входил последним (STATE, пишет сам
// экран входа). Файлы вида пишет пользователь: здесь они только данные, всё проверяется.

use std::path::{Path, PathBuf};
use yami_px::gfx::Img;

/// папка вида: её отдаёт пользователю установщик, шелл пишет сюда при каждой смене вида.
/// YAMI_GREET_LOOK — для проверок (окружение экрана входа задают greetd и наш конфиг niri)
pub fn look_dir() -> PathBuf {
    std::env::var_os("YAMI_GREET_LOOK").map(PathBuf::from).unwrap_or_else(|| "/var/lib/yami-greet/look".into())
}
/// память экрана входа (последний пользователь и сеанс): папка пользователя greetd
pub const STATE: &str = "/var/lib/yami-greet/state";
/// шрифты экрана входа (Tiny5, DotGothic16): кладёт установщик
pub const FONTS: &str = "/usr/local/share/yami-greet/fonts";

#[derive(Clone, Debug)]
pub struct User {
    pub name: String,
    /// имя из GECOS («Иван Петров»), пустое — нет
    pub real: String,
}

#[derive(Clone, Debug)]
pub struct Session {
    /// имя файла без .desktop (XDG_SESSION_DESKTOP)
    pub id: String,
    pub name: String,
    pub cmd: Vec<String>,
    /// DesktopNames через «:» (XDG_CURRENT_DESKTOP)
    pub desktops: String,
}

fn login_defs() -> (u32, u32) {
    let t = std::fs::read_to_string("/etc/login.defs").unwrap_or_default();
    let get = |k: &str, d: u32| t.lines().filter_map(|l| l.trim().strip_prefix(k)).find_map(|v| v.split_whitespace().next()?.parse().ok()).unwrap_or(d);
    (get("UID_MIN", 1000), get("UID_MAX", 60000))
}

/// люди, которые могут войти: по uid, без nologin/false
pub fn users() -> Vec<User> {
    let (lo, hi) = login_defs();
    let t = std::fs::read_to_string("/etc/passwd").unwrap_or_default();
    let mut v: Vec<(u32, User)> = t
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split(':').collect();
            if f.len() < 7 {
                return None;
            }
            let uid: u32 = f[2].parse().ok()?;
            let sh = f[6].trim();
            if uid < lo || uid > hi || sh.is_empty() || sh.ends_with("nologin") || sh.ends_with("/false") || f[0].is_empty() {
                return None;
            }
            let real = f[4].split(',').next().unwrap_or("").trim().to_string();
            Some((uid, User { name: f[0].to_string(), real }))
        })
        .collect();
    v.sort_by_key(|x| x.0);
    v.into_iter().map(|x| x.1).take(64).collect()
}

/// Exec из .desktop → argv: кавычки и обратная косая, коды полей (%f, %U…) — прочь
fn split_exec(s: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let (mut q, mut any, mut it) = (false, false, s.chars());
    while let Some(c) = it.next() {
        match c {
            '"' => {
                q = !q;
                any = true;
            }
            '\\' => {
                if let Some(n) = it.next() {
                    cur.push(n);
                }
            }
            c if c.is_whitespace() && !q => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                any = false;
            }
            c => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out.retain(|a| !(a.len() == 2 && a.starts_with('%')));
    out
}

fn session_dirs() -> Vec<PathBuf> {
    let dirs = std::env::var("XDG_DATA_DIRS").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    let mut v: Vec<PathBuf> = dirs.split(':').filter(|d| d.starts_with('/')).map(|d| Path::new(d).join("wayland-sessions")).collect();
    for d in ["/usr/local/share/wayland-sessions", "/usr/share/wayland-sessions"] {
        if !v.iter().any(|x| x == Path::new(d)) {
            v.push(d.into());
        }
    }
    v
}

/// сеансы Wayland; с одинаковым именем файла — первый по XDG_DATA_DIRS
pub fn sessions() -> Vec<Session> {
    let mut v: Vec<Session> = vec![];
    for d in session_dirs() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "desktop")).collect();
        files.sort();
        for p in files {
            let id = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            if v.iter().any(|s| s.id == id) {
                continue;
            }
            let Ok(t) = std::fs::read_to_string(&p) else { continue };
            let mut sec = false;
            let (mut name, mut exec, mut desk, mut hidden) = (String::new(), String::new(), String::new(), false);
            for l in t.lines() {
                let l = l.trim();
                if l.starts_with('[') {
                    sec = l == "[Desktop Entry]";
                    continue;
                }
                let Some((k, val)) = l.split_once('=').filter(|_| sec) else { continue };
                match k.trim() {
                    "Name" => name = val.trim().to_string(),
                    "Exec" => exec = val.trim().to_string(),
                    "DesktopNames" => desk = val.trim().trim_end_matches(';').replace(';', ":"),
                    "Hidden" | "NoDisplay" => hidden |= val.trim() == "true",
                    _ => {}
                }
            }
            let cmd = split_exec(&exec);
            if hidden || cmd.is_empty() {
                continue;
            }
            v.push(Session { id: id.clone(), name: if name.is_empty() { id } else { name }, cmd, desktops: desk });
        }
    }
    v
}

impl Session {
    /// окружение сеанса: тип, имя, рабочий стол
    pub fn env(&self) -> Vec<String> {
        let mut e = vec!["XDG_SESSION_TYPE=wayland".to_string(), format!("XDG_SESSION_DESKTOP={}", self.id)];
        if !self.desktops.is_empty() {
            e.push(format!("XDG_CURRENT_DESKTOP={}", self.desktops));
        }
        e
    }
}

/// последний вход: пользователь и сеанс
pub fn last() -> (String, String) {
    let t = std::fs::read_to_string(Path::new(STATE).join("last")).unwrap_or_default();
    let mut it = t.lines().map(|l| l.trim().to_string());
    (it.next().unwrap_or_default(), it.next().unwrap_or_default())
}

pub fn remember(user: &str, session: &str) {
    let d = Path::new(STATE);
    if d.is_dir() {
        let _ = yami_px::paths::write_atomic(&d.join("last"), &format!("{user}\n{session}\n"));
    }
}

/// вид, выложенный шеллом: настройки в формате yami-shell.json
pub fn look_json() -> serde_json::Value {
    let p = look_dir().join("look.json");
    // больше мегабайта — не наш файл
    if std::fs::metadata(&p).map(|m| m.len() > 1 << 20).unwrap_or(true) {
        return serde_json::Value::Null;
    }
    std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(serde_json::Value::Null)
}

/// обои: картинка шелла, уже обрезанная и в тонах палитры. Не больше 8K по стороне
pub fn wall() -> Option<Img> {
    let p = look_dir().join("wall.png");
    let r = image::ImageReader::open(p).ok()?.with_guessed_format().ok()?;
    let mut d = r;
    let mut lim = image::Limits::default();
    lim.max_image_width = Some(8192);
    lim.max_image_height = Some(8192);
    lim.max_alloc = Some(300 << 20);
    d.limits(lim);
    let img = d.decode().ok()?.to_rgba8();
    let out = yami_px::icons::from_rgba(&img);
    drop(img);
    yami_px::trim_heap();
    Some(out)
}

pub fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname").map(|s| s.trim().to_string()).unwrap_or_default()
}

/// язык системы, когда в виде не задан: LANG greetd или /etc/locale.conf
pub fn sys_lang() -> String {
    if let Some(l) = ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty())) {
        return l;
    }
    let t = std::fs::read_to_string("/etc/locale.conf").unwrap_or_default();
    t.lines().find_map(|l| l.strip_prefix("LANG=")).map(|v| v.trim_matches('"').to_string()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::split_exec;
    #[test]
    fn exec() {
        assert_eq!(split_exec("niri-session"), vec!["niri-session"]);
        assert_eq!(split_exec(r#"env A=1 "/opt/my app/run" %U"#), vec!["env", "A=1", "/opt/my app/run"]);
        assert_eq!(split_exec(r#"sh -c "exec \"x\"""#), vec!["sh", "-c", "exec \"x\""]);
    }
}
