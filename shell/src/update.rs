// Обновления: проверка, установка и откат — из настроек (вкладка «обновления») и из терминала
// (`yami update`). Источник — свой клон репозитория в ~/.local/share/yami/src (не рабочая
// копия разработчика): git fetch, выпуск = самый новый тег vX.Y.Z (канал «main» — вершина
// ветки). Подпись выпуска проверяется ключами keys/allowed_signers из УЖЕ установленной версии
// (доверие с первой установки): подменённый на сервере тег без ключа автора не встанет.
// Ставит install.sh --update (сборка из исходников, без sudo, настройки не трогает), прежние
// бинари — в ~/.local/lib/yami/prev для отката. Что установлено — ~/.local/state/yami/install.json.

use crate::app::{App, Msg, SKind, State};
use crate::config::Settings;
use crate::gfx::R;
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
}

impl Upd {
    pub fn load() -> Upd {
        let stamp = installed();
        let mut info = last();
        // проверка была до установки: то, что она нашла, уже стоит
        if let Some(i) = info.as_mut().filter(|i| i.target == stamp.commit) {
            i.available = false;
        }
        Upd { stamp, info, rollback: prev_dir().join("yami-shell").exists(), ..Default::default() }
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
            u.busy = Some("shell: build from source".into());
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
fn last() -> Option<Info> {
    std::fs::read_to_string(info_path()).ok().map(|t| yami_px::paths::lenient_json(&t))
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
    Ok(())
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
    let (what, obj) = if main { ("verify-commit", i.target.as_str()) } else { ("verify-tag", i.reference.as_str()) };
    let o = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "gpg.format=ssh", "-c", &format!("gpg.ssh.allowedSignersFile={kfs}"), what, "--", obj])
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output();
    let Ok(o) = o else { return "bad".into() };
    let err = String::from_utf8_lossy(&o.stderr).to_string();
    if o.status.success() {
        // Good "git" signature for NAME with ED25519 key SHA256:…
        let who = err.split("signature for ").nth(1).and_then(|s| s.split(" with ").next()).unwrap_or("?").trim().to_string();
        format!("good:{who}")
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
        s => crate::tf!("подписано: {}", "signed: {}", s.trim_start_matches("good:")),
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

fn copy_bins(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for b in ["yami-shell", "yami-files", "yami-view"] {
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
    let dir = clone_dir();
    if !git(&dir, &["status", "--porcelain", "--untracked-files=no"]).unwrap_or_default().is_empty() {
        return Err(crate::tf!("в {} есть правки — убери их, и обновлю", "{} has local edits: remove them and update again", dir.display()));
    }
    progress(&crate::tf!("перехожу на {}", "switching to {}", i.reference));
    git(&dir, &["-c", "advice.detachedHead=false", "checkout", "-q", "--detach", &i.target]).map_err(explain)?;
    // прежние бинари и запись об установке — для отката
    let _ = std::fs::remove_dir_all(prev_dir());
    copy_bins(&bin_dir(), &prev_dir()).map_err(|e| e.to_string())?;
    let _ = std::fs::copy(stamp_path(), prev_dir().join("install.json"));
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
    if !ok {
        return Err(crate::tf!("установка не удалась — подробности в {}", "install failed: details in {}", log_path().display()));
    }
    Ok(crate::tf!("установлено: {}", "installed: {}", i.latest))
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
    copy_bins(&p, &bin_dir()).map_err(|e| e.to_string())?;
    if p.join("install.json").exists() {
        let _ = std::fs::copy(p.join("install.json"), stamp_path());
    }
    let _ = std::fs::remove_dir_all(&p);
    restart_shell();
    Ok(crate::tf!("вернул {}", "rolled back to {}", installed().version))
}

/// перезапуск шелла, если он работает и игра не в фокусе
pub fn restart_shell() {
    if crate::niri::game_focused() {
        println!("{}", crate::tf!("  игра в фокусе — шелл перезапустится сам при следующем входе (или: yami restart)", "  a game has focus: the shell restarts at the next login (or: yami restart)"));
        return;
    }
    let _ = crate::ipc::send("restart");
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
            "update_apply" | "update_rollback" => {
                if self.st.upd.busy.is_some() {
                    return true;
                }
                self.save_now();
                self.st.upd.busy = Some(crate::tf!("запускаю", "starting"));
                self.st.upd.done = None;
                let what = if k == "update_apply" { "apply" } else { "rollback" };
                spawn_detached(what);
            }
            _ => return false,
        }
        self.dirty(&SKind::Modal);
        true
    }
}

/// `yami-shell update apply --notify` отдельным процессом: живёт, даже когда шелл перезапустится
fn spawn_detached(what: &str) {
    let log = log_path();
    if let Some(p) = log.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let Ok(f) = std::fs::File::create(&log) else { return };
    let Ok(f2) = f.try_clone() else { return };
    let _ = Command::new("setsid").arg("-f").arg(yami_px::paths::self_exe()).args(["update", what, "--notify"]).stdin(Stdio::null()).stdout(f).stderr(f2).status();
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
        "require" => "только подписанные",
        "off" => "не проверять",
        _ => "если есть ключи",
    };
    opt_btn(ui, R::new(b.x + half + 20, y, half, 26), "проверка подписей", ver_l, Act::SCycle("update_verify"));
    y += 34;
    let every = crate::tf!("раз в {} ч", "every {} h", s.update_every);
    ui.toggle(R::new(b.x, y, half, 40), "проверять сама", "только проверка: ставишь ты", s.update_check, Act::SBool("update_check"));
    opt_btn(ui, R::new(b.x + half + 20, y + 7, half, 26), "как часто", &every, Act::SCycle("update_every"));
    y += 50;
    // состояние: идёт установка / проверка / итог
    let (line, col) = if let Some(p) = &u.busy {
        (crate::tf!("ставлю: {}", "installing: {}", p), t.red)
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
    y += 26;
    // что нового
    if let Some(i) = u.info.as_ref().filter(|i| i.available && i.err.is_empty() && u.busy.is_none()) {
        for n in i.notes.iter().take(6) {
            ui.text_in(px, R::new(b.x + 14, y, b.w - 14, 20), &format!("· {n}"), t.text_dim);
            y += 20;
        }
    }
    let by = b.bottom() - 62;
    let bw = (b.w - 12) / 3;
    let busy = u.busy.is_some() || u.checking;
    let can = u.info.as_ref().map(|i| i.available && i.err.is_empty() && i.sig_ok(&s.update_verify)).unwrap_or(false);
    ui.button(R::new(b.x, by, bw, 28), "проверить", "search", Tone::Plain, false, !busy, Act::SCycle("update_now"));
    ui.button(R::new(b.x + bw + 6, by, bw, 28), "обновить", "down", Tone::Primary, false, can && !busy, Act::SCycle("update_apply"));
    ui.button(R::new(b.x + 2 * (bw + 6), by, bw, 28), "откатить", "redo", Tone::Plain, false, u.rollback && !busy, Act::SCycle("update_rollback"));
    ui.text_in(px, R::new(b.x, by + 36, b.w, 20), "или в терминале: yami update", t.text_faint);
}
