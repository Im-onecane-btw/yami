// Сеть и bluetooth для всплывашек: Wi-Fi через nmcli, bluetooth через BlueZ по системной
// шине (один запрос GetManagedObjects, дальше — по сигналам).

use crate::app::Msg;
use calloop::channel::Sender;
use std::collections::HashMap;
use std::process::Command;
use std::sync::OnceLock;
use zbus::blocking::{Connection, MessageIterator, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

#[derive(Clone, Default, Debug)]
pub struct WifiNet {
    pub ssid: String,
    pub signal: u8,
    pub secure: bool,
    pub active: bool,
    pub known: bool,
}

#[derive(Clone, Default, Debug)]
pub struct NetState {
    pub loaded: bool,
    pub wifi_present: bool,
    pub wifi_on: bool,
    pub nets: Vec<WifiNet>,
    pub active: Vec<(String, String, String)>,
    pub busy: bool,
    pub msg: String,
}

fn nmcli(args: &[&str]) -> String {
    Command::new("nmcli").args(args).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default()
}

/// разбор `nmcli -t`: двоеточия внутри полей экранированы обратной чертой
fn fields(l: &str) -> Vec<String> {
    let mut v = vec![String::new()];
    let mut esc = false;
    for c in l.chars() {
        if esc {
            v.last_mut().unwrap().push(c);
            esc = false;
        } else if c == '\\' {
            esc = true;
        } else if c == ':' {
            v.push(String::new());
        } else {
            v.last_mut().unwrap().push(c);
        }
    }
    v
}

pub fn query_wifi(rescan: bool) -> NetState {
    let mut s = NetState { loaded: true, ..Default::default() };
    s.wifi_on = nmcli(&["-t", "-f", "WIFI", "general"]).trim() == "enabled";
    s.wifi_present = nmcli(&["-t", "-f", "TYPE", "device"]).lines().any(|l| l == "wifi");
    for l in nmcli(&["-t", "-f", "NAME,TYPE,DEVICE", "connection", "show", "--active"]).lines() {
        let f = fields(l);
        if f.len() >= 3 && f[1] != "loopback" {
            let kind = match f[1].as_str() {
                "802-3-ethernet" => "кабель",
                "802-11-wireless" => "wi-fi",
                "wireguard" | "vpn" => "vpn",
                "bridge" => "мост",
                x => x,
            };
            s.active.push((f[0].clone(), kind.to_string(), f[2].clone()));
        }
    }
    if s.wifi_present && s.wifi_on {
        let known: Vec<String> = nmcli(&["-t", "-f", "NAME,TYPE", "connection", "show"]).lines().map(fields).filter(|f| f.get(1).map(|t| t == "802-11-wireless").unwrap_or(false)).map(|f| f[0].clone()).collect();
        let list = nmcli(&["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "device", "wifi", "list", "--rescan", if rescan { "yes" } else { "auto" }]);
        let mut seen: HashMap<String, usize> = HashMap::new();
        for l in list.lines() {
            let f = fields(l);
            if f.len() < 4 || f[1].is_empty() {
                continue;
            }
            let n = WifiNet { active: f[0] == "*", ssid: f[1].clone(), signal: f[2].parse().unwrap_or(0), secure: !f[3].is_empty() && f[3] != "--", known: known.contains(&f[1]) };
            if let Some(&i) = seen.get(&n.ssid) {
                if n.signal > s.nets[i].signal || n.active {
                    s.nets[i] = n;
                }
            } else {
                seen.insert(n.ssid.clone(), s.nets.len());
                s.nets.push(n);
            }
        }
        s.nets.sort_by(|a, b| b.active.cmp(&a.active).then(b.signal.cmp(&a.signal)));
        s.nets.truncate(12);
    }
    s
}

pub fn refresh_wifi(tx: Sender<Msg>) {
    std::thread::spawn(move || {
        let _ = tx.send(Msg::Net(query_wifi(false)));
    });
}

pub fn wifi_cmd(args: Vec<String>, tx: Sender<Msg>, mut cur: NetState) {
    cur.busy = true;
    cur.msg = "подключаюсь…".into();
    let _ = tx.send(Msg::Net(cur));
    std::thread::spawn(move || {
        let o = Command::new("nmcli").args(&args).output();
        let mut s = query_wifi(false);
        if let Ok(o) = o {
            if !o.status.success() {
                let e = String::from_utf8_lossy(&o.stderr).to_string();
                s.msg = if e.contains("Secrets were required") || e.contains("password") { "нужен пароль".into() } else { e.lines().last().unwrap_or("не вышло").chars().take(60).collect() };
            }
        }
        let _ = tx.send(Msg::Net(s));
    });
}

// ── bluetooth ───────────────────────────────────────────────────────────────
#[derive(Clone, Default, Debug)]
pub struct BtDev {
    pub path: String,
    pub name: String,
    pub icon: String,
    pub connected: bool,
    pub paired: bool,
    pub battery: Option<u8>,
}

#[derive(Clone, Default, Debug)]
pub struct BtState {
    pub present: bool,
    pub powered: bool,
    pub discovering: bool,
    pub adapter: String,
    pub devices: Vec<BtDev>,
}

static SYS: OnceLock<Connection> = OnceLock::new();

/// одно подключение к системной шине на весь шелл (bluez, polkit, профили питания)
pub fn system() -> Option<Connection> {
    static INIT: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = INIT.lock().ok()?;
    if let Some(c) = SYS.get() {
        return Some(c.clone());
    }
    let c = Connection::system().ok()?;
    let _ = SYS.set(c.clone());
    Some(c)
}

type Objects = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>>;

fn query_bt() -> BtState {
    let mut s = BtState::default();
    let Some(c) = SYS.get() else { return s };
    let Ok(p) = Proxy::new(c, "org.bluez", "/", "org.freedesktop.DBus.ObjectManager") else { return s };
    let Ok(m) = p.call_method("GetManagedObjects", &()) else { return s };
    let Ok(objs) = m.body().deserialize::<Objects>() else { return s };
    let b = |m: &HashMap<String, OwnedValue>, k: &str| matches!(m.get(k).map(|v| &**v), Some(Value::Bool(true)));
    let st = |m: &HashMap<String, OwnedValue>, k: &str| match m.get(k).map(|v| &**v) {
        Some(Value::Str(x)) => x.to_string(),
        _ => String::new(),
    };
    for (path, ifs) in &objs {
        if let Some(a) = ifs.get("org.bluez.Adapter1") {
            if s.adapter.is_empty() {
                s.present = true;
                s.adapter = path.to_string();
                s.powered = b(a, "Powered");
                s.discovering = b(a, "Discovering");
            }
        }
        if let Some(d) = ifs.get("org.bluez.Device1") {
            let name = { let n = st(d, "Alias"); if n.is_empty() { st(d, "Name") } else { n } };
            if name.is_empty() {
                continue;
            }
            let battery = ifs.get("org.bluez.Battery1").and_then(|bt| match bt.get("Percentage").map(|v| &**v) {
                Some(Value::U8(x)) => Some(*x),
                _ => None,
            });
            s.devices.push(BtDev { path: path.to_string(), name, icon: st(d, "Icon"), connected: b(d, "Connected"), paired: b(d, "Paired"), battery });
        }
    }
    s.devices.sort_by(|a, b| b.connected.cmp(&a.connected).then(b.paired.cmp(&a.paired)).then(a.name.cmp(&b.name)));
    s
}

pub fn refresh_bt(tx: Sender<Msg>) {
    std::thread::spawn(move || {
        let _ = tx.send(Msg::Bt(query_bt()));
    });
}

pub fn spawn_bt(tx: Sender<Msg>) {
    std::thread::Builder::new()
        .name("bluez".into())
        .spawn(move || {
            let Some(c) = system() else { return };
            let _ = tx.send(Msg::Bt(query_bt()));
            let Ok(rule) = zbus::MatchRule::builder().msg_type(zbus::message::Type::Signal).sender("org.bluez").map(|r| r.build()) else { return };
            let Ok(it) = MessageIterator::for_match_rule(rule, &c, Some(64)) else { return };
            let mut last = std::time::Instant::now();
            for m in it {
                if m.is_err() {
                    continue;
                }
                // пачки сигналов при поиске устройств — не чаще раза в 300 мс
                if last.elapsed().as_millis() < 300 {
                    continue;
                }
                last = std::time::Instant::now();
                let _ = tx.send(Msg::Bt(query_bt()));
            }
        })
        .ok();
}

pub fn bt_power(adapter: &str, on: bool, tx: Sender<Msg>) {
    let a = adapter.to_string();
    std::thread::spawn(move || {
        if let Some(c) = SYS.get() {
            if let Ok(p) = Proxy::new(c, "org.bluez", a, "org.freedesktop.DBus.Properties") {
                let _ = p.call_method("Set", &("org.bluez.Adapter1", "Powered", Value::from(on)));
            }
        }
        let _ = tx.send(Msg::Bt(query_bt()));
    });
}

pub fn bt_toggle_dev(path: &str, connected: bool, tx: Sender<Msg>) {
    let p = path.to_string();
    std::thread::spawn(move || {
        if let Some(c) = SYS.get() {
            if let Ok(px) = Proxy::new(c, "org.bluez", p, "org.bluez.Device1") {
                let _ = px.call_method(if connected { "Disconnect" } else { "Connect" }, &());
            }
        }
        let _ = tx.send(Msg::Bt(query_bt()));
    });
}

pub fn bt_scan(adapter: &str, tx: Sender<Msg>) {
    let a = adapter.to_string();
    std::thread::spawn(move || {
        let Some(c) = SYS.get() else { return };
        let Ok(p) = Proxy::new(c, "org.bluez", a, "org.bluez.Adapter1") else { return };
        let _ = p.call_method("StartDiscovery", &());
        let _ = tx.send(Msg::Bt(query_bt()));
        std::thread::sleep(std::time::Duration::from_secs(12));
        let _ = p.call_method("StopDiscovery", &());
        let _ = tx.send(Msg::Bt(query_bt()));
    });
}
