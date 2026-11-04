// Обновления: проверка, установка и откат — из настроек (вкладка «обновления») и из терминала
// (`yami update`). Источник — свой клон репозитория в ~/.local/share/yami/src (не рабочая
// копия разработчика): git fetch, выпуск = самый новый тег vX.Y.Z (канал «main» — вершина
// ветки). Подпись выпуска проверяется ключами keys/allowed_signers из УЖЕ установленной версии
// (доверие с первой установки): подменённый на сервере тег без ключа автора не встанет.
// Ставит install.sh --update (сборка из исходников, без sudo, настройки не трогает), прежние
// бинари — в ~/.local/lib/yami/prev для отката. Что установлено — ~/.local/state/yami/install.json.

use crate::app::{App, Msg, SKind, State};
use crate::config::Settings;
use crate::gfx::{BoxStyle, R};
use crate::settings::{opt_btn, title};
use crate::ui::{Act, Tone, Ui};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// ключи подписи выпусков (формат ssh allowed_signers) в дереве репозитория
pub const KEYS: &str = "keys/allowed_signers";
/// откуда ставит установщик по умолчанию; свой адрес — updateRepo
pub const REPO: &str = "https://github.com/Trendorin/yami.git";
const NOTE_ID: u32 = 9_400_001;

/// что установлено: пишет `yami-shell update stamp` (установщик, tools/deploy.sh)
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Stamp {
    pub version: String,
    pub commit: String,
    /// собрано из дерева с несохранёнными правками
    pub dirty: bool,
    pub repo: String,
    pub components: String,
    pub date: u64,
}

/// результат проверки; последний хранится в ~/.local/state/yami/update.json
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Info {
    /// версия выпуска (2.1.0) или короткий коммит (канал main)
    pub latest: String,
    /// тег или origin/main
    pub reference: String,
    pub target: String,
    pub available: bool,
    pub notes: Vec<String>,
    /// good:<кто> | unsigned | bad | nokeys | off
    pub sig: String,
    pub err: String,
    pub at: u64,
    /// последние выпуски (новые первыми) — на что можно перейти из вкладки и `yami update to`
    pub releases: Vec<Rel>,
}

/// выпуск: тег, версия, подпись (как Info::sig), дата тега
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Rel {
    pub tag: String,
    pub version: String,
    pub sig: String,
    pub date: u64,
}

impl Info {
    /// можно ставить: подпись по правилам updateVerify
    pub fn sig_ok(&self, policy: &str) -> bool {
        match policy {
            "off" => true,
            "require" => self.sig.starts_with("good:"),
            // auto: ключей ещё нет — как при первой установке; ключи есть — только подписанное
            _ => self.sig.starts_with("good:") || self.sig == "nokeys" || self.sig == "off",
        }
    }
}

/// состояние для вкладки
#[derive(Default)]
pub struct Upd {
    pub stamp: Stamp,
    pub info: Option<Info>,
    pub checking: bool,
    /// идёт установка: последний шаг
    pub busy: Option<String>,
    /// чем кончилась установка: (успех, текст)
    pub done: Option<(bool, String)>,
    pub rollback: bool,
    /// хвост update.log для консоли вкладки и размер файла, по которому он прочитан
    pub log: Vec<String>,
    pub log_len: u64,
    /// открыт выбор версии для отката и что выбрано («prev» или X.Y.Z); ставится только кнопкой
    /// «поставить», так что случайный щелчок по «откатить» ничего не меняет
    pub pick: bool,
    pub sel: String,
}

impl Upd {
    pub fn load() -> Upd {
        let (log, log_len) = log_tail();
        Upd { stamp: installed(), info: last(), rollback: prev_dir().join("yami-shell").exists(), log, log_len, ..Default::default() }
    }
    /// для `yami-shell shot settings:updates[-new|-busy]`
    pub fn mock(what: &str) -> Upd {
        let stamp = Stamp { version: "2.0.0".into(), commit: "24da2407c0ffee00".into(), repo: REPO.into(), ..Default::default() };
        let mut u = Upd { stamp, rollback: true, ..Default::default() };
        let mut i = Info { latest: "2.0.0".into(), reference: "v2.0.0".into(), sig: "good:trendorin".into(), at: now() - 600, ..Default::default() };
        if what != "updates" {
            i.latest = "2.1.0".into();
            i.reference = "v2.1.0".into();
            i.available = true;
            i.notes = vec!["yami: one command for the whole desktop".into(), "settings: updates and services tabs".into(), "shell: notification images are size-checked".into()];
        }
        if what == "updates-busy" {
            u.busy = Some("[3/8] shell: build from source".into());
            u.log = ["▪ checking what is new", "▪ switching to v2.1.0", "▪ build and install (a few minutes)", "▪ [1/8] update from ~/.local/share/yami/src", "✓ update from ~/.local/share/yami/src 0s", "▪ [2/8] packages", "✓ packages 3s", "▪ [3/8] shell: build from source", "Compiling yami-px v2.1.0", "Compiling yami-files v2.1.0", "Compiling yami-shell v2.1.0"]
                .iter()
                .map(|s| s.to_string())
                .collect();
        }
        if what == "updates-pick" {
            // выбор версии: прошлой сборки в образце нет, три выпуска, один не подписан
            u.pick = true;
            u.sel = "2.1.0".into();
            i.available = false;
            i.releases = vec![
                Rel { tag: "v2.1.0".into(), version: "2.1.0".into(), sig: "good:trendorin".into(), date: 1_793_721_454 },
                Rel { tag: "v2.0.0".into(), version: "2.0.0".into(), sig: "good:trendorin".into(), date: 1_793_721_346 },
                Rel { tag: "v1.9.0".into(), version: "1.9.0".into(), sig: "unsigned".into(), date: 1_793_600_000 },
            ];
        }
        u.info = Some(i);
        u
    }
}

