// Всплывашки бара (ЛКМ по модулю): календарь + история уведомлений, звук (выход, микрофон,
// громкость программ, музыка с эквалайзером), сеть (график ↓↑, Wi-Fi), bluetooth, система.
// Клик мимо — закрыть (ловец кликов в app.rs), Esc — тоже.

use crate::app::{App, SKind, State};
use crate::font::{self, PX};
use crate::gfx::{BoxStyle, R};
use crate::theme::WHITE;
use crate::ui::{Act, Cur, Tone, Ui};
use crate::widgets;
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Keysym};

pub fn size(kind: &str, st: &State) -> (i32, i32) {
    match kind {
        "cal" => (660, 372),
        "sound" => {
            let apps = st.audio.streams.len().min(6) as i32;
            let sinks = if st.audio.sinks.len() > 1 { st.audio.sinks.len().min(5) as i32 * 26 + 30 } else { 0 };
            let media = if st.media.title.is_empty() { 0 } else { 170 };
            (420, 44 + 56 + sinks + 60 + if apps > 0 { 30 + apps * 46 } else { 0 } + media + 50)
        }
        "net" => {
            let wifi = if st.net.wifi_present { 40 + if st.net.wifi_on { st.net.nets.len().min(8) as i32 * 28 + if st.wifi_target.is_empty() { 0 } else { 44 } } else { 0 } } else { 0 };
            (420, 44 + 24 + st.net.active.len().max(1) as i32 * 22 + 130 + wifi + 50)
        }
        "bt" => (380, 44 + 40 + if st.bt.powered { st.bt.devices.len().clamp(1, 8) as i32 * 34 } else { 30 } + 50),
        "sys" => (440, 44 + 4 * 74 + 40 + 50),
        "profile" => (340, 44 + 36 * 3 + if st.prof.degraded.is_empty() { 0 } else { 26 } + 30),
        _ => (300, 200),
    }
}

pub fn draw(ui: &mut Ui, st: &State, w: i32, h: i32) {
    let Some(p) = &st.popup else { return };
    let fr = R::new(0, 0, w - 6, h - 6);
    ui.frame(fr, true);
    // «язычок» к модулю бара
    let t = ui.t;
    ui.cv.fill(R::new(2, 0, fr.w - 4, 2), t.red);
    let body = R::new(16, 12, fr.w - 32, fr.h - 24);
    match p.kind {
        "cal" => cal(ui, st, body),
        "sound" => sound(ui, st, body),
        "net" => net(ui, st, body),
        "bt" => bt(ui, st, body),
        "sys" => sys(ui, st, body),
        "profile" => profile(ui, st, body),
        _ => {}
    }
}

fn head(ui: &mut Ui, r: R, icon: &str, title: &str, sub: &str) -> i32 {
    let t = ui.t;
    ui.cv.icon(icon, r.x, r.y + 3, 2, t.red);
    let w = ui.text_in(PX, R::new(r.x + 22, r.y, r.w - 22, 20), title, t.text);
    if !sub.is_empty() {
        ui.text_in(PX, R::new(r.x + 30 + w, r.y, r.w - 40 - w, 20), sub, t.text_faint);
    }
    ui.cv.fill(R::new(r.x, r.y + 26, r.w, 1), t.border);
    r.y + 36
}

