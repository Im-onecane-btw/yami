// Заголовки окон в режиме «стол» (на любом niri): над каждым плавающим окном — полоса во всю
// ширину, как в DE: значок, название, свернуть / развернуть / закрыть. Тащишь — едет окно,
// двойной клик — развернуть, ПКМ — меню окна. Части, закрытые окнами выше, не рисуются и
// не ловят клики (порядок — по времени фокуса, активное сверху).

use crate::app::{App, SKind, State, BAR_H, TITLE_H};
use crate::font::PX;
use crate::gfx::{BoxStyle, R};
use crate::icons;
use crate::niri::Win;
use crate::theme::CLEAR;
use crate::ui::{Act, Cur, Tone, Ui};
use serde_json::json;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};
use smithay_client_toolkit::shell::WaylandSurface;

/// плавающие окна видимых столов, снизу вверх
fn stack(st: &State, out: &str) -> Vec<Win> {
    let Some(ws) = st.niri.active_ws(out) else { return vec![] };
    let mut v: Vec<Win> = st.niri.wins.iter().filter(|w| w.ws == Some(ws.id) && w.floating && !st.niri.is_min(w) && w.layout.pos.is_some()).cloned().collect();
    v.sort_by(|a, b| a.focused.cmp(&b.focused).then(a.ts.partial_cmp(&b.ts).unwrap_or(std::cmp::Ordering::Equal)));
    v
}

fn tile(w: &Win) -> R {
    let (x, y) = w.layout.pos.unwrap_or((0.0, 0.0));
    R::new(x.round() as i32, y.round() as i32, w.layout.size.0.round() as i32, w.layout.size.1.round() as i32)
}

/// куски полосы (в её координатах), не закрытые окнами выше
fn visible(bar: R, above: &[R]) -> Vec<R> {
    let mut parts = vec![bar];
    for a in above {
        let mut next = vec![];
        for p in parts {
            let i = p.inter(a);
            if i.empty() {
                next.push(p);
                continue;
            }
            // полоса низкая: делим только по горизонтали, по вертикали — сверху/снизу
            if i.x > p.x {
                next.push(R::new(p.x, p.y, i.x - p.x, p.h));
            }
            if i.right() < p.right() {
                next.push(R::new(i.right(), p.y, p.right() - i.right(), p.h));
            }
            if i.y > p.y {
                next.push(R::new(i.x, p.y, i.w, i.y - p.y));
            }
            if i.bottom() < p.bottom() {
                next.push(R::new(i.x, i.bottom(), i.w, p.bottom() - i.bottom()));
            }
        }
        parts = next.into_iter().filter(|r| !r.empty()).collect();
    }
    parts
}

