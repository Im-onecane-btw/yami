// Клавиатура. Сочетания ловятся по физической клавише (evdev), а не по символу: ctrl+c
// работает и в русской раскладке.

use crate::fm::{Cmd, Dlg, Fm, Focus, Fx, Mode};
use crate::ui::Field;
use smithay_client_toolkit::seat::keyboard::Keysym;
use std::path::PathBuf;
use std::time::Instant;

pub struct Key {
    pub sym: Keysym,
    pub raw: u32,
    pub utf8: Option<String>,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

// evdev-коды букв и цифр
pub const K_A: u32 = 30;
pub const K_C: u32 = 46;
pub const K_F: u32 = 33;
pub const K_H: u32 = 35;
pub const K_I: u32 = 23;
pub const K_L: u32 = 38;
pub const K_N: u32 = 49;
pub const K_Q: u32 = 16;
pub const K_T: u32 = 20;
pub const K_V: u32 = 47;
pub const K_W: u32 = 17;
pub const K_X: u32 = 45;
pub const K_Z: u32 = 44;
pub const K_1: u32 = 2;
pub const K_2: u32 = 3;
pub const K_EQ: u32 = 13;
pub const K_MINUS: u32 = 12;
pub const K_COMMA: u32 = 51;

impl Key {
    fn printable(&self) -> Option<&str> {
        match &self.utf8 {
            Some(s) if !s.is_empty() && !s.chars().any(|c| c.is_control()) && !self.ctrl && !self.alt => Some(s),
            _ => None,
        }
    }
}

/// правка однострочного поля; true — клавиша поглощена
pub fn edit(f: &mut Field, k: &Key) -> bool {
    match k.sym {
        Keysym::BackSpace => f.backspace(k.ctrl),
        Keysym::Delete => f.delete(),
        Keysym::Left => f.left(),
        Keysym::Right => f.right(),
        Keysym::Home => f.home(),
        Keysym::End => f.end(),
        _ => {
            if k.ctrl {
                match k.raw {
                    22 => f.clear(), // ctrl+u
                    K_V => {
                        if let Some(t) = crate::ui::paste() {
                            f.insert(&t);
                        }
                    }
                    _ => return false,
                }
                return true;
            }
            match k.printable() {
                Some(s) => f.insert(s),
                None => return false,
            }
        }
    }
    true
}

fn expand(s: &str) -> PathBuf {
    crate::icons::uri_path(s.trim()).unwrap_or_else(|| crate::paths::expand(s))
}

/// дополнение пути по Tab: общий префикс подходящих папок
fn complete(f: &mut Field) {
    let text = f.text.clone();
    let p = expand(&text);
    let (dir, part) = if text.ends_with('/') { (p.clone(), String::new()) } else { (p.parent().map(|d| d.to_path_buf()).unwrap_or_default(), p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()) };
    let Ok(rd) = std::fs::read_dir(&dir) else { return };
    let mut hits: Vec<String> = rd.flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.starts_with(&part) && (!n.starts_with('.') || part.starts_with('.'))).collect();
    hits.sort();
    if hits.is_empty() {
        return;
    }
    let mut common = hits[0].clone();
    for h in &hits[1..] {
        while !h.starts_with(&common) {
            common.pop();
        }
    }
    let full = dir.join(&common);
    let mut s = full.to_string_lossy().to_string();
    if hits.len() == 1 {
        s.push('/');
    }
    let home = crate::paths::home().to_string_lossy().to_string();
    if text.starts_with('~') {
        if let Some(r) = s.strip_prefix(&home) {
            s = format!("~{r}");
        }
    }
    f.set(&s);
}

