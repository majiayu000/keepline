//! 标记完成 from a pane's header. The animation is the one an `effects.json` plugin picks
//! (`keepline_core::effect`), the built-in seal otherwise: every frame is a pure function
//! of the run's progress `t`, drawn partly in the pane (button, veil, words, lids) and
//! partly on a window-wide overlay so it can leave the pane and land in the sidebar row.
//! 「留着这个画面」 keeps the pane open; the session stays completed either way.
//! Prototype of every style: specs/complete-animation/directions.html.
use super::layout::PaneAction;
use super::*;
use gpui::{Bounds, Hsla, PathBuilder, Pixels, Point, point, size};
use keepline_core::effect::{Complete, Style};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

/// The most specks the dissolve sends; a fuller screen sends every n-th cell.
const MAX_SPECKS: usize = 900;

/// One pane's run.
pub(super) struct Run {
    /// So a late timer from an earlier, kept run does not close the pane.
    seq: u64,
    start: Instant,
    effect: Complete,
    session: String,
    /// Window points of the cells with text when it started, and their colors.
    glyphs: Vec<(Point<Pixels>, Hsla, u16)>,
    cell: gpui::Size<Pixels>,
    /// Held at one point of the run for screenshots.
    pinned: Option<f32>,
}
impl Run {
    fn t(&self) -> f32 {
        self.pinned.unwrap_or_else(|| {
            (self.start.elapsed().as_secs_f32() / self.effect.duration.as_secs_f32()).min(1.)
        })
    }
}

/// Where the panes and sidebar rows were last drawn, recorded while painting so the
/// overlay, painted after them in the same frame, knows where to go.
#[derive(Default)]
pub(super) struct Spots {
    pub(super) panes: HashMap<String, Bounds<Pixels>>,
    pub(super) rows: HashMap<String, Bounds<Pixels>>,
}
pub(super) type SharedSpots = Rc<RefCell<Spots>>;

/// A canvas filling its parent that records the parent's bounds under `key`. An absolute
/// child here starts at the parent's content box whatever its insets say, though it takes
/// the padded size; `padding` moves it back onto the parent's edge.
pub(super) fn spot(
    spots: &SharedSpots,
    key: String,
    padding: (f32, f32),
    pick: fn(&mut Spots) -> &mut HashMap<String, Bounds<Pixels>>,
) -> impl IntoElement {
    let spots = spots.clone();
    gpui::canvas(
        move |bounds, _, _| {
            let edge = point(
                bounds.origin.x - px(padding.0),
                bounds.origin.y - px(padding.1),
            );
            pick(&mut spots.borrow_mut()).insert(key, Bounds::new(edge, bounds.size));
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}

/// `t`'s progress through `from..to` of the whole run, clamped to 0..=1.
fn sl(t: f32, from: f32, to: f32) -> f32 {
    ((t - from) / (to - from)).clamp(0., 1.)
}
fn eo(x: f32) -> f32 {
    1. - (1. - x).powi(4)
}
fn eio(x: f32) -> f32 {
    if x < 0.5 {
        4. * x * x * x
    } else {
        1. - (-2. * x + 2.).powi(3) / 2.
    }
}
/// Overshoots a little before settling, like a stamp pressed down.
fn back_out(x: f32) -> f32 {
    let (c1, c3) = (1.70158, 2.70158);
    1. + c3 * (x - 1.).powi(3) + c1 * (x - 1.).powi(2)
}
fn lerp(a: f32, b: f32, k: f32) -> f32 {
    a + (b - a) * k
}
fn bez(a: Point<Pixels>, c: Point<Pixels>, b: Point<Pixels>, k: f32) -> Point<Pixels> {
    let m = 1. - k;
    point(
        a.x * (m * m) + c.x * (2. * m * k) + b.x * (k * k),
        a.y * (m * m) + c.y * (2. * m * k) + b.y * (k * k),
    )
}
/// A small deterministic random sequence, so the same `t` is always the same frame.
fn rng(mut seed: u32) -> impl FnMut() -> f32 {
    move || {
        seed = seed.wrapping_add(0x6D2B_79F5);
        let mut t = (seed ^ (seed >> 15)).wrapping_mul(1 | seed);
        t = (t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t))) ^ t;
        ((t ^ (t >> 14)) as f32) / 4_294_967_296.
    }
}

