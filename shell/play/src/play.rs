// Состояние плеера и его интерфейс. mpv (app.rs) сообщает свойства сюда, ввод превращается в
// команды mpv (`Fx::Cmd`). Рисуется поверх кадра: верхняя полоса с названием, внизу — шкала
// времени из клеток, кнопки, громкость; всё прячется, пока видео идёт и мышь не двигается.

use crate::font::PX;
use crate::gfx::{BoxStyle, R};
use crate::ui::{Cur, Tone};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Block,
    Bg,
    Seek,
    Vol,
    Btn(&'static str),
}

impl yami_px::ui::Action for Act {
    fn block() -> Self {
        Act::Block
    }
}
pub type Ui<'a, 'b> = yami_px::ui::Ui<'a, 'b, Act>;

#[derive(Clone, Debug, PartialEq)]
pub enum Fx {
    Close,
    Fullscreen(bool),
    Cmd(Vec<String>),
    Set(&'static str, String),
}

/// что открывается: видео и музыка
pub const EXT: &[&str] = &[
    "mkv", "mp4", "m4v", "webm", "avi", "mov", "wmv", "flv", "mpg", "mpeg", "m2ts", "mts", "ts", "3gp", "3g2", "ogv", "vob", "divx", "rm", "rmvb", "asf", "f4v", "y4m", "mxf", "hevc", "h264", "av1", "ivf",
    "mp3", "flac", "ogg", "oga", "opus", "m4a", "m4b", "aac", "wav", "wma", "alac", "ape", "wv", "mka", "aiff", "aif", "dsf", "dff", "tta", "mpc", "spx", "ac3", "dts", "amr", "mid", "midi", "mod", "xm", "it", "s3m",
];

pub fn is_media(p: &Path) -> bool {
    p.extension().map(|e| EXT.contains(&e.to_string_lossy().to_lowercase().as_str())).unwrap_or(false)
}

/// видео и музыка той же папки, по имени (с числами по порядку: 2 < 10)
pub fn siblings(p: &Path) -> Vec<PathBuf> {
    let dir = if p.is_dir() { p.to_path_buf() } else { p.parent().map(|d| d.to_path_buf()).unwrap_or_default() };
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_file() && is_media(p)).collect()).unwrap_or_default();
    v.sort_by(|a, b| yami_px::paths::natural(&a.to_string_lossy(), &b.to_string_lossy()));
    v
}

const SPEEDS: &[f64] = &[0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 3.0, 4.0];

pub struct P {
    pub look: crate::look::Look,
    pub list: Vec<PathBuf>,
    pub idx: usize,
    pub title: String,
    pub artist: String,
    pub pos: f64,
    pub dur: f64,
    pub cache: f64,
    pub paused: bool,
    pub buffering: bool,
    pub volume: f64,
    pub mute: bool,
    pub speed: f64,
    pub looping: bool,
    pub pixel: bool,
    /// у файла есть картинка (видео или обложка)
    pub video: bool,
    pub loaded: bool,
    pub vw: i64,
    pub vh: i64,
    pub aid: String,
    pub sid: String,
    pub tracks: (i64, i64),
    /// поля вокруг кадра в пикселях буфера: сверху, снизу, слева, справа
    pub margins: [f64; 4],
    pub vcodec: String,
    pub acodec: String,
    pub hwdec: String,
    pub fps: f64,
    pub format: String,
    pub size: i64,
    pub err: Option<String>,
    pub info: bool,
    pub full: bool,
    pub toast: Option<(String, Instant)>,
    pub active_at: Instant,
    pub born: Instant,
    pub w: i32,
    pub h: i32,
    pub ptr: Option<(f64, f64)>,
    pub over_ui: bool,
    /// тянем шкалу времени: доля
    pub seek_drag: Option<f64>,
    pub fx: Vec<Fx>,
}

