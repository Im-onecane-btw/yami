// Картинки: список в папке, декодирование в нити (с соседями наперёд), правка (поворот,
// отражение, кадр), сохранение, корзина.

use crate::gfx::Img;
use image::{AnimationDecoder, RgbaImage};
use std::path::{Path, PathBuf};

pub const EXTS: &[&str] = &["png", "jpg", "jpeg", "jpe", "gif", "webp", "bmp", "ico", "tga", "tif", "tiff", "pnm", "ppm", "qoi"];

pub fn is_image(p: &Path) -> bool {
    p.extension().map(|e| EXTS.contains(&e.to_string_lossy().to_lowercase().as_str())).unwrap_or(false)
}

/// картинки папки по имени («естественный» порядок)
pub fn siblings(p: &Path) -> Vec<PathBuf> {
    let dir = if p.is_dir() { p.to_path_buf() } else { p.parent().map(|d| d.to_path_buf()).unwrap_or_default() };
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir).map(|r| r.flatten().map(|e| e.path()).filter(|p| p.is_file() && is_image(p)).collect()).unwrap_or_default();
    v.sort_by(|a, b| yami_px::paths::natural(&a.file_name().unwrap_or_default().to_string_lossy(), &b.file_name().unwrap_or_default().to_string_lossy()));
    v
}

pub struct Pic {
    pub path: PathBuf,
    /// кадры (у gif/webp может быть много) и их длительность, мс
    pub frames: Vec<(RgbaImage, u32)>,
    pub bytes: u64,
    pub format: String,
    pub edited: bool,
}

impl Pic {
    pub fn w(&self) -> u32 {
        self.frames.first().map(|f| f.0.width()).unwrap_or(0)
    }
    pub fn h(&self) -> u32 {
        self.frames.first().map(|f| f.0.height()).unwrap_or(0)
    }
    pub fn has_alpha(&self) -> bool {
        self.frames.first().map(|f| f.0.pixels().step_by(97).any(|p| p.0[3] < 250)).unwrap_or(false)
    }
}

/// кадры анимации с потолком: не больше 600 и 1 ГБ вместе (GIF 4000×4000 на тысячу кадров
/// иначе съел бы всю память раньше, чем мы их обрежем)
fn capped(fr: image::Frames<'_>) -> Result<Vec<image::Frame>, image::ImageError> {
    let mut out = vec![];
    let mut total = 0u64;
    for f in fr.take(600) {
        let f = f?;
        total += f.buffer().len() as u64;
        if total > 1 << 30 {
            break;
        }
        out.push(f);
    }
    Ok(out)
}

pub fn load(p: &Path) -> Result<Pic, String> {
    let bytes = std::fs::metadata(p).map(|m| m.len()).map_err(|e| e.to_string())?;
    let rd = image::ImageReader::open(p).map_err(|e| e.to_string())?.with_guessed_format().map_err(|e| e.to_string())?;
    let fmt = rd.format();
    let format = fmt.map(|f| format!("{f:?}").to_uppercase()).unwrap_or_default();
    // анимация
    let anim = |frames: Vec<image::Frame>| -> Vec<(RgbaImage, u32)> {
        frames.into_iter().take(600).map(|f| {
            let (n, d) = f.delay().numer_denom_ms();
            let ms = if d == 0 { 100 } else { (n / d).max(20) };
            (f.into_buffer(), ms)
        }).collect()
    };
    if fmt == Some(image::ImageFormat::Gif) {
        let f = std::io::BufReader::new(std::fs::File::open(p).map_err(|e| e.to_string())?);
        if let Ok(dec) = image::codecs::gif::GifDecoder::new(f) {
            if let Ok(fr) = capped(dec.into_frames()) {
                if !fr.is_empty() {
                    return Ok(Pic { path: p.into(), frames: anim(fr), bytes, format, edited: false });
                }
            }
        }
    }
    if fmt == Some(image::ImageFormat::WebP) {
        let f = std::io::BufReader::new(std::fs::File::open(p).map_err(|e| e.to_string())?);
        if let Ok(dec) = image::codecs::webp::WebPDecoder::new(f) {
            if dec.has_animation() {
                if let Ok(fr) = capped(dec.into_frames()) {
                    if !fr.is_empty() {
                        return Ok(Pic { path: p.into(), frames: anim(fr), bytes, format, edited: false });
                    }
                }
            }
        }
    }
    let mut rd = rd;
    let mut lim = image::Limits::default();
    lim.max_alloc = Some(1 << 30);
    rd.limits(lim);
    let img = rd.decode().map_err(|e| e.to_string())?;
    Ok(Pic { path: p.into(), frames: vec![(img.to_rgba8(), 0)], bytes, format, edited: false })
}