impl Fm {
    pub fn key(&mut self, k: &Key) {
        if self.menu.is_some() {
            self.menu_key(k);
            return;
        }
        if self.dlg.is_some() {
            self.dlg_key(k);
            return;
        }
        if self.v().rename.is_some() {
            self.rename_key(k);
            return;
        }
        match self.focus {
            Focus::Path => {
                match k.sym {
                    Keysym::Escape => self.focus = Focus::Content,
                    Keysym::Return | Keysym::KP_Enter => {
                        let p = expand(&self.path_f.text);
                        self.focus = Focus::Content;
                        if p.is_file() {
                            if let Some(d) = p.parent() {
                                self.go(d.to_path_buf());
                                self.vm().pending_sel = vec![p];
                            }
                        } else {
                            self.go(p);
                        }
                    }
                    Keysym::Tab => complete(&mut self.path_f),
                    _ => {
                        edit(&mut self.path_f, k);
                    }
                }
                return;
            }
            Focus::Filter => {
                match k.sym {
                    Keysym::Escape => {
                        self.filter_f.clear();
                        self.apply_filter();
                        self.focus = Focus::Content;
                    }
                    Keysym::Return | Keysym::KP_Enter | Keysym::Down => {
                        self.focus = Focus::Content;
                        if !self.v().items.is_empty() {
                            self.select_only(0);
                            if k.sym != Keysym::Down && self.v().items.len() == 1 {
                                self.activate(0);
                            }
                        }
                    }
                    _ => {
                        if edit(&mut self.filter_f, k) {
                            self.apply_filter();
                        }
                    }
                }
                return;
            }
            Focus::Content => {}
        }
        if let Some(p) = &mut self.pick {
            if p.name_focus {
                match k.sym {
                    Keysym::Return | Keysym::KP_Enter => self.pick_accept(),
                    Keysym::Escape => self.pick_cancel(),
                    Keysym::Tab | Keysym::Down => p.name_focus = false,
                    _ => {
                        edit(&mut p.name, k);
                        p.overwrite = None;
                    }
                }
                return;
            }
        }
        self.content_key(k);
    }

    pub fn pick_cancel(&mut self) {
        if let Some(p) = &mut self.pick {
            p.done = None;
        }
        self.fx.push(Fx::Close);
    }

    pub fn apply_filter(&mut self) {
        let q = self.filter_f.text.clone();
        let cfg = self.cfg.clone();
        let pf = self.pick_filter();
        let v = self.vm();
        v.filter = q;
        crate::fm::refilter(v, &cfg, pf.as_ref());
        v.cur = if v.items.is_empty() { None } else { Some(0) };
        v.target = 0.0;
    }

