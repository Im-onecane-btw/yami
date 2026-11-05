// Настройки и темы одним файлом. `yami theme export ФАЙЛ` упаковывает только вид: палитру и
// тона, светлую или тёмную тему, эффекты, шрифты, бар, виджеты, вид дока, отступы, курсор и саму
// картинку обоев; `yami theme import ФАЙЛ` применяет его и больше ничего не трогает. Чужая тема
// ничего не может запустить: ни клавиш, ни команд, ни скриптов, ни источника обновлений.
// `yami export ФАЙЛ` — полный бэкап yami этой машины (все настройки, настройки файлов,
// niri/user.kdl, скрипты бара, обои); `yami import ФАЙЛ` показывает, что идёт с командами (свои
// клавиши, user.kdl, скрипты), и ставит это только с `--yes`. Перед любым импортом всё текущее
// копируется в ~/.local/state/yami/before-import-*.json (тот же формат, пять последних;
// `yami import КОПИЯ --yes` возвращает). Файл — JSON, двоичное в
// base64: читается глазами, сравнивается diff'ом, делится одним файлом.
// i18n: inline pairs

use crate::cmd::Sty;
use crate::config::{config_dir, settings_path, state_dir, write_atomic};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

const FORMAT: &str = "yami-bundle";

/// ключи вида (camelCase, как в yami-shell.json)
fn is_look(k: &str) -> bool {
    const EXACT: &[&str] = &[
        "accent", "palette", "tint", "light", "glass", "lift", "inner", "dim", "blurShell", "blurTerm", "animations", "animSpeed", "animClose", "reveal", "revealGrain", "scanlines", "iconTheme", "iconPixel", "cursor", "cursorSize",
        "curtainStyle", "curtainCell", "cpuStyle", "tempStyle", "trayMode", "clockDate", "clockSeconds", "taskWidth", "widgets", "widgetContrast", "titlebars", "fetchFull", "winGap", "winLeft", "winRight", "winTop", "winBottom",
    ];
    EXACT.contains(&k) || ((k.starts_with("bar") || k.starts_with("wall") || k.starts_with("font") || k.starts_with("dock")) && k != "dockPins" && k != "wallpaper")
}

/// ключи, которые никуда не переносятся: откуда обновления и проверяются ли они
fn is_policy(k: &str) -> bool {
    k.starts_with("update")
}

// ── base64 (RFC 4648; crate ради 40 строк не нужен) ───────────────────────────
const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64(d: &[u8]) -> String {
    let mut s = String::with_capacity(d.len().div_ceil(3) * 4);
    for c in d.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                s.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

pub fn unb64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0);
    for ch in s.bytes().filter(|c| !c.is_ascii_whitespace()) {
        if ch == b'=' {
            break;
        }
        let v = B64.iter().position(|&x| x == ch)? as u32;
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

fn settings_value() -> Map<String, Value> {
    std::fs::read_to_string(settings_path()).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v.as_object().cloned()).unwrap_or_default()
}

/// картинка обоев в файл (подготовленная шеллом копия не нужна: она делается заново)
fn wall_entry(set: &Map<String, Value>) -> Option<Value> {
    let p = set.get("wallpaper")?.as_str()?;
    if p.is_empty() {
        return None;
    }
    let path = Path::new(p);
    let data = std::fs::read(path).ok()?;
    // больше 32 МБ — не обои, чтобы пересылать их файлом
    if data.len() > 32 << 20 {
        return None;
    }
    Some(json!({ "name": path.file_name()?.to_string_lossy(), "data": b64(&data) }))
}

