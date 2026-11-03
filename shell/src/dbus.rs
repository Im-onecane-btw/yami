// D-Bus: сервер уведомлений (org.freedesktop.Notifications) и трей (StatusNotifierWatcher +
// хост, меню через com.canonical.dbusmenu). Всё в своей нити; в шелл уходят готовые данные.

use crate::app::Msg;
use crate::gfx::Img;
use crate::menu::{MAct, MItem};
use calloop::channel::Sender;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use zbus::blocking::{Connection, MessageIterator, Proxy};
use zbus::zvariant::{OwnedValue, Value};

static CONN: OnceLock<Connection> = OnceLock::new();

// ── уведомления ─────────────────────────────────────────────────────────────
pub struct Incoming {
    pub id: u32,
    pub app: String,
    pub icon: String,
    pub summary: String,
    pub body: String,
    pub actions: Vec<(String, String)>,
    pub urgency: u8,
    pub image: Option<Img>,
    pub timeout: i32,
}

struct Notif {
    tx: Sender<Msg>,
    next: Arc<AtomicU32>,
}

fn strip_markup(s: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for c in s.chars() {
        match c {
            '<' => tag = true,
            '>' if tag => tag = false,
            _ if !tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&#39;", "'")
}

fn image_data(v: &Value) -> Option<Img> {
    let v = match v {
        Value::Value(b) => b.as_ref(),
        x => x,
    };
    let Value::Structure(s) = v else { return None };
    let f = s.fields();
    let num = |i: usize| -> i32 {
        match f.get(i) {
            Some(Value::I32(x)) => *x,
            _ => 0,
        }
    };
    let (w, h, stride) = (num(0) as i64, num(1) as i64, num(2) as i64);
    let alpha = matches!(f.get(3), Some(Value::Bool(true)));
    let ch = num(5) as i64;
    // размеры от чужого процесса: без переполнений, строка не короче пикселей, не больше 1024²
    // (иначе «картинка» 10⁵×10⁵ при stride 0 потребовала бы десятки гигабайт)
    if !(3..=4).contains(&ch) || !(1..=1024).contains(&w) || !(1..=1024).contains(&h) || stride < w * ch {
        return None;
    }
    let Some(Value::Array(a)) = f.get(6) else { return None };
    if (a.len() as i64) < stride * (h - 1) + w * ch {
        return None;
    }
    let bytes: Vec<u8> = a.inner().iter().filter_map(|x| if let Value::U8(b) = x { Some(*b) } else { None }).collect();
    if (bytes.len() as i64) < stride * (h - 1) + w * ch {
        return None;
    }
    let mut img = image::RgbaImage::new(w as u32, h as u32);
    for y in 0..h {
        for x in 0..w {
            let o = (y * stride + x * ch) as usize;
            let a = if alpha && ch == 4 { bytes[o + 3] } else { 255 };
            img.put_pixel(x as u32, y as u32, image::Rgba([bytes[o], bytes[o + 1], bytes[o + 2], a]));
        }
    }
    Some(crate::icons::fit(img, 48))
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Notif {
    #[allow(clippy::too_many_arguments)]
    fn notify(&self, app_name: &str, replaces_id: u32, app_icon: &str, summary: &str, body: &str, actions: Vec<String>, hints: HashMap<String, OwnedValue>, expire_timeout: i32) -> u32 {
        let id = if replaces_id > 0 { replaces_id } else { self.next.fetch_add(1, Ordering::SeqCst) };
        let urgency = match hints.get("urgency").map(|v| &**v) {
            Some(Value::U8(u)) => *u,
            _ => 1,
        };
        let mut image = None;
        for k in ["image-data", "image_data", "icon_data"] {
            if let Some(v) = hints.get(k) {
                image = image_data(v);
                if image.is_some() {
                    break;
                }
            }
        }
        if image.is_none() {
            for k in ["image-path", "image_path"] {
                if let Some(Value::Str(p)) = hints.get(k).map(|v| &**v) {
                    let p = p.as_str().trim_start_matches("file://");
                    image = crate::icons::load_file(std::path::Path::new(p), 48);
                }
            }
        }
        let acts: Vec<(String, String)> = actions.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0].clone(), c[1].clone())).collect();
        let _ = self.tx.send(Msg::Notif(Incoming {
            id,
            app: app_name.to_string(),
            icon: app_icon.trim_start_matches("file://").to_string(),
            summary: strip_markup(summary),
            body: strip_markup(body),
            actions: acts,
            urgency,
            image,
            timeout: expire_timeout,
        }));
        id
    }
    fn close_notification(&self, id: u32) {
        let _ = self.tx.send(Msg::NotifClose(id));
    }
    fn get_capabilities(&self) -> Vec<String> {
        ["actions", "body", "body-markup", "icon-static", "persistence"].iter().map(|s| s.to_string()).collect()
    }
    fn get_server_information(&self) -> (String, String, String, String) {
        ("yami-shell".into(), "YAMI".into(), env!("CARGO_PKG_VERSION").into(), "1.2".into())
    }
}