    fn content_key(&mut self, k: &Key) {
        let mode = self.mode();
        let cols = self.v().geo.get().cols.max(1);
        let page = {
            let g = self.v().geo.get();
            ((g.area.h - g.top) / g.ch.max(1)).max(1) * if mode == Mode::Grid { cols } else { 1 }
        };
        let (c, s, a) = (k.ctrl, k.shift, k.alt);
        match k.sym {
            Keysym::Up if a => self.up(),
            Keysym::Left if a => self.back(),
            Keysym::Right if a => self.forward(),
            Keysym::Return | Keysym::KP_Enter if a => self.run(Cmd::Props),
            Keysym::Up => self.move_cursor(if mode == Mode::Grid { -cols } else { -1 }, s, c),
            Keysym::Down => self.move_cursor(if mode == Mode::Grid { cols } else { 1 }, s, c),
            Keysym::Left if mode == Mode::Grid => self.move_cursor(-1, s, c),
            Keysym::Right if mode == Mode::Grid => self.move_cursor(1, s, c),
            Keysym::Page_Up if c => self.tab = (self.tab + self.tabs.len() - 1) % self.tabs.len(),
            Keysym::Page_Down if c => self.tab = (self.tab + 1) % self.tabs.len(),
            Keysym::Page_Up => self.move_cursor(-page, s, false),
            Keysym::Page_Down => self.move_cursor(page, s, false),
            Keysym::Home => self.move_cursor(-100000, s, false),
            Keysym::End => self.move_cursor(100000, s, false),
            Keysym::Return | Keysym::KP_Enter => {
                if self.pick.is_some() && self.v().cur_entry().map(|e| !e.dir).unwrap_or(true) {
                    self.pick_accept();
                } else if self.v().sel.len() > 1 {
                    self.run(Cmd::Open);
                } else if let Some(i) = self.v().cur {
                    self.activate(i);
                }
            }
            Keysym::BackSpace => self.up(),
            Keysym::Escape => {
                if self.pick.is_some() {
                    self.pick_cancel();
                } else if !self.filter_f.text.is_empty() {
                    self.filter_f.clear();
                    self.apply_filter();
                } else {
                    self.vm().sel.clear();
                }
            }
            Keysym::Tab if self.tabv().panes.len() > 1 && !c => {
                let o = 1 - self.tabv().active;
                self.set_active(o);
            }
            Keysym::Tab if c => {
                let n = self.tabs.len();
                self.tab = if s { (self.tab + n - 1) % n } else { (self.tab + 1) % n };
                self.fx.push(Fx::Title(self.title()));
            }
            Keysym::ISO_Left_Tab if c => {
                let n = self.tabs.len();
                self.tab = (self.tab + n - 1) % n;
                self.fx.push(Fx::Title(self.title()));
            }
            Keysym::Tab if self.pick.as_ref().map(|p| p.save).unwrap_or(false) => {
                if let Some(p) = &mut self.pick {
                    p.name_focus = true;
                }
            }
            Keysym::F2 => self.run(Cmd::Rename),
            Keysym::F3 => self.run(Cmd::Split),
            Keysym::F4 => self.run(Cmd::Terminal),
            Keysym::F5 => self.run(Cmd::Reload),
            Keysym::F9 => self.cfg.sidebar = !self.cfg.sidebar,
            Keysym::F11 => self.run(Cmd::Info),
            Keysym::Delete if s => self.run(Cmd::Delete),
            Keysym::Delete => self.run(Cmd::Trash),
            Keysym::Menu => self.fx.push(Fx::Title(String::new())),
            _ if c => match k.raw {
                K_C if a => self.run(Cmd::CopyPath),
                K_C => self.run(Cmd::Copy),
                K_X => self.run(Cmd::Cut),
                K_V => self.run(Cmd::Paste),
                K_A => self.run(Cmd::SelectAll),
                K_I => self.run(Cmd::Invert),
                K_Z => self.run(Cmd::Undo),
                K_H => self.run(Cmd::Hidden),
                K_L => self.edit_path(),
                K_F => self.focus = Focus::Filter,
                K_T => self.run(Cmd::NewTab),
                K_W => self.run(Cmd::CloseTab),
                K_N if s => self.run(Cmd::NewFolder),
                K_N => self.run(Cmd::NewWindow),
                K_Q => self.fx.push(Fx::Close),
                K_1 => self.run(Cmd::Mode(Mode::Grid)),
                K_2 => self.run(Cmd::Mode(Mode::List)),
                K_EQ => self.run(Cmd::Zoom(1)),
                K_MINUS => self.run(Cmd::Zoom(-1)),
                K_COMMA => self.run(Cmd::Settings),
                _ => {}
            },
            _ => {
                // набор с клавиатуры: прыжок к имени, которое так начинается
                if let Some(ch) = k.printable() {
                    let ch = ch.to_string();
                    if self.typeahead.1.elapsed().as_millis() > 900 {
                        self.typeahead.0.clear();
                    }
                    self.typeahead.0.push_str(&ch.to_lowercase());
                    self.typeahead.1 = Instant::now();
                    let q = self.typeahead.0.clone();
                    let v = self.v();
                    let start = v.cur.map(|c| if q.chars().count() == 1 { c + 1 } else { c }).unwrap_or(0);
                    let n = v.items.len();
                    let hit = (0..n).map(|i| (start + i) % n.max(1)).find(|&i| v.item(i).map(|e| e.name.to_lowercase().starts_with(&q)).unwrap_or(false));
                    if let Some(i) = hit {
                        self.select_only(i);
                        self.reveal_cursor();
                    }
                }
            }
        }
    }

    pub fn edit_path(&mut self) {
        let d = self.v().dir.clone();
        let s = crate::fsx::pretty(&d);
        self.path_f.set(&if s.ends_with('/') { s } else { s + "/" });
        self.focus = Focus::Path;
    }

    fn rename_key(&mut self, k: &Key) {
        let v = self.vm();
        let Some(r) = v.rename.as_mut() else { return };
        match k.sym {
            Keysym::Escape => v.rename = None,
            Keysym::Return | Keysym::KP_Enter => self.finish_rename(),
            Keysym::Left | Keysym::Right | Keysym::Home | Keysym::End => {
                r.fresh = false;
                edit(&mut r.f, k);
            }
            Keysym::BackSpace | Keysym::Delete if r.fresh => {
                let tail: String = r.f.text.chars().skip(r.f.cur).collect();
                r.f.text = tail;
                r.f.cur = 0;
                r.fresh = false;
            }
            _ => {
                if r.fresh && k.printable().is_some() {
                    let tail: String = r.f.text.chars().skip(r.f.cur).collect();
                    r.f.text = tail;
                    r.f.cur = 0;
                }
                if edit(&mut r.f, k) {
                    r.fresh = false;
                }
            }
        }
    }