impl P {
    pub fn new(look: crate::look::Look, list: Vec<PathBuf>, idx: usize) -> P {
        let now = Instant::now();
        P {
            look,
            list,
            idx,
            title: String::new(),
            artist: String::new(),
            pos: 0.0,
            dur: 0.0,
            cache: 0.0,
            paused: false,
            buffering: false,
            volume: 100.0,
            mute: false,
            speed: 1.0,
            looping: false,
            pixel: false,
            video: false,
            loaded: false,
            vw: 0,
            vh: 0,
            aid: String::new(),
            sid: String::new(),
            tracks: (0, 0),
            margins: [0.0; 4],
            vcodec: String::new(),
            acodec: String::new(),
            hwdec: String::new(),
            fps: 0.0,
            format: String::new(),
            size: 0,
            err: None,
            info: false,
            full: false,
            toast: None,
            active_at: now,
            born: now,
            w: 960,
            h: 600,
            ptr: None,
            over_ui: false,
            seek_drag: None,
            fx: vec![],
        }
    }

    pub fn path(&self) -> Option<&PathBuf> {
        self.list.get(self.idx)
    }

    pub fn name(&self) -> String {
        if !self.title.is_empty() {
            return self.title.clone();
        }
        self.path().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "yami-play".into())
    }

    pub fn win_title(&self) -> String {
        format!("{} — yami-play", self.name())
    }

    pub fn toast(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), Instant::now()));
    }

    fn cmd(&mut self, a: &[&str]) {
        self.fx.push(Fx::Cmd(a.iter().map(|s| s.to_string()).collect()));
    }

    /// панели видны: пауза, мышь недавно двигалась, курсор над ними, нет картинки
    pub fn ui_visible(&self) -> bool {
        self.ui_held() || self.ui_recent()
    }

    /// панели держит не время: пауза, музыка, мышь над ними, перемотка, ошибка
    fn ui_held(&self) -> bool {
        self.paused || !self.video || self.over_ui || self.seek_drag.is_some() || self.err.is_some()
    }

    fn ui_recent(&self) -> bool {
        self.active_at.elapsed().as_millis() < 2200
    }

    /// панели видны, но вот-вот спрячутся сами: тику надо успеть (иначе он раз в секунду)
    pub fn hide_soon(&self) -> bool {
        !self.ui_held() && self.ui_recent()
    }

    pub fn seek_rel(&mut self, s: f64) {
        self.cmd(&["seek", &format!("{s}"), "relative"]);
        self.active_at = Instant::now();
    }

    pub fn seek_frac(&mut self, k: f64, exact: bool) {
        if self.dur > 0.0 {
            let t = (k.clamp(0.0, 1.0) * self.dur).min(self.dur - 0.1).max(0.0);
            self.cmd(&["seek", &format!("{t:.3}"), if exact { "absolute+exact" } else { "absolute+keyframes" }]);
            self.pos = t;
        }
    }

    pub fn set_volume(&mut self, v: f64) {
        let v = v.clamp(0.0, 130.0).round();
        self.volume = v;
        self.fx.push(Fx::Set("volume", format!("{v}")));
        if self.mute && v > 0.0 {
            self.fx.push(Fx::Set("mute", "no".into()));
        }
        self.toast(crate::tf!("громкость {}%", "volume {}%", "volume {}%", v));
    }

    pub fn go(&mut self, d: i32) {
        if self.list.len() > 1 {
            self.cmd(&[if d > 0 { "playlist-next" } else { "playlist-prev" }, "force"]);
        }
    }

    fn set_speed(&mut self, d: i32) {
        let i = SPEEDS.iter().position(|s| (*s - self.speed).abs() < 0.01).unwrap_or(3) as i32;
        let n = if d == 0 { 3 } else { (i + d).clamp(0, SPEEDS.len() as i32 - 1) };
        let s = SPEEDS[n as usize];
        self.fx.push(Fx::Set("speed", format!("{s}")));
        self.toast(crate::tf!("скорость ×{}", "speed ×{}", "velocidade ×{}", s));
    }

    pub fn btn(&mut self, a: &str) {
        self.active_at = Instant::now();
        match a {
            "close" => self.fx.push(Fx::Close),
            "play" => self.cmd(&["cycle", "pause"]),
            "prev" => self.go(-1),
            "next" => self.go(1),
            "back" => self.seek_rel(-5.0),
            "fwd" => self.seek_rel(5.0),
            "mute" => self.fx.push(Fx::Set("mute", if self.mute { "no" } else { "yes" }.into())),
            "vol+" => self.set_volume(self.volume + 5.0),
            "vol-" => self.set_volume(self.volume - 5.0),
            "faster" => self.set_speed(1),
            "slower" => self.set_speed(-1),
            "speed" => self.set_speed(if (self.speed - 1.0).abs() < 0.01 { 1 } else { 0 }),
            "audio" => {
                self.cmd(&["cycle", "aid"]);
                self.toast(crate::i18n::tr("следующая дорожка звука"));
            }
            "subs" => {
                self.cmd(&["cycle", "sid"]);
                self.toast(crate::i18n::tr("следующие субтитры"));
            }
            "loop" => {
                self.fx.push(Fx::Set("loop-file", if self.looping { "no" } else { "inf" }.into()));
                self.toast(if self.looping { crate::i18n::tr("повтор выключен") } else { crate::i18n::tr("повтор файла") });
            }
            "pixel" => {
                self.pixel = !self.pixel;
                let s = if self.pixel { "point" } else { "bilinear" };
                self.fx.push(Fx::Set("zimg-scaler", s.into()));
                self.fx.push(Fx::Set("sws-scaler", s.into()));
                self.toast(if self.pixel { crate::i18n::tr("пиксели: честные клетки") } else { crate::i18n::tr("пиксели: сглаженно") });
            }
            "shot" => {
                self.cmd(&["screenshot", "video"]);
                self.toast(crate::i18n::tr("кадр сохранён в «изображения»"));
            }
            "frame" => self.cmd(&["frame-step"]),
            "frameback" => self.cmd(&["frame-back-step"]),
            "info" => self.info = !self.info,
            "full" => self.fx.push(Fx::Fullscreen(!self.full)),
            "folder" => {
                if let Some(p) = self.path().cloned() {
                    spawn(&["yami-files".into(), p.to_string_lossy().to_string()]);
                }
            }
            "start" => self.seek_frac(0.0, true),
            _ => {}
        }
    }

    /// свойство mpv изменилось
    pub fn prop(&mut self, k: &str, v: crate::mpv::Val) {
        use crate::mpv::Val;
        let num = |v: &Val| if let Val::Num(x) = v { *x } else { 0.0 };
        let int = |v: &Val| if let Val::Int(x) = v { *x } else { 0 };
        let flag = |v: &Val| matches!(v, Val::Flag(true));
        let s = |v: &Val| if let Val::Str(x) = v { x.clone() } else { String::new() };
        match k {
            "time-pos" => self.pos = num(&v),
            "duration" => self.dur = num(&v),
            "demuxer-cache-time" => self.cache = num(&v),
            "pause" => self.paused = flag(&v),
            "paused-for-cache" => self.buffering = flag(&v),
            "volume" => self.volume = num(&v),
            "mute" => self.mute = flag(&v),
            "speed" => self.speed = num(&v),
            "loop-file" => self.looping = !matches!(s(&v).as_str(), "" | "no"),
            "media-title" => self.title = s(&v),
            "metadata/by-key/artist" => self.artist = s(&v),
            "vo-configured" => self.video = flag(&v),
            "dwidth" => self.vw = int(&v),
            "dheight" => self.vh = int(&v),
            "aid" => self.aid = s(&v),
            "sid" => self.sid = s(&v),
            "playlist-pos" => {
                let i = int(&v);
                if i >= 0 && (i as usize) < self.list.len() && i as usize != self.idx {
                    self.idx = i as usize;
                    self.err = None;
                }
            }
            "video-codec" => self.vcodec = s(&v),
            "audio-codec-name" => self.acodec = s(&v),
            "hwdec-current" => self.hwdec = s(&v),
            "container-fps" => self.fps = num(&v),
            "file-format" => self.format = s(&v),
            "file-size" => self.size = int(&v),
            "track-list/count" => self.tracks.0 = int(&v),
            "osd-dimensions/mt" => self.margins[0] = num(&v),
            "osd-dimensions/mb" => self.margins[1] = num(&v),
            "osd-dimensions/ml" => self.margins[2] = num(&v),
            "osd-dimensions/mr" => self.margins[3] = num(&v),
            _ => {}
        }
    }

    pub fn animating(&self) -> bool {
        self.toast.as_ref().map(|t| t.1.elapsed().as_millis() < 2600).unwrap_or(false) || self.born.elapsed().as_millis() < 400 || self.buffering
    }
}

