// Контекстные меню (ПКМ): по файлу, по пустому месту, по месту в боковой панели, и главное
// меню (кнопка ≡). Пункты — MItem с командой Cmd; подменю — «открыть с помощью», «вид»…

use crate::fm::{Cmd, Fm, MItem, Menu, Mode};
use crate::fsx::Sort;
use crate::mime::{self, Kind};
use std::time::Instant;

fn view_items(fm: &Fm) -> Vec<MItem> {
    let s = |id: &str| fm.cfg.sort == id;
    vec![
        MItem::new("значки", "grid", "ctrl+1", Cmd::Mode(Mode::Grid)).check(fm.mode() == Mode::Grid),
        MItem::new("список", "menu", "ctrl+2", Cmd::Mode(Mode::List)).check(fm.mode() == Mode::List),
        MItem::sep(),
        MItem::new("по имени", "", "", Cmd::Sort(Sort::Name)).check(s("name")),
        MItem::new("по размеру", "", "", Cmd::Sort(Sort::Size)).check(s("size")),
        MItem::new("по дате", "", "", Cmd::Sort(Sort::Time)).check(s("time")),
        MItem::new("по типу", "", "", Cmd::Sort(Sort::Kind)).check(s("type")),
        MItem::new("по убыванию", "", "", Cmd::Desc).check(fm.cfg.desc),
        MItem::new("папки сверху", "", "", Cmd::FoldersFirst).check(fm.cfg.folders_first),
        MItem::sep(),
        MItem::new("скрытые файлы", "eye", "ctrl+h", Cmd::Hidden).check(fm.cfg.hidden),
        MItem::new("сведения справа", "about", "f11", Cmd::Info).check(fm.cfg.info),
        MItem::new("крупнее", "plus", "ctrl+=", Cmd::Zoom(1)).off(fm.cfg.zoom >= 4 && fm.mode() == Mode::Grid),
        MItem::new("мельче", "min", "ctrl+-", Cmd::Zoom(-1)).off(fm.cfg.zoom <= 0 || fm.mode() == Mode::List),
    ]
}

fn new_items() -> Vec<MItem> {
    vec![MItem::new("папку", "folder", "ctrl+shift+n", Cmd::NewFolder), MItem::new("пустой файл", "plus", "", Cmd::NewFile)]
}