// ── файлы ───────────────────────────────────────────────────────────────────
fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
pub fn stamp_path() -> PathBuf {
    crate::config::state_dir().join("install.json")
}
fn info_path() -> PathBuf {
    crate::config::state_dir().join("update.json")
}
pub fn clone_dir() -> PathBuf {
    crate::config::data_dir().join("yami/src")
}
fn bin_dir() -> PathBuf {
    crate::config::home().join(".local/bin")
}
fn prev_dir() -> PathBuf {
    crate::config::home().join(".local/lib/yami/prev")
}
pub fn log_path() -> PathBuf {
    yami_px::paths::cache_dir().join("yami/update.log")
}
pub fn installed() -> Stamp {
    std::fs::read_to_string(stamp_path()).map(|t| yami_px::paths::lenient_json(&t)).unwrap_or_default()
}
/// последняя проверка; то, что она нашла и что уже установлено, — не «доступно»
pub fn last() -> Option<Info> {
    let mut i: Info = std::fs::read_to_string(info_path()).ok().map(|t| yami_px::paths::lenient_json(&t))?;
    if i.target == installed().commit {
        i.available = false;
    }
    Some(i)
}
fn save_json<T: Serialize>(p: &Path, v: &T) {
    if let Ok(j) = serde_json::to_string_pretty(v) {
        let _ = crate::config::write_atomic(p, &(j + "\n"));
    }
}

// ── git ─────────────────────────────────────────────────────────────────────
/// git без вопросов: приватный репозиторий без ключей — ошибка сразу, а не ожидание пароля;
/// медленная сеть обрывается (20 с меньше 1 КБ/с)
fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let o = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "http.lowSpeedLimit=1000", "-c", "http.lowSpeedTime=20", "-c", "core.hooksPath=/dev/null"])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| crate::tf!("нет git: {e}", "git not found: {e}"))?;
    if o.status.success() {
        Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&o.stderr).lines().filter(|l| !l.trim().is_empty()).last().unwrap_or("git failed").trim().to_string())
    }
}

/// понятная причина вместо последней строки git
fn explain(e: String) -> String {
    let l = e.to_lowercase();
    if l.contains("could not read username") || l.contains("authentication") || l.contains("repository not found") || l.contains("403") {
        crate::tf!("нет доступа к репозиторию (приватный? gh auth login)", "no access to the repository (private? gh auth login)")
    } else if l.contains("could not resolve host") || l.contains("timed out") || l.contains("unable to access") || l.contains("operation too slow") {
        crate::tf!("нет сети или сервер недоступен", "no network or the server is unreachable")
    } else {
        e
    }
}

/// свой клон: нет — клонируем (история без файлов, они придут при переходе на версию)
fn ensure_clone(set: &Settings, st: &Stamp) -> Result<PathBuf, String> {
    let dir = clone_dir();
    let url = [set.update_repo.as_str(), st.repo.as_str(), REPO].into_iter().find(|u| !u.is_empty()).unwrap_or(REPO).to_string();
    if !dir.join(".git").exists() {
        if let Some(p) = dir.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        let d = dir.to_string_lossy().to_string();
        git(Path::new("/"), &["clone", "-q", "--filter=blob:none", "--", &url, &d]).map_err(explain)?;
    } else {
        if git(&dir, &["remote", "get-url", "origin"]).map(|u| u != url).unwrap_or(true) {
            git(&dir, &["remote", "set-url", "origin", &url])?;
        }
        // установщик клонировал с --depth 1: для сравнения версий нужна история
        if git(&dir, &["rev-parse", "--is-shallow-repository"]).map(|s| s == "true").unwrap_or(false) {
            git(&dir, &["fetch", "-q", "--unshallow", "--filter=blob:none", "origin"]).map_err(explain)?;
        }
    }
    Ok(dir)
}

fn has_commit(dir: &Path, c: &str) -> bool {
    !c.is_empty() && git(dir, &["cat-file", "-e", &format!("{c}^{{commit}}")]).is_ok()
}

/// «2.10.0» > «2.9.1»: по числам
fn ver(s: &str) -> Vec<u64> {
    s.trim_start_matches('v').split(|c: char| !c.is_ascii_digit()).filter(|x| !x.is_empty()).map(|x| x.parse().unwrap_or(0)).collect()
}

/// проверить, что нового (сеть, git): из нити или из терминала
pub fn check(set: &Settings) -> Info {
    let mut i = Info { at: now(), ..Default::default() };
    if let Err(e) = check_into(set, &mut i) {
        i.err = e;
    }
    save_json(&info_path(), &i);
    i
}

fn check_into(set: &Settings, i: &mut Info) -> Result<(), String> {
    let st = installed();
    let dir = ensure_clone(set, &st)?;
    // теги без --force: тег, переписанный на сервере, не заменит уже виденный
    git(&dir, &["fetch", "-q", "--tags", "--prune", "origin", "+refs/heads/main:refs/remotes/origin/main"]).map_err(explain)?;
    let main = set.update_channel == "main";
    let reference = if main {
        "origin/main".to_string()
    } else {
        // выпусков ещё нет — не ошибка: ставить нечего (reference пустой)
        match git(&dir, &["tag", "-l", "v[0-9]*", "--sort=-v:refname"])?.lines().next() {
            Some(t) => t.to_string(),
            None => return Ok(()),
        }
    };
    i.target = git(&dir, &["rev-parse", &format!("{reference}^{{commit}}")])?;
    i.latest = if main { i.target.chars().take(8).collect() } else { reference.trim_start_matches('v').to_string() };
    i.reference = reference;
    let known = has_commit(&dir, &st.commit);
    i.available = if st.commit == i.target {
        false
    } else if known && git(&dir, &["merge-base", "--is-ancestor", &i.target, &st.commit]).is_ok() {
        // установлено то же или новее (сборка разработчика впереди выпуска)
        false
    } else if main {
        true
    } else {
        st.version.is_empty() || ver(&i.latest) > ver(&st.version)
    };
    i.notes = notes(&dir, &st, i, known, main);
    i.sig = signature(&dir, &st, i, known, main, &set.update_verify);
    i.releases = releases(&dir, &st, known, &set.update_verify);
    Ok(())
}

/// последние восемь выпусков с подписями (их проверяют ключи установленной версии)
fn releases(dir: &Path, st: &Stamp, known: bool, policy: &str) -> Vec<Rel> {
    let Ok(tags) = git(dir, &["tag", "-l", "v[0-9]*", "--sort=-v:refname"]) else { return vec![] };
    tags.lines()
        .take(8)
        .map(|t| Rel {
            tag: t.to_string(),
            version: t.trim_start_matches('v').to_string(),
            sig: verify(dir, st, known, policy, "verify-tag", t),
            date: git(dir, &["log", "-1", "--format=%ct", &format!("{t}^{{commit}}")]).ok().and_then(|d| d.parse().ok()).unwrap_or(0),
        })
        .collect()
}

