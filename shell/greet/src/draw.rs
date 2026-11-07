// Состояние экрана входа и его отрисовка. Лицо — общее с экраном блокировки шелла
// (yami_px::face): обои под вуалью, часы, дата, 闇. Под ним — пользователь, поле пароля, строка
// состояния; внизу слева — сеанс и раскладка, по центру — сон, перезагрузка, выключение,
// вверху слева — имя машины (что из этого видно — настройки, data::Opts). Появление — проявлением
// клетками из чёрного, уход — в чёрный; последний кадр ухода целиком чёрный: его и видно, пока
// greetd запускает сеанс.

use crate::data::{Opts, Session, User};
use yami_px::font::{self, PX};
use yami_px::gfx::{BoxStyle, Img, C, R};
use yami_px::look::Look;
use yami_px::ui::{Action, Cur, Field, Tone};
use std::time::Instant;

#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Block,
    Field,
    UserPrev,
    UserNext,
    Session,
    Layout,
    Sleep,
    Reboot,
    PowerOff,
}

impl Action for Act {
    fn block() -> Self {
        Act::Block
    }
}

pub type Ui<'a, 'b> = yami_px::ui::Ui<'a, 'b, Act>;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    /// пароль выбранного пользователя
    Pass,
    /// имя (пользователей не нашлось — вводят сами)
    Name,
    /// ждём greetd
    Busy,
    /// отвечаем на вопрос PAM (второй фактор, смена пароля)
    Prompt { secret: bool },
    /// сеанс запущен, экран уходит
    Done,
}

pub struct G {
    pub look: Look,
    pub opts: Opts,
    /// обои с впечённой вуалью (face::bake)
    pub wall: Option<Img>,
    pub veil: C,
    pub users: Vec<User>,
    /// выбранный; users.len() — «другой пользователь» (имя вводят)
    pub user: usize,
    /// имя, введённое руками (Phase::Name)
    pub typed: String,
    pub sessions: Vec<Session>,
    pub session: usize,
    /// коды раскладок из вида шелла (us, ru) или имена niri
    pub layouts: Vec<String>,
    pub layout: usize,
    pub caps: bool,
    pub field: Field,
    pub phase: Phase,
    /// вопрос PAM над полем
    pub prompt: String,
    pub status: String,
    /// строка состояния — ошибка (сообщение PAM об ошибке)
    pub bad: bool,
    pub fails: u32,
    pub shake: Option<Instant>,
    pub host: String,
    /// выключить и перезагрузить можно (logind или elogind)
    pub power: bool,
    pub blink: bool,
    /// появление; уход — с момента входа
    pub born: Instant,
    pub gone: Option<Instant>,
    pub demo: bool,
}

const IN_MS: f32 = 420.0;
const OUT_MS: f32 = 260.0;

impl G {
    pub fn user_name(&self) -> String {
        if self.name_mode() {
            return self.typed.clone();
        }
        self.users.get(self.user).map(|u| u.name.clone()).unwrap_or_else(|| self.typed.clone())
    }

    /// имя вводят руками: так настроено, списка нет или выбран «другой пользователь»
    pub fn name_mode(&self) -> bool {
        !self.opts.list || self.user >= self.users.len()
    }

    /// с чего начинать ввод (после ошибки, отмены, смены пользователя)
    pub fn first_phase(&self) -> Phase {
        if self.name_mode() {
            Phase::Name
        } else {
            Phase::Pass
        }
    }

    /// стрелки выбора: список включён и в нём кто-то есть (последняя позиция — «другой»)
    pub fn can_pick(&self) -> bool {
        self.opts.list && !self.users.is_empty()
    }

    /// новая картинка обоев: вуаль впекается сразу, кадр потом — копирование
    pub fn set_wall(&mut self, wall: Option<Img>) {
        self.wall = wall.map(|mut i| {
            yami_px::face::bake(&mut i, self.veil);
            i
        });
    }

    /// проявление: 0..1 при появлении, 1..0 при уходе
    pub fn reveal(&self) -> f32 {
        let d = |ms: f32| self.look.dur(ms);
        if let Some(g) = self.gone {
            let k = d(OUT_MS);
            return if k <= 0.0 { 0.0 } else { 1.0 - (g.elapsed().as_millis() as f32 / k).min(1.0).powi(2) };
        }
        let k = d(IN_MS);
        if k <= 0.0 {
            return 1.0;
        }
        let x = (self.born.elapsed().as_millis() as f32 / k).min(1.0);
        1.0 - (1.0 - x).powi(3)
    }

    /// уход доигран: можно выходить
    pub fn out_done(&self) -> bool {
        self.gone.is_some_and(|g| g.elapsed().as_millis() as f32 >= self.look.dur(OUT_MS))
    }
}

/// подпись кнопки раскладки: код («US»)
fn layout_label(g: &G) -> String {
    g.layouts.get(g.layout).map(|s| s.to_uppercase()).unwrap_or_default()
}

