// Операции с файлами в отдельной нити: копирование и перенос с прогрессом, корзина, удаление,
// ссылки, архивы (bsdtar). Конфликт имён — вопрос в окне, нить ждёт ответа.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Copy,
    Move,
    Trash,
    Delete,
    Link,
    Extract,
    Compress,
    Restore,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Resolve {
    Skip,
    Replace,
    Rename,
    Cancel,
}

pub enum Ev {
    Progress { id: u32, done: u64, total: u64, name: String },
    Conflict { src: PathBuf, dst: PathBuf, reply: mpsc::Sender<(Resolve, bool)> },
    /// pairs: (откуда, куда) — для отмены и подсветки нового
    Done { id: u32, kind: Kind, errors: Vec<String>, pairs: Vec<(PathBuf, PathBuf)> },
}

pub struct Job {
    pub id: u32,
    pub kind: Kind,
    pub srcs: Vec<PathBuf>,
    pub dst: PathBuf,
    /// для сжатия: имя архива
    pub name: String,
    pub stop: Arc<AtomicBool>,
}

pub fn spawn(job: Job, send: impl Fn(Ev) + Send + 'static) {
    std::thread::spawn(move || run(job, &send));
}

struct Ctx<'a> {
    id: u32,
    send: &'a dyn Fn(Ev),
    done: u64,
    total: u64,
    last: Instant,
    stop: Arc<AtomicBool>,
    /// «для всех»: ответ на следующие конфликты
    all: Option<Resolve>,
    errors: Vec<String>,
}

impl Ctx<'_> {
    fn tick(&mut self, name: &str, force: bool) {
        if force || self.last.elapsed().as_millis() > 60 {
            self.last = Instant::now();
            (self.send)(Ev::Progress { id: self.id, done: self.done, total: self.total, name: name.to_string() });
        }
    }
    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
    fn err(&mut self, p: &Path, e: &std::io::Error) {
        self.errors.push(format!("{}: {}", p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), crate::fsx::err_text(e)));
    }
    /// куда класть src в dir: None — пропустить
    fn target(&mut self, src: &Path, dir: &Path) -> Option<PathBuf> {
        let name = src.file_name()?.to_string_lossy().to_string();
        let dst = dir.join(&name);
        if std::fs::symlink_metadata(&dst).is_err() {
            return Some(dst);
        }
        // копия в ту же папку — сразу «имя (2)»
        if src.parent() == Some(dir) {
            return Some(crate::fsx::free_name(dir, &name));
        }
        let r = match self.all {
            Some(r) => r,
            None => {
                let (tx, rx) = mpsc::channel();
                (self.send)(Ev::Conflict { src: src.to_path_buf(), dst: dst.clone(), reply: tx });
                let (r, all) = rx.recv().unwrap_or((Resolve::Cancel, false));
                if all {
                    self.all = Some(r);
                }
                r
            }
        };
        match r {
            Resolve::Skip => None,
            Resolve::Cancel => {
                self.stop.store(true, Ordering::Relaxed);
                None
            }
            Resolve::Rename => Some(crate::fsx::free_name(dir, &name)),
            Resolve::Replace => {
                if same(src, &dst) {
                    return None;
                }
                let m = std::fs::symlink_metadata(&dst);
                let gone = if m.map(|m| m.is_dir()).unwrap_or(false) && !src.is_dir() { std::fs::remove_dir_all(&dst) } else if src.is_dir() && dst.is_dir() { Ok(()) } else { std::fs::remove_file(&dst) };
                if let Err(e) = gone {
                    self.err(&dst, &e);
                    return None;
                }
                Some(dst)
            }
        }
    }
}

