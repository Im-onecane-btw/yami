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
    /// папка прочитана; свободное место её диска меряется там же, в нити (statvfs сетевой папки
    /// может висеть — окно не должно)
    Listed { view: u32, gen: u32, res: Result<Vec<Entry>, String>, free: Option<(u64, u64)> },
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
            return crate::tf!("поиск «{}»", "search “{}”", "pesquisar “{}”", q);
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
    /// своё имя закладки
    RenamePlace(PathBuf),
    /// скрыть место (ключ) или устройство (Dev::key) из боковой панели и показать снова
    HidePlace(String),
    ShowPlace(String),
    HideDev(String),
    ShowDev(String),
    /// места — как после установки: порядок, скрытое, свёрнутое
    ResetPlaces,
    /// папка — в новой вкладке, в соседней панели (из меню мест и устройств)
    TabAt(PathBuf),
    PaneAt(PathBuf),
    /// свойства папки места или устройства (не выделенного)
    PropsOf(PathBuf),
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
    /// своё имя закладки боковой панели
    Bookmark(PathBuf),
}

pub enum Dlg {
    Name { kind: NameKind, f: Field, err: String },
    Confirm { title: String, text: String, ok: String, cmd: Cmd },
    Conflict { src: PathBuf, dst: PathBuf, all: bool, reply: mpsc::Sender<(ops::Resolve, bool)> },
    /// `free` — место на диске, замерено при открытии (не в каждом кадре)
    Props { paths: Vec<PathBuf>, gen: u32, du: Option<((u64, u64, u64), bool)>, stop: Arc<AtomicBool>, free: Option<(u64, u64)> },
    OpenWith { mime: &'static str, files: Vec<PathBuf>, f: Field, sel: usize, remember: bool, scroll: i32 },
    Settings,
    Errors(Vec<String>),
    /// подключение к серверу или телефону: адрес, логин, домен, пароль; `field` — где курсор;
    /// `question` — вопрос сервера (неизвестный ключ), `busy` — ждём gio
    Server { addr: Field, user: Field, pass: Field, field: u8, err: String, question: Option<String>, busy: bool },
    /// пароль зашифрованного диска (LUKS, BitLocker): раздел, его имя для заголовка
    Unlock { dev: String, name: String, pass: Field, err: String, busy: bool },
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
    /// картинка и номер кадра, в котором её показали последний раз
    Ready(Rc<Img>, Cell<u32>),
    None,
}

/// миниатюры в памяти — не больше этого: папка на тысячи фото иначе копила бы их все, пока открыта.
/// Вытесняются давно не показанные; с диска (кеш миниатюр) они возвращаются за доли миллисекунды
const THUMB_BUDGET: usize = 32 << 20;

pub struct Fm {
    pub look: Look,
    pub cfg: Cfg,
    pub cfg_saved: Cfg,
    pub tabs: Vec<Tab>,
    pub tab: usize,
    /// места боковой панели в своём порядке (стандартные, закладки, корзина) и скрытые из них
    pub side: Vec<Place>,
    pub side_hidden: Vec<Place>,
    /// прокрутка боковой панели и высота её содержимого с прошлого кадра
    pub side_scroll: i32,
    pub side_h: Cell<i32>,
    /// место, которое тянут мышью (номер в `side`), и где сейчас курсор по y
    pub side_drag: Option<(usize, f64)>,
    pub devs: Vec<Dev>,
    pub trash_n: usize,
    pub(crate) apps: Option<Apps>,
    pub thumbs: HashMap<(PathBuf, i32), Thumb>,
    /// сколько байт пикселей в готовых миниатюрах
    pub thumb_bytes: usize,
    /// номер кадра (для вытеснения миниатюр)
    pub frame: u32,
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
    /// отпечаток того, что видно без запуска программ (devices::sig): пока он тот же,
    /// lsblk и gio не запускаются; раз в минуту — всё равно
    pub dev_sig: u64,
    pub dev_full: Instant,
    /// опрос устройств идёт в нити
    pub dev_busy: Arc<AtomicBool>,
    pub open_list: RefCell<Vec<(usize, bool)>>,
}

impl Fm {
    /// готовая миниатюра (или «не вышло») из нити
    pub fn put_thumb(&mut self, key: (PathBuf, i32), img: Option<Img>) {
        let t = match img {
            Some(i) => {
                self.thumb_bytes += i.px.len() * 4;
                Thumb::Ready(Rc::new(i), Cell::new(self.frame))
            }
            None => Thumb::None,
        };
        if let Some(Thumb::Ready(old, _)) = self.thumbs.insert(key, t) {
            self.thumb_bytes = self.thumb_bytes.saturating_sub(old.px.len() * 4);
        }
    }

