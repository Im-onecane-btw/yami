// Состояние файлового менеджера: вкладки, в каждой одна или две панели (F3), у панели — папка,
// история, выделение, прокрутка. Здесь же команды (открыть, копировать, в корзину…), которые
// вызываются и из меню, и с клавиатуры. Wayland сюда не заходит: окно (app.rs) передаёт ввод
// и забирает «эффекты» (буфер обмена, перетаскивание, заголовок, закрыть).

use crate::cfg::Cfg;
use crate::fsx::{self, Entry, Sort};
use crate::gfx::Img;
use crate::look::Look;
use crate::mime::{self, Apps, Kind};
use crate::ops;
use crate::places::{self, Dev, Place};
use crate::ui::Field;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};
use std::time::Instant;

pub enum Msg {
    Listed { view: u32, gen: u32, res: Result<Vec<Entry>, String> },
    Changed(PathBuf),
    Thumb(PathBuf, i32, Option<Img>),
    Op(ops::Ev),
    Du { gen: u32, v: (u64, u64, u64), done: bool },
    Devices(Vec<Dev>),
    Mounted { err: Option<String>, open: Option<PathBuf> },
    /// итог `gio mount`: адрес и что вышло (и с чем пробовали — для повтора)
    Remote { uri: String, res: crate::remote::Mounted, auth: crate::remote::Auth },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Grid,
    List,
}

/// раскладка панели с прошлого кадра: клавиатуре нужно знать колонки и высоту страницы
#[derive(Clone, Copy, Default, Debug)]
pub struct Geo {
    pub area: crate::gfx::R,
    pub cols: i32,
    pub cw: i32,
    pub ch: i32,
    pub top: i32,
}

pub struct Rename {
    pub path: PathBuf,
    pub f: Field,
    /// ещё ничего не набрано: первая буква заменяет имя без расширения
    pub fresh: bool,
}

pub struct View {
    pub id: u32,
    pub dir: PathBuf,
    pub trash: bool,
    /// поиск по вложенным папкам: запрос
    pub search: Option<String>,
    pub all: Vec<Entry>,
    pub items: Vec<usize>,
    pub sel: HashSet<PathBuf>,
    pub cur: Option<usize>,
    pub anchor: Option<usize>,
    pub scroll: f32,
    pub target: f32,
    pub back: Vec<PathBuf>,
    pub fwd: Vec<PathBuf>,
    pub filter: String,
    pub err: Option<String>,
    pub loading: bool,
    pub gen: u32,
    pub born: Instant,
    pub fresh: HashMap<PathBuf, Instant>,
    pub geo: Cell<Geo>,
    pub rename: Option<Rename>,
    /// резиновая рамка в координатах содержимого (с учётом прокрутки)
    pub band: Option<(f64, f64, f64, f64)>,
    /// выделить после перечитывания (новая папка, вставленное)
    pub pending_sel: Vec<PathBuf>,
    pub free: Option<(u64, u64)>,
}

impl View {
    pub fn new(id: u32, dir: PathBuf) -> View {
        View {
            id,
            dir,
            trash: false,
            search: None,
            all: vec![],
            items: vec![],
            sel: HashSet::new(),
            cur: None,
            anchor: None,
            scroll: 0.0,
            target: 0.0,
            back: vec![],
            fwd: vec![],
            filter: String::new(),
            err: None,
            loading: true,
            gen: 0,
            born: Instant::now(),
            fresh: HashMap::new(),
            geo: Cell::new(Geo::default()),
            rename: None,
            band: None,
            pending_sel: vec![],
            free: None,
        }
    }
    pub fn item(&self, i: usize) -> Option<&Entry> {
        self.items.get(i).map(|&k| &self.all[k])
    }
    pub fn cur_entry(&self) -> Option<&Entry> {
        self.cur.and_then(|i| self.item(i))
    }
    pub fn selected(&self) -> Vec<&Entry> {
        self.items.iter().map(|&k| &self.all[k]).filter(|e| self.sel.contains(&e.path)).collect()
    }
    /// выделенное, а если ничего — то, что под курсором
    pub fn targets(&self) -> Vec<PathBuf> {
        let s: Vec<PathBuf> = self.selected().iter().map(|e| e.path.clone()).collect();
        if !s.is_empty() {
            return s;
        }
        self.cur_entry().map(|e| vec![e.path.clone()]).unwrap_or_default()
    }
    pub fn title(&self) -> String {
        if self.trash {
            return crate::i18n::tr("корзина").to_string();
        }
        if let Some(q) = &self.search {
            return crate::tf!("поиск «{}»", "search “{}”", q);
        }
        if self.dir == crate::paths::home() {
            return crate::i18n::tr("дом").to_string();
        }
        self.dir.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "/".into())
    }
}