/// уведомление закрыто (reason: 1 истекло, 2 закрыл человек, 3 закрыли через D-Bus)
pub fn notif_closed(id: u32, reason: u32) {
    if let Some(c) = CONN.get() {
        let _ = c.emit_signal(None::<&str>, "/org/freedesktop/Notifications", "org.freedesktop.Notifications", "NotificationClosed", &(id, reason));
    }
}
pub fn notif_action(id: u32, key: &str) {
    if let Some(c) = CONN.get() {
        let _ = c.emit_signal(None::<&str>, "/org/freedesktop/Notifications", "org.freedesktop.Notifications", "ActionInvoked", &(id, key));
    }
}

// ── трей ────────────────────────────────────────────────────────────────────
pub struct TrayItem {
    pub service: String,
    pub bus: String,
    pub path: String,
    pub title: String,
    pub status: String,
    pub icon: String,
    pub theme_path: Option<String>,
    pub pixmap: Option<std::rc::Rc<Img>>,
    pub menu: String,
    pub is_menu: bool,
}

pub enum TrayEv {
    Upsert { service: String, bus: String, path: String, title: String, status: String, icon: String, theme_path: Option<String>, pixmap: Option<Img>, menu: String, is_menu: bool },
    Remove(String),
}

pub fn apply(list: &mut Vec<TrayItem>, e: TrayEv) {
    match e {
        TrayEv::Remove(s) => list.retain(|i| i.service != s),
        TrayEv::Upsert { service, bus, path, title, status, icon, theme_path, pixmap, menu, is_menu } => {
            let it = TrayItem { service: service.clone(), bus, path, title, status, icon, theme_path, pixmap: pixmap.map(std::rc::Rc::new), menu, is_menu };
            if let Some(x) = list.iter_mut().find(|i| i.service == service) {
                *x = it;
            } else {
                list.push(it);
            }
        }
    }
}

struct Watcher {
    items: Arc<Mutex<Vec<String>>>,
    tx: Sender<Msg>,
}

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    fn register_status_notifier_item(&self, service: &str, #[zbus(header)] hdr: zbus::message::Header<'_>) {
        let sender = hdr.sender().map(|s| s.to_string()).unwrap_or_default();
        let (bus, path) = if service.starts_with('/') { (sender, service.to_string()) } else { (service.to_string(), "/StatusNotifierItem".to_string()) };
        let key = format!("{bus}{path}");
        {
            let mut it = self.items.lock().unwrap();
            // потолок: процесс, регистрирующий значки в цикле, не забьёт бар и память
            if it.len() >= 64 && !it.contains(&key) {
                return;
            }
            if !it.contains(&key) {
                it.push(key.clone());
            }
        }
        let tx = self.tx.clone();
        std::thread::spawn(move || refresh_item(&bus, &path, &tx));
    }
    fn register_status_notifier_host(&self, _service: &str) {}
    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.items.lock().unwrap().clone()
    }
    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
}

