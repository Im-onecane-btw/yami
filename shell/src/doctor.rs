// `yami doctor` и `yami status`: что есть в системе и что работает. Шелл без внешних программ не
// падает — просто гаснет то, чему они нужны; doctor говорит, что именно и почему.
// Службы D-Bus — кто держит имя (процесс по pid), а не «есть ли пакет».
// i18n: inline pairs

use crate::cmd::Sty;
use std::path::Path;

/// программа → для чего (RU, EN); true — без неё не работает основное
const DEPS: &[(&str, &str, &str, bool)] = &[
    ("niri", "композитор", "the compositor", true),
    ("setsid", "запуск программ", "launching programs", true),
    ("grim", "скриншоты, шторка", "screenshots, the curtain", false),
    ("wl-copy", "буфер обмена", "clipboard", false),
    ("wl-paste", "буфер обмена", "clipboard", false),
    ("cliphist", "история буфера", "clipboard history", false),
    ("pactl", "звук", "sound", false),
    ("parec", "визуализатор музыки", "music visualizer", false),
    ("playerctl", "музыка в баре", "music in the bar", false),
    ("nmcli", "Wi-Fi", "Wi-Fi", false),
    ("brightnessctl", "яркость с клавиш", "brightness keys", false),
    ("python3", "перекраска программ, обои (theme.py)", "recolouring apps, wallpaper (theme.py)", false),
    ("fc-match", "шрифты", "fonts", false),
    ("curl", "обложки музыки из сети", "album covers from the web", false),
    ("xdg-open", "открыть файл настроек", "opening the settings file", false),
    ("ffmpegthumbnailer", "миниатюры видео в файлах", "video thumbnails in files", false),
    ("bsdtar", "архивы в файлах", "archives in files", false),
    ("udisksctl", "флешки и диски", "USB drives", false),
    ("git", "обновления", "updates", false),
    ("cargo", "сборка обновлений", "building updates", false),
    ("ssh-keygen", "проверка подписи выпусков", "release signature check", false),
];

