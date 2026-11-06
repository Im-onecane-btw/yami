// Внешние программы: запуск отвязанными (setsid), долгие дети нитей (PDEATHSIG), curl к чужим
// сайтам, окно выбора yami-files, tools/theme.py с пониженным приоритетом, подбор детей,
// доставшихся от образа до перезапуска.

use crate::app::App;
use crate::config;

/// curl к чужому сайту (погода, тексты песен, тема по ссылке): только https, и после
/// переадресаций; потолок времени и размера; имя yami в User-Agent. Флаги -f, -G, -L, -o — у вызова
pub fn curl(secs: u32, max: u64) -> std::process::Command {
    let mut c = std::process::Command::new("curl");
    let ua = concat!("yami/", env!("CARGO_PKG_VERSION"), " (https://github.com/Trendorin/yami)");
    c.args(["-q", "-s", "--proto", "=https", "--proto-redir", "=https", "-A", ua, "--max-time", &secs.to_string(), "--max-filesize", &max.to_string()]);
    c
}

/// GET url?k=v&… тем же curl: значения — через --data-urlencode (пробелы, кириллица, «&»)
pub fn curl_get(secs: u32, max: u64, url: &str, q: &[(&str, String)]) -> std::process::Command {
    let mut c = curl(secs, max);
    c.args(["-G", url]);
    for (k, v) in q {
        c.arg("--data-urlencode").arg(format!("{k}={v}"));
    }
    c
}

/// долгий процесс нити шелла (playerctl -F, pactl subscribe, parec) уходит вместе с ней: при
/// падении шелла и при перезапуске (exec в том же pid уничтожает нити, их дети получают сигнал).
/// Без этого каждый `yami restart` и каждое обновление оставляли по живой копии каждого из них
pub fn owned(cmd: &mut std::process::Command) -> &mut std::process::Command {
    use std::os::unix::process::CommandExt;
    // SAFETY: между fork и exec только prctl
    unsafe {
        cmd.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
            Ok(())
        })
    }
}

/// окно выбора yami-files (`--pick MODE`, папка, свои ключи): первая строка ответа или None
/// (отмена). Ждёт, пока окно открыто, — только из нити; окно закрывается вместе с нитью (owned)
pub fn files_pick(mode: &str, folder: &std::path::Path, args: &[&str]) -> Option<String> {
    let mut c = std::process::Command::new("yami-files");
    c.args(["--pick", mode]).args(args).arg("--folder").arg(folder);
    owned(&mut c).output().ok().filter(|o| o.status.success()).and_then(|o| String::from_utf8_lossy(&o.stdout).lines().next().map(str::to_string))
}

/// дети, доставшиеся от образа до перезапуска: их Child остались в старых нитях, ждать их некому.
/// Список берётся до первого запуска чего-либо (все дети на этот момент — чужие) и
/// подбирается в tick, иначе каждый перезапуск оставлял зомби
static INHERITED: std::sync::Mutex<Vec<i32>> = std::sync::Mutex::new(Vec::new());

pub(crate) fn note_inherited() {
    let me = std::process::id().to_string();
    let Ok(rd) = std::fs::read_dir("/proc") else { return };
    let kids: Vec<i32> = rd
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.parse::<i32>().ok())
        .filter(|p| {
            // «pid (comm) S ppid …»: comm может содержать пробелы и скобки — режем по последней «)»
            let Ok(s) = std::fs::read_to_string(format!("/proc/{p}/stat")) else { return false };
            s.rsplit_once(')').and_then(|(_, r)| r.split_whitespace().nth(1)) == Some(me.as_str())
        })
        .collect();
    if let Ok(mut v) = INHERITED.lock() {
        *v = kids;
    }
}

pub(crate) fn reap_inherited() {
    let Ok(mut v) = INHERITED.lock() else { return };
    // 0 — ещё жив; pid — подобран; -1 — уже не наш
    v.retain(|&p| unsafe { libc::waitpid(p, std::ptr::null_mut(), libc::WNOHANG) } == 0);
}

/// tools/theme.py, тихо и с пониженным приоритетом: на слабой машине перекраска (обои, курсор,
/// темы программ) идёт секунды и не должна отнимать процессор у стола
pub fn theme_py() -> std::process::Command {
    let mut c = std::process::Command::new("python3");
    c.arg("-I").arg(theme_script()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    gentle(&mut c);
    c
}

/// фоновая работа (theme.py, распознавание текста): nice 10 у процесса, шелл свой не меняет
pub fn gentle(cmd: &mut std::process::Command) -> &mut std::process::Command {
    use std::os::unix::process::CommandExt;
    // SAFETY: между fork и exec только setpriority
    unsafe {
        cmd.pre_exec(|| {
            libc::setpriority(libc::PRIO_PROCESS, 0, 10);
            Ok(())
        })
    }
}

pub fn theme_script() -> std::path::PathBuf {
    // tools/theme.py вшит в шелл и пишется рядом с состоянием
    let p = config::state_dir().join("theme.py");
    let src = include_str!("../../tools/theme.py");
    if std::fs::read_to_string(&p).map(|t| t != src).unwrap_or(true) {
        let _ = std::fs::write(&p, src);
    }
    p
}

impl App {
    // ── запуск программ ─────────────────────────────────────────────────────
    pub fn spawn(argv: &[String]) {
        if argv.is_empty() {
            return;
        }
        // setsid -f: процесс отвязывается от шелла и не становится зомби
        let _ = std::process::Command::new("setsid")
            .arg("-f")
            .args(argv)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    pub fn sh(cmd: &str) {
        Self::spawn(&["sh".to_string(), "-c".to_string(), cmd.to_string()]);
    }
}