pub fn spawn(argv: &[String]) {
    let _ = std::process::Command::new("setsid").arg("-f").args(argv).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
}

fn human(b: i64) -> String {
    let b = b.max(0) as f64;
    let u = ["Б", "КБ", "МБ", "ГБ", "ТБ"]; // i18n-ok: английские — ниже
    let ue = ["B", "KB", "MB", "GB", "TB"];
    let mut v = b;
    let mut i = 0;
    while v >= 1024.0 && i < 4 {
        v /= 1024.0;
        i += 1;
    }
    let unit = if crate::i18n::translated() { ue[i] } else { u[i] };
    if i == 0 {
        format!("{v:.0} {unit}")
    } else {
        format!("{v:.1} {unit}")
    }
}

fn ease(born: Instant, ms: f32, p: &P) -> f32 {
    let d = p.look.dur(ms);
    if d <= 0.0 {
        return 1.0;
    }
    let t = (born.elapsed().as_secs_f32() * 1000.0 / d).min(1.0);
    1.0 - (1.0 - t).powi(3)
}

/// интерфейс поверх кадра. `bg` — кадра нет (музыка без обложки, загрузка, ошибка): фон темы
/// true — на холсте что-то нарисовано (false: панели спрятаны, поверх кадра видео пусто)
pub fn draw(ui: &mut Ui, p: &P, w: i32, h: i32, bg: bool) -> bool {
    let t = ui.t;
    let mut drew = bg;
    if bg {
        ui.cv.clear(t.bg.alpha(t.glass.max(0.92)));
        let (icon, msg) = match &p.err {
            Some(e) => ("screen", crate::tf!("не открывается: {}", "cannot open: {}", "não foi possível abrir: {}", e)),
            None if !p.loaded => {
                let dots = ".".repeat(1 + (p.born.elapsed().as_millis() / 250 % 3) as usize);
                ui.anim = true;
                ("screen", format!("{}{dots}", crate::i18n::tr("открываю")))
            }
            None => ("music", p.artist.clone()),
        };
        let cell = (h / 60).clamp(4, 10);
        ui.cv.icon(icon, w / 2 - 7 * cell / 2, h / 2 - 7 * cell - 24, cell, t.border.lerp(t.red, 0.5));
        if p.loaded && p.err.is_none() {
            ui.cv.text_center(ui.f, PX, R::new(0, h / 2 + 4, w, 22), &p.name(), t.text);
        }
        ui.cv.text_center(ui.f, PX, R::new(0, h / 2 + 30, w, 20), &msg, t.text_faint);
    }
    ui.zone(R::new(0, 0, w, h), Act::Bg, Cur::Default);
    if p.ui_visible() {
        top(ui, p, w);
        bottom(ui, p, w, h);
        drew = true;
    } else if p.buffering {
        bottom_line(ui, p, w, h);
        drew = true;
    }
    if p.paused && p.video && p.err.is_none() {
        drew = true;
        let r = R::new(w / 2 - 34, h / 2 - 34, 68, 68);
        ui.cv.pxbox(r, &BoxStyle::new(t.panel.alpha(0.85), t.border.lerp(t.red, 0.5)).shadow(3));
        ui.cv.icon("play", r.x + 17, r.y + 17, 5, t.red);
    }
    if p.info {
        info(ui, p, w);
        drew = true;
    }
    if let Some((s, born)) = &p.toast {
        if born.elapsed().as_millis() < 2600 {
            drew = true;
            let tw = ui.w(PX, s).min(w - 60) + 36;
            let r = R::new((w - tw) / 2, 54, tw, 32);
            ui.cv.pxbox(r, &BoxStyle::new(t.panel.alpha(0.95), t.red).shadow(3));
            ui.cv.fill(R::new(r.x + 12, r.cy() - 3, 6, 6), t.red);
            ui.cv.text_in(ui.f, PX, R::new(r.x + 26, r.y, r.w - 32, r.h), s, t.text);
            ui.anim = true;
        }
    }
    let k = ease(p.born, 260.0, p);
    if k < 1.0 {
        ui.cv.dissolve(k, 2);
        ui.anim = true;
    }
    drew
}

