// Бэкенд xdg-desktop-portal: org.freedesktop.impl.portal.FileChooser (и ScreenCast — screencast.rs). Браузеры (Firefox,
// LibreWolf, Chromium), Telegram, Electron и всё, что идёт через портал, вместо окна GNOME
// получают пиксельный диалог yami-files. Сам процесс без окон (~2 МБ): на каждый запрос
// запускает `yami-files --pick …` и ждёт выбранные пути.
//
// Подключение: ~/.local/share/xdg-desktop-portal/portals/yami.portal,
// ~/.config/xdg-desktop-portal/niri-portals.conf (FileChooser=yami), D-Bus-активация из
// ~/.local/share/dbus-1/services — всё кладёт установщик (шаг «портал»).
//
// Новый бинарь (обновление, tools/deploy.sh): `yami-files --portal-reload` шлёт SIGHUP, портал
// exec'ает себя заново, как только свободен (ни показа экрана, ни открытого диалога), — идущая
// трансляция не рвётся. Портала без бэкенда быть не должно: xdg-desktop-portal берёт типы
// источников ScreenCast у живого бэкенда, без него OBS и браузеры не видят, что показывать.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use zbus::zvariant::{OwnedValue, Value};

pub const NAME: &str = "org.freedesktop.impl.portal.desktop.yami";
/// «показать в папке» из браузеров, торрент-клиентов, Telegram
pub const FM1: &str = "org.freedesktop.FileManager1";

struct FileManager;

/// свой бинарь; после обновления старый файл удалён («… (deleted)») — берём новый по тому же пути
pub fn exe() -> Option<std::path::PathBuf> {
    let p = yami_px::paths::self_exe();
    Some(if p.exists() { p } else { "yami-files".into() })
}

fn open_window(args: Vec<String>) {
    let Some(exe) = exe() else { return };
    let _ = std::process::Command::new("setsid").arg("-f").arg(exe).args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
}

fn paths_of(uris: &[String]) -> Vec<String> {
    // окно на каждую папку: потолок, чтобы вызов с тысячей ссылок не открыл тысячу вкладок
    uris.iter().take(16).filter_map(|u| crate::icons::uri_path(u).or_else(|| if u.starts_with('/') { Some(std::path::PathBuf::from(u)) } else { None })).map(|p| p.to_string_lossy().to_string()).collect()
}

#[zbus::interface(name = "org.freedesktop.FileManager1")]
impl FileManager {
    /// открыть папки (каждая — вкладка)
    fn show_folders(&self, uris: Vec<String>, _startup_id: &str) {
        open_window(paths_of(&uris));
    }
    /// открыть папку и выделить файлы
    fn show_items(&self, uris: Vec<String>, _startup_id: &str) {
        open_window(paths_of(&uris));
    }
    fn show_item_properties(&self, uris: Vec<String>, _startup_id: &str) {
        let mut a = vec!["--props".to_string()];
        a.extend(paths_of(&uris));
        open_window(a);
    }
}

struct Chooser;

type Filter = (String, Vec<(u32, String)>);

fn opt_bool(o: &HashMap<String, OwnedValue>, k: &str) -> bool {
    o.get(k).and_then(|v| bool::try_from(v).ok()).unwrap_or(false)
}
fn opt_str(o: &HashMap<String, OwnedValue>, k: &str) -> String {
    match o.get(k).map(|v| &**v) {
        Some(Value::Str(s)) => s.to_string(),
        _ => String::new(),
    }
}
/// ay с нулём на конце → путь
fn opt_path(o: &HashMap<String, OwnedValue>, k: &str) -> Option<String> {
    let v = o.get(k)?;
    let b: Vec<u8> = Vec::<u8>::try_from(v.try_clone().ok()?).ok()?;
    let b: Vec<u8> = b.into_iter().take_while(|&c| c != 0).collect();
    if b.is_empty() {
        return None;
    }
    Some(String::from_utf8_lossy(&b).to_string())
}
fn opt_filters(o: &HashMap<String, OwnedValue>, k: &str) -> Vec<Filter> {
    o.get(k).and_then(|v| v.try_clone().ok()).and_then(|v| Vec::<Filter>::try_from(v).ok()).unwrap_or_default()
}
fn opt_filter(o: &HashMap<String, OwnedValue>, k: &str) -> Option<Filter> {
    o.get(k).and_then(|v| v.try_clone().ok()).and_then(|v| Filter::try_from(v).ok())
}

fn filter_arg(f: &Filter) -> String {
    // 0 — шаблон (*.png), 1 — MIME (image/png): yami-files различает их по «/»
    let pats: Vec<&str> = f.1.iter().map(|(_, p)| p.as_str()).collect();
    format!("{}|{}", f.0.replace('|', "/"), pats.join(";"))
}

/// чем занят портал: сеансы показа экрана (screencast.rs) и открытые диалоги
pub static BUSY: AtomicUsize = AtomicUsize::new(0);
/// пришёл SIGHUP: на месте бинаря новый, перезапуститься при первой свободе
static RELOAD: AtomicBool = AtomicBool::new(false);

