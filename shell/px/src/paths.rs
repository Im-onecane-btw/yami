// Каталоги XDG и атомарная запись.

use std::path::{Path, PathBuf};

/// переменная XDG: пустая или относительная — как не заданная (так велит спецификация)
fn xdg(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute())
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}
pub fn config_dir() -> PathBuf {
    xdg("XDG_CONFIG_HOME").unwrap_or_else(|| home().join(".config"))
}
pub fn state_dir() -> PathBuf {
    let d = xdg("XDG_STATE_HOME").unwrap_or_else(|| home().join(".local/state")).join("yami");
    let _ = std::fs::create_dir_all(&d);
    d
}
pub fn data_dir() -> PathBuf {
    xdg("XDG_DATA_HOME").unwrap_or_else(|| home().join(".local/share"))
}
pub fn cache_dir() -> PathBuf {
    xdg("XDG_CACHE_HOME").unwrap_or_else(|| home().join(".cache"))
}
/// новый личный каталог для временных файлов: в $XDG_RUNTIME_DIR (он только наш) или в /tmp, с
/// правами 0700 и всегда новый — заготовка другого пользователя в /tmp со ссылками внутри
/// перенаправила бы наши записи в чужие файлы, а снимок экрана в /tmp иначе читали бы все
pub fn private_tmp(name: &str) -> std::io::Result<PathBuf> {
    use std::os::unix::fs::DirBuilderExt;
    let base = xdg("XDG_RUNTIME_DIR").filter(|p| p.is_dir()).unwrap_or_else(std::env::temp_dir);
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    let d = base.join(format!("{name}-{}-{n:08x}", std::process::id()));
    std::fs::DirBuilder::new().mode(0o700).create(&d)?;
    Ok(d)
}

/// настройки шелла: оттуда же приложения берут палитру, язык и анимации
pub fn shell_settings() -> PathBuf {
    config_dir().join("yami-shell.json")
}

/// свой бинарь: после обновления старый файл удалён («… (deleted)») — берём новый по тому же пути
pub fn self_exe() -> PathBuf {
    let Some(p) = std::fs::read_link("/proc/self/exe").ok().or_else(|| std::env::current_exe().ok()) else { return PathBuf::from("yami-shell") };
    let s = p.to_string_lossy();
    PathBuf::from(s.strip_suffix(" (deleted)").unwrap_or(&s))
}

/// файл, путь к которому дал чужой процесс (уведомление, плеер), читаем с потолком: /dev/zero,
/// fifo или гигантский файл не съедят память и не подвесят нить. Не обычный файл — None.
pub fn read_capped(p: &Path, max: u64) -> Option<Vec<u8>> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    if !std::fs::metadata(p).ok()?.is_file() {
        return None;
    }
    // O_NONBLOCK: если файл подменили на fifo между проверкой и открытием, open не повиснет
    let f = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(p).ok()?;
    let m = f.metadata().ok()?;
    if !m.is_file() || m.len() > max {
        return None;
    }
    let mut v = Vec::with_capacity(m.len() as usize);
    f.take(max).read_to_end(&mut v).ok()?;
    Some(v)
}

/// запись через временный файл: читатель никогда не увидит половину
pub fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    if std::fs::read_to_string(path).map(|t| t == text).unwrap_or(false) {
        return Ok(());
    }
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let tmp = path.with_extension("tmp~");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// Разбор JSON-настроек, прощающий ошибки: ключ с негодным значением берётся по умолчанию, а не
/// весь файл (правят руками по ключу — одна опечатка не сбрасывает остальное). Неизвестные
/// ключи пропускаются. Не JSON вовсе — значения по умолчанию.
pub fn lenient_json<T: serde::de::DeserializeOwned + serde::Serialize + Default>(text: &str) -> T {
    if let Ok(v) = serde_json::from_str::<T>(text) {
        return v;
    }
    let Some(user) = serde_json::from_str::<serde_json::Value>(text).ok().and_then(|v| v.as_object().cloned()) else { return T::default() };
    let mut base = serde_json::to_value(T::default()).unwrap_or_default();
    for (k, x) in user {
        let mut one = base.clone();
        one[&k] = x.clone();
        if serde_json::from_value::<T>(one).is_ok() {
            base[&k] = x;
        } else {
            eprintln!("bad value for {k}, default used");
        }
    }
    serde_json::from_value(base).unwrap_or_default()
}
