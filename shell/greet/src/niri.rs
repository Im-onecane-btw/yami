// niri экрана входа ($NIRI_SOCKET): выйти после входа, следующая раскладка, погасить мониторы
// в простое. Запрос — строка JSON, ответ — строка JSON.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

fn request(req: &str) -> Option<serde_json::Value> {
    let mut s = UnixStream::connect(std::env::var_os("NIRI_SOCKET")?).ok()?;
    s.set_read_timeout(Some(std::time::Duration::from_secs(2))).ok()?;
    s.write_all(format!("{req}\n").as_bytes()).ok()?;
    let mut out = String::new();
    BufReader::new(s).read_line(&mut out).ok()?;
    serde_json::from_str(&out).ok()
}

pub fn quit() {
    let _ = request(r#"{"Action":{"Quit":{"skip_confirmation":true}}}"#);
}

pub fn next_layout() {
    let _ = request(r#"{"Action":{"SwitchLayout":{"layout":"Next"}}}"#);
}

pub fn monitors_off() {
    let _ = request(r#"{"Action":{"PowerOffMonitors":{}}}"#);
}

/// раскладки niri: имена («English (US)», «Russian») и текущая
pub fn layouts() -> Option<(Vec<String>, usize)> {
    let v = request(r#""KeyboardLayouts""#)?;
    let k = &v["Ok"]["KeyboardLayouts"];
    let names = k["names"].as_array()?.iter().filter_map(|n| n.as_str().map(str::to_string)).collect();
    Some((names, k["current_idx"].as_u64().unwrap_or(0) as usize))
}
