//! Small shared view pieces: buttons, labels, status dots and chips.
use super::theme::{Group, Theme as T};
use gpui::{div, prelude::*, px};

pub fn badge(group: Group, count: usize) -> gpui::Div {
    let (_, ink, bg) = group.colors();
    div()
        .px(px(T::S4))
        .rounded(px(T::CHIP_RADIUS))
        .text_size(px(T::CHIP_SIZE))
        .text_color(ink)
        .bg(bg)
        .child(count.to_string())
}
pub fn task_icon() -> gpui::Div {
    div()
        .size(px(T::CHIP_SIZE))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(T::S2))
        .border(px(T::BORDER_WIDTH))
        .border_color(T::color(T::link()))
        .text_color(T::color(T::link()))
        .text_size(px(T::CHIP_SIZE))
        .child("✓")
}
pub fn button(
    id: impl Into<gpui::ElementId>,
    text: impl Into<gpui::SharedString>,
) -> gpui::Stateful<gpui::Div> {
    let text = text.into();
    div()
        .id(id)
        .role(gpui::accesskit::Role::Button)
        .aria_label(text.clone())
        .h(px(T::ROW))
        .px(px(T::S12))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(T::CONTROL_RADIUS))
        .text_size(px(T::SMALL))
        .text_color(T::color(T::body()))
        .cursor_pointer()
        .hover(|s| s.bg(T::color(T::button_hover())))
        .child(text)
}
/// A square button showing one of the app's SVG marks, tinted like button text.
pub fn icon_button(
    id: impl Into<gpui::ElementId>,
    icon: &'static str,
    label: impl Into<gpui::SharedString>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .role(gpui::accesskit::Role::Button)
        .aria_label(label.into())
        .size(px(T::ROW))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(T::CONTROL_RADIUS))
        .cursor_pointer()
        .hover(|s| s.bg(T::color(T::button_hover())))
        .child(
            gpui::svg()
                .path(icon)
                .size(px(T::ICON_GLYPH))
                .text_color(T::color(T::secondary())),
        )
}
pub fn label(text: impl Into<gpui::SharedString>) -> gpui::Div {
    div()
        .text_size(px(T::META))
        .text_color(T::color(T::secondary()))
        .child(text.into())
}
/// The terminal style's status: one symbol in the state's ink, as a multiplexer would.
pub fn glyph(group: Group) -> gpui::Div {
    let symbol = match group {
        Group::Waiting => "◆",
        Group::Error => "✕",
        Group::Working | Group::Running => "◐",
        Group::Background => "◔",
        Group::Done | Group::Review => "✓",
        Group::Paused => "‖",
        Group::Ready | Group::Fresh => "○",
        Group::Stale => "·",
    };
    div()
        .flex_shrink_0()
        .text_color(group.colors().1)
        .child(symbol)
}
pub fn dot(group: Group) -> gpui::Div {
    div()
        .size(px(T::DOT))
        .flex_shrink_0()
        .rounded_full()
        .bg(group.colors().0)
}
/// Running work keeps its filled status color even after it has been viewed.
/// Other seen rows follow `colors.json`'s `seen.dot` style.
pub fn seen_dot(group: Group) -> gpui::Div {
    use super::status_colors::{SeenDot, StatusColors};
    if matches!(group, Group::Working | Group::Running) {
        return dot(group);
    }
    let hue = group.colors().0;
    match StatusColors::get().seen_dot {
        SeenDot::Fill => dot(group),
        SeenDot::Fade => dot(group).bg(hue.opacity(0.45)),
        SeenDot::Ring => div()
            .size(px(T::DOT))
            .flex_shrink_0()
            .rounded_full()
            .border(px(T::CARET_WIDTH))
            .border_color(hue),
    }
}
pub fn chip(group: Group) -> gpui::Div {
    div()
        .px(px(T::S6))
        .py(px(T::BORDER_WIDTH))
        .rounded(px(T::CHIP_RADIUS))
        .text_size(px(T::CHIP_SIZE))
        .text_color(group.colors().1)
        .bg(group.colors().2)
        .child(group.label())
}

/// A row of thin ticks, as on the diagnostics panels and the sidebar's quota rows: the used
/// share is filled in whole ticks, the rest stays drawn in the grid color so what is left
/// reads at a glance.
pub fn gauge(share: f64, fill: gpui::Hsla, height: f32) -> gpui::Canvas<()> {
    gpui::canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let ticks = (f32::from(bounds.size.width) / T::GAUGE_PITCH).floor() as usize;
            let filled = (ticks as f64 * share.clamp(0., 1.)).round() as usize;
            let empty = T::color(T::chart_grid());
            for i in 0..ticks {
                let at = bounds.origin + gpui::point(px(i as f32 * T::GAUGE_PITCH), px(0.));
                window.paint_quad(gpui::fill(
                    gpui::Bounds::new(at, gpui::size(px(T::GAUGE_TICK), bounds.size.height)),
                    if i < filled { fill } else { empty },
                ));
            }
        },
    )
    .w_full()
    .h(px(height))
}
