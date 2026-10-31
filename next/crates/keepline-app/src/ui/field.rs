use crate::ui::theme::Theme as T;
use gpui::{
    App, Bounds, Context, ElementInputHandler, EntityInputHandler, FocusHandle, Focusable, Pixels,
    Point, UTF16Selection, Window, canvas, div, prelude::*, px,
};
use std::ops::Range;

/// Single-line fields report Enter / Esc so their owner can save or cancel.
pub enum FieldEvent {
    Submit,
    Cancel,
}
impl gpui::EventEmitter<FieldEvent> for Field {}
pub struct Field {
    pub text: String,
    focus: FocusHandle,
    selection: Range<usize>,
    marked: Option<Range<usize>>,
    label: String,
    /// Grey text shown while empty in place of the label.
    placeholder: Option<String>,
    compact: bool,
    bare: bool,
    inline: bool,
    /// The caret sits at the selection's start, as after Shift+Left.
    reversed: bool,
    /// A drag with the mouse is extending the selection.
    dragging: bool,
    /// How far a single-line field's text is shifted left to keep the caret visible.
    scroll: Pixels,
    /// The last drawn text, so clicks map back to a position in it.
    drawn: Option<gpui::TextLayout>,
    undo: Vec<(String, Range<usize>)>,
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
            placeholder: None,
            compact: false,
            bare: false,
            inline: false,
            reversed: false,
            dragging: false,
            scroll: px(0.),
            drawn: None,
            undo: Vec::new(),
        }
    }
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }
    /// One line with no box, standing in for text that is edited in place.
    pub fn inline(mut self) -> Self {
        self.compact = true;
        self.inline = true;
        self
    }
    pub fn placeholder(mut self, text: &str) -> Self {
        self.placeholder = Some(text.into());
        self
    }
    /// No box of its own: the field sits in a surface that already frames it.
    pub fn bare(mut self) -> Self {
        self.bare = true;
        self
    }
    /// An input method is composing, so arrow keys and Enter belong to it.
    pub fn composing(&self) -> bool {
        self.marked.is_some()
    }
    /// Typing then replaces the whole text, as when renaming.
    /// The caret end stays at the start, so a long name shows from its beginning.
    pub fn select_all(&mut self) {
        self.selection = 0..self.text.len();
        self.reversed = true;
    }
    pub fn set_text(&mut self, text: &str) {
        self.text = text.into();
        self.selection = self.text.len()..self.text.len();
        self.marked = None;
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
        if self.marked.is_none() && (!range.is_empty() || !text.is_empty()) {
            self.undo.push((self.text.clone(), self.selection.clone()));
            if self.undo.len() > 100 {
                self.undo.remove(0);
            }
        }
        self.text.replace_range(range.clone(), text);
        self.selection = range.start + text.len()..range.start + text.len();
        self.reversed = false;
        range.start..range.start + text.len()
    }
    /// Where the caret is: the end of the selection that moves.
    fn head(&self) -> usize {
        if self.reversed {
            self.selection.start
        } else {
            self.selection.end
        }
    }
    fn anchor(&self) -> usize {
        if self.reversed {
            self.selection.end
        } else {
            self.selection.start
        }
    }
    /// Moves the caret, dragging the selection along when `extend` is held.
    fn move_to(&mut self, to: usize, extend: bool) {
        let anchor = if extend { self.anchor() } else { to };
        self.reversed = to < anchor;
        self.selection = anchor.min(to)..anchor.max(to);
    }
    fn prev_char(&self, at: usize) -> usize {
        self.text[..at].char_indices().last().map_or(0, |(i, _)| i)
    }
    fn next_char(&self, at: usize) -> usize {
        self.text[at..]
            .chars()
            .next()
            .map_or(at, |c| at + c.len_utf8())
    }
    /// The start of the word before `at`, skipping the spaces in between.
    fn prev_word(&self, at: usize) -> usize {
        let mut chars = self.text[..at].char_indices().rev().peekable();
        while chars.next_if(|(_, c)| !c.is_alphanumeric()).is_some() {}
        let mut start = chars.peek().map_or(0, |(i, _)| *i);
        while let Some((i, _)) = chars.next_if(|(_, c)| c.is_alphanumeric()) {
            start = i;
        }
        start
    }
    fn next_word(&self, at: usize) -> usize {
        let mut chars = self.text[at..].char_indices().peekable();
        while chars.next_if(|(_, c)| !c.is_alphanumeric()).is_some() {}
        while chars.next_if(|(_, c)| c.is_alphanumeric()).is_some() {}
        chars.peek().map_or(self.text.len(), |(i, _)| at + i)
    }
    fn line_start(&self, at: usize) -> usize {
        self.text[..at].rfind('\n').map_or(0, |i| i + 1)
    }
    fn line_end(&self, at: usize) -> usize {
        self.text[at..]
            .find('\n')
            .map_or(self.text.len(), |i| at + i)
    }
    /// The word around `at`, as a double click selects it.
    fn word_at(&self, at: usize) -> Range<usize> {
        let start = if self.text[..at]
            .chars()
            .last()
            .is_some_and(char::is_alphanumeric)
        {
            self.prev_word(at)
        } else {
            at
        };
        let end = if self.text[at..]
            .chars()
            .next()
            .is_some_and(char::is_alphanumeric)
        {
            self.next_word(at)
        } else {
            self.next_char(at)
        };
        start..end
    }
    fn index_at(&self, at: Point<Pixels>) -> usize {
        if self.text.is_empty() {
            return 0;
        }
        let Some(layout) = &self.drawn else {
            return self.text.len();
        };
        let i = match layout.index_for_position(at) {
            Ok(i) | Err(i) => i.min(self.text.len()),
        };
        // Stay on a character boundary even if the layout rounds into one.
        (0..=i)
            .rev()
            .find(|&i| self.text.is_char_boundary(i))
            .unwrap_or(0)
    }
    /// Keeps the caret of a one-line field inside its box, and the text against
    /// its right edge once there is no more to the right.
    fn follow_caret(&mut self, layout: &gpui::TextLayout, visible: Pixels) -> bool {
        let origin = layout.bounds().origin.x;
        let x = |i| {
            layout
                .position_for_index(i)
                .map_or(px(0.), |p| p.x - origin)
        };
        let caret = x(self.head());
        let width = x(self.text.len());
        let visible = visible - px(T::CARET_WIDTH);
        let scroll = self
            .scroll
            .max(caret - visible)
            .min(caret)
            .min((width - visible).max(px(0.)))
            .max(px(0.));
        let moved = scroll != self.scroll;
        self.scroll = scroll;
        moved
    }
}
impl Field {
    /// Keys that edit or move; anything else falls through to the owner.
    fn key_down(&mut self, event: &gpui::KeyDownEvent, cx: &mut Context<Self>) {
        if self.marked.is_some() {
            return;
        }
        let m = &event.keystroke.modifiers;
        let (cmd, alt, shift) = (m.platform, m.alt, m.shift);
        let head = self.head();
        match event.keystroke.key.as_str() {
            "a" if cmd => self.select_all(),
            "c" | "x" if cmd => {
                if self.selection.is_empty() {
                    return;
                }
                let text = self.text[self.selection.clone()].to_owned();
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                if event.keystroke.key == "x" {
                    self.replace(None, "");
                }
            }
            "v" if cmd => {
                if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    // A one-line field takes a pasted paragraph as one line.
                    let text = if self.compact {
                        text.split_whitespace().collect::<Vec<_>>().join(" ")
                    } else {
                        text
                    };
                    self.replace(None, &text);
                }
            }
            "z" if cmd => {
                let Some((text, selection)) = self.undo.pop() else {
                    return;
                };
                self.text = text;
                self.selection = selection;
                self.reversed = false;
            }
            "left" | "right" | "home" | "end" => {
                let left = matches!(event.keystroke.key.as_str(), "left" | "home");
                let line = cmd || matches!(event.keystroke.key.as_str(), "home" | "end");
                let to = if !shift && !line && !alt && !self.selection.is_empty() {
                    // Without Shift an arrow first collapses the selection to that side.
                    if left {
                        self.selection.start
                    } else {
                        self.selection.end
                    }
                } else {
                    match (left, line, alt) {
                        (true, true, _) => self.line_start(head),
                        (false, true, _) => self.line_end(head),
                        (true, _, true) => self.prev_word(head),
                        (false, _, true) => self.next_word(head),
                        (true, ..) => self.prev_char(head),
                        (false, ..) => self.next_char(head),
                    }
                };
                self.move_to(to, shift);
            }
            "backspace" | "delete" => {
                if self.selection.is_empty() {
                    let back = event.keystroke.key == "backspace";
                    let to = match (back, cmd, alt) {
                        (true, true, _) => self.line_start(head),
                        (true, _, true) => self.prev_word(head),
                        (true, ..) => self.prev_char(head),
                        (false, true, _) => self.line_end(head),
                        (false, _, true) => self.next_word(head),
                        (false, ..) => self.next_char(head),
                    };
                    self.selection = to.min(head)..to.max(head);
                }
                self.replace(None, "");
            }
            // ⌘⏎ belongs to the owner: a form saves with it.
            "enter" if cmd => return,
            "enter" if !self.compact => {
                self.replace(None, "\n");
            }
            "enter" => cx.emit(FieldEvent::Submit),
            "escape" if self.compact => cx.emit(FieldEvent::Cancel),
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }
    fn mouse_down(
        &mut self,
        event: &gpui::MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        let at = self.index_at(event.position);
        match event.click_count {
            2 => {
                self.selection = self.word_at(at);
                self.reversed = false;
            }
            3.. => self.select_all(),
            _ => {
                self.move_to(at, event.modifiers.shift);
                self.dragging = true;
            }
        }
        cx.stop_propagation();
        cx.notify();
    }
    fn mouse_move(&mut self, event: &gpui::MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.dragging {
            return;
        }
        if event.pressed_button != Some(gpui::MouseButton::Left) {
            self.dragging = false;
            return;
        }
        self.move_to(self.index_at(event.position), true);
        cx.notify();
    }
}
impl Render for Field {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let empty = self.text.is_empty();
        let shown = if empty {
            match &self.placeholder {
                Some(p) => p.clone(),
                None if self.compact => self.label.clone(),
                None => " ".to_owned(),
            }
        } else {
            self.text.clone()
        };
        let mut highlights = Vec::new();
        if !empty && !self.selection.is_empty() {
            highlights.push((
                self.selection.clone(),
                gpui::HighlightStyle {
                    background_color: Some(T::color(T::selection())),
                    ..Default::default()
                },
            ));
        }
        if let Some(marked) = self.marked.clone().filter(|r| !r.is_empty()) {
            highlights.push((
                marked,
                gpui::HighlightStyle {
                    underline: Some(gpui::UnderlineStyle {
                        thickness: px(T::BORDER_WIDTH),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ));
        }
        let text = gpui::StyledText::new(shown).with_highlights(highlights);
        let layout = text.layout().clone();
        self.drawn = Some(layout.clone());
        // The caret only shows on an empty selection in the focused field.
        let caret = (self.focus.is_focused(window) && self.selection.is_empty())
            .then_some(if empty { 0 } else { self.selection.end });
        let compact = self.compact;
        let pad = if self.inline { 0. } else { T::S12 };
        div()
            .flex()
            .flex_col()
            .gap(px(T::S4))
            .when(self.inline, |d| d.size_full().justify_center())
            .when(!self.compact, |d| {
                d.child(
                    div()
                        .text_size(px(T::META))
                        .text_color(T::color(T::secondary()))
                        .child(self.label.clone()),
                )
            })
            .child(
                div()
                    .id("field")
                    .relative()
                    .min_h(px(if self.compact {
                        T::ROW
                    } else {
                        T::FIELD_HEIGHT
                    }))
                    .role(gpui::accesskit::Role::TextInput)
                    .aria_label(self.label.clone())
                    .px(px(pad))
                    .py(px(if self.compact { T::S4 } else { T::S8 }))
                    .bg(T::color(T::panel()))
                    .border(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::field_edge()))
                    .when(self.focus.is_focused(window), |d| {
                        d.border_color(T::color(T::focus()))
                    })
                    .when(self.compact && empty, |d| {
                        d.text_color(T::color(T::secondary()))
                    })
                    .when(self.compact, |d| {
                        d.h(px(T::ROW))
                            .flex()
                            .items_center()
                            .overflow_hidden()
                            .whitespace_nowrap()
                    })
                    .when(self.inline, |d| {
                        d.h_full()
                            .min_h_0()
                            .py_0()
                            .border_0()
                            .bg(gpui::transparent_black())
                            .cursor_text()
                    })
                    .when(self.bare, |d| {
                        d.h(px(T::SEARCH_FIELD))
                            .flex()
                            .items_center()
                            .px_0()
                            .text_size(px(T::SEARCH_TEXT))
                            .border_0()
                            .bg(gpui::transparent_black())
                    })
                    .rounded(px(T::CONTROL_RADIUS))
                    .track_focus(&self.focus)
                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(Self::mouse_down))
                    .on_mouse_move(cx.listener(Self::mouse_move))
                    .on_mouse_up(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, _, _| this.dragging = false),
                    )
                    .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                        this.key_down(event, cx)
                    }))
                    // A one-line field slides its text left as the caret moves right.
                    .child(
                        div()
                            .when(compact, |d| d.flex_shrink_0().relative().left(-self.scroll))
                            .child(text),
                    )
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, cx| {
                                if compact && !entity.read(cx).text.is_empty() {
                                    let visible = bounds.size.width - px(pad * 2.);
                                    let moved =
                                        entity.update(cx, |f, _| f.follow_caret(&layout, visible));
                                    if moved {
                                        // Drawn once more at the new offset next frame.
                                        let entity = entity.clone();
                                        window.on_next_frame(move |_, cx| {
                                            entity.update(cx, |_, cx| cx.notify())
                                        });
                                        window.refresh();
                                    }
                                }
                                if let Some(at) = caret.and_then(|i| layout.position_for_index(i)) {
                                    window.paint_quad(gpui::fill(
                                        gpui::Bounds::new(
                                            at,
                                            gpui::size(px(T::CARET_WIDTH), layout.line_height()),
                                        ),
                                        T::color(T::primary()),
                                    ));
                                }
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
