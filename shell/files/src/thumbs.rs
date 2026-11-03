// Миниатюры в отдельной нити: картинки (png, jpeg) — сами, видео — через ffmpegthumbnailer,
// если он есть. Готовое кладётся в ~/.cache/yami/thumbs, чтобы второй раз не декодировать.
// Очередь — «последний пришёл, первый ушёл»: сначала то, что сейчас на экране.

use crate::gfx::Img;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};

pub struct Req {
    pub path: PathBuf,
    pub mtime: i64,
    pub size: i32,
    pub video: bool,
}

#[derive(Clone)]
pub struct Thumbs {
    q: Arc<(Mutex<Vec<Req>>, Condvar)>,
}

fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn cache_path(r: &Req) -> PathBuf {
    crate::paths::cache_dir().join("yami/thumbs").join(format!("{:016x}.png", fnv(&format!("{}|{}|{}", r.path.display(), r.mtime, r.size))))
}

fn has(cmd: &str) -> bool {
    std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|d| d.join(cmd).is_file())).unwrap_or(false)
}

fn make(r: &Req) -> Option<Img> {
    let cp = cache_path(r);
    if let Ok(img) = image::open(&cp) {
        return Some(crate::icons::from_rgba(&img.to_rgba8()));
    }
    let src = if r.video {
        if !has("ffmpegthumbnailer") {
            return None;
        }
        let _ = std::fs::create_dir_all(cp.parent()?);
        let ok = std::process::Command::new("ffmpegthumbnailer").arg("-i").arg(&r.path).arg("-o").arg(&cp).args(["-s", &r.size.to_string(), "-c", "png", "-q", "6"]).stderr(std::process::Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
        if !ok {
            return None;
        }
        image::open(&cp).ok()?
    } else {
        // гигантские картинки не декодируем целиком: память важнее миниатюры
        let rd = image::ImageReader::open(&r.path).ok()?.with_guessed_format().ok()?;
        let (w, h) = rd.into_dimensions().ok()?;
        if (w as u64) * (h as u64) > 40_000_000 {
            return None;
        }
        let mut rd = image::ImageReader::open(&r.path).ok()?.with_guessed_format().ok()?;
        let mut lim = image::Limits::default();
        lim.max_alloc = Some(256 << 20);
        rd.limits(lim);
        rd.decode().ok()?
    };
    let t = src.thumbnail(r.size as u32, r.size as u32).to_rgba8();
    if !r.video {
        let _ = std::fs::create_dir_all(cp.parent()?);
        let _ = t.save(&cp);
    }
    Some(crate::icons::from_rgba(&t))
}

impl Thumbs {
    pub fn spawn(send: impl Fn(PathBuf, i32, Option<Img>) + Send + 'static) -> Thumbs {
        let q: Arc<(Mutex<Vec<Req>>, Condvar)> = Arc::new((Mutex::new(vec![]), Condvar::new()));
        let q2 = q.clone();
        std::thread::spawn(move || loop {
            let r = {
                let (m, cv) = &*q2;
                let mut g = m.lock().unwrap();
                while g.is_empty() {
                    g = cv.wait(g).unwrap();
                }
                g.pop().unwrap()
            };
            let img = make(&r);
            send(r.path, r.size, img);
            // большие картинки декодировались — вернуть память системе
            unsafe { libc::malloc_trim(0) };
        });
        Thumbs { q }
    }

    pub fn ask(&self, r: Req) {
        let (m, cv) = &*self.q;
        let mut g = m.lock().unwrap();
        g.retain(|x| x.path != r.path);
        g.push(r);
        cv.notify_one();
    }

    /// сменилась папка: старые заявки больше не нужны
    pub fn clear(&self) {
        self.q.0.lock().unwrap().clear();
    }
}