pub struct Tab {
    pub panes: Vec<View>,
    pub active: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    None,
    Open,
    OpenWith(String),
    OpenWithOther,
    OpenTab,
    OpenSplit,
    Terminal,
    Cut,
    Copy,
    Paste,
    PasteInto(PathBuf),
    Rename,
    Trash,
    TrashNow,
    Delete,
    DeleteNow,
    Restore,
    EmptyTrash,
    EmptyTrashNow,
    NewFolder,
    NewFile,
    CopyPath,
    Props,
    Extract,
    Compress(&'static str),
    Link,
    AddPlace,
    RemovePlace(PathBuf),
    SelectAll,
    Invert,
    Hidden,
    Sort(Sort),
    Desc,
    FoldersFirst,
    Mode(Mode),
    Zoom(i32),
    Reload,
    Settings,
    NewWindow,
    NewTab,
    CloseTab,
    Split,
    Info,
    Mount(usize),
    Unmount(usize),
    /// окно «подключиться к серверу» (адрес заранее, если есть)
    Connect(String),
    /// убрать сохранённый адрес сервера
    Forget(String),
    Undo,
    Wallpaper,
    Run,
    GoTrash,
    Go(PathBuf),
    /// перетащили: что и куда
    DropMove(Vec<PathBuf>, PathBuf),
    DropCopy(Vec<PathBuf>, PathBuf),
    DropLink(Vec<PathBuf>, PathBuf),
    Search,
    PickFilter(usize),
}

#[derive(Clone, Debug)]
pub struct MItem {
    pub label: String,
    pub icon: &'static str,
    pub key: &'static str,
    pub cmd: Cmd,
    pub on: bool,
    pub check: Option<bool>,
    pub sub: Vec<MItem>,
}

impl MItem {
    pub fn new(label: &str, icon: &'static str, key: &'static str, cmd: Cmd) -> MItem {
        MItem { label: label.to_string(), icon, key, cmd, on: true, check: None, sub: vec![] }
    }
    pub fn sep() -> MItem {
        MItem::new("-", "", "", Cmd::None)
    }
    pub fn off(mut self, off: bool) -> MItem {
        self.on = !off;
        self
    }
    pub fn check(mut self, v: bool) -> MItem {
        self.check = Some(v);
        self
    }
    pub fn sub(label: &str, icon: &'static str, items: Vec<MItem>) -> MItem {
        let mut m = MItem::new(label, icon, "", Cmd::None);
        m.sub = items;
        m
    }
    pub fn is_sep(&self) -> bool {
        self.label == "-"
    }
}

pub struct Menu {
    pub items: Vec<MItem>,
    pub x: i32,
    pub y: i32,
    pub born: Instant,
    /// открытое подменю: номер пункта и когда
    pub sub: Option<(usize, Instant)>,
    pub hover: Option<usize>,
    pub sub_hover: Option<usize>,
    pub title: String,
}

pub enum NameKind {
    NewFolder,
    NewFile,
    Compress,
}

pub enum Dlg {
    Name { kind: NameKind, f: Field, err: String },
    Confirm { title: String, text: String, ok: String, cmd: Cmd },
    Conflict { src: PathBuf, dst: PathBuf, all: bool, reply: mpsc::Sender<(ops::Resolve, bool)> },
    Props { paths: Vec<PathBuf>, gen: u32, du: Option<((u64, u64, u64), bool)>, stop: Arc<AtomicBool> },
    OpenWith { mime: &'static str, files: Vec<PathBuf>, f: Field, sel: usize, remember: bool, scroll: i32 },
    Settings,
    Errors(Vec<String>),
    /// подключение к серверу или телефону: адрес, логин, домен, пароль; `field` — где курсор;
    /// `question` — вопрос сервера (неизвестный ключ), `busy` — ждём gio
    Server { addr: Field, user: Field, pass: Field, field: u8, err: String, question: Option<String>, busy: bool },
}

pub struct Job {
    pub id: u32,
    pub kind: ops::Kind,
    pub done: u64,
    pub total: u64,
    pub name: String,
    pub stop: Arc<AtomicBool>,
}

pub enum Undo {
    Moved(Vec<(PathBuf, PathBuf)>),
    Trashed(Vec<(PathBuf, PathBuf)>),
    Created(Vec<PathBuf>),
    Renamed(PathBuf, PathBuf),
}

#[derive(Clone, Debug)]
pub enum Pat {
    Glob(String),
    Mime(String),
}

pub struct Pick {
    pub save: bool,
    pub dirs: bool,
    pub multiple: bool,
    pub title: String,
    pub accept: String,
    pub name: Field,
    pub filters: Vec<(String, Vec<Pat>)>,
    pub filter: usize,
    pub done: Option<Vec<PathBuf>>,
    pub name_focus: bool,
    /// спросить «заменить?» — путь, который уже есть
    pub overwrite: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Fx {
    Clip(Vec<PathBuf>, bool),
    Paste(PathBuf),
    Title(String),
    Close,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    Content,
    Path,
    Filter,
}

pub enum Thumb {
    Wait,
    Ready(Rc<Img>),
    None,
}

pub struct Fm {
    pub look: Look,
    pub cfg: Cfg,
    pub cfg_saved: Cfg,
    pub tabs: Vec<Tab>,
    pub tab: usize,
    pub places: Vec<Place>,
    pub marks: Vec<Place>,
    pub devs: Vec<Dev>,
    pub trash_n: usize,
    apps: Option<Apps>,
    pub thumbs: HashMap<(PathBuf, i32), Thumb>,
    pub want: RefCell<Vec<crate::thumbs::Req>>,
    pub worker: Option<crate::thumbs::Thumbs>,
    pub menu: Option<Menu>,
    pub dlg: Option<(Dlg, Instant)>,
    pub jobs: Vec<Job>,
    pub undo: Vec<Undo>,
    pub path_f: Field,
    pub filter_f: Field,
    pub focus: Focus,
    /// наш буфер обмена (что скопировали/вырезали из этого окна)
    pub clip: Option<(Vec<PathBuf>, bool)>,
    pub toast: Option<(String, Instant)>,
    pub pick: Option<Pick>,
    pub fx: Vec<Fx>,
    pub tx: Option<calloop::channel::Sender<Msg>>,
    pub born: Instant,
    pub w: i32,
    pub h: i32,
    next_id: u32,
    next_job: u32,
    pub du_gen: u32,
    pub typeahead: (String, Instant),
    pub preview: RefCell<Option<(PathBuf, Vec<String>)>>,
    /// куда сейчас тащат (подсветка папки-цели)
    pub drop_hint: Option<(usize, Option<usize>)>,
    pub watch_dirty: bool,
    pub changed: HashMap<PathBuf, Instant>,
    pub dev_tick: Instant,
    /// опрос устройств идёт в нити
    pub dev_busy: Arc<AtomicBool>,
    pub open_list: RefCell<Vec<(usize, bool)>>,
}

impl Fm {
    pub fn new(look: Look, cfg: Cfg, dirs: Vec<PathBuf>, tx: Option<calloop::channel::Sender<Msg>>) -> Fm {
        let mut fm = Fm {
            look,
            cfg_saved: cfg.clone(),
            cfg,
            tabs: vec![],
            tab: 0,
            places: places::standard(),
            marks: places::bookmarks(),
            devs: vec![],
            trash_n: crate::trash::count(),
            apps: None,
            thumbs: HashMap::new(),
            want: RefCell::new(vec![]),
            worker: None,
            menu: None,
            dlg: None,
            jobs: vec![],
            undo: vec![],
            path_f: Field::default(),
            filter_f: Field::default(),
            focus: Focus::Content,
            clip: None,
            toast: None,
            pick: None,
            fx: vec![],
            tx,
            born: Instant::now(),
            w: 1000,
            h: 640,
            next_id: 1,
            next_job: 1,
            du_gen: 0,
            typeahead: (String::new(), Instant::now()),
            preview: RefCell::new(None),
            drop_hint: None,
            watch_dirty: true,
            changed: HashMap::new(),
            dev_tick: Instant::now(),
            dev_busy: Arc::new(AtomicBool::new(false)),
            open_list: RefCell::new(vec![]),
        };
        let dirs = if dirs.is_empty() { vec![crate::paths::home()] } else { dirs };
        for d in dirs {
            fm.open_tab(d);
        }
        fm.tab = 0;
        if fm.cfg.split && fm.pick.is_none() {
            fm.toggle_split();
        }
        fm.refresh_devices();
        fm
    }

