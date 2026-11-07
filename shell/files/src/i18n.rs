// Язык файлового менеджера — тот же механизм, что в шелле: надписи по-русски, английский по
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

const MON: [&str; 12] = ["янв", "фев", "мар", "апр", "мая", "июн", "июл", "авг", "сен", "окт", "ноя", "дек"];

pub fn month_short(m: i32) -> &'static str {
    tr(MON[((m - 1).clamp(0, 11)) as usize])
}

const PAIRS: &[(&str, &str)] = &[
    // устройства и сеть (remote.rs)
    ("сеть", "network"),
    ("подключиться…", "connect…"),
    ("подключиться к серверу", "connect to a server"),
    ("адрес", "address"),
    ("логин", "user"),
    ("пароль", "password"),
    ("необязательно", "optional"),
    ("smb · sftp · ftp · davs · nfs · домен — ДОМЕН\\логин", "smb · sftp · ftp · davs · nfs · domain — DOMAIN\\user"),
    ("сервер просит подтверждения: подключиться всё равно?", "the server asks to confirm: connect anyway?"),
    ("подключаю", "connecting"),
    ("подключиться всё равно", "connect anyway"),
    ("устройства слева", "devices on the left"),
    ("диски, телефоны", "drives, phones"),
    ("сеть слева", "network on the left"),
    ("нужен gvfs-mtp", "needs gvfs-mtp"),
    ("подключено", "connected"),
    ("неверный логин или пароль", "wrong user or password"),
    ("нужны логин и пароль", "user and password needed"),
    ("разблокируй телефон и выбери «передача файлов»", "unlock the phone and choose «file transfer»"),
    ("введи адрес: smb://, sftp://, ftp://, davs://, nfs://", "enter an address: smb://, sftp://, ftp://, davs://, nfs://"),
    ("для телефонов нужен пакет gvfs-mtp (и gvfs-gphoto2 для камер)", "phones need the gvfs-mtp package (and gvfs-gphoto2 for cameras)"),
    ("подключить как…", "connect as…"),
    ("забыть адрес", "forget the address"),
    ("нет gio (пакет glib2 / gvfs)", "no gio (glib2 / gvfs package)"),
    ("не подключается", "cannot connect"),
    ("янв", "Jan"),
    ("фев", "Feb"),
    ("мар", "Mar"),
    ("апр", "Apr"),
    ("мая", "May"),
    ("июн", "Jun"),
    ("июл", "Jul"),
    ("авг", "Aug"),
    ("сен", "Sep"),
    ("окт", "Oct"),
    ("ноя", "Nov"),
    ("дек", "Dec"),
    ("в буфере нет файлов", "no files in the clipboard"),
    ("буфер обмена пуст", "clipboard is empty"),
    ("открыть", "open"),
    ("фильтр по имени", "filter by name"),
    ("путь", "path"),
    ("дом", "home"),
    ("места", "places"),
    ("корзина", "trash"),
    ("устройства", "devices"),
    ("не подключён", "not mounted"),
    ("читаю", "reading"),
    ("корзина пуста", "trash is empty"),
    ("ничего не нашлось", "nothing found"),
    ("папка пуста", "folder is empty"),
    ("имя", "name"),
    ("размер", "size"),
    ("изменён", "modified"),
    ("тип", "type"),
    ("выбери файл", "pick a file"),
    ("откуда", "from"),
    ("удалён", "deleted"),
    ("права", "permissions"),
    ("ссылка на", "links to"),
    ("копирую", "copying"),
    ("переношу", "moving"),
    ("в корзину", "move to trash"),
    ("удаляю", "deleting"),
    ("распаковываю", "extracting"),
    ("сжимаю", "compressing"),
    ("возвращаю", "restoring"),
    ("ссылки", "linking"),
    ("отмена", "cancel"),
    ("имя файла", "file name"),
    ("выбери папку (или открой её и нажми «выбрать»)", "pick a folder (or open it and press “select”)"),
    ("выбери файл двойным щелчком или кнопкой", "double-click a file or use the button"),
    ("новая папка", "new folder"),
    ("новый файл", "new file"),
    ("сжать в архив", "compress"),
    ("создать", "create"),
    ("такой файл уже есть", "this file already exists"),
    ("новый", "new"),
    ("уже есть", "existing"),
    ("папка", "folder"),
    ("для всех остальных", "for all the rest"),
    ("пропустить", "skip"),
    ("оставить оба", "keep both"),
    ("заменить", "replace"),
    ("не всё получилось", "not everything worked"),
    ("понятно", "ok"),
    ("открыть с помощью", "open with"),
    ("найти программу", "find an app"),
    ("остальные программы", "other apps"),
    ("всегда открывать этот тип так", "always open this type with it"),
    ("свойства", "properties"),
    ("считаю…", "counting…"),
    ("где", "location"),
    ("владелец", "owner"),
    ("на диске", "disk"),
    ("чтение", "read"),
    ("запись", "write"),
    ("запуск", "run"),
    ("группа", "group"),
    ("остальные", "others"),
    ("закрыть", "close"),
    ("настройки файлов", "file manager settings"),
    ("вид", "view"),
    ("поведение", "behaviour"),
    ("скрытые файлы", "hidden files"),
    ("папки сверху", "folders first"),
    ("миниатюры", "thumbnails"),
    ("картинки и видео", "images and videos"),
    ("места слева", "places on the left"),
    ("сведения справа", "details on the right"),
    ("открывать одним щелчком", "open with a single click"),
    ("спрашивать перед корзиной", "ask before trashing"),
    ("новое окно — две панели", "new window — two panes"),
    ("по убыванию", "descending"),
    ("список", "list"),
    ("значки", "icons"),
    ("по имени", "by name"),
    ("по размеру", "by size"),
    ("по дате", "by date"),
    ("по типу", "by type"),
    ("порядок", "order"),
    ("размер значков", "icon size"),
    ("цвета и язык — из настроек шелла (mod+s)", "colours and language come from the shell settings (mod+s)"),
    ("терминал", "terminal"),
    ("готово", "done"),
    ("файлы", "files"),
    ("сначала верни файл из корзины", "restore the file from the trash first"),
    ("это программа: запусти её из меню → «запустить»", "this is a program: use menu → “run”"),
    ("введи имя файла", "type a file name"),
    ("удалить навсегда", "delete permanently"),
    ("удалить", "delete"),
    ("очистить корзину", "empty trash"),
    ("очистить", "empty"),
    ("новый файл.txt", "new file.txt"),
    ("архив", "archive"),
    ("путь скопирован", "path copied"),
    ("добавлено в места", "added to places"),
    ("скрытые файлы видны", "hidden files shown"),
    ("скрытые файлы спрятаны", "hidden files hidden"),
    ("обои поменяются через секунду", "the wallpaper changes in a second"),
    ("нечего отменять", "nothing to undo"),
    ("перенос отменён", "move undone"),
    ("имя возвращено", "name restored"),
    ("в имени не может быть «/»", "a name cannot contain “/”"),
    ("имя пустое", "the name is empty"),
    ("такое имя уже есть", "this name is taken"),
    ("нет udisksctl (пакет udisks2)", "no udisksctl (package udisks2)"),
    ("нет доступа", "permission denied"),
    ("не найдено", "not found"),
    ("уже существует", "already exists"),
    ("это не папка", "not a folder"),
    ("нет места на диске", "no space left on the disk"),
    ("только для чтения", "read-only"),
    ("сохранить файл", "save file"),
    ("выбрать папку", "select folder"),
    ("открыть файлы", "open files"),
    ("открыть файл", "open file"),
    ("сохранить", "save"),
    ("выбрать", "select"),
    ("крупнее", "bigger"),
    ("мельче", "smaller"),
    ("папку", "folder"),
    ("пустой файл", "empty file"),
    ("вернуть", "restore"),
    ("в новой вкладке", "in a new tab"),
    ("в соседней панели", "in the other pane"),
    ("терминал здесь", "terminal here"),
    ("другая программа…", "other app…"),
    ("запустить", "run"),
    ("вырезать", "cut"),
    ("копировать", "copy"),
    ("вставить сюда", "paste here"),
    ("копировать путь", "copy path"),
    ("переименовать", "rename"),
    ("распаковать сюда", "extract here"),
    ("сжать", "compress"),
    ("создать ссылку", "create link"),
    ("сделать обоями", "set as wallpaper"),
    ("добавить в места", "add to places"),
    ("вставить", "paste"),
    ("отменить", "undo"),
    ("выделить всё", "select all"),
    ("обратить выделение", "invert selection"),
    ("обновить", "reload"),
    ("свойства папки", "folder properties"),
    ("новое окно", "new window"),
    ("новая вкладка", "new tab"),
    ("разделить", "split"),
    ("поиск", "search"),
    ("настройки", "settings"),
    ("убрать из мест", "remove from places"),
    ("извлечь", "eject"),
    ("отключить", "unmount"),
    ("подключить", "mount"),
    ("переместить сюда", "move here"),
    ("копировать сюда", "copy here"),
    ("ссылку сюда", "link here"),
    ("изображение", "image"),
    ("видео", "video"),
    ("аудио", "audio"),
    ("текст", "text"),
    ("код", "code"),
    ("документ", "document"),
    ("программа", "program"),
    ("шрифт", "font"),
    ("образ диска", "disk image"),
    ("пакет", "package"),
    ("веб-страница", "web page"),
    ("файл", "file"),
    ("рабочий стол", "desktop"),
    ("документы", "documents"),
    ("загрузки", "downloads"),
    ("музыка", "music"),
    ("изображения", "pictures"),
    ("система", "system"),
    ("картинки", "images"),
    ("все файлы", "all files"),
    ("корзина недоступна", "trash is not available"),
    ("неизвестно, откуда удалён", "unknown original location"),
    ("зашифрован", "encrypted"),
    ("зашифрованный диск", "encrypted drive"),
    ("неверный пароль", "wrong password"),
    ("скрыть", "hide"),
    ("показать скрытые", "show hidden"),
    ("места по умолчанию", "default places"),
    ("имя в местах", "name in places"),
    ("открыть паролем…", "unlock with password…"),
    ("пароль диска", "drive password"),
    ("пароль не сохраняется", "the password is not saved"),
    ("открываю", "unlocking"),
    ("подключиться к серверу…", "connect to a server…"),
];
