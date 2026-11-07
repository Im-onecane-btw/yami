// Открытие: файлы — своей программой (mimeapps.list, список «открыть с помощью»), папки — на
// месте, и выбор файлов для портала (Pick: открыть, сохранить, фильтры).

use crate::fm::*;
use crate::fsx::Entry;
use crate::mime::{self, Kind};
use crate::ui::Field;
use std::path::PathBuf;
use std::time::Instant;

impl Fm {
    // ── фильтр выбора файлов (диалог портала) ──────────────────────────────
    pub fn pick_filter(&self) -> Option<Vec<Pat>> {
        let p = self.pick.as_ref()?;
        p.filters.get(p.filter).map(|f| f.1.clone())
    }

    // ── открыть ────────────────────────────────────────────────────────────
    pub fn activate(&mut self, i: usize) {
        let Some(e) = self.v().item(i).cloned() else { return };
        if e.dir {
            if self.v().trash {
                return;
            }
            self.go(e.path.clone());
            return;
        }
        if self.pick.is_some() {
            self.pick_accept();
            return;
        }
        if self.v().trash {
            self.toast(crate::i18n::tr("сначала верни файл из корзины"));
            return;
        }
        self.open_files(std::slice::from_ref(&e.path), None);
    }

    pub fn open_files(&mut self, files: &[PathBuf], with: Option<String>) {
        if files.is_empty() {
            return;
        }
        // по программам: каждый тип — своей
        let mut groups: Vec<(String, Vec<PathBuf>)> = vec![];
        for f in files {
            let e = Entry::from_path(f);
            let m = e.as_ref().map(|e| e.mime).unwrap_or("application/octet-stream");
            let app = match &with {
                Some(id) => Some(id.clone()),
                None => self.apps().default_for(m).map(|a| a.id.clone()),
            };
            match app {
                Some(id) => match groups.iter_mut().find(|g| g.0 == id) {
                    Some(g) => g.1.push(f.clone()),
                    None => groups.push((id, vec![f.clone()])),
                },
                None => {
                    if mime::kind(m) == Kind::Exec && e.map(|e| e.exec()).unwrap_or(false) {
                        self.toast(crate::i18n::tr("это программа: запусти её из меню → «запустить»"));
                    } else {
                        self.open_with_dialog(f.clone(), m);
                    }
                    return;
                }
            }
        }
        for (id, set) in groups {
            let app = self.apps().all.iter().find(|a| a.id == id).cloned();
            if let Some(a) = app {
                for argv in crate::icons::exec_with(&a.exec, &set) {
                    let argv = if a.terminal { [vec![self.cfg.terminal.clone(), "-e".into()], argv].concat() } else { argv };
                    spawn(&argv, set.first().and_then(|p| p.parent()));
                }
            }
        }
    }

    pub fn open_with_dialog(&mut self, f: PathBuf, m: &'static str) {
        let files = { let t = self.v().targets(); if t.is_empty() { vec![f] } else { t } };
        self.dlg = Some((Dlg::OpenWith { mime: m, files, f: Field::default(), sel: 0, remember: false, scroll: 0 }, Instant::now()));
        self.refresh_open_list();
    }

    /// программы для окна «открыть с помощью»: подходящие сверху, потом остальные
    pub fn open_with_list(&mut self) -> Vec<(usize, bool)> {
        let (m, q) = match &self.dlg {
            Some((Dlg::OpenWith { mime, f, .. }, _)) => (*mime, f.text.to_lowercase()),
            _ => return vec![],
        };
        let apps = self.apps();
        let cand: Vec<String> = apps.candidates(m).iter().map(|a| a.id.clone()).collect();
        let mut out: Vec<(usize, bool)> = vec![];
        for id in &cand {
            if let Some(i) = apps.all.iter().position(|a| &a.id == id) {
                out.push((i, true));
            }
        }
        for (i, a) in apps.all.iter().enumerate() {
            if !cand.contains(&a.id) && (a.takes_files || !q.is_empty()) {
                out.push((i, false));
            }
        }
        if !q.is_empty() {
            out.retain(|(i, _)| {
                let a = &apps.all[*i];
                a.name.to_lowercase().contains(&q) || a.id.to_lowercase().contains(&q) || a.generic.to_lowercase().contains(&q)
            });
        }
        out
    }

    pub fn open_with_run(&mut self, idx: usize) {
        let Some((Dlg::OpenWith { mime, files, remember, .. }, _)) = self.dlg.take() else { return };
        let Some(a) = self.apps().all.get(idx).cloned() else { return };
        if remember {
            self.apps().set_default(mime, &a.id);
            self.toast(crate::tf!("теперь «{}» открывается в {}", "“{}” now opens with {}", "“{}” agora abre com {}", mime::label(mime, ""), a.name));
        }
        self.open_files(&files, Some(a.id));
    }

    // ── выбор файлов (портал) ──────────────────────────────────────────────
    pub fn pick_accept(&mut self) {
        let dir = self.v().dir.clone();
        let sel: Vec<Entry> = self.v().selected().into_iter().cloned().collect();
        let cur = self.v().cur_entry().cloned();
        let Some(p) = &mut self.pick else { return };
        if p.save {
            let name = p.name.text.trim().to_string();
            // выбрана папка — сначала зайти в неё
            if let Some(e) = sel.first().or(cur.as_ref()) {
                if e.dir && sel.len() <= 1 && !p.name_focus {
                    let d = e.path.clone();
                    self.go(d);
                    return;
                }
            }
            if name.is_empty() {
                self.toast(crate::i18n::tr("введи имя файла"));
                return;
            }
            let target = if name.starts_with('/') { PathBuf::from(&name) } else { dir.join(&name) };
            if target.is_dir() {
                self.go(target);
                if let Some(p) = &mut self.pick {
                    p.name.clear();
                }
                return;
            }
            if target.exists() && p.overwrite.as_ref() != Some(&target) {
                p.overwrite = Some(target);
                return;
            }
            p.done = Some(vec![target]);
            self.fx.push(Fx::Close);
            return;
        }
        if p.dirs {
            let chosen: Vec<PathBuf> = sel.iter().filter(|e| e.dir).map(|e| e.path.clone()).collect();
            p.done = Some(if chosen.is_empty() { vec![dir] } else if p.multiple { chosen } else { vec![chosen[0].clone()] });
            self.fx.push(Fx::Close);
            return;
        }
        let files: Vec<PathBuf> = sel.iter().filter(|e| !e.dir).map(|e| e.path.clone()).collect();
        if files.is_empty() {
            if let Some(e) = sel.first().or(cur.as_ref()) {
                if e.dir {
                    let d = e.path.clone();
                    self.go(d);
                    return;
                }
            }
            self.toast(crate::i18n::tr("выбери файл"));
            return;
        }
        p.done = Some(if p.multiple { files } else { vec![files[0].clone()] });
        self.fx.push(Fx::Close);
    }
}
