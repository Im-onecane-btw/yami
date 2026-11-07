// Язык экрана входа — тот же механизм, что в шелле: надписи по-русски, английский по таблице
// при отрисовке. Язык — из вида, который выложил шелл, иначе системный (LANG, /etc/locale.conf).
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
            let l = crate::data::sys_lang();
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
    ("пароль", "password"),
    ("имя пользователя", "user name"),
    ("кто входит?", "who is logging in?"),
    ("другой пользователь", "another user"),
    ("проверяю…", "checking…"),
    ("enter — войти", "enter — log in"),
    ("демо: пароль yami, esc — выйти", "demo: the password is yami, esc to leave"),
    ("сон", "sleep"),
    ("перезагрузка", "reboot"),
    ("выключить", "power off"),
    ("засыпаю…", "going to sleep…"),
    ("перезагружаю…", "rebooting…"),
    ("выключаю…", "powering off…"),
    ("в демо не выключаю", "the demo does not power off"),
    ("нет связи с greetd", "no connection to greetd"),
    ("сеанс не запустился", "the session did not start"),
    ("нет сеансов Wayland", "no Wayland sessions"),
    ("воскресенье", "Sunday"),
    ("понедельник", "Monday"),
    ("вторник", "Tuesday"),
    ("среда", "Wednesday"),
    ("четверг", "Thursday"),
    ("пятница", "Friday"),
    ("суббота", "Saturday"),
    ("января", "January"),
    ("февраля", "February"),
    ("марта", "March"),
    ("апреля", "April"),
    ("мая", "May"),
    ("июня", "June"),
    ("июля", "July"),
    ("августа", "August"),
    ("сентября", "September"),
    ("октября", "October"),
    ("ноября", "November"),
    ("декабря", "December"),
];