// ── календарь ───────────────────────────────────────────────────────────────
fn cal(ui: &mut Ui, st: &State, b: R) {
    let t = ui.t;
    let now = crate::app::local_now();
    let face = font::px(32);
    ui.text(face, b.x, b.y - 2, &format!("{:02}:{:02}", now.hour, now.min), t.text);
    let date = format!("{}, {} {}", crate::i18n::tr(crate::app::RU_DAY_FULL[now.wday as usize]), now.day, crate::i18n::tr(crate::app::RU_MON[(now.mon - 1) as usize]));
    ui.text_in(PX, R::new(b.x, b.y + 34, 300, 20), &date, t.text_dim);
    let cr = R::new(b.x, b.y + 66, 290, b.h - 66);
    widgets::calendar(ui, st, cr, st.cal_month, t.red, true);
    // уведомления
    let nx = b.x + 310;
    ui.cv.fill(R::new(nx - 10, b.y, 1, b.h), t.border);
    let nr = R::new(nx, b.y, b.right() - nx, b.h);
    let n = st.notifs.history.len();
    let sub = if n == 0 { String::new() } else { format!("{n}") };
    ui.cv.icon(if st.set.dnd { "bellx" } else { "bell" }, nr.x, nr.y + 3, 2, t.red);
    ui.text_in(PX, R::new(nr.x + 22, nr.y, 160, 20), &crate::tf!("уведомления {sub}", "notifications {sub}"), t.text);
    ui.button(R::new(nr.right() - 172, nr.y - 2, 104, 24), "не беспокоить", "", Tone::Ghost, st.set.dnd, true, Act::Dnd);
    ui.button(R::new(nr.right() - 64, nr.y - 2, 64, 24), "очистить", "", Tone::Ghost, false, n > 0, Act::NotifClear);
    ui.cv.fill(R::new(nr.x, nr.y + 26, nr.w, 1), t.border);
    let mut y = nr.y + 34;
    if n == 0 {
        ui.text_center(PX, R::new(nr.x, y + 60, nr.w, 20), "тихо. ничего нового", t.text_faint);
        ui.cv.icon_in("bell", R::new(nr.x, y + 20, nr.w, 30), 3, t.border);
        return;
    }
    ui.cv.push_clip(R::new(nr.x, y, nr.w, nr.bottom() - y));
    for note in &st.notifs.history {
        if y > nr.bottom() {
            break;
        }
        let ch = if note.body.is_empty() { 52 } else { 70 };
        let r = R::new(nr.x, y, nr.w, ch);
        let hov = ui.hover(r);
        ui.cv.pxbox(r, &BoxStyle::new(if hov { t.surface_hi } else { t.surface }, if note.critical { t.ember } else { t.border }));
        ui.zone(r, Act::NotifClick(note.id), Cur::Pointer);
        let mut x = r.x + 8;
        if !note.icon.is_empty() {
            let icon = note.icon.clone();
            ui.app_icon(&icon, x, r.y + 6, 16, 1.0);
            x += 22;
        }
        let app = if note.app.is_empty() { "система".to_string() } else { note.app.to_lowercase() };
        let aw = ui.text_in(PX, R::new(x, r.y + 4, r.w / 2, 18), &app, t.red);
        ui.text_in(PX, R::new(x + aw + 8, r.y + 4, 60, 18), &note.time, t.text_faint);
        ui.ghost(R::new(r.right() - 24, r.y + 4, 20, 18), "close", Act::NotifDel(note.id));
        ui.text_in(PX, R::new(r.x + 8, r.y + 26, r.w - 16, 18), &note.summary, t.text);
        if !note.body.is_empty() {
            ui.text_in(PX, R::new(r.x + 8, r.y + 46, r.w - 16, 18), &note.body.replace('\n', " "), t.text_dim);
        }
        y += ch + 6;
    }
    ui.cv.pop_clip();
}

