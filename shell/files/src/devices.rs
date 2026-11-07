// Устройства и подключения в файлах: диски, телефоны, сетевые места; подключение, отключение и
// извлечение через udisksctl и gio (находят их places.rs и remote.rs).

use crate::fm::*;
use crate::places::{self, Dev};
use crate::ui::Field;
use std::path::PathBuf;
use std::time::Instant;

impl Fm {
    // ── устройства ─────────────────────────────────────────────────────────
    /// из tick, раз в 4 секунды: опросить устройства, только если что-то изменилось
    pub fn poll_devices(&mut self) {
        self.dev_tick = Instant::now();
        let s = sig();
        if s == self.dev_sig && self.dev_full.elapsed().as_secs() < 60 {
            return;
        }
        self.refresh_devices();
    }

    pub fn refresh_devices(&mut self) {
        self.dev_tick = Instant::now();
        if !self.cfg.devices && !self.cfg.network {
            self.devs.clear();
            return;
        }
        // прошлый опрос ещё идёт (gio ждёт телефон) — второй не запускаем; отпечаток не обновляем,
        // чтобы следующий тик увидел перемену и спросил снова (иначе флешка ждала бы минутного опроса)
        if self.tx.is_some() && self.dev_busy.swap(true, std::sync::atomic::Ordering::AcqRel) {
            return;
        }
        self.dev_sig = sig();
        self.dev_full = Instant::now();
        let (disks, net, servers) = (self.cfg.devices, self.cfg.network, self.cfg.servers.clone());
        match &self.tx {
            Some(tx) => {
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
    pub(crate) fn remote_mount(&mut self, uri: String, auth: crate::remote::Auth) {
        let Some(tx) = self.tx.clone() else { return };
        std::thread::spawn(move || {
            let res = crate::remote::mount(&uri, &auth);
            let _ = tx.send(Msg::Remote { uri, res, auth });
        });
    }

    /// окно пароля зашифрованного диска (щелчок по нему или `yami-files --unlock /dev/…`)
    pub fn unlock_dlg(&mut self, dev: &str, name: String) {
        if !dev.starts_with("/dev/") || dev.contains("..") {
            return;
        }
        self.dlg = Some((Dlg::Unlock { dev: dev.to_string(), name, pass: Field::default(), err: String::new(), busy: false }, Instant::now()));
    }

    /// итог открытия зашифрованного диска
    fn on_unlock(&mut self, res: crate::remote::Mounted) {
        use crate::remote::Mounted as M;
        let e = match res {
            M::Ok(p) => {
                if matches!(self.dlg, Some((Dlg::Unlock { .. }, _))) {
                    self.dlg = None;
                }
                match p {
                    Some(p) => self.go(p),
                    None => self.toast(crate::i18n::tr("подключено")),
                }
                return self.refresh_devices();
            }
            M::NeedAuth { .. } => crate::i18n::tr("неверный пароль").to_string(),
            M::Question(q) => q,
            // udisks: «Failed to activate device: Incorrect passphrase» — по-человечески
            M::Err(e) if e.contains("passphrase") || e.contains("No key available") => crate::i18n::tr("неверный пароль").to_string(),
            M::Err(e) => e,
        };
        match &mut self.dlg {
            Some((Dlg::Unlock { err, busy, .. }, _)) => {
                *err = e;
                *busy = false;
            }
            _ => self.toast(e),
        }
    }

    pub fn on_remote(&mut self, uri: String, res: crate::remote::Mounted, auth: crate::remote::Auth) {
        use crate::remote::Mounted as M;
        if uri.starts_with("/dev/") {
            return self.on_unlock(res);
        }
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
                    crate::tf!("{} · нужен пакет gvfs-{}", "{} · needs the gvfs-{} package", "{} · requer o pacote gvfs-{}", e, if scheme == "smb" || scheme == "nfs" || scheme == "afp" { scheme } else { "backends" })
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
    pub(crate) fn server_ok(&mut self) {
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

    pub(crate) fn mount(&mut self, i: usize, on: bool) {
        let Some(d) = self.devs.get(i).cloned() else { return };
        let Some(tx) = self.tx.clone() else { return };
        if d.kind != places::DevKind::Disk {
            return self.mount_remote(d, on, tx);
        }
        if on && d.locked {
            return self.unlock_dlg(&d.dev, d.title());
        }
        self.toast(if on { crate::tf!("подключаю {}…", "mounting {}…", "montando {}…", d.title()) } else { crate::tf!("отключаю {}…", "unmounting {}…", "desmontando {}…", d.title()) });
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
            let out = std::process::Command::new("udisksctl").args([if on { "mount" } else { "unmount" }, "-b", &d.dev]).stdin(std::process::Stdio::null()).output();
            let (err, open) = match out {
                Ok(o) if o.status.success() => {
                    if on {
                        // «Mounted /dev/sdb1 at /run/media/user/LABEL»
                        let s = String::from_utf8_lossy(&o.stdout).to_string();
                        let p = s.split(" at ").nth(1).map(|x| PathBuf::from(x.trim().trim_end_matches('.')));
                        (None, p)
                    } else {
                        let ud = |a: &[&str]| std::process::Command::new("udisksctl").args(a).stdin(std::process::Stdio::null()).output();
                        // открытый LUKS — закрыть: без этого содержимое диска осталось бы открытым
                        if !d.crypt.is_empty() {
                            let _ = ud(&["lock", "-b", &d.crypt, "--no-user-interaction"]);
                        }
                        if d.removable && !d.disk.is_empty() {
                            let _ = ud(&["power-off", "-b", &d.disk, "--no-user-interaction"]);
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
    pub(crate) fn mount_remote(&mut self, d: Dev, on: bool, tx: calloop::channel::Sender<Msg>) {
        if on {
            if d.uri.is_empty() {
                // телефон виден по USB, но у gvfs нет бэкенда MTP
                self.toast(crate::i18n::tr("для телефонов нужен пакет gvfs-mtp (и gvfs-gphoto2 для камер)"));
                return;
            }
            self.toast(crate::tf!("подключаю {}…", "mounting {}…", "montando {}…", d.title()));
            self.remote_mount(d.uri.clone(), crate::remote::Auth::default());
            return;
        }
        if let Some(m) = &d.mount {
            if self.v().dir.starts_with(m) {
                self.go(crate::paths::home());
            }
        }
        self.toast(crate::tf!("отключаю {}…", "unmounting {}…", "desmontando {}…", d.title()));
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

/// отпечаток устройств без запуска программ: разделы (/proc/partitions), точки монтирования
/// (mountinfo), USB (телефоны и камеры появляются там раньше, чем у gio) и подключения gvfs
fn sig() -> u64 {
    let mut h = yami_px::FNV0;
    let mut eat = |b: &[u8]| h = yami_px::fnv(h, b);
    for f in ["/proc/partitions", "/proc/self/mountinfo"] {
        eat(&std::fs::read(f).unwrap_or_default());
    }
    let gvfs = std::env::var_os("XDG_RUNTIME_DIR").map(|d| std::path::PathBuf::from(d).join("gvfs"));
    for d in [Some(std::path::PathBuf::from("/sys/bus/usb/devices")), gvfs].into_iter().flatten() {
        let mut names: Vec<_> = std::fs::read_dir(&d).map(|r| r.flatten().map(|e| e.file_name()).collect()).unwrap_or_default();
        names.sort();
        for n in names {
            eat(n.as_encoded_bytes());
        }
    }
    h
}
