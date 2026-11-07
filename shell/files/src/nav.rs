// Файлы: переходы (папка, назад, вперёд, вверх, корзина), выделение (щелчок, Ctrl/Shift,
// стрелки, рамка) и переименование на месте. Состояние — в fm.rs.

use crate::fm::*;
use crate::fsx;
use crate::ui::Field;
use std::collections::HashSet;
use std::path::PathBuf;

impl Fm {
    // ── переходы ───────────────────────────────────────────────────────────
    pub fn go(&mut self, dir: PathBuf) {
        let dir = if dir.as_os_str().is_empty() { crate::paths::home() } else { dir };
        if !dir.is_dir() {
            self.toast(crate::tf!("нет такой папки: {}", "no such folder: {}", "pasta inexistente: {}", dir.display()));
            return;
        }
        let v = self.vm();
        if v.dir == dir && !v.trash && v.search.is_none() {
            return;
        }
        let prev = v.dir.clone();
        let was_special = v.trash || v.search.is_some();
        if !was_special {
            v.back.push(prev.clone());
        }
        v.fwd.clear();
        self.enter(dir, Some(prev));
    }

    /// сменить папку панели без истории; select — что выделить (выход вверх: папку, откуда пришли)
    pub(crate) fn enter(&mut self, dir: PathBuf, select: Option<PathBuf>) {
        let v = self.vm();
        v.dir = dir;
        v.trash = false;
        v.search = None;
        v.all.clear();
        v.items.clear();
        v.sel.clear();
        v.cur = None;
        v.anchor = None;
        v.scroll = 0.0;
        v.target = 0.0;
        v.filter.clear();
        v.rename = None;
        v.fresh.clear();
        v.pending_sel = select.into_iter().collect();
        self.filter_f.clear();
        if self.focus == Focus::Filter {
            self.focus = Focus::Content;
        }
        if let Some(w) = &self.worker {
            w.clear();
        }
        let keep: HashSet<PathBuf> = self.tabs.iter().flat_map(|t| t.panes.iter().map(|p| p.dir.clone())).collect();
        self.thumbs.retain(|(p, _), _| p.parent().map(|d| keep.contains(d)).unwrap_or(false));
        self.thumb_bytes = self.thumbs.values().map(|t| if let Thumb::Ready(i, _) = t { i.px.len() * 4 } else { 0 }).sum();
        self.load_active();
        self.fx.push(Fx::Title(self.title()));
        if let Some(p) = &mut self.pick {
            p.overwrite = None;
        }
    }

    /// место боковой панели: корзина (её папка files) — особым видом, остальное — как папка
    pub fn go_place(&mut self, p: PathBuf) {
        if p == crate::trash::home_trash().join("files") {
            self.go_trash();
        } else {
            self.go(p);
        }
    }

    /// место — в новой вкладке (корзина тоже)
    pub fn open_place_tab(&mut self, p: PathBuf) {
        if p == crate::trash::home_trash().join("files") {
            self.open_tab(crate::paths::home());
            self.go_trash();
        } else if p.is_dir() {
            self.open_tab(p);
        }
    }

    pub fn go_trash(&mut self) {
        let v = self.vm();
        if !v.trash && v.search.is_none() {
            v.back.push(v.dir.clone());
        }
        v.trash = true;
        v.search = None;
        v.all.clear();
        v.items.clear();
        v.sel.clear();
        v.cur = None;
        v.scroll = 0.0;
        v.target = 0.0;
        v.rename = None;
        self.load_active();
        self.fx.push(Fx::Title(self.title()));
    }

    pub fn back(&mut self) {
        let v = self.vm();
        if v.trash || v.search.is_some() {
            let d = v.dir.clone();
            self.enter(d, None);
            return;
        }
        if let Some(p) = v.back.pop() {
            let cur = v.dir.clone();
            v.fwd.push(cur.clone());
            self.enter(p, Some(cur));
        }
    }

    pub fn forward(&mut self) {
        let v = self.vm();
        if let Some(p) = v.fwd.pop() {
            let cur = v.dir.clone();
            v.back.push(cur);
            self.enter(p, None);
        }
    }

    pub fn up(&mut self) {
        let v = self.vm();
        if v.trash || v.search.is_some() {
            let d = v.dir.clone();
            self.enter(d, None);
            return;
        }
        if let Some(p) = v.dir.parent().map(|p| p.to_path_buf()) {
            let cur = v.dir.clone();
            v.back.push(cur.clone());
            v.fwd.clear();
            self.enter(p, Some(cur));
        }
    }

    // ── выделение ──────────────────────────────────────────────────────────
    pub fn click_item(&mut self, i: usize, ctrl: bool, shift: bool) {
        let v = self.vm();
        let Some(path) = v.item(i).map(|e| e.path.clone()) else { return };
        if shift {
            let a = v.anchor.or(v.cur).unwrap_or(i);
            let (lo, hi) = (a.min(i), a.max(i));
            if !ctrl {
                v.sel.clear();
            }
            for k in lo..=hi {
                if let Some(e) = v.item(k) {
                    let p = e.path.clone();
                    v.sel.insert(p);
                }
            }
        } else if ctrl {
            if !v.sel.remove(&path) {
                v.sel.insert(path);
            }
            v.anchor = Some(i);
        } else {
            if !v.sel.contains(&path) {
                v.sel.clear();
                v.sel.insert(path);
            }
            v.anchor = Some(i);
        }
        v.cur = Some(i);
    }

