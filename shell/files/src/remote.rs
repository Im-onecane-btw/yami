// Телефоны, фотоаппараты и сеть через gvfs (`gio`): MTP/PTP (Android, камеры), AFC (iPhone),
// сетевые ресурсы smb/sftp/ftp/webdav/nfs. Сами файлы открываются как обычные папки через
// FUSE-точку gvfs (/run/user/UID/gvfs/…), поэтому всё остальное в менеджере работает как с диском.
// Плюс сетевые монтирования ядра (cifs, nfs, sshfs, rclone) из /proc/self/mounts.
// Пароли идут в `gio mount` через stdin, не в argv и не на диск.

use crate::places::{Dev, DevKind};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// чем ответить на вопросы `gio mount`
#[derive(Default, Clone)]
pub struct Auth {
    pub user: String,
    pub domain: String,
    pub pass: String,
    /// согласие на вопрос сервера (например, неизвестный ключ sftp-сервера)
    pub accept: bool,
}

impl Drop for Auth {
    fn drop(&mut self) {
        crate::ui::wipe(&mut self.pass);
    }
}

/// чем закончилось подключение
pub enum Mounted {
    Ok(Option<PathBuf>),
    /// серверу нужен логин и пароль (или прошлый не подошёл)
    NeedAuth { retry: bool },
    /// сервер спрашивает подтверждение: текст вопроса
    Question(String),
    Err(String),
}

fn gio() -> Command {
    let mut c = Command::new("gio");
    // подсказки gio («User», «Password») переводятся — нужен английский
    c.env("LC_ALL", "C").env("LANG", "C");
    c
}

/// адрес, который поймёт gio: «host/share» без схемы → smb://
pub fn normalize(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        return String::new();
    }
    if let Some(r) = s.strip_prefix("\\\\") {
        return format!("smb://{}", r.replace('\\', "/"));
    }
    if s.contains("://") {
        s.to_string()
    } else {
        format!("smb://{s}")
    }
}

/// адрес без пароля (его не храним и не показываем)
pub fn strip_secret(uri: &str) -> String {
    let Some((scheme, rest)) = uri.split_once("://") else { return uri.to_string() };
    let (auth, tail) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    match auth.rsplit_once('@') {
        Some((cred, host)) => {
            let user = cred.split(':').next().unwrap_or("");
            format!("{scheme}://{user}@{host}{tail}")
        }
        None => uri.to_string(),
    }
}

/// пароль из адреса («sftp://me:pw@host») — отдельно: в argv его увидит любой процесс (ps)
pub fn split_secret(uri: &str) -> (String, Option<String>) {
    let Some((_, rest)) = uri.split_once("://") else { return (uri.to_string(), None) };
    let auth = rest.split('/').next().unwrap_or("");
    let pass = auth.rsplit_once('@').and_then(|(cred, _)| cred.split_once(':')).map(|(_, p)| p.to_string());
    (strip_secret(uri), pass)
}

/// схемы, которые gio умеет монтировать как сеть
pub fn known_scheme(uri: &str) -> bool {
    let s = uri.split("://").next().unwrap_or("");
    matches!(s, "smb" | "sftp" | "ssh" | "ftp" | "ftps" | "dav" | "davs" | "nfs" | "afp" | "webdav" | "webdavs")
}

/// короткое имя ресурса: «share на host»
pub fn title_of(uri: &str) -> String {
    let rest = uri.split_once("://").map(|x| x.1).unwrap_or(uri);
    let rest = rest.trim_end_matches('/');
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i + 1..]),
        None => (rest, ""),
    };
    let host = host.rsplit('@').next().unwrap_or(host);
    match path.rsplit('/').next().filter(|s| !s.is_empty()) {
        Some(last) => crate::tf!("{} на {}", "{} on {}", "{} em {}", last, host),
        None => host.to_string(),
    }
}