pub fn for_items(fm: &mut Fm) -> Vec<MItem> {
    let v = fm.v();
    let trash = v.trash;
    let sel: Vec<crate::fsx::Entry> = {
        let s: Vec<crate::fsx::Entry> = v.selected().into_iter().cloned().collect();
        if s.is_empty() {
            v.cur_entry().cloned().into_iter().collect()
        } else {
            s
        }
    };
    if sel.is_empty() {
        return for_bg(fm);
    }
    if trash {
        return vec![
            MItem::new("вернуть", "reboot", "", Cmd::Restore),
            MItem::new("удалить навсегда", "trash", "del", Cmd::Delete),
            MItem::sep(),
            MItem::new("свойства", "about", "alt+enter", Cmd::Props),
        ];
    }
    let one = sel.len() == 1;
    let all_dirs = sel.iter().all(|e| e.dir);
    let any_dir = sel.iter().any(|e| e.dir);
    let first = sel[0].clone();
    let mut m = vec![];
    if fm.pick.is_some() {
        m.push(MItem::new(if any_dir && one { "открыть" } else { "выбрать" }, "check", "enter", Cmd::Open));
    } else {
        m.push(MItem::new("открыть", "right", "enter", Cmd::Open));
    }
    if all_dirs {
        m.push(MItem::new("в новой вкладке", "plus", "", Cmd::OpenTab));
        m.push(MItem::new("в соседней панели", "tile", "", Cmd::OpenSplit));
        if one {
            m.push(MItem::new("терминал здесь", "exit", "f4", Cmd::Terminal));
        }
    } else if fm.pick.is_none() {
        let cands: Vec<(String, String)> = fm.apps().candidates(first.mime).iter().take(8).map(|a| (a.id.clone(), a.name.clone())).collect();
        let mut sub: Vec<MItem> = cands.into_iter().map(|(id, name)| MItem::new(&name, "", "", Cmd::OpenWith(id))).collect();
        if !sub.is_empty() {
            sub.push(MItem::sep());
        }
        sub.push(MItem::new("другая программа…", "search", "", Cmd::OpenWithOther));
        m.push(MItem::sub("открыть с помощью", "dots", sub));
        if one && first.exec() {
            m.push(MItem::new("запустить", "play", "", Cmd::Run));
        }
    }
    m.push(MItem::sep());
    m.push(MItem::new("вырезать", "", "ctrl+x", Cmd::Cut));
    m.push(MItem::new("копировать", "copy", "ctrl+c", Cmd::Copy));
    if one && first.dir {
        m.push(MItem::new("вставить сюда", "down", "", Cmd::PasteInto(first.path.clone())));
    }
    m.push(MItem::new("копировать путь", "", "ctrl+alt+c", Cmd::CopyPath));
    m.push(MItem::new("переименовать", "", "f2", Cmd::Rename).off(!one));
    m.push(MItem::sep());
    let k = mime::kind(first.mime);
    if one && k == Kind::Archive {
        m.push(MItem::new("распаковать сюда", "down", "", Cmd::Extract));
    }
    m.push(MItem::sub("сжать", "zip", vec![MItem::new(".zip", "", "", Cmd::Compress("zip")), MItem::new(".tar.zst", "", "", Cmd::Compress("tar.zst")), MItem::new(".tar.gz", "", "", Cmd::Compress("tar.gz")), MItem::new(".7z", "", "", Cmd::Compress("7z"))]));
    m.push(MItem::new("создать ссылку", "", "", Cmd::Link));
    if one && k == Kind::Image {
        m.push(MItem::new("сделать обоями", "image", "", Cmd::Wallpaper));
    }
    if all_dirs {
        m.push(MItem::new("добавить в места", "pin", "", Cmd::AddPlace));
    }
    m.push(MItem::sep());
    m.push(MItem::new("в корзину", "trash", "del", Cmd::Trash));
    m.push(MItem::new("удалить навсегда", "", "shift+del", Cmd::Delete));
    m.push(MItem::sep());
    m.push(MItem::new("свойства", "about", "alt+enter", Cmd::Props));
    m
}

pub fn for_bg(fm: &mut Fm) -> Vec<MItem> {
    let v = fm.v();
    if v.trash {
        return vec![MItem::new("очистить корзину", "trash", "", Cmd::EmptyTrash).off(fm.trash_n == 0), MItem::sep(), MItem::sub("вид", "eye", view_items(fm))];
    }
    let can_paste = true;
    vec![
        MItem::sub("создать", "plus", new_items()),
        MItem::new("вставить", "down", "ctrl+v", Cmd::Paste).off(!can_paste),
        MItem::new("отменить", "left", "ctrl+z", Cmd::Undo).off(fm.undo.is_empty()),
        MItem::sep(),
        MItem::new("выделить всё", "check", "ctrl+a", Cmd::SelectAll),
        MItem::new("обратить выделение", "", "ctrl+i", Cmd::Invert),
        MItem::sep(),
        MItem::sub("вид", "eye", view_items(fm)),
        MItem::new("обновить", "reboot", "f5", Cmd::Reload),
        MItem::sep(),
        MItem::new("терминал здесь", "exit", "f4", Cmd::Terminal),
        MItem::new("копировать путь", "copy", "", Cmd::CopyPath),
        MItem::new("добавить в места", "pin", "", Cmd::AddPlace),
        MItem::new("свойства папки", "about", "", Cmd::Props),
    ]
}

