// Модули бара из скриптов, как custom-модули waybar: исполняемый файл
// ~/.config/yami/modules/<имя> становится модулем `x:<имя>` (он есть в настройках → бар).
// Скрипт печатает строку на каждое обновление — просто текст или JSON:
//   {"text": "21°", "icon": "temp", "color": "accent", "click": "xdg-open https://…", "interval": 600}
// icon — любой значок 7×7 шелла (список в docs/CONFIG.md), color — роль темы (accent, bright, ok,
// warn, text, dim). Скрипт, что работает постоянно, шлёт обновления строками; завершившийся
// запускается снова через `interval` секунд (по умолчанию 10). Каждый модуль — одна нить и один
// процесс, и только пока он в баре.
// i18n: inline pairs

use crate::app::Msg;
use calloop::channel::Sender;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

/// что показывает модуль
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plug {
    pub text: String,
    pub icon: String,
    pub color: String,
    pub click: String,
}

/// работающий модуль: флаг остановки и pid текущего запуска скрипта
struct Run {
    stop: Arc<AtomicBool>,
    pid: Arc<AtomicU32>,
}

#[derive(Default)]
pub struct Plugins {
    pub shown: HashMap<String, Plug>,
    runs: HashMap<String, Run>,
}

pub fn dir() -> PathBuf {
    crate::config::config_dir().join("yami/modules")
}

/// имя модуля — одно имя файла в папке модулей: без «/», «..» и пробелов (из настроек или
/// чужой темы пришёл бы путь к любой программе на диске)
pub fn name_ok(n: &str) -> bool {
    !n.is_empty() && n.len() <= 64 && !n.starts_with('.') && n.chars().all(|c| c.is_alphanumeric() || "._-".contains(c))
}

/// исполняемые файлы в папке модулей → id модулей `x:<имя>`
pub fn available() -> Vec<String> {
    use std::os::unix::fs::PermissionsExt;
    let mut v: Vec<String> = std::fs::read_dir(dir())
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.metadata().map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false))
                .filter_map(|e| e.file_name().to_str().map(|n| format!("x:{n}")))
                .filter(|n| name_ok(&n[2..]))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// строка вывода → что показать; обычный текст — сам текст
pub fn parse(line: &str) -> (Plug, Option<u64>) {
    let line = line.trim();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
        if v.is_object() {
            let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
            let text: String = s("text").chars().take(48).collect();
            return (Plug { text, icon: s("icon"), color: s("color"), click: s("click") }, v.get("interval").and_then(|x| x.as_u64()));
        }
    }
    (Plug { text: line.chars().take(48).collect(), ..Default::default() }, None)
}

impl Plugins {
    /// запустить модули, что в баре, остановить те, что из него ушли
    pub fn sync(&mut self, on_bar: &[String], tx: &Sender<Msg>) {
        let want: Vec<&String> = on_bar.iter().filter(|m| m.strip_prefix("x:").is_some_and(name_ok)).collect();
        let gone: Vec<String> = self.runs.keys().filter(|k| !want.contains(k)).cloned().collect();
        for k in gone {
            if let Some(r) = self.runs.remove(&k) {
                r.stop.store(true, Ordering::Relaxed);
                let pid = r.pid.load(Ordering::Relaxed);
                if pid > 0 {
                    unsafe { libc::kill(pid as i32, libc::SIGTERM) };
                }
            }
            self.shown.remove(&k);
        }
        for m in want {
            if self.runs.contains_key(m) {
                continue;
            }
            let run = Run { stop: Arc::new(AtomicBool::new(false)), pid: Arc::new(AtomicU32::new(0)) };
            spawn(m.clone(), run.stop.clone(), run.pid.clone(), tx.clone());
            self.runs.insert(m.clone(), run);
        }
    }
}

impl Drop for Plugins {
    fn drop(&mut self) {
        for r in self.runs.values() {
            r.stop.store(true, Ordering::Relaxed);
            let pid = r.pid.load(Ordering::Relaxed);
            if pid > 0 {
                unsafe { libc::kill(pid as i32, libc::SIGTERM) };
            }
        }
    }
}

fn spawn(id: String, stop: Arc<AtomicBool>, pid: Arc<AtomicU32>, tx: Sender<Msg>) {
    let path = dir().join(&id[2..]);
    std::thread::spawn(move || {
        let mut wait = 10u64;
        while !stop.load(Ordering::Relaxed) {
            let child = Command::new(&path).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn();
            let Ok(mut child) = child else {
                let _ = tx.send(Msg::Plugin(id.clone(), Plug { text: "?".into(), icon: "close".into(), color: "warn".into(), click: String::new() }));
                return;
            };
            pid.store(child.id(), Ordering::Relaxed);
            if let Some(out) = child.stdout.take() {
                // строка длиннее 4 КБ — не дело модуля бара
                for line in BufReader::new(out.take(1 << 20)).lines() {
                    let Ok(line) = line else { break };
                    if line.trim().is_empty() || line.len() > 4096 {
                        continue;
                    }
                    let (p, iv) = parse(&line);
                    if let Some(iv) = iv {
                        wait = iv.clamp(1, 86_400);
                    }
                    if tx.send(Msg::Plugin(id.clone(), p)).is_err() {
                        return;
                    }
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                }
            }
            let _ = child.kill();
            let _ = child.wait();
            pid.store(0, Ordering::Relaxed);
            // спим короткими шагами: убрали модуль из бара — он быстро останавливается
            for _ in 0..wait * 4 {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        }
    });
}

/// цвет темы по имени роли
pub fn color(t: &crate::theme::Theme, role: &str) -> crate::gfx::C {
    match role {
        "accent" => t.red,
        "bright" => t.ember,
        "ok" => t.ok,
        "warn" => t.warn,
        "dim" => t.text_dim,
        _ => t.text,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn lines() {
        let (p, iv) = super::parse(r#"{"text":"21°","icon":"temp","color":"accent","interval":600}"#);
        assert_eq!((p.text.as_str(), p.icon.as_str(), p.color.as_str(), iv), ("21°", "temp", "accent", Some(600)));
        let (p, iv) = super::parse("  hello  ");
        assert_eq!((p.text.as_str(), iv), ("hello", None));
        assert!(super::name_ok("weather.sh") && !super::name_ok("../../bin/sh") && !super::name_ok("/usr/bin/x") && !super::name_ok(".hidden"));
    }
}
