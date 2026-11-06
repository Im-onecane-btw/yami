// Экран входа (yami-greet) в виде шелла: шелл выкладывает в /var/lib/yami-greet/look то, из
// чего тот рисуется, — look.json (палитра и тона, язык, проявление, 闇, шрифт, раскладки,
// мониторы из yami-outputs.kdl) и wall.png (обои, уже обрезанные и в тонах палитры). Папку отдаёт
// пользователю установщик; нет её или она не наша — ничего не пишется. Экран входа проверяет всё
// сам: для него это чужие данные.

use crate::config::Settings;
use std::path::PathBuf;
use std::sync::Mutex;
use yami_px::gfx::Img;

/// ключи настроек, которые читает экран входа (yami_px::look::Look::from_json и он сам)
const KEYS: [&str; 15] = ["palette", "accent", "tint", "glass", "lift", "inner", "light", "lang", "reveal", "revealGrain", "animations", "animSpeed", "lite", "wallKanji", "fontShell"];

/// папка вида экрана входа; YAMI_GREET_LOOK — для проверок без root
pub fn dir() -> PathBuf {
    std::env::var_os("YAMI_GREET_LOOK").map(PathBuf::from).unwrap_or_else(|| "/var/lib/yami-greet/look".into())
}

/// экран входа установлен и папка вида — наша
pub fn writable() -> bool {
    let Ok(c) = std::ffi::CString::new(dir().to_string_lossy().as_bytes()) else { return false };
    unsafe { libc::access(c.as_ptr(), libc::W_OK) == 0 }
}

pub fn look(set: &Settings) -> String {
    let v = serde_json::to_value(set).unwrap_or_default();
    let mut o = serde_json::Map::new();
    o.insert("_".into(), "written by yami-shell for the login screen (yami-greet)".into());
    for k in KEYS {
        if let Some(x) = v.get(k) {
            o.insert(k.into(), x.clone());
        }
    }
    o.insert("kbLayout".into(), crate::keys::layouts(set).into());
    o.insert("kbSwitch".into(), set.kb_switch.clone().into());
    let outs = std::fs::read_to_string(crate::config::config_dir().join("niri/yami-outputs.kdl")).unwrap_or_default();
    o.insert("outputs".into(), outs.chars().take(64 * 1024).collect::<String>().into());
    serde_json::to_string_pretty(&serde_json::Value::Object(o)).unwrap_or_default() + "\n"
}

static LAST: Mutex<String> = Mutex::new(String::new());

/// look.json, если вид изменился (настройки, палитра, мониторы, раскладки)
pub fn publish(set: &Settings) {
    let text = look(set);
    let Ok(mut last) = LAST.lock() else { return };
    if *last == text || !writable() {
        return;
    }
    let p = dir().join("look.json");
    if std::fs::read_to_string(&p).ok().as_deref() != Some(text.as_str()) {
        let _ = crate::config::write_atomic(&p, &text);
    }
    readable(&p);
    *last = text;
}

/// экран входа работает от своего пользователя: файл — на чтение всем, какой бы ни была umask
fn readable(p: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o644)).is_ok()
}

fn key_file() -> PathBuf {
    crate::config::state_dir().join("login-wall.key")
}

/// экрану входа нужны эти обои: он есть, а картинку с таким ключом (wall::Job::key: всё, от чего
/// она зависит) ему ещё не писали
pub fn wanted(key: &str) -> bool {
    writable() && std::fs::read_to_string(key_file()).map(|k| k != key).unwrap_or(true)
}

/// обои экрану входа (из нити подготовки обоев): та же картинка, что на рабочем столе
pub fn wall(img: &Img, key: &str) {
    let kf = key_file();
    let rgb: Vec<u8> = img.px.iter().flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8]).collect();
    let Some(buf) = image::RgbImage::from_raw(img.w as u32, img.h as u32, rgb) else { return };
    let (tmp, dst) = (dir().join(".wall.png.tmp"), dir().join("wall.png"));
    if buf.save_with_format(&tmp, image::ImageFormat::Png).is_ok() && readable(&tmp) && std::fs::rename(&tmp, &dst).is_ok() {
        let _ = crate::config::write_atomic(&kf, key);
    } else {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// `yami login sync`: выложить вид сейчас (установщик — до первого входа; шелл делает это сам при
/// каждой смене вида). Обои — под размер первого монитора niri
pub fn cli(a: &[&str]) -> i32 {
    if !matches!(a.first().copied(), None | Some("sync")) {
        eprintln!("yami login sync");
        return 2;
    }
    if !writable() {
        eprintln!("{}", crate::tf!("экран входа yami не установлен: {} не открыта на запись", "the yami login screen is not installed: {} is not writable", dir().display()));
        return 1;
    }
    let set = Settings::load();
    publish(&set);
    let (w, h) = crate::niri::request(&serde_json::json!("Outputs"))
        .and_then(|v| {
            let o = v["Ok"]["Outputs"].as_object()?.values().find_map(|o| o["logical"].as_object().cloned())?;
            Some((o.get("width")?.as_i64()? as i32, o.get("height")?.as_i64()? as i32))
        })
        .unwrap_or((1920, 1080));
    let t = yami_px::theme::Theme::new(set.accent_rgb(), set.tint, set.glass, set.lift, set.inner, set.light).apply();
    let job = crate::wall::Job::new(&set, &t, w, h, crate::wall::own(&set));
    let key = job.key();
    if let (Some(img), _) = crate::wall::prepare(&job) {
        wall(&img, &key);
    }
    println!("{}", dir().display());
    0
}