    pub fn select_only(&mut self, i: usize) {
        let v = self.vm();
        if let Some(p) = v.item(i).map(|e| e.path.clone()) {
            v.sel.clear();
            v.sel.insert(p);
            v.cur = Some(i);
            v.anchor = Some(i);
        }
    }

    pub fn move_cursor(&mut self, d: i32, shift: bool, ctrl: bool) {
        let v = self.vm();
        let n = v.items.len() as i32;
        if n == 0 {
            return;
        }
        let c = v.cur.map(|c| c as i32).unwrap_or(-1);
        let to = if c < 0 { if d > 0 { 0 } else { n - 1 } } else { (c + d).clamp(0, n - 1) };
        let to = to as usize;
        if shift {
            let a = v.anchor.unwrap_or(c.max(0) as usize);
            v.anchor = Some(a);
            v.sel.clear();
            for k in a.min(to)..=a.max(to) {
                let p = v.all[v.items[k]].path.clone();
                v.sel.insert(p);
            }
            v.cur = Some(to);
        } else if ctrl {
            v.cur = Some(to);
        } else {
            v.cur = Some(to);
            v.anchor = Some(to);
            v.sel.clear();
            let p = v.all[v.items[to]].path.clone();
            v.sel.insert(p);
        }
        self.reveal_cursor();
    }

    /// прокрутить так, чтобы курсор был виден
    pub fn reveal_cursor(&mut self) {
        let mode = self.mode();
        let v = self.vm();
        let Some(c) = v.cur else { return };
        let g = v.geo.get();
        if g.ch <= 0 {
            return;
        }
        let (y0, y1) = match mode {
            Mode::Grid => {
                let row = c as i32 / g.cols.max(1);
                (row * g.ch, row * g.ch + g.ch)
            }
            Mode::List => (c as i32 * g.ch, c as i32 * g.ch + g.ch),
        };
        let vis = (g.area.h - g.top).max(g.ch);
        if (y0 as f32) < v.target {
            v.target = y0 as f32;
        } else if y1 as f32 > v.target + vis as f32 {
            v.target = (y1 - vis) as f32;
        }
        if v.target < 0.0 {
            v.target = 0.0;
        }
    }

    pub fn band_select(&mut self, ctrl: bool, base: &HashSet<PathBuf>) {
        let mode = self.mode();
        let v = self.vm();
        let Some((x0, y0, x1, y1)) = v.band else { return };
        let (ax, bx) = (x0.min(x1), x0.max(x1));
        let (ay, by) = (y0.min(y1), y0.max(y1));
        let g = v.geo.get();
        let mut sel: HashSet<PathBuf> = if ctrl { base.clone() } else { HashSet::new() };
        for (i, &k) in v.items.iter().enumerate() {
            let r = item_rect(mode, &g, i);
            let hit = (r.x as f64) < bx && (r.right() as f64) > ax && (r.y as f64) < by && (r.bottom() as f64) > ay;
            if hit {
                let p = v.all[k].path.clone();
                if ctrl && base.contains(&p) {
                    sel.remove(&p);
                } else {
                    sel.insert(p);
                }
            }
        }
        v.sel = sel;
    }

    // ── переименование на месте ────────────────────────────────────────────
    pub fn start_rename(&mut self) {
        let v = self.vm();
        if v.trash {
            return;
        }
        let Some(e) = v.cur_entry().cloned().or_else(|| v.selected().first().cloned().cloned()) else { return };
        let mut f = Field::default();
        f.set(&e.name);
        f.cur = fsx::stem_len(&e.name, e.dir);
        v.rename = Some(Rename { path: e.path.clone(), f, fresh: true });
    }

    pub fn finish_rename(&mut self) {
        let v = self.vm();
        let Some(r) = v.rename.take() else { return };
        let name = r.f.text.trim().to_string();
        let old = r.path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        if name.is_empty() || name == old {
            return;
        }
        if name.contains('/') {
            self.toast(crate::i18n::tr("в имени не может быть «/»"));
            return;
        }
        let to = r.path.with_file_name(&name);
        if std::fs::symlink_metadata(&to).is_ok() {
            self.toast(crate::tf!("«{}» уже есть", "“{}” already exists", "“{}” já existe", name));
            return;
        }
        match std::fs::rename(&r.path, &to) {
            Ok(()) => {
                self.undo.push(Undo::Renamed(r.path.clone(), to.clone()));
                self.vm().pending_sel = vec![to];
                let d = self.v().dir.clone();
                self.reload_all_of(&d);
            }
            Err(e) => self.toast(fsx::err_text(&e)),
        }
    }
}