    pub fn apps(&mut self) -> &mut Apps {
        self.apps.get_or_insert_with(Apps::load)
    }
    pub fn apps_ref(&self) -> Option<&Apps> {
        self.apps.as_ref()
    }
    pub fn refresh_open_list(&mut self) {
        let l = self.open_with_list();
        if let Some((Dlg::OpenWith { sel, .. }, _)) = &mut self.dlg {
            if *sel >= l.len() {
                *sel = l.len().saturating_sub(1);
            }
        }
        *self.open_list.borrow_mut() = l;
    }

    pub fn id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    pub fn tabv(&self) -> &Tab {
        &self.tabs[self.tab]
    }
    pub fn v(&self) -> &View {
        let t = &self.tabs[self.tab];
        &t.panes[t.active]
    }
    pub fn vm(&mut self) -> &mut View {
        let t = &mut self.tabs[self.tab];
        &mut t.panes[t.active]
    }
    pub fn pane(&self, p: usize) -> Option<&View> {
        self.tabs[self.tab].panes.get(p)
    }
    pub fn pane_m(&mut self, p: usize) -> Option<&mut View> {
        self.tabs[self.tab].panes.get_mut(p)
    }
    fn view_by_id(&mut self, id: u32) -> Option<&mut View> {
        self.tabs.iter_mut().flat_map(|t| t.panes.iter_mut()).find(|v| v.id == id)
    }

    pub fn mode(&self) -> Mode {
        if self.cfg.view == "list" {
            Mode::List
        } else {
            Mode::Grid
        }
    }

    pub fn toast(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), Instant::now()));
    }

    pub fn title(&self) -> String {
        if let Some(p) = &self.pick {
            return p.title.clone();
        }
        format!("{} — {}", self.v().title(), crate::i18n::tr("файлы"))
    }

    // ── вкладки и панели ───────────────────────────────────────────────────
    pub fn open_tab(&mut self, dir: PathBuf) {
        let id = self.id();
        let v = View::new(id, dir);
        self.tabs.push(Tab { panes: vec![v], active: 0 });
        self.tab = self.tabs.len() - 1;
        self.load_active();
    }

    pub fn close_tab(&mut self, i: usize) {
        if self.tabs.len() <= 1 {
            self.fx.push(Fx::Close);
            return;
        }
        self.tabs.remove(i);
        if self.tab >= self.tabs.len() || self.tab > i {
            self.tab = self.tab.saturating_sub(1).min(self.tabs.len() - 1);
        }
        self.watch_dirty = true;
        self.fx.push(Fx::Title(self.title()));
    }

