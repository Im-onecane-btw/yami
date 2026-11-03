// Бэкенд xdg-desktop-portal: org.freedesktop.impl.portal.ScreenCast. OBS, Discord/Equibop,
// браузеры просят показ экрана — окно выбора рисует шелл (пиксельное, `pick-screen` по его
// сокету), а сам поток делает niri через свой Mutter ScreenCast API. PipeWire-удалёнку
// приложению открывает уже сам xdg-desktop-portal по номерам узлов из ответа Start.
// Шелла нет — показываем экран в фокусе, без вопросов.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex, OnceLock};
use zbus::blocking::{Connection, MessageIterator, Proxy};
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

pub static CONN: OnceLock<Connection> = OnceLock::new();

const MUTTER: &str = "org.gnome.Mutter.ScreenCast";

/// портал: 1 — монитор, 2 — окно; курсор: 1 — скрыт, 2 — в кадре, 4 — отдельно (метаданные)
const SOURCES: u32 = 1 | 2;
const CURSORS: u32 = 1 | 2 | 4;

#[derive(Clone, Default)]
struct Sess {
    types: u32,
    cursor: u32,
    /// сеанс у niri (после Start)
    mutter: Option<OwnedObjectPath>,
}

type Sessions = Arc<Mutex<HashMap<String, Sess>>>;

pub struct ScreenCast {
    sessions: Sessions,
}

impl ScreenCast {
    pub fn new() -> ScreenCast {
        ScreenCast { sessions: Arc::new(Mutex::new(HashMap::new())) }
    }
}

/// объект сеанса портала: приложение закрывает его — останавливаем поток у niri
struct Session {
    sessions: Sessions,
    key: String,
}

#[zbus::interface(name = "org.freedesktop.impl.portal.Session")]
impl Session {
    async fn close(&self, #[zbus(object_server)] server: &zbus::ObjectServer) {
        let s = self.sessions.lock().unwrap().remove(&self.key);
        if let Some(m) = s.and_then(|s| s.mutter) {
            blocking::unblock(move || stop(&m)).await;
        }
        let _ = server.remove::<Session, _>(self.key.as_str()).await;
    }

    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        1
    }
}

fn stop(m: &OwnedObjectPath) {
    if let Some(c) = CONN.get() {
        if let Ok(p) = Proxy::new(c, MUTTER, m.as_str(), "org.gnome.Mutter.ScreenCast.Session") {
            let _ = p.call_method("Stop", &());
        }
    }
}

fn opt_u32(o: &HashMap<String, OwnedValue>, k: &str) -> Option<u32> {
    o.get(k).and_then(|v| u32::try_from(v).ok())
}

/// выбор в шелле: «monitor DP-1», «window 42» или None (отмена)
fn ask_shell(types: u32) -> Option<String> {
    let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));
    match std::os::unix::net::UnixStream::connect(std::path::Path::new(&dir).join("yami-shell.sock")) {
        Ok(mut s) => {
            writeln!(s, "pick-screen {types}").ok()?;
            let mut line = String::new();
            BufReader::new(s).read_line(&mut line).ok()?;
            let line = line.trim().to_string();
            (line.starts_with("monitor ") || line.starts_with("window ")).then_some(line)
        }
        // шелла нет: экран в фокусе
        Err(_) => focused_output().map(|o| format!("monitor {o}")),
    }
}

fn focused_output() -> Option<String> {
    let sock = std::env::var("NIRI_SOCKET").ok()?;
    let mut s = std::os::unix::net::UnixStream::connect(sock).ok()?;
    writeln!(s, "\"FocusedOutput\"").ok()?;
    let mut line = String::new();
    BufReader::new(s).read_line(&mut line).ok()?;
    let v: serde_json::Value = serde_json::from_str(&line).ok()?;
    v.pointer("/Ok/FocusedOutput/name").and_then(|n| n.as_str()).map(String::from)
}

struct Started {
    mutter: OwnedObjectPath,
    node: u32,
    kind: u32,
    pos: Option<(i32, i32)>,
    size: Option<(i32, i32)>,
}