/// локальная FUSE-точка для адреса gvfs (уже подключённого)
pub fn local_path(uri: &str) -> Option<PathBuf> {
    let o = gio().args(["info", "-a", "standard::type", "--", uri]).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    String::from_utf8_lossy(&o.stdout).lines().find_map(|l| l.strip_prefix("local path: ").map(|p| PathBuf::from(p.trim()))).filter(|p| p.is_dir())
}

struct GVol {
    name: String,
    monitor: String,
    root: String,
}

/// тома и подключения gvfs, кроме дисков (их показывает lsblk)
pub fn gvfs_devices() -> Vec<Dev> {
    let Ok(o) = gio().args(["mount", "-li"]).stdin(Stdio::null()).stderr(Stdio::null()).output() else { return vec![] };
    let text = String::from_utf8_lossy(&o.stdout);
    let mut vols: Vec<GVol> = vec![];
    // подключения: имя, адрес, монитор
    let mut mounts: Vec<(String, String, String)> = vec![];
    // что сейчас разбираем: 1 — том, 2 — подключение
    let mut cur = 0;
    for l in text.lines() {
        let s = l.trim_start();
        if let Some(rest) = s.strip_prefix("Volume(").and_then(|r| r.split_once("): ")) {
            vols.push(GVol { name: rest.1.to_string(), monitor: String::new(), root: String::new() });
            cur = 1;
        } else if let Some(rest) = s.strip_prefix("Mount(").and_then(|r| r.split_once("): ")) {
            let (name, uri) = rest.1.split_once(" -> ").unwrap_or((rest.1, ""));
            mounts.push((name.to_string(), uri.trim().to_string(), String::new()));
            cur = 2;
        } else if s.starts_with("Drive(") {
            cur = 0;
        } else if let Some(t) = s.strip_prefix("Type: ") {
            let mon = t.split('(').nth(1).unwrap_or("").trim_end_matches(')').to_string();
            let mon = if mon.is_empty() { t.to_string() } else { mon };
            match cur {
                1 => vols.last_mut().map(|v| v.monitor = mon),
                2 => mounts.last_mut().map(|m| m.2 = mon),
                _ => None,
            };
        } else if cur == 1 {
            if let Some(r) = s.strip_prefix("activation_root=") {
                if let Some(v) = vols.last_mut() {
                    v.root = r.trim().to_string();
                }
            }
        }
    }
    let kind_of = |monitor: &str, uri: &str| -> Option<DevKind> {
        let scheme = uri.split("://").next().unwrap_or("");
        let m = monitor.to_ascii_lowercase();
        if m.contains("mtp") || scheme == "mtp" || m.contains("afc") || scheme == "afc" {
            Some(DevKind::Phone)
        } else if m.contains("gphoto2") || scheme == "gphoto2" {
            Some(DevKind::Camera)
        } else if known_scheme(uri) {
            Some(DevKind::Net)
        } else {
            None
        }
    };
    let mut out: Vec<Dev> = vec![];
    for v in &vols {
        // диски (udisks2) уже есть из lsblk; Goa и прочее без адреса пропускаем. У томов телефонов
        // и камер тоже есть unix-device (/dev/bus/usb/…): по нему их не отсеивать — раньше из-за
        // этого телефон в панели не появлялся, пока его не подключали руками
        if v.monitor.contains("UDisks2") || v.root.is_empty() {
            continue;
        }
        let Some(kind) = kind_of(&v.monitor, &v.root) else { continue };
        let m = mounts.iter().find(|m| m.1.trim_end_matches('/') == v.root.trim_end_matches('/') || m.0 == v.name && !m.2.contains("UDisks2"));
        let mount = m.and_then(|m| local_path(&m.1));
        out.push(Dev { uri: m.map(|m| m.1.clone()).unwrap_or_else(|| v.root.clone()), label: v.name.clone(), mount, removable: true, kind, ..Default::default() });
    }
    // подключения без тома: сетевые ресурсы, телефон, подключённый адресом
    for (name, uri, mon) in &mounts {
        if mon.contains("UDisks2") || uri.starts_with("file://") || out.iter().any(|d| d.uri.trim_end_matches('/') == uri.trim_end_matches('/')) {
            continue;
        }
        let Some(kind) = kind_of(mon, uri) else { continue };
        let label = if kind == DevKind::Net { title_of(uri) } else { name.clone() };
        out.push(Dev { uri: uri.clone(), label, mount: local_path(uri), removable: true, kind, ..Default::default() });
    }
    out.extend(usb_phones_without_backend());
    out
}

