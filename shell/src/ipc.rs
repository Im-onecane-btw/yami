// Команды шеллу: `yami-shell msg launcher` → сокет $XDG_RUNTIME_DIR/yami-shell.sock.
// Ими пользуются бинды niri. Второй экземпляр шелла сюда же и упирается — и выходит.
// Безопасность: сокет 0600, говорим только с процессами своего пользователя (SO_PEERCRED),
// строка не длиннее 4 КБ и ждём её не дольше 2 с — зависший клиент не заткнёт шелл.

use crate::app::Msg;
use calloop::channel::Sender;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

pub fn path() -> PathBuf {
    let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));
    PathBuf::from(dir).join("yami-shell.sock")
}

pub fn send(cmd: &str) -> bool {
    match UnixStream::connect(path()) {
        Ok(mut s) => {
            let _ = writeln!(s, "{cmd}");
            let mut r = String::new();
            let _ = BufReader::new(s).read_line(&mut r);
            true
        }
        Err(_) => false,
    }
}

pub fn serve(tx: Sender<Msg>) {
    let p = path();
    if UnixStream::connect(&p).is_ok() {
        eprintln!("{}", crate::tf!("yami-shell уже запущен", "yami-shell is already running"));
        std::process::exit(0);
    }
    let _ = std::fs::remove_file(&p);
    let Ok(l) = UnixListener::bind(&p) else {
        eprintln!("{}", crate::tf!("ipc: не создать {}", "ipc: cannot create {}", p.display()));
        return;
    };
    let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
    std::thread::Builder::new()
        .name("ipc".into())
        .spawn(move || {
            for s in l.incoming().flatten() {
                if !same_user(&s) {
                    continue;
                }
                let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(2)));
                let mut line = String::new();
                if BufReader::new((&s).take(4096)).read_line(&mut line).is_err() {
                    continue;
                }
                let _ = s.set_read_timeout(None);
                let line = line.trim();
                // портал ждёт выбора экрана: соединение уходит шеллу, ответ — когда выберут
                if let Some(t) = line.strip_prefix("pick-screen") {
                    let _ = tx.send(Msg::ScreenPick(t.trim().parse().unwrap_or(3), s));
                    continue;
                }
                if !line.is_empty() {
                    let _ = tx.send(Msg::Ipc(line.to_string()));
                }
                let _ = (&s).write_all(b"ok\n");
            }
        })
        .ok();
}

/// собеседник — процесс того же пользователя
fn same_user(s: &UnixStream) -> bool {
    let mut cred = libc::ucred { pid: 0, uid: u32::MAX, gid: 0 };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: getsockopt пишет не больше len байт в cred
    let r = unsafe { libc::getsockopt(s.as_raw_fd(), libc::SOL_SOCKET, libc::SO_PEERCRED, &mut cred as *mut _ as *mut libc::c_void, &mut len) };
    r == 0 && cred.uid == unsafe { libc::getuid() }
}
