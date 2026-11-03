// Музыка: `playerctl -F metadata` сообщает о смене трека и паузе; обложка скачивается один
// раз и пикселизуется под стиль. Кнопки — playerctl play-pause / next / previous.

use crate::app::Msg;
use calloop::channel::Sender;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct MediaState {
    pub player: String,
    pub status: String,
    pub artist: String,
    pub title: String,
    pub art: String,
    pub length: f64,
    pub pos: f64,
    pub at: Instant,
}

impl Default for MediaState {
    fn default() -> Self {
        MediaState { player: String::new(), status: String::new(), artist: String::new(), title: String::new(), art: String::new(), length: 0.0, pos: 0.0, at: Instant::now() }
    }
}

impl MediaState {
    pub fn playing(&self) -> bool {
        self.status == "Playing"
    }
    /// позиция сейчас, с; пока играет — идёт сама
    pub fn now_pos(&self) -> f64 {
        let p = if self.playing() { self.pos + self.at.elapsed().as_secs_f64() } else { self.pos };
        if self.length > 0.0 {
            p.min(self.length)
        } else {
            p
        }
    }
}

pub fn viz_wanted(on: bool) {
    crate::audio::viz_wanted(on);
}

pub fn ctl(cmd: &str) {
    let cmd = cmd.to_string();
    std::thread::spawn(move || {
        let _ = Command::new("playerctl").arg(cmd).status();
    });
}

pub fn spawn(tx: Sender<Msg>) {
    std::thread::Builder::new()
        .name("media".into())
        .spawn(move || loop {
            let fmt = "{{playerName}}\t{{status}}\t{{artist}}\t{{title}}\t{{mpris:artUrl}}\t{{mpris:length}}\t{{position}}";
            let Ok(mut c) = Command::new("playerctl").args(["-F", "metadata", "--format", fmt]).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() else {
                std::thread::sleep(std::time::Duration::from_secs(30));
                continue;
            };
            let out = c.stdout.take().unwrap();
            for l in BufReader::new(out).lines() {
                let Ok(l) = l else { break };
                let f: Vec<&str> = l.split('\t').collect();
                let g = |i: usize| f.get(i).copied().unwrap_or("").to_string();
                let m = MediaState {
                    player: g(0),
                    status: g(1),
                    artist: g(2),
                    title: g(3),
                    art: g(4),
                    length: g(5).parse::<f64>().unwrap_or(0.0) / 1e6,
                    pos: g(6).parse::<f64>().unwrap_or(0.0) / 1e6,
                    at: Instant::now(),
                };
                let _ = tx.send(Msg::Media(m));
            }
            let _ = c.wait();
            let _ = tx.send(Msg::Media(MediaState::default()));
            std::thread::sleep(std::time::Duration::from_secs(3));
        })
        .ok();
}

/// обложка: файл или ссылка → 64×64 клетками по 2 px
pub fn fetch_art(url: String, tx: Sender<Msg>) {
    std::thread::spawn(move || {
        let data = if let Some(p) = url.strip_prefix("file://") {
            std::fs::read(p).ok()
        } else if url.starts_with("http") {
            Command::new("curl").args(["-sL", "--max-time", "8", &url]).output().ok().map(|o| o.stdout).filter(|d| !d.is_empty())
        } else {
            None
        };
        let img = data.and_then(|d| image::load_from_memory(&d).ok()).map(|i| {
            let small = image::imageops::resize(&i.to_rgba8(), 32, 32, image::imageops::FilterType::Triangle);
            let big = image::imageops::resize(&small, 64, 64, image::imageops::FilterType::Nearest);
            crate::icons::from_rgba(&big)
        });
        let _ = tx.send(Msg::Art(url, img));
    });
}