/// сетевые файловые системы ядра и FUSE: cifs, nfs, sshfs, rclone…
pub fn kernel_net() -> Vec<Dev> {
    let Ok(t) = std::fs::read_to_string("/proc/self/mounts") else { return vec![] };
    let mut out = vec![];
    for l in t.lines() {
        let f: Vec<&str> = l.split(' ').collect();
        if f.len() < 3 {
            continue;
        }
        let fs = f[2];
        if !matches!(fs, "cifs" | "smb3" | "nfs" | "nfs4" | "fuse.sshfs" | "fuse.rclone" | "9p" | "davfs" | "fuse.davfs2") {
            continue;
        }
        // /proc/mounts кодирует пробел как \040
        let unesc = |s: &str| s.replace("\\040", " ").replace("\\011", "\t").replace("\\134", "\\");
        let mp = PathBuf::from(unesc(f[1]));
        let src = unesc(f[0]);
        let name = mp.file_name().map(|s| s.to_string_lossy().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| src.clone());
        out.push(Dev { dev: src, label: name, mount: Some(mp), removable: true, fstype: fs.to_string(), kind: DevKind::Net, ..Default::default() });
    }
    out
}

/// подключить адрес gvfs или раздел («/dev/sdb1» — зашифрованный: пароль спросит gio, udisks
/// откроет LUKS и смонтирует содержимое); на вопросы отвечаем из `auth`, чего нет — спрашиваем
pub fn mount(uri: &str, auth: &Auth) -> Mounted {
    let dev = uri.starts_with("/dev/");
    let anon = !dev && auth.user.is_empty() && auth.pass.is_empty() && !uri.contains('@');
    let mut c = gio();
    c.arg("mount");
    // без логина — гостем: иначе gio сразу спросит имя и пароль
    if anon {
        c.arg("-a");
    }
    if dev {
        c.arg("-d").arg(uri);
    } else {
        c.arg("--").arg(uri);
    }
    let mut ch = match c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
        Ok(ch) => ch,
        Err(_) => return Mounted::Err(crate::i18n::tr("нет gio (пакет glib2 / gvfs)").to_string()),
    };
    let (Some(mut out), Some(mut inp)) = (ch.stdout.take(), ch.stdin.take()) else { return Mounted::Err("gio".into()) };
    let mut err = ch.stderr.take();
    // сторож: подключение не дольше 40 с (телефон ждёт разблокировки)
    let pid = ch.id() as i32;
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let done = done.clone();
        std::thread::spawn(move || {
            let t = Instant::now();
            while t.elapsed() < Duration::from_secs(40) {
                std::thread::sleep(Duration::from_millis(200));
                if done.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
            }
            unsafe { libc::kill(pid, libc::SIGTERM) };
        });
    }
    let mut buf = String::new();
    let mut asked_pass = 0;
    let mut result: Option<Mounted> = None;
    let mut b = [0u8; 512];
    loop {
        let n = match out.read(&mut b) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        buf.push_str(&String::from_utf8_lossy(&b[..n]));
        // подсказка — последняя строка, кончается на «: »
        let Some(line) = buf.rsplit('\n').next().filter(|l| l.ends_with(": ")).map(|l| l.to_string()) else { continue };
        let label = line.split([' ', ':']).next().unwrap_or("");
        let answer: Option<String> = match label {
            "User" => {
                if anon || auth.user.is_empty() && auth.pass.is_empty() {
                    None
                } else {
                    Some(auth.user.clone())
                }
            }
            "Domain" => Some(auth.domain.clone()),
            "Password" => {
                asked_pass += 1;
                if asked_pass > 1 || auth.pass.is_empty() && !auth.accept {
                    result = Some(Mounted::NeedAuth { retry: asked_pass > 1 });
                    None
                } else {
                    Some(auth.pass.clone())
                }
            }
            "Choice" => {
                if auth.accept {
                    Some("1".into())
                } else {
                    let q: Vec<&str> = buf.lines().filter(|l| !l.starts_with('[') && !l.starts_with("Choice") && !l.trim().is_empty()).collect();
                    result = Some(Mounted::Question(q.join(" ")));
                    None
                }
            }
            _ => continue,
        };
        match answer {
            Some(mut a) => {
                a.push('\n');
                let _ = inp.write_all(a.as_bytes());
                let _ = inp.flush();
                crate::ui::wipe(&mut a);
                buf.clear();
            }
            None => {
                if result.is_none() {
                    result = Some(Mounted::NeedAuth { retry: false });
                }
                let _ = ch.kill();
                break;
            }
        }
    }
    drop(inp);
    let st = ch.wait();
    done.store(true, std::sync::atomic::Ordering::Relaxed);
    if let Some(r) = result {
        return r;
    }
    let mut e = String::new();
    if let Some(er) = err.as_mut() {
        let _ = er.read_to_string(&mut e);
    }
    let ok = st.map(|s| s.success()).unwrap_or(false);
    // «уже подключено» — тоже успех
    if ok || e.contains("already mounted") {
        if dev {
            // открытый LUKS смонтирован в своём разделе-потомке: первая точка из lsblk
            let o = Command::new("lsblk").args(["-nr", "-o", "MOUNTPOINT", uri]).stdin(Stdio::null()).output();
            let p = o.ok().and_then(|o| String::from_utf8_lossy(&o.stdout).lines().map(|l| l.trim().replace("\\x20", " ")).find(|l| l.starts_with('/'))).map(PathBuf::from);
            return Mounted::Ok(p);
        }
        // том (телефон) подключается под своим адресом: ищем точку по имени из списка
        let p = local_path(uri).or_else(|| gvfs_devices().into_iter().find(|d| d.uri.starts_with(uri.trim_end_matches('/')) || uri.starts_with(d.uri.trim_end_matches('/'))).and_then(|d| d.mount));
        return Mounted::Ok(p);
    }
    if e.contains("denied") && anon {
        return Mounted::NeedAuth { retry: false };
    }
    Mounted::Err(gio_err(&e))
}