fn split_service(s: &str) -> (String, String) {
    match s.find('/') {
        Some(i) => (s[..i].to_string(), s[i..].to_string()),
        None => (s.to_string(), "/StatusNotifierItem".to_string()),
    }
}

fn pixmap(v: Option<OwnedValue>) -> Option<Img> {
    let v = v?;
    let Value::Array(a) = &*v else { return None };
    // берём картинку ближе всего к 16 (но не меньше, если есть)
    let mut best: Option<(i32, i32, Vec<u8>)> = None;
    for e in a.inner() {
        let Value::Structure(s) = e else { continue };
        let f = s.fields();
        let (Some(Value::I32(w)), Some(Value::I32(h)), Some(Value::Array(d))) = (f.first(), f.get(1), f.get(2)) else { continue };
        let better = match &best {
            None => true,
            Some((bw, _, _)) => (*bw < 16 && *w > *bw) || (*w >= 16 && *w < *bw),
        };
        if better {
            best = Some((*w, *h, d.inner().iter().filter_map(|x| if let Value::U8(b) = x { Some(*b) } else { None }).collect()));
        }
    }
    let (w, h, d) = best?;
    // размеры от чужого процесса: не больше 1024² и без переполнения
    if !(1..=1024).contains(&w) || !(1..=1024).contains(&h) || d.len() < (w * h * 4) as usize {
        return None;
    }
    let mut img = image::RgbaImage::new(w as u32, h as u32);
    for (i, px) in d.chunks_exact(4).take((w * h) as usize).enumerate() {
        // ARGB32 в сетевом порядке
        img.put_pixel(i as u32 % w as u32, i as u32 / w as u32, image::Rgba([px[1], px[2], px[3], px[0]]));
    }
    Some(crate::icons::fit(img, 16))
}

fn refresh_item(bus: &str, path: &str, tx: &Sender<Msg>) {
    let Some(c) = CONN.get() else { return };
    let Ok(p) = Proxy::new(c, bus.to_string(), path.to_string(), "org.kde.StatusNotifierItem") else { return };
    let s = |k: &str| p.get_property::<String>(k).unwrap_or_default();
    let menu = p.get_property::<zbus::zvariant::OwnedObjectPath>("Menu").map(|m| m.to_string()).unwrap_or_default();
    let icon = s("IconName");
    let pm = if icon.is_empty() { pixmap(p.get_property::<OwnedValue>("IconPixmap").ok()) } else { None };
    let theme = s("IconThemePath");
    let _ = tx.send(Msg::Tray(TrayEv::Upsert {
        service: format!("{bus}{path}"),
        bus: bus.into(),
        path: path.into(),
        title: { let t = s("Title"); if t.is_empty() { s("Id") } else { t } },
        status: s("Status"),
        icon,
        theme_path: if theme.is_empty() { None } else { Some(theme) },
        pixmap: pm,
        menu,
        is_menu: p.get_property::<bool>("ItemIsMenu").unwrap_or(false),
    }));
}

/// ЛКМ / СКМ / колесо по значку трея
pub fn tray_call(bus: &str, path: &str, method: &'static str, x: i32, y: i32) {
    let (bus, path) = (bus.to_string(), path.to_string());
    std::thread::spawn(move || {
        let Some(c) = CONN.get() else { return };
        if let Ok(p) = Proxy::new(c, bus, path, "org.kde.StatusNotifierItem") {
            let _ = match method {
                "Scroll" => p.call_method("Scroll", &(x, "vertical")),
                m => p.call_method(m, &(x, y)),
            };
        }
    });
}