/// RGBA → предумноженные ARGB для холста
pub fn to_img(r: &RgbaImage) -> Img {
    crate::icons::from_rgba(r)
}

/// уменьшенная копия (сглаженная) — когда картинка больше окна
pub fn scaled(r: &RgbaImage, w: u32, h: u32) -> Img {
    to_img(&image::imageops::resize(r, w.max(1), h.max(1), image::imageops::FilterType::Triangle))
}

pub fn save(pic: &Pic, to: &Path) -> Result<(), String> {
    let f = &pic.frames.first().ok_or("пусто")?.0;
    let ext = to.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let tmp = to.with_extension(format!("{ext}.tmp~"));
    let r = if ext == "jpg" || ext == "jpeg" {
        // jpeg без альфы, качество 92
        let rgb = image::DynamicImage::ImageRgba8(f.clone()).to_rgb8();
        std::fs::File::create(&tmp).map_err(|e| e.to_string()).and_then(|file| {
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(file), 92);
            enc.encode_image(&rgb).map_err(|e| e.to_string())
        })
    } else {
        let fmt = image::ImageFormat::from_extension(&ext).unwrap_or(image::ImageFormat::Png);
        f.save_with_format(&tmp, fmt).map_err(|e| e.to_string())
    };
    r?;
    std::fs::rename(&tmp, to).map_err(|e| e.to_string())
}

/// в корзину freedesktop (домашняя; файл на другом диске — в .Trash-uid его точки монтирования)
pub fn trash(p: &Path) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    let home_t = crate::paths::data_dir().join("Trash");
    let same = std::fs::metadata(crate::paths::data_dir()).map(|m| m.dev()).ok() == std::fs::metadata(p).map(|m| m.dev()).ok();
    let t = if same {
        home_t
    } else {
        let dev = std::fs::metadata(p).map_err(|e| e.to_string())?.dev();
        let mut top = p.to_path_buf();
        while let Some(par) = top.parent() {
            if std::fs::metadata(par).map(|m| m.dev()).ok() != Some(dev) {
                break;
            }
            top = par.to_path_buf();
        }
        top.join(format!(".Trash-{}", unsafe { libc::getuid() }))
    };
    std::fs::create_dir_all(t.join("files")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(t.join("info")).map_err(|e| e.to_string())?;
    let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut n = 1;
    let mut target = name.clone();
    while t.join("files").join(&target).exists() || t.join("info").join(format!("{target}.trashinfo")).exists() {
        n += 1;
        target = format!("{n}-{name}");
    }
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    unsafe { libc::localtime_r(&now, &mut tm) };
    let enc: String = p.to_string_lossy().bytes().map(|b| if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
    let info = format!("[Trash Info]\nPath={enc}\nDeletionDate={:04}-{:02}-{:02}T{:02}:{:02}:{:02}\n", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec);
    std::fs::write(t.join("info").join(format!("{target}.trashinfo")), info).map_err(|e| e.to_string())?;
    std::fs::rename(p, t.join("files").join(&target)).map_err(|e| e.to_string())
}

pub fn human(n: u64) -> String {
    let u = if crate::i18n::translated() { ["B", "KB", "MB", "GB"] } else { ["Б", "КБ", "МБ", "ГБ"] }; // i18n-ok
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < 3 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 { format!("{n} {}", u[0]) } else { format!("{v:.1} {}", u[i]) }
}