fn same(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

fn size_of(p: &Path) -> u64 {
    let Ok(m) = std::fs::symlink_metadata(p) else { return 0 };
    if !m.is_dir() {
        return m.len();
    }
    let mut n = 0;
    if let Ok(rd) = std::fs::read_dir(p) {
        for e in rd.flatten() {
            n += size_of(&e.path());
        }
    }
    n
}

fn copy_file(c: &mut Ctx, src: &Path, dst: &Path) -> std::io::Result<()> {
    use std::io::{Read, Write};
    use std::os::fd::AsRawFd;
    let mut a = std::fs::File::open(src)?;
    let meta = a.metadata()?;
    let mut b = std::fs::File::create(dst)?;
    let name = src.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    // копирует ядро (copy_file_range): на btrfs и xfs внутри одного диска это reflink — мгновенно
    // и без лишнего места, между дисками — без прохода данных через память программы. Кусками:
    // прогресс и отмена. Не умеет (FUSE — телефоны и gvfs, старое ядро между ФС, «файлы» /proc
    // с нулевым размером) — обычное чтение и запись с того же места
    let mut kernel = true;
    let mut copied = 0u64;
    let mut buf: Option<Vec<u8>> = None;
    loop {
        if c.stopped() {
            drop(b);
            let _ = std::fs::remove_file(dst);
            return Ok(());
        }
        let n = if kernel {
            let n = unsafe { libc::copy_file_range(a.as_raw_fd(), std::ptr::null_mut(), b.as_raw_fd(), std::ptr::null_mut(), 16 << 20, 0) };
            if n < 0 {
                let e = std::io::Error::last_os_error();
                if e.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                if copied == 0 && matches!(e.raw_os_error(), Some(libc::EXDEV | libc::ENOSYS | libc::EINVAL | libc::EOPNOTSUPP | libc::EPERM | libc::EBADF)) {
                    kernel = false;
                    continue;
                }
                return Err(e);
            }
            if n == 0 && copied == 0 {
                // пустой — или псевдофайл, у которого размер 0, а данные есть: пусть решит read
                kernel = false;
                continue;
            }
            n as usize
        } else {
            let buf = buf.get_or_insert_with(|| vec![0u8; 1 << 20]);
            let n = a.read(buf)?;
            b.write_all(&buf[..n])?;
            n
        };
        if n == 0 {
            break;
        }
        copied += n as u64;
        c.done += n as u64;
        c.tick(&name, false);
    }
    let _ = b.set_permissions(meta.permissions());
    if let Ok(t) = meta.modified() {
        let _ = b.set_modified(t);
    }
    Ok(())
}

fn copy_tree(c: &mut Ctx, src: &Path, dst: &Path) {
    if c.stopped() {
        return;
    }
    let Ok(m) = std::fs::symlink_metadata(src) else { return };
    if m.file_type().is_symlink() {
        if let Ok(t) = std::fs::read_link(src) {
            if let Err(e) = std::os::unix::fs::symlink(t, dst) {
                c.err(src, &e);
            }
        }
    } else if m.is_dir() {
        if let Err(e) = std::fs::create_dir(dst) {
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                c.err(src, &e);
                return;
            }
        }
        if let Ok(rd) = std::fs::read_dir(src) {
            for e in rd.flatten() {
                copy_tree(c, &e.path(), &dst.join(e.file_name()));
            }
        }
        let _ = std::fs::set_permissions(dst, m.permissions());
    } else if let Err(e) = copy_file(c, src, dst) {
        c.err(src, &e);
    }
}

