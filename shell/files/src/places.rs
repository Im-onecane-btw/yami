// Боковая панель: места (дом, папки XDG, корзина) и закладки (общие с GTK:
// ~/.config/gtk-3.0/bookmarks) одним списком в своём порядке — его тянут мышью, лишнее прячут
// (yami-files.json: placesOrder, placesHidden); устройства (lsblk; подключение через udisksctl,
// зашифрованные — через gio с паролем; телефоны и сеть — remote.rs через gvfs).

use crate::picons::Special;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    /// «home», «desktop»… «trash» — стандартные; у закладок — путь
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub sp: Special,
    /// закладка пользователя (можно убрать и переименовать)
    pub user: bool,
}

impl Place {
    pub fn trash(&self) -> bool {
        self.id == "trash"
    }
}

/// папка XDG из ~/.config/user-dirs.dirs (общее с шеллом, yami-px)
pub use crate::paths::xdg_dir;

/// стандартные места: ключ, имя, ключ XDG, запасная папка, значок
const STD: [(&str, &str, &str, &str, Special); 7] = [
    ("home", "дом", "", "", Special::Home),
    ("desktop", "рабочий стол", "DESKTOP", "Desktop", Special::Desktop),
    ("documents", "документы", "DOCUMENTS", "Documents", Special::Docs),
    ("downloads", "загрузки", "DOWNLOAD", "Downloads", Special::Downloads),
    ("music", "музыка", "MUSIC", "Music", Special::Music),
    ("pictures", "изображения", "PICTURES", "Pictures", Special::Pictures),
    ("videos", "видео", "VIDEOS", "Videos", Special::Videos),
];

thread_local! {
    /// папки XDG со значками: special_of зовут на каждую папку каждого кадра — файл user-dirs.dirs
    /// читается один раз (и заново при «обновить», `standard`)
    static XDG: RefCell<Option<Vec<(PathBuf, Special)>>> = const { RefCell::new(None) };
}

fn xdg_table() -> Vec<(PathBuf, Special)> {
    let home = crate::paths::home();
    STD.iter().map(|(_, _, k, fb, sp)| (if k.is_empty() { home.clone() } else { xdg_dir(k).unwrap_or_else(|| home.join(fb)) }, *sp)).collect()
}

pub fn special_of(p: &Path) -> Special {
    if p == Path::new("/") {
        return Special::Root;
    }
    XDG.with(|c| {
        let mut c = c.borrow_mut();
        c.get_or_insert_with(xdg_table).iter().find(|(d, _)| d == p).map(|x| x.1).unwrap_or(Special::None)
    })
}

fn bookmarks_file() -> PathBuf {
    crate::paths::config_dir().join("gtk-3.0/bookmarks")
}

pub fn bookmarks() -> Vec<Place> {
    let Ok(t) = std::fs::read_to_string(bookmarks_file()) else { return vec![] };
    t.lines()
        .filter_map(|l| {
            let (u, name) = match l.split_once(' ') {
                Some((u, n)) => (u, Some(n.to_string())),
                None => (l, None),
            };
            let p = crate::icons::uri_path(u)?;
            let name = name.unwrap_or_else(|| p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| p.display().to_string()));
            Some(Place { id: p.to_string_lossy().into_owned(), name, sp: Special::Star, path: p, user: true })
        })
        .collect()
}

pub fn add_bookmark(p: &Path) {
    let mut t = std::fs::read_to_string(bookmarks_file()).unwrap_or_default();
    let u = crate::icons::file_uri(p);
    if t.lines().any(|l| l.split(' ').next() == Some(u.as_str())) {
        return;
    }
    if !t.is_empty() && !t.ends_with('\n') {
        t.push('\n');
    }
    t.push_str(&u);
    t.push('\n');
    let _ = crate::paths::write_atomic(&bookmarks_file(), &t);
}

/// переписать строки закладок: `f` получает (путь, строка) и отдаёт новую строку или None — убрать
fn edit_bookmarks(f: impl Fn(&Path, &str) -> Option<String>) {
    let t = std::fs::read_to_string(bookmarks_file()).unwrap_or_default();
    let keep: Vec<String> = t.lines().filter_map(|l| match crate::icons::uri_path(l.split(' ').next().unwrap_or("")) {
        Some(p) => f(&p, l),
        None => Some(l.to_string()),
    }).collect();
    let _ = crate::paths::write_atomic(&bookmarks_file(), &(keep.join("\n") + if keep.is_empty() { "" } else { "\n" }));
}

pub fn remove_bookmark(p: &Path) {
    edit_bookmarks(|q, l| (q != p).then(|| l.to_string()));
}

