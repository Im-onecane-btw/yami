// Язык плеера — тот же механизм, что в шелле: надписи по-русски, английский по
// таблице при отрисовке. Язык берётся из настроек шелла.
// Третий язык — бразильский португальский: таблица PT_BR (i18n_pt_br.rs) от английского, у tf! —
// третий шаблон. Какой язык — решает yami_px::i18n (общий выбор для всех программ).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

pub use yami_px::i18n::{en, pt_br, set, translated};

/// `shot`: собирать надписи, которые остались без перевода
static AUDIT: AtomicBool = AtomicBool::new(false);
static MISSED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// перевод целой надписи (английский по PAIRS, португальский по PT_BR от английского); нет — как есть
pub fn tr(s: &str) -> &str {
    if !translated() {
        return s;
    }
    let english = table().get(s).copied().unwrap_or(s);
    let out = select(s, english);
    let missing = out.chars().any(|c| matches!(c, 'а'..='я' | 'А'..='Я' | 'ё' | 'Ё')) || (pt_br() && table().contains_key(s) && !pt_table().contains_key(english));
    if AUDIT.load(Ordering::Relaxed) && missing {
        if let Ok(mut m) = MISSED.lock() {
            if !m.iter().any(|x| x == out) {
                m.push(out.to_string());
            }
        }
    }
    out
}

/// пара прямо в коде (клавиши, doctor): русская и английская, португальская — по английской
pub fn select<'a>(ru: &'a str, en: &'a str) -> &'a str {
    if pt_br() {
        pt_table().get(en).copied().unwrap_or(en)
    } else if translated() {
        en
    } else {
        ru
    }
}

pub fn audit(on: bool) {
    AUDIT.store(on, Ordering::Relaxed);
}

/// надписи без перевода с прошлого вызова
pub fn take_missed() -> Vec<String> {
    MISSED.lock().map(|mut m| std::mem::take(&mut *m)).unwrap_or_default()
}

/// строка с подстановками: русский, английский и португальский шаблоны рядом (литералы: Rust
/// проверяет аргументы при сборке)
#[macro_export]
macro_rules! tf {
    ($ru:literal, $en:literal, $pt:literal $(, $a:expr)* $(,)?) => {
        if $crate::i18n::pt_br() {
            format!($pt $(, $a)*)
        } else if $crate::i18n::en() {
            format!($en $(, $a)*)
        } else {
            format!($ru $(, $a)*)
        }
    };
}

fn table() -> &'static HashMap<&'static str, &'static str> {
    static T: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    T.get_or_init(|| PAIRS.iter().copied().collect())
}

fn pt_table() -> &'static HashMap<&'static str, &'static str> {
    static T: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    T.get_or_init(|| PT_BR.iter().copied().collect())
}

include!("i18n_pt_br.rs");

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
