// Конфиг niri для экрана входа (`yami-greet kdl`, его пишет greet-session при каждом старте).
// Свой файл niri вида не читает: из look.json берутся только мониторы (то, что написал шелл в
// yami-outputs.kdl) и раскладки, и каждое значение проверяется по белому списку символов.
// Иначе файл пользователя мог бы дать niri экрана входа spawn или бинд — а он видит пароли всех.
// --plain: без мониторов, раскладок и ключей новых niri (если полный конфиг не прошёл проверку).

use serde_json::Value;

fn ok_chars(s: &str, extra: &str, max: usize) -> bool {
    !s.is_empty() && s.len() <= max && s.chars().all(|c| c.is_ascii_alphanumeric() || extra.contains(c))
}

const TRANSFORMS: [&str; 8] = ["normal", "90", "180", "270", "flipped", "flipped-90", "flipped-180", "flipped-270"];

/// блоки output из yami-outputs.kdl шелла: имя, off, mode, scale, transform, position
pub fn outputs(text: &str) -> String {
    let mut out = String::new();
    let mut open = false;
    for l in text.lines().map(str::trim).take(400) {
        if let Some(rest) = l.strip_prefix("output \"") {
            if open {
                out += "}\n";
            }
            open = false;
            let Some(name) = rest.strip_suffix("\" {") else { continue };
            if !ok_chars(name, " -_.", 96) {
                continue;
            }
            out += &format!("output \"{name}\" {{\n");
            open = true;
            continue;
        }
        if !open {
            continue;
        }
        let w: Vec<&str> = l.split_whitespace().collect();
        match w.as_slice() {
            ["}"] => {
                out += "}\n";
                open = false;
            }
            ["off"] => out += "    off\n",
            ["mode", m] => {
                let m = m.trim_matches('"');
                let good = m.split_once('x').and_then(|(a, b)| Some((a, b.split_once('@')?))).is_some_and(|(a, (b, r))| {
                    a.parse::<u32>().is_ok_and(|v| v <= 16384) && b.parse::<u32>().is_ok_and(|v| v <= 16384) && r.parse::<f64>().is_ok_and(|v| (1.0..=1000.0).contains(&v))
                });
                if good {
                    out += &format!("    mode \"{m}\"\n");
                }
            }
            ["scale", s] => {
                if let Some(v) = s.parse::<f64>().ok().filter(|v| (0.25..=8.0).contains(v)) {
                    out += &format!("    scale {v}\n");
                }
            }
            ["transform", t] => {
                let t = t.trim_matches('"');
                if TRANSFORMS.contains(&t) {
                    out += &format!("    transform \"{t}\"\n");
                }
            }
            ["position", x, y] => {
                let n = |s: &str, k: &str| s.strip_prefix(k).and_then(|v| v.parse::<i32>().ok()).filter(|v| v.abs() <= 65536);
                if let (Some(x), Some(y)) = (n(x, "x="), n(y, "y=")) {
                    out += &format!("    position x={x} y={y}\n");
                }
            }
            _ => {}
        }
    }
    if open {
        out += "}\n";
    }
    out
}

fn q(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// весь конфиг; me — путь к yami-greet
pub fn config(look: &Value, plain: bool, me: &str) -> String {
    let s = |k: &str| look.get(k).and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let t = yami_px::look::Look::from_json(look, None).theme;
    let mut c = String::from("// yami-greet: niri of the login screen (written by `yami-greet kdl` at every start)\n\n");
    c += "hotkey-overlay {\n    skip-at-startup\n}\nprefer-no-csd\nscreenshot-path null\nanimations {\n    off\n}\n";
    if !plain {
        // ключи niri 25.x: без них старый niri конфиг не примет, тогда идёт --plain
        c += "config-notification {\n    disable-failed\n}\nxwayland-satellite {\n    off\n}\n";
        c += &format!("layout {{\n    background-color \"#{:06x}\"\n}}\n", t.bg.0 & 0xffffff);
    }
    if std::path::Path::new("/usr/share/icons/yami-cursor/cursors").is_dir() {
        c += "cursor {\n    xcursor-theme \"yami-cursor\"\n    xcursor-size 24\n    hide-when-typing\n}\n";
    }
    c += "input {\n    keyboard {\n";
    let (lay, opt) = (s("kbLayout"), s("kbSwitch"));
    if !plain && ok_chars(&lay, ",_-()", 120) {
        c += &format!("        xkb {{\n            layout {}\n", q(&lay));
        if ok_chars(&opt, ",_-():", 200) {
            c += &format!("            options {}\n", q(&opt));
        }
        c += "        }\n";
    }
    if !plain {
        c += "        numlock\n";
    }
    c += "    }\n    touchpad {\n        tap\n    }\n}\n";
    if !plain {
        c += &outputs(&s("outputs"));
    }
    // экран входа вышел (вошли, упал, выключают) — niri следом; код выхода — greet-session
    c += &format!(
        "\nspawn-at-startup \"sh\" \"-c\" {} {}\n",
        q("\"$0\"; echo $? >\"${XDG_RUNTIME_DIR:-/tmp}/yami-greet.rc\"; niri msg action quit --skip-confirmation"),
        q(me)
    );
    c
}

#[cfg(test)]
mod tests {
    #[test]
    fn outputs_whitelist() {
        let t = "// yami\n\noutput \"DP-2\" {\n    mode \"1920x1080@180.000\"\n    scale 1\n    transform \"normal\"\n    position x=0 y=0\n    variable-refresh-rate on-demand=true\n}\n\noutput \"HDMI-A-1\" {\n    off\n}\nspawn-at-startup \"evil\"\noutput \"x\\\" { spawn\" {\n    mode \"1x1@60;rm\"\n}\n";
        let o = super::outputs(t);
        assert!(o.contains("output \"DP-2\" {\n    mode \"1920x1080@180.000\"\n    scale 1\n    transform \"normal\"\n    position x=0 y=0\n}\n"));
        assert!(o.contains("output \"HDMI-A-1\" {\n    off\n}"));
        assert!(!o.contains("evil") && !o.contains("spawn") && !o.contains("1x1"));
    }
}