fn run(job: Job, send: &dyn Fn(Ev)) {
    let mut c = Ctx { id: job.id, send, done: 0, total: 0, last: Instant::now(), stop: job.stop.clone(), all: None, errors: vec![] };
    let mut pairs: Vec<(PathBuf, PathBuf)> = vec![];
    match job.kind {
        Kind::Copy | Kind::Move => {
            c.total = job.srcs.iter().map(|s| size_of(s)).sum();
            c.tick("", true);
            for s in &job.srcs {
                if c.stopped() {
                    break;
                }
                if job.dst.starts_with(s) && s.is_dir() {
                    c.errors.push(crate::tf!("{}: нельзя положить папку в саму себя", "{}: cannot put a folder inside itself", "{}: não é possível colocar uma pasta dentro dela mesma", s.display()));
                    continue;
                }
                if job.kind == Kind::Move && s.parent() == Some(job.dst.as_path()) {
                    continue;
                }
                let Some(d) = c.target(s, &job.dst) else { continue };
                if job.kind == Kind::Move {
                    match std::fs::rename(s, &d) {
                        Ok(()) => {
                            c.done += size_of(&d);
                            pairs.push((s.clone(), d));
                            continue;
                        }
                        Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {}
                        Err(e) => {
                            c.err(s, &e);
                            continue;
                        }
                    }
                }
                let before = c.errors.len();
                copy_tree(&mut c, s, &d);
                if c.stopped() {
                    break;
                }
                if job.kind == Kind::Move && c.errors.len() == before {
                    let r = if s.is_dir() && !s.is_symlink() { std::fs::remove_dir_all(s) } else { std::fs::remove_file(s) };
                    if let Err(e) = r {
                        c.err(s, &e);
                    }
                }
                pairs.push((s.clone(), d));
            }
        }
        Kind::Trash => {
            c.total = job.srcs.len() as u64;
            for s in &job.srcs {
                match crate::trash::put(s) {
                    Ok(t) => pairs.push((s.clone(), t)),
                    Err(e) => c.errors.push(format!("{}: {e}", s.display())),
                }
                c.done += 1;
                c.tick(&s.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default(), false);
            }
        }
        Kind::Delete => {
            c.total = job.srcs.len() as u64;
            for s in &job.srcs {
                let r = if s.is_dir() && !s.is_symlink() { std::fs::remove_dir_all(s) } else { std::fs::remove_file(s) };
                match r {
                    Ok(()) => {
                        crate::trash::forget(s);
                        pairs.push((s.clone(), PathBuf::new()));
                    }
                    Err(e) => c.err(s, &e),
                }
                c.done += 1;
                c.tick("", false);
            }
        }
        Kind::Restore => {
            for s in &job.srcs {
                if let Some(e) = crate::fsx::Entry::from_path(s) {
                    let mut e = e;
                    // путь оригинала — из .trashinfo
                    if let Some(t) = crate::trash::list().into_iter().find(|t| t.path == *s) {
                        e.orig = t.orig;
                    }
                    match crate::trash::restore(&e) {
                        Ok(d) => pairs.push((s.clone(), d)),
                        Err(er) => c.errors.push(format!("{}: {er}", e.name)),
                    }
                }
            }
        }
        Kind::Link => {
            for s in &job.srcs {
                let name = s.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
                let d = crate::fsx::free_name(&job.dst, &name);
                match std::os::unix::fs::symlink(s, &d) {
                    Ok(()) => pairs.push((s.clone(), d)),
                    Err(e) => c.err(s, &e),
                }
            }
        }
        Kind::Extract => {
            for s in &job.srcs {
                let name = s.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
                let stem: String = name.chars().take(crate::fsx::stem_len(&name, false)).collect();
                let d = crate::fsx::free_name(&job.dst, &stem);
                if let Err(e) = std::fs::create_dir_all(&d) {
                    c.err(s, &e);
                    continue;
                }
                c.tick(&name, true);
                let ok = std::process::Command::new("bsdtar").arg("-xf").arg(s).arg("-C").arg(&d).status().map(|st| st.success()).unwrap_or(false);
                if ok {
                    // единственная папка внутри — поднять на уровень вверх
                    let inner: Vec<PathBuf> = std::fs::read_dir(&d).map(|r| r.flatten().map(|e| e.path()).collect()).unwrap_or_default();
                    if inner.len() == 1 && inner[0].is_dir() {
                        let up = crate::fsx::free_name(&job.dst, &inner[0].file_name().unwrap().to_string_lossy());
                        if std::fs::rename(&inner[0], &up).is_ok() {
                            let _ = std::fs::remove_dir(&d);
                            pairs.push((s.clone(), up));
                            continue;
                        }
                    }
                    pairs.push((s.clone(), d));
                } else {
                    let _ = std::fs::remove_dir(&d);
                    c.errors.push(crate::tf!("{}: не удалось распаковать", "{}: could not extract", "{}: não foi possível extrair", name));
                }
            }
        }
        Kind::Compress => {
            let out = crate::fsx::free_name(&job.dst, &job.name);
            c.tick(&job.name, true);
            let mut cmd = std::process::Command::new("bsdtar");
            // «--»: файл с именем «--use-compress-program=…» не станет ключом bsdtar
            cmd.arg("-a").arg("-cf").arg(&out).arg("-C").arg(&job.dst).arg("--");
            for s in &job.srcs {
                cmd.arg(s.strip_prefix(&job.dst).unwrap_or(s));
            }
            if cmd.status().map(|st| st.success()).unwrap_or(false) {
                pairs.push((job.dst.clone(), out));
            } else {
                let _ = std::fs::remove_file(&out);
                c.errors.push(crate::tf!("{}: не удалось сжать", "{}: could not compress", "{}: não foi possível compactar", job.name));
            }
        }
    }
    c.tick("", true);
    send(Ev::Done { id: job.id, kind: job.kind, errors: c.errors, pairs });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx<'a>(send: &'a dyn Fn(Ev)) -> Ctx<'a> {
        Ctx { id: 0, send, done: 0, total: 0, last: Instant::now(), stop: Arc::new(AtomicBool::new(false)), all: None, errors: vec![] }
    }

    #[test]
    fn copies_by_kernel_and_by_hand() {
        let dir = yami_px::paths::private_tmp("ops-test").expect("tmp");
        let big: Vec<u8> = (0..(40u32 << 20) / 4).flat_map(|i| i.to_le_bytes()).collect();
        let cases: Vec<(PathBuf, Vec<u8>)> = vec![
            (dir.join("big"), big),
            (dir.join("empty"), vec![]),
            (dir.join("small"), b"yami".to_vec()),
        ];
        let send = |_: Ev| {};
        for (p, data) in &cases {
            std::fs::write(p, data).unwrap();
            let to = p.with_extension("copy");
            let mut c = ctx(&send);
            copy_file(&mut c, p, &to).unwrap();
            assert_eq!(&std::fs::read(&to).unwrap(), data);
            assert_eq!(c.done, data.len() as u64);
        }
        // размер 0, а данные есть: копия не пустая
        let to = dir.join("status");
        let mut c = ctx(&send);
        copy_file(&mut c, Path::new("/proc/self/status"), &to).unwrap();
        assert!(std::fs::read_to_string(&to).unwrap().contains("Name:"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
