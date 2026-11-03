// Слежка за открытыми папками через inotify: что-то создали, удалили, переименовали —
// окно перечитывает папку само (с задержкой, чтобы пачка событий дала одно перечитывание).

use std::collections::HashMap;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct Watch {
    fd: i32,
    wds: Arc<Mutex<HashMap<i32, PathBuf>>>,
}

impl Watch {
    pub fn spawn(send: impl Fn(PathBuf) + Send + 'static) -> Option<Watch> {
        let fd = unsafe { libc::inotify_init1(libc::IN_CLOEXEC) };
        if fd < 0 {
            return None;
        }
        let wds: Arc<Mutex<HashMap<i32, PathBuf>>> = Arc::new(Mutex::new(HashMap::new()));
        let w2 = wds.clone();
        std::thread::spawn(move || {
            let mut buf = vec![0u8; 16384];
            loop {
                let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
                if n <= 0 {
                    if n < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
                        continue;
                    }
                    break;
                }
                let mut off = 0usize;
                let mut hit: Vec<PathBuf> = vec![];
                while off + 16 <= n as usize {
                    let wd = i32::from_ne_bytes(buf[off..off + 4].try_into().unwrap());
                    let len = u32::from_ne_bytes(buf[off + 12..off + 16].try_into().unwrap()) as usize;
                    if let Some(p) = w2.lock().unwrap().get(&wd) {
                        if !hit.contains(p) {
                            hit.push(p.clone());
                        }
                    }
                    off += 16 + len;
                }
                for p in hit {
                    send(p);
                }
            }
        });
        Some(Watch { fd, wds })
    }

    /// следить ровно за этими папками
    pub fn set(&self, dirs: &[PathBuf]) {
        let mut m = self.wds.lock().unwrap();
        let old: Vec<(i32, PathBuf)> = m.iter().map(|(k, v)| (*k, v.clone())).collect();
        for (wd, p) in old {
            if !dirs.contains(&p) {
                unsafe { libc::inotify_rm_watch(self.fd, wd) };
                m.remove(&wd);
            }
        }
        for d in dirs {
            if m.values().any(|p| p == d) {
                continue;
            }
            let Ok(c) = CString::new(d.as_os_str().as_bytes()) else { continue };
            let mask = libc::IN_CREATE | libc::IN_DELETE | libc::IN_MOVED_FROM | libc::IN_MOVED_TO | libc::IN_CLOSE_WRITE | libc::IN_ATTRIB | libc::IN_DELETE_SELF | libc::IN_MOVE_SELF;
            let wd = unsafe { libc::inotify_add_watch(self.fd, c.as_ptr(), mask) };
            if wd >= 0 {
                m.insert(wd, d.clone());
            }
        }
    }
}