// ── звук ────────────────────────────────────────────────────────────────────
fn sound(ui: &mut Ui, st: &State, b: R) {
    let t = ui.t;
    let a = &st.audio;
    let sink = a.sinks.iter().find(|s| s.name == a.sink).map(|s| crate::audio::label(&st.set, s)).unwrap_or_default();
    let mut y = head(ui, b, "speaker", "звук", &sink);
    // выход
    ui.button(R::new(b.x, y + 18, 26, 22), "", if a.muted { "mute" } else { "speaker" }, Tone::Ghost, a.muted, true, Act::VolMute);
    let v = if a.muted { "тишина".to_string() } else { format!("{:.0}%", a.volume * 100.0) };
    ui.slider(R::new(b.x + 34, y, b.w - 34, 40), "громкость", a.volume.min(1.0) as f32, &v, 20, Act::VolSlider);
    y += 50;
    if a.sinks.len() > 1 {
        ui.text_in(PX, R::new(b.x, y, b.w, 20), "выход", t.text_dim);
        y += 24;
        for s in a.sinks.iter().take(5) {
            let on = s.name == a.sink;
            let r = R::new(b.x, y, b.w, 24);
            ui.button(r, "", "", Tone::Ghost, on, true, Act::Sink(s.name.clone()));
            ui.cv.fill(R::new(r.x + 8, r.cy() - 3, 6, 6), if on { t.ember } else { t.border });
            ui.text_in(PX, R::new(r.x + 22, r.y, r.w - 26, r.h), &crate::audio::label(&st.set, s), if on { WHITE } else { t.text });
            y += 26;
        }
        y += 6;
    }
    // микрофон
    ui.button(R::new(b.x, y + 18, 26, 22), "", if a.mic_muted { "mute" } else { "mic" }, Tone::Ghost, a.mic_muted, true, Act::MicMute);
    let mv = if a.mic_muted { "выкл".to_string() } else { format!("{:.0}%", a.mic * 100.0) };
    ui.slider(R::new(b.x + 34, y, b.w - 34, 40), "микрофон", a.mic.min(1.0) as f32, &mv, 20, Act::MicSlider);
    y += 54;
    // программы
    if !a.streams.is_empty() {
        ui.text_in(PX, R::new(b.x, y, b.w, 20), "программы", t.text_dim);
        y += 26;
        for s in a.streams.iter().take(6) {
            let icon = crate::icons::icon_for(&st.apps, &s.icon.to_lowercase());
            ui.app_icon(&icon, b.x + 4, y + 20, 16, if s.muted { 0.4 } else { 1.0 });
            let label = if s.title.is_empty() || s.title == s.app { s.app.clone() } else { format!("{} — {}", s.app, s.title) };
            let v = if s.muted { "тихо".to_string() } else { format!("{:.0}%", s.vol * 100.0) };
            ui.slider(R::new(b.x + 34, y, b.w - 34 - 30, 40), &label, s.vol.min(1.0) as f32, &v, 20, Act::AppVol(s.idx));
            ui.ghost(R::new(b.right() - 24, y + 18, 24, 22), if s.muted { "mute" } else { "speaker" }, Act::AppMute(s.idx));
            y += 46;
        }
    }
    // музыка
    if !st.media.title.is_empty() {
        ui.cv.fill(R::new(b.x, y, b.w, 1), t.border);
        y += 10;
        widgets::media_card(ui, st, R::new(b.x, y, b.w, 150), "bars", t.red, true);
        y += 160;
    }
    let _ = y;
    ui.button(R::new(b.x, b.bottom() - 28, b.w, 28), "микшер pavucontrol", "speaker", Tone::Plain, false, true, Act::Pavu);
}