/// содержимое файла: тема (только вид) или все настройки; обои — картинкой, если with_wall
fn bundle(theme: bool, with_wall: bool) -> Value {
    // все ключи, и те, которых в файле ещё нет (значения по умолчанию): тема должна быть полной.
    // Через текст, не to_value: f32 там становится f64 (0.78 → 0.7799999713897705), и импорт
    // своего же файла показывал бы «изменения»
    let set = serde_json::to_string(&crate::config::Settings::load()).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v.as_object().cloned()).unwrap_or_else(settings_value);
    let mut s = Map::new();
    for (k, v) in &set {
        if (theme && is_look(k)) || (!theme && !is_policy(k) && k != "wallpaper") {
            s.insert(k.clone(), v.clone());
        }
    }
    let mut b = json!({ "format": FORMAT, "version": 1, "kind": if theme { "theme" } else { "full" }, "yami": env!("CARGO_PKG_VERSION"), "settings": s });
    if let Some(w) = with_wall.then(|| wall_entry(&set)).flatten() {
        b["wallpaper"] = w;
    }
    if !theme {
        let mut files = Map::new();
        for rel in ["yami-files.json", "niri/user.kdl"] {
            if let Ok(t) = std::fs::read_to_string(config_dir().join(rel)) {
                files.insert(rel.into(), Value::String(t));
            }
        }
        for m in crate::plugins::available() {
            let name = &m[2..];
            if let Ok(t) = std::fs::read(crate::plugins::dir().join(name)) {
                files.insert(format!("yami/modules/{name}"), json!({ "exec": true, "data": b64(&t) }));
            }
        }
        b["files"] = Value::Object(files);
    }
    b
}

pub fn export(y: &Sty, file: &str, theme: bool) -> i32 {
    let b = bundle(theme, true);
    let text = serde_json::to_string_pretty(&b).unwrap_or_default() + "\n";
    if write_atomic(Path::new(file), &text).is_err() {
        eprintln!(" {} {}", y.bad("✕"), crate::tf!("не могу записать {}", "cannot write {}", file));
        return 1;
    }
    let what = if theme { crate::tf!("тема", "theme") } else { crate::tf!("все настройки", "every setting") };
    println!(" {} {} → {}", y.ok("✓"), what, y.bold(file));
    0
}

/// ключи настроек, что запускают команды: только с --yes
const RUNS: &[&str] = &["customBinds", "binds"];

/// какие файлы бывают в полном файле (ровно то, что пишет export) и запускает ли файл команды
fn file_kind(rel: &str) -> Option<bool> {
    match rel {
        "yami-files.json" => Some(false),
        "niri/user.kdl" => Some(true),
        _ => rel.strip_prefix("yami/modules/").filter(|n| crate::plugins::name_ok(n)).map(|_| true),
    }
}

/// что в полном файле может что-то запустить: свои клавиши, user.kdl, скрипты, терминал файлов
fn risky(b: &Value) -> Vec<String> {
    let mut v = vec![];
    if b["settings"]["customBinds"].as_array().is_some_and(|a| !a.is_empty()) {
        v.push(crate::tf!("свои сочетания с командами (customBinds)", "custom keybindings with commands (customBinds)"));
    }
    if b["settings"]["binds"].as_object().is_some_and(|a| !a.is_empty()) {
        v.push(crate::tf!("переназначенные клавиши (binds)", "reassigned keys (binds)"));
    }
    if let Some(f) = b["files"].as_object() {
        for k in f.keys() {
            if file_kind(k) == Some(true) {
                v.push(k.clone());
            }
        }
        if f.get("yami-files.json").and_then(|t| t.as_str()).is_some_and(|t| t.contains("\"terminal\"")) {
            v.push(crate::tf!("терминал файлового менеджера", "the file manager's terminal"));
        }
    }
    v
}

/// yami-files.json из файла; без --yes терминал остаётся свой: это команда, которую файлы запускают
fn files_json(incoming: &str, yes: bool) -> Option<String> {
    let mut v: Map<String, Value> = serde_json::from_str::<Value>(incoming).ok()?.as_object()?.clone();
    if !yes {
        let cur = std::fs::read_to_string(config_dir().join("yami-files.json")).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
        match cur.as_ref().and_then(|c| c.get("terminal")) {
            Some(t) => v.insert("terminal".into(), t.clone()),
            None => v.remove("terminal"),
        };
    }
    Some(serde_json::to_string_pretty(&Value::Object(v)).unwrap_or_default() + "\n")
}

/// больше — не файл настроек и не тема (обои внутри — до 32 МБ, в base64 это ~43 МБ)
const MAX_FILE: u64 = 64 << 20;