fn top(ui: &mut Ui, p: &P, w: i32) {
    let t = ui.t;
    let r = R::new(0, 0, w, 34);
    ui.cv.fill(r, t.panel.alpha(0.88));
    ui.cv.fill(R::new(0, r.bottom() - 1, w, 1), t.border);
    ui.zone(r, Act::Block, Cur::Default);
    ui.cv.icon(if p.video { "screen" } else { "music" }, 10, 10, 2, t.red);
    let mut name = p.name();
    if !p.artist.is_empty() && !p.video {
        name = format!("{} — {}", p.artist, name);
    }
    let nw = ui.cv.text_in(ui.f, PX, R::new(32, 0, w * 2 / 3, 34), &name, t.text);
    if p.list.len() > 1 {
        ui.cv.text_in(ui.f, PX, R::new(32 + nw + 12, 0, 120, 34), &format!("{} / {}", p.idx + 1, p.list.len()), t.text_faint);
    }
    let bs = 26;
    ui.button(R::new(w - bs - 6, 4, bs, bs), "", "close", Tone::Ghost, false, true, Act::Btn("close"));
    if p.vw > 0 && p.video {
        ui.cv.text_right(ui.f, PX, R::new(0, 0, w - bs - 18, 34), &format!("{} × {}", p.vw, p.vh), t.text_dim);
    }
}

