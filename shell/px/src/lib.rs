// yami-px: всё, из чего состоит пиксельный вид — холст (целые пиксели, фаски, байеровское
// проявление), шрифты, тема из одного акцента, иконки, немедленный UI. Общая для шелла и
// приложений (файлы, диалог выбора файлов), чтобы они выглядели как одно целое.

pub mod font;
pub mod gfx;
pub mod icons;
pub mod look;
pub mod paths;
pub mod theme;
pub mod ui;

/// время «1:05» или «1:02:03» (плееры: шелл, yami-play)
pub fn clock(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// вернуть системе свободную память кучи после большого декода. Только glibc: musl (Alpine) такого
/// вызова не знает и свободные страницы отдаёт сам
pub fn trim_heap() {
    #[cfg(target_env = "gnu")]
    unsafe {
        libc::malloc_trim(0);
    }
}

/// Код, который процесс не исполняет, — из памяти. При старте ядро читает бинарь с запасом
/// (упреждающее чтение и соседние страницы на каждый промах): у портала файлов так в памяти 5 МБ
/// кода из 6.6, хотя работает малая часть. Когда старт позади: страницы своего бинаря (только
/// чтение и исполнение, не данные) — назад ядру (MADV_PAGEOUT), упреждающее чтение для них выкл.
/// (MADV_RANDOM): дальше в памяти то, что исполняется, холодное читается при первом вызове.
/// Страницы, которые держит и другой процесс, ядро не трогает. Ядро старше 5.4 — ничего не меняется
pub fn shed_code() {
    use std::os::unix::fs::MetadataExt;
    // по inode и пути: устройство в maps у btrfs другое, чем в stat (номер подтома)
    let Ok(me) = std::fs::metadata("/proc/self/exe") else { return };
    let ino = me.ino().to_string();
    let exe = paths::self_exe().to_string_lossy().to_string();
    let gone = format!("{exe} (deleted)");
    let Ok(maps) = std::fs::read_to_string("/proc/self/maps") else { return };
    for l in maps.lines() {
        // адреса права смещение устройство inode путь
        let f: Vec<&str> = l.split_whitespace().take(5).collect();
        if f.len() < 5 || f[4] != ino || !(l.ends_with(&exe) || l.ends_with(&gone)) || f[1].as_bytes().get(1) != Some(&b'-') {
            continue;
        }
        let Some((a, b)) = f[0].split_once('-') else { continue };
        let (Ok(a), Ok(b)) = (usize::from_str_radix(a, 16), usize::from_str_radix(b, 16)) else { continue };
        if b > a {
            unsafe {
                libc::madvise(a as *mut libc::c_void, b - a, libc::MADV_RANDOM);
                libc::madvise(a as *mut libc::c_void, b - a, libc::MADV_PAGEOUT);
            }
        }
    }
}

/// shed_code через secs секунд, из своей короткой нити (окна приложений: старт позади, окно открыто)
pub fn shed_code_later(secs: u64) {
    let _ = std::thread::Builder::new().name("shed".into()).spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(secs));
        shed_code();
    });
}
