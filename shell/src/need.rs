// Пакеты, которых не хватает выбранным частям установки (ночной свет, текст со скриншота,
// GameMode…). Установщик (install.sh, do_need_pkgs) пишет
// ~/.local/state/yami/packages-needed: «pm pacman» и «pkgs a b c» — только то, что есть в
// репозиториях и не стоит. Обновление идёт без sudo и само их не поставит: `yami update need`
// печатает одну команду, `--run` выполняет её здесь же, вкладка «обновления» открывает её в
// терминале (пароль sudo спрашивает там). Команду собирает шелл по своей таблице: из файла
// берутся только имена пакетов, и только безопасные.
// i18n: inline pairs

use std::path::PathBuf;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Need {
    pub pm: String,
    pub pkgs: Vec<String>,
}

pub fn path() -> PathBuf {
    crate::config::state_dir().join("packages-needed")
}

/// имя пакета: буквы, цифры и «+._-/:» (gentoo-категории, :arch у debian), не с «-»
fn name_ok(p: &str) -> bool {
    !p.is_empty() && p.len() <= 96 && !p.starts_with('-') && p.chars().all(|c| c.is_ascii_alphanumeric() || "+._-/:".contains(c))
}

/// установка пакетов у этого менеджера
fn base(pm: &str) -> Option<&'static [&'static str]> {
    Some(match pm {
        "pacman" => &["pacman", "-S", "--needed"],
        "dnf" => &["dnf", "install"],
        "rpm-ostree" => &["rpm-ostree", "install"],
        "apt" => &["apt", "install"],
        "zypper" => &["zypper", "install"],
        "xbps" => &["xbps-install"],
        "apk" => &["apk", "add"],
        _ => return None,
    })
}

pub fn parse(t: &str) -> Option<Need> {
    let mut n = Need::default();
    for l in t.lines() {
        if let Some(v) = l.strip_prefix("pm ") {
            n.pm = v.trim().to_string();
        } else if let Some(v) = l.strip_prefix("pkgs ") {
            n.pkgs = v.split_whitespace().filter(|p| name_ok(p)).map(String::from).collect();
        }
    }
    (base(&n.pm).is_some() && !n.pkgs.is_empty()).then_some(n)
}

pub fn load() -> Option<Need> {
    parse(&std::fs::read_to_string(path()).ok()?)
}

impl Need {
    pub fn argv(&self) -> Vec<String> {
        let su = if crate::doctor::which("sudo") || !crate::doctor::which("doas") { "sudo" } else { "doas" };
        let mut v = vec![su.to_string()];
        v.extend(base(&self.pm).unwrap_or(&[]).iter().map(|s| s.to_string()));
        v.extend(self.pkgs.iter().cloned());
        v
    }
    pub fn line(&self) -> String {
        self.argv().join(" ")
    }
}

fn sq(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// открыть команду в терминале; после успеха файл убирается
pub fn run_in_term(n: &Need) {
    let script = format!("{} && rm -f {}; printf '\\n[Enter]'; read _", n.line(), sq(&path().to_string_lossy()));
    crate::app::App::spawn(&crate::launcher::in_term(vec!["sh".into(), "-c".into(), script]));
}

impl crate::app::App {
    /// раз в секунду, пока открыта вкладка «обновления»: пакеты поставили (терминал удалил файл)
    /// — кнопка уходит; закрыли терминал раньше — остаётся
    pub fn need_tick(&mut self) {
        if !matches!(self.st.modal, Some(crate::app::Modal::Settings)) || self.st.stab != "updates" {
            return;
        }
        let n = load();
        if n != self.st.upd.need {
            self.st.upd.need = n;
            self.dirty(&crate::app::SKind::Modal);
        }
    }
}

/// `yami update need [--print | --run]`
pub fn cli(y: &crate::cmd::Sty, args: &[&str]) -> i32 {
    let Some(n) = load() else {
        if !args.contains(&"--print") {
            println!(" {} {}", y.ok("✓"), crate::tf!("всех пакетов хватает", "no packages missing"));
        }
        return 0;
    };
    if args.contains(&"--print") {
        println!("{}", n.line());
        return 0;
    }
    if args.contains(&"--run") {
        let a = n.argv();
        let ok = std::process::Command::new(&a[0]).args(&a[1..]).status().map(|s| s.success()).unwrap_or(false);
        if ok {
            let _ = std::fs::remove_file(path());
            println!(" {} {}", y.ok("✓"), crate::tf!("поставлено", "installed"));
            return 0;
        }
        eprintln!(" {} {}", y.bad("✕"), crate::tf!("не вышло — команда выше", "failed, see the command above"));
        return 1;
    }
    y.section(&crate::tf!("не хватает пакетов", "missing packages"));
    println!("   {}", n.pkgs.join(" "));
    println!("\n   {}  {}", y.bold(&n.line()), y.dim(&crate::tf!("(или: yami update need --run)", "(or: yami update need --run)")));
    println!();
    0
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_safe_names() {
        let n = super::parse("pm pacman\npkgs wlsunset zbar ;rm -rf --no-preserve-root $(id)\n").unwrap();
        assert_eq!(n.pkgs, vec!["wlsunset", "zbar"]);
        assert!(super::parse("pm sh -c\npkgs a\n").is_none());
        assert!(super::parse("pm apt\npkgs\n").is_none());
    }
}