/// что нового: раздел CHANGELOG.md выпуска, иначе заголовки коммитов
fn notes(dir: &Path, st: &Stamp, i: &Info, known: bool, main: bool) -> Vec<String> {
    if !main {
        if let Ok(log) = git(dir, &["show", &format!("{}:CHANGELOG.md", i.target)]) {
            let head = format!("## {}", i.latest);
            let sec: Vec<String> = log.lines().skip_while(|l| !l.starts_with(&head)).skip(1).take_while(|l| !l.starts_with("## ")).map(|l| l.trim().trim_start_matches(['-', '*']).trim().to_string()).filter(|l| !l.is_empty()).take(12).collect();
            if !sec.is_empty() {
                return sec;
            }
        }
    }
    let range = if known { format!("{}..{}", st.commit, i.target) } else { i.target.clone() };
    git(dir, &["log", "--no-merges", "--format=%s", "-n", "12", &range]).map(|t| t.lines().map(String::from).collect()).unwrap_or_default()
}

/// подпись выпуска ключами из установленной версии (а если её коммита нет в клоне — из того,
/// что клон ставил последним)
fn signature(dir: &Path, st: &Stamp, i: &Info, known: bool, main: bool, policy: &str) -> String {
    let (what, obj) = if main { ("verify-commit", i.target.as_str()) } else { ("verify-tag", i.reference.as_str()) };
    verify(dir, st, known, policy, what, obj)
}

/// git verify-tag/verify-commit ключами установленной версии
fn verify(dir: &Path, st: &Stamp, known: bool, policy: &str, what: &str, obj: &str) -> String {
    if policy == "off" {
        return "off".into();
    }
    let base = if known { st.commit.clone() } else { "HEAD".into() };
    let keys = git(dir, &["show", &format!("{base}:{KEYS}")]).unwrap_or_default();
    if keys.lines().all(|l| l.trim().is_empty() || l.trim_start().starts_with('#')) {
        return "nokeys".into();
    }
    let kf = crate::config::state_dir().join("update-keys");
    if crate::config::write_atomic(&kf, &(keys + "\n")).is_err() {
        return "bad".into();
    }
    let kfs = kf.to_string_lossy().to_string();
    let o = Command::new("git")
        .arg("-C")
        .arg(dir)
        // minTrustLevel=fully: подпись ключом не из allowed_signers («No principal matched») — не «good»
        .args(["-c", "gpg.format=ssh", "-c", &format!("gpg.ssh.allowedSignersFile={kfs}"), "-c", "gpg.minTrustLevel=fully", what, "--", obj])
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output();
    let Ok(o) = o else { return "bad".into() };
    let err = String::from_utf8_lossy(&o.stderr).to_string();
    // Good "git" signature for NAME with ED25519 key SHA256:… — без «for NAME» ключ не из списка
    let who = err.split("Good \"git\" signature for ").nth(1).and_then(|s| s.split(" with ").next()).map(|s| s.trim().to_string());
    if o.status.success() && !err.contains("No principal matched") {
        match who {
            Some(w) if !w.is_empty() => format!("good:{w}"),
            _ => "bad".into(),
        }
    } else if err.contains("no signature") || err.is_empty() || err.contains("cannot verify a non-tag") {
        "unsigned".into()
    } else {
        "bad".into()
    }
}

/// подпись для людей
pub fn sig_text(sig: &str) -> String {
    match sig {
        "off" => crate::tf!("не проверяется", "not checked"),
        "nokeys" => crate::tf!("ключей автора ещё нет (keys/allowed_signers)", "no author keys yet (keys/allowed_signers)"),
        "unsigned" => crate::tf!("не подписано", "not signed"),
        "bad" => crate::tf!("подпись НЕ сходится", "signature does NOT match"),
        "" => "—".into(),
        s => {
            // 193045225+Trendorin@users.noreply.github.com → Trendorin
            let who = s.trim_start_matches("good:");
            let who = who.split('@').next().unwrap_or(who);
            let who = who.split_once('+').map(|(_, n)| n).unwrap_or(who);
            crate::tf!("подписано: {}", "signed: {}", who)
        }
    }
}