pub fn draw(ui: &mut Ui, st: &State, id: u64, w: i32, h: i32) {
    let t = ui.t;
    let Some(win) = st.niri.win(id) else { return };
    let r = R::new(0, 0, w, h);
    let focused = win.focused;
    // пиксельная плашка с фаской; активное — акцентная кромка снизу
    ui.cv.pxbox(r, &BoxStyle::new(if focused { t.panel } else { t.bg.lerp(t.panel, 0.5) }, if focused { t.red } else { t.border }).bevel(true));
    if focused {
        ui.cv.fill(R::new(2, h - 3, w - 4, 1), t.ember.fade(0.7));
    }
    ui.zone(r, Act::TDrag(id), Cur::Default);
    let mut x = 6;
    // ручка: точки 2×3
    for i in 0..6 {
        ui.cv.fill(R::new(x + (i % 2) * 4, h / 2 - 5 + (i / 2) * 4, 2, 2), if focused { t.red } else { t.dim });
    }
    x += 14;
    let icon = icons::icon_for(&st.apps, &win.app_id);
    ui.app_icon(&icon, x, (h - 16) / 2, 16, if focused { 1.0 } else { 0.6 });
    x += 22;
    let bw = 24;
    let btns = 3 * (bw + 2);
    let title = if win.title.is_empty() { win.app_id.clone() } else { win.title.clone() };
    ui.text_in(PX, R::new(x, 0, w - x - btns - 10, h - 1), &title, if focused { t.text_hi } else { t.text_dim });
    let mut bx = w - btns - 2;
    for (icon, act, tone) in [("min", Act::TMin(id), Tone::Ghost), ("max", Act::TMax(id), Tone::Ghost), ("close", Act::TClose(id), Tone::Danger)] {
        let br = R::new(bx, 3, bw, h - 6);
        let hov = ui.hover(br);
        if tone == Tone::Danger && hov {
            ui.cv.pxbox(br, &BoxStyle::new(t.blood, t.ember).bevel(true));
            ui.cv.icon_in(icon, br, 2, t.text_hi);
            ui.zone(br, act, Cur::Pointer);
        } else {
            ui.button(br, "", icon, Tone::Ghost, false, true, act);
        }
        bx += bw + 2;
    }
    // закрытые чужими окнами места — прозрачные
    if let Some(s) = st.titles.get(&id) {
        for c in &s.covered {
            ui.cv.set(*c, CLEAR);
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TitleGeo {
    pub covered: Vec<R>,
}

impl App {
    pub fn sync_titles(&mut self) {
        let on = self.st.niri.stacking && self.st.set.titlebars && !self.st.locked && !self.st.niri.overview;
        let mut want: Vec<(u64, String, R, Vec<R>, Vec<R>)> = vec![];
        if on {
            for out in self.output_names() {
                let (ow, oh) = self.output_size(&out);
                let wins = stack(&self.st, &out);
                for (i, w) in wins.iter().enumerate() {
                    let tr = tile(w);
                    // во весь экран — без заголовка
                    if tr.w >= ow && tr.h >= oh - BAR_H {
                        continue;
                    }
                    let bar = R::new(tr.x, tr.y - TITLE_H, tr.w, TITLE_H);
                    if bar.bottom() <= 0 || bar.y >= oh {
                        continue;
                    }
                    let above: Vec<R> = wins[i + 1..].iter().flat_map(|a| {
                        let ar = tile(a);
                        // окно выше вместе со своим заголовком
                        [ar, R::new(ar.x, ar.y - TITLE_H, ar.w, TITLE_H)]
                    }).collect();
                    let vis = visible(bar, &above);
                    if vis.is_empty() {
                        continue;
                    }
                    let local = |r: &R| R::new(r.x - bar.x, r.y - bar.y, r.w, r.h);
                    let vis_l: Vec<R> = vis.iter().map(local).collect();
                    let cov: Vec<R> = above.iter().map(|a| local(&a.inter(&bar))).filter(|r| !r.empty()).collect();
                    want.push((w.id, out.clone(), bar, vis_l, cov));
                }
            }
        }
        // лишние — убрать сразу (без анимации: окно ушло — заголовок тоже)
        let have: Vec<(u32, u64)> = self.surfs.iter().filter_map(|(sid, s)| if let SKind::Title(id) = s.kind { Some((*sid, id)) } else { None }).collect();
        for (sid, id) in &have {
            if !want.iter().any(|w| w.0 == *id) {
                self.destroy(*sid);
                self.st.titles.remove(id);
            }
        }
        let dragging = self.press.as_ref().and_then(|p| if let Act::TDrag(id) = p.act { if p.moved { Some(id) } else { None } } else { None });
        for (id, out, bar, vis, cov) in want {
            self.st.titles.insert(id, TitleGeo { covered: cov });
            // пока тащим — положение ведёт шелл, ответы niri запаздывают
            if dragging == Some(id) {
                continue;
            }
            match have.iter().find(|h| h.1 == id).map(|h| h.0) {
                None => {
                    let sid = self.mk(SKind::Title(id), &out, Layer::Top, "yami-title", Anchor::TOP | Anchor::LEFT, (bar.w, bar.h), (bar.y, 0, 0, bar.x), -1, KeyboardInteractivity::None);
                    if let Some(s) = self.surfs.get_mut(&sid) {
                        s.margin = (bar.x, bar.y);
                        s.applied = (bar.x, bar.y);
                        s.input = Some(vis);
                    }
                }
                Some(sid) => {
                    self.move_surface(sid, bar.x, bar.y);
                    if let Some(s) = self.surfs.get_mut(&sid) {
                        if s.w != bar.w {
                            s.layer.set_size(bar.w as u32, bar.h as u32);
                        }
                        s.input = Some(vis);
                        s.layer.commit();
                        s.dirty = true;
                    }
                }
            }
        }
    }

    /// тащим заголовок: окно едет на ту же разницу (niri: смещение относительно текущего)
    pub fn title_drag(&mut self, win: u64, sid: u32, start: (i32, i32), dx: f64, dy: f64) {
        let (dx, dy) = (dx.round(), dy.round());
        let sent = self.tdrag.filter(|t| t.0 == win).map(|t| t.1).unwrap_or((0.0, 0.0));
        let (mx, my) = (dx - sent.0, dy - sent.1);
        if mx == 0.0 && my == 0.0 {
            return;
        }
        self.tdrag = Some((win, (dx, dy)));
        self.st.niri.act(json!({"MoveFloatingWindow": {"id": win, "x": {"AdjustFixed": mx}, "y": {"AdjustFixed": my}}}));
        // заголовок едет сразу, не дожидаясь niri
        let (nx, ny) = (start.0 + dx as i32, (start.1 + dy as i32).max(0));
        if let Some(s) = self.surfs.get_mut(&sid) {
            // пока тащим, ловим клики всей полосой (куски под чужими окнами — тоже)
            if s.input.is_some() {
                s.input = None;
                s.dirty = true;
            }
        }
        self.move_surface(sid, nx, ny);
        if let Some(g) = self.st.titles.get_mut(&win) {
            g.covered.clear();
        }
    }

    pub fn title_drag_end(&mut self, win: u64) {
        if self.tdrag.map(|t| t.0) == Some(win) {
            self.tdrag = None;
        }
        // заголовок не должен уехать под бар: окно — ниже
        let top = BAR_H + TITLE_H;
        if let Some(w) = self.st.niri.win(win) {
            if let Some((_, y)) = w.layout.pos {
                if (y as i32) < top {
                    self.st.niri.act(json!({"MoveFloatingWindow": {"id": win, "x": {"AdjustFixed": 0.0}, "y": {"AdjustFixed": (top - y as i32) as f64}}}));
                }
            }
        }
        self.sync_titles();
    }

    /// новое плавающее окно под самым баром — сдвинуть, чтобы заголовок поместился
    pub fn fit_title(&mut self, id: u64) {
        if !(self.st.niri.stacking && self.st.set.titlebars) {
            return;
        }
        let top = BAR_H + TITLE_H;
        if let Some(w) = self.st.niri.win(id) {
            if let Some((_, y)) = w.layout.pos {
                if (y as i32) < top && w.floating {
                    self.st.niri.act(json!({"MoveFloatingWindow": {"id": id, "x": {"AdjustFixed": 0.0}, "y": {"AdjustFixed": (top - y as i32) as f64}}}));
                }
            }
        }
    }
}
