// Меню программ и буфер обмена: шапка, поиск, список, строка подсказок.
// Tab переключает режим. Частые программы — выше. В поиске ещё и действия системы
// («сон», «заблокировать», «настройки»…). В буфере: закреплённое сверху (хранится копией,
// переживает очистку и ротацию cliphist), Ctrl+P — закрепить, Del — удалить.

use crate::app::{App, Modal, Msg, SKind, State};
use crate::config;
use crate::font::PX;
use crate::gfx::{BoxStyle, R};
use crate::icons::{exec_argv, App as DApp};
use crate::theme::WHITE;
use crate::ui::{Act, Cur, Field, Tone, Ui};
use calloop::channel::Sender;
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Keysym};
use std::collections::HashMap;
use std::time::Instant;

pub const SIZE: (i32, i32) = (648, 478);
const ROW: i32 = 30;
pub const ROWS: usize = 11;

#[derive(Clone, Debug)]
pub enum Kind {
    App(usize),
    Clip(String),
    Pin(String),
    Sys(&'static str),
}

#[derive(Clone, Debug)]
pub struct Item {
    pub kind: Kind,
    pub name: String,
    pub sub: String,
    pub icon: String,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Pin {
    pub id: String,
    pub file: String,
    pub mime: String,
    pub image: bool,
    pub name: String,
    pub ts: i64,
}

#[derive(Default)]
pub struct LState {
    pub field: Field,
    pub cur: usize,
    pub scroll: usize,
    pub clip: bool,
    pub clip_lines: Vec<String>,
    pub pins: Vec<Pin>,
    pub confirm_wipe: Option<Instant>,
    pub items: Vec<Item>,
    pub counts: HashMap<String, u32>,
}

const SYS: &[(&str, &str, &str)] = &[
    ("lock", "заблокировать", "lock"),
    ("sleep", "сон", "moon"),
    ("logout", "выйти из сеанса", "exit"),
    ("reboot", "перезагрузка", "reboot"),
    ("poweroff", "выключить", "power"),
    ("settings", "настройки шелла", "gear"),
    ("clipboard", "буфер обмена", "copy"),
    ("stacking", "режим стол / лента", "float"),
    ("edit", "расставить виджеты", "grid"),
];

fn clip_dir() -> std::path::PathBuf {
    config::data_dir().join("yami-clip")
}

fn score(hay: &str, q: &str) -> i32 {
    let hay = hay.to_lowercase();
    if q.is_empty() {
        return 1;
    }
    if hay.starts_with(q) {
        return 100;
    }
    if let Some(i) = hay.find(q) {
        let prev = hay[..i].chars().last();
        if matches!(prev, Some(' ') | Some('-') | Some('_') | Some('.')) {
            return 80;
        }
        return 60;
    }
    let qc: Vec<char> = q.chars().collect();
    let mut j = 0;
    for ch in hay.chars() {
        if j < qc.len() && ch == qc[j] {
            j += 1;
        }
    }
    if j == qc.len() {
        20
    } else {
        0
    }
}

impl LState {
    pub fn open(&mut self, clip: bool, apps: &[DApp]) {
        self.field.clear();
        self.cur = 0;
        self.scroll = 0;
        self.clip = clip;
        self.confirm_wipe = None;
        self.counts = std::fs::read_to_string(config::state_dir().join("launch.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        self.load_pins();
        self.refresh(apps);
    }
    pub fn set_mode(&mut self, clip: bool, apps: &[DApp]) {
        self.clip = clip;
        self.cur = 0;
        self.scroll = 0;
        self.refresh(apps);
    }
    /// выбранная строка всегда в видимой части списка
    pub fn keep_visible(&mut self) {
        if self.cur < self.scroll {
            self.scroll = self.cur;
        }
        if self.cur >= self.scroll + ROWS {
            self.scroll = self.cur + 1 - ROWS;
        }
    }
    pub fn load_pins(&mut self) {
        self.pins = std::fs::read_to_string(clip_dir().join("index.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    }

    pub fn refresh(&mut self, apps: &[DApp]) {
        let q = self.field.text.trim().to_lowercase();
        let mut out: Vec<Item> = vec![];
        if self.clip {
            for p in &self.pins {
                if q.is_empty() || p.name.to_lowercase().contains(&q) {
                    out.push(Item { kind: Kind::Pin(p.id.clone()), name: p.name.clone(), sub: if p.image { "картинка".into() } else { String::new() }, icon: "pin".into() });
                }
            }
            let same = |n: &str| self.pins.iter().any(|p| p.name == n || (n.chars().count() > 40 && p.name.starts_with(&n.chars().take(40).collect::<String>())));
            for l in &self.clip_lines {
                let name = l.split_once('\t').map(|x| x.1).unwrap_or(l).split_whitespace().collect::<Vec<_>>().join(" ");
                if (!q.is_empty() && !name.to_lowercase().contains(&q)) || same(&name) {
                    continue;
                }
                let img = name.starts_with("[[ binary");
                out.push(Item { kind: Kind::Clip(l.clone()), name, sub: if img { "картинка".into() } else { String::new() }, icon: String::new() });
                if out.len() > 120 {
                    break;
                }
            }
        } else {
            let mut scored: Vec<(f32, usize)> = vec![];
            for (i, a) in apps.iter().enumerate() {
                let s = (score(&a.name, &q) as f32)
                    .max(score(&a.generic, &q) as f32 * 0.8)
                    .max(score(&a.keywords, &q) as f32 * 0.6)
                    .max(score(&a.comment, &q) as f32 * 0.3)
                    .max(score(&a.id, &q) as f32 * 0.5);
                if s > 0.0 {
                    let used = *self.counts.get(&a.id).unwrap_or(&0) as f32;
                    scored.push((s + used.min(40.0) * if q.is_empty() { 1.0 } else { 0.25 }, i));
                }
            }
            scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal).then(apps[a.1].name.cmp(&apps[b.1].name)));
            if !q.is_empty() {
                for (id, name, icon) in SYS {
                    if score(name, &q) >= 60 {
                        out.push(Item { kind: Kind::Sys(id), name: name.to_string(), sub: "система".into(), icon: icon.to_string() });
                    }
                }
            }
            for (_, i) in scored.into_iter().take(80) {
                let a = &apps[i];
                out.push(Item { kind: Kind::App(i), name: a.name.clone(), sub: if a.generic.is_empty() { a.comment.clone() } else { a.generic.clone() }, icon: a.icon.clone() });
            }
        }
        self.items = out;
        self.cur = self.cur.min(self.items.len().saturating_sub(1));
    }
}

pub fn load_clip(tx: Sender<Msg>) {
    std::thread::spawn(move || {
        let o = std::process::Command::new("cliphist").arg("list").output();
        let lines = o.map(|o| String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.is_empty()).map(String::from).collect()).unwrap_or_default();
        let _ = tx.send(Msg::Clip(lines));
    });
}

fn mime_of(d: &[u8]) -> (&'static str, &'static str) {
    if d.starts_with(b"\x89PNG") {
        ("image/png", "png")
    } else if d.starts_with(&[0xff, 0xd8, 0xff]) {
        ("image/jpeg", "jpg")
    } else if d.starts_with(b"GIF8") {
        ("image/gif", "gif")
    } else if d.len() > 12 && &d[..4] == b"RIFF" && &d[8..12] == b"WEBP" {
        ("image/webp", "webp")
    } else {
        ("text/plain;charset=utf-8", "txt")
    }
}

fn cliphist(args: &[&str], input: &str) -> Vec<u8> {
    use std::io::Write;
    let Ok(mut c) = std::process::Command::new("cliphist").args(args).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn() else { return vec![] };
    if let Some(mut i) = c.stdin.take() {
        let _ = i.write_all(input.as_bytes());
    }
    c.wait_with_output().map(|o| o.stdout).unwrap_or_default()
}

fn save_pins(p: &[Pin]) {
    let _ = config::write_atomic(&clip_dir().join("index.json"), &serde_json::to_string_pretty(p).unwrap_or_default());
}

fn wl_copy(data: Vec<u8>, mime: &str) {
    use std::io::Write;
    if let Ok(mut c) = std::process::Command::new("setsid").args(["-f", "wl-copy", "--type", mime]).stdin(std::process::Stdio::piped()).spawn() {
        if let Some(mut i) = c.stdin.take() {
            let _ = i.write_all(&data);
        }
        let _ = c.wait();
    }
}

impl App {
    fn lrefresh(&mut self) {
        self.st.launcher.refresh(&self.st.apps);
        self.dirty(&SKind::Modal);
    }

    pub fn launcher_run(&mut self, i: usize) {
        let Some(it) = self.st.launcher.items.get(i).cloned() else { return };
        match it.kind {
            Kind::App(ai) => {
                if let Some(a) = self.st.apps.get(ai).cloned() {
                    *self.st.launcher.counts.entry(a.id.clone()).or_insert(0) += 1;
                    let _ = config::write_atomic(&config::state_dir().join("launch.json"), &serde_json::to_string(&self.st.launcher.counts).unwrap_or_default());
                    let mut argv = exec_argv(&a.exec);
                    if a.terminal {
                        // Terminal=true: обернуть в терминал
                        let t = term();
                        let mut pre = vec![t.clone()];
                        match t.as_str() {
                            "kitty" => {}
                            "wezterm" => pre.extend(["start".into(), "--".into()]),
                            _ => pre.push("-e".into()),
                        }
                        pre.extend(argv);
                        argv = pre;
                    }
                    App::spawn(&argv);
                }
            }
            Kind::Clip(line) => {
                std::thread::spawn(move || {
                    let data = cliphist(&["decode"], &line);
                    let (mime, _) = mime_of(&data);
                    wl_copy(data, mime);
                });
            }
            Kind::Pin(id) => {
                if let Some(p) = self.st.launcher.pins.iter().find(|p| p.id == id) {
                    let path = clip_dir().join(&p.file);
                    let mime = p.mime.clone();
                    std::thread::spawn(move || {
                        if let Ok(d) = std::fs::read(path) {
                            wl_copy(d, &mime);
                        }
                    });
                }
            }
            Kind::Sys(a) => {
                self.close_modal();
                self.sys_action(a);
                return;
            }
        }
        self.close_modal();
    }

    pub fn sys_action(&mut self, a: &str) {
        match a {
            "lock" => self.lock_now(),
            "sleep" => {
                self.lock_now();
                App::sh("sleep 0.6; systemctl suspend");
            }
            "logout" => self.st.niri.act(serde_json::json!({"Quit": {"skip_confirmation": true}})),
            "reboot" => App::spawn(&["systemctl".into(), "reboot".into()]),
            "poweroff" => App::spawn(&["systemctl".into(), "poweroff".into()]),
            "settings" => self.toggle_modal(Modal::Settings, false),
            "clipboard" => self.toggle_modal(Modal::Launcher, true),
            "stacking" => self.ipc("stacking"),
            "edit" => self.set_editing(true),
            _ => {}
        }
    }

    pub fn launcher_pin(&mut self, i: usize) {
        let Some(it) = self.st.launcher.items.get(i).cloned() else { return };
        match it.kind {
            Kind::Pin(id) => {
                let mut pins = self.st.launcher.pins.clone();
                if let Some(p) = pins.iter().find(|p| p.id == id) {
                    let _ = std::fs::remove_file(clip_dir().join(&p.file));
                }
                pins.retain(|p| p.id != id);
                save_pins(&pins);
            }
            Kind::Clip(line) => {
                let data = cliphist(&["decode"], &line);
                if data.is_empty() {
                    return;
                }
                let id = format!("{:016x}", fnv(&data));
                let mut pins = self.st.launcher.pins.clone();
                if pins.iter().any(|p| p.id == id) {
                    return;
                }
                let (mime, ext) = mime_of(&data);
                let file = format!("{id}.{ext}");
                let _ = std::fs::create_dir_all(clip_dir());
                let _ = std::fs::write(clip_dir().join(&file), &data);
                let name = if ext == "txt" {
                    String::from_utf8_lossy(&data).split_whitespace().collect::<Vec<_>>().join(" ").chars().take(300).collect()
                } else {
                    line.split_once('\t').map(|x| x.1.trim().to_string()).unwrap_or_default()
                };
                let ts = unsafe { libc::time(std::ptr::null_mut()) };
                pins.insert(0, Pin { id, file, mime: mime.into(), image: ext != "txt", name, ts });
                save_pins(&pins);
            }
            _ => return,
        }
        self.st.launcher.load_pins();
        self.lrefresh();
    }

    pub fn launcher_del(&mut self, i: usize) {
        let Some(it) = self.st.launcher.items.get(i).cloned() else { return };
        match it.kind {
            Kind::Pin(_) => self.launcher_pin(i),
            Kind::Clip(line) => {
                cliphist(&["delete"], &line);
                self.st.launcher.clip_lines.retain(|l| *l != line);
                self.lrefresh();
            }
            _ => {}
        }
    }

    pub fn launcher_wipe(&mut self) {
        match self.st.launcher.confirm_wipe {
            Some(t) if t.elapsed().as_secs() < 3 => {
                self.st.launcher.confirm_wipe = None;
                cliphist(&["wipe"], "");
                self.st.launcher.clip_lines.clear();
                self.lrefresh();
            }
            _ => {
                self.st.launcher.confirm_wipe = Some(Instant::now());
                self.dirty(&SKind::Modal);
            }
        }
    }

    pub fn launcher_key(&mut self, e: &KeyEvent) {
        let l = &mut self.st.launcher;
        let n = l.items.len();
        match e.keysym {
            Keysym::Escape => self.close_modal(),
            Keysym::Return | Keysym::KP_Enter => {
                let c = l.cur;
                self.launcher_run(c);
            }
            Keysym::Down => l.cur = (l.cur + 1).min(n.saturating_sub(1)),
            Keysym::Up => l.cur = l.cur.saturating_sub(1),
            Keysym::Page_Down => l.cur = (l.cur + 10).min(n.saturating_sub(1)),
            Keysym::Page_Up => l.cur = l.cur.saturating_sub(10),
            Keysym::Tab | Keysym::ISO_Left_Tab => {
                let c = !l.clip;
                l.set_mode(c, &self.st.apps);
                if c {
                    load_clip(self.tx.clone());
                }
            }
            Keysym::Delete if l.clip && l.field.cur == l.field.text.chars().count() => {
                let c = l.cur;
                self.launcher_del(c);
            }
            Keysym::p if self.mods.ctrl && l.clip => {
                let c = l.cur;
                self.launcher_pin(c);
            }
            _ => {
                let mods = self.mods;
                if App::edit_field(&mut self.st.launcher.field, e, &mods) {
                    self.st.launcher.cur = 0;
                    self.st.launcher.scroll = 0;
                    self.st.launcher.refresh(&self.st.apps);
                }
            }
        }
        self.st.launcher.keep_visible();
        self.dirty(&SKind::Modal);
    }
}

pub fn term() -> String {
    for t in ["kitty", "foot", "alacritty", "wezterm", "ghostty", "konsole", "xterm"] {
        if std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|d| d.join(t).exists())).unwrap_or(false) {
            return t.into();
        }
    }
    "xterm".into()
}

fn fnv(d: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in d {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub fn draw(ui: &mut Ui, st: &State, w: i32, h: i32) {
    let t = ui.t;
    let l = &st.launcher;
    let fr = R::new(0, 0, w - 8, h - 8);
    ui.frame(fr, true);
    // шапка
    let head = R::new(1, 1, fr.w - 2, 30);
    ui.header(head, "", if l.clip { "буфер обмена" } else { "программы" }, &l.items.len().to_string());
    let mut bx = head.right() - 6;
    let mut btn = |ui: &mut Ui, label: &str, icon: &str, tone: Tone, sel: bool, act: Act| {
        let bw = if label.is_empty() { 22 } else { ui.w(PX, label) + 12 + if icon.is_empty() { 0 } else { 18 } };
        bx -= bw;
        ui.button(R::new(bx, head.y + 5, bw, 20), label, icon, tone, sel, true, act);
        bx -= 6;
    };
    btn(ui, "", "close", Tone::Danger, false, Act::CloseModal);
    btn(ui, "БУФЕР", "", Tone::Plain, l.clip, Act::LTab(true));
    btn(ui, "ПРОГРАММЫ", "", Tone::Plain, !l.clip, Act::LTab(false));
    if l.clip {
        let wiping = l.confirm_wipe.map(|t| t.elapsed().as_secs() < 3).unwrap_or(false);
        btn(ui, if wiping { "точно? ещё раз" } else { "очистить" }, "trash", Tone::Danger, false, Act::LWipe);
    }

    // поиск
    let fx = 14;
    let field = R::new(fx, head.bottom() + 13, fr.w - 28, 32);
    ui.field(field, &l.field, if l.clip { "ищи в буфере… Enter — скопировать" } else { "напиши… Enter — запустить" }, true, false, Act::Block);

    // список
    let list = R::new(fx, field.bottom() + 8, fr.w - 28, fr.h - field.bottom() - 8 - 34);
    let rows = (list.h / ROW).max(1) as usize;
    let scroll = l.scroll.min(l.items.len().saturating_sub(rows));
    ui.cv.push_clip(list);
    for (vi, it) in l.items.iter().enumerate().skip(scroll).take(rows + 1) {
        let r = R::new(list.x, list.y + (vi - scroll) as i32 * ROW, list.w, ROW);
        let cur = vi == l.cur;
        let hov = ui.hover(r);
        if cur || hov {
            ui.cv.pxbox(R::new(r.x, r.y, r.w, r.h - 2), &BoxStyle::new(if cur { t.selection } else { t.surface_hi }, if cur { t.red } else { t.border }).bevel(cur));
        }
        if cur {
            ui.cv.fill(R::new(r.x, r.y + 2, 2, r.h - 6), t.ember);
        }
        ui.zone(r, Act::LRun(vi), Cur::Pointer);
        let mut x = r.x + 10;
        let cy = r.y + (r.h - 2) / 2;
        match &it.kind {
            Kind::Pin(_) => {
                ui.cv.icon("pin", x, cy - 7, 2, t.red);
                x += 24;
            }
            Kind::Sys(_) => {
                ui.cv.icon(&it.icon, x, cy - 7, 2, t.red);
                x += 26;
            }
            Kind::App(_) => {
                ui.app_icon(&it.icon, x, cy - 8, 16, 1.0);
                x += 26;
            }
            Kind::Clip(_) => {}
        }
        let maxw = if l.clip { list.w - (x - r.x) - 70 } else { list.w - 230 };
        let nw = ui.w(PX, &it.name).min(maxw);
        ui.text_in(PX, R::new(x, r.y, nw, r.h - 2), &it.name, if cur { WHITE } else { t.text });
        if !it.sub.is_empty() {
            let sx = x + nw + 10;
            ui.text_in(PX, R::new(sx, r.y, r.right() - sx - 16, r.h - 2), &it.sub, if cur { t.text } else { t.text_faint });
        }
        // в буфере: закрепить и удалить на выбранной строке
        if l.clip && (cur || hov) {
            let pinned = matches!(it.kind, Kind::Pin(_));
            ui.button(R::new(r.right() - 54, r.y + 3, 24, 22), "", "pin", Tone::Ghost, pinned, true, Act::LPin(vi));
            ui.button(R::new(r.right() - 28, r.y + 3, 24, 22), "", "trash", Tone::Ghost, false, true, Act::LDel(vi));
        }
    }
    ui.cv.pop_clip();
    if l.items.is_empty() {
        ui.text_center(PX, R::new(list.x, list.y + 20, list.w, 20), if l.clip { "буфер пуст" } else { "ничего не нашлось" }, t.text_faint);
    }

    // строка подсказок внизу
    let hint = if l.clip { "Enter — скопировать · Ctrl+P — закрепить · Del — удалить · Tab — программы" } else { "Enter — запустить · ↑↓ — выбор · Tab — буфер · Esc — закрыть" };
    ui.text_in(PX, R::new(fx, fr.h - 30, fr.w - 28, 20), hint, t.text_faint);
}