// ── установка и откат (процесс `yami update apply`, отдельно от шелла) ─────
/// замок: две установки сразу не пойдут
/// файл держится открытым, пока жив замок: закрытие снимает flock
struct Lock(#[allow(dead_code)] std::fs::File);
fn lock() -> Option<Lock> {
    use std::os::unix::io::AsRawFd;
    let p = yami_px::paths::cache_dir().join("yami/update.lock");
    let _ = std::fs::create_dir_all(p.parent()?);
    let f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(p).ok()?;
    // SAFETY: flock на своём открытом файле; снимается при закрытии
    (unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0).then_some(Lock(f))
}

/// шаг установки: в терминал и в шелл (вкладка показывает, где мы)
fn progress(s: &str) {
    println!("  ▪ {s}");
    let _ = crate::ipc::send(&format!("update-progress {s}"));
}

/// последние строки update.log (без цветов и пустых) и размер файла; читается только хвост
pub fn log_tail() -> (Vec<String>, u64) {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(log_path()) else { return (vec![], 0) };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let from = len.saturating_sub(16 * 1024);
    let mut buf = vec![];
    if f.seek(SeekFrom::Start(from)).is_err() || f.take(16 * 1024).read_to_end(&mut buf).is_err() {
        return (vec![], len);
    }
    let text = String::from_utf8_lossy(&buf);
    let mut lines: Vec<String> = text.lines().skip(usize::from(from > 0)).map(|l| strip_ansi(l).trim().to_string()).filter(|l| !l.is_empty()).collect();
    let keep = lines.len().saturating_sub(40);
    lines.drain(..keep);
    (lines, len)
}

/// «[3/8] шаг» → (3, 8) и текст без счётчика
pub fn step_of(s: &str) -> (Option<(u32, u32)>, &str) {
    let Some(rest) = s.strip_prefix('[') else { return (None, s) };
    let Some((nums, text)) = rest.split_once(']') else { return (None, s) };
    let Some((k, n)) = nums.split_once('/') else { return (None, s) };
    match (k.trim().parse(), n.trim().parse()) {
        (Ok(k), Ok(n)) if n > 0 => (Some((k, n)), text.trim()),
        _ => (None, s),
    }
}

fn copy_bins(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for b in ["yami-shell", "yami-files", "yami-view", "yami-play"] {
        let src = from.join(b);
        if src.exists() {
            // через временное имя: работающий бинарь подменяется целиком (rename), не переписывается
            let tmp = to.join(format!(".{b}.new"));
            std::fs::copy(&src, &tmp)?;
            std::fs::rename(&tmp, to.join(b))?;
        }
    }
    Ok(())
}

/// поставить найденное обновление; Err — почему нет
pub fn apply(set: &Settings) -> Result<String, String> {
    let Some(_l) = lock() else { return Err(crate::tf!("обновление уже идёт", "an update is already running")) };
    progress(&crate::tf!("проверяю, что нового", "checking what is new"));
    let i = check(set);
    if !i.err.is_empty() {
        return Err(i.err);
    }
    if !i.available {
        return Ok(crate::tf!("уже свежее: {}", "already up to date: {}", installed().version));
    }
    if !i.sig_ok(&set.update_verify) {
        return Err(crate::tf!("{} — не ставлю (настройки → обновления → подписи)", "{}: not installing (settings → updates → signatures)", sig_text(&i.sig)));
    }
    build_install(&i)?;
    Ok(crate::tf!("установлено: {}", "installed: {}", i.latest))
}

/// перейти на выбранный выпуск (тег vX.Y.Z или X.Y.Z), старее или новее установленного.
/// Ставится только то, что проходит правило подписей, как и обычное обновление
pub fn install_tag(set: &Settings, want: &str) -> Result<String, String> {
    let Some(_l) = lock() else { return Err(crate::tf!("обновление уже идёт", "an update is already running")) };
    let tag = format!("v{}", want.trim().trim_start_matches('v'));
    if ver(&tag).is_empty() || !tag[1..].chars().all(|c| c.is_ascii_digit() || c == '.') {
        return Err(crate::tf!("нет такой версии: {}", "no such version: {}", want));
    }
    progress(&crate::tf!("ищу {}", "looking for {}", tag));
    let st = installed();
    let dir = ensure_clone(set, &st)?;
    git(&dir, &["fetch", "-q", "--tags", "origin"]).map_err(explain)?;
    let Ok(target) = git(&dir, &["rev-parse", "-q", "--verify", &format!("refs/tags/{tag}^{{commit}}")]) else {
        return Err(crate::tf!("нет такой версии: {}", "no such version: {}", tag));
    };
    let known = has_commit(&dir, &st.commit);
    let mut i = Info { latest: tag[1..].to_string(), reference: tag.clone(), target, available: true, at: now(), ..Default::default() };
    i.sig = verify(&dir, &st, known, &set.update_verify, "verify-tag", &tag);
    if !i.sig_ok(&set.update_verify) {
        return Err(crate::tf!("{} — не ставлю (настройки → обновления → подписи)", "{}: not installing (settings → updates → signatures)", sig_text(&i.sig)));
    }
    if st.commit == i.target {
        return Ok(crate::tf!("{} уже стоит", "{} is already installed", tag));
    }
    build_install(&i)?;
    Ok(crate::tf!("установлено: {}", "installed: {}", i.latest))
}

/// сборка и установка коммита i.target из своего клона; прежние бинари — в откат
fn build_install(i: &Info) -> Result<(), String> {
    let dir = clone_dir();
    if !git(&dir, &["status", "--porcelain", "--untracked-files=no"]).unwrap_or_default().is_empty() {
        return Err(crate::tf!("в {} есть правки — убери их, и обновлю", "{} has local edits: remove them and update again", dir.display()));
    }
    let from = installed().version;
    progress(&crate::tf!("перехожу на {}", "switching to {}", i.reference));
    git(&dir, &["-c", "advice.detachedHead=false", "checkout", "-q", "--detach", &i.target]).map_err(explain)?;
    // прежние бинари и запись об установке — для отката
    let _ = std::fs::remove_dir_all(prev_dir());
    copy_bins(&bin_dir(), &prev_dir()).map_err(|e| e.to_string())?;
    let _ = std::fs::copy(stamp_path(), prev_dir().join("install.json"));
    // старая версия не знает новых настроек и при сохранении их выкинет: снимок, вернутся при обновлении
    if older(&i.latest, &from) {
        snapshot_settings(&from);
    }
    progress(&crate::tf!("сборка и установка (несколько минут)", "build and install (a few minutes)"));
    let mut ch = Command::new("bash")
        .arg(dir.join("install.sh"))
        .arg("--update")
        .env("YAMI_UPDATE", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(out) = ch.stdout.take() {
        use std::io::BufRead;
        for l in std::io::BufReader::new(out).lines().map_while(Result::ok) {
            // шаги установщика («▪ …») — в шелл, остальное — только в лог
            let clean: String = strip_ansi(&l);
            let c = clean.trim();
            if let Some(s) = c.strip_prefix('▪').or_else(|| c.strip_prefix('▸')).or_else(|| c.strip_prefix('✗')) {
                progress(s.trim());
            } else if !c.is_empty() {
                println!("    {c}");
            }
        }
    }
    let ok = ch.wait().map(|s| s.success()).unwrap_or(false);
    // установщик выбранной версии мог не поднять шелл (у старых был такой баг): поднимаем сами
    ensure_shell();
    if !ok {
        return Err(crate::tf!("установка не удалась — подробности в {}", "install failed: details in {}", log_path().display()));
    }
    if !older(&i.latest, &from) {
        restore_settings(&i.latest);
    }
    Ok(())
}

/// a старее b (по номерам версий; пустое — не старее)
fn older(a: &str, b: &str) -> bool {
    !a.is_empty() && !b.is_empty() && ver(a) < ver(b)
}

/// шелл отвечает на сокете; если нет за ~6 с — новый цикл автозапуска niri (второй экземпляр
/// уходит сам). Игру не трогаем: при ней установщик шелл не перезапускал, он жив
fn ensure_shell() {
    let up = || (0..60).any(|_| crate::ipc::send("ping") || { std::thread::sleep(std::time::Duration::from_millis(100)); false });
    if up() || std::env::var_os("NIRI_SOCKET").is_none() {
        return;
    }
    let bin = bin_dir().join("yami-shell").to_string_lossy().replace('\'', "'\\''");
    println!("  {}", crate::tf!("шелл не ответил — запускаю его снова", "the shell did not answer: starting it again"));
    let _ = Command::new("niri").args(["msg", "action", "spawn-sh", "--", &format!("until '{bin}'; do sleep 1; done")]).stdin(Stdio::null()).stdout(Stdio::null()).status();
    if !up() {
        println!("  {}", crate::tf!("шелл так и не запустился: yami run в терминале покажет почему", "the shell did not start: yami run in a terminal shows why"));
    }
}

fn snap_path() -> PathBuf {
    crate::config::state_dir().join("settings-newer.json")
}

/// перед переходом на старую версию: настройки, как есть, с версией, из которой ушли
fn snapshot_settings(from: &str) {
    let Ok(text) = std::fs::read_to_string(crate::config::settings_path()) else { return };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return };
    // снимок уже есть (откат через несколько версий) — первый, самый полный, не трогаем
    if snap_path().exists() {
        return;
    }
    let _ = crate::config::write_atomic(&snap_path(), &(serde_json::json!({ "version": from, "settings": v }).to_string() + "\n"));
}

/// вернулись на версию не старее снимка: ключи, которых нет в настройках, — из снимка
fn restore_settings(now_ver: &str) {
    let Ok(text) = std::fs::read_to_string(snap_path()) else { return };
    let Ok(snap) = serde_json::from_str::<serde_json::Value>(&text) else {
        let _ = std::fs::remove_file(snap_path());
        return;
    };
    if older(now_ver, snap["version"].as_str().unwrap_or("")) {
        return;
    }
    let p = crate::config::settings_path();
    let mut cur = std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).and_then(|v| v.as_object().cloned()).unwrap_or_default();
    let mut added = 0;
    if let Some(old) = snap["settings"].as_object() {
        for (k, v) in old {
            if !cur.contains_key(k) {
                cur.insert(k.clone(), v.clone());
                added += 1;
            }
        }
    }
    if added > 0 {
        let _ = crate::config::write_atomic(&p, &(serde_json::to_string_pretty(&serde_json::Value::Object(cur)).unwrap_or_default() + "\n"));
    }
    let _ = std::fs::remove_file(snap_path());
}

