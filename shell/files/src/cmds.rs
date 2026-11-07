// Команды файлового менеджера (Cmd): что делает пункт меню, клавиша или кнопка с папкой,
// выделением и буфером обмена. Состояние — в Fm (fm.rs); долгая работа — в ops.rs.

use crate::fm::*;
use crate::fsx::Sort;
use crate::ops;
use crate::places::{self};
use crate::ui::Field;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

impl Fm {
    // ── команды ────────────────────────────────────────────────────────────
    pub fn run(&mut self, c: Cmd) {
        let dir = self.v().dir.clone();
        match c {
            Cmd::None => {}
            Cmd::Open => {
                let t = self.v().targets();
                if t.len() == 1 {
                    if let Some(i) = self.v().items.iter().position(|&k| self.v().all[k].path == t[0]) {
                        self.activate(i);
                    }
                } else {
                    let files: Vec<PathBuf> = t.into_iter().filter(|p| !p.is_dir()).collect();
                    self.open_files(&files, None);
                }
            }
            Cmd::OpenWith(id) => {
                let t: Vec<PathBuf> = self.v().targets();
                self.open_files(&t, Some(id));
            }
            Cmd::OpenWithOther => {
                if let Some(e) = self.v().selected().first().cloned().cloned().or_else(|| self.v().cur_entry().cloned()) {
                    self.open_with_dialog(e.path.clone(), e.mime);
                }
            }
            Cmd::OpenTab => {
                for p in self.v().targets().into_iter().filter(|p| p.is_dir()) {
                    let keep = self.tab;
                    self.open_tab(p);
                    self.tab = keep;
                }
                self.watch_dirty = true;
            }
            Cmd::OpenSplit => {
                if let Some(p) = self.v().targets().into_iter().find(|p| p.is_dir()) {
                    if self.tabv().panes.len() < 2 {
                        self.toggle_split();
                    } else {
                        let other = 1 - self.tabv().active;
                        self.set_active(other);
                    }
                    self.go(p);
                }
            }
            Cmd::Terminal => {
                let d = self.v().targets().into_iter().find(|p| p.is_dir()).filter(|_| self.v().sel.len() == 1).unwrap_or(dir);
                let term = self.cfg.terminal.clone();
                let mut argv = vec![term.clone()];
                if term.contains("kitty") {
                    argv.push("--directory".into());
                    argv.push(d.to_string_lossy().to_string());
                    spawn(&argv, None);
                } else {
                    spawn(&argv, Some(&d));
                }
            }
            Cmd::Cut | Cmd::Copy => {
                let t = self.v().targets();
                if t.is_empty() || self.v().trash && c == Cmd::Copy {
                    return;
                }
                let cut = c == Cmd::Cut;
                self.clip = Some((t.clone(), cut));
                self.fx.push(Fx::Clip(t.clone(), cut));
                self.toast(if cut { crate::tf!("вырезано: {}", "cut: {}", "recortado: {}", t.len()) } else { crate::tf!("скопировано: {}", "copied: {}", "copiado: {}", t.len()) });
            }
            Cmd::Paste => {
                if !self.v().trash && self.v().search.is_none() {
                    self.fx.push(Fx::Paste(dir));
                }
            }
            Cmd::PasteInto(d) => self.fx.push(Fx::Paste(d)),
            Cmd::Rename => self.start_rename(),
            Cmd::Trash => {
                if self.v().trash {
                    self.run(Cmd::Delete);
                    return;
                }
                let t = self.v().targets();
                if t.is_empty() {
                    return;
                }
                if self.cfg.confirm_trash {
                    self.dlg = Some((Dlg::Confirm { title: "в корзину".into(), text: count_text(&t), ok: "в корзину".into(), cmd: Cmd::TrashNow }, Instant::now()));
                    return;
                }
                self.job(ops::Kind::Trash, t, PathBuf::new(), String::new());
            }
            Cmd::TrashNow => {
                let t = self.v().targets();
                self.job(ops::Kind::Trash, t, PathBuf::new(), String::new());
            }
            Cmd::Delete => {
                let t = self.v().targets();
                if t.is_empty() {
                    return;
                }
                self.dlg = Some((Dlg::Confirm { title: "удалить навсегда".into(), text: count_text(&t), ok: "удалить".into(), cmd: Cmd::DeleteNow }, Instant::now()));
            }
            Cmd::DeleteNow => {
                let t = self.v().targets();
                self.job(ops::Kind::Delete, t, PathBuf::new(), String::new());
            }
            Cmd::Restore => {
                let t = self.v().targets();
                self.job(ops::Kind::Restore, t, PathBuf::new(), String::new());
            }
            Cmd::EmptyTrash => {
                self.dlg = Some((Dlg::Confirm { title: "очистить корзину".into(), text: crate::tf!("файлов в корзине: {}. Вернуть их будет нельзя.", "{} items in the trash. They cannot be restored.", "{} itens na lixeira. Eles não poderão ser restaurados.", self.trash_n), ok: "очистить".into(), cmd: Cmd::EmptyTrashNow }, Instant::now()));
            }
            Cmd::EmptyTrashNow => {
                let all = crate::trash::all_files();
                self.job(ops::Kind::Delete, all, PathBuf::new(), String::new());
            }
            Cmd::NewFolder => self.dlg = Some((Dlg::Name { kind: NameKind::NewFolder, f: { let mut f = Field::default(); f.set(crate::i18n::tr("новая папка")); f }, err: String::new() }, Instant::now())),
            Cmd::NewFile => self.dlg = Some((Dlg::Name { kind: NameKind::NewFile, f: { let mut f = Field::default(); f.set(crate::i18n::tr("новый файл.txt")); f }, err: String::new() }, Instant::now())),
            Cmd::Compress(ext) => {
                let t = self.v().targets();
                let base = if t.len() == 1 { t[0].file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default() } else { crate::i18n::tr("архив").to_string() };
                let mut f = Field::default();
                f.set(&format!("{base}.{ext}"));
                self.dlg = Some((Dlg::Name { kind: NameKind::Compress, f, err: String::new() }, Instant::now()));
            }
            Cmd::Extract => {
                let t: Vec<PathBuf> = self.v().targets();
                self.job(ops::Kind::Extract, t, dir, String::new());
            }
            Cmd::Link => {
                let t = self.v().targets();
                self.job(ops::Kind::Link, t, dir, String::new());
            }
            Cmd::CopyPath => {
                let t = self.v().targets();
                let s = if t.is_empty() { dir.display().to_string() } else { t.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n") };
                copy_text(&s);
                self.toast(crate::i18n::tr("путь скопирован"));
            }
            Cmd::Props => {
                let mut t = self.v().targets();
                if t.is_empty() {
                    t = vec![dir];
                }
                self.open_props(t);
            }
            Cmd::AddPlace => {
                let t = self.v().targets().into_iter().filter(|p| p.is_dir()).collect::<Vec<_>>();
                let t = if t.is_empty() { vec![dir] } else { t };
                for p in &t {
                    places::add_bookmark(p);
                }
                self.reload_places();
                self.toast(crate::i18n::tr("добавлено в места"));
            }
            Cmd::RemovePlace(p) => {
                places::remove_bookmark(&p);
                let id = p.to_string_lossy().into_owned();
                self.cfg.places_order.retain(|o| *o != id);
                self.cfg.places_hidden.retain(|o| *o != id);
                self.reload_places();
            }
            Cmd::RenamePlace(p) => {
                let mut f = Field::default();
                f.set(&self.side.iter().find(|x| x.path == p && x.user).map(|x| x.name.clone()).unwrap_or_default());
                f.select_all();
                self.dlg = Some((Dlg::Name { kind: NameKind::Bookmark(p), f, err: String::new() }, Instant::now()));
            }
            Cmd::HidePlace(id) => {
                if !self.cfg.places_hidden.contains(&id) {
                    self.cfg.places_hidden.push(id);
                }
                self.reload_places();
            }
            Cmd::ShowPlace(id) => {
                self.cfg.places_hidden.retain(|h| *h != id);
                self.reload_places();
            }
            Cmd::HideDev(k) => {
                if !self.cfg.devs_hidden.contains(&k) {
                    self.cfg.devs_hidden.push(k);
                }
            }
            Cmd::ShowDev(k) => self.cfg.devs_hidden.retain(|h| *h != k),
            Cmd::ResetPlaces => {
                self.cfg.places_order.clear();
                self.cfg.places_hidden.clear();
                self.cfg.devs_hidden.clear();
                self.cfg.collapsed.clear();
                self.side_scroll = 0;
                self.reload_places();
            }
            Cmd::TabAt(p) => {
                // вкладка рядом, фокус остаётся здесь — как у средней кнопки
                let keep = self.tab;
                self.open_place_tab(p);
                self.tab = keep;
                self.watch_dirty = true;
            }
            Cmd::PropsOf(p) => self.open_props(vec![p]),
            Cmd::PaneAt(p) => {
                if self.tabv().panes.len() < 2 {
                    self.toggle_split();
                } else {
                    let other = 1 - self.tabv().active;
                    self.set_active(other);
                }
                self.go_place(p);
            }
            Cmd::SelectAll => {
                let v = self.vm();
                v.sel = v.items.iter().map(|&k| v.all[k].path.clone()).collect();
            }
            Cmd::Invert => {
                let v = self.vm();
                let all: HashSet<PathBuf> = v.items.iter().map(|&k| v.all[k].path.clone()).collect();
                v.sel = all.difference(&v.sel).cloned().collect();
            }
            Cmd::Hidden => {
                self.cfg.hidden = !self.cfg.hidden;
                self.refilter_all();
                self.toast(if self.cfg.hidden { crate::i18n::tr("скрытые файлы видны") } else { crate::i18n::tr("скрытые файлы спрятаны") });
            }
            Cmd::Sort(s) => {
                if self.cfg.sort == s.id() {
                    self.cfg.desc = !self.cfg.desc;
                } else {
                    self.cfg.sort = s.id().into();
                    self.cfg.desc = s == Sort::Time || s == Sort::Size;
                }
                self.refilter_all();
            }
            Cmd::Desc => {
                self.cfg.desc = !self.cfg.desc;
                self.refilter_all();
            }
            Cmd::FoldersFirst => {
                self.cfg.folders_first = !self.cfg.folders_first;
                self.refilter_all();
            }
            Cmd::Mode(m) => {
                self.cfg.view = if m == Mode::List { "list".into() } else { "grid".into() };
                for t in self.tabs.iter_mut() {
                    for v in t.panes.iter_mut() {
                        v.born = Instant::now();
                    }
                }
                self.reveal_cursor();
            }
            Cmd::Zoom(d) => {
                if self.mode() == Mode::List {
                    self.cfg.view = "grid".into();
                } else {
                    self.cfg.zoom = (self.cfg.zoom + d).clamp(0, 4);
                }
            }
            Cmd::Reload => {
                self.reload_places();
                self.refresh_devices();
                self.load_active();
            }
            Cmd::Settings => self.dlg = Some((Dlg::Settings, Instant::now())),
            Cmd::NewWindow => spawn(&[crate::portal::exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "yami-files".into()), dir.to_string_lossy().to_string()], None),
            Cmd::NewTab => {
                self.open_tab(dir);
                self.watch_dirty = true;
            }
            Cmd::CloseTab => {
                let t = self.tab;
                self.close_tab(t);
            }
            Cmd::Split => self.toggle_split(),
            Cmd::Info => self.cfg.info = !self.cfg.info,
            Cmd::Mount(i) => self.mount(i, true),
            Cmd::Unmount(i) => self.mount(i, false),
            Cmd::Connect(uri) => self.server_dlg(&uri, String::new(), None),
            Cmd::Forget(uri) => {
                self.cfg.servers.retain(|s| s != &uri);
                self.refresh_devices();
            }
            Cmd::Undo => self.undo(),
            Cmd::Wallpaper => {
                if let Some(p) = self.v().targets().first() {
                    if crate::look::set_wallpaper(p) {
                        self.toast(crate::i18n::tr("обои поменяются через секунду"));
                    }
                }
            }
            Cmd::Run => {
                if let Some(p) = self.v().targets().first().cloned() {
                    spawn(&[p.to_string_lossy().to_string()], p.parent());
                }
            }
            Cmd::GoTrash => self.go_trash(),
            Cmd::Go(p) => self.go(p),
            Cmd::DropMove(s, d) => self.job(ops::Kind::Move, s, d, String::new()),
            Cmd::DropCopy(s, d) => self.job(ops::Kind::Copy, s, d, String::new()),
            Cmd::DropLink(s, d) => self.job(ops::Kind::Link, s, d, String::new()),
            Cmd::Search => {
                self.focus = Focus::Filter;
            }
            Cmd::PickFilter(i) => {
                if let Some(p) = &mut self.pick {
                    p.filter = i;
                }
                self.refilter_all();
            }
        }
    }