/// Per style, the slices of the run each part plays in.
struct Timing {
    /// Where the button has filled and turned into ✓ 已完成.
    button: f32,
    /// The words in, and out (0 when they stay).
    words: (f32, f32, f32, f32),
    /// Where the pane starts and finishes fading out.
    fade: (f32, f32),
    /// Where the finished session reaches its sidebar row.
    land: f32,
}
fn timing(style: Style) -> Timing {
    match style {
        Style::Seal => Timing {
            button: 0.25,
            words: (0.3, 0.5, 0., 0.),
            fade: (0.85, 1.),
            land: 1.,
        },
        Style::Stamp => Timing {
            button: 0.14,
            words: (0.36, 0.52, 0.7, 0.78),
            fade: (0.74, 0.86),
            land: 0.88,
        },
        Style::Sweep => Timing {
            button: 0.12,
            words: (0.52, 0.64, 0.62, 0.7),
            fade: (0.88, 0.98),
            land: 0.9,
        },
        Style::Dissolve => Timing {
            button: 0.12,
            words: (0.3, 0.42, 0.66, 0.74),
            fade: (0.76, 0.9),
            land: 0.88,
        },
        Style::Fade => Timing {
            button: 0.12,
            words: (2., 2., 0., 0.),
            fade: (0.15, 1.),
            land: 2.,
        },
    }
}
/// Where the sweep's light is across the body, as a fraction of its width.
fn sweep_at(t: f32) -> f32 {
    eio(sl(t, 0.05, 0.4)) * 1.3 - 0.15
}