pub fn which(c: &str) -> bool {
    std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|d| is_exec(&d.join(c)))).unwrap_or(false)
}
fn is_exec(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

/// процессы, чья командная строка начинается с exe (по имени) и содержит arg
fn pids(exe: &str, arg: &str) -> Vec<u32> {
    let Ok(rd) = std::fs::read_dir("/proc") else { return vec![] };
    rd.flatten()
        .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
        .filter(|pid| {
            let c = std::fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
            let mut parts = c.split(|b| *b == 0).map(|s| String::from_utf8_lossy(s).to_string());
            let first = parts.next().unwrap_or_default();
            first.rsplit('/').next() == Some(exe) && (arg.is_empty() || parts.any(|p| p == arg))
        })
        .collect()
}

fn pss_mb(pid: u32) -> f32 {
    std::fs::read_to_string(format!("/proc/{pid}/smaps_rollup")).ok().and_then(|t| t.lines().find(|l| l.starts_with("Pss:"))?.split_whitespace().nth(1)?.parse::<f32>().ok()).map(|k| k / 1024.0).unwrap_or(0.0)
}

/// кто держит имя на шине: имя процесса или None
fn owner(c: &zbus::blocking::Connection, name: &str) -> Option<String> {
    let p = zbus::blocking::fdo::DBusProxy::new(c).ok()?;
    let n = zbus::names::BusName::try_from(name).ok()?;
    let o = p.get_name_owner(n.clone()).ok()?;
    let pid = p.get_connection_unix_process_id(zbus::names::BusName::from(o)).ok();
    Some(pid.and_then(|pid| std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()).map(|s| s.trim().to_string()).unwrap_or_else(|| "?".into()))
}
fn activatable(c: &zbus::blocking::Connection, name: &str) -> bool {
    zbus::blocking::fdo::DBusProxy::new(c).ok().and_then(|p| p.list_activatable_names().ok()).map(|l| l.iter().any(|n| n.as_str() == name)).unwrap_or(false)
}

fn line(y: &Sty, mark: &str, what: &str, note: &str) {
    let pad = 22usize.saturating_sub(what.chars().count());
    println!("   {mark} {}{}{}", what, " ".repeat(pad), y.dim(note));
}

/// службы D-Bus: (имя, для чего RU, EN); ours — должен держать шелл (если служба включена)
fn services(y: &Sty, problems: &mut u32) {
    let set = crate::config::Settings::load();
    y.section(&crate::tf!("службы", "services"));
    if let Ok(c) = zbus::blocking::Connection::session() {
        for (name, ru, en, want) in [
            ("org.freedesktop.Notifications", "уведомления", "notifications", set.agent_notif.then_some("yami-shell")),
            ("org.kde.StatusNotifierWatcher", "трей", "tray", set.agent_tray.then_some("yami-shell")),
            ("org.freedesktop.portal.Desktop", "порталы", "portals", None),
        ] {
            let label = if crate::i18n::en() { en } else { ru };
            match owner(&c, name) {
                Some(who) if want.map(|w| w == who).unwrap_or(true) => line(y, &y.ok("✓"), label, &who),
                Some(who) => line(y, &y.warn("!"), label, &crate::tf!("держит {}, а не шелл", "owned by {}, not the shell", who)),
                None if want.is_none() && activatable(&c, name) => line(y, &y.ok("✓"), label, &crate::tf!("запустится по запросу", "starts on demand")),
                None => {
                    *problems += 1;
                    line(y, &y.bad("✕"), label, &crate::tf!("никто не держит {}", "nobody owns {}", name));
                }
            }
        }
        let yp = "org.freedesktop.impl.portal.desktop.yami";
        let run = !pids("yami-files", "--portal").is_empty();
        if run || activatable(&c, yp) {
            line(y, &y.ok("✓"), &crate::tf!("портал yami-files", "yami-files portal"), &if run { crate::tf!("работает", "running") } else { crate::tf!("запустится по запросу", "starts on demand") });
        } else {
            *problems += 1;
            line(y, &y.bad("✕"), &crate::tf!("портал yami-files", "yami-files portal"), &crate::tf!("не установлен: install.sh (шаг «файлы»)", "not installed: install.sh (the «files» step)"));
        }
    } else {
        *problems += 1;
        line(y, &y.bad("✕"), &crate::tf!("сессионная шина", "session bus"), &crate::tf!("нет", "missing"));
    }
    if let Some(c) = crate::net::system() {
        for (name, ru, en) in [
            ("org.freedesktop.PolicyKit1", "polkit", "polkit"),
            ("org.freedesktop.NetworkManager", "сеть", "network"),
            ("net.hadess.PowerProfiles", "профили питания", "power profiles"),
            ("org.bluez", "bluetooth", "bluetooth"),
        ] {
            let label = if crate::i18n::en() { en } else { ru };
            match owner(&c, name) {
                Some(who) => line(y, &y.ok("✓"), label, &who),
                None if activatable(&c, name) => line(y, &y.ok("✓"), label, &crate::tf!("запустится по запросу", "starts on demand")),
                None => line(y, &y.dim("·"), label, &crate::tf!("нет (то, что от неё, в баре скрыто)", "absent (what needs it is hidden)")),
            }
        }
    }
    let helper = Path::new(crate::auth::SOCKET).exists() || crate::auth::HELPERS.iter().any(|h| Path::new(h).exists());
    if set.agent_polkit && !helper {
        *problems += 1;
        line(y, &y.bad("✕"), &crate::tf!("помощник polkit", "polkit helper"), &crate::tf!("нет polkit-agent-helper-1: окна пароля не будет", "no polkit-agent-helper-1: no password window"));
    }
}

pub fn run(y: &Sty) -> i32 {
    let mut problems = 0u32;
    y.section(&crate::tf!("программы", "programs"));
    for (c, ru, en, req) in DEPS {
        let what = if crate::i18n::en() { en } else { ru };
        if which(c) {
            line(y, &y.ok("✓"), c, what);
        } else if *req {
            problems += 1;
            line(y, &y.bad("✕"), c, &crate::tf!("нужен: {}", "required: {}", what));
        } else {
            line(y, &y.warn("·"), c, &crate::tf!("нет — не будет: {}", "missing, no {}", what));
        }
    }
    if which("python3") {
        let pil = std::process::Command::new("python3").args(["-I", "-c", "import PIL"]).stderr(std::process::Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
        if !pil {
            line(y, &y.warn("·"), "python3-pillow", &crate::tf!("нет — обои из своей картинки не перекрасятся", "missing, own wallpapers are not recoloured"));
        }
    }
    services(y, &mut problems);
    y.section(&crate::tf!("настройки", "config"));
    let (bad, unknown) = crate::config::Settings::problems(&crate::cli::file_json());
    if bad.is_empty() && unknown.is_empty() {
        line(y, &y.ok("✓"), "yami-shell.json", &crate::config::settings_path().display().to_string());
    }
    for k in &bad {
        problems += 1;
        line(y, &y.bad("✕"), k, &crate::tf!("негодное значение, взято по умолчанию", "bad value, default used"));
    }
    for k in &unknown {
        line(y, &y.warn("·"), k, &crate::tf!("неизвестный ключ, пропущен", "unknown key, ignored"));
    }
    if which("niri") {
        let cfg = crate::config::config_dir().join("niri/config.kdl");
        let ok = std::process::Command::new("niri").arg("validate").arg("-c").arg(&cfg).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
        if ok {
            line(y, &y.ok("✓"), "niri/config.kdl", &crate::tf!("проходит проверку", "validates"));
        } else {
            problems += 1;
            line(y, &y.bad("✕"), "niri/config.kdl", &crate::tf!("не проходит: niri validate", "fails: niri validate"));
        }
    }
    println!();
    if problems == 0 {
        println!(" {} {}\n", y.ok("✓"), crate::tf!("всё в порядке", "all good"));
        0
    } else {
        println!(" {} {}\n", y.warn("!"), crate::tf!("проблем: {}", "problems: {}", problems));
        1
    }
}

pub fn status(y: &Sty) {
    y.section(&crate::tf!("работает", "running"));
    match pids("yami-shell", "").into_iter().find(|p| *p != std::process::id()) {
        Some(pid) => line(y, &y.ok("✓"), "yami-shell", &crate::tf!("pid {}, {:.1} МБ", "pid {}, {:.1} MB", pid, pss_mb(pid))),
        None => line(y, &y.bad("✕"), "yami-shell", &crate::tf!("не запущен", "not running")),
    }
    match pids("yami-files", "--portal").first() {
        Some(pid) => line(y, &y.ok("✓"), &crate::tf!("портал файлов", "file portal"), &crate::tf!("pid {}, {:.1} МБ", "pid {}, {:.1} MB", pid, pss_mb(*pid))),
        None => line(y, &y.dim("·"), &crate::tf!("портал файлов", "file portal"), &crate::tf!("запустится по запросу", "starts on demand")),
    }
    let lvl = crate::niri::fork_level();
    let nv = std::process::Command::new("niri").arg("--version").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    if std::env::var_os("NIRI_SOCKET").is_some() {
        line(y, &y.ok("✓"), "niri", &if lvl > 0 { crate::tf!("{} (yami-niri, уровень {})", "{} (yami-niri, level {})", nv, lvl) } else { nv });
    } else {
        line(y, &y.dim("·"), "niri", &crate::tf!("не в сеансе niri", "not in a niri session"));
    }
    if crate::niri::game_focused() {
        line(y, &y.acc("▪"), &crate::tf!("игра", "game"), &crate::tf!("в фокусе — шелл не перезапускается", "has focus: the shell will not restart"));
    }
    let u = crate::update::installed();
    let last = std::fs::read_to_string(crate::config::state_dir().join("update.json")).ok().map(|t| yami_px::paths::lenient_json::<crate::update::Info>(&t));
    let upd = match last {
        Some(i) if !i.err.is_empty() => y.warn(&i.err),
        Some(i) if i.available => y.acc(&crate::tf!("доступно {} (yami update)", "{} available (yami update)", i.latest)),
        Some(_) => crate::tf!("свежее", "up to date"),
        None => crate::tf!("не проверялось", "never checked"),
    };
    line(y, &y.ok("✓"), &crate::tf!("версия", "version"), &format!("{}  ·  {upd}", if u.version.is_empty() { env!("CARGO_PKG_VERSION") } else { &u.version }));
}