/// занятость на время диалога или сеанса; конец — повод перезапуститься, если ждали
pub struct Busy;
impl Busy {
    pub fn take() -> Busy {
        BUSY.fetch_add(1, Ordering::SeqCst);
        Busy
    }
}
impl Drop for Busy {
    fn drop(&mut self) {
        BUSY.fetch_sub(1, Ordering::SeqCst);
        if RELOAD.load(Ordering::SeqCst) {
            // ответ на вызов ещё не ушёл: exec — через секунду, когда zbus его отправит
            std::thread::spawn(|| {
                std::thread::sleep(Duration::from_secs(1));
                reexec_if_idle();
            });
        } else if BUSY.load(Ordering::SeqCst) == 0 {
            // запись или выбор файла кончились: их код (окно, захват) — из памяти, если полминуты
            // никто ничего не просил
            let _ = std::thread::Builder::new().name("shed".into()).spawn(|| {
                std::thread::sleep(Duration::from_secs(30));
                if BUSY.load(Ordering::SeqCst) == 0 {
                    yami_px::shed_code();
                }
            });
        }
    }
}

fn reexec_if_idle() {
    if !RELOAD.load(Ordering::SeqCst) || BUSY.load(Ordering::SeqCst) > 0 {
        return;
    }
    use std::os::unix::process::CommandExt;
    let Some(exe) = exe() else { return };
    // тот же pid (D-Bus-активация его и держит), имена на шине берутся заново
    let e = std::process::Command::new(exe).arg("--portal").exec();
    eprintln!("yami-files --portal: exec: {e}");
}

/// `yami-files --portal-reload`: работающий портал — на новый бинарь, нет его — поднять
pub fn reload() -> i32 {
    let me = std::process::id();
    let running = || yami_px::paths::pids("yami-files", "--portal").into_iter().filter(|p| *p != me).collect::<Vec<u32>>();
    let pids = running();
    for p in &pids {
        unsafe { libc::kill(*p as i32, libc::SIGHUP) };
    }
    // старый портал (без обработчика) от SIGHUP умирает, новый exec'ается: его exe уже не «(deleted)»;
    // занятый (идёт трансляция) остаётся на старом бинаре и перейдёт сам — не ждём его дольше 3 с
    let t0 = Instant::now();
    let deleted = |p: &u32| std::fs::read_link(format!("/proc/{p}/exe")).map(|e| e.to_string_lossy().ends_with(" (deleted)")).unwrap_or(false);
    while pids.iter().any(deleted) && t0.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(50));
    }
    if !running().is_empty() {
        return 0;
    }
    // никого: запуск через шину (D-Bus-активация). Освобождение имени шина видит не мгновенно —
    // пока у имени есть хозяин, просьба о запуске ничего не делает, поэтому проверяем и повторяем
    let Ok(c) = zbus::blocking::Connection::session() else { return 0 };
    let Ok(bus) = zbus::blocking::fdo::DBusProxy::new(&c) else { return 0 };
    let Ok(wk) = zbus::names::WellKnownName::try_from(NAME) else { return 0 };
    for _ in 0..10 {
        if bus.name_has_owner(wk.clone().into()).unwrap_or(false) {
            return 0;
        }
        let _ = bus.start_service_by_name(wk.clone(), 0);
        std::thread::sleep(Duration::from_millis(200));
    }
    eprintln!("yami-files --portal-reload: {NAME} did not start");
    1
}

/// запустить диалог и дождаться ответа; None — отменили
fn pick(args: Vec<String>) -> Option<Vec<String>> {
    let _busy = Busy::take();
    let exe = exe()?;
    let out = std::process::Command::new(exe).arg("--pick").args(&args).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::inherit()).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let v: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().filter(|l| !l.is_empty()).map(|l| crate::icons::file_uri(std::path::Path::new(l))).collect();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

fn common(title: &str, o: &HashMap<String, OwnedValue>, args: &mut Vec<String>) -> Vec<Filter> {
    if !title.is_empty() {
        args.extend(["--title".into(), title.to_string()]);
    }
    let accept = opt_str(o, "accept_label").replace('_', "");
    if !accept.is_empty() {
        args.extend(["--accept".into(), accept.to_lowercase()]);
    }
    if let Some(f) = opt_path(o, "current_folder") {
        args.extend(["--folder".into(), f]);
    }
    let filters = opt_filters(o, "filters");
    let cur = opt_filter(o, "current_filter");
    for f in &filters {
        args.extend(["--filter".into(), filter_arg(f)]);
    }
    if let Some(c) = &cur {
        match filters.iter().position(|f| f == c) {
            Some(i) => args.extend(["--current-filter".into(), i.to_string()]),
            None if filters.is_empty() => args.extend(["--filter".into(), filter_arg(c)]),
            None => {}
        }
    }
    filters
}