    fn menu_key(&mut self, k: &Key) {
        let Some(m) = self.menu.as_mut() else { return };
        let in_sub = m.sub.is_some() && m.sub_hover.is_some();
        let items: Vec<bool> = if in_sub { m.items[m.sub.unwrap().0].sub.iter().map(|i| i.on && !i.is_sep()).collect() } else { m.items.iter().map(|i| i.on && !i.is_sep()).collect() };
        let step = |cur: Option<usize>, d: i32| -> Option<usize> {
            let n = items.len() as i32;
            let mut i = cur.map(|c| c as i32).unwrap_or(if d > 0 { -1 } else { n });
            for _ in 0..n {
                i = (i + d).rem_euclid(n.max(1));
                if items[i as usize] {
                    return Some(i as usize);
                }
            }
            cur
        };
        match k.sym {
            Keysym::Escape => {
                if in_sub {
                    m.sub_hover = None;
                    m.sub = None;
                } else {
                    self.menu = None;
                }
            }
            Keysym::Down | Keysym::Up => {
                let d = if k.sym == Keysym::Down { 1 } else { -1 };
                if in_sub {
                    m.sub_hover = step(m.sub_hover, d);
                } else {
                    m.hover = step(m.hover, d);
                    m.sub = None;
                }
            }
            Keysym::Right => {
                if let Some(h) = m.hover {
                    if !m.items[h].sub.is_empty() {
                        m.sub = Some((h, Instant::now()));
                        m.sub_hover = Some(0);
                    }
                }
            }
            Keysym::Left => {
                m.sub = None;
                m.sub_hover = None;
            }
            Keysym::Return | Keysym::KP_Enter | Keysym::space => {
                if in_sub {
                    let (si, _) = m.sub.unwrap();
                    let j = m.sub_hover.unwrap();
                    self.menu_pick(si, Some(j));
                } else if let Some(h) = m.hover {
                    self.menu_pick(h, None);
                }
            }
            _ => {}
        }
    }

    /// выбран пункт меню (j — пункт подменю)
    pub fn menu_pick(&mut self, i: usize, j: Option<usize>) {
        let Some(m) = self.menu.as_mut() else { return };
        let Some(it) = m.items.get(i).cloned() else { return };
        let it = match j {
            Some(j) => match it.sub.get(j) {
                Some(s) => s.clone(),
                None => return,
            },
            None => it,
        };
        if !it.sub.is_empty() {
            m.sub = Some((i, Instant::now()));
            m.sub_hover = None;
            return;
        }
        if !it.on {
            return;
        }
        self.menu = None;
        self.run(it.cmd);
    }

    fn dlg_key(&mut self, k: &Key) {
        let Some((d, _)) = self.dlg.as_mut() else { return };
        match k.sym {
            Keysym::Escape => {
                self.dlg_cancel();
                return;
            }
            Keysym::Return | Keysym::KP_Enter => {
                self.dlg_ok();
                return;
            }
            _ => {}
        }
        match d {
            Dlg::Name { f, err, .. } => {
                if edit(f, k) {
                    err.clear();
                }
            }
            Dlg::Server { addr, user, pass, field, err, busy, .. } => {
                if *busy {
                    return;
                }
                match k.sym {
                    Keysym::Tab | Keysym::Down => *field = (*field + 1) % 3,
                    Keysym::ISO_Left_Tab | Keysym::Up => *field = (*field + 2) % 3,
                    _ => {
                        let f = match field {
                            0 => addr,
                            1 => user,
                            _ => pass,
                        };
                        if edit(f, k) {
                            err.clear();
                        }
                    }
                }
            }
            Dlg::Unlock { pass, err, busy, .. } => {
                if !*busy && edit(pass, k) {
                    err.clear();
                }
            }
            Dlg::OpenWith { f, sel, scroll, .. } => match k.sym {
                Keysym::Down => *sel += 1,
                Keysym::Up => *sel = sel.saturating_sub(1),
                _ => {
                    if edit(f, k) {
                        *sel = 0;
                        *scroll = 0;
                    }
                }
            },
            _ => {}
        }
        if let Some((Dlg::OpenWith { .. }, _)) = &self.dlg {
            self.refresh_open_list();
        }
    }
}
