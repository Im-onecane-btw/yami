// Язык плеера — тот же механизм, что в шелле: надписи по-русски, английский по
// таблице при отрисовке. Язык берётся из настроек шелла.
//
// Interface language: Russian (source) or English. UI strings are written in Russian and
// swapped through the table below at draw time, so switching is instant.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

static EN: AtomicBool = AtomicBool::new(false);
/// `yami-shell shot`: собирать надписи, которые при английском остались русскими
static AUDIT: AtomicBool = AtomicBool::new(false);
static MISSED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// "" — по $LANG, "ru", "en"
pub fn set(lang: &str) {
    let en = match lang {
        "en" => true,
        "ru" => false,
        _ => {
            let l = std::env::var("LC_ALL").or_else(|_| std::env::var("LC_MESSAGES")).or_else(|_| std::env::var("LANG")).unwrap_or_default();
            !l.starts_with("ru")
        }
    };
    EN.store(en, Ordering::Relaxed);
}

pub fn en() -> bool {
    EN.load(Ordering::Relaxed)
}

/// перевод целой надписи; нет в таблице — как есть
pub fn tr(s: &str) -> &str {
    if !en() {
        return s;
    }
    let out = table().get(s).copied().unwrap_or(s);
    if AUDIT.load(Ordering::Relaxed) && out.chars().any(|c| matches!(c, 'а'..='я' | 'А'..='Я' | 'ё' | 'Ё')) {
        if let Ok(mut m) = MISSED.lock() {
            if !m.iter().any(|x| x == out) {
                m.push(out.to_string());
            }
        }
    }
    out
}

pub fn audit(on: bool) {
    AUDIT.store(on, Ordering::Relaxed);
}

/// надписи без перевода с прошлого вызова
pub fn take_missed() -> Vec<String> {
    MISSED.lock().map(|mut m| std::mem::take(&mut *m)).unwrap_or_default()
}

/// строка с подстановками: русский и английский шаблоны рядом
#[macro_export]
macro_rules! tf {
    ($ru:literal, $en:literal $(, $a:expr)* $(,)?) => {
        if $crate::i18n::en() { format!($en $(, $a)*) } else { format!($ru $(, $a)*) }
    };
}

fn table() -> &'static HashMap<&'static str, &'static str> {
    static T: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    T.get_or_init(|| PAIRS.iter().copied().collect())
}

const PAIRS: &[(&str, &str)] = &[
    ("открываю", "opening"),
    ("сведения", "details"),
    ("формат", "format"),
    ("видео", "video"),
    ("звук", "audio"),
    ("декодер", "decoder"),
    ("процессор", "CPU"),
    ("файл", "file"),
    ("дорожек", "tracks"),
    ("папка", "folder"),
    ("следующая дорожка звука", "next audio track"),
    ("следующие субтитры", "next subtitles"),
    ("повтор выключен", "repeat off"),
    ("повтор файла", "repeat this file"),
    ("пиксели: честные клетки", "pixels: sharp cells"),
    ("пиксели: сглаженно", "pixels: smooth"),
    ("кадр сохранён в «изображения»", "frame saved to Pictures"),
    ("предыдущий · p", "previous · p"),
    ("следующий · n", "next · n"),
    ("пауза · пробел", "pause · space"),
    ("без звука · m", "mute · m"),
    ("скорость · [ ]", "speed · [ ]"),
    ("дорожка звука · a", "audio track · a"),
    ("субтитры · s", "subtitles · s"),
    ("повтор · l", "repeat · l"),
    ("пиксельный масштаб · x", "pixel scaling · x"),
    ("сохранить кадр · c", "save frame · c"),
    ("сведения · i", "details · i"),
    ("во весь экран · f", "fullscreen · f"),
    ("нет файлов", "no files"),
    ("файл кончился", "end of file"),
];
