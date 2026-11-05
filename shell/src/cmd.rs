// Командная строка: `yami <команда>` (ссылка ~/.local/bin/yami → yami-shell) и то же самое
// через `yami-shell <команда>`. Без команды `yami` печатает справку, а `yami-shell` запускает
// шелл (так его зовёт niri). Справка — в цветах палитры, на языке шелла; NO_COLOR — без цвета.
// Команды для настроек — cli.rs, проверка системы — doctor.rs, обновления — update.rs.
// i18n: inline pairs

use crate::config::Settings;
use std::io::IsTerminal;

/// раскраска терминала: акцент палитры (truecolor), тусклый, жирный, итоги проверок
pub struct Sty {
    on: bool,
    acc: (u8, u8, u8),
}

impl Sty {
    pub fn new(s: &Settings) -> Sty {
        let on = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none() && std::env::var("TERM").map(|t| t != "dumb").unwrap_or(true);
        let a = s.accent_rgb();
        Sty { on, acc: ((a >> 16) as u8, (a >> 8) as u8, a as u8) }
    }
    fn wrap(&self, code: &str, s: &str) -> String {
        if self.on {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
    pub fn acc(&self, s: &str) -> String {
        self.wrap(&format!("38;2;{};{};{}", self.acc.0, self.acc.1, self.acc.2), s)
    }
    pub fn head(&self, s: &str) -> String {
        self.wrap(&format!("1;38;2;{};{};{}", self.acc.0, self.acc.1, self.acc.2), s)
    }
    pub fn bold(&self, s: &str) -> String {
        self.wrap("1", s)
    }
    pub fn dim(&self, s: &str) -> String {
        self.wrap("2", s)
    }
    pub fn ok(&self, s: &str) -> String {
        self.wrap("32", s)
    }
    pub fn warn(&self, s: &str) -> String {
        self.wrap("33", s)
    }
    pub fn bad(&self, s: &str) -> String {
        self.wrap("31", s)
    }
    /// цветной квадратик (образец палитры)
    pub fn swatch(&self, rgb: u32) -> String {
        if self.on {
            format!("\x1b[38;2;{};{};{}m██\x1b[0m", (rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
        } else {
            String::new()
        }
    }
    /// «▪ заголовок» и строки «  имя   что делает»
    pub fn section(&self, name: &str) {
        println!("\n {} {}", self.acc("▪"), self.head(&name.to_uppercase()));
    }
    pub fn row(&self, cmd: &str, what: &str) {
        let pad = 24usize.saturating_sub(cmd.chars().count());
        println!("   {}{}{}", self.bold(cmd), " ".repeat(pad), self.dim(what));
    }
}

fn banner(y: &Sty) {
    println!();
    println!(" {} {}  {}", y.acc("▚▞"), y.head(&format!("YAMI {}", env!("CARGO_PKG_VERSION"))), y.dim(&crate::tf!("пиксельный рабочий стол для niri", "pixel desktop for niri")));
}

fn help(y: &Sty, me: &str) {
    banner(y);
    println!("\n {}  {me} {} {}", y.dim(&crate::tf!("как", "usage")), y.acc(&crate::tf!("<команда>", "<command>")), y.dim(&crate::tf!("[аргументы]", "[args]")));
    y.section(&crate::tf!("сеанс", "session"));
    y.row("open <panel>", &crate::tf!("launcher clipboard settings[:вкладка] power lock screenshot …", "launcher clipboard settings[:tab] power lock screenshot …"));
    y.row("msg <cmd>", &crate::tf!("прямая команда шеллу (yami help msg)", "a raw command to the running shell (yami help msg)"));
    y.row("restart", &crate::tf!("перезапустить шелл (подождёт, если играешь)", "restart the shell (waits while you play)"));
    y.row("status", &crate::tf!("что работает: шелл, портал, niri, службы D-Bus", "what is running: shell, portal, niri, D-Bus services"));
    y.row("run", &crate::tf!("запустить шелл (так делает niri)", "run the shell (niri does this)"));
    y.section(&crate::tf!("вид", "look"));
    y.row("palette [name|#hex]", &crate::tf!("палитры или сменить: blood ash sakura …", "list palettes or switch: blood ash sakura …"));
    y.row("wallpaper [path]", &crate::tf!("свои обои; без пути — пиксельные из палитры", "own picture; no path: the palette's pixel art"));
    y.section(&crate::tf!("настройки", "config"));
    y.row("theme export | import FILE", &crate::tf!("тема одним файлом: поделиться и поставить чужую (только вид)", "a theme as one file: share yours, apply others' (the look only)"));
    y.row("export | import FILE", &crate::tf!("все настройки одним файлом: переезд на новую систему", "every setting as one file: move to a new system"));
    y.row("config …", &crate::tf!("все настройки: list get set reset check path kdl", "every setting: list get set reset check path kdl"));
    y.row("binds …", &crate::tf!("клавиши: list print write", "keybindings: list print write"));
    y.section(&crate::tf!("обслуживание", "maintenance"));
    y.row("update [check]", &crate::tf!("что нового в репозитории", "what is new upstream"));
    y.row("update apply", &crate::tf!("собрать и поставить новое (прежнее — в откат)", "build and install it (the old one is kept)"));
    y.row("update rollback", &crate::tf!("вернуть то, что было до обновления", "go back to what was there before"));
    y.row("update status | log", &crate::tf!("что стоит, откуда, последний лог", "what is installed, from where, the last log"));
    y.row("doctor", &crate::tf!("проверить зависимости и службы, сказать, чего нет", "check dependencies and services, say what is missing"));
    y.row("doctor games", &crate::tf!("ntsync, gamemode, MangoHud: что есть и как добавить", "ntsync, gamemode, MangoHud: what is there, how to add it"));
    y.row("ocr FILE", &crate::tf!("текст с картинки (как «текст» на скриншоте)", "text from an image (like «text» in screenshots)"));
    y.row("report [-o FILE]", &crate::tf!("архив для issue: версии, doctor, журнал, конфиги без секретов", "an archive for an issue: versions, doctor, journal, configs without secrets"));
    y.row("clean [--dry-run]", &crate::tf!("убрать кеш сборки: исходники yami-niri и target шелла (гигабайты)", "remove the build cache: yami-niri sources and the shell's target (gigabytes)"));
    y.row("version", &crate::tf!("версии всех частей", "versions of every part"));
    y.row("shot …", &crate::tf!("панели в PNG без Wayland (разработка)", "panels to PNG without Wayland (development)"));
    println!("\n {} {}", y.dim(&crate::tf!("подробнее:", "more:")), y.dim(&crate::tf!("yami help <команда> · docs/CONFIG.md · NO_COLOR=1 — без цвета", "yami help <command> · docs/CONFIG.md · NO_COLOR=1 for plain output")));
    println!();
}

fn help_msg(y: &Sty) {
    y.section(&crate::tf!("команды шеллу", "shell commands"));
    for (c, ru, en) in [
        ("launcher | clipboard", "программы / буфер обмена", "apps / clipboard history"),
        ("settings [tab]", "настройки (на вкладке)", "settings (on a tab)"),
        ("power | lock", "питание / заблокировать", "power menu / lock"),
        ("screenshot [area|window|screen] [s]", "скриншот, задержка в секундах", "screenshot, delay in seconds"),
        ("popup cal|sound|media|net|bt|sys|profile", "всплывашка бара", "a bar popup"),
        ("bar | edit | stacking", "бар / правка виджетов / режим «стол»", "bar / edit widgets / desk mode"),
        ("minimize | restore", "свернуть / вернуть окно", "minimize / restore a window"),
        ("wallpaper [path] | curtain [on|off]", "обои / шторка для трансляций", "wallpaper / privacy curtain"),
        ("profile", "следующий профиль питания", "next power profile"),
        ("night", "ночной свет вкл-выкл", "night light on-off"),
        ("reload | restart | quit", "перечитать настройки / перезапуск / выход", "reload settings / restart / quit"),
    ] {
        y.row(c, if crate::i18n::en() { en } else { ru });
    }
    println!();
}

fn help_open(y: &Sty) {
    y.section(&crate::tf!("что открыть", "what to open"));
    y.row("launcher clipboard power", "");
    y.row("settings[:tab]", &crate::settings::TABS.iter().map(|t| t.0).collect::<Vec<_>>().join(" "));
    y.row("screenshot[:area|window|screen]", "");
    y.row("cal sound media net bt sys profile", &crate::tf!("всплывашки бара", "bar popups"));
    println!();
}

fn help_update(y: &Sty) {
    y.section(&crate::tf!("обновления", "updates"));
    y.row("update [check]", &crate::tf!("git fetch своего клона (~/.local/share/yami/src), что нового", "git fetch of its own clone (~/.local/share/yami/src), what is new"));
    y.row("update apply [-y]", &crate::tf!("проверить подпись, собрать из исходников, поставить", "check the signature, build from source, install"));
    y.row("update rollback", &crate::tf!("бинари, что стояли до последнего обновления (сразу)", "the binaries from before the last update (instant)"));
    y.row("update list", &crate::tf!("выпуски, на которые можно перейти, и их подписи", "releases you can switch to and their signatures"));
    y.row("update to X.Y.Z", &crate::tf!("собрать и поставить выбранный подписанный выпуск", "build and install the chosen signed release"));
    y.row("update status", &crate::tf!("версия, коммит, канал, источник", "version, commit, channel, source"));
    y.row("update log", &crate::tf!("лог последнего обновления", "the log of the last update"));
    y.row("update need [--run]", &crate::tf!("пакеты, которых не хватает новым функциям: одна команда (--run — поставить)", "packages new features miss: one command (--run installs them)"));
    println!("\n   {}", y.dim(&crate::tf!("канал, подписи, расписание: настройки → обновления или yami config set updateChannel main", "channel, signatures, schedule: settings → updates or yami config set updateChannel main")));
    println!();
}

/// шелл не отвечает — понятная ошибка вместо молчания
/// команда шеллу понятна ему (App::ipc): опечатка — ошибка здесь, а не молчаливое «ok»
pub fn msg_ok(cmd: &str) -> bool {
    let mut w = cmd.split_whitespace();
    let head = w.next().unwrap_or("");
    let one = |w: &mut std::str::SplitWhitespace| w.next().is_none();
    match head {
        "launcher" | "clipboard" | "power" | "lock" | "bar" | "stacking" | "minimize" | "restore" | "edit" | "reload" | "quit" | "ping" | "restart" | "profile" | "night" => one(&mut w),
        "settings" => w.next().is_none_or(|t| crate::settings::TABS.iter().any(|x| x.0 == t)) && one(&mut w),
        "popup" => matches!(w.next(), Some("cal" | "sound" | "media" | "net" | "bt" | "sys" | "profile")) && one(&mut w),
        "screenshot" => w.next().is_none_or(|m| matches!(m, "area" | "window" | "screen")) && w.next().is_none_or(|d| d.parse::<u32>().is_ok()) && one(&mut w),
        "curtain" => w.next().is_none_or(|a| matches!(a, "on" | "off" | "toggle")) && one(&mut w),
        // путь к картинке (с пробелами) или пусто — пиксельные из палитры
        "wallpaper" => true,
        h => h.starts_with("update-"),
    }
}

fn send(y: &Sty, cmd: &str) -> i32 {
    if crate::ipc::send(cmd) {
        0
    } else {
        eprintln!(" {} {}", y.bad("✕"), crate::tf!("шелл не запущен (yami run или войди в сеанс niri)", "the shell is not running (yami run, or log into a niri session)"));
        1
    }
}

fn open(y: &Sty, what: &str) -> i32 {
    let (a, b) = what.split_once(':').unwrap_or((what, ""));
    let cmd = match a {
        "launcher" | "clipboard" | "power" | "lock" => a.to_string(),
        "settings" if b.is_empty() => "settings".into(),
        "settings" => format!("settings {b}"),
        "screenshot" => format!("screenshot {}", if b.is_empty() { "area" } else { b }).trim().to_string(),
        "cal" | "sound" | "media" | "net" | "bt" | "sys" | "profile" => format!("popup {a}"),
        _ => {
            help_open(y);
            return 2;
        }
    };
    send(y, &cmd)
}

fn palette(y: &Sty, args: &[&str]) -> i32 {
    let mut s = Settings::load();
    match args.first() {
        None => {
            y.section(&crate::tf!("палитры", "palettes"));
            let cur = s.accent_rgb();
            for p in crate::theme::PRESETS {
                let mark = if p.accent == cur { y.acc("●") } else { " ".into() };
                let name = if crate::i18n::en() { p.id.to_string() } else { format!("{} · {}", p.id, p.name) }; // i18n-ok: preset names are Russian data
                println!("   {mark} {} {}  {}", y.swatch(p.accent), y.bold(&format!("{:<8}", p.id)), y.dim(&format!("#{:06x}  {name}", p.accent)));
            }
            if crate::theme::PRESETS.iter().all(|p| p.accent != cur) {
                println!("   {} {} {}", y.acc("●"), y.swatch(cur), y.bold(&format!("#{cur:06x}")));
            }
            println!("\n   {}\n", y.dim(&crate::tf!("сменить: yami palette ash  ·  свой цвет: yami palette '#3a9fd0'", "switch: yami palette ash  ·  own colour: yami palette '#3a9fd0'")));
            0
        }
        Some(v) => {
            if crate::theme::parse_hex(v).is_some() {
                s.accent = v.to_string();
            } else if let Some(p) = crate::theme::PRESETS.iter().find(|p| p.id == *v) {
                s.palette = p.id.into();
                s.accent.clear();
            } else {
                eprintln!(" {} {}", y.bad("✕"), crate::tf!("нет такой палитры: {} (yami palette — список)", "no such palette: {} (yami palette lists them)", v));
                return 2;
            }
            // выбрана руками: обои больше не перекрашивают
            s.wall_accent = false;
            if crate::config::write_atomic(&crate::config::settings_path(), &s.to_json()).is_err() {
                return 1;
            }
            println!(" {} {} {}", y.swatch(s.accent_rgb()), y.bold(v), y.dim(&crate::tf!("— шелл перекрасит всё сам", "— the shell recolours everything itself")));
            0
        }
    }
}

fn version(y: &Sty) -> i32 {
    let st = crate::update::installed();
    let out = |c: &str| std::process::Command::new(c).arg("--version").output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    banner(y);
    println!();
    let row = |k: &str, v: String| println!("   {}{}", y.dim(&format!("{k:<14}")), v);
    let commit: String = st.commit.chars().take(8).collect();
    row("yami", format!("{}{}", if st.version.is_empty() { env!("CARGO_PKG_VERSION") } else { &st.version }, if commit.is_empty() { String::new() } else { format!("  {}", y.dim(&commit)) }));
    row("yami-shell", env!("CARGO_PKG_VERSION").into());
    for b in ["yami-files", "yami-view", "yami-play"] {
        row(b, out(b).map(|v| v.split_whitespace().last().unwrap_or("").to_string()).unwrap_or_else(|| y.warn(&crate::tf!("не найден", "not found"))));
    }
    row("niri", out("niri").unwrap_or_else(|| y.warn(&crate::tf!("не найден", "not found"))));
    row(&crate::tf!("канал", "channel"), Settings::load().update_channel);
    row(&crate::tf!("источник", "source"), crate::update::short_repo(&st.repo));
    println!();
    0
}

fn status(y: &Sty) -> i32 {
    banner(y);
    crate::doctor::status(y);
    println!();
    0
}

fn restart(y: &Sty, force: bool) -> i32 {
    if !force && crate::niri::game_focused() {
        eprintln!(" {} {}", y.warn("!"), crate::tf!("в фокусе игра — не перезапускаю (yami restart --force)", "a game has focus: not restarting (yami restart --force)"));
        return 3;
    }
    send(y, "restart")
}

fn update(y: &Sty, args: &[&str]) -> i32 {
    let set = Settings::load();
    let notify = args.contains(&"--notify");
    let raw = args;
    let args: Vec<&str> = args.iter().copied().filter(|a| !a.starts_with('-')).collect();
    match args.first().copied().unwrap_or("check") {
        "check" => {
            println!(" {} {}", y.acc("▪"), crate::tf!("проверяю {}…", "checking {}…", crate::update::short_repo(&crate::update::installed().repo)));
            let i = crate::update::check(&set);
            let _ = crate::ipc::send("update-reload");
            if !i.err.is_empty() {
                eprintln!(" {} {}", y.bad("✕"), i.err);
                return 1;
            }
            if i.reference.is_empty() {
                println!(" {} {}", y.ok("✓"), crate::tf!("выпусков ещё нет; всё новое в main: yami config set updateChannel main", "no releases yet; everything new on main: yami config set updateChannel main"));
                return 0;
            }
            if !i.available {
                println!(" {} {}", y.ok("✓"), crate::tf!("всё свежее ({})", "up to date ({})", i.latest));
                return 0;
            }
            println!(" {} {}  {}", y.head(&crate::tf!("доступно {}", "{} is available", i.latest)), y.dim(&i.reference), crate::update::sig_text(&i.sig));
            for n in &i.notes {
                println!("   {} {}", y.acc("·"), n);
            }
            if !i.sig_ok(&set.update_verify) {
                println!(" {} {}", y.warn("!"), crate::tf!("подпись не проходит правило «{}» — ставить не буду", "the signature fails the «{}» rule: it will not be installed", set.update_verify));
            } else {
                println!("\n   {}", y.dim(&crate::tf!("поставить: yami update apply", "install: yami update apply")));
            }
            0
        }
        "apply" => {
            let r = crate::update::apply(&set);
            let (ok, text) = match &r {
                Ok(t) => (true, t.clone()),
                Err(e) => (false, e.clone()),
            };
            if notify {
                let _ = crate::ipc::send(&format!("update-done {} {text}", if ok { "ok" } else { "fail" }));
            }
            if ok {
                // шелл перезапустил установщик (или подождёт игру)
                println!(" {} {text}", y.ok("✓"));
                0
            } else {
                eprintln!(" {} {text}", y.bad("✕"));
                1
            }
        }
        "rollback" => match crate::update::rollback() {
            Ok(t) => {
                if notify {
                    let _ = crate::ipc::send(&format!("update-done ok {t}"));
                }
                println!(" {} {t}", y.ok("✓"));
                0
            }
            Err(e) => {
                if notify {
                    let _ = crate::ipc::send(&format!("update-done fail {e}"));
                }
                eprintln!(" {} {e}", y.bad("✕"));
                1
            }
        },
        "list" => {
            let i = crate::update::check(&set);
            if !i.err.is_empty() {
                eprintln!(" {} {}", y.bad("✕"), i.err);
                return 1;
            }
            let cur = crate::update::installed().version;
            y.section(&crate::tf!("выпуски", "releases"));
            if let Some(p) = crate::update::prev_version() {
                println!("   {} {}", y.acc(&format!("{:<10}", "previous")), y.dim(&crate::tf!("сборка до последнего обновления ({}) · yami update rollback", "the build before the last update ({}) · yami update rollback", p)));
            }
            for r in &i.releases {
                let mark = if r.version == cur { y.ok(&crate::tf!("стоит", "installed")) } else if r.sig.starts_with("good:") || r.sig == "nokeys" || r.sig == "off" { y.dim(&format!("yami update to {}", r.version)) } else { y.bad(&crate::tf!("не ставится", "will not install")) };
                println!("   {:<10} {}  {}", r.tag, crate::update::sig_text(&r.sig), mark);
            }
            if i.releases.is_empty() {
                println!("   {}", y.dim(&crate::tf!("выпусков ещё нет", "no releases yet")));
            }
            println!();
            0
        }
        "to" => {
            let Some(want) = args.get(1) else {
                eprintln!(" {} {}", y.bad("✕"), crate::tf!("какую версию? yami update list", "which version? yami update list"));
                return 2;
            };
            let r = crate::update::install_tag(&set, want);
            let (ok, text) = match &r {
                Ok(t) => (true, t.clone()),
                Err(e) => (false, e.clone()),
            };
            if notify {
                let _ = crate::ipc::send(&format!("update-done {} {text}", if ok { "ok" } else { "fail" }));
            }
            if ok {
                println!(" {} {text}", y.ok("✓"));
                0
            } else {
                eprintln!(" {} {text}", y.bad("✕"));
                1
            }
        }
        "status" => {
            let st = crate::update::installed();
            y.section(&crate::tf!("установлено", "installed"));
            let row = |k: &str, v: &str| println!("   {}{}", y.dim(&format!("{k:<14}")), v);
            row(&crate::tf!("версия", "version"), if st.version.is_empty() { "?" } else { &st.version });
            row(&crate::tf!("коммит", "commit"), &format!("{}{}", st.commit, if st.dirty { crate::tf!(" (с правками)", " (with local edits)") } else { String::new() }));
            row(&crate::tf!("источник", "source"), if st.repo.is_empty() { crate::update::REPO } else { &st.repo });
            row(&crate::tf!("канал", "channel"), &set.update_channel);
            row(&crate::tf!("подписи", "signatures"), &set.update_verify);
            row(&crate::tf!("клон", "clone"), &crate::update::clone_dir().display().to_string());
            row(&crate::tf!("части", "components"), &st.components);
            println!();
            0
        }
        "log" => match std::fs::read_to_string(crate::update::log_path()) {
            Ok(t) => {
                print!("{t}");
                0
            }
            Err(_) => {
                println!("{}", crate::tf!("обновлений ещё не было", "no update yet"));
                0
            }
        },
        "need" => crate::need::cli(y, raw),
        "stamp" => {
            // установщик и tools/deploy.sh: `update stamp DIR [--components "core apps …"]`
            let all: Vec<String> = std::env::args().collect();
            let dir = args.get(1).map(std::path::PathBuf::from).unwrap_or_else(|| ".".into());
            let comp = all.iter().position(|a| a == "--components").and_then(|i| all.get(i + 1)).cloned().unwrap_or_default();
            match crate::update::stamp(&dir, &comp) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("{e}");
                    1
                }
            }
        }
        _ => {
            help_update(y);
            2
        }
    }
}

/// разбор команды; as_yami — вызвали как `yami` (без команды — справка, а не запуск шелла)
pub fn run(args: &[String], me: &str) -> i32 {
    let set = Settings::load();
    crate::i18n::set(&set.lang);
    let y = Sty::new(&set);
    let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    // `… | head` не должен падать с паникой
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL) };
    match a.first().copied() {
        None | Some("help") | Some("-h") | Some("--help") => {
            match a.get(1).copied() {
                Some("msg") => help_msg(&y),
                Some("open") => help_open(&y),
                Some("update") => help_update(&y),
                Some("config") | Some("binds") => {
                    let _ = crate::cli::config(&["help".to_string()]);
                }
                _ => help(&y, me),
            }
            0
        }
        Some("run") => {
            crate::config::migrate();
            crate::app::run();
            0
        }
        Some("--version") | Some("-V") => {
            println!("yami-shell {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Some("version") => version(&y),
        Some("msg") => {
            let cmd = a[1..].join(" ");
            if cmd.is_empty() {
                help_msg(&y);
                return 2;
            }
            if !msg_ok(&cmd) {
                eprintln!(" {} {}", y.bad("✕"), crate::tf!("шелл не знает такой команды: {} (yami help msg)", "the shell has no such command: {} (yami help msg)", cmd));
                return 2;
            }
            send(&y, &cmd)
        }
        Some("open") => match a.get(1) {
            Some(w) => open(&y, w),
            None => {
                help_open(&y);
                2
            }
        },
        Some("restart") => restart(&y, a.contains(&"--force")),
        Some("status") => status(&y),
        Some("doctor") if a.get(1) == Some(&"games") => crate::gameboost::doctor(&y),
        Some("doctor") => crate::doctor::run(&y),
        Some("report") => crate::report::run(&y, &a[1..]),
        Some("ocr") => crate::ocr::cli(&y, &a[1..]),
        Some("theme") => match a.get(1) {
            Some(&"export") => crate::bundle::run(&y, "theme-export", &a[2..]),
            Some(&"import") => crate::bundle::run(&y, "theme-import", &a[2..]),
            _ => crate::bundle::run(&y, "", &[]),
        },
        Some("export") => crate::bundle::run(&y, "export", &a[1..]),
        Some("import") => crate::bundle::run(&y, "import", &a[1..]),
        Some("clean") => crate::clean::run(&y, &a[1..]),
        Some("palette") => palette(&y, &a[1..]),
        Some("wallpaper") => {
            // шелл живёт в другой папке: путь — абсолютный
            let p = a[1..].join(" ");
            let p = if p.is_empty() { p } else { std::fs::canonicalize(&p).map(|x| x.display().to_string()).unwrap_or(p) };
            send(&y, format!("wallpaper {p}").trim())
        }
        Some("update") => update(&y, &a[1..]),
        Some("shot") => crate::shot::run(&args[1..]),
        Some("config") => crate::cli::config(&args[1..]),
        Some("binds") => crate::cli::binds(&args[1..]),
        Some(c) => {
            eprintln!(" {} {}", y.bad("✕"), crate::tf!("нет такой команды: {} (yami help)", "no such command: {} (yami help)", c));
            2
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn msg_commands() {
        for ok in ["launcher", "popup cal", "popup media", "screenshot window 3", "screenshot", "settings keys", "wallpaper /a b.png", "wallpaper", "curtain on", "update-progress 1/3 x", "night"] {
            assert!(super::msg_ok(ok), "{ok}");
        }
        for bad in ["cal", "popup foo", "screenshot win", "settings nope", "launcher now", "curtain maybe", ""] {
            assert!(!super::msg_ok(bad), "{bad}");
        }
        // каждая команда, что вешается на клавиши, шеллу понятна
        for b in crate::keys::BINDS {
            if let Some(i) = b.act.find("\"msg\"") {
                let cmd: Vec<&str> = b.act[i + 5..].split('"').filter(|s| !s.trim().is_empty() && s.trim() != ";").collect();
                let cmd = cmd.join(" ");
                assert!(super::msg_ok(cmd.trim_end_matches(';').trim()), "{}: {cmd}", b.id);
            }
        }
    }
}
