//! 标记完成 from a pane's header: the button fills and ripples, a seal pops over the
//! pane with a countdown on the header, then the pane fades and closes. 「留着这个画面」
//! keeps it open; the session stays completed either way.
use super::layout::PaneAction;
use super::*;
use gpui::{Animation, AnimationExt};
use std::time::Duration;

/// From the click to the close; every part below is a slice of it.
pub(super) const COMPLETE_HOLD: Duration = Duration::from_millis(1600);
const SEAL: f32 = 56.;
const SEAL_MARK: f32 = 28.;
const RIPPLE: f32 = 10.;
const COUNTDOWN: f32 = 2.;

/// `t`'s progress through `from..to` of the whole run, clamped to 0..=1.
fn slice(t: f32, from: f32, to: f32) -> f32 {
    ((t - from) / (to - from)).clamp(0., 1.)
}
fn ease_out(x: f32) -> f32 {
    1. - (1. - x).powi(4)
}
/// Overshoots a little before settling, like a stamp pressed down.
fn back_out(x: f32) -> f32 {
    let (c1, c3) = (1.70158, 2.70158);
    1. + c3 * (x - 1.).powi(3) + c1 * (x - 1.).powi(2)
}
fn run(key: &str, id: &str, seq: u64) -> (gpui::ElementId, Animation) {
    (
        gpui::SharedString::from(format!("{key}-{id}-{seq}")).into(),
        Animation::new(COMPLETE_HOLD),
    )
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
        self.layout.completing_seq += 1;
        let seq = self.layout.completing_seq;
        self.layout.completing.insert(terminal.clone(), seq);
        self.edit(
            Edit::SetOutcome {
                id: session.clone(),
                outcome: Outcome::Completed,
            },
            cx,
        );
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(COMPLETE_HOLD).await;
            let _ = this.update(cx, |this, cx| {
                if this.layout.completing.get(&terminal) != Some(&seq) {
                    return;
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
    pub(super) fn completing(&self, terminal: &str) -> Option<u64> {
        self.layout.completing.get(terminal).copied()
    }
    /// The header button mid-run: a fill sweeps in, the words turn to a check, a ring
    /// spreads out and fades.
    pub(super) fn completing_button(&self, id: &str, seq: u64) -> gpui::AnyElement {
        let (dot, _, _) = Group::Done.colors();
        let (fill_id, fill_run) = run("complete-fill", id, seq);
        let (ring_id, ring_run) = run("complete-ring", id, seq);
        let (text_id, text_run) = run("complete-text", id, seq);
        let fill = div()
            .absolute()
            .left_0()
            .top_0()
            .bottom_0()
            .bg(dot)
            .with_animation(fill_id, fill_run, |d, t| {
                d.w(gpui::relative(ease_out(slice(t, 0., 0.25))))
            });
        let ring = div()
            .absolute()
            .inset_0()
            .rounded(px(T::PANE_ACTION_RADIUS))
            .border(px(T::FOCUS_RING))
            .border_color(dot)
            .with_animation(ring_id, ring_run, move |d, t| {
                let e = ease_out(slice(t, 0.12, 0.5));
                let out = px(-RIPPLE * e);
                d.top(out)
                    .bottom(out)
                    .left(out)
                    .right(out)
                    .rounded(px(T::PANE_ACTION_RADIUS + RIPPLE * e))
                    .opacity((1. - e) * 0.8)
            });
        let words = div()
            .relative()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .with_animation(text_id, text_run, |d, t| {
                let turned = slice(t, 0.08, 0.2);
                d.text_color(if turned > 0.5 {
                    T::color(T::on_accent())
                } else {
                    T::color(T::primary())
                })
                .opacity(if turned > 0.5 { turned } else { 1. - turned })
                .child(if turned > 0.5 {
                    "✓ 已完成"
                } else {
                    "标记完成"
                })
            });
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
            .child(ring)
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(T::PANE_ACTION_RADIUS))
                    .overflow_hidden()
                    .child(fill),
            )
            .child(words)
            .into_any_element()
    }
    /// Over the pane below its header: a veil fades in, the seal pops, the title and a
    /// way to keep the pane follow; a line along the header counts down to the close.
    pub(super) fn completing_veil(
        &self,
        id: &str,
        seq: u64,
        title: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let (dot, ink, _) = Group::Done.colors();
        let veil_color = T::color(T::pane());
        let (veil_id, veil_run) = run("complete-veil", id, seq);
        let (seal_id, seal_run) = run("complete-seal", id, seq);
        let (words_id, words_run) = run("complete-words", id, seq);
        let (line_id, line_run) = run("complete-line", id, seq);
        let keep = id.to_owned();
        let seal = div()
            .rounded_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(dot)
            .text_color(T::color(T::on_accent()))
            .font_weight(gpui::FontWeight::BOLD)
            .child("✓")
            .with_animation(seal_id, seal_run, |d, t| {
                let pop = back_out(slice(t, 0.15, 0.42)).max(0.);
                d.size(px(SEAL * pop))
                    .text_size(px((SEAL_MARK * pop).max(1.)))
                    .opacity(slice(t, 0.15, 0.25))
            });
        let words = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(T::S4))
            .child(
                div()
                    .text_size(px(T::BODY_SIZE))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(ink)
                    .child("已完成"),
            )
            .child(
                div()
                    .max_w(px(SEAL * 6.))
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
            )
            .with_animation(words_id, words_run, |d, t| {
                let e = ease_out(slice(t, 0.3, 0.5));
                d.opacity(e).mt(px(T::S8 * (1. - e)))
            });
        div()
            .absolute()
            .inset_0()
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(T::PANE_HEADER))
                    .bottom_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(T::S12))
                    .child(seal)
                    .child(words)
                    .with_animation(veil_id, veil_run, move |d, t| {
                        d.bg(veil_color.opacity(0.88 * ease_out(slice(t, 0.1, 0.35))))
                    }),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(T::PANE_HEADER - COUNTDOWN))
                    .h(px(COUNTDOWN))
                    .bg(dot)
                    .with_animation(line_id, line_run, |d, t| d.w(gpui::relative(t))),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slices_and_easing_stay_in_range() {
        assert_eq!(slice(0.1, 0.2, 0.4), 0.);
        assert!((slice(0.3, 0.2, 0.4) - 0.5).abs() < 1e-5);
        assert_eq!(slice(0.9, 0.2, 0.4), 1.);
        assert_eq!(ease_out(1.), 1.);
        assert!((back_out(1.) - 1.).abs() < 1e-5);
        // The stamp overshoots before it settles.
        assert!(back_out(0.7) > 1.);
    }
}