pub fn main_menu(fm: &mut Fm) -> Vec<MItem> {
    vec![
        MItem::new("новое окно", "plus", "ctrl+n", Cmd::NewWindow).off(fm.pick.is_some()),
        MItem::new("новая вкладка", "plus", "ctrl+t", Cmd::NewTab).off(fm.pick.is_some()),
        MItem::new("разделить", "tile", "f3", Cmd::Split).check(fm.tabv().panes.len() > 1),
        MItem::sep(),
        MItem::sub("создать", "plus", new_items()),
        MItem::sub("вид", "eye", view_items(fm)),
        MItem::new("поиск", "search", "ctrl+f", Cmd::Search),
        MItem::new("корзина", "trash", "", Cmd::GoTrash),
        MItem::sep(),
        MItem::new("отменить", "left", "ctrl+z", Cmd::Undo).off(fm.undo.is_empty()),
        MItem::new("настройки", "gear", "ctrl+,", Cmd::Settings),
    ]
}

pub fn for_place(path: &std::path::Path, user: bool) -> Vec<MItem> {
    let p = path.to_path_buf();
    let mut m = vec![MItem::new("открыть", "right", "", Cmd::Go(p.clone())), MItem::new("в новой вкладке", "plus", "", Cmd::OpenTab)];
    m.push(MItem::new("вставить сюда", "down", "", Cmd::PasteInto(p.clone())));
    if user {
        m.push(MItem::sep());
        m.push(MItem::new("убрать из мест", "close", "", Cmd::RemovePlace(p)));
    }
    m
}

pub fn for_trash_place(fm: &Fm) -> Vec<MItem> {
    vec![MItem::new("открыть", "right", "", Cmd::GoTrash), MItem::new("очистить корзину", "trash", "", Cmd::EmptyTrash).off(fm.trash_n == 0)]
}

pub fn for_dev(fm: &Fm, i: usize) -> Vec<MItem> {
    let Some(d) = fm.devs.get(i) else { return vec![] };
    let mut m = vec![];
    match &d.mount {
        Some(p) => {
            m.push(MItem::new("открыть", "right", "", Cmd::Go(p.clone())));
            if p != std::path::Path::new("/") {
                let eject = d.removable && d.kind != crate::places::DevKind::Net;
                m.push(MItem::new(if eject { "извлечь" } else { "отключить" }, "up", "", Cmd::Unmount(i)));
            }
        }
        None => m.push(MItem::new("подключить", "down", "", Cmd::Mount(i))),
    }
    // сеть: войти под другим логином и забыть адрес
    if d.kind == crate::places::DevKind::Net && !d.uri.is_empty() {
        let keep = crate::remote::strip_secret(&d.uri);
        if d.mount.is_none() {
            m.push(MItem::new("подключить как…", "user", "", Cmd::Connect(keep.clone())));
        }
        if fm.cfg.servers.contains(&keep) {
            m.push(MItem::new("забыть адрес", "close", "", Cmd::Forget(keep)));
        }
    }
    m
}

pub fn for_drop(srcs: Vec<std::path::PathBuf>, dst: std::path::PathBuf) -> Vec<MItem> {
    vec![
        MItem::new("переместить сюда", "right", "shift", Cmd::DropMove(srcs.clone(), dst.clone())),
        MItem::new("копировать сюда", "copy", "ctrl", Cmd::DropCopy(srcs.clone(), dst.clone())),
        MItem::new("ссылку сюда", "", "ctrl+shift", Cmd::DropLink(srcs, dst)),
        MItem::sep(),
        MItem::new("отмена", "close", "esc", Cmd::None),
    ]
}

pub fn open(fm: &mut Fm, items: Vec<MItem>, x: i32, y: i32, title: &str) {
    if items.is_empty() {
        return;
    }
    fm.menu = Some(Menu { items, x, y, born: Instant::now(), sub: None, hover: None, sub_hover: None, title: title.to_string() });
}