/// поток у niri: сеанс Mutter, запись монитора или окна, номер узла PipeWire
fn record(pick: &str, cursor: u32) -> Option<Started> {
    let c = CONN.get()?;
    let sc = Proxy::new(c, MUTTER, "/org/gnome/Mutter/ScreenCast", MUTTER).ok()?;
    let sess: OwnedObjectPath = sc.call("CreateSession", &(HashMap::<&str, Value>::new(),)).ok()?;
    let sp = Proxy::new(c, MUTTER, sess.to_string(), "org.gnome.Mutter.ScreenCast.Session").ok()?;
    // курсор портала → курсор Mutter: 1 скрыт → 0, 2 в кадре → 1, 4 метаданные → 2
    let cm: u32 = match cursor {
        1 => 0,
        4 => 2,
        _ => 1,
    };
    let (kind, rest) = pick.split_once(' ')?;
    let try_record = |cm: u32| -> Option<OwnedObjectPath> {
        let mut props: HashMap<&str, Value> = HashMap::new();
        props.insert("cursor-mode", Value::from(cm));
        if kind == "window" {
            props.insert("window-id", Value::from(rest.parse::<u64>().ok()?));
            sp.call("RecordWindow", &(props,)).ok()
        } else {
            sp.call("RecordMonitor", &(rest, props)).ok()
        }
    };
    // метаданные курсора умеют не все — тогда курсор в кадре
    let stream = try_record(cm).or_else(|| if cm == 2 { try_record(1) } else { None });
    let Some(stream) = stream else {
        let _ = sp.call_method("Stop", &());
        return None;
    };
    // сигнал о номере узла слушаем до Start, ждём не дольше 5 с
    let rule = zbus::MatchRule::builder().msg_type(zbus::message::Type::Signal).interface("org.gnome.Mutter.ScreenCast.Stream").ok()?.member("PipeWireStreamAdded").ok()?.path(stream.as_str()).ok()?.build();
    let it = MessageIterator::for_match_rule(rule, c, Some(4)).ok()?;
    let (ntx, nrx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for m in it {
            if let Ok(m) = m {
                if let Ok(n) = m.body().deserialize::<u32>() {
                    let _ = ntx.send(n);
                    return;
                }
            }
        }
    });
    if sp.call_method("Start", &()).is_err() {
        return None;
    }
    let node = nrx.recv_timeout(std::time::Duration::from_secs(5)).ok();
    let Some(node) = node else {
        let _ = sp.call_method("Stop", &());
        return None;
    };
    let st = Proxy::new(c, MUTTER, stream.to_string(), "org.gnome.Mutter.ScreenCast.Stream").ok()?;
    let params: HashMap<String, OwnedValue> = st.get_property("Parameters").unwrap_or_default();
    let pair = |k: &str| params.get(k).and_then(|v| <(i32, i32)>::try_from(v.try_clone().ok()?).ok());
    Some(Started { mutter: sess, node, kind: if kind == "window" { 2 } else { 1 }, pos: pair("position"), size: pair("size") })
}

#[zbus::interface(name = "org.freedesktop.impl.portal.ScreenCast")]
impl ScreenCast {
    async fn create_session(&self, _handle: ObjectPath<'_>, session_handle: ObjectPath<'_>, _app_id: &str, _options: HashMap<String, OwnedValue>, #[zbus(object_server)] server: &zbus::ObjectServer) -> (u32, HashMap<String, OwnedValue>) {
        let key = session_handle.to_string();
        self.sessions.lock().unwrap().insert(key.clone(), Sess { types: 1, cursor: 2, mutter: None });
        if server.at(key.as_str(), Session { sessions: self.sessions.clone(), key: key.clone() }).await.is_err() {
            return (2, HashMap::new());
        }
        let mut r = HashMap::new();
        if let Ok(v) = OwnedValue::try_from(Value::from(key)) {
            r.insert("session_id".to_string(), v);
        }
        (0, r)
    }

    async fn select_sources(&self, _handle: ObjectPath<'_>, session_handle: ObjectPath<'_>, _app_id: &str, options: HashMap<String, OwnedValue>) -> (u32, HashMap<String, OwnedValue>) {
        let mut all = self.sessions.lock().unwrap();
        let Some(s) = all.get_mut(session_handle.as_str()) else { return (2, HashMap::new()) };
        s.types = opt_u32(&options, "types").map(|t| t & SOURCES).filter(|t| *t != 0).unwrap_or(1);
        s.cursor = opt_u32(&options, "cursor_mode").filter(|c| CURSORS & c != 0).unwrap_or(2);
        (0, HashMap::new())
    }

    async fn start(&self, _handle: ObjectPath<'_>, session_handle: ObjectPath<'_>, _app_id: &str, _parent_window: &str, _options: HashMap<String, OwnedValue>) -> (u32, HashMap<String, OwnedValue>) {
        let key = session_handle.to_string();
        let Some(s) = self.sessions.lock().unwrap().get(&key).cloned() else { return (2, HashMap::new()) };
        let res = blocking::unblock(move || {
            let pick = ask_shell(s.types)?;
            Some(record(&pick, s.cursor))
        })
        .await;
        let started = match res {
            None => return (1, HashMap::new()),
            Some(None) => return (2, HashMap::new()),
            Some(Some(st)) => st,
        };
        if let Some(s) = self.sessions.lock().unwrap().get_mut(&key) {
            s.mutter = Some(started.mutter.clone());
        }
        let mut props: HashMap<String, OwnedValue> = HashMap::new();
        let mut put = |k: &str, v: Value| {
            if let Ok(v) = OwnedValue::try_from(v) {
                props.insert(k.to_string(), v);
            }
        };
        put("source_type", Value::from(started.kind));
        if let Some(p) = started.pos {
            put("position", Value::from(p));
        }
        if let Some(sz) = started.size {
            put("size", Value::from(sz));
        }
        let streams = vec![(started.node, props)];
        let mut r = HashMap::new();
        if let Ok(v) = OwnedValue::try_from(Value::from(streams)) {
            r.insert("streams".to_string(), v);
        }
        if let Ok(v) = OwnedValue::try_from(Value::from(0u32)) {
            r.insert("persist_mode".to_string(), v);
        }
        (0, r)
    }

    #[zbus(property)]
    fn available_source_types(&self) -> u32 {
        SOURCES
    }

    #[zbus(property)]
    fn available_cursor_modes(&self) -> u32 {
        CURSORS
    }

    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        5
    }
}