/// версия прошлой сборки (для мгновенного отката), если она есть
pub fn prev_version() -> Option<String> {
    if !prev_dir().join("yami-shell").exists() {
        return None;
    }
    let s: Stamp = std::fs::read_to_string(prev_dir().join("install.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    Some(if s.version.is_empty() { "?".into() } else { s.version })
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::new();
    let mut esc = false;
    for c in s.chars() {
        match c {
            '\x1b' => esc = true,
            c if esc && c.is_ascii_alphabetic() => esc = false,
            _ if esc => {}
            '\r' => {}
            _ => out.push(c),
        }
    }
    out
}

/// вернуть бинари, что стояли до последнего обновления
pub fn rollback() -> Result<String, String> {
    let Some(_l) = lock() else { return Err(crate::tf!("обновление уже идёт", "an update is already running")) };
    let p = prev_dir();
    if !p.join("yami-shell").exists() {
        return Err(crate::tf!("откатывать не на что: обновлений ещё не было", "nothing to roll back to: no update yet"));
    }
    let from = installed().version;
    let to = prev_version().unwrap_or_default();
    if older(&to, &from) {
        snapshot_settings(&from);
    }
    copy_bins(&p, &bin_dir()).map_err(|e| e.to_string())?;
    if p.join("install.json").exists() {
        let _ = std::fs::copy(p.join("install.json"), stamp_path());
    }
    let _ = std::fs::remove_dir_all(&p);
    if !older(&to, &from) {
        restore_settings(&to);
    }
    restart_shell();
    Ok(crate::tf!("вернул {}", "rolled back to {}", installed().version))
}

/// перезапуск шелла, если он работает и игра не в фокусе
/// перезапустить шелл новым бинарём и дождаться именно нового: сокет создаётся заново (другой
/// inode) и отвечает. Не дождались за ~8 с — ensure_shell. При игре не трогаем
pub fn restart_shell() {
    if crate::niri::game_focused() {
        println!("{}", crate::tf!("  игра в фокусе — шелл перезапустится сам при следующем входе (или: yami restart)", "  a game has focus: the shell restarts at the next login (or: yami restart)"));
        return;
    }
    use std::os::unix::fs::MetadataExt;
    let ino = || std::fs::metadata(crate::ipc::path()).map(|m| m.ino()).ok();
    let before = ino();
    if !crate::ipc::send("restart") {
        ensure_shell();
        return;
    }
    for _ in 0..80 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if ino() != before && crate::ipc::send("ping") {
            return;
        }
    }
    ensure_shell();
}

/// записать, что установлено (из дерева dir): установщик и tools/deploy.sh
pub fn stamp(dir: &Path, components: &str) -> Result<(), String> {
    let version = std::fs::read_to_string(dir.join("VERSION")).map(|v| v.trim().to_string()).unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());
    let commit = git(dir, &["rev-parse", "HEAD"]).unwrap_or_default();
    let dirty = !git(dir, &["status", "--porcelain", "--untracked-files=no"]).unwrap_or_default().is_empty();
    let mut repo = git(dir, &["remote", "get-url", "origin"]).unwrap_or_default();
    // свой клон обновлений: адрес тот же, что у исходной установки
    if repo.is_empty() || repo.starts_with('/') {
        repo = installed().repo;
    }
    let old = installed();
    let components = if components.is_empty() { old.components } else { components.to_string() };
    save_json(&stamp_path(), &Stamp { version, commit, dirty, repo, components, date: now() });
    Ok(())
}

// ── шелл: проверка по расписанию, вкладка, команды от `yami update` ─────────
impl App {
    pub fn update_check(&mut self) {
        if self.st.upd.checking {
            return;
        }
        self.st.upd.checking = true;
        let set = self.st.set.clone();
        let tx = self.tx.clone();
        std::thread::Builder::new()
            .name("update".into())
            .spawn(move || {
                let i = check(&set);
                let _ = tx.send(Msg::Update(i));
            })
            .ok();
        self.dirty(&SKind::Modal);
    }