/// своё имя закладки (пустое — снова имя папки); формат GTK: «адрес имя»
pub fn rename_bookmark(p: &Path, name: &str) {
    let name = name.trim().replace(['\n', '\r'], " ");
    edit_bookmarks(|q, l| {
        if q != p {
            return Some(l.to_string());
        }
        let u = l.split(' ').next().unwrap_or("");
        Some(if name.is_empty() { u.to_string() } else { format!("{u} {name}") })
    });
}

/// все стандартные места, что есть на диске, и корзина
pub fn standard() -> Vec<Place> {
    let table = xdg_table();
    XDG.with(|c| *c.borrow_mut() = Some(table.clone()));
    let mut v: Vec<Place> = STD
        .iter()
        .zip(table)
        .filter(|((id, ..), (p, _))| *id == "home" || p.is_dir())
        .map(|((id, name, ..), (path, sp))| Place { id: id.to_string(), name: name.to_string(), path, sp, user: false })
        .collect();
    v.push(Place { id: "trash".into(), name: "корзина".into(), path: crate::trash::home_trash().join("files"), sp: Special::Trash, user: false });
    v
}

/// места боковой панели: свой порядок (что в нём не упомянуто — следом, как было), без скрытых.
/// Второй список — скрытые (для «показать снова»)
pub fn sidebar(order: &[String], hidden: &[String]) -> (Vec<Place>, Vec<Place>) {
    let mut all = standard();
    all.extend(bookmarks());
    // закладка на ту же папку, что стандартное место, — второй строкой не нужна
    let mut seen: Vec<PathBuf> = vec![];
    all.retain(|p| {
        let dup = seen.contains(&p.path) && p.user;
        seen.push(p.path.clone());
        !dup
    });
    let rank = |p: &Place| order.iter().position(|o| *o == p.id).unwrap_or(usize::MAX);
    // устойчивая сортировка: неупомянутые остаются в своём порядке, в конце
    all.sort_by_key(rank);
    all.into_iter().partition(|p| !hidden.contains(&p.id))
}