pub fn import(y: &Sty, file: &str, theme_only: bool, yes: bool, dry: bool) -> i32 {
    if std::fs::metadata(file).map(|m| m.len() > MAX_FILE).unwrap_or(false) {
        eprintln!(" {} {}", y.bad("✕"), crate::tf!("слишком большой файл: {}", "file too large: {}", file));
        return 2;
    }
    let b: Value = match std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()) {
        Some(v) => v,
        None => {
            eprintln!(" {} {}", y.bad("✕"), crate::tf!("не читается: {}", "cannot read: {}", file));
            return 2;
        }
    };
    if b["format"] != FORMAT {
        eprintln!(" {} {}", y.bad("✕"), crate::tf!("это не файл yami (export)", "not a yami file (export)"));
        return 2;
    }
    let theme = theme_only || b["kind"] == "theme";
    let incoming = b["settings"].as_object().cloned().unwrap_or_default();
    let mut set = settings_value();
    let mut changed = vec![];
    for (k, v) in &incoming {
        // тема — только вид; полный — всё, кроме политики обновлений
        if is_policy(k) || k == "wallpaper" || (theme && !is_look(k)) || (!yes && RUNS.contains(&k.as_str())) {
            continue;
        }
        if set.get(k) != Some(v) {
            changed.push(k.clone());
            set.insert(k.clone(), v.clone());
        }
    }
    let warn = if theme { vec![] } else { risky(&b) };
    y.section(&if theme { crate::tf!("тема", "theme") } else { crate::tf!("все настройки", "every setting") });
    println!("   {} {}", y.dim(&crate::tf!("меняется:", "changes:")), if changed.is_empty() { "—".to_string() } else { changed.join(" ") });
    let wall = b.get("wallpaper").filter(|w| w.is_object());
    if let Some(w) = wall {
        println!("   {} {}", y.dim(&crate::tf!("обои:", "wallpaper:")), w["name"].as_str().unwrap_or(""));
    }
    if !warn.is_empty() {
        let note = if yes { crate::tf!("ставится (--yes):", "installed (--yes):") } else { crate::tf!("пропущено, запускает команды (добавь --yes, если файл твой):", "skipped, it runs commands (add --yes if the file is yours):") };
        println!("   {} {}", y.warn(&note), warn.join(", "));
    }
    if dry {
        println!("\n   {}\n", y.dim(&crate::tf!("--dry-run: ничего не изменено", "--dry-run: nothing changed")));
        return 0;
    }
    // копия того, что было, тем же форматом: `yami import КОПИЯ --yes` возвращает всё назад
    let Some(backup) = backup() else {
        eprintln!(" {} {}", y.bad("✕"), crate::tf!("не могу сохранить копию текущих настроек — ничего не меняю", "cannot save a copy of the current settings: nothing changed"));
        return 1;
    };
    if let Some(w) = wall {
        if let (Some(name), Some(data)) = (w["name"].as_str(), w["data"].as_str().and_then(unb64)) {
            if let Some(p) = wall_file(name, &data) {
                set.insert("wallpaper".into(), Value::String(p.to_string_lossy().to_string()));
            }
        }
    } else if theme {
        // тема без своей картинки — пиксельные обои из её палитры
        set.insert("wallpaper".into(), Value::String(String::new()));
    } else if let Some(p) = b["restore"]["wallpaper"].as_str() {
        // копия перед импортом: обои были файлом на этой машине — путь назад, если файл на месте
        if p.is_empty() || Path::new(p).is_file() {
            set.insert("wallpaper".into(), Value::String(p.to_string()));
        }
    }
    if !theme {
        if let Some(files) = b["files"].as_object() {
            for (rel, v) in files {
                // только файлы, что пишет export: путь вроде autostart/… или fish/config.fish
                // запустил бы что угодно при следующем входе
                let Some(runs) = file_kind(rel) else {
                    println!("   {} {}", y.warn(&crate::tf!("пропущен чужой файл:", "skipped an unknown file:")), rel);
                    continue;
                };
                if (runs && !yes) || !safe_rel(rel) {
                    continue;
                }
                let dst = config_dir().join(rel);
                if let Some(d) = dst.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                match v {
                    Value::String(t) if rel == "yami-files.json" => {
                        if let Some(t) = files_json(t, yes) {
                            let _ = write_atomic(&dst, &t);
                        }
                    }
                    Value::String(t) => {
                        let _ = write_atomic(&dst, t);
                    }
                    Value::Object(o) => {
                        if let Some(data) = o.get("data").and_then(|d| d.as_str()).and_then(unb64) {
                            if std::fs::write(&dst, data).is_ok() && o.get("exec").and_then(|e| e.as_bool()) == Some(true) {
                                use std::os::unix::fs::PermissionsExt;
                                let _ = std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o755));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    // через Settings: чужие строки чистятся (sanitize), негодные значения — по умолчанию, файл —
    // в том же виде, что пишет шелл
    let raw = serde_json::to_string(&Value::Object(set)).unwrap_or_default();
    let mut merged = crate::config::Settings::lenient(&raw);
    merged.sanitize();
    let text = merged.to_json();
    if write_atomic(&settings_path(), &text).is_err() {
        eprintln!(" {} {}", y.bad("✕"), crate::tf!("не могу записать настройки", "cannot write the settings"));
        return 1;
    }
    println!("\n {} {}", y.ok("✓"), crate::tf!("готово — шелл перечитает сам", "done; the shell reloads by itself"));
    println!("   {}\n", y.dim(&crate::tf!("вернуть как было: yami import {} --yes", "to undo: yami import {} --yes", backup.display())));
    0
}

/// картинка обоев из файла → ~/.local/share/yami/wallpapers: имя — только имя файла (без пути и
/// скрытых), расширение — по содержимому (PNG, JPEG, WebP; другое не обои); чужую картинку с тем же
/// именем не затирает
fn wall_file(name: &str, data: &[u8]) -> Option<PathBuf> {
    let ext = if data.starts_with(b"\x89PNG") {
        "png"
    } else if data.starts_with(&[0xff, 0xd8, 0xff]) {
        "jpg"
    } else if data.len() > 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        "webp"
    } else {
        return None;
    };
    let stem: String = Path::new(name).file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default().chars().filter(|c| c.is_alphanumeric() || "._- ".contains(*c)).take(64).collect();
    let stem = stem.trim_start_matches('.').trim();
    let stem = if stem.is_empty() { "wallpaper" } else { stem };
    let dir = crate::config::data_dir().join("yami/wallpapers");
    std::fs::create_dir_all(&dir).ok()?;
    let p = (0..100).map(|n| dir.join(if n == 0 { format!("{stem}.{ext}") } else { format!("{stem}-{n}.{ext}") })).find(|p| std::fs::read(p).map(|d| d == data).unwrap_or(true))?;
    if !p.exists() {
        std::fs::write(&p, data).ok()?;
    }
    Some(p)
}

/// номер копии из имени before-import-<секунды>[-<n>].json: по нему, а не по времени файла
/// (копии за одну секунду получают одно и то же время изменения)
fn backup_key(p: &Path) -> Option<(u64, u32)> {
    let n = p.file_name()?.to_str()?.strip_prefix("before-import-")?.strip_suffix(".json")?;
    let (a, b) = n.split_once('-').unwrap_or((n, "0"));
    Some((a.parse().ok()?, b.parse().ok()?))
}

/// копия всех настроек перед импортом: ~/.local/state/yami/before-import-<время>.json, пять последних
fn backup() -> Option<PathBuf> {
    let dir = state_dir();
    std::fs::create_dir_all(&dir).ok()?;
    let mut b = bundle(false, false);
    b["restore"] = json!({ "wallpaper": settings_value().get("wallpaper").and_then(|w| w.as_str()).unwrap_or("") });
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    // два импорта за секунду не затирают копию первого: номер — следующий за самым большим в эту
    // секунду, так копии по имени идут ровно в том порядке, в каком их сняли
    let n = std::fs::read_dir(&dir).ok()?.flatten().filter_map(|e| backup_key(&e.path())).filter(|k| k.0 == stamp).map(|k| k.1 + 1).max().unwrap_or(0);
    let p = dir.join(if n == 0 { format!("before-import-{stamp}.json") } else { format!("before-import-{stamp}-{n}.json") });
    write_atomic(&p, &(serde_json::to_string_pretty(&b).ok()? + "\n")).ok()?;
    let mut old: Vec<(PathBuf, (u64, u32))> = std::fs::read_dir(&dir).ok()?.flatten().map(|e| e.path()).filter_map(|p| backup_key(&p).map(|k| (p, k))).collect();
    old.sort_by_key(|o| o.1);
    // только что снятая копия — последняя по имени и никогда не удаляется
    for o in old.iter().take(old.len().saturating_sub(5)).filter(|o| o.0 != p) {
        let _ = std::fs::remove_file(&o.0);
    }
    Some(p)
}

/// только простые относительные пути внутри ~/.config: без «..» и без абсолютных
fn safe_rel(rel: &str) -> bool {
    let p = PathBuf::from(rel);
    !rel.is_empty() && p.is_relative() && p.components().all(|c| matches!(c, std::path::Component::Normal(_)))
}

/// `yami theme …`, `yami export …`, `yami import …`
pub fn run(y: &Sty, cmd: &str, args: &[&str]) -> i32 {
    let flags = |f: &str| args.contains(&f);
    let file = args.iter().find(|a| !a.starts_with("--")).copied();
    // опечатка во флаге («--dryrun») не должна молча превратиться в настоящий импорт
    let known: &[&str] = match cmd {
        "import" => &["--yes", "--dry-run"],
        "theme-import" => &["--dry-run"],
        _ => &[],
    };
    if let Some(bad) = args.iter().find(|a| a.starts_with("--") && !known.contains(a)) {
        eprintln!(" {} {}", y.bad("✕"), crate::tf!("неизвестный флаг: {}", "unknown flag: {}", bad));
        return 2;
    }
    if args.iter().filter(|a| !a.starts_with("--")).count() > 1 {
        eprintln!(" {} {}", y.bad("✕"), crate::tf!("нужен один файл", "one file, please"));
        return 2;
    }
    match (cmd, file) {
        ("theme-export", Some(f)) => export(y, f, true),
        ("export", Some(f)) => export(y, f, false),
        ("theme-import", Some(f)) => import(y, f, true, false, flags("--dry-run")),
        ("import", Some(f)) => import(y, f, false, flags("--yes"), flags("--dry-run")),
        _ => {
            y.section("yami theme / export / import");
            y.row("theme export FILE", &crate::tf!("вид в один файл: палитра, эффекты, шрифты, бар, виджеты, обои", "the look in one file: palette, effects, fonts, bar, widgets, wallpaper"));
            y.row("theme import FILE [--dry-run]", &crate::tf!("применить чужую тему: только вид, без команд", "apply someone's theme: the look only, no commands"));
            y.row("export FILE", &crate::tf!("все настройки, user.kdl, скрипты бара, обои", "every setting, user.kdl, bar scripts, wallpaper"));
            y.row("import FILE [--yes] [--dry-run]", &crate::tf!("вернуть всё; команды и скрипты — только с --yes", "restore it all; commands and scripts only with --yes"));
            println!();
            2
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_round_trip() {
        for d in [&b""[..], b"f", b"fo", b"foo", b"foob", b"fooba", b"foobar", &[0, 255, 128, 7]] {
            assert_eq!(super::unb64(&super::b64(d)).unwrap(), d);
        }
        assert_eq!(super::b64(b"foobar"), "Zm9vYmFy");
        assert_eq!(super::b64(b"fo"), "Zm8=");
    }
    #[test]
    fn keys() {
        assert!(super::is_look("barLeft") && super::is_look("light") && super::is_look("fontShell"));
        assert!(!super::is_look("customBinds") && !super::is_look("updateRepo") && !super::is_look("dockPins"));
        assert!(super::is_policy("updateRepo"));
        assert!(super::safe_rel("niri/user.kdl") && !super::safe_rel("../x") && !super::safe_rel("/etc/x"));
        assert_eq!(super::file_kind("yami-files.json"), Some(false));
        assert_eq!(super::file_kind("yami/modules/w.sh"), Some(true));
        assert_eq!(super::file_kind("autostart/x.desktop"), None);
        assert_eq!(super::file_kind("yami/modules/../../fish/config.fish"), None);
        assert_eq!(super::backup_key(std::path::Path::new("/s/before-import-1793885973-12.json")), Some((1793885973, 12)));
        assert_eq!(super::backup_key(std::path::Path::new("/s/before-import-1793885973.json")), Some((1793885973, 0)));
        assert_eq!(super::backup_key(std::path::Path::new("/s/other.json")), None);
    }
}
