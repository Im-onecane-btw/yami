// Экран блокировки (ext-session-lock): обои под тёмной вуалью, большие часы, дата, поле
// пароля. Ошибка — поле вздрагивает ступенями. Остальные
// мониторы — только часы.

use crate::app::{local_now, App, State, RU_DAY_FULL, RU_MON};
use crate::font::{self, PX};
use crate::gfx::{BoxStyle, C, R};
use crate::ui::{Act, Cur, Ui};
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Keysym};
use std::time::Instant;

pub fn draw(ui: &mut Ui, st: &State, w: i32, h: i32, main: bool) {
    let t = ui.t;
    ui.cv.clear(t.bg);
    if let Some(img) = &st.wall {
        crate::wall::cover(ui, img, w, h);
    }
    ui.cv.fill(R::new(0, 0, w, h), C(0xff000000).alpha(0.55));
    // строки развёртки — через одну, еле видно
    for y in (0..h).step_by(4) {
        ui.cv.fill(R::new(0, y, w, 1), C(0xff000000).alpha(0.18));
    }
    let tm = local_now();
    let big = font::px(128);
    let clock = format!("{:02}:{:02}", tm.hour, tm.min);
    let cw = ui.w(big, &clock);
    let cy = h / 2 - 150;
    // тень-сдвиг на 4 px, как у панелей
    ui.text(big, (w - cw) / 2 + 4, cy + 4, &clock, t.blood.fade(0.8));
    ui.text(big, (w - cw) / 2, cy, &clock, t.text);
    let date = format!("{}, {} {}", crate::i18n::tr(RU_DAY_FULL[tm.wday as usize]), tm.day, crate::i18n::tr(RU_MON[(tm.mon - 1) as usize]));
    let dy = cy + ui.f.line(big) + 6;
    ui.text_center(font::PX24, R::new(0, dy, w, 30), &date, t.text_dim);
    ui.text(font::jp(32), w - 72, h - 64, "闇", t.red.fade(0.5));
    if !main {
        return;
    }
    // поле пароля
    let fw = 380;
    let mut fx = (w - fw) / 2;
    if let Some(s) = st.lock_shake {
        let e = s.elapsed().as_millis() as i32;
        if e < 420 {
            fx += [8, -8, 6, -6, 4, -4, 0][(e / 60) as usize % 7];
            ui.anim = true;
        }
    }
    let fy = dy + 70;
    let user = std::env::var("USER").unwrap_or_default();
    ui.cv.icon("lock", fx, fy - 26, 2, t.red);
    ui.text_in(PX, R::new(fx + 22, fy - 30, fw - 22, 20), &user, t.text);
    let fr = R::new(fx, fy, fw, 36);
    ui.cv.pxbox(R::new(fr.x - 6, fr.y - 6, fr.w + 12, fr.h + 12), &BoxStyle::new(t.glass_panel(), t.border).shadow(4));
    ui.field(fr, &st.lock_field, "пароль", true, true, Act::LockField);
    let status = if !st.lock_status.is_empty() {
        st.lock_status.clone()
    } else if st.lock_fails > 0 {
        crate::tf!("неверно ×{}", "wrong ×{}", st.lock_fails)
    } else {
        "enter — войти".to_string()
    };
    let sc = if st.lock_fails > 0 && st.lock_status.is_empty() { t.ember } else { t.text_faint };
    ui.text_center(PX, R::new(fx, fy + 48, fw, 20), &status, sc);
    ui.zone(R::new(0, 0, w, h), Act::Block, Cur::Default);
    ui.zone(fr, Act::LockField, Cur::Text);
}

impl App {
    pub fn lock_key(&mut self, e: &KeyEvent) {
        if self.st.lock_status == "проверяю…" {
            return;
        }
        match e.keysym {
            Keysym::Return | Keysym::KP_Enter => {
                let mut pass = self.st.lock_field.text.clone();
                if pass.is_empty() {
                    return;
                }
                self.st.lock_status = "проверяю…".into();
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let ok = crate::pam::check(&pass);
                    yami_px::ui::wipe(&mut pass);
                    let _ = tx.send(crate::app::Msg::Pam(ok));
                });
            }
            Keysym::Escape => self.st.lock_field.clear(),
            _ => {
                let mods = self.mods;
                App::edit_field(&mut self.st.lock_field, e, &mods);
            }
        }
        for l in self.lock_surfs.iter_mut() {
            l.dirty = true;
        }
    }

    pub fn on_pam(&mut self, ok: bool) {
        self.st.lock_status.clear();
        if ok {
            self.st.lock_fails = 0;
            self.unlock();
            self.sync_widgets();
            self.sync_titles();
            self.sync_dock();
            self.auth_show();
        } else {
            self.st.lock_fails += 1;
            self.st.lock_field.wipe();
            self.st.lock_shake = Some(Instant::now());
            for l in self.lock_surfs.iter_mut() {
                l.dirty = true;
            }
        }
    }
}