fn reply(uris: Option<Vec<String>>, filters: &[Filter]) -> (u32, HashMap<String, OwnedValue>) {
    let mut r: HashMap<String, OwnedValue> = HashMap::new();
    match uris {
        Some(u) => {
            if let Ok(v) = OwnedValue::try_from(Value::from(u)) {
                r.insert("uris".into(), v);
            }
            if let Some(f) = filters.first() {
                if let Ok(v) = OwnedValue::try_from(Value::from(f.clone())) {
                    r.insert("current_filter".into(), v);
                }
            }
            (0, r)
        }
        None => (1, r),
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.FileChooser")]
impl Chooser {
    async fn open_file(&self, _handle: zbus::zvariant::ObjectPath<'_>, _app_id: &str, _parent: &str, title: &str, options: HashMap<String, OwnedValue>) -> (u32, HashMap<String, OwnedValue>) {
        let mut args = vec![if opt_bool(&options, "directory") { "dir".to_string() } else { "open".to_string() }];
        if opt_bool(&options, "multiple") {
            args.push("--multiple".into());
        }
        let filters = common(title, &options, &mut args);
        let r = blocking::unblock(move || pick(args)).await;
        reply(r, &filters)
    }

    async fn save_file(&self, _handle: zbus::zvariant::ObjectPath<'_>, _app_id: &str, _parent: &str, title: &str, options: HashMap<String, OwnedValue>) -> (u32, HashMap<String, OwnedValue>) {
        let mut args = vec!["save".to_string()];
        let filters = common(title, &options, &mut args);
        let mut name = opt_str(&options, "current_name");
        if let Some(f) = opt_path(&options, "current_file") {
            let p = std::path::PathBuf::from(&f);
            if let Some(d) = p.parent() {
                args.extend(["--folder".into(), d.to_string_lossy().to_string()]);
            }
            if name.is_empty() {
                name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            }
        }
        if !name.is_empty() {
            args.extend(["--name".into(), name]);
        }
        let r = blocking::unblock(move || pick(args)).await;
        reply(r, &filters)
    }

    /// несколько файлов в одну папку: спрашиваем папку, имена — как дали
    async fn save_files(&self, _handle: zbus::zvariant::ObjectPath<'_>, _app_id: &str, _parent: &str, title: &str, options: HashMap<String, OwnedValue>) -> (u32, HashMap<String, OwnedValue>) {
        let mut args = vec!["dir".to_string()];
        common(title, &options, &mut args);
        let files: Vec<Vec<u8>> = options.get("files").and_then(|v| v.try_clone().ok()).and_then(|v| Vec::<Vec<u8>>::try_from(v).ok()).unwrap_or_default();
        let r = blocking::unblock(move || pick(args)).await;
        let uris = r.and_then(|d| crate::icons::uri_path(&d[0])).map(|dir| {
            files.iter().map(|f| {
                let n: Vec<u8> = f.iter().copied().take_while(|&c| c != 0).collect();
                let n = String::from_utf8_lossy(&n).to_string();
                crate::icons::file_uri(&crate::fsx::free_name(&dir, &n))
            }).collect::<Vec<String>>()
        });
        reply(uris, &[])
    }
}

pub fn serve() -> i32 {
    // SIGHUP ждёт главная нить (sigwait): маску наследуют все нити, которые zbus запустит ниже
    let mut hup: libc::sigset_t = unsafe { std::mem::zeroed() };
    unsafe {
        libc::sigemptyset(&mut hup);
        libc::sigaddset(&mut hup, libc::SIGHUP);
        libc::pthread_sigmask(libc::SIG_BLOCK, &hup, std::ptr::null_mut());
    }
    let built = zbus::blocking::connection::Builder::session()
        .and_then(|b| b.serve_at("/org/freedesktop/portal/desktop", Chooser))
        .and_then(|b| b.serve_at("/org/freedesktop/portal/desktop", crate::screencast::ScreenCast::new()))
        .and_then(|b| b.serve_at("/org/freedesktop/FileManager1", FileManager))
        .and_then(|b| b.build());
    let conn = match built {
        Ok(c) => c,
        Err(e) => {
            eprintln!("yami-files --portal: {e}");
            return 1;
        }
    };
    let _ = crate::screencast::CONN.set(conn.clone());
    // имена: портал и FileManager1 (второе может быть занято Dolphin — тогда без него)
    use zbus::fdo::RequestNameFlags;
    if let Err(e) = conn.request_name_with_flags(NAME, RequestNameFlags::ReplaceExisting.into()) {
        eprintln!("yami-files --portal: {NAME}: {e}");
    }
    let _ = conn.request_name_with_flags(FM1, RequestNameFlags::ReplaceExisting | RequestNameFlags::AllowReplacement);
    // живём, пока жива сессия; шина держит соединение
    let _keep = conn;
    // старт позади: код окна файлов, который порталу не нужен, — из памяти (5 МБ → ~1)
    std::thread::sleep(std::time::Duration::from_secs(2));
    yami_px::shed_code();
    loop {
        let mut sig = 0;
        if unsafe { libc::sigwait(&hup, &mut sig) } == 0 && sig == libc::SIGHUP {
            RELOAD.store(true, Ordering::SeqCst);
            reexec_if_idle();
        }
    }
}
