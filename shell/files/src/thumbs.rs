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

fn cache_path(r: &Req) -> PathBuf {
    crate::icons::thumb_path(&r.path, r.mtime, r.size)
}

fn has(cmd: &str) -> bool {
    std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|d| d.join(cmd).is_file())).unwrap_or(false)
}

fn make(r: &Req) -> Option<Img> {
    let cp = cache_path(r);
    if let Ok(img) = image::open(&cp) {
        return Some(crate::icons::from_rgba(&img.to_rgba8()));
    }
    if !r.video {
        // картинки: общий с шеллом декод и кеш (yami-px)
        return crate::icons::thumb(&r.path, r.mtime, r.size).map(|t| crate::icons::from_rgba(&t));
    }
    if !has("ffmpegthumbnailer") {
        return None;
    }
    let _ = std::fs::create_dir_all(cp.parent()?);
    let ok = std::process::Command::new("ffmpegthumbnailer").arg("-i").arg(&r.path).arg("-o").arg(&cp).args(["-s", &r.size.to_string(), "-c", "png", "-q", "6"]).stderr(std::process::Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
    if !ok {
        return None;
    }
    let t = image::open(&cp).ok()?.thumbnail(r.size as u32, r.size as u32).to_rgba8();
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
            yami_px::trim_heap();
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
