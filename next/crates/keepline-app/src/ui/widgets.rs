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
        Group::Done | Group::Review => "✓",
        Group::Paused => "‖",
        Group::Ready => "○",
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