/// шкала времени, кнопки, громкость
fn bottom(ui: &mut Ui, p: &P, w: i32, h: i32) {
    let t = ui.t;
    let r = R::new(12, h - 84, w - 24, 72);
    ui.cv.pxbox(r, &BoxStyle::new(t.panel.alpha(0.94), t.border.lerp(t.red, 0.5)).shadow(4));
    ui.zone(r, Act::Block, Cur::Default);
    // шкала: сыграно — акцент, в буфере — светлее, курсор — ember
    let sr = R::new(r.x + 14, r.y + 10, r.w - 28, 12);
    let track = R::new(sr.x, sr.y + 3, sr.w, 6);
    ui.cv.fill(track, t.surface.lerp(t.border, 0.4));
    let k = match p.seek_drag {
        Some(k) => k,
        None if p.dur > 0.0 => (p.pos / p.dur).clamp(0.0, 1.0),
        None => 0.0,
    };
    if p.dur > 0.0 && p.cache > 0.0 {
        let c = ((p.pos + p.cache) / p.dur).clamp(0.0, 1.0);
        ui.cv.fill(R::new(track.x, track.y, (track.w as f64 * c) as i32, track.h), t.surface_hi);
    }
    let px = (track.w as f64 * k) as i32;
    ui.cv.fill(R::new(track.x, track.y, px, track.h), t.red);
    // клетки поверх: шкала из сегментов, как остальные в шелле
    let mut x = track.x + 8;
    while x < track.right() {
        ui.cv.fill(R::new(x, track.y, 1, track.h), t.panel.alpha(0.6));
        x += 8;
    }
    if p.dur > 0.0 {
        ui.cv.fill(R::new(track.x + px - 2, sr.y, 4, sr.h), t.ember);
    }
    ui.zone(sr.inset(-4), Act::Seek, Cur::Pointer);
    // время под курсором
    if let Some((mx, my)) = p.ptr {
        if p.dur > 0.0 && (sr.inset(-4).contains(mx, my) || p.seek_drag.is_some()) {
            let kk = p.seek_drag.unwrap_or(((mx - track.x as f64) / track.w as f64).clamp(0.0, 1.0));
            let s = yami_px::clock(kk * p.dur);
            let tw = ui.f.width(PX, &s) + 16;
            let tx = (track.x + (track.w as f64 * kk) as i32 - tw / 2).clamp(4, w - tw - 4);
            let tr = R::new(tx, r.y - 28, tw, 22);
            ui.cv.pxbox(tr, &BoxStyle::new(t.panel, t.red));
            ui.cv.text_center(ui.f, PX, tr, &s, t.text);
        }
    }
    // кнопки
    let bs = 30;
    let y = r.y + 32;
    let mut x = r.x + 8;
    let many = p.list.len() > 1;
    for (icon, a, on) in [("prev", "prev", many), (if p.paused { "play" } else { "pause" }, "play", p.loaded), ("next", "next", many)] {
        ui.button(R::new(x, y, bs, bs), "", icon, if a == "play" { Tone::Primary } else { Tone::Ghost }, false, on, Act::Btn(a));
        x += bs + 2;
    }
    x += 8;
    let tm = if p.dur > 0.0 { format!("{} / {}", yami_px::clock(p.seek_drag.map(|k| k * p.dur).unwrap_or(p.pos)), yami_px::clock(p.dur)) } else { yami_px::clock(p.pos) };
    let tw = ui.cv.text_in(ui.f, PX, R::new(x, y, 160, bs), &tm, t.text_dim);
    let left_end = x + tw + 12;
    // справа налево: весь экран, сведения, кадр, пиксели, повтор, субтитры, звук, скорость
    let mut rx = r.right() - 8;
    let right: [(&str, &str, bool, bool); 7] = [
        ("full", "full", p.full, true),
        ("about", "info", p.info, true),
        ("camera", "shot", false, p.video),
        ("grid", "pixel", p.pixel, p.video),
        ("redo", "loop", p.looping, p.loaded),
        ("text", "subs", p.sid != "no" && !p.sid.is_empty(), p.loaded),
        ("music", "audio", false, p.loaded),
    ];
    for (icon, a, sel, on) in right {
        rx -= bs;
        if rx < left_end + 200 && !matches!(a, "full" | "info") {
            rx += bs;
            continue;
        }
        ui.button(R::new(rx, y, bs, bs), "", icon, Tone::Ghost, sel, on, Act::Btn(a));
        rx -= 2;
    }
    // скорость — надписью
    let sp = if (p.speed - 1.0).abs() < 0.01 { "×1".to_string() } else { format!("×{}", p.speed) };
    let sw = ui.f.width(PX, &sp) + 16;
    rx -= sw;
    if rx > left_end + 130 {
        ui.button(R::new(rx, y, sw, bs), &sp, "", Tone::Ghost, (p.speed - 1.0).abs() > 0.01, p.loaded, Act::Btn("speed"));
        rx -= 8;
    } else {
        rx += sw;
    }
    // громкость: значок и 10 клеток
    let vw = 10 * 8;
    let vx = rx - vw - 4;
    if vx > left_end + 30 {
        let vr = R::new(vx, y + 9, vw, 12);
        let k = if p.mute { 0.0 } else { (p.volume / 100.0).clamp(0.0, 1.0) as f32 };
        ui.cv.gauge(vr, 10, k, if p.volume > 100.0 { t.warn } else { t.red }, t.surface.lerp(t.border, 0.4), 2);
        ui.zone(vr.inset(-4), Act::Vol, Cur::Pointer);
        ui.button(R::new(vx - bs - 2, y, bs, bs), "", if p.mute || p.volume < 1.0 { "mute" } else { "speaker" }, Tone::Ghost, p.mute, true, Act::Btn("mute"));
    }
    // подсказка над кнопкой
    if let Some((mx, my)) = p.ptr {
        let hit = ui.zones.iter().rev().find(|z| z.r.contains(mx, my)).and_then(|z| if let Act::Btn(a) = &z.act { Some(*a) } else { None });
        if let Some(a) = hit {
            let s = tip(a);
            if !s.is_empty() {
                let s = crate::i18n::tr(s);
                let sw = ui.f.width(PX, s) + 18;
                let tr = R::new(((mx as i32) - sw / 2).clamp(4, w - sw - 4), r.y - 28, sw, 22);
                ui.cv.pxbox(tr, &BoxStyle::new(t.panel, t.red));
                ui.cv.text_center(ui.f, PX, tr, s, t.text);
            }
        }
    }
}