// ── сеть ────────────────────────────────────────────────────────────────────
fn net(ui: &mut Ui, st: &State, b: R) {
    let t = ui.t;
    let s = &st.sys;
    let sub = if s.online { s.ip.clone() } else { "нет сети".to_string() };
    let mut y = head(ui, b, "net", "сеть", &sub);
    if st.net.active.is_empty() {
        ui.text_in(PX, R::new(b.x, y, b.w, 20), if st.net.loaded { "не подключено" } else { "смотрю…" }, t.text_faint);
        y += 22;
    }
    for (name, kind, dev) in &st.net.active {
        ui.cv.fill(R::new(b.x, y + 7, 6, 6), t.ok);
        ui.text_in(PX, R::new(b.x + 14, y, b.w - 120, 20), name, t.text);
        ui.text_right(PX, R::new(b.x, y, b.w, 20), &format!("{kind} · {dev}"), t.text_faint);
        y += 22;
    }
    y += 6;
    // ↓↑ график
    let full = s.h_rx.max().max(s.h_tx.max()).max(64.0 * 1024.0);
    let rx: Vec<f32> = s.h_rx.tail(120).iter().map(|v| v / full).collect();
    let tx: Vec<f32> = s.h_tx.tail(120).iter().map(|v| v / full).collect();
    let w1 = ui.text_in(PX, R::new(b.x, y, 160, 20), &crate::tf!("↓ {}/с", "↓ {}/s", crate::sys::rate(s.rx)), t.red);
    ui.text_in(PX, R::new(b.x + w1 + 16, y, 160, 20), &crate::tf!("↑ {}/с", "↑ {}/s", crate::sys::rate(s.tx)), t.text_dim);
    ui.text_right(PX, R::new(b.x, y, b.w, 20), &crate::tf!("пик {}/с", "peak {}/s", crate::sys::rate(full as f64)), t.text_faint);
    y += 24;
    let gr = R::new(b.x, y, b.w, 90);
    ui.cv.pxbox(gr, &BoxStyle::new(t.code_bg, t.border));
    widgets::graph(ui, gr.inset(6), &rx, "line", "scroll", st.sys_phase(), t.red, Some((&tx, t.text_dim)));
    y += 100;
    // Wi-Fi
    if st.net.wifi_present {
        ui.toggle(R::new(b.x, y, 140, 26), "wi-fi", "", st.net.wifi_on, Act::Wifi(!st.net.wifi_on));
        if st.net.wifi_on {
            ui.button(R::new(b.right() - 110, y, 110, 24), "искать", "search", Tone::Ghost, false, !st.net.busy, Act::WifiRescan);
        }
        if !st.net.msg.is_empty() {
            ui.text_in(PX, R::new(b.x + 150, y, b.w - 270, 24), &st.net.msg, t.warn);
        }
        y += 34;
        if st.net.wifi_on {
            for n in st.net.nets.iter().take(8) {
                let r = R::new(b.x, y, b.w, 26);
                ui.button(r, "", "", Tone::Ghost, n.active, true, Act::WifiNet(n.ssid.clone()));
                // сила сигнала — 4 столбика
                for k in 0..4 {
                    let on = n.signal as i32 > k * 25;
                    ui.cv.fill(R::new(r.x + 6 + k * 4, r.y + 18 - k * 3, 3, 3 + k * 3), if on { if n.active { t.ember } else { t.red } } else { t.border });
                }
                ui.text_in(PX, R::new(r.x + 30, r.y, r.w - 90, r.h), &n.ssid, if n.active { WHITE } else { t.text });
                if n.secure {
                    ui.cv.icon("lock", r.right() - 40, r.cy() - 7, 2, if n.known { t.text_dim } else { t.text_faint });
                }
                ui.text_right(PX, R::new(r.x, r.y, r.w - 4, r.h), &format!("{}", n.signal), t.text_faint);
                y += 28;
                if st.wifi_target == n.ssid {
                    let focused = st.field_focus == Some("wifi");
                    ui.field(R::new(b.x + 30, y, b.w - 130, 30), &st.wifi_pass, "пароль", focused, true, Act::SField("wifi"));
                    ui.button(R::new(b.right() - 92, y + 2, 92, 26), "войти", "check", Tone::Primary, false, true, Act::WifiNet(n.ssid.clone()));
                    y += 44;
                }
            }
        }
    }
    ui.button(R::new(b.x, b.bottom() - 28, b.w, 28), "все подключения", "gear", Tone::Plain, false, true, Act::NetEditor);
}

