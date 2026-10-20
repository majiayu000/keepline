use crate::{
    connection::Connection,
    screen::{self, Screen},
    theme::{Group, Theme as T},
};
use futures::{StreamExt, channel::mpsc::unbounded};
use gpui::{
    App, Bounds, Context, ElementInputHandler, EntityInputHandler, FocusHandle, Focusable,
    KeyDownEvent, Pixels, Point, UTF16Selection, Window, canvas, div, prelude::*, px,
};
use keepline_core::{
    protocol::Request,
    terminal::libghostty_vt::{key::Mods, terminal::Mode},
};
use std::{ops::Range, path::Path, sync::atomic::Ordering};

pub struct Pane {
    connection: Connection,
    focus: FocusHandle,
    screen: Screen,
    marked: String,
    selection: Range<usize>,
    bounds: Option<Bounds<Pixels>>,
    metrics: Option<screen::CellMetrics>,
    cursor_anchor: screen::CursorAnchor,
    requested_size: (u16, u16),
    local_error: Option<String>,
    pending_prefill: Option<String>,
    text_selection: Option<((u16, u16), (u16, u16))>,
    selecting: bool,
    search: Option<gpui::Entity<crate::field::Field>>,
    matches: Vec<(usize, String)>,
}
impl Focusable for Pane {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Pane {
    #[cfg(feature = "visual-test")]
    pub fn verify_marked_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Bounds<Pixels> {
        let rows = self.screen.rows.clone();
        self.replace_and_mark_text_in_range(None, "nihao", Some(2..2), window, cx);
        let bounds = self.bounds.expect("painted terminal");
        let expected = self
            .metrics
            .unwrap()
            .cursor_bounds(bounds, self.cursor_anchor.get().unwrap());
        let range = self.bounds_for_range(0..5, bounds, window, cx).unwrap();
        assert_eq!(
            range.origin, expected.origin,
            "IME must anchor at the terminal cursor"
        );
        assert_eq!(range.size.height, expected.size.height);
        let caret = self.bounds_for_range(2..2, bounds, window, cx).unwrap();
        assert!(caret.left() > range.left() && caret.left() < range.right());
        assert_eq!(
            caret.bottom(),
            range.bottom(),
            "candidate anchor under the composed text"
        );
        assert_eq!(
            self.screen.rows, rows,
            "marked text must not enter terminal cells"
        );
        eprintln!("Pane IME handler: overlay={range:?}, requested caret={caret:?}");
        range
    }
    pub fn new(socket: &Path, id: &str, takeover: bool, cx: &mut Context<Self>) -> Self {
        let (tx, mut rx) = unbounded();
        let connection = Connection::new(socket, id, takeover, tx);
        cx.spawn(async move |this, cx| {
            while rx.next().await.is_some() {
                // Coalesce bursts at 60 Hz; no timer exists when the terminal is idle.
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(16))
                    .await;
                while rx.try_recv().is_ok() {}
                if this
                    .update(cx, |this, cx| {
                        let ready = this.connection.state.lock().is_ok_and(|s| {
                            s.writable
                                && s.vt.as_ref().is_some_and(|vt| {
                                    let multiline = this
                                        .pending_prefill
                                        .as_ref()
                                        .is_some_and(|c| c.contains('\n') || c.contains('\r'));
                                    !multiline
                                        || vt
                                            .terminal()
                                            .mode(Mode::BRACKETED_PASTE)
                                            .unwrap_or(false)
                                })
                        });
                        if ready {
                            if let Some(command) = this.pending_prefill.take() {
                                this.prefill(&command, cx);
                            }
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let focus = cx.focus_handle();
        Self {
            connection,
            focus,
            screen: Screen::default(),
            marked: String::new(),
            selection: 0..0,
            bounds: None,
            metrics: None,
            cursor_anchor: screen::CursorAnchor::default(),
            requested_size: (0, 0),
            local_error: None,
            pending_prefill: None,
            text_selection: None,
            selecting: false,
            search: None,
            matches: vec![],
        }
    }
    pub fn connection_label(&self) -> &'static str {
        let Ok(s) = self.connection.state.lock() else {
            return "连接错误";
        };
        if s.error.is_some() {
            "连接错误"
        } else if s.ended {
            "已退出"
        } else if s.vt.is_none() {
            "正在连接"
        } else if s.writable {
            "已连接"
        } else {
            "只读"
        }
    }
    pub fn has_other_owner(&self) -> bool {
        self.connection
            .state
            .lock()
            .is_ok_and(|s| s.vt.is_some() && !s.writable && !s.ended && s.error.is_none())
    }
    pub fn interrupt(&mut self, cx: &mut Context<Self>) {
        self.input(vec![3], cx);
    }
    pub fn paste(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
            self.prefill(&text, cx);
        }
    }
    pub fn copy(&self, cx: &mut Context<Self>) {
        let selected = self
            .text_selection
            .filter(|(a, b)| a != b)
            .map(|(a, b)| (a.min(b), a.max(b)));
        let mut lines = Vec::new();
        for (y, row) in self.screen.rows.iter().enumerate() {
            let mut text = String::new();
            for run in row {
                if run.text.is_ascii() {
                    for (i, c) in run.text.chars().enumerate() {
                        let pos = (y as u16, run.col + i as u16);
                        if selected.is_none_or(|(a, b)| a <= pos && pos <= b) {
                            text.push(c);
                        }
                    }
                } else {
                    let pos = (y as u16, run.col);
                    if selected.is_none_or(|(a, b)| a <= pos && pos <= b) {
                        text.push_str(&run.text);
                    }
                }
            }
            if selected.is_none_or(|(a, b)| a.0 <= y as u16 && y as u16 <= b.0) {
                lines.push(text.trim_end().to_owned());
            }
        }
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(lines.join("\n")));
    }
    fn cell_at(&self, point: Point<Pixels>) -> Option<(u16, u16)> {
        let bounds = self.bounds?;
        let metrics = self.metrics?;
        if !bounds.contains(&point) {
            return None;
        }
        Some((
            ((point.y - bounds.top()) / metrics.height).floor().max(0.) as u16,
            ((point.x - bounds.left()) / metrics.width).floor().max(0.) as u16,
        ))
    }
    pub fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let field = cx.new(|cx| crate::field::Field::new("搜索滚动内容", "", cx));
        window.focus(&field.focus_handle(cx), cx);
        self.search = Some(field);
        self.matches.clear();
        cx.notify();
    }
    fn find_text(&mut self, cx: &mut Context<Self>) {
        let Some(field) = &self.search else { return };
        let query = field.read(cx).text.trim().to_owned();
        let state = self.connection.state.clone();
        let job = cx.background_spawn(async move {
            state
                .lock()
                .map_err(|_| "terminal lock poisoned".to_owned())?
                .vt
                .as_ref()
                .ok_or("终端尚未恢复".to_owned())?
                .search(&query)
                .map_err(|e| e.to_string())
        });
        cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(lines) => this.matches = lines,
                    Err(e) => this.local_error = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn input(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        if let Err(error) = self.connection.send(Request::Input { bytes }) {
            self.local_error = Some(error);
            cx.notify();
        }
    }
    pub fn queue_prefill(&mut self, command: &str, cx: &mut Context<Self>) {
        self.pending_prefill = Some(command.into());
        cx.notify();
    }
    pub fn prefill(&mut self, command: &str, cx: &mut Context<Self>) {
        let bracketed = self
            .connection
            .state
            .lock()
            .ok()
            .and_then(|s| {
                s.vt.as_ref()
                    .and_then(|v| v.terminal().mode(Mode::BRACKETED_PASTE).ok())
            })
            .unwrap_or(false);
        if !bracketed && (command.contains('\n') || command.contains('\r')) {
            self.local_error = Some("终端尚未启用安全粘贴；请等待 shell 就绪后重试".into());
            cx.notify();
            return;
        }
        if command.contains('\x1b') {
            self.local_error = Some("待填入文本包含终端控制字符".into());
            cx.notify();
            return;
        }
        let bytes = if bracketed {
            format!("\x1b[200~{command}\x1b[201~").into_bytes()
        } else {
            command.as_bytes().to_vec()
        };
        self.input(bytes, cx);
    }
    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &event.keystroke;
        if k.modifiers.platform && k.key == "c" {
            self.copy(cx);
            cx.stop_propagation();
            return;
        }
        if k.modifiers.platform && k.key == "v" {
            if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                self.prefill(&text, cx);
            }
            cx.stop_propagation();
            return;
        }
        if !self.marked.is_empty() {
            return;
        }
        let Some(key) = crate::keyboard::physical_key(&k.key) else {
            return;
        };
        // Ordinary text belongs to the IME/text-input path, avoiding duplicate commits.
        if !k.modifiers.control && !k.modifiers.alt && crate::keyboard::is_text_key(&k.key) {
            return;
        }
        let result = (|| -> Result<Vec<u8>, String> {
            let state = self
                .connection
                .state
                .lock()
                .map_err(|_| "terminal state unavailable")?;
            let vt = state.vt.as_ref().ok_or("终端尚未恢复")?;
            let mut mods = Mods::empty();
            if k.modifiers.control {
                mods |= Mods::CTRL;
            }
            if k.modifiers.alt {
                mods |= Mods::ALT;
            }
            if k.modifiers.shift {
                mods |= Mods::SHIFT;
            }
            if k.modifiers.platform {
                mods |= Mods::SUPER;
            }
            crate::keyboard::encode(vt, key, &k.key, mods)
        })();
        match result {
            Ok(bytes) => self.input(bytes, cx),
            Err(error) => {
                self.local_error = Some(error);
                cx.notify();
            }
        }
        let _ = window;
        cx.stop_propagation();
    }
}
impl Render for Pane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.connection.dirty.store(false, Ordering::Release);
        let state = self.connection.state.lock().unwrap();
        if let Some(vt) = &state.vt {
            match screen::capture(vt) {
                Ok(screen) => {
                    self.cursor_anchor.observe(screen.cursor);
                    self.screen = screen;
                }
                Err(e) => self.local_error = Some(e.to_string()),
            }
        }
        let error = self.local_error.clone().or(state.error.clone());
        let status = if state.ended {
            "进程已退出 · 待核对"
        } else if self.pending_prefill.is_some() {
            "等待 shell 就绪后预填命令（回车才执行）"
        } else if state.vt.is_none() {
            "正在恢复终端…"
        } else if state.writable {
            "终端已连接"
        } else {
            "只读 · 可明确接管"
        };
        drop(state);
        let entity = cx.entity();
        let content = canvas(
            |_, _, _| (),
            move |bounds, _, window, cx| {
                entity.update(cx, |pane, cx| {
                    pane.bounds = Some(bounds);
                    window.handle_input(
                        &pane.focus,
                        ElementInputHandler::new(bounds, cx.entity()),
                        cx,
                    );
                    let font = T::mono(cx);
                    pane.metrics = screen::CellMetrics::measure(&font, window).or(pane.metrics);
                    let Some(metrics) = pane.metrics else { return };
                    screen::paint_pane(
                        &pane.screen,
                        &font,
                        bounds,
                        metrics,
                        pane.marked.is_empty(),
                        window,
                        cx,
                    );
                    if let Some((a, b)) = pane.text_selection {
                        let (start, end) = (a.min(b), a.max(b));
                        if start != end {
                            for row in start.0..=end.0 {
                                let left = if row == start.0 { start.1 } else { 0 };
                                let right = if row == end.0 {
                                    end.1 + 1
                                } else {
                                    pane.requested_size.0
                                };
                                window.paint_quad(gpui::fill(
                                    Bounds::new(
                                        gpui::point(
                                            bounds.left() + metrics.width * left as f32,
                                            bounds.top() + metrics.height * row as f32,
                                        ),
                                        gpui::size(
                                            metrics.width * right.saturating_sub(left) as f32,
                                            metrics.height,
                                        ),
                                    ),
                                    T::color(T::ACCENT).opacity(0.25),
                                ));
                            }
                        }
                    }
                    if !pane.marked.is_empty()
                        && let Some(cursor) = pane.cursor_anchor.get()
                    {
                        screen::MarkedText::new(
                            &pane.marked,
                            font,
                            metrics.cursor_bounds(bounds, cursor),
                            window,
                        )
                        .paint(pane.screen.background, window, cx);
                    }
                    let dims = (
                        (bounds.size.width / metrics.width).floor().clamp(2., 400.) as u16,
                        (bounds.size.height / metrics.height)
                            .floor()
                            .clamp(1., 200.) as u16,
                    );
                    if pane.requested_size != dims
                        && pane
                            .connection
                            .send(Request::Resize {
                                cols: dims.0,
                                rows: dims.1,
                            })
                            .is_ok()
                    {
                        pane.requested_size = dims;
                    }
                });
            },
        )
        .size_full();
        let mut view = div()
            .id("terminal")
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(T::color(T::PANE))
            .track_focus(&self.focus)
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this,event:&gpui::MouseDownEvent,window,cx|{window.focus(&this.focus,cx);this.text_selection=this.cell_at(event.position).map(|p|(p,p));this.selecting=true;cx.notify();}),
            )
            .on_mouse_move(cx.listener(|this,event:&gpui::MouseMoveEvent,_,cx|{if this.selecting {if let (Some((start,_)),Some(end))=(this.text_selection,this.cell_at(event.position)){this.text_selection=Some((start,end));cx.notify();}}}))
            .on_mouse_up(gpui::MouseButton::Left,cx.listener(|this,_,_,_|this.selecting=false))
            .on_mouse_up_out(gpui::MouseButton::Left,cx.listener(|this,_,_,_|this.selecting=false))
            .on_scroll_wheel(cx.listener(|this,event:&gpui::ScrollWheelEvent,_,cx|{
                if let Ok(mut state)=this.connection.state.lock(){if let Some(vt)=&mut state.vt {vt.scroll(keepline_core::terminal::libghostty_vt::terminal::ScrollViewport::Delta(-(event.delta.pixel_delta(px(T::MONO_SIZE*T::TERMINAL_LINE_HEIGHT)).y/px(T::MONO_SIZE*T::TERMINAL_LINE_HEIGHT)).round() as isize));}}cx.notify();
            }))
            .on_key_down(cx.listener(Self::key))
            .when(status != "终端已连接", |d| {
                d.child(
                    div()
                        .px(px(T::S12))
                        .py(px(T::S4))
                        .text_size(px(T::META))
                        .text_color(T::color(T::SECONDARY))
                        .child(status),
                )
            })
            .when_some(self.search.clone(),|d,field| {
                let mut panel=div().p(px(T::S8)).flex().flex_col().gap(px(T::S4)).child(field)
                    .child(div().flex().gap(px(T::S8))
                        .child(div().id("find").cursor_pointer().child("查找").on_click(cx.listener(|this,_,_,cx|this.find_text(cx))))
                        .child(div().id("close-find").cursor_pointer().child("关闭搜索").on_click(cx.listener(|this,_,window,cx|{this.search=None;this.matches.clear();window.focus(&this.focus,cx);cx.notify();}))))
                    .child(format!("{} 条匹配（显示前 20 条）",self.matches.len()));
                for (row,line) in self.matches.iter().take(20){let row=*row;panel=panel.child(div().id(gpui::SharedString::from(format!("match-{row}"))).truncate().cursor_pointer().child(line.clone())
                    .on_click(cx.listener(move|this,_,_,cx|{if let Ok(mut s)=this.connection.state.lock(){if let Some(vt)=&mut s.vt{vt.scroll(keepline_core::terminal::libghostty_vt::terminal::ScrollViewport::Row(row));}}this.search=None;cx.notify();})));}
                d.child(panel.id("search-results").max_h(px(T::SEARCH_HEIGHT)).overflow_y_scroll())
            })
            .child(div().flex_1().min_h_0().child(content));
        if let Some(error) = error {
            view = view.child(
                div()
                    .px(px(T::S12))
                    .py(px(T::S4))
                    .text_size(px(T::META))
                    .text_color(Group::Error.colors().1)
                    .child(error),
            );
        }
        view
    }
}
impl EntityInputHandler for Pane {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let utf16: Vec<u16> = self.marked.encode_utf16().collect();
        let end = range.end.min(utf16.len());
        let start = range.start.min(end);
        *actual = Some(start..end);
        String::from_utf16(&utf16[start..end]).ok()
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.selection.clone(),
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        (!self.marked.is_empty()).then(|| 0..self.marked.encode_utf16().count())
    }
    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.marked.clear();
        self.selection = 0..0;
        cx.notify();
        window.refresh();
    }
    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked.clear();
        self.selection = 0..0;
        self.input(text.as_bytes().to_vec(), cx);
        cx.notify();
        window.refresh();
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        selection: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked = text.into();
        self.selection = selection.unwrap_or_else(|| {
            let n = text.encode_utf16().count();
            n..n
        });
        cx.notify();
        window.refresh();
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let bounds = self.bounds?;
        let anchor = self
            .metrics?
            .cursor_bounds(bounds, self.cursor_anchor.get()?);
        Some(screen::MarkedText::new(&self.marked, T::mono(cx), anchor, window).range_bounds(range))
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(0)
    }
}
