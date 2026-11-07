// Разговор с greetd (его сокет — $GREETD_SOCK): длина u32 в порядке байт машины + JSON. Идёт в
// своей нити, окну шлёт события. Пароль собирается в запрос вручную и затирается после отправки:
// ни serde, ни лишние копии его не держат. Демо (--demo) отвечает так же, но без greetd.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::Receiver;

/// что окно просит
pub enum Ask {
    /// войти: имя, пароль на первый скрытый вопрос PAM (None — спросить), что запустить
    Login { user: String, pass: Option<String>, cmd: Vec<String>, env: Vec<String> },
    /// ответ на вопрос PAM (None — на сообщение, где отвечать нечего)
    Answer(Option<String>),
    Cancel,
}

/// что окно узнаёт
pub enum Ev {
    /// PAM спрашивает: hidden — скрытый ввод (тип «secret»)
    Prompt { hidden: bool, text: String },
    /// PAM сообщает (error — сообщение об ошибке)
    Info { text: String, error: bool },
    /// неверный пароль
    Failed,
    /// другая ошибка (учётка заблокирована, сеанс не запустился)
    Error(String),
    /// сеанс запущен: экран входа уходит
    Started,
    /// greetd недоступен
    Lost(String),
}

/// JSON-строка из текста: кавычки, обратная косая, управляющие
fn esc(out: &mut Vec<u8>, s: &str) {
    out.push(b'"');
    for c in s.chars() {
        match c {
            '"' => out.extend_from_slice(b"\\\""),
            '\\' => out.extend_from_slice(b"\\\\"),
            c if (c as u32) < 0x20 => out.extend_from_slice(format!("\\u{:04x}", c as u32).as_bytes()),
            c => {
                let mut b = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut b).as_bytes());
            }
        }
    }
    out.push(b'"');
}

fn wipe(v: &mut Vec<u8>) {
    // SAFETY: запись нулей в свой буфер
    unsafe { std::ptr::write_bytes(v.as_mut_ptr(), 0, v.len()) };
    v.clear();
}

struct Conn {
    s: UnixStream,
}

impl Conn {
    fn send(&mut self, mut body: Vec<u8>) -> std::io::Result<()> {
        // сразу нужного размера: при росте буфера старый (с паролем) ушёл бы в кучу нестёртым
        let mut msg = Vec::with_capacity(4 + body.len());
        msg.extend_from_slice(&(body.len() as u32).to_ne_bytes());
        msg.extend_from_slice(&body);
        let r = self.s.write_all(&msg);
        wipe(&mut msg);
        wipe(&mut body);
        r
    }
    fn recv(&mut self) -> std::io::Result<serde_json::Value> {
        let mut n = [0u8; 4];
        self.s.read_exact(&mut n)?;
        let n = u32::from_ne_bytes(n) as usize;
        if n > 1 << 20 {
            return Err(std::io::Error::other("greetd: reply too long"));
        }
        let mut b = vec![0u8; n];
        self.s.read_exact(&mut b)?;
        serde_json::from_slice(&b).map_err(std::io::Error::other)
    }
    fn create(&mut self, user: &str) -> std::io::Result<()> {
        let mut b = br#"{"type":"create_session","username":"#.to_vec();
        esc(&mut b, user);
        b.push(b'}');
        self.send(b)
    }
    fn answer(&mut self, resp: Option<&str>) -> std::io::Result<()> {
        // запас под экранирование (\u00XX — 6 байт на знак): буфер не растёт и не копируется
        let mut b = Vec::with_capacity(64 + 6 * resp.map_or(0, str::len));
        b.extend_from_slice(br#"{"type":"post_auth_message_response","response":"#);
        match resp {
            Some(r) => esc(&mut b, r),
            None => b.extend_from_slice(b"null"),
        }
        b.push(b'}');
        self.send(b)
    }
    fn start(&mut self, cmd: &[String], env: &[String]) -> std::io::Result<()> {
        let list = |b: &mut Vec<u8>, xs: &[String]| {
            b.push(b'[');
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    b.push(b',');
                }
                esc(b, x);
            }
            b.push(b']');
        };
        let mut b = br#"{"type":"start_session","cmd":"#.to_vec();
        list(&mut b, cmd);
        b.extend_from_slice(br#","env":"#);
        list(&mut b, env);
        b.push(b'}');
        self.send(b)
    }
    fn cancel(&mut self) {
        if self.send(br#"{"type":"cancel_session"}"#.to_vec()).is_ok() {
            let _ = self.recv();
        }
    }
}