    /// раз в минуту: пора ли проверить (не раньше минуты после старта; ошибка — снова через час)
    pub fn update_tick(&mut self) {
        let s = &self.st.set;
        if !s.update_check || s.update_every == 0 || self.st.upd.checking || self.st.upd.busy.is_some() || self.st.started.elapsed().as_secs() < 60 {
            return;
        }
        let every = match &self.st.upd.info {
            Some(i) if !i.err.is_empty() => 3600,
            _ => s.update_every as u64 * 3600,
        };
        if self.st.upd.info.as_ref().map(|i| now().saturating_sub(i.at) >= every).unwrap_or(true) {
            self.update_check();
        }
    }

    pub fn on_update(&mut self, i: Info) {
        let was = self.st.upd.info.as_ref().filter(|x| x.available).map(|x| x.target.clone()).unwrap_or_default();
        self.st.upd.checking = false;
        self.st.upd.stamp = installed();
        // новое — карточкой, один раз на версию
        if i.available && i.target != was && i.err.is_empty() {
            let body = i.notes.first().cloned().unwrap_or_default();
            let n = crate::dbus::Incoming {
                id: NOTE_ID,
                app: "yami-update".into(),
                icon: String::new(),
                summary: crate::tf!("доступно обновление yami {}", "yami {} is available", i.latest),
                body,
                actions: vec![("default".into(), crate::i18n::tr("подробнее").to_string())],
                urgency: 1,
                image: None,
                timeout: -1,
            };
            self.on_notif(n);
        }
        self.st.upd.info = Some(i);
        self.dirty(&SKind::Modal);
    }

    /// действие карточки «доступно обновление»: открыть вкладку
    pub fn update_note_action(&mut self, id: u32) -> bool {
        if id != NOTE_ID {
            return false;
        }
        self.open_settings_tab("updates");
        true
    }

    /// выбор версии в списке отката
    pub fn update_click(&mut self, a: &Act) -> bool {
        let Act::UpdSel(s) = a else { return false };
        self.st.upd.sel = if self.st.upd.sel == *s { String::new() } else { s.clone() };
        self.dirty(&SKind::Modal);
        true
    }

    /// пока открыта вкладка «обновления»: перечитать хвост лога, если файл вырос
    pub fn update_log_tick(&mut self) {
        if self.st.modal != Some(crate::app::Modal::Settings) || self.st.stab != "updates" {
            return;
        }
        let len = std::fs::metadata(log_path()).map(|m| m.len()).unwrap_or(0);
        if len != self.st.upd.log_len {
            (self.st.upd.log, self.st.upd.log_len) = log_tail();
            self.dirty(&SKind::Modal);
        }
    }

    /// команды шеллу от процесса обновления; true — команда наша
    pub fn update_ipc(&mut self, c: &str) -> bool {
        if let Some(s) = c.strip_prefix("update-progress ") {
            self.st.upd.busy = Some(s.to_string());
            self.st.upd.done = None;
        } else if let Some(s) = c.strip_prefix("update-done ") {
            let (ok, text) = s.split_once(' ').unwrap_or((s, ""));
            self.st.upd.busy = None;
            self.st.upd.done = Some((ok == "ok", text.to_string()));
            self.st.upd = Upd { busy: None, done: self.st.upd.done.take(), ..Upd::load() };
        } else if c == "update-reload" {
            // `yami update check` из терминала уже проверил: перечитать его итог
            let busy = self.st.upd.busy.take();
            self.st.upd = Upd { busy, ..Upd::load() };
        } else {
            return false;
        }
        self.dirty(&SKind::Modal);
        true
    }

    /// кнопки вкладки
    pub fn update_cycle(&mut self, k: &str) -> bool {
        let s = &mut self.st.set;
        match k {
            "update_channel" => {
                s.update_channel = if s.update_channel == "main" { "release".into() } else { "main".into() };
                self.st.upd.info = None;
                self.update_check();
            }
            "update_verify" => {
                s.update_verify = match s.update_verify.as_str() {
                    "auto" => "require".into(),
                    "require" => "off".into(),
                    _ => "auto".into(),
                };
            }
            "update_every" => {
                s.update_every = crate::settings::step_in(&[6, 12, 24, 72, 168], s.update_every as i32) as u32;
            }
            "update_now" => self.update_check(),
            "update_rollback" => {
                // только открыть выбор; список выпусков — из последней проверки, нет его — проверить
                let u = &mut self.st.upd;
                u.pick = !u.pick;
                u.sel.clear();
                if u.pick && u.info.as_ref().map(|i| i.releases.is_empty()).unwrap_or(true) {
                    self.update_check();
                }
            }
            "update_pick_cancel" => {
                self.st.upd.pick = false;
                self.st.upd.sel.clear();
            }
            "update_apply" | "update_pick_go" => {
                if self.st.upd.busy.is_some() {
                    return true;
                }
                let sel = std::mem::take(&mut self.st.upd.sel);
                if k == "update_pick_go" && sel.is_empty() {
                    return true;
                }
                self.st.upd.pick = false;
                self.save_now();
                self.st.upd.busy = Some(crate::tf!("запускаю", "starting"));
                self.st.upd.done = None;
                if k == "update_apply" {
                    spawn_detached(&["apply"]);
                } else if sel == "prev" {
                    spawn_detached(&["rollback"]);
                } else {
                    spawn_detached(&["to", &sel]);
                }
            }
            _ => return false,
        }
        self.dirty(&SKind::Modal);
        true
    }
}

/// `yami-shell update apply --notify` отдельным процессом: живёт, даже когда шелл перезапустится
fn spawn_detached(what: &[&str]) {
    let log = log_path();
    if let Some(p) = log.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let Ok(f) = std::fs::File::create(&log) else { return };
    let Ok(f2) = f.try_clone() else { return };
    let _ = Command::new("setsid").arg("-f").arg(yami_px::paths::self_exe()).arg("update").args(what).arg("--notify").stdin(Stdio::null()).stdout(f).stderr(f2).status();
}

fn ago(at: u64) -> String {
    let d = now().saturating_sub(at);
    if d < 90 {
        crate::tf!("только что", "just now")
    } else if d < 5400 {
        crate::tf!("{} мин назад", "{} min ago", d / 60)
    } else if d < 172_800 {
        crate::tf!("{} ч назад", "{} h ago", d / 3600)
    } else {
        crate::tf!("{} дн назад", "{} days ago", d / 86400)
    }
}