fn menu_item(v: &Value, svc: &str, mpath: &str) -> Option<MItem> {
    let v = match v {
        Value::Value(b) => b.as_ref(),
        x => x,
    };
    let Value::Structure(s) = v else { return None };
    let f = s.fields();
    let Some(Value::I32(id)) = f.first() else { return None };
    let mut props: HashMap<String, Value> = HashMap::new();
    if let Some(Value::Dict(d)) = f.get(1) {
        for (k, val) in d.iter() {
            if let Value::Str(k) = k {
                let val = match val {
                    Value::Value(b) => b.as_ref().try_clone().ok()?,
                    x => x.try_clone().ok()?,
                };
                props.insert(k.to_string(), val);
            }
        }
    }
    let st = |k: &str| match props.get(k) {
        Some(Value::Str(s)) => s.to_string(),
        _ => String::new(),
    };
    let bl = |k: &str, d: bool| match props.get(k) {
        Some(Value::Bool(b)) => *b,
        _ => d,
    };
    if !bl("visible", true) {
        return None;
    }
    if st("type") == "separator" {
        return Some(MItem::sep());
    }
    let mut children = vec![];
    if let Some(Value::Array(a)) = f.get(2) {
        for c in a.inner() {
            if let Some(m) = menu_item(c, svc, mpath) {
                children.push(m);
            }
        }
    }
    let check = if st("toggle-type").is_empty() {
        None
    } else {
        Some(matches!(props.get("toggle-state"), Some(Value::I32(1))))
    };
    // «_Файл» → «Файл»
    let label = st("label").replace("__", "\u{1}").replace('_', "").replace('\u{1}', "_");
    Some(MItem {
        label,
        icon: st("icon-name"),
        check,
        enabled: bl("enabled", true),
        sep: false,
        danger: false,
        act: Some(MAct::Tray(svc.to_string(), mpath.to_string(), *id)),
        children,
    })
}

/// ПКМ по значку: dbusmenu → пиксельное меню
pub fn tray_menu(service: &str, bus: &str, menu: &str, tx: Sender<Msg>) {
    let (svc, bus, menu) = (service.to_string(), bus.to_string(), menu.to_string());
    std::thread::spawn(move || {
        let Some(c) = CONN.get() else { return };
        let Ok(p) = Proxy::new(c, bus.clone(), menu.clone(), "com.canonical.dbusmenu") else { return };
        let _ = p.call_method("AboutToShow", &(0i32,));
        let Ok(m) = p.call_method("GetLayout", &(0i32, -1i32, Vec::<String>::new())) else { return };
        let Ok((_rev, (_, _, kids))): Result<(u32, (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>)), _> = m.body().deserialize() else { return };
        let key = format!("{bus}|{menu}");
        let items: Vec<MItem> = kids.iter().filter_map(|k| menu_item(k, &key, &menu)).collect();
        let _ = tx.send(Msg::TrayMenu(svc, items));
    });
}

pub fn tray_menu_click(svc: &str, path: &str, id: i32) {
    let bus = svc.split('|').next().unwrap_or("").to_string();
    let path = path.to_string();
    std::thread::spawn(move || {
        let Some(c) = CONN.get() else { return };
        if let Ok(p) = Proxy::new(c, bus, path, "com.canonical.dbusmenu") {
            let ts = 0u32;
            let _ = p.call_method("Event", &(id, "clicked", Value::from(0i32), ts));
        }
    });
}