/// нить greetd: ждёт просьб окна, ведёт вход до запуска сеанса или отказа
pub fn spawn(rx: Receiver<Ask>, tx: impl Fn(Ev) + Send + 'static) {
    std::thread::spawn(move || {
        let Some(path) = std::env::var_os("GREETD_SOCK") else {
            tx(Ev::Lost("GREETD_SOCK".into()));
            return;
        };
        let mut c = match UnixStream::connect(&path) {
            Ok(s) => Conn { s },
            Err(e) => {
                tx(Ev::Lost(e.to_string()));
                return;
            }
        };
        while let Ok(a) = rx.recv() {
            let Ask::Login { user, mut pass, cmd, env } = a else { continue };
            if let Err(e) = login(&mut c, &rx, &tx, &user, &mut pass, &cmd, &env) {
                tx(Ev::Lost(e.to_string()));
                return;
            }
            if let Some(p) = pass.as_mut() {
                yami_px::ui::wipe(p);
            }
        }
    });
}

fn login(c: &mut Conn, rx: &Receiver<Ask>, tx: &impl Fn(Ev), user: &str, pass: &mut Option<String>, cmd: &[String], env: &[String]) -> std::io::Result<()> {
    c.create(user)?;
    let mut started = false;
    loop {
        let r = c.recv()?;
        let s = |k: &str| r.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
        match s("type").as_str() {
            "success" if started => {
                tx(Ev::Started);
                return Ok(());
            }
            "success" => {
                c.start(cmd, env)?;
                started = true;
            }
            "error" => {
                c.cancel();
                if s("error_type") == "auth_error" {
                    tx(Ev::Failed);
                } else {
                    tx(Ev::Error(s("description")));
                }
                return Ok(());
            }
            "auth_message" => {
                let text = s("auth_message");
                match s("auth_message_type").as_str() {
                    // первый скрытый вопрос — пароль, уже введённый в поле
                    "secret" if pass.is_some() => {
                        let mut p = pass.take().unwrap_or_default();
                        let r = c.answer(Some(&p));
                        yami_px::ui::wipe(&mut p);
                        r?;
                    }
                    t @ ("secret" | "visible") => {
                        tx(Ev::Prompt { hidden: t == "secret", text });
                        match rx.recv() {
                            Ok(Ask::Answer(mut a)) => {
                                let r = c.answer(a.as_deref());
                                if let Some(a) = a.as_mut() {
                                    yami_px::ui::wipe(a);
                                }
                                r?;
                            }
                            _ => {
                                c.cancel();
                                return Ok(());
                            }
                        }
                    }
                    t => {
                        tx(Ev::Info { text, error: t == "error" });
                        c.answer(None)?;
                    }
                }
            }
            _ => {
                c.cancel();
                tx(Ev::Error(String::new()));
                return Ok(());
            }
        }
    }
}