/// отключить: gvfs — `gio mount -u` (или -e для телефона), сеть ядра — umount/fusermount
pub fn unmount(d: &Dev) -> Result<(), String> {
    let o = if !d.uri.is_empty() {
        gio().args(["mount", if d.kind == DevKind::Net { "-u" } else { "-e" }, "--", &d.uri]).stdin(Stdio::null()).output()
    } else {
        let Some(m) = &d.mount else { return Ok(()) };
        if d.fstype.starts_with("fuse.") {
            let fu = if Path::new("/usr/bin/fusermount3").exists() { "fusermount3" } else { "fusermount" };
            Command::new(fu).arg("-u").arg(m).stdin(Stdio::null()).output()
        } else {
            Command::new("umount").arg(m).stdin(Stdio::null()).output()
        }
    };
    match o {
        Ok(o) if o.status.success() => Ok(()),
        // телефон без «извлечь» — просто отключаем
        Ok(_) if !d.uri.is_empty() && d.kind != DevKind::Net => match gio().args(["mount", "-u", "--", &d.uri]).stdin(Stdio::null()).output() {
            Ok(o) if o.status.success() => Ok(()),
            Ok(o) => Err(gio_err(&String::from_utf8_lossy(&o.stderr))),
            Err(e) => Err(e.to_string()),
        },
        Ok(o) => Err(gio_err(&String::from_utf8_lossy(&o.stderr))),
        Err(e) => Err(e.to_string()),
    }
}