    pub fn job(&mut self, kind: ops::Kind, srcs: Vec<PathBuf>, dst: PathBuf, name: String) {
        if srcs.is_empty() {
            return;
        }
        self.next_job += 1;
        let id = self.next_job;
        let stop = Arc::new(AtomicBool::new(false));
        self.jobs.push(Job { id, kind, done: 0, total: 0, name: String::new(), stop: stop.clone() });
        let job = ops::Job { id, kind, srcs, dst, name, stop };
        match &self.tx {
            Some(tx) => {
                let tx = tx.clone();
                ops::spawn(job, move |e| {
                    let _ = tx.send(Msg::Op(e));
                });
            }
            None => {}
        }
    }

    pub fn on_op(&mut self, e: ops::Ev) {
        match e {
            ops::Ev::Progress { id, done, total, name } => {
                if let Some(j) = self.jobs.iter_mut().find(|j| j.id == id) {
                    j.done = done;
                    j.total = total;
                    j.name = name;
                }
            }
            ops::Ev::Conflict { src, dst, reply } => {
                self.dlg = Some((Dlg::Conflict { src, dst, all: false, reply }, Instant::now()));
            }
            ops::Ev::Done { id, kind, errors, pairs } => {
                self.jobs.retain(|j| j.id != id);
                let made: Vec<PathBuf> = pairs.iter().map(|p| p.1.clone()).filter(|p| !p.as_os_str().is_empty()).collect();
                match kind {
                    ops::Kind::Move if !pairs.is_empty() => self.undo.push(Undo::Moved(pairs.clone())),
                    ops::Kind::Trash if !pairs.is_empty() => {
                        self.undo.push(Undo::Trashed(pairs.clone()));
                        self.toast(crate::tf!("в корзине: {} · ctrl+z — вернуть", "moved to trash: {} · ctrl+z to undo", "movido para a lixeira: {} · ctrl+z para desfazer", pairs.len()));
                    }
                    ops::Kind::Copy | ops::Kind::Link | ops::Kind::Extract | ops::Kind::Compress if !made.is_empty() => self.undo.push(Undo::Created(made.clone())),
                    ops::Kind::Delete if !pairs.is_empty() => self.toast(crate::tf!("удалено: {}", "deleted: {}", "excluído: {}", pairs.len())),
                    ops::Kind::Restore if !pairs.is_empty() => self.toast(crate::tf!("возвращено: {}", "restored: {}", "restaurado: {}", pairs.len())),
                    _ => {}
                }
                if matches!(kind, ops::Kind::Copy | ops::Kind::Move | ops::Kind::Link | ops::Kind::Extract | ops::Kind::Compress) {
                    // вставленное — выделить там, куда положили
                    let dir = self.v().dir.clone();
                    let here: Vec<PathBuf> = made.iter().filter(|p| p.parent() == Some(dir.as_path())).cloned().collect();
                    if !here.is_empty() {
                        self.vm().pending_sel = here;
                    }
                }
                if kind == ops::Kind::Move {
                    if let Some((c, true)) = &self.clip {
                        if c.iter().any(|p| pairs.iter().any(|q| &q.0 == p)) {
                            self.clip = None;
                        }
                    }
                }
                if !errors.is_empty() {
                    self.dlg = Some((Dlg::Errors(errors), Instant::now()));
                }
                let mut dirs: Vec<PathBuf> = vec![];
                for (a, b) in &pairs {
                    for p in [a, b] {
                        if let Some(d) = p.parent() {
                            if !dirs.iter().any(|x| x == d) {
                                dirs.push(d.to_path_buf());
                            }
                        }
                    }
                }
                for d in dirs {
                    self.reload_all_of(&d);
                }
                if self.v().trash || matches!(kind, ops::Kind::Trash | ops::Kind::Delete | ops::Kind::Restore) {
                    self.trash_n = crate::trash::count();
                    let tabs: Vec<(usize, usize)> = self.tabs.iter().enumerate().flat_map(|(ti, t)| t.panes.iter().enumerate().filter(|(_, v)| v.trash).map(move |(pi, _)| (ti, pi))).collect();
                    for (t, p) in tabs {
                        self.load(t, p);
                    }
                }
            }
        }
    }

