// Боковая панель: места (дом и папки XDG), закладки (общие с GTK: ~/.config/gtk-3.0/bookmarks),
// устройства (lsblk; подключение через udisksctl; телефоны и сеть — remote.rs через gvfs), корзина.

use crate::picons::Special;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub name: String,
    pub path: PathBuf,
    pub sp: Special,
    /// закладка пользователя (можно убрать)
    pub user: bool,
}

/// папка XDG из ~/.config/user-dirs.dirs
pub fn xdg_dir(key: &str) -> Option<PathBuf> {
    let t = std::fs::read_to_string(crate::paths::config_dir().join("user-dirs.dirs")).ok()?;
    let home = crate::paths::home();
    for l in t.lines() {
        if let Some(v) = l.strip_prefix(&format!("XDG_{key}_DIR=")) {
            let v = v.trim().trim_matches('"').replace("$HOME", &home.to_string_lossy());
            let p = PathBuf::from(v);
            if p != home {
                return Some(p);
            }
        }
    }
    None
}

pub fn special_of(p: &Path) -> Special {
    let home = crate::paths::home();
    if p == home {
        return Special::Home;
    }
    if p == Path::new("/") {
        return Special::Root;
    }
    for (k, s, fb) in [
        ("DESKTOP", Special::Desktop, "Desktop"),
        ("DOCUMENTS", Special::Docs, "Documents"),
        ("DOWNLOAD", Special::Downloads, "Downloads"),
        ("MUSIC", Special::Music, "Music"),
        ("PICTURES", Special::Pictures, "Pictures"),
        ("VIDEOS", Special::Videos, "Videos"),
    ] {
        if xdg_dir(k).unwrap_or_else(|| home.join(fb)) == p {
            return s;
        }
    }
    Special::None
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
            Some(Place { name, sp: Special::Star, path: p, user: true })
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

pub fn remove_bookmark(p: &Path) {
    let t = std::fs::read_to_string(bookmarks_file()).unwrap_or_default();
    let keep: Vec<&str> = t.lines().filter(|l| crate::icons::uri_path(l.split(' ').next().unwrap_or("")).as_deref() != Some(p)).collect();
    let _ = crate::paths::write_atomic(&bookmarks_file(), &(keep.join("\n") + if keep.is_empty() { "" } else { "\n" }));
}

pub fn standard() -> Vec<Place> {
    let home = crate::paths::home();
    let mut v = vec![Place { name: "дом".into(), path: home.clone(), sp: Special::Home, user: false }];
    for (k, name, sp, fb) in [
        ("DESKTOP", "рабочий стол", Special::Desktop, "Desktop"),
        ("DOCUMENTS", "документы", Special::Docs, "Documents"),
        ("DOWNLOAD", "загрузки", Special::Downloads, "Downloads"),
        ("MUSIC", "музыка", Special::Music, "Music"),
        ("PICTURES", "изображения", Special::Pictures, "Pictures"),
        ("VIDEOS", "видео", Special::Videos, "Videos"),
    ] {
        let p = xdg_dir(k).unwrap_or_else(|| home.join(fb));
        if p.is_dir() {
            v.push(Place { name: name.into(), path: p, sp, user: false });
        }
    }
    v
}

// ── устройства ─────────────────────────────────────────────────────────────
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DevKind {
    Disk,
    Phone,
    Camera,
    Net,
}

#[derive(Clone, Debug, PartialEq)]
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
        crate::tf!("диск {}", "{} drive", crate::fsx::human(self.size))
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
                n.push(Dev { dev: String::new(), uri: s.clone(), label: crate::remote::title_of(s), size: 0, mount: None, removable: true, fstype: String::new(), kind: DevKind::Net });
            }
        }
        out.extend(n);
    }
    out
}

/// разделы с файловыми системами: смонтированные и те, что можно смонтировать
fn drives() -> Vec<Dev> {
    let Ok(o) = std::process::Command::new("lsblk").args(["-J", "-b", "-o", "PATH,LABEL,SIZE,MOUNTPOINTS,FSTYPE,HOTPLUG,RM,TYPE"]).output() else { return vec![] };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&o.stdout) else { return vec![] };
    let mut out = vec![];
    fn walk(n: &serde_json::Value, out: &mut Vec<Dev>) {
        let s = |k: &str| n.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let b = |k: &str| n.get(k).map(|x| x.as_bool().unwrap_or(x.as_str() == Some("1")) || x.as_u64() == Some(1)).unwrap_or(false);
        let fs = s("fstype");
        // btrfs с подтомами смонтирован в несколько мест: корень важнее, потом самое короткое
        let mps: Vec<PathBuf> = n.get("mountpoints").and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|m| m.as_str()).map(PathBuf::from).collect()).unwrap_or_default();
        let mount = mps.iter().find(|m| m.as_path() == Path::new("/")).or_else(|| mps.iter().min_by_key(|m| m.as_os_str().len())).cloned();
        let ty = s("type");
        let skip_fs = fs.is_empty() || fs == "swap" || fs.contains("LVM") || fs == "crypto_LUKS" || fs == "linux_raid_member";
        let boot = mount.as_deref().map(|m| m.starts_with("/boot") || m.starts_with("/efi") || m == Path::new("[SWAP]")).unwrap_or(false);
        let size = n.get("size").and_then(|x| x.as_u64().or_else(|| x.as_str().and_then(|s| s.parse().ok()))).unwrap_or(0);
        // системные разделы, кроме корня, не показываем, пока они не смонтированы где-то в /home, /mnt…
        if !skip_fs && !boot && ty != "loop" && size > 0 {
            let removable = b("hotplug") || b("rm");
            let shown = removable || mount.is_none() || mount.as_deref().map(|m| m == Path::new("/") || m.starts_with("/mnt") || m.starts_with("/media") || m.starts_with("/run/media") || m.starts_with("/home/")).unwrap_or(false);
            let mount_dup = mount.as_deref().map(|m| m.starts_with("/var") || m.starts_with("/home") && m.components().count() <= 2 || m == Path::new("/home") || m.starts_with("/.snapshots") || m.starts_with("/srv") || m.starts_with("/root") || m.starts_with("/var/cache")).unwrap_or(false);
            if shown && !mount_dup && !(mount.is_none() && !removable && fs == "vfat") {
                out.push(Dev { dev: s("path"), uri: String::new(), label: s("label"), size, mount: mount.clone(), removable, fstype: fs, kind: DevKind::Disk });
            }
        }
        if let Some(ch) = n.get("children").and_then(|c| c.as_array()) {
            for c in ch {
                walk(c, out);
            }
        }
    }
    if let Some(bd) = v.get("blockdevices").and_then(|b| b.as_array()) {
        for n in bd {
            walk(n, &mut out);
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