impl Workbench {
    /// Marks the pane's session completed and closes the pane once the animation ends,
    /// unless the user keeps it or the host turned the mark down.
    pub(super) fn complete_pane(
        &mut self,
        terminal: String,
        session: String,
        cx: &mut Context<Self>,
    ) {
        self.complete_pane_with(terminal, session, keepline_core::effect::complete(), cx);
    }
    pub(super) fn complete_pane_with(
        &mut self,
        terminal: String,
        session: String,
        effect: Complete,
        cx: &mut Context<Self>,
    ) {
        self.layout.completing_seq += 1;
        let seq = self.layout.completing_seq;
        let (glyphs, cell) = match (effect.style, self.layout.panes.get(&terminal)) {
            (Style::Dissolve, Some(pane)) => pane.read(cx).glyph_cells(MAX_SPECKS),
            _ => (vec![], size(px(0.), px(0.))),
        };
        let glyphs = glyphs
            .into_iter()
            .map(|(p, c, w)| (p, T::color(c), w))
            .collect();
        self.layout.completing.insert(
            terminal.clone(),
            Run {
                seq,
                start: Instant::now(),
                effect,
                session: session.clone(),
                glyphs,
                cell,
                pinned: None,
            },
        );
        self.edit(
            Edit::SetOutcome {
                id: session.clone(),
                outcome: Outcome::Completed,
            },
            cx,
        );
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(effect.duration).await;
            let _ = this.update(cx, |this, cx| {
                match this.layout.completing.get(&terminal) {
                    Some(run) if run.seq == seq && run.pinned.is_none() => {}
                    _ => return,
                }
                this.layout.completing.remove(&terminal);
                let done = this
                    .workspace
                    .agent_sessions
                    .iter()
                    .any(|s| s.id == session && s.outcome == Outcome::Completed);
                if done {
                    this.close_pane(&terminal, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
    /// The run's style and progress on this pane, if one is playing.
    pub(super) fn completing(&self, terminal: &str) -> Option<(Style, f32)> {
        self.layout
            .completing
            .get(terminal)
            .map(|r| (r.effect.style, r.t()))
    }
    /// Whether a run is landing `session` in the sidebar, so its row records its spot.
    pub(super) fn marking(&self, session: &str) -> bool {
        self.layout
            .completing
            .values()
            .any(|r| r.effect.land && r.session == session)
    }
    /// Holds a run at `t` for screenshots, or lets it go on.
    #[cfg(feature = "visual-test")]
    pub(super) fn pin_completing(&mut self, terminal: &str, t: Option<f32>) {
        if let Some(run) = self.layout.completing.get_mut(terminal) {
            run.pinned = t;
        }
    }
    /// The whole pane's opacity at `t`.
    pub(super) fn completing_opacity(style: Style, t: f32) -> f32 {
        let (a, b) = timing(style).fade;
        match style {
            Style::Seal | Style::Sweep => 1. - sl(t, a, b),
            _ => 1. - eo(sl(t, a, b)),
        }
    }
    /// The header button mid-run: a fill sweeps in, the words turn to a check, a ring
    /// spreads out and fades.
    pub(super) fn completing_button(&self, id: &str, style: Style, t: f32) -> gpui::AnyElement {
        let (dot, _, _) = Group::Done.colors();
        let end = timing(style).button;
        let fill = eo(sl(t, 0., end));
        let turned = sl(t, end * 0.45, end);
        let ripple = eo(sl(t, end * 0.6, end * 3.));
        let out = px(-10. * ripple);
        div()
            .id(gpui::SharedString::from(format!("accept-pane-{id}")))
            .relative()
            .flex_shrink_0()
            .h(px(T::PANE_ACTION))
            .px(px(T::S6))
            .flex()
            .items_center()
            .rounded(px(T::PANE_ACTION_RADIUS))
            .bg(T::color(T::button()))
            .text_size(px(T::SMALL))
            .when(ripple > 0. && ripple < 1., |d| {
                d.child(
                    div()
                        .absolute()
                        .top(out)
                        .bottom(out)
                        .left(out)
                        .right(out)
                        .rounded(px(T::PANE_ACTION_RADIUS + 10. * ripple))
                        .border(px(T::FOCUS_RING))
                        .border_color(dot.opacity((1. - ripple) * 0.8)),
                )
            })
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(T::PANE_ACTION_RADIUS))
                    .overflow_hidden()
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .top_0()
                            .bottom_0()
                            .w(gpui::relative(fill))
                            .bg(dot),
                    ),
            )
            .child(
                div()
                    .relative()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(if turned > 0.5 {
                        T::color(T::on_accent())
                    } else {
                        T::color(T::primary())
                    })
                    .opacity(if turned > 0.5 { turned } else { 1. - turned })
                    .child(if turned > 0.5 {
                        "✓ 已完成"
                    } else {
                        "标记完成"
                    }),
            )
            .into_any_element()
    }
    /// What the run draws inside the pane: the veil, the words with a way to keep the
    /// pane, the sweep's wash and lids, and a line along the header counting down.
    pub(super) fn completing_veil(
        &self,
        id: &str,
        style: Style,
        t: f32,
        title: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let (dot, ink, _) = Group::Done.colors();
        let pane = T::color(T::pane());
        let tm = timing(style);
        let keep = id.to_owned();
        let (a, b, c, d) = tm.words;
        let shown = eo(sl(t, a, b)) * if c > 0. { 1. - sl(t, c, d) } else { 1. };
        let veil = match style {
            Style::Seal => 0.88 * eo(sl(t, 0.1, 0.35)),
            Style::Stamp => 0.86 * eo(sl(t, 0.06, 0.28)),
            Style::Sweep => 0.8,
            // Cells a full screen sends no speck for still fade with the rest.
            Style::Dissolve => 0.25 * sl(t, 0.05, 0.2) + 0.65 * sl(t, 0.35, 0.65),
            Style::Fade => 0.,
        };
        let veil_width = if style == Style::Sweep {
            sweep_at(t).clamp(0., 1.)
        } else {
            1.
        };
        let lids = if style == Style::Sweep {
            eio(sl(t, 0.66, 0.84))
        } else {
            0.
        };
        let countdown = (t / tm.fade.0).min(1.);
        let words = div()
            .absolute()
            .left_0()
            .right_0()
            .top(gpui::relative(0.5))
            .mt(px(
                if style == Style::Sweep { 34. } else { 42. } + T::S8 * (1. - shown)
            ))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(T::S4))
            .opacity(shown)
            .child(
                div()
                    .text_size(px(T::BODY_SIZE))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(ink)
                    .child("已完成"),
            )
            .child(
                div()
                    .max_w(px(336.))
                    .truncate()
                    .text_size(px(T::META))
                    .text_color(T::color(T::secondary()))
                    .child(title.to_owned()),
            )
            .child(
                button(
                    gpui::SharedString::from(format!("keep-pane-{id}")),
                    "留着这个画面",
                )
                .pane_action()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.layout.completing.remove(&keep);
                    cx.notify();
                })),
            );
        let body = div()
            .absolute()
            .left_0()
            .right_0()
            .top(px(T::PANE_HEADER))
            .bottom_0()
            .overflow_hidden()
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w(gpui::relative(veil_width))
                    .bg(pane.opacity(veil)),
            )
            .when(lids > 0., |d| {
                d.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top_0()
                        .h(gpui::relative(0.5 * lids))
                        .bg(pane),
                )
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .h(gpui::relative(0.5 * lids))
                        .bg(pane),
                )
            })
            .when(shown > 0.01, |d| d.child(words));
        div()
            .absolute()
            .inset_0()
            .when(style == Style::Sweep, |d| {
                d.child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .h(px(T::PANE_HEADER))
                        .w(gpui::relative(sweep_at(t).clamp(0., 1.)))
                        .bg(dot.opacity(0.14)),
                )
            })
            .child(body)
            .when(style != Style::Fade, |d| {
                d.child(
                    div()
                        .absolute()
                        .left_0()
                        .top(px(T::PANE_HEADER - 2.))
                        .h(px(2.))
                        .w(gpui::relative(countdown))
                        .bg(dot),
                )
            })
    }
    /// The part of every run that is not confined to its pane: seals, rings, specks and
    /// the light, drawn over the whole window and landing in the session's sidebar row.
    pub(super) fn completing_overlay(&self, window: &Window) -> Option<impl IntoElement> {
        if self.layout.completing.is_empty() {
            return None;
        }
        window.request_animation_frame();
        struct Frame {
            terminal: String,
            session: String,
            effect: Complete,
            t: f32,
            glyphs: Vec<(Point<Pixels>, Hsla, u16)>,
            cell: gpui::Size<Pixels>,
        }
        let frames: Vec<Frame> = self
            .layout
            .completing
            .iter()
            .map(|(terminal, r)| Frame {
                terminal: terminal.clone(),
                session: r.session.clone(),
                effect: r.effect,
                t: r.t(),
                glyphs: r.glyphs.clone(),
                cell: r.cell,
            })
            .collect();
        let spots = self.layout.spots.clone();
        let (green, _, _) = Group::Done.colors();
        let on_accent = T::color(T::on_accent());
        let pane_color = T::color(T::pane());
        Some(
            gpui::canvas(
                |_, _, _| (),
                move |_, _, window, _| {
                    let spots = spots.borrow();
                    for f in &frames {
                        let Some(pane) = spots.panes.get(&f.terminal).copied() else {
                            continue;
                        };
                        let row = spots.rows.get(&f.session).copied();
                        let target = row.filter(|_| f.effect.land).map(|r| {
                            point(
                                r.left() + px(T::S8 + T::DOT / 2.),
                                r.top() + px(T::ROW / 2.),
                            )
                        });
                        let mut p = Painter {
                            window,
                            green,
                            on_accent,
                            pane_color,
                        };
                        p.frame(f.effect.style, f.t, pane, target, &f.glyphs, f.cell);
                        if let (Some(r), Some(at)) = (row.filter(|_| f.effect.land), target) {
                            p.landed(f.t, timing(f.effect.style).land, r, at);
                        }
                    }
                },
            )
            .absolute()
            .inset_0(),
        )
    }
}

