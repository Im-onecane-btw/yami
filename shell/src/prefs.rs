// Применение настроек: любое изменение (панель, правка yami-shell.json руками, CLI) пересобирает
// тему и шрифты, переписывает то, что читают niri и другие программы, перезапускает процессы шелла.

use crate::app::*;
use crate::config::{self, Settings};
use crate::theme::Theme;
use crate::font::Fonts;


impl App {
    // ── настройки ───────────────────────────────────────────────────────────
    pub fn settings_changed(&mut self) {
        let s = &self.st.set;
        crate::i18n::set(&s.lang);
        if s.font_shell != self.res.font {
            self.res.font = s.font_shell.clone();
            self.res.fonts = Fonts::with(&self.res.font);
        }
        self.st.t = Theme::new(s.accent_rgb(), s.tint, s.glass, s.lift, s.inner, s.light).apply();
        yami_px::gfx::set_reveal(yami_px::gfx::Reveal::from_id(&s.reveal), s.reveal_grain);
        self.res.icons.set_pixel(s.icon_pixel.then(|| self.st.t.icon_ramp()));
        self.write_palette();
        self.write_fx();
        self.side_sync();
        crate::agents::write_portals(&self.st.set);
        self.sync_wall();
        crate::keys::write(&self.st.set);
        self.setup_idle();
        self.sync_widgets();
        self.sync_dock();
        self.dirty_all();
    }

    /// записать настройки сейчас (перед перезапуском), даже посреди перетаскивания
    pub fn save_now(&mut self) {
        let j = self.st.set.to_json();
        if j != self.set_saved {
            let _ = config::write_atomic(&config::settings_path(), &j);
            self.set_saved = j;
        }
    }

    pub(crate) fn save_settings_if_changed(&mut self) {
        if self.press.is_some() {
            return;
        }
        let j = self.st.set.to_json();
        if j != self.set_saved {
            let _ = config::write_atomic(&config::settings_path(), &j);
            self.set_saved = j;
            self.set_mtime = std::fs::metadata(config::settings_path()).and_then(|m| m.modified()).ok();
        }
    }

    pub(crate) fn check_settings_file(&mut self) {
        let m = std::fs::metadata(config::settings_path()).and_then(|m| m.modified()).ok();
        if m != self.set_mtime {
            self.set_mtime = m;
            let s = Settings::load();
            if s != self.st.set {
                self.st.set = s;
                self.set_saved = self.st.set.to_json();
                self.settings_changed();
            }
        }
    }

    /// палитра для остальных программ; tools/theme.py перекрашивает их и рисует обои
    pub fn write_palette(&mut self) {
        let j = self.st.t.palette_json();
        if j == self.palette_written {
            return;
        }
        self.palette_written = j.clone();
        let p = config::state_dir().join("palette.json");
        let changed = std::fs::read_to_string(&p).map(|t| t != j).unwrap_or(true);
        let _ = config::write_atomic(&p, &j);
        if changed || !config::state_dir().join("wallpaper.png").exists() {
            let tx = self.tx.clone();
            std::thread::spawn(move || {
                let script = crate::app::theme_script();
                let _ = std::process::Command::new("python3").arg("-I").arg(&script).arg(&p).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
                let _ = tx.send(Msg::Theme);
            });
        }
    }

    /// курсор сменили в настройках: theme.py --cursor (niri перечитает свой конфиг сам)
    pub fn apply_cursor(&mut self) {
        // настройки должны быть на диске до запуска скрипта
        let j = self.st.set.to_json();
        let _ = config::write_atomic(&config::settings_path(), &j);
        self.set_saved = j;
        std::thread::spawn(|| {
            let _ = std::process::Command::new("python3").arg("-I").arg(crate::app::theme_script()).arg("--cursor").stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
        });
    }

    /// размер шрифта программ сменили (или экраны): theme.py --fonts — GTK, Qt, kitty, браузер
    pub fn apply_fonts(&mut self) {
        let j = self.st.set.to_json();
        let _ = config::write_atomic(&config::settings_path(), &j);
        self.set_saved = j;
        std::thread::spawn(|| {
            let _ = std::process::Command::new("python3").arg("-I").arg(crate::app::theme_script()).arg("--fonts").stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
        });
    }

    /// процессы при шелле: ночной свет, модули-скрипты — по настройкам
    pub fn side_sync(&mut self) {
        self.night_sync();
        let s = &self.st.set;
        let on_bar: Vec<String> = s.bar_left.iter().chain(&s.bar_center).chain(&s.bar_right).cloned().collect();
        self.st.plugins.sync(&on_bar, &self.tx);
    }

    pub fn write_fx(&mut self) {
        let _ = config::write_atomic(&config::config_dir().join("niri/yami-fx.kdl"), &self.st.set.fx_kdl(self.st.niri.level, self.st.curtain.is_some()));
    }
}