/// пока панели спрятаны, а поток догружается — тонкая полоса снизу
fn bottom_line(ui: &mut Ui, p: &P, w: i32, h: i32) {
    let t = ui.t;
    let ph = (p.born.elapsed().as_millis() / 60) as i32;
    let mut x = -(ph % 16);
    while x < w {
        ui.cv.fill(R::new(x, h - 3, 8, 3), t.red);
        x += 16;
    }
    ui.anim = true;
}

fn tip(a: &str) -> &'static str {
    match a {
        "prev" => "предыдущий · p",
        "next" => "следующий · n",
        "play" => "пауза · пробел",
        "mute" => "без звука · m",
        "speed" => "скорость · [ ]",
        "audio" => "дорожка звука · a",
        "subs" => "субтитры · s",
        "loop" => "повтор · l",
        "pixel" => "пиксельный масштаб · x",
        "shot" => "сохранить кадр · c",
        "info" => "сведения · i",
        "full" => "во весь экран · f",
        _ => "",
    }
}

fn info(ui: &mut Ui, p: &P, w: i32) {
    let t = ui.t;
    let iw = 320;
    let dir = p.path().and_then(|x| x.parent()).map(|d| d.to_string_lossy().replace(&crate::paths::home().to_string_lossy().to_string(), "~")).unwrap_or_default();
    let mut rows: Vec<(&str, String)> = vec![("формат", p.format.clone())];
    if p.video {
        let fps = if p.fps > 0.0 { format!(" · {:.3} fps", p.fps).replace(".000", "") } else { String::new() };
        rows.push(("видео", format!("{} · {} × {}{fps}", p.vcodec, p.vw, p.vh)));
    }
    if !p.acodec.is_empty() {
        rows.push(("звук", p.acodec.clone()));
    }
    let hw = if p.hwdec.is_empty() || p.hwdec == "no" { crate::i18n::tr("процессор").to_string() } else { p.hwdec.clone() };
    if p.video {
        rows.push(("декодер", hw));
    }
    if p.size > 0 {
        rows.push(("файл", human(p.size)));
    }
    rows.push(("дорожек", p.tracks.0.to_string()));
    rows.push(("папка", dir));
    let r = R::new(w - iw - 12, 46, iw, 40 + rows.len() as i32 * 24);
    ui.cv.pxbox(r, &BoxStyle::new(t.panel.alpha(0.96), t.border).shadow(4));
    ui.zone(r, Act::Block, Cur::Default);
    ui.cv.fill(R::new(r.x + 12, r.y + 15, 6, 6), t.red);
    ui.text_in(PX, R::new(r.x + 24, r.y + 6, r.w - 30, 24), "сведения", t.text);
    let mut y = r.y + 36;
    for (k, val) in rows {
        ui.text_in(PX, R::new(r.x + 14, y, 100, 20), k, t.text_faint);
        ui.cv.text_in(ui.f, PX, R::new(r.x + 114, y, r.w - 126, 20), &val, t.text_dim);
        y += 24;
    }
}

#[cfg(test)]
mod t {
    #[test]
    fn natural_order() {
        let mut v = vec!["ep10.mkv", "ep2.mkv", "ep1.mkv"];
        v.sort_by(|a, b| yami_px::paths::natural(a, b));
        assert_eq!(v, ["ep1.mkv", "ep2.mkv", "ep10.mkv"]);
        assert_eq!(yami_px::clock(3725.0), "1:02:05");
    }
}