/// github.com/user/repo из адреса
pub fn short_repo(u: &str) -> String {
    let u = if u.is_empty() { REPO } else { u };
    u.trim_start_matches("https://").trim_start_matches("git@").replace(':', "/").trim_end_matches(".git").to_string()
}

/// мини-консоль: хвост update.log, шаги акцентом, готовое зелёным, ошибки красным; пока идёт
/// установка — мигающий блок-курсор в конце
fn console(ui: &mut Ui, r: R, log: &[String], busy: bool) {
    let t = ui.t;
    let px = crate::font::PX;
    ui.cv.pxbox(r, &BoxStyle::new(t.code_bg, t.border));
    let head = R::new(r.x + 10, r.y + 4, r.w - 20, 18);
    ui.cv.fill(R::new(head.x, head.y + 6, 6, 6), if busy { t.red } else { t.text_faint });
    let name = log_path().display().to_string().replace(&std::env::var("HOME").unwrap_or_default(), "~");
    ui.cv.text_in(ui.f, px, R::new(head.x + 12, head.y, head.w - 12, 18), &name, t.text_faint);
    ui.cv.fill(R::new(r.x + 1, r.y + 24, r.w - 2, 1), t.border);
    let rows = ((r.h - 34) / 18).max(1) as usize;
    let skip = log.len().saturating_sub(rows);
    let mut y = r.y + 28;
    let n = log.len() - skip;
    for (i, l) in log[skip..].iter().enumerate() {
        let (c, mark) = if l.starts_with('✓') {
            (t.ok, true)
        } else if l.starts_with('✕') || l.starts_with('✗') || l.to_lowercase().contains("error") {
            (t.warn, true)
        } else if l.starts_with('▪') || l.starts_with('▸') {
            (t.red, true)
        } else {
            (t.text_dim, false)
        };
        let x = r.x + 10 + if mark { 0 } else { 12 };
        let line = ui.f.elide(px, l, r.right() - 10 - x - 14);
        let w = ui.cv.text_in(ui.f, px, R::new(x, y, r.right() - 10 - x, 18), &line, c);
        if busy && i + 1 == n && ui.blink {
            ui.cv.fill(R::new(x + w + 4, y + 3, 7, 12), t.ember);
        }
        y += 18;
    }
}

/// выбор версии: прошлая сборка (сразу) и подписанные выпуски; ставится только кнопкой «поставить»
fn picker(ui: &mut Ui, st: &State, r: R) {
    let t = ui.t;
    let u = &st.upd;
    let px = crate::font::PX;
    let cur = u.stamp.version.clone();
    let mut y = title(ui, r.x, r.y, r.w, "на какую версию перейти");
    let foot = r.bottom() - 28;
    let mut rows: Vec<(String, String, String, bool)> = vec![];
    if let Some(p) = prev_version() {
        rows.push(("prev".into(), crate::tf!("прошлая сборка  {}", "previous build  {}", p), crate::tf!("сразу, без сборки", "instant, no build"), true));
    }
    if let Some(i) = &u.info {
        for rel in &i.releases {
            let ok = Info { sig: rel.sig.clone(), ..Default::default() }.sig_ok(&st.set.update_verify);
            let here = rel.version == cur;
            let note = if here {
                crate::tf!("стоит сейчас", "installed now")
            } else if !ok {
                crate::tf!("{} — не ставится", "{}: will not install", sig_text(&rel.sig))
            } else {
                format!("{}  ·  {}", sig_text(&rel.sig), date_of(rel.date))
            };
            rows.push((rel.version.clone(), rel.tag.clone(), note, ok && !here));
        }
    }
    if rows.is_empty() {
        let msg = if u.checking { crate::tf!("загружаю список выпусков…", "loading the releases…") } else { crate::tf!("переходить некуда: выпусков и прошлой сборки нет", "nothing to switch to: no releases and no previous build") };
        ui.text_in(px, R::new(r.x, y, r.w, 20), &msg, t.text_dim);
    }
    for (id, name, note, can) in &rows {
        if y + 26 > foot - 6 {
            break;
        }
        let rr = R::new(r.x, y, r.w, 24);
        ui.button(rr, "", "", Tone::Ghost, u.sel == *id, *can, Act::UpdSel(id.clone()));
        let c = if !can { t.text_faint.fade(0.6) } else { t.text };
        ui.cv.fill(R::new(rr.x + 8, rr.cy() - 3, 6, 6), if u.sel == *id { t.red } else if *can { t.border } else { t.surface });
        ui.cv.text_in(ui.f, px, R::new(rr.x + 22, rr.y, 170, rr.h), name, c);
        let note = ui.f.elide(px, note, rr.w - 200);
        ui.cv.text_in(ui.f, px, R::new(rr.x + 196, rr.y, rr.w - 204, rr.h), &note, if *can { t.text_dim } else { t.text_faint.fade(0.6) });
        y += 28;
    }
    let half = (r.w - 6) / 2;
    let go = if u.sel.is_empty() {
        crate::tf!("выбери версию", "pick a version")
    } else if u.sel == "prev" {
        crate::tf!("вернуть прошлую сборку", "restore the previous build")
    } else {
        crate::tf!("поставить {}", "install {}", u.sel)
    };
    ui.button(R::new(r.x, foot, half, 28), &go, "check", Tone::Primary, false, !u.sel.is_empty(), Act::SCycle("update_pick_go"));
    ui.button(R::new(r.x + half + 6, foot, half, 28), "отмена", "close", Tone::Plain, false, true, Act::SCycle("update_pick_cancel"));
}

/// дата тега: «3 ноя 2026»
fn date_of(at: u64) -> String {
    if at == 0 {
        return String::new();
    }
    let days = (at / 86400) as i64;
    let (y, m, d) = civil(days);
    const RU: [&str; 12] = ["янв", "фев", "мар", "апр", "мая", "июн", "июл", "авг", "сен", "окт", "ноя", "дек"]; // i18n-ok
    const EN: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let mon = if crate::i18n::en() { EN[(m - 1) as usize] } else { RU[(m - 1) as usize] };
    format!("{d} {mon} {y}")
}