/// демо без greetd: пароль «yami» впускает, второй вопрос не задаётся
pub fn spawn_demo(rx: Receiver<Ask>, tx: impl Fn(Ev) + Send + 'static) {
    std::thread::spawn(move || {
        while let Ok(a) = rx.recv() {
            let Ask::Login { pass, .. } = a else { continue };
            std::thread::sleep(std::time::Duration::from_millis(400));
            match pass.as_deref() {
                Some("yami") => tx(Ev::Started),
                None => tx(Ev::Prompt { hidden: true, text: "Password:".into() }),
                _ => tx(Ev::Failed),
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    fn reply(s: &mut UnixStream, j: &str) {
        let mut m = (j.len() as u32).to_ne_bytes().to_vec();
        m.extend_from_slice(j.as_bytes());
        s.write_all(&m).unwrap();
    }
    fn read(s: &mut UnixStream) -> serde_json::Value {
        let mut n = [0u8; 4];
        s.read_exact(&mut n).unwrap();
        let mut b = vec![0u8; u32::from_ne_bytes(n) as usize];
        s.read_exact(&mut b).unwrap();
        serde_json::from_slice(&b).unwrap()
    }

    /// greetd понарошку: неверный пароль, потом верный со вторым фактором и запуском сеанса
    #[test]
    fn flow() {
        let dir = std::env::temp_dir().join(format!("yami-greet-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let sock = dir.join("g.sock");
        let l = UnixListener::bind(&sock).unwrap();
        std::env::set_var("GREETD_SOCK", &sock);
        let srv = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut seen = vec![];
            // 1: неверный пароль
            let r = read(&mut s);
            assert_eq!(r["type"], "create_session");
            assert_eq!(r["username"], "yami \"q\"");
            reply(&mut s, r#"{"type":"auth_message","auth_message_type":"secret","auth_message":"Password: "}"#);
            seen.push(read(&mut s)["response"].as_str().unwrap().to_string());
            reply(&mut s, r#"{"type":"error","error_type":"auth_error","description":"nope"}"#);
            assert_eq!(read(&mut s)["type"], "cancel_session");
            reply(&mut s, r#"{"type":"success"}"#);
            // 2: верный, сообщение, код, запуск
            assert_eq!(read(&mut s)["type"], "create_session");
            reply(&mut s, r#"{"type":"auth_message","auth_message_type":"secret","auth_message":"Password: "}"#);
            seen.push(read(&mut s)["response"].as_str().unwrap().to_string());
            reply(&mut s, r#"{"type":"auth_message","auth_message_type":"info","auth_message":"hello"}"#);
            assert!(read(&mut s)["response"].is_null());
            reply(&mut s, r#"{"type":"auth_message","auth_message_type":"visible","auth_message":"Code:"}"#);
            seen.push(read(&mut s)["response"].as_str().unwrap().to_string());
            reply(&mut s, r#"{"type":"success"}"#);
            let st = read(&mut s);
            assert_eq!(st["type"], "start_session");
            assert_eq!(st["cmd"][0], "niri-session");
            assert_eq!(st["env"][0], "XDG_SESSION_TYPE=wayland");
            reply(&mut s, r#"{"type":"success"}"#);
            seen
        });
        let (atx, arx) = std::sync::mpsc::channel();
        let (etx, erx) = std::sync::mpsc::channel();
        spawn(arx, move |e| {
            let _ = etx.send(e);
        });
        let login = |p: &str| Ask::Login { user: "yami \"q\"".into(), pass: Some(p.into()), cmd: vec!["niri-session".into()], env: vec!["XDG_SESSION_TYPE=wayland".into()] };
        atx.send(login("wr\"ong")).unwrap();
        assert!(matches!(erx.recv().unwrap(), Ev::Failed));
        atx.send(login("right")).unwrap();
        assert!(matches!(erx.recv().unwrap(), Ev::Info { ref text, error: false } if text == "hello"));
        assert!(matches!(erx.recv().unwrap(), Ev::Prompt { hidden: false, ref text } if text == "Code:"));
        atx.send(Ask::Answer(Some("123456".into()))).unwrap();
        assert!(matches!(erx.recv().unwrap(), Ev::Started));
        assert_eq!(srv.join().unwrap(), vec!["wr\"ong", "right", "123456"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