pub fn draw(ui: &mut Ui, g: &G, w: i32, h: i32, main: bool) {
    let t = ui.t;
    let k = g.reveal();
    if g.gone.is_some() && k <= 0.0 {
        // ушли: целиком чёрный кадр (его держит экран, пока запускается сеанс)
        ui.cv.clear(C(0xff000000));
        ui.anim = true;
        return;
    }
    yami_px::face::backdrop_with(ui, g.wall.as_ref(), w, h, g.veil, g.wall.is_some());
    let dy = if g.opts.clock { yami_px::face::clock(ui, w, h) - 30 } else { h / 2 - 90 };
    if g.opts.kanji {
        yami_px::face::kanji(ui, w, h);
    }
    ui.zone(R::new(0, 0, w, h), Act::Block, Cur::Default);
    if main {
        draw_login(ui, g, w, dy);
        // имя машины, внизу — сеанс, раскладка, питание
        if g.opts.host {
            ui.text(PX, 24, 20, &g.host.clone(), t.text_faint);
        }
        let by = h - 56;
        let mut x = 24;
        if !g.sessions.is_empty() {
            let name = g.sessions.get(g.session).map(|s| s.name.clone()).unwrap_or_default();
            let bw = ui.w(PX, &name) + 44;
            ui.button(R::new(x, by, bw, 32), &name, "grid", Tone::Plain, false, g.phase != Phase::Busy, Act::Session);
            x += bw + 8;
        }
        if g.layouts.len() > 1 {
            let l = layout_label(g);
            let bw = ui.w(PX, &l) + 44;
            ui.button(R::new(x, by, bw, 32), &l, "keys", Tone::Plain, false, true, Act::Layout);
        }
        if g.opts.power && g.power {
            let btns: [(&str, &str, Act); 3] = [("сон", "moon", Act::Sleep), ("перезагрузка", "reboot", Act::Reboot), ("выключить", "power", Act::PowerOff)];
            let ws: Vec<i32> = btns.iter().map(|b| ui.w(PX, b.0) + 44).collect();
            let total: i32 = ws.iter().sum::<i32>() + 8 * (ws.len() as i32 - 1);
            let mut x = (w - total) / 2;
            for (b, bw) in btns.into_iter().zip(ws) {
                ui.button(R::new(x, by, bw, 32), b.0, b.1, Tone::Ghost, false, true, b.2);
                x += bw + 8;
            }
        }
    }
    if k < 1.0 {
        ui.cv.dissolve(k, 2);
        ui.anim = true;
    }
}

fn draw_login(ui: &mut Ui, g: &G, w: i32, dy: i32) {
    let t = ui.t;
    let fw = 380;
    let mut fx = (w - fw) / 2;
    if let Some(s) = g.shake {
        let e = s.elapsed().as_millis() as i32;
        if e < 420 {
            fx += [8, -8, 6, -6, 4, -4, 0][(e / 60) as usize % 7];
            ui.anim = true;
        }
    }
    let fy = dy + 70;
    // кто входит: имя (и настоящее имя бледнее), стрелки — другой пользователь
    let name_phase = g.phase == Phase::Name;
    let asking_name = name_phase || (g.name_mode() && g.typed.is_empty());
    ui.cv.icon(if asking_name { "user" } else { "lock" }, fx, fy - 26, 2, t.red);
    if !asking_name {
        let u = if g.name_mode() { None } else { g.users.get(g.user) };
        let login = u.map(|u| u.name.clone()).unwrap_or_else(|| g.typed.clone());
        let nw = ui.cv.text_in(ui.f, PX, R::new(fx + 22, fy - 30, fw - 90, 20), &login, t.text);
        if let Some(real) = u.map(|u| u.real.clone()).filter(|r| !r.is_empty() && *r != login) {
            ui.cv.text_in(ui.f, PX, R::new(fx + 30 + nw, fy - 30, fw - 120 - nw, 20), &real, t.text_faint);
        }
    } else {
        let q = if g.can_pick() { "другой пользователь" } else { "кто входит?" };
        ui.text_in(PX, R::new(fx + 22, fy - 30, fw - 90, 20), q, t.text);
    }
    if g.can_pick() && matches!(g.phase, Phase::Pass | Phase::Name) {
        ui.ghost(R::new(fx + fw - 54, fy - 34, 26, 24), "left", Act::UserPrev);
        ui.ghost(R::new(fx + fw - 26, fy - 34, 26, 24), "right", Act::UserNext);
    }
    let fr = R::new(fx, fy, fw, 36);
    ui.cv.pxbox(R::new(fr.x - 6, fr.y - 6, fr.w + 12, fr.h + 12), &BoxStyle::new(t.glass_panel(), t.border).shadow(4));
    let (hint, secret) = match g.phase {
        Phase::Name => ("имя пользователя".to_string(), false),
        Phase::Prompt { secret } => (g.prompt.trim().trim_end_matches(':').to_lowercase(), secret),
        _ => ("пароль".to_string(), true),
    };
    let focused = g.phase != Phase::Busy && g.phase != Phase::Done;
    ui.field(fr, &g.field, &hint, focused, secret, Act::Field);
    // состояние: проверка, ошибка, caps lock, подсказка
    let (status, sc) = if !g.status.is_empty() {
        (g.status.clone(), if g.bad || g.fails > 0 { t.ember } else { t.text_dim })
    } else if g.phase == Phase::Busy {
        ("проверяю…".to_string(), t.text_dim)
    } else if g.caps {
        ("caps lock".to_string(), t.warn)
    } else if g.fails > 0 {
        (crate::tf!("неверно ×{}", "wrong ×{}", g.fails), t.ember)
    } else if g.demo {
        ("демо: пароль yami, esc — выйти".to_string(), t.text_faint)
    } else {
        ("enter — войти".to_string(), t.text_faint)
    };
    ui.text_center(font::PX, R::new(fx - 60, fy + 48, fw + 120, 20), &status, sc);
}