struct Painter<'a> {
    window: &'a mut Window,
    green: Hsla,
    on_accent: Hsla,
    pane_color: Hsla,
}
impl Painter<'_> {
    fn disc(&mut self, at: Point<Pixels>, r: f32, color: Hsla) {
        if r <= 0.05 || color.a <= 0.003 {
            return;
        }
        let b = Bounds::new(
            point(at.x - px(r), at.y - px(r)),
            size(px(2. * r), px(2. * r)),
        );
        self.window.paint_quad(gpui::quad(
            b,
            px(r),
            color,
            px(0.),
            gpui::transparent_black(),
            Default::default(),
        ));
    }
    /// A soft halo: a few wider, fainter discs under the real one.
    fn glow(&mut self, at: Point<Pixels>, r: f32, strength: f32) {
        if strength <= 0. {
            return;
        }
        for (grow, a) in [(16., 0.08), (10., 0.12), (5., 0.18)] {
            self.disc(at, r + grow * strength, self.green.opacity(a * strength));
        }
    }
    fn ring(&mut self, at: Point<Pixels>, r: f32, width: f32, color: Hsla) {
        if r <= 0. || color.a <= 0.003 {
            return;
        }
        let b = Bounds::new(
            point(at.x - px(r), at.y - px(r)),
            size(px(2. * r), px(2. * r)),
        );
        self.window.paint_quad(gpui::quad(
            b,
            px(r),
            gpui::transparent_black(),
            px(width),
            color,
            Default::default(),
        ));
    }
    fn rect(&mut self, b: Bounds<Pixels>, color: impl Into<gpui::Background>) {
        self.window.paint_quad(gpui::fill(b, color));
    }
    fn stroke(&mut self, points: &[Point<Pixels>], width: f32, color: Hsla) {
        if points.len() < 2 || color.a <= 0.003 {
            return;
        }
        let mut path = PathBuilder::stroke(px(width));
        path.move_to(points[0]);
        for &p in &points[1..] {
            path.line_to(p);
        }
        if let Ok(path) = path.build() {
            self.window.paint_path(path, color);
        }
    }
    /// The check mark drawn `progress` of the way along its two strokes.
    fn check(&mut self, at: Point<Pixels>, s: f32, progress: f32, color: Hsla, width: f32) {
        if progress <= 0. || s <= 0. {
            return;
        }
        let pts = [(-0.24, 0.02), (-0.07, 0.19), (0.26, -0.17)]
            .map(|(x, y)| point(at.x + px(x * s), at.y + px(y * s)));
        let d =
            |a: Point<Pixels>, b: Point<Pixels>| f32::from(a.x - b.x).hypot(f32::from(a.y - b.y));
        let (l1, l2) = (d(pts[0], pts[1]), d(pts[1], pts[2]));
        let len = (l1 + l2) * progress;
        let mid = |a: Point<Pixels>, b: Point<Pixels>, k: f32| {
            point(a.x + (b.x - a.x) * k, a.y + (b.y - a.y) * k)
        };
        let line = if len <= l1 {
            vec![pts[0], mid(pts[0], pts[1], len / l1)]
        } else {
            vec![pts[0], pts[1], mid(pts[1], pts[2], (len - l1) / l2)]
        };
        self.stroke(&line, width, color);
        // Round the joints and ends a stroke path leaves square.
        for p in &line {
            self.disc(*p, width / 2., color);
        }
    }
    fn arc(&mut self, at: Point<Pixels>, r: f32, sweep: f32, width: f32, color: Hsla) {
        let n = (48. * sweep).ceil().max(2.) as usize;
        let pts: Vec<_> = (0..=n)
            .map(|i| {
                let a = -std::f32::consts::FRAC_PI_2
                    + std::f32::consts::TAU * sweep * i as f32 / n as f32;
                point(at.x + px(r * a.cos()), at.y + px(r * a.sin()))
            })
            .collect();
        self.stroke(&pts, width, color);
    }

    fn frame(
        &mut self,
        style: Style,
        t: f32,
        pane: Bounds<Pixels>,
        target: Option<Point<Pixels>>,
        glyphs: &[(Point<Pixels>, Hsla, u16)],
        cell: gpui::Size<Pixels>,
    ) {
        let body = Bounds::new(
            point(pane.left(), pane.top() + px(T::PANE_HEADER)),
            size(pane.size.width, pane.size.height - px(T::PANE_HEADER)),
        );
        let center = point(body.center().x, body.center().y - px(14.));
        let fade = Workbench::completing_opacity(style, t);
        let g = self.green;
        match style {
            Style::Seal => {
                let pop = back_out(sl(t, 0.15, 0.42)).max(0.);
                let a = sl(t, 0.15, 0.25) * fade;
                self.disc(center, 28. * pop, g.opacity(a));
                if pop > 0.3 {
                    self.check(center, 45. * pop, 1., self.on_accent.opacity(a), 3.);
                }
            }
            Style::Fade => {}
            Style::Stamp => {
                let drop = sl(t, 0.12, 0.3);
                let settle = sl(t, 0.3, 0.4);
                let mut r =
                    lerp(48., 28., eo(drop)) * (1. - 0.07 * (std::f32::consts::PI * settle).sin());
                let glow = if drop < 1. {
                    drop * 0.5
                } else {
                    1. - eo(sl(t, 0.28, 0.55)) * 0.7
                };
                for (from, to, max, w) in [(0.3, 0.62, 120., 2.5), (0.35, 0.72, 80., 1.5)] {
                    let e = eo(sl(t, from, to));
                    if e > 0. && e < 1. {
                        self.ring(center, 28. + max * e, w, g.opacity(0.7 * (1. - e)));
                    }
                }
                let burst = sl(t, 0.3, 0.64);
                if burst > 0. && burst < 1. {
                    let mut rand = rng(7);
                    let tau = burst * 0.55;
                    for i in 0..22 {
                        let a = i as f32 / 22. * std::f32::consts::TAU + (rand() - 0.5) * 0.4;
                        let speed = 110. + rand() * 130.;
                        let s = (2.5 + rand() * 2.5) * (1. - burst * 0.7);
                        let light = rand() < 0.3;
                        let at = point(
                            center.x + px(a.cos() * speed * eo(burst) * 0.6),
                            center.y
                                + px((a.sin() * speed - 30.) * eo(burst) * 0.6 + 160. * tau * tau),
                        );
                        let color = if light {
                            gpui::hsla(0.41, 0.6, 0.93, 1.)
                        } else {
                            g
                        };
                        self.disc(at, s / 2., color.opacity(1. - burst));
                    }
                }
                let (fly, pos) = self.flight(t, 0.74, 0.88, center, target, (-150., 0.));
                if target.is_some() {
                    r = lerp(r, 3.5, eo(sl(t, 0.74, 0.86)));
                }
                let a = sl(t, 0.12, 0.2) * if target.is_some() { 1. } else { fade };
                if fly > 0. && fly < 1. {
                    for i in 1..=6 {
                        let k = (fly - i as f32 * 0.035).max(0.);
                        let (_, p) = self.flight_at(k, center, target, (-150., 0.));
                        self.disc(
                            p,
                            (r * (1. - i as f32 / 8.)).max(1.),
                            g.opacity(0.35 * (1. - i as f32 / 7.)),
                        );
                    }
                }
                if t < 0.88 || target.is_none() {
                    self.glow(pos, r, glow * a);
                    self.disc(pos, r, g.opacity(a));
                    self.check(
                        pos,
                        r * 1.6,
                        sl(t, 0.24, 0.38),
                        self.on_accent.opacity(a),
                        (r / 9.).max(1.2),
                    );
                }
            }
            Style::Sweep => {
                let x = body.left() + body.size.width * sweep_at(t);
                let sweep = eio(sl(t, 0.05, 0.4));
                if sweep > 0. && sweep < 1. {
                    let clip = |l: Pixels, r: Pixels| (l.max(body.left()), r.min(body.right()));
                    let (l, r) = clip(x - px(64.), x);
                    if r > l {
                        let b = Bounds::new(point(l, pane.top()), size(r - l, pane.size.height));
                        self.rect(
                            b,
                            gpui::linear_gradient(
                                90.,
                                gpui::linear_color_stop(g.opacity(0.), 0.),
                                gpui::linear_color_stop(g.opacity(0.22), 1.),
                            ),
                        );
                    }
                    let (l, r) = clip(x, x + px(16.));
                    if r > l {
                        let b = Bounds::new(point(l, pane.top()), size(r - l, pane.size.height));
                        self.rect(
                            b,
                            gpui::linear_gradient(
                                90.,
                                gpui::linear_color_stop(g.opacity(0.22), 0.),
                                gpui::linear_color_stop(g.opacity(0.), 1.),
                            ),
                        );
                    }
                    if x > body.left() && x < body.right() {
                        self.rect(
                            Bounds::new(
                                point(x - px(0.75), pane.top()),
                                size(px(1.5), pane.size.height),
                            ),
                            gpui::hsla(0.41, 0.6, 0.93, 0.9),
                        );
                    }
                }
                let close = eio(sl(t, 0.66, 0.84));
                let width = eo(sl(t, 0.4, 0.56)) * 180. * (1. - eio(sl(t, 0.84, 0.94)));
                let ly = body.center().y;
                let a = fade;
                if width > 0.5 {
                    let h = 1.5 + close;
                    let b = Bounds::new(
                        point(center.x - px(width / 2.), ly - px(h / 2.)),
                        size(px(width), px(h)),
                    );
                    for (grow, ga) in [(6., 0.08), (3., 0.15)] {
                        let gb = Bounds::new(
                            point(b.left() - px(grow), b.top() - px(grow)),
                            size(b.size.width + px(2. * grow), b.size.height + px(2. * grow)),
                        );
                        self.window.paint_quad(gpui::quad(
                            gb,
                            px(grow),
                            g.opacity(ga * (0.5 + close) * a),
                            px(0.),
                            gpui::transparent_black(),
                            Default::default(),
                        ));
                    }
                    self.rect(b, g.opacity(a));
                }
                self.check(
                    point(center.x, ly - px(34.)),
                    40.,
                    sl(t, 0.46, 0.6),
                    g.opacity(1. - sl(t, 0.62, 0.7)),
                    3.,
                );
                // The line, shrunk to a point, takes off for the sidebar.
                let (fly, pos) =
                    self.flight(t, 0.86, 0.94, point(center.x, ly), target, (-120., 0.));
                if target.is_some() && fly > 0. && fly < 1. {
                    self.glow(pos, 3., 1.);
                    self.disc(pos, 3.5, g);
                } else if target.is_none() && t > 0.86 {
                    let k = sl(t, 0.88, 0.97);
                    self.disc(point(center.x, ly), 2. + 10. * eo(k), g.opacity(1. - k));
                }
            }
            Style::Dissolve => {
                let target_or_up = target.unwrap_or(point(center.x, pane.top() - px(40.)));
                let mut rand = rng(42);
                let mut arrived = 0usize;
                let width = f32::from(body.size.width).max(1.);
                let height = f32::from(body.size.height).max(1.);
                for (at, color, cols) in glyphs {
                    let ord = f32::from(at.y - body.top()) / height * 0.72
                        + f32::from(at.x - body.left()) / width * 0.28;
                    let (j, cx_, cy_, rad) = (rand(), rand(), rand(), 0.7 + rand() * 0.8);
                    let start = 0.2 + 0.3 * ord.clamp(0., 1.) + j * 0.04;
                    let tau = sl(t, start, start + 0.36);
                    if tau <= 0. {
                        continue;
                    }
                    // The cell's text is covered as its speck leaves.
                    let w = cell.width * *cols as f32;
                    let cover = Bounds::new(
                        point(at.x - w / 2., at.y - cell.height / 2.),
                        size(w, cell.height),
                    );
                    self.rect(cover, self.pane_color.opacity(sl(tau, 0., 0.2)));
                    if tau >= 1. {
                        arrived += 1;
                        continue;
                    }
                    let ctl = point(
                        px(lerp(f32::from(at.x), f32::from(target_or_up.x), 0.35)
                            + (cx_ - 0.5) * 90.),
                        at.y.min(target_or_up.y) - px(30. + cy_ * 90.),
                    );
                    let k1 = sl(tau, 0., 0.2);
                    let m = eio(sl(tau, 0.08, 1.));
                    let p = bez(*at, ctl, target_or_up, m);
                    let q = bez(*at, ctl, target_or_up, (m - 0.022).max(0.));
                    let a = k1 * 0.9 * (1. - sl(tau, 0.9, 1.));
                    self.stroke(&[q, p], rad * 1.1, g.opacity(a * 0.35));
                    let tint = Hsla {
                        h: lerp(color.h, g.h, k1),
                        s: lerp(color.s, g.s, k1),
                        l: lerp(color.l, g.l, k1),
                        a,
                    };
                    self.disc(p, rad, tint);
                }
                if let Some(at) = target {
                    let f = arrived as f32 / glyphs.len().max(1) as f32;
                    if f > 0. {
                        self.glow(at, 3.5, f);
                        self.disc(at, 3.5 + 1.5 * f, g.opacity(0.25 + 0.75 * f));
                    }
                }
                let ring = eo(sl(t, 0.03, 0.2));
                let fill = back_out(sl(t, 0.2, 0.3));
                let (_, pos) = self.flight(t, 0.72, 0.86, center, target, (-170., 40.));
                let r = if target.is_some() {
                    lerp(24., 3.5, eo(sl(t, 0.72, 0.84)))
                } else {
                    24.
                };
                let a = if target.is_some() { 1. } else { fade };
                if t < 0.86 || target.is_none() {
                    if fill < 0.02 {
                        self.ring(center, 24., 2.5, g.opacity(0.18 * a));
                        self.arc(center, 24., ring, 2.5, g.opacity(a));
                    } else {
                        let rr = r * fill.min(1.15);
                        self.glow(pos, rr, 0.6 * a);
                        self.disc(pos, rr, g.opacity(a));
                        self.check(
                            pos,
                            r * 1.6,
                            sl(t, 0.24, 0.36),
                            self.on_accent.opacity(a),
                            (r / 8.).max(1.2),
                        );
                    }
                }
            }
        }
    }
    /// Progress of the flight between `from` and `to` of the run, and where it is.
    fn flight(
        &self,
        t: f32,
        from: f32,
        to: f32,
        start: Point<Pixels>,
        target: Option<Point<Pixels>>,
        bend: (f32, f32),
    ) -> (f32, Point<Pixels>) {
        self.flight_at(eio(sl(t, from, to)), start, target, bend)
    }
    fn flight_at(
        &self,
        k: f32,
        start: Point<Pixels>,
        target: Option<Point<Pixels>>,
        bend: (f32, f32),
    ) -> (f32, Point<Pixels>) {
        match target {
            Some(end) => {
                let ctl = point((start.x + end.x) / 2. + px(bend.1), start.y + px(bend.0));
                (k, bez(start, ctl, end, k))
            }
            None => (0., start),
        }
    }
    /// The sidebar row lights up and its dot sends two ripples once the session lands.
    fn landed(&mut self, t: f32, at: f32, row: Bounds<Pixels>, dot: Point<Pixels>) {
        if t < at {
            return;
        }
        let k = sl(t, at, 1.);
        let wash = self.green.opacity(0.16 * (1. - k) + 0.04);
        self.window.paint_quad(gpui::quad(
            row,
            px(T::CONTROL_RADIUS),
            wash,
            px(0.),
            gpui::transparent_black(),
            Default::default(),
        ));
        for (delay, max) in [(0., 18.), (0.25, 12.)] {
            let e = eo(sl(k, delay, 1.));
            if e > 0. && e < 1. {
                self.ring(dot, 3.5 + max * e, 1.5, self.green.opacity(0.7 * (1. - e)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slices_and_easing_stay_in_range() {
        assert_eq!(sl(0.1, 0.2, 0.4), 0.);
        assert!((sl(0.3, 0.2, 0.4) - 0.5).abs() < 1e-5);
        assert_eq!(sl(0.9, 0.2, 0.4), 1.);
        assert_eq!(eo(1.), 1.);
        assert_eq!(eio(0.), 0.);
        assert!((eio(1.) - 1.).abs() < 1e-5);
        assert!((back_out(1.) - 1.).abs() < 1e-5);
        // The stamp overshoots before it settles.
        assert!(back_out(0.7) > 1.);
    }

    #[test]
    fn every_style_ends_with_the_pane_gone() {
        for style in [
            Style::Seal,
            Style::Stamp,
            Style::Sweep,
            Style::Dissolve,
            Style::Fade,
        ] {
            assert_eq!(
                Workbench::completing_opacity(style, 0.),
                1.,
                "{style:?} starts shown"
            );
            assert!(
                Workbench::completing_opacity(style, 1.) < 1e-4,
                "{style:?} ends gone"
            );
            let tm = timing(style);
            assert!(
                tm.button < tm.fade.0,
                "{style:?}: the button turns before the fade"
            );
        }
    }

    #[test]
    fn the_same_seed_draws_the_same_frame() {
        let (mut a, mut b) = (rng(7), rng(7));
        for _ in 0..50 {
            let x = a();
            assert_eq!(x, b());
            assert!((0. ..1.).contains(&x));
        }
    }
}