    /// сверх бюджета: выкинуть давно не показанные, пока не останется половина
    pub fn trim_thumbs(&mut self) {
        if self.thumb_bytes <= THUMB_BUDGET {
            return;
        }
        let now = self.frame;
        let mut old: Vec<(u32, (PathBuf, i32))> = self.thumbs.iter().filter_map(|(k, t)| if let Thumb::Ready(_, u) = t { (u.get() != now).then(|| (u.get(), k.clone())) } else { None }).collect();
        old.sort_unstable_by_key(|o| o.0);
        for (_, k) in old {
            if self.thumb_bytes <= THUMB_BUDGET / 2 {
                break;
            }
            if let Some(Thumb::Ready(i, _)) = self.thumbs.remove(&k) {
                self.thumb_bytes = self.thumb_bytes.saturating_sub(i.px.len() * 4);
            }
        }
    }

    pub fn new(look: Look, cfg: Cfg, dirs: Vec<PathBuf>, tx: Option<calloop::channel::Sender<Msg>>) -> Fm {
        let mut fm = Fm {
            look,
            cfg_saved: cfg.clone(),
            cfg,
            tabs: vec![],
            tab: 0,
            side: vec![],
            side_hidden: vec![],
            side_scroll: 0,
            side_h: Cell::new(0),
            side_drag: None,
            devs: vec![],
            trash_n: crate::trash::count(),
            apps: None,
            thumbs: HashMap::new(),
            thumb_bytes: 0,
            frame: 0,
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
            dev_sig: 0,
            dev_full: Instant::now(),
            dev_busy: Arc::new(AtomicBool::new(false)),
            open_list: RefCell::new(vec![]),
        };
        fm.reload_places();
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

    /// места боковой панели заново: после закладок, скрытия, перетаскивания, «обновить»
    pub fn reload_places(&mut self) {
        let (shown, hidden) = places::sidebar(&self.cfg.places_order, &self.cfg.places_hidden);
        self.side = shown;
        self.side_hidden = hidden;
        self.side_drag = None;
    }

    /// перенести место `from` на позицию `to` (номера в `side`) и запомнить весь порядок
    pub fn move_place(&mut self, from: usize, to: usize) {
        if from >= self.side.len() {
            return;
        }
        let p = self.side.remove(from);
        let to = to.min(self.side.len());
        self.side.insert(to, p);
        // скрытые сохраняют своё место в порядке: в конце, чтобы «показать» вернуло их вниз
        let mut order: Vec<String> = self.side.iter().map(|p| p.id.clone()).collect();
        order.extend(self.side_hidden.iter().map(|p| p.id.clone()));
        self.cfg.places_order = order;
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
        self.watch_dirty = true;
        let work = move || (if trash { Ok(crate::trash::list()) } else { fsx::list(&dir) }, fsx::disk_free(&dir));
        match &self.tx {
            Some(tx) => {
                let tx = tx.clone();
                std::thread::spawn(move || {
                    let (res, free) = work();
                    let _ = tx.send(Msg::Listed { view: id, gen, res, free });
                });
            }
            None => {
                let (res, free) = work();
                self.on_listed(id, gen, res, free);
            }
        }
    }

    pub fn on_listed(&mut self, id: u32, gen: u32, res: Result<Vec<Entry>, String>, free: Option<(u64, u64)>) {
        let cfg = self.cfg.clone();
        let pickf = self.pick_filter();
        let Some(v) = self.view_by_id(id) else { return };
        if v.gen != gen {
            return;
        }
        v.free = free;
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
            let work = move || (if trash { Ok(crate::trash::list()) } else { fsx::list(&d) }, fsx::disk_free(&d));
            match &self.tx {
                Some(tx) => {
                    let tx = tx.clone();
                    std::thread::spawn(move || {
                        let (res, free) = work();
                    let _ = tx.send(Msg::Listed { view: id, gen, res, free });
                    });
                }
                None => {
                    let (r, free) = work();
                    self.on_listed(id, gen, r, free);
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
}

pub(crate) fn count_text(t: &[PathBuf]) -> String {
    if t.len() == 1 {
        format!("«{}»", t[0].file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default())
    } else {
        crate::tf!("объектов: {}", "{} items", "{} itens", t.len())
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