/// дни от 1970-01-01 → (год, месяц, день) (Howard Hinnant)
fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (era * 400 + yoe + i64::from(m <= 2), m, d)
}

pub fn tab(ui: &mut Ui, st: &State, b: R) {
    let t = ui.t;
    let s = &st.set;
    let u = &st.upd;
    let half = (b.w - 20) / 2;
    let px = crate::font::PX;
    let mut y = title(ui, b.x, b.y, b.w, "установлено");
    let ver = if u.stamp.version.is_empty() { env!("CARGO_PKG_VERSION").to_string() } else { u.stamp.version.clone() };
    let commit: String = u.stamp.commit.chars().take(8).collect();
    let mut cur = ver;
    if !commit.is_empty() {
        cur += &format!("  ·  {commit}");
    }
    if u.stamp.dirty {
        cur += &crate::tf!("  ·  с правками", "  ·  with local edits");
    }
    for (k, v) in [("версия", cur), ("источник", short_repo(if s.update_repo.is_empty() { &u.stamp.repo } else { &s.update_repo }))] {
        ui.text_in(px, R::new(b.x, y, 160, 22), k, t.text_faint);
        ui.text_in(px, R::new(b.x + 170, y, b.w - 170, 22), &v, t.text);
        y += 24;
    }
    y += 10;
    y = title(ui, b.x, y, b.w, "обновления");
    let chan = if s.update_channel == "main" { "каждый коммит" } else { "выпуски" };
    opt_btn(ui, R::new(b.x, y, half, 26), "канал", chan, Act::SCycle("update_channel"));
    let ver_l = match s.update_verify.as_str() {
        "require" => "всегда",
        "off" => "выкл",
        _ => "по ключам",
    };
    opt_btn(ui, R::new(b.x + half + 20, y, half, 26), "проверка подписей", ver_l, Act::SCycle("update_verify"));
    y += 34;
    let every = crate::tf!("раз в {} ч", "every {} h", s.update_every);
    ui.toggle(R::new(b.x, y, half, 40), "проверять сама", "только проверка: ставишь ты", s.update_check, Act::SBool("update_check"));
    opt_btn(ui, R::new(b.x + half + 20, y + 7, half, 26), "как часто", &every, Act::SCycle("update_every"));
    y += 50;
    // состояние: идёт установка / проверка / итог
    let (line, col) = if let Some(p) = &u.busy {
        (crate::tf!("ставлю: {}", "installing: {}", step_of(p).1), t.red)
    } else if let Some((ok, txt)) = &u.done {
        (txt.clone(), if *ok { t.ok } else { t.warn })
    } else if u.checking {
        (crate::tf!("проверяю…", "checking…"), t.text_dim)
    } else if let Some(i) = &u.info {
        if !i.err.is_empty() {
            (crate::tf!("не вышло проверить: {}", "check failed: {}", i.err), t.warn)
        } else if i.available {
            (crate::tf!("доступно {}  ·  {}", "{} is available  ·  {}", i.latest, sig_text(&i.sig)), if i.sig_ok(&s.update_verify) { t.ok } else { t.warn })
        } else if i.reference.is_empty() {
            (crate::tf!("выпусков ещё нет  ·  проверено {}", "no releases yet  ·  checked {}", ago(i.at)), t.text_dim)
        } else {
            (crate::tf!("всё свежее  ·  проверено {}", "up to date  ·  checked {}", ago(i.at)), t.text_dim)
        }
    } else {
        (crate::tf!("ещё не проверялось", "not checked yet"), t.text_dim)
    };
    ui.cv.fill(R::new(b.x, y + 7, 6, 6), col);
    ui.text_in(px, R::new(b.x + 14, y, b.w - 14, 20), &line, col);
    y += 24;
    let by = b.bottom() - 62;
    // прогресс по шагам установщика («[k/n]»); до них — первые деления
    if let Some(p) = &u.busy {
        let v = match step_of(p).0 {
            Some((k, n)) => (k.saturating_sub(1) as f32 + 0.5) / n as f32,
            None => 0.04,
        };
        ui.cv.gauge(R::new(b.x, y, b.w, 8), 40, v.clamp(0.0, 1.0), t.red, t.surface.lerp(t.border, 0.3), 2);
        y += 16;
    }
    let notes = u.info.as_ref().filter(|i| i.available && i.err.is_empty() && u.busy.is_none() && u.done.is_none());
    if u.pick && u.busy.is_none() {
        picker(ui, st, R::new(b.x, y, b.w, by - y - 8));
    } else if let Some(i) = notes {
        // что нового: строки CHANGELOG выпуска (английский текст репозитория — без перевода)
        y = title(ui, b.x, y, b.w, &crate::tf!("что нового в {}", "what is new in {}", i.latest));
        for n in &i.notes {
            if y + 18 > by - 8 {
                break;
            }
            ui.cv.fill(R::new(b.x + 2, y + 7, 4, 4), t.red);
            let line = ui.f.elide(px, n, b.w - 14);
            ui.cv.text_in(ui.f, px, R::new(b.x + 14, y, b.w - 14, 18), &line, t.text_dim);
            y += 18;
        }
    } else if !u.log.is_empty() && by - y > 60 {
        console(ui, R::new(b.x, y + 2, b.w, by - y - 12), &u.log, u.busy.is_some());
    }
    let bw = (b.w - 12) / 3;
    let busy = u.busy.is_some() || u.checking;
    let can = u.info.as_ref().map(|i| i.available && i.err.is_empty() && i.sig_ok(&s.update_verify)).unwrap_or(false);
    ui.button(R::new(b.x, by, bw, 28), "проверить", "search", Tone::Plain, false, !busy, Act::SCycle("update_now"));
    ui.button(R::new(b.x + bw + 6, by, bw, 28), "обновить", "down", Tone::Primary, false, can && !busy, Act::SCycle("update_apply"));
    let any = u.rollback || u.info.as_ref().map(|i| !i.releases.is_empty()).unwrap_or(true);
    ui.button(R::new(b.x + 2 * (bw + 6), by, bw, 28), "откатить", "redo", Tone::Plain, u.pick, any && !busy, Act::SCycle("update_rollback"));
    ui.text_in(px, R::new(b.x, by + 36, b.w, 20), "или в терминале: yami update", t.text_faint);
}