// ── bluetooth ───────────────────────────────────────────────────────────────
fn bt(ui: &mut Ui, st: &State, b: R) {
    let t = ui.t;
    let s = &st.bt;
    let conn = s.devices.iter().filter(|d| d.connected).count();
    let sub = if !s.present { "нет адаптера".to_string() } else if conn > 0 { crate::tf!("подключено {conn}", "{conn} connected") } else { String::new() };
    let mut y = head(ui, b, "bt", "bluetooth", &sub);
    if !s.present {
        ui.text_in(PX, R::new(b.x, y, b.w, 20), "адаптер не найден (bluez не запущен?)", t.text_faint);
        return;
    }
    ui.toggle(R::new(b.x, y, 160, 26), "включён", "", s.powered, Act::BtPower(!s.powered));
    if s.powered {
        ui.button(R::new(b.right() - 110, y, 110, 24), if s.discovering { "ищу…" } else { "искать" }, "search", Tone::Ghost, s.discovering, !s.discovering, Act::BtScan);
    }
    y += 36;
    if !s.powered {
        ui.text_in(PX, R::new(b.x, y, b.w, 20), "выключен", t.text_faint);
    } else if s.devices.is_empty() {
        ui.text_in(PX, R::new(b.x, y, b.w, 20), "устройств нет — нажми «искать»", t.text_faint);
    }
    if s.powered {
        for d in s.devices.iter().take(8) {
            let r = R::new(b.x, y, b.w, 30);
            ui.button(r, "", "", Tone::Ghost, d.connected, true, Act::BtDev(d.path.clone()));
            let icon = match d.icon.as_str() {
                i if i.contains("audio") || i.contains("headset") => "music",
                i if i.contains("input") || i.contains("keyboard") || i.contains("mouse") => "grid",
                i if i.contains("phone") => "bell",
                _ => "bt",
            };
            ui.cv.icon(icon, r.x + 8, r.cy() - 7, 2, if d.connected { t.ember } else { t.text_dim });
            ui.text_in(PX, R::new(r.x + 30, r.y, r.w - 130, r.h), &d.name, if d.connected { WHITE } else { t.text });
            let st_txt = if d.connected { "подключено".to_string() } else if d.paired { "сопряжено".to_string() } else { "новое".to_string() };
            let st_txt = match d.battery {
                Some(p) => format!("{p}% · {st_txt}"),
                None => st_txt,
            };
            ui.text_right(PX, R::new(r.x, r.y, r.w - 8, r.h), &st_txt, t.text_faint);
            y += 34;
        }
    }
    ui.button(R::new(b.x, b.bottom() - 28, b.w, 28), "менеджер устройств", "gear", Tone::Plain, false, true, Act::SysMon);
}

// ── профиль питания ─────────────────────────────────────────────────────────
fn profile(ui: &mut Ui, st: &State, b: R) {
    let t = ui.t;
    let p = &st.prof;
    let mut y = head(ui, b, p.icon(), "профиль питания", if p.present { crate::profile::label(&p.active) } else { "" });
    if !p.present {
        ui.wrap(PX, R::new(b.x, y, b.w, 60), "power-profiles-daemon не запущен: установи его и включи (systemctl enable --now power-profiles-daemon)", t.text_faint, 3);
        return;
    }
    for (k, icon, text) in crate::profile::KINDS {
        let have = p.list.iter().any(|l| l == k);
        let r = R::new(b.x, y, b.w, 30);
        let on = p.active == *k;
        ui.button(r, text, icon, Tone::Ghost, on, have, Act::Prof(k.to_string()));
        if on {
            ui.cv.icon("check", r.right() - 22, r.cy() - 7, 2, t.ember);
        }
        y += 36;
    }
    if !p.degraded.is_empty() {
        ui.text_in(PX, R::new(b.x, y, b.w, 20), &crate::tf!("урезано: {}", "limited: {}", p.degraded), t.ember);
    }
}