/// последняя строка ошибки gio без «gio: адрес:»
fn gio_err(e: &str) -> String {
    let l = e.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    let l = l.strip_prefix("gio: ").unwrap_or(l);
    // «smb://host/: Failed to mount…» → «Failed to mount…»
    let l = match l.find(": ") {
        Some(i) if l[..i].contains("://") => &l[i + 2..],
        _ => l,
    };
    if l.is_empty() {
        crate::i18n::tr("не подключается").to_string()
    } else {
        l.to_string()
    }
}

/// телефоны и камеры на USB, которые gvfs не видит, потому что нет бэкенда: строка-подсказка
pub fn usb_phones_without_backend() -> Vec<Dev> {
    if phone_backends() {
        return vec![];
    }
    let Ok(rd) = std::fs::read_dir("/sys/bus/usb/devices") else { return vec![] };
    let rf = |p: &Path, f: &str| std::fs::read_to_string(p.join(f)).map(|s| s.trim().to_string()).unwrap_or_default();
    let mut out: Vec<Dev> = vec![];
    for e in rd.flatten() {
        let p = e.path();
        // интерфейсы: «1-2:1.0»; класс 06 — PTP/MTP, или имя интерфейса «MTP»
        if !e.file_name().to_string_lossy().contains(':') {
            continue;
        }
        let cls = rf(&p, "bInterfaceClass");
        let name = rf(&p, "interface");
        let Some(parent) = p.parent().map(|x| x.to_path_buf()) else { continue };
        let apple = rf(&parent, "idVendor") == "05ac";
        if cls != "06" && !name.contains("MTP") && !apple {
            continue;
        }
        let label = [rf(&parent, "manufacturer"), rf(&parent, "product")].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" ");
        let dev = parent.to_string_lossy().to_string();
        if out.iter().any(|d| d.dev == dev) {
            continue;
        }
        let kind = if name.contains("MTP") || apple { DevKind::Phone } else { DevKind::Camera };
        out.push(Dev { dev, label, removable: true, kind, ..Default::default() });
    }
    out
}

/// есть ли у gvfs бэкенды для телефонов (иначе подскажем пакет): служба монитора MTP в папках
/// данных (XDG_DATA_DIRS — так и на NixOS, где gvfs в /nix/store), libexec рядом с ними, PATH,
/// потом пути дистрибутивов (#1)
fn phone_backends() -> bool {
    let svc = yami_px::paths::data_dirs().iter().any(|d| d.join("dbus-1/services/org.gtk.vfs.MTPVolumeMonitor.service").exists() || d.join("../libexec/gvfsd-mtp").exists());
    svc || yami_px::paths::find_exe("gvfsd-mtp").is_some()
        || ["/usr/lib/gvfsd-mtp", "/usr/libexec/gvfsd-mtp", "/usr/lib/gvfs/gvfsd-mtp", "/usr/lib/x86_64-linux-gnu/gvfs/gvfsd-mtp", "/usr/lib64/gvfs/gvfsd-mtp"].iter().any(|p| Path::new(p).exists())
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn uris() {
        assert_eq!(normalize("nas/media"), "smb://nas/media");
        assert_eq!(normalize("\\\\nas\\media"), "smb://nas/media");
        assert_eq!(strip_secret("sftp://me:pw@host/x"), "sftp://me@host/x");
        assert_eq!(strip_secret("smb://host/x"), "smb://host/x");
        assert_eq!(split_secret("sftp://me:pw@host/x"), ("sftp://me@host/x".to_string(), Some("pw".to_string())));
    }
}