    pub fn toggle_split(&mut self) {
        let t = &mut self.tabs[self.tab];
        if t.panes.len() > 1 {
            t.panes.truncate(1);
            t.active = 0;
        } else {
            let dir = t.panes[0].dir.clone();
            self.next_id += 1;
            let id = self.next_id;
            let t = &mut self.tabs[self.tab];
            t.panes.push(View::new(id, dir));
            t.active = 1;
            self.load_active();
        }
        self.watch_dirty = true;
    }

    pub fn set_active(&mut self, p: usize) {
        let t = &mut self.tabs[self.tab];
        if p < t.panes.len() && t.active != p {
            t.active = p;
            self.fx.push(Fx::Title(self.title()));
        }
    }

    // ── загрузка папок ─────────────────────────────────────────────────────
    pub fn load_active(&mut self) {
        let (t, p) = (self.tab, self.tabs[self.tab].active);
        self.load(t, p);
    }

    pub fn load(&mut self, t: usize, p: usize) {
        let Some(v) = self.tabs.get_mut(t).and_then(|t| t.panes.get_mut(p)) else { return };
        v.gen += 1;
        v.loading = true;
        v.err = None;
        let (id, gen, dir, trash) = (v.id, v.gen, v.dir.clone(), v.trash);
        v.free = fsx::disk_free(&dir);
        self.watch_dirty = true;
        let work = move || if trash { Ok(crate::trash::list()) } else { fsx::list(&dir) };
        match &self.tx {
            Some(tx) => {
                let tx = tx.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(Msg::Listed { view: id, gen, res: work() });
                });
            }
            None => {
                let res = work();
                self.on_listed(id, gen, res);
            }
        }
    }

    pub fn on_listed(&mut self, id: u32, gen: u32, res: Result<Vec<Entry>, String>) {
        let cfg = self.cfg.clone();
        let pickf = self.pick_filter();
        let Some(v) = self.view_by_id(id) else { return };
        if v.gen != gen {
            return;
        }
        let first = v.loading && v.all.is_empty();
        let old: HashSet<PathBuf> = v.all.iter().map(|e| e.path.clone()).collect();
        let was_loading = v.loading;
        v.loading = false;
        match res {
            Ok(all) => {
                let cur_path = v.cur_entry().map(|e| e.path.clone());
                // новое при перечитывании той же папки — коротко подсветить
                if !was_loading || (!first && !old.is_empty()) {
                    for e in &all {
                        if !old.contains(&e.path) && !old.is_empty() {
                            v.fresh.insert(e.path.clone(), Instant::now());
                        }
                    }
                }
                v.all = all;
                v.err = None;
                refilter(v, &cfg, pickf.as_ref());
                if !v.pending_sel.is_empty() {
                    let ps = std::mem::take(&mut v.pending_sel);
                    let present: Vec<PathBuf> = ps.iter().filter(|p| v.all.iter().any(|e| &e.path == *p)).cloned().collect();
                    if !present.is_empty() {
                        v.sel = present.iter().cloned().collect();
                        v.cur = v.items.iter().position(|&k| v.all[k].path == present[0]);
                        for p in &present {
                            v.fresh.insert(p.clone(), Instant::now());
                        }
                    } else {
                        v.pending_sel = ps;
                    }
                } else if let Some(cp) = cur_path {
                    v.cur = v.items.iter().position(|&k| v.all[k].path == cp).or(v.cur.map(|c| c.min(v.items.len().saturating_sub(1))));
                }
                v.sel.retain(|p| old.contains(p) || true);
                let all_paths: HashSet<&PathBuf> = v.all.iter().map(|e| &e.path).collect();
                v.sel.retain(|p| all_paths.contains(p));
                if v.items.is_empty() {
                    v.cur = None;
                }
            }
            Err(e) => {
                v.all.clear();
                v.items.clear();
                v.err = Some(e);
            }
        }
        if was_loading {
            v.born = Instant::now();
        }
        self.trash_n = crate::trash::count();
    }

    pub fn reload_all_of(&mut self, dir: &Path) {
        let mut hits = vec![];
        for (ti, t) in self.tabs.iter().enumerate() {
            for (pi, v) in t.panes.iter().enumerate() {
                if v.dir == dir && v.search.is_none() || v.trash && crate::trash::is_trashed(&dir.join("x")) {
                    hits.push((ti, pi));
                }
            }
        }
        for (t, p) in hits {
            let v = &mut self.tabs[t].panes[p];
            v.gen += 1;
            let (id, gen, d, trash) = (v.id, v.gen, v.dir.clone(), v.trash);
            v.free = fsx::disk_free(&d);
            let work = move || if trash { Ok(crate::trash::list()) } else { fsx::list(&d) };
            match &self.tx {
                Some(tx) => {
                    let tx = tx.clone();
                    std::thread::spawn(move || {
                        let _ = tx.send(Msg::Listed { view: id, gen, res: work() });
                    });
                }
                None => {
                    let r = work();
                    self.on_listed(id, gen, r);
                }
            }
        }
    }

    pub fn refilter_all(&mut self) {
        let cfg = self.cfg.clone();
        let pf = self.pick_filter();
        for t in self.tabs.iter_mut() {
            for v in t.panes.iter_mut() {
                let cur = v.cur_entry().map(|e| e.path.clone());
                refilter(v, &cfg, pf.as_ref());
                v.cur = cur.and_then(|c| v.items.iter().position(|&k| v.all[k].path == c));
            }
        }
    }

    pub fn watched(&self) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = vec![];
        for t in &self.tabs {
            for p in &t.panes {
                if !p.trash && p.search.is_none() && !v.contains(&p.dir) {
                    v.push(p.dir.clone());
                }
            }
        }
        let tr = crate::trash::home_trash().join("files");
        if !v.contains(&tr) {
            v.push(tr);
        }
        v
    }

    // ── переходы ───────────────────────────────────────────────────────────
    pub fn go(&mut self, dir: PathBuf) {
        let dir = if dir.as_os_str().is_empty() { crate::paths::home() } else { dir };
        if !dir.is_dir() {
            self.toast(crate::tf!("нет такой папки: {}", "no such folder: {}", dir.display()));
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
    fn enter(&mut self, dir: PathBuf, select: Option<PathBuf>) {
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
        self.load_active();
        self.fx.push(Fx::Title(self.title()));
        if let Some(p) = &mut self.pick {
            p.overwrite = None;
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
        self.open_files(&[e.path.clone()], None);
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
            self.toast(crate::tf!("теперь «{}» открывается в {}", "“{}” now opens with {}", mime::label(mime, ""), a.name));
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
                if e.dir && sel.len() <= 1 && (name.is_empty() || p.name_focus == false) && !p.name_focus {
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
                self.toast(if cut { crate::tf!("вырезано: {}", "cut: {}", t.len()) } else { crate::tf!("скопировано: {}", "copied: {}", t.len()) });
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
                self.dlg = Some((Dlg::Confirm { title: "очистить корзину".into(), text: crate::tf!("файлов в корзине: {}. Вернуть их будет нельзя.", "{} items in the trash. They cannot be restored.", self.trash_n), ok: "очистить".into(), cmd: Cmd::EmptyTrashNow }, Instant::now()));
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
                self.marks = places::bookmarks();
                self.toast(crate::i18n::tr("добавлено в места"));
            }
            Cmd::RemovePlace(p) => {
                places::remove_bookmark(&p);
                self.marks = places::bookmarks();
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
                self.places = places::standard();
                self.marks = places::bookmarks();
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
            ops::Ev::Conflict { id, src, dst, reply } => {
                let _ = id;
                self.dlg = Some((Dlg::Conflict { src, dst, all: false, reply }, Instant::now()));
            }
            ops::Ev::Done { id, kind, errors, pairs } => {
                self.jobs.retain(|j| j.id != id);
                let made: Vec<PathBuf> = pairs.iter().map(|p| p.1.clone()).filter(|p| !p.as_os_str().is_empty()).collect();
                match kind {
                    ops::Kind::Move if !pairs.is_empty() => self.undo.push(Undo::Moved(pairs.clone())),
                    ops::Kind::Trash if !pairs.is_empty() => {
                        self.undo.push(Undo::Trashed(pairs.clone()));
                        self.toast(crate::tf!("в корзине: {} · ctrl+z — вернуть", "moved to trash: {} · ctrl+z to undo", pairs.len()));
                    }
                    ops::Kind::Copy | ops::Kind::Link | ops::Kind::Extract | ops::Kind::Compress if !made.is_empty() => self.undo.push(Undo::Created(made.clone())),
                    ops::Kind::Delete if !pairs.is_empty() => self.toast(crate::tf!("удалено: {}", "deleted: {}", pairs.len())),
                    ops::Kind::Restore if !pairs.is_empty() => self.toast(crate::tf!("возвращено: {}", "restored: {}", pairs.len())),
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
                self.toast(crate::tf!("возвращено из корзины: {}", "restored from trash: {}", n));
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
            self.toast(crate::tf!("«{}» уже есть", "“{}” already exists", name));
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

    // ── окна-вопросы ───────────────────────────────────────────────────────
    pub fn dlg_ok(&mut self) {
        if matches!(self.dlg, Some((Dlg::Server { .. }, _))) {
            return self.server_ok();
        }
        let dir = self.v().dir.clone();
        let Some((d, born)) = self.dlg.take() else { return };
        match d {
            Dlg::Name { kind, f, .. } => {
                let name = f.text.trim().to_string();
                let err = if name.is_empty() {
                    Some("имя пустое")
                } else if name.contains('/') {
                    Some("в имени не может быть «/»")
                } else if std::fs::symlink_metadata(dir.join(&name)).is_ok() {
                    Some("такое имя уже есть")
                } else {
                    None
                };
                if let Some(e) = err {
                    self.dlg = Some((Dlg::Name { kind, f, err: e.into() }, born));
                    return;
                }
                let p = dir.join(&name);
                let r = match kind {
                    NameKind::NewFolder => std::fs::create_dir(&p),
                    NameKind::NewFile => std::fs::OpenOptions::new().write(true).create_new(true).open(&p).map(|_| ()),
                    NameKind::Compress => {
                        let t = self.v().targets();
                        self.job(ops::Kind::Compress, t, dir.clone(), name);
                        return;
                    }
                };
                match r {
                    Ok(()) => {
                        self.undo.push(Undo::Created(vec![p.clone()]));
                        self.vm().pending_sel = vec![p];
                        self.reload_all_of(&dir);
                    }
                    Err(e) => self.dlg = Some((Dlg::Name { kind, f, err: fsx::err_text(&e) }, born)),
                }
            }
            Dlg::Confirm { cmd, .. } => self.run(cmd),
            Dlg::OpenWith { .. } => {
                self.dlg = Some((d, born));
                let list = self.open_with_list();
                let sel = match &self.dlg {
                    Some((Dlg::OpenWith { sel, .. }, _)) => *sel,
                    _ => 0,
                };
                if let Some((i, _)) = list.get(sel) {
                    self.open_with_run(*i);
                }
            }
            Dlg::Conflict { reply, all, .. } => {
                let _ = reply.send((ops::Resolve::Rename, all));
            }
            Dlg::Props { stop, .. } => stop.store(true, std::sync::atomic::Ordering::Relaxed),
            Dlg::Settings | Dlg::Errors(_) | Dlg::Server { .. } => {}
        }
    }

    pub fn dlg_cancel(&mut self) {
        match self.dlg.take() {
            Some((Dlg::Conflict { reply, .. }, _)) => {
                let _ = reply.send((ops::Resolve::Cancel, false));
            }
            Some((Dlg::Props { stop, .. }, _)) => stop.store(true, std::sync::atomic::Ordering::Relaxed),
            Some((Dlg::Server { mut pass, .. }, _)) => pass.wipe(),
            _ => {}
        }
    }

    pub fn open_props(&mut self, paths: Vec<PathBuf>) {
        self.du_gen += 1;
        let gen = self.du_gen;
        let stop = Arc::new(AtomicBool::new(false));
        let has_dir = paths.iter().any(|p| p.is_dir() && !p.is_symlink());
        if has_dir {
            if let Some(tx) = &self.tx {
                let tx = tx.clone();
                let ps = paths.clone();
                let st = stop.clone();
                std::thread::spawn(move || {
                    let mut tot = (0u64, 0u64, 0u64);
                    for p in &ps {
                        if p.is_dir() {
                            let (b, f, d) = fsx::du(p, &st);
                            tot = (tot.0 + b, tot.1 + f, tot.2 + d + 1);
                        } else {
                            tot.0 += std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                            tot.1 += 1;
                        }
                        let _ = tx.send(Msg::Du { gen, v: tot, done: false });
                    }
                    let _ = tx.send(Msg::Du { gen, v: tot, done: true });
                });
            }
        }
        let du = if has_dir { None } else { Some(((paths.iter().map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)).sum(), paths.len() as u64, 0), true)) };
        self.dlg = Some((Dlg::Props { paths, gen, du, stop }, Instant::now()));
    }

    pub fn on_du(&mut self, g: u32, v: (u64, u64, u64), done: bool) {
        if let Some((Dlg::Props { gen, du, .. }, _)) = &mut self.dlg {
            if *gen == g {
                *du = Some((v, done));
            }
        }
    }

    pub fn chmod(&mut self, bit: u32) {
        let Some((Dlg::Props { paths, .. }, _)) = &self.dlg else { return };
        for p in paths.clone() {
            if let Ok(m) = std::fs::metadata(&p) {
                use std::os::unix::fs::PermissionsExt;
                let mode = m.permissions().mode() ^ bit;
                if let Err(e) = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)) {
                    self.toast(fsx::err_text(&e));
                }
            }
        }
        let d = self.v().dir.clone();
        self.reload_all_of(&d);
    }

    // ── устройства ─────────────────────────────────────────────────────────
    pub fn refresh_devices(&mut self) {
        self.dev_tick = Instant::now();
        if !self.cfg.devices && !self.cfg.network {
            self.devs.clear();
            return;
        }
        let (disks, net, servers) = (self.cfg.devices, self.cfg.network, self.cfg.servers.clone());
        match &self.tx {
            Some(tx) => {
                // прошлый опрос ещё идёт (gio ждёт телефон) — второй не запускаем
                if self.dev_busy.swap(true, std::sync::atomic::Ordering::AcqRel) {
                    return;
                }
                let (tx, busy) = (tx.clone(), self.dev_busy.clone());
                std::thread::spawn(move || {
                    let _ = tx.send(Msg::Devices(places::devices(disks, net, &servers)));
                    busy.store(false, std::sync::atomic::Ordering::Release);
                });
            }
            None => self.devs = places::devices(disks, net, &servers),
        }
    }

    /// окно подключения: адрес, логин и пароль; ошибка или вопрос сервера — сверху
    pub fn server_dlg(&mut self, uri: &str, err: String, question: Option<String>) {
        let mut addr = Field::default();
        addr.set(uri);
        let mut user = Field::default();
        // логин из адреса (sftp://me@host) — в поле
        if let Some(u) = uri.split_once("://").and_then(|(_, r)| r.split('/').next()).and_then(|a| a.rsplit_once('@')).map(|x| x.0.split(':').next().unwrap_or("").to_string()) {
            user.set(&u);
        }
        let field = if uri.is_empty() { 0 } else if user.text.is_empty() { 1 } else { 2 };
        self.dlg = Some((Dlg::Server { addr, user, pass: Field::default(), field, err, question, busy: false }, Instant::now()));
    }

    /// подключить адрес gvfs в нити; итог придёт в `on_remote`
    fn remote_mount(&mut self, uri: String, auth: crate::remote::Auth) {
        let Some(tx) = self.tx.clone() else { return };
        std::thread::spawn(move || {
            let res = crate::remote::mount(&uri, &auth);
            let _ = tx.send(Msg::Remote { uri, res, auth });
        });
    }

    pub fn on_remote(&mut self, uri: String, res: crate::remote::Mounted, auth: crate::remote::Auth) {
        use crate::remote::Mounted as M;
        let was_dlg = matches!(self.dlg, Some((Dlg::Server { .. }, _)));
        match res {
            M::Ok(p) => {
                if was_dlg {
                    self.dlg = None;
                }
                // сетевой адрес запоминаем (без пароля): в следующий раз — одним щелчком
                if crate::remote::known_scheme(&uri) {
                    let keep = crate::remote::strip_secret(&uri);
                    if !self.cfg.servers.contains(&keep) {
                        self.cfg.servers.push(keep);
                    }
                }
                match p {
                    Some(p) => self.go(p),
                    None => self.toast(crate::i18n::tr("подключено")),
                }
            }
            M::NeedAuth { retry } => {
                let e = if retry { "неверный логин или пароль" } else { "нужны логин и пароль" };
                match &mut self.dlg {
                    Some((Dlg::Server { err, busy, pass, field, .. }, _)) => {
                        *err = crate::i18n::tr(e).to_string();
                        *busy = false;
                        pass.wipe();
                        *field = 2;
                    }
                    _ => self.server_dlg(&uri, crate::i18n::tr(e).to_string(), None),
                }
            }
            M::Question(q) => match &mut self.dlg {
                Some((Dlg::Server { question, busy, .. }, _)) => {
                    *question = Some(q);
                    *busy = false;
                }
                _ => {
                    self.server_dlg(&uri, String::new(), Some(q));
                    // ответ на вопрос — с тем же логином, что пробовали
                    if let Some((Dlg::Server { user, .. }, _)) = &mut self.dlg {
                        if !auth.user.is_empty() {
                            user.set(&auth.user);
                        }
                    }
                }
            },
            M::Err(e) => {
                // телефон заблокирован или не в режиме передачи файлов
                let scheme = uri.split("://").next().unwrap_or("");
                let e = if scheme == "mtp" || scheme == "gphoto2" {
                    format!("{e} · {}", crate::i18n::tr("разблокируй телефон и выбери «передача файлов»"))
                } else if e.contains("not mountable") || e.contains("not supported") {
                    // у gvfs нет бэкенда для этой схемы: smb, nfs, afp — отдельные пакеты
                    crate::tf!("{} · нужен пакет gvfs-{}", "{} · needs the gvfs-{} package", e, if scheme == "smb" || scheme == "nfs" || scheme == "afp" { scheme } else { "backends" })
                } else {
                    e
                };
                match &mut self.dlg {
                    Some((Dlg::Server { err, busy, .. }, _)) => {
                        *err = e;
                        *busy = false;
                    }
                    _ => self.toast(e),
                }
            }
        }
        self.refresh_devices();
    }

    /// «подключить» в окне сервера
    fn server_ok(&mut self) {
        let Some((Dlg::Server { addr, user, pass, question, busy, err, .. }, _)) = &mut self.dlg else { return };
        if *busy {
            return;
        }
        let (uri, secret) = crate::remote::split_secret(&crate::remote::normalize(&addr.text));
        if uri.is_empty() {
            *err = crate::i18n::tr("введи адрес: smb://, sftp://, ftp://, davs://, nfs://").to_string();
            return;
        }
        // логин вида «ДОМЕН\имя» — домен отдельно
        let (domain, name) = match user.text.split_once('\\') {
            Some((d, n)) => (d.to_string(), n.to_string()),
            None => (String::new(), user.text.trim().to_string()),
        };
        let pass = if pass.text.is_empty() { secret.unwrap_or_default() } else { pass.text.clone() };
        let auth = crate::remote::Auth { user: name, domain, pass, accept: question.is_some() };
        *busy = true;
        err.clear();
        self.remote_mount(uri, auth);
    }

    fn mount(&mut self, i: usize, on: bool) {
        let Some(d) = self.devs.get(i).cloned() else { return };
        let Some(tx) = self.tx.clone() else { return };
        if d.kind != places::DevKind::Disk {
            return self.mount_remote(d, on, tx);
        }
        self.toast(if on { crate::tf!("подключаю {}…", "mounting {}…", d.title()) } else { crate::tf!("отключаю {}…", "unmounting {}…", d.title()) });
        // уходим из папки на отключаемом диске
        if !on {
            if let Some(m) = &d.mount {
                if self.v().dir.starts_with(m) {
                    self.go(crate::paths::home());
                }
            }
        }
        let (disks, net, servers) = (self.cfg.devices, self.cfg.network, self.cfg.servers.clone());
        std::thread::spawn(move || {
            let out = std::process::Command::new("udisksctl").args([if on { "mount" } else { "unmount" }, "-b", &d.dev, "--no-user-interaction"]).output();
            let (err, open) = match out {
                Ok(o) if o.status.success() => {
                    if on {
                        // «Mounted /dev/sdb1 at /run/media/user/LABEL»
                        let s = String::from_utf8_lossy(&o.stdout).to_string();
                        let p = s.split(" at ").nth(1).map(|x| PathBuf::from(x.trim().trim_end_matches('.')));
                        (None, p)
                    } else {
                        if d.removable {
                            let disk = d.dev.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end_matches('p').to_string();
                            let _ = std::process::Command::new("udisksctl").args(["power-off", "-b", &disk, "--no-user-interaction"]).output();
                        }
                        (None, None)
                    }
                }
                Ok(o) => (Some(String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("").to_string()), None),
                Err(_) => (Some(crate::i18n::tr("нет udisksctl (пакет udisks2)").to_string()), None),
            };
            let _ = tx.send(Msg::Mounted { err, open });
            let _ = tx.send(Msg::Devices(places::devices(disks, net, &servers)));
        });
    }

    /// телефоны, камеры, сеть: gvfs или сетевые монтирования ядра
    fn mount_remote(&mut self, d: Dev, on: bool, tx: calloop::channel::Sender<Msg>) {
        if on {
            if d.uri.is_empty() {
                // телефон виден по USB, но у gvfs нет бэкенда MTP
                self.toast(crate::i18n::tr("для телефонов нужен пакет gvfs-mtp (и gvfs-gphoto2 для камер)"));
                return;
            }
            self.toast(crate::tf!("подключаю {}…", "mounting {}…", d.title()));
            self.remote_mount(d.uri.clone(), crate::remote::Auth::default());
            return;
        }
        if let Some(m) = &d.mount {
            if self.v().dir.starts_with(m) {
                self.go(crate::paths::home());
            }
        }
        self.toast(crate::tf!("отключаю {}…", "unmounting {}…", d.title()));
        let (disks, net, servers) = (self.cfg.devices, self.cfg.network, self.cfg.servers.clone());
        std::thread::spawn(move || {
            let err = crate::remote::unmount(&d).err();
            let _ = tx.send(Msg::Mounted { err, open: None });
            let _ = tx.send(Msg::Devices(places::devices(disks, net, &servers)));
        });
    }

    /// сохранить настройки, если поменялись
    pub fn save_cfg(&mut self) {
        if self.cfg != self.cfg_saved && self.pick.is_none() {
            self.cfg.save();
            self.cfg_saved = self.cfg.clone();
        }
    }
}

fn count_text(t: &[PathBuf]) -> String {
    if t.len() == 1 {
        format!("«{}»", t[0].file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default())
    } else {
        crate::tf!("объектов: {}", "{} items", t.len())
    }
}

/// отсортировать и отфильтровать: скрытые, строка фильтра, фильтр диалога
pub fn refilter(v: &mut View, cfg: &Cfg, pick: Option<&Vec<Pat>>) {
    fsx::sort(&mut v.all, Sort::from_id(&cfg.sort), cfg.desc, cfg.folders_first);
    let q = v.filter.to_lowercase();
    v.items = (0..v.all.len())
        .filter(|&i| {
            let e = &v.all[i];
            (cfg.hidden || !e.hidden || v.trash) && (q.is_empty() || e.name.to_lowercase().contains(&q)) && (e.dir || pick.map(|p| matches(e, p)).unwrap_or(true))
        })
        .collect();
}

fn glob_match(pat: &str, s: &str) -> bool {
    let (p, s): (Vec<char>, Vec<char>) = (pat.to_lowercase().chars().collect(), s.to_lowercase().chars().collect());
    fn m(p: &[char], s: &[char]) -> bool {
        match p.first() {
            None => s.is_empty(),
            Some('*') => (0..=s.len()).any(|i| m(&p[1..], &s[i..])),
            Some('?') => !s.is_empty() && m(&p[1..], &s[1..]),
            Some(c) => s.first() == Some(c) && m(&p[1..], &s[1..]),
        }
    }
    m(&p, &s)
}

pub fn matches(e: &Entry, pats: &[Pat]) -> bool {
    if pats.is_empty() {
        return true;
    }
    pats.iter().any(|p| match p {
        Pat::Glob(g) => glob_match(g, &e.name),
        Pat::Mime(m) => {
            if let Some(top) = m.strip_suffix("/*") {
                e.mime.starts_with(&format!("{top}/"))
            } else {
                mime::ancestry(e.mime).iter().any(|a| a == m)
            }
        }
    })
}

/// прямоугольник элемента в координатах содержимого (без прокрутки)
pub fn item_rect(mode: Mode, g: &Geo, i: usize) -> crate::gfx::R {
    let i = i as i32;
    match mode {
        Mode::Grid => {
            let c = g.cols.max(1);
            crate::gfx::R::new(g.area.x + 8 + (i % c) * g.cw, g.area.y + g.top + (i / c) * g.ch, g.cw, g.ch)
        }
        Mode::List => crate::gfx::R::new(g.area.x, g.area.y + g.top + i * g.ch, g.area.w, g.ch),
    }
}

pub fn spawn(argv: &[String], cwd: Option<&Path>) {
    if argv.is_empty() {
        return;
    }
    let mut c = std::process::Command::new("setsid");
    c.arg("-f").args(argv).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    if let Some(d) = cwd {
        c.current_dir(d);
    }
    let _ = c.status();
}

pub fn copy_text(s: &str) {
    use std::io::Write;
    if let Ok(mut ch) = std::process::Command::new("wl-copy").stdin(std::process::Stdio::piped()).spawn() {
        if let Some(i) = ch.stdin.as_mut() {
            let _ = i.write_all(s.as_bytes());
        }
        let _ = ch.wait();
    }
}
