// Бэкенд xdg-desktop-portal: org.freedesktop.impl.portal.FileChooser (и ScreenCast — screencast.rs). Браузеры (Firefox,
// LibreWolf, Chromium), Telegram, Electron и всё, что идёт через портал, вместо окна GNOME
// получают пиксельный диалог yami-files. Сам процесс без окон (~2 МБ): на каждый запрос
// запускает `yami-files --pick …` и ждёт выбранные пути.
//
// Подключение: ~/.local/share/xdg-desktop-portal/portals/yami.portal,
// ~/.config/xdg-desktop-portal/niri-portals.conf (FileChooser=yami), D-Bus-активация из
// ~/.local/share/dbus-1/services — всё кладёт установщик (шаг «портал»).

use std::collections::HashMap;
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

/// запустить диалог и дождаться ответа; None — отменили
fn pick(args: Vec<String>) -> Option<Vec<String>> {
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
    loop {
        std::thread::park();
    }
}
