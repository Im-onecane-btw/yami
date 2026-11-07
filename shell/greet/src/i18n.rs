// Язык экрана входа — тот же механизм, что в шелле: надписи по-русски, английский по таблице
// при отрисовке. Язык — из вида, который выложил шелл, иначе системный (LANG, /etc/locale.conf).
// Третий язык — бразильский португальский: таблица PT_BR (i18n_pt_br.rs) от английского, у tf! —
// третий шаблон. Какой язык — решает yami_px::i18n (общий выбор для всех программ).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

pub use yami_px::i18n::{en, pt_br, translated};

/// `shot`: собирать надписи, которые остались без перевода
static AUDIT: AtomicBool = AtomicBool::new(false);
static MISSED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// "" — по LANG greetd или /etc/locale.conf, иначе «ru», «en», «pt-BR»
pub fn set(lang: &str) {
    let sys = if lang.is_empty() { crate::data::sys_lang() } else { lang.to_string() };
    yami_px::i18n::set(&sys);
}

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