// ── устройства ─────────────────────────────────────────────────────────────
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum DevKind {
    #[default]
    Disk,
    Phone,
    Camera,
    Net,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Dev {
    /// блочное устройство (/dev/sdb1) или источник сетевого монтирования ядра
    pub dev: String,
    /// адрес gvfs (mtp://…, smb://host/share/); пусто у дисков
    pub uri: String,
    pub label: String,
    pub size: u64,
    pub mount: Option<PathBuf>,
    pub removable: bool,
    pub fstype: String,
    pub kind: DevKind,
    pub uuid: String,
    /// весь диск (/dev/sdb): его выключают после «извлечь»
    pub disk: String,
    /// раздел LUKS, у которого этот — открытое содержимое: после отключения его закрыть
    pub crypt: String,
    /// зашифрованный раздел, ещё не открыт: щелчок спросит пароль
    pub locked: bool,
    /// свободно и всего: меряется в нити опроса, не в кадре (statvfs сети и телефона может висеть)
    pub free: Option<(u64, u64)>,
}

impl Dev {
    pub fn title(&self) -> String {
        if self.kind != DevKind::Disk {
            return if self.label.is_empty() { crate::remote::title_of(&self.uri) } else { self.label.clone() };
        }
        if self.mount.as_deref() == Some(Path::new("/")) {
            return crate::i18n::tr("система").to_string();
        }
        if !self.label.is_empty() {
            return self.label.clone();
        }
        crate::tf!("диск {}", "{} drive", "disco de {}", crate::fsx::human(self.size))
    }
    /// ключ для «скрыть»: UUID раздела, адрес без пароля или имя устройства
    pub fn key(&self) -> String {
        if !self.uuid.is_empty() {
            self.uuid.clone()
        } else if !self.uri.is_empty() {
            crate::remote::strip_secret(&self.uri)
        } else {
            self.dev.clone()
        }
    }
}

/// всё для боковой панели: диски (`disks`), телефоны и камеры, сеть (`net`) с сохранёнными адресами
pub fn devices(disks: bool, net: bool, servers: &[String]) -> Vec<Dev> {
    let mut out = if disks { drives() } else { vec![] };
    let remote = crate::remote::gvfs_devices();
    if disks {
        out.extend(remote.iter().filter(|d| d.kind != DevKind::Net).cloned());
    }
    if net {
        let mut n: Vec<Dev> = remote.into_iter().filter(|d| d.kind == DevKind::Net).collect();
        n.extend(crate::remote::kernel_net());
        // сохранённые адреса, которые сейчас не подключены
        for s in servers {
            let k = s.trim_end_matches('/');
            if !n.iter().any(|d| !d.uri.is_empty() && crate::remote::strip_secret(&d.uri).trim_end_matches('/') == k) {
                n.push(Dev { uri: s.clone(), label: crate::remote::title_of(s), removable: true, kind: DevKind::Net, ..Default::default() });
            }
        }
        out.extend(n);
    }
    for d in out.iter_mut().filter(|d| matches!(d.kind, DevKind::Disk | DevKind::Phone)) {
        d.free = d.mount.as_deref().and_then(crate::fsx::disk_free);
    }
    out
}

/// разделы с файловыми системами: смонтированные и те, что можно смонтировать
fn drives() -> Vec<Dev> {
    let Ok(o) = std::process::Command::new("lsblk").args(["-J", "-b", "-o", "PATH,LABEL,SIZE,MOUNTPOINTS,FSTYPE,HOTPLUG,RM,TYPE,UUID"]).output() else { return vec![] };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&o.stdout) else { return vec![] };
    let mut out = vec![];
    /// `disk` — весь диск, `crypt` — открытый над этим разделом LUKS, `hot` — диск съёмный
    fn walk(n: &serde_json::Value, disk: &str, crypt: &str, hot: bool, out: &mut Vec<Dev>) {
        let s = |k: &str| n.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let b = |k: &str| n.get(k).map(|x| x.as_bool().unwrap_or(x.as_str() == Some("1")) || x.as_u64() == Some(1)).unwrap_or(false);
        let fs = s("fstype");
        let path = s("path");
        let ty = s("type");
        let disk = if ty == "disk" || disk.is_empty() { path.clone() } else { disk.to_string() };
        let children = n.get("children").and_then(|c| c.as_array());
        // btrfs с подтомами смонтирован в несколько мест: корень важнее, потом самое короткое
        let mps: Vec<PathBuf> = n.get("mountpoints").and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|m| m.as_str()).map(PathBuf::from).collect()).unwrap_or_default();
        let mount = mps.iter().find(|m| m.as_path() == Path::new("/")).or_else(|| mps.iter().min_by_key(|m| m.as_os_str().len())).cloned();
        let luks = fs == "crypto_LUKS" || fs == "BitLocker";
        let removable = b("hotplug") || b("rm") || hot;
        // закрытый LUKS (без открытого содержимого) показываем: щелчок спросит пароль
        let locked = luks && children.is_none_or(|c| c.is_empty());
        let skip_fs = fs.is_empty() || fs == "swap" || fs.contains("LVM") || (luks && !locked) || fs == "linux_raid_member";
        let boot = mount.as_deref().map(|m| m.starts_with("/boot") || m.starts_with("/efi") || m == Path::new("[SWAP]")).unwrap_or(false);
        let size = n.get("size").and_then(|x| x.as_u64().or_else(|| x.as_str().and_then(|s| s.parse().ok()))).unwrap_or(0);
        // системные разделы, кроме корня, не показываем, пока они не смонтированы где-то в /home, /mnt…
        if !skip_fs && !boot && ty != "loop" && size > 0 {
            let shown = removable || mount.is_none() || mount.as_deref().map(|m| m == Path::new("/") || m.starts_with("/mnt") || m.starts_with("/media") || m.starts_with("/run/media") || m.starts_with("/home/")).unwrap_or(false);
            let mount_dup = mount.as_deref().map(|m| m.starts_with("/var") || m.starts_with("/home") && m.components().count() <= 2 || m == Path::new("/home") || m.starts_with("/.snapshots") || m.starts_with("/srv") || m.starts_with("/root") || m.starts_with("/var/cache")).unwrap_or(false);
            if shown && !mount_dup && !(mount.is_none() && !removable && fs == "vfat") {
                out.push(Dev { dev: path.clone(), label: s("label"), size, mount: mount.clone(), removable, fstype: fs.clone(), kind: DevKind::Disk, uuid: s("uuid"), disk: disk.clone(), crypt: crypt.to_string(), locked, ..Default::default() });
            }
        }
        for c in children.into_iter().flatten() {
            walk(c, &disk, if luks { &path } else { crypt }, removable, out);
        }
    }
    if let Some(bd) = v.get("blockdevices").and_then(|b| b.as_array()) {
        for n in bd {
            walk(n, "", "", false, &mut out);
        }
    }
    // btrfs-подтома одного раздела — один раз
    let mut seen: Vec<String> = vec![];
    out.retain(|d| {
        if seen.contains(&d.dev) {
            return false;
        }
        seen.push(d.dev.clone());
        true
    });
    out.sort_by_key(|d| (d.mount.as_deref() != Some(Path::new("/")), d.removable, d.dev.clone()));
    out
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn order() {
        // дом и корзина есть всегда, остальные папки — смотря по HOME
        let (shown, hidden) = sidebar(&["trash".into(), "home".into()], &["desktop".into()]);
        assert_eq!(shown[0].id, "trash");
        assert_eq!(shown[1].id, "home");
        assert!(shown.iter().all(|p| p.id != "desktop"));
        assert!(hidden.iter().all(|p| p.id == "desktop"));
    }
}
