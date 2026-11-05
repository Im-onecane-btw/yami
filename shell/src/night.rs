// Ночной свет: вечером цвета теплее через wlsunset (wlr-gamma-control, он у niri есть). От заката
// до восхода по координатам или по часам, если координат нет. Процесс принадлежит шеллу и
// умирает вместе с ним. Модуль бара `night` показывает и переключает.
// i18n: inline pairs

use crate::app::{App, SKind};
use std::process::{Child, Command, Stdio};

#[derive(Default)]
pub struct Night {
    /// работал при последней проверке (по этому рисует бар, обновляет tick)
    pub on: bool,
    child: Option<Child>,
    args: Vec<String>,
}

impl Night {
    pub fn running(&mut self) -> bool {
        match &mut self.child {
            Some(c) => matches!(c.try_wait(), Ok(None)),
            None => false,
        }
    }
    fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            // wlsunset сам возвращает гамму, когда завершается
            unsafe { libc::kill(c.id() as i32, libc::SIGTERM) };
            let _ = c.wait();
        }
    }
}

impl Drop for Night {
    fn drop(&mut self) {
        self.stop();
    }
}

/// «55.75, 37.62» → (широта, долгота)
pub fn geo(s: &str) -> Option<(f64, f64)> {
    let (a, b) = s.split_once([',', ' '])?;
    let (lat, lon) = (a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?);
    ((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon)).then_some((lat, lon))
}

/// аргументы wlsunset по настройкам
pub fn args(set: &crate::config::Settings) -> Vec<String> {
    let mut a = vec!["-t".to_string(), set.night_temp.clamp(1500, 6500).to_string(), "-T".into(), "6500".into()];
    match geo(&set.night_geo) {
        Some((lat, lon)) => a.extend(["-l".into(), format!("{lat}"), "-L".into(), format!("{lon}")]),
        None => {
            let h = |x: i32| format!("{:02}:00", x.clamp(0, 23));
            a.extend(["-S".into(), h(set.night_off_h), "-s".into(), h(set.night_on_h)]);
        }
    }
    a
}

impl App {
    pub fn night_sync(&mut self) {
        let s = &self.st.set;
        if !s.night || !crate::doctor::which("wlsunset") {
            if self.st.night.on {
                self.st.night.stop();
                self.st.night.on = false;
                self.dirty(&SKind::Bar);
            }
            return;
        }
        let a = args(s);
        if self.st.night.running() && self.st.night.args == a {
            return;
        }
        self.st.night.stop();
        let mut cmd = Command::new("wlsunset");
        cmd.args(&a).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        self.st.night.child = crate::app::owned(&mut cmd).spawn().ok();
        self.st.night.on = self.st.night.child.is_some();
        self.st.night.args = a;
        self.dirty(&SKind::Bar);
    }

    pub fn night_toggle(&mut self) {
        if !crate::doctor::which("wlsunset") {
            self.toast_note("ночной свет", "нужен wlsunset: поставь его из своего дистрибутива");
            return;
        }
        self.st.set.night = !self.st.set.night;
        self.settings_changed();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn coords() {
        assert_eq!(super::geo("55.75, 37.62"), Some((55.75, 37.62)));
        assert_eq!(super::geo("55.75 37.62"), Some((55.75, 37.62)));
        assert_eq!(super::geo("95, 0"), None);
        assert_eq!(super::geo(""), None);
    }
}
