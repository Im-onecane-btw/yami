// Состояние файлового менеджера: вкладки, в каждой одна или две панели (F3), у панели — папка,
// история, выделение, прокрутка. Здесь же команды (открыть, копировать, в корзину…), которые
// вызываются и из меню, и с клавиатуры. Wayland сюда не заходит: окно (app.rs) передаёт ввод
// и забирает «эффекты» (буфер обмена, перетаскивание, заголовок, закрыть).

use crate::cfg::Cfg;
use crate::fsx::{self, Entry, Sort};
use crate::gfx::Img;
use crate::look::Look;
use crate::mime::{self, Apps};
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
    pub(crate) apps: Option<Apps>,
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
    pub(crate) next_id: u32,
    pub(crate) next_job: u32,
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
    pub(crate) fn view_by_id(&mut self, id: u32) -> Option<&mut View> {
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

}

pub(crate) fn count_text(t: &[PathBuf]) -> String {
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

pub(crate) fn glob_match(pat: &str, s: &str) -> bool {
    let (p, s): (Vec<char>, Vec<char>) = (pat.to_lowercase().chars().collect(), s.to_lowercase().chars().collect());
    pub(crate) fn m(p: &[char], s: &[char]) -> bool {
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