/// notif, tray: шелл — сервер уведомлений и хозяин трея (настройки → службы); выключенное
/// отдаётся другим (mako, dunst, waybar)
pub fn spawn(tx: Sender<Msg>, notif: bool, tray: bool) {
    std::thread::Builder::new()
        .name("dbus".into())
        .spawn(move || {
            let Ok(c) = Connection::session() else {
                eprintln!("{}", crate::tf!("dbus: нет сессионной шины", "dbus: no session bus"));
                return;
            };
            let _ = CONN.set(c.clone());
            use zbus::fdo::RequestNameFlags;
            if notif {
                let next = Arc::new(AtomicU32::new(1));
                let _ = c.object_server().at("/org/freedesktop/Notifications", Notif { tx: tx.clone(), next });
                let flags = RequestNameFlags::ReplaceExisting | RequestNameFlags::AllowReplacement;
                if let Err(e) = c.request_name_with_flags("org.freedesktop.Notifications", flags) {
                    eprintln!("{}", crate::tf!("dbus: уведомления заняты другим сервером: {e}", "dbus: notifications are owned by another server: {e}"));
                }
            }
            if !tray {
                // соединение живёт в CONN: меню трея и сигналы уведомлений ходят через него
                return;
            }
            // трей: свой наблюдатель, а если он уже есть — подключаемся к нему хостом
            let items = Arc::new(Mutex::new(Vec::<String>::new()));
            let _ = c.object_server().at("/StatusNotifierWatcher", Watcher { items: items.clone(), tx: tx.clone() });
            let own = c.request_name_with_flags("org.kde.StatusNotifierWatcher", RequestNameFlags::DoNotQueue.into()).is_ok();
            let host = format!("org.kde.StatusNotifierHost-{}", std::process::id());
            let _ = c.request_name(host.as_str());
            if !own {
                if let Ok(p) = Proxy::new(&c, "org.kde.StatusNotifierWatcher", "/StatusNotifierWatcher", "org.kde.StatusNotifierWatcher") {
                    let _ = p.call_method("RegisterStatusNotifierHost", &(host.as_str(),));
                    for s in p.get_property::<Vec<String>>("RegisteredStatusNotifierItems").unwrap_or_default() {
                        let (b, pa) = split_service(&s);
                        refresh_item(&b, &pa, &tx);
                    }
                }
            }
            // сигналы: значки меняются, программы уходят
            let rule = zbus::MatchRule::builder().msg_type(zbus::message::Type::Signal).build();
            let Ok(it) = MessageIterator::for_match_rule(rule, &c, Some(256)) else { return };
            for m in it {
                let Ok(m) = m else { continue };
                let h = m.header();
                let iface = h.interface().map(|i| i.to_string()).unwrap_or_default();
                let member = h.member().map(|i| i.to_string()).unwrap_or_default();
                let sender = h.sender().map(|s| s.to_string()).unwrap_or_default();
                let path = h.path().map(|p| p.to_string()).unwrap_or_default();
                match (iface.as_str(), member.as_str()) {
                    ("org.kde.StatusNotifierItem", _) => {
                        let key = format!("{sender}{path}");
                        let known = items.lock().unwrap().contains(&key) || !own;
                        if known {
                            refresh_item(&sender, &path, &tx);
                        }
                    }
                    ("org.kde.StatusNotifierWatcher", "StatusNotifierItemRegistered") if !own => {
                        if let Ok(s) = m.body().deserialize::<String>() {
                            let (b, pa) = split_service(&s);
                            refresh_item(&b, &pa, &tx);
                        }
                    }
                    ("org.kde.StatusNotifierWatcher", "StatusNotifierItemUnregistered") if !own => {
                        if let Ok(s) = m.body().deserialize::<String>() {
                            let (b, pa) = split_service(&s);
                            let _ = tx.send(Msg::Tray(TrayEv::Remove(format!("{b}{pa}"))));
                        }
                    }
                    ("org.freedesktop.DBus", "NameOwnerChanged") => {
                        if let Ok((name, _old, new)) = m.body().deserialize::<(String, String, String)>() {
                            if new.is_empty() {
                                let gone: Vec<String> = {
                                    let mut it = items.lock().unwrap();
                                    let g: Vec<String> = it.iter().filter(|k| k.starts_with(&format!("{name}/"))).cloned().collect();
                                    it.retain(|k| !g.contains(k));
                                    g
                                };
                                for k in gone {
                                    let _ = tx.send(Msg::Tray(TrayEv::Remove(k)));
                                }
                                if !own {
                                    let _ = tx.send(Msg::Tray(TrayEv::Remove(format!("{name}/StatusNotifierItem"))));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        })
        .ok();
}