    pub fn conflict_answer(&mut self, r: ops::Resolve) {
        if let Some((Dlg::Conflict { reply, all, .. }, _)) = self.dlg.take() {
            let _ = reply.send((r, all));
        }
    }

    pub fn undo(&mut self) {
        let Some(u) = self.undo.pop() else {
            self.toast(crate::i18n::tr("нечего отменять"));
            return;
        };
        match u {
            Undo::Moved(pairs) => {
                for (a, b) in pairs.iter().rev() {
                    if std::fs::rename(b, a).is_err() {
                        if let Some(d) = a.parent() {
                            self.job(ops::Kind::Move, vec![b.clone()], d.to_path_buf(), String::new());
                        }
                    }
                }
                self.toast(crate::i18n::tr("перенос отменён"));
            }
            Undo::Trashed(pairs) => {
                let mut n = 0;
                for (orig, t) in pairs {
                    if std::fs::symlink_metadata(&orig).is_err() && std::fs::rename(&t, &orig).is_ok() {
                        crate::trash::forget(&t);
                        n += 1;
                    }
                }
                self.toast(crate::tf!("возвращено из корзины: {}", "restored from trash: {}", "restaurado da lixeira: {}", n));
            }
            Undo::Created(paths) => {
                self.job(ops::Kind::Trash, paths, PathBuf::new(), String::new());
            }
            Undo::Renamed(a, b) => {
                if std::fs::rename(&b, &a).is_ok() {
                    self.vm().pending_sel = vec![a];
                    self.toast(crate::i18n::tr("имя возвращено"));
                }
            }
        }
        let d = self.v().dir.clone();
        self.reload_all_of(&d);
        self.trash_n = crate::trash::count();
    }
}
