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
        self.sync_bar_outputs();
        self.sync_widgets();
        self.sync_dock();
        self.sync_bar();
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
        if self.press.is_some() || self.st.set == self.set_seen {
            return;
        }
        self.set_seen = self.st.set.clone();
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
                let _ = crate::app::theme_py().arg(&p).status();
                let _ = tx.send(Msg::Theme);
            });
        }
    }

    /// ползунок настроек: значение в настройки, всё зависящее — следом
    pub fn slider_set(&mut self, key: &str, v: f32) {
        if key == "pixel_level" {
            self.set_pixel_level(v.round().clamp(0.0, 3.0) as usize);
            return;
        }
        crate::app::settings_set_f32(&mut self.st.set, key, v);
        self.settings_changed();
    }

    /// пиксельность одним ползунком (настройки → вид): шрифты шелла, файлов, просмотра и программ,
    /// пиксельные значки и курсор. 3 — всё пиксельное (как по умолчанию), 2 — только yami (GTK и Qt
    /// гладкие), 1 — мягче (пиксельные только панели шелла), 0 — гладко везде
    pub fn set_pixel_level(&mut self, lvl: usize) {
        if lvl == crate::settings::pixel_level(&self.st.set) {
            return;
        }
        let (shell, apps, ui, icons, cursor) = match lvl {
            0 => ("sans", "sans", "sans", false, "system"),
            1 => ("", "sans", "sans", false, "yami"),
            2 => ("", "", "sans", true, "yami"),
            _ => ("", "", "", true, "yami"),
        };
        let s = &mut self.st.set;
        let (ui_new, cur_new) = (s.font_ui != ui, s.cursor != cursor);
        s.font_shell = shell.into();
        s.font_files = apps.into();
        s.font_view = apps.into();
        s.font_ui = ui.into();
        s.icon_pixel = icons;
        s.cursor = cursor.into();
        self.settings_changed();
        // программы (GTK, Qt) и курсор перекрашивает theme.py — только если они правда сменились,
        // и не на каждое деление, пока ползунок тянут: один раз при отпускании (look_apply)
        self.look_pending.0 |= ui_new;
        self.look_pending.1 |= cur_new;
        if self.press.is_none() {
            self.look_apply();
        }
    }

    /// отложенное от ползунка пиксельности: шрифты программ и курсор
    pub fn look_apply(&mut self) {
        let (fonts, cursor) = std::mem::take(&mut self.look_pending);
        if fonts {
            self.apply_fonts();
        }
        if cursor {
            self.apply_cursor();
        }
    }

    /// курсор сменили в настройках: theme.py --cursor (niri перечитает свой конфиг сам)
    pub fn apply_cursor(&mut self) {
        // настройки должны быть на диске до запуска скрипта
        let j = self.st.set.to_json();
        let _ = config::write_atomic(&config::settings_path(), &j);
        self.set_saved = j;
        std::thread::spawn(|| {
            let _ = crate::app::theme_py().arg("--cursor").status();
        });
    }

    /// размер шрифта программ сменили (или экраны): theme.py --fonts — GTK, Qt, kitty, браузер
    pub fn apply_fonts(&mut self) {
        let j = self.st.set.to_json();
        let _ = config::write_atomic(&config::settings_path(), &j);
        self.set_saved = j;
        std::thread::spawn(|| {
            let _ = crate::app::theme_py().arg("--fonts").status();
        });
    }

    /// процессы при шелле: ночной свет, модули-скрипты — по настройкам
    pub fn side_sync(&mut self) {
        self.night_sync();
        let s = &self.st.set;
        let mut on_bar: Vec<String> = s.bar_left.iter().chain(&s.bar_center).chain(&s.bar_right).cloned().collect();
        // виджет-скрипт на столе — тот же модуль, запущенный один раз
        on_bar.extend(s.widgets.iter().filter(|w| w.on && w.kind == "script" && !w.source.is_empty()).map(|w| format!("x:{}", w.source)));
        on_bar.dedup();
        self.st.plugins.sync(&on_bar, &self.tx);
    }

    /// yami-fx.kdl — только если он другой: каждая запись — перечитывание конфига niri
    pub fn write_fx(&mut self) {
        let p = config::config_dir().join("niri/yami-fx.kdl");
        let text = self.st.set.fx_kdl(self.st.niri.level, self.st.curtain.is_some());
        if std::fs::read_to_string(&p).ok().as_deref() != Some(text.as_str()) {
            let _ = config::write_atomic(&p, &text);
        }
    }
}