// ── система ─────────────────────────────────────────────────────────────────
fn sys(ui: &mut Ui, st: &State, b: R) {
    let t = ui.t;
    let s = &st.sys;
    let up = std::fs::read_to_string("/proc/uptime").ok().and_then(|x| x.split_whitespace().next().and_then(|v| v.parse::<f64>().ok())).unwrap_or(0.0) as u64;
    let mut y = head(ui, b, "cpu", "система", &crate::tf!("работает {} ч {} мин", "up {} h {} min", up / 3600, up / 60 % 60));
    let rows: [(&str, &str, String, &str); 4] = [
        ("cpu", "процессор", format!("{:.0}%", s.cpu * 100.0), "cpu"),
        ("ram", "память", crate::tf!("{:.1} / {:.0} ГБ", "{:.1} / {:.0} GB", s.mem_used, s.mem_total), "ram"),
        ("temp", "температура", format!("{:.0}°C", s.temp), "temp"),
        ("temp", "видеокарта", if s.gpu_temp > 0.0 { format!("{:.0}°C  {:.0}%", s.gpu_temp, s.gpu_load * 100.0) } else { "—".into() }, "gpu"),
    ];
    for (icon, label, val, src) in rows {
        ui.cv.icon(icon, b.x, y + 3, 2, t.red);
        ui.text_in(PX, R::new(b.x + 22, y, 200, 20), label, t.text_dim);
        ui.text_right(PX, R::new(b.x, y, b.w, 20), &val, t.text);
        let (ser, full) = s.series(src);
        let data: Vec<f32> = ser.tail(140).iter().map(|v| v / full).collect();
        let gr = R::new(b.x, y + 24, b.w, 42);
        widgets::graph(ui, gr, &data, "bars", "scroll", st.sys_phase(), if src == "temp" && s.temp > 80.0 { t.ember } else { t.red }, None);
        y += 74;
    }
    ui.text_in(PX, R::new(b.x, y, 140, 20), "диск /", t.text_dim);
    ui.cv.gauge(R::new(b.x + 90, y + 4, b.w - 150, 12), 24, s.disk, t.red, t.surface.lerp(t.border, 0.3), 2);
    ui.text_right(PX, R::new(b.x, y, b.w, 20), &format!("{:.0}%", s.disk * 100.0), t.text);
    ui.button(R::new(b.x, b.bottom() - 28, b.w, 28), "процессы", "cpu", Tone::Plain, false, true, Act::SysMon);
}

impl App {
    pub fn popup_key(&mut self, e: &KeyEvent) {
        if self.st.field_focus == Some("wifi") {
            match e.keysym {
                Keysym::Escape => {
                    self.st.field_focus = None;
                    self.st.wifi_target.clear();
                    self.resize_popup();
                }
                Keysym::Return | Keysym::KP_Enter => {
                    let ssid = self.st.wifi_target.clone();
                    self.wifi_connect(&ssid);
                }
                _ => {
                    let mods = self.mods;
                    App::edit_field(&mut self.st.wifi_pass, e, &mods);
                }
            }
            self.dirty(&SKind::Popup);
            return;
        }
        if e.keysym == Keysym::Escape {
            self.close_popup();
        }
    }

    pub fn wifi_connect(&mut self, ssid: &str) {
        let n = self.st.net.nets.iter().find(|n| n.ssid == ssid).cloned();
        let Some(n) = n else { return };
        if n.active {
            crate::net::wifi_cmd(vec!["connection".into(), "down".into(), "id".into(), ssid.into()], self.tx.clone(), self.st.net.clone());
            return;
        }
        if n.known {
            crate::net::wifi_cmd(vec!["connection".into(), "up".into(), "id".into(), ssid.into()], self.tx.clone(), self.st.net.clone());
            return;
        }
        if n.secure && (self.st.wifi_target != ssid || self.st.wifi_pass.text.is_empty()) {
            // сначала — пароль
            self.st.wifi_target = ssid.to_string();
            self.st.wifi_pass.clear();
            self.st.field_focus = Some("wifi");
            self.resize_popup();
            self.dirty(&SKind::Popup);
            return;
        }
        let mut args = vec!["device".into(), "wifi".into(), "connect".into(), ssid.to_string()];
        if n.secure {
            args.push("password".into());
            args.push(self.st.wifi_pass.text.clone());
        }
        self.st.wifi_target.clear();
        self.st.wifi_pass.clear();
        self.st.field_focus = None;
        crate::net::wifi_cmd(args, self.tx.clone(), self.st.net.clone());
        self.resize_popup();
    }
}
