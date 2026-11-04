// Игровые ускорители, которые уже есть в системе: находятся и включаются сами, на любом дистрибутиве.
// - ntsync (/dev/ntsync, ядро 6.14+): PROTON_USE_NTSYNC=1 для Proton через yami-fx.kdl;
// - GameMode (gamemoded на сессионной шине): pid игры в фокусе регистрируется, пока она в
//   фокусе, — без `gamemoderun %command%` в каждой игре.
// Системное (планировщик, лимиты кадров) не трогаем: это дело игры, MangoHud и самой системы.
// `yami doctor games` показывает, что есть, и одну команду, которая добавит недостающее.
// i18n: inline pairs

use crate::cmd::Sty;
use std::path::Path;

/// что есть в системе
#[derive(Clone, Debug, Default)]
pub struct Probe {
    pub ntsync: bool,
    /// модуль есть, но не загружен
    pub ntsync_mod: bool,
    pub gamemode: bool,
    pub ppd: bool,
    pub mangohud: bool,
}

pub fn probe() -> Probe {
    let which = crate::doctor::which;
    let rel = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
    let kmod = |m: &str| {
        let dir = Path::new("/lib/modules").join(rel.trim());
        // в modules.dep — все модули, которые ядро может загрузить
        std::fs::read_to_string(dir.join("modules.dep")).map(|t| t.lines().any(|l| l.split(':').next().unwrap_or("").contains(&format!("/{m}.ko")))).unwrap_or(false)
    };
    Probe {
        ntsync: Path::new("/dev/ntsync").exists(),
        ntsync_mod: !Path::new("/dev/ntsync").exists() && kmod("ntsync"),
        gamemode: which("gamemoded"),
        ppd: which("powerprofilesctl") || which("tuned-adm"),
        mangohud: which("mangohud"),
    }
}

/// строки для yami-fx.kdl (внутри `environment { }`)
pub fn env_kdl(set: &crate::config::Settings, l: &mut Vec<String>) {
    if set.game_ntsync && Path::new("/dev/ntsync").exists() {
        l.push("    PROTON_USE_NTSYNC \"1\"".into());
    }
}

/// игра в фокусе, зарегистрированная в GameMode
#[derive(Default)]
pub struct Boost {
    pub pid: Option<u32>,
}

/// сменился фокус игры: зарегистрировать / снять в GameMode. Вызовы D-Bus — в нити: поток
/// Wayland никогда не ждёт демонов
pub fn focus(set: &crate::config::Settings, b: &mut Boost, pid: Option<u32>) {
    let old = std::mem::replace(&mut b.pid, pid);
    if old == pid || !set.game_gamemode {
        return;
    }
    std::thread::spawn(move || {
        let Ok(c) = zbus::blocking::Connection::session() else { return };
        let call = |m: &str, p: u32| {
            let _ = c.call_method(Some("com.feralinteractive.GameMode"), "/com/feralinteractive/GameMode", Some("com.feralinteractive.GameMode"), m, &(p as i32));
        };
        if let Some(p) = old {
            call("UnregisterGame", p);
        }
        if let Some(p) = pid {
            call("RegisterGame", p);
        }
    });
}

/// пакетный менеджер дистрибутива — для подсказки одной строкой
fn pm() -> Option<(&'static str, &'static str)> {
    let w = crate::doctor::which;
    [("pacman", "sudo pacman -S --needed"), ("dnf", "sudo dnf install"), ("apt", "sudo apt install"), ("zypper", "sudo zypper install"), ("xbps-install", "sudo xbps-install"), ("apk", "sudo apk add"), ("emerge", "sudo emerge")]
        .into_iter()
        .find(|(b, _)| w(b))
}

/// имена пакетов: одинаковые во всех семействах, куда ставится yami
fn pkg(what: &str) -> &'static str {
    match what {
        "gamemode" => "gamemode",
        "mangohud" => "mangohud",
        _ => "",
    }
}

/// `yami doctor games`
pub fn doctor(y: &Sty) -> i32 {
    let p = probe();
    let set = crate::config::Settings::load();
    y.section(&crate::tf!("игры: что есть в системе", "games: what this system has"));
    let mark = |ok: bool| if ok { y.ok("✓") } else { y.warn("·") };
    let mut missing: Vec<&str> = vec![];
    let row = |ok: bool, name: &str, note: String| println!("   {} {:<24}{}", mark(ok), name, y.dim(&note));
    row(p.ntsync, "ntsync", if p.ntsync {
        if set.game_ntsync { crate::tf!("Proton получает PROTON_USE_NTSYNC=1", "Proton gets PROTON_USE_NTSYNC=1") } else { crate::tf!("выключено в настройках → игры", "off in settings → games") }
    } else if p.ntsync_mod {
        crate::tf!("модуль есть, не загружен: sudo modprobe ntsync; навсегда: echo ntsync | sudo tee /etc/modules-load.d/ntsync.conf", "the module is there but not loaded: sudo modprobe ntsync; for good: echo ntsync | sudo tee /etc/modules-load.d/ntsync.conf")
    } else {
        crate::tf!("нужно ядро 6.14+ с ntsync", "needs a 6.14+ kernel with ntsync")
    });
    row(p.gamemode, "gamemode", if p.gamemode { crate::tf!("игра в фокусе регистрируется сама", "the focused game is registered by itself") } else { missing.push("gamemode"); crate::tf!("не установлен", "not installed") });
    row(p.ppd, "power profiles", if p.ppd { crate::tf!("«производительность» на время игры", "«performance» while you play") } else { crate::tf!("нет power-profiles-daemon или tuned-ppd", "no power-profiles-daemon or tuned-ppd") });
    row(p.mangohud, "MangoHud", if p.mangohud { crate::tf!("конфиг в цветах палитры (если он не твой)", "a config in the palette colours (unless it is your own)") } else { missing.push("mangohud"); crate::tf!("не установлен", "not installed") });
    if !missing.is_empty() {
        if let Some((_, cmd)) = pm() {
            let pk: Vec<&str> = missing.iter().map(|m| pkg(m)).filter(|p| !p.is_empty()).collect();
            println!("\n   {} {}", y.dim(&crate::tf!("добавить:", "add:")), y.bold(&format!("{cmd} {}", pk.join(" "))));
        }
    }
    println!();
    0
}
