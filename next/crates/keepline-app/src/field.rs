use crate::theme::Theme as T;
use gpui::{
    App, Bounds, Context, ElementInputHandler, EntityInputHandler, FocusHandle, Focusable, Pixels,
    Point, UTF16Selection, Window, canvas, div, prelude::*, px,
};
use std::ops::Range;

pub struct Field {
    pub text: String,
    focus: FocusHandle,
    selection: Range<usize>,
    marked: Option<Range<usize>>,
    label: String,
}
impl Focusable for Field {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Field {
    pub fn new(label: &str, initial: &str, cx: &mut Context<Self>) -> Self {
        Self {
            text: initial.into(),
            focus: cx.focus_handle(),
            selection: initial.len()..initial.len(),
            marked: None,
            label: label.into(),
        }
    }
    fn byte(&self, utf16: usize) -> usize {
        let mut count = 0;
        for (i, ch) in self.text.char_indices() {
            if count >= utf16 {
                return i;
            }
            count += ch.len_utf16();
        }
        self.text.len()
    }
    fn utf16(&self, byte: usize) -> usize {
        self.text[..byte].encode_utf16().count()
    }
    fn replace(&mut self, range: Option<Range<usize>>, text: &str) -> Range<usize> {
        let range = range
            .map(|r| self.byte(r.start)..self.byte(r.end))
            .or(self.marked.clone())
            .unwrap_or(self.selection.clone());
        self.text.replace_range(range.clone(), text);
        self.selection = range.start + text.len()..range.start + text.len();
        range.start..range.start + text.len()
    }
}
impl Render for Field {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        div()
            .flex()
            .flex_col()
            .gap(px(T::S4))
            .child(
                div()
                    .text_size(px(T::META))
                    .text_color(T::color(T::SECONDARY))
                    .child(self.label.clone()),
            )
            .child(
                div()
                    .id("field")
                    .min_h(px(T::FIELD_HEIGHT))
                    .px(px(T::S12))
                    .py(px(T::S8))
                    .bg(T::color(T::RAISED))
                    .rounded(px(T::CONTROL_RADIUS))
                    .track_focus(&self.focus)
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, window, cx| window.focus(&this.focus, cx)),
                    )
                    .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                        if this.marked.is_some() {
                            return;
                        }
                        match event.keystroke.key.as_str() {
                            "a" if event.keystroke.modifiers.platform => {
                                this.selection = 0..this.text.len()
                            }
                            "v" if event.keystroke.modifiers.platform => {
                                if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text())
                                {
                                    this.replace(None, &text);
                                }
                            }
                            "backspace" => {
                                if this.selection.is_empty() && this.selection.start > 0 {
                                    this.selection.start = this.text[..this.selection.start]
                                        .char_indices()
                                        .last()
                                        .map(|(i, _)| i)
                                        .unwrap_or(0);
                                }
                                this.replace(None, "");
                            }
                            "enter" => {
                                this.replace(None, "\n");
                            }
                            _ => return,
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .child(if self.text.is_empty() {
                        " ".to_owned()
                    } else {
                        self.text.clone()
                    })
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, cx| {
                                let focus = entity.read(cx).focus.clone();
                                window.handle_input(
                                    &focus,
                                    ElementInputHandler::new(bounds, entity.clone()),
                                    cx,
                                );
                            },
                        )
                        .absolute()
                        .size_full(),
                    ),
            )
    }
}
impl EntityInputHandler for Field {
    fn text_for_range(
        &mut self,
        r: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.byte(r.start)..self.byte(r.end);
        *actual = Some(self.utf16(range.start)..self.utf16(range.end));
        Some(self.text[range].into())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.utf16(self.selection.start)..self.utf16(self.selection.end),
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked
            .as_ref()
            .map(|r| self.utf16(r.start)..self.utf16(r.end))
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        r: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace(r, text);
        self.marked = None;
        cx.notify();
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        r: Option<Range<usize>>,
        text: &str,
        selection: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let inserted = self.replace(r, text);
        self.marked = (!text.is_empty()).then_some(inserted.clone());
        if let Some(s) = selection {
            let prefix = self.utf16(inserted.start);
            self.selection = self.byte(prefix + s.start)..self.byte(prefix + s.end);
        }
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(bounds)
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.utf16(self.selection.end))
    }
}
