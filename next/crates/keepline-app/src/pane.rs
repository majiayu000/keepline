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
    terminal::libghostty_vt::{key::Mods, mouse, terminal::Mode},
};
use std::{ops::Range, path::Path, sync::atomic::Ordering, time::Instant};

#[path = "pane_selection.rs"]
mod selection;
use selection::TextSelection;

pub struct Pane {
    terminal: String,
    connection: Connection,
    focus: FocusHandle,
    screen: Screen,
    capture: Option<screen::Capture>,
    layout_cache: screen::LayoutCache,
    metrics_font: Option<(gpui::Font, f32)>,
    marked: String,
    selection: Range<usize>,
    bounds: Option<Bounds<Pixels>>,
    metrics: Option<screen::CellMetrics>,
    cursor_anchor: screen::CursorAnchor,
    requested_size: (u16, u16),
    /// Device-pixel cell size last sent with the size, for the program's size reports.
    requested_cell: (u32, u32),
    local_error: Option<String>,
    handoff: Option<Handoff>,
    text_selection: Option<TextSelection>,
    selecting: bool,
    mouse: Option<crate::terminal_input::MouseEncoder>,
    pressed_button: Option<gpui::MouseButton>,
    scroll_lines: f32,
    hover_link: Option<(u16, u16)>,
    focus_observers: bool,
    last_focus: bool,
    search: Option<gpui::Entity<crate::field::Field>>,
    matches: Vec<(usize, String)>,
    /// A multi-line paste waiting for the user's go-ahead (no bracketed paste).
    paste_confirm: Option<String>,
    bells_seen: u64,
    /// The program rang while the user looked elsewhere; the header shows it until focus.
    pub bell: bool,
    /// A bell in the pane the user is looking at flashes its border once.
    flash: bool,
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
                    .update(cx, |pane, cx| {
                        pane.ring(cx);
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
            terminal: id.into(),
            connection,
            focus,
            screen: Screen::default(),
            capture: None,
            layout_cache: screen::LayoutCache::default(),
            metrics_font: None,
            marked: String::new(),
            selection: 0..0,
            bounds: None,
            metrics: None,
            cursor_anchor: screen::CursorAnchor::default(),
            requested_size: (0, 0),
            requested_cell: (0, 0),
            local_error: None,
            handoff: None,
            text_selection: None,
            selecting: false,
            mouse: None,
            pressed_button: None,
            scroll_lines: 0.,
            hover_link: None,
            focus_observers: false,
            last_focus: false,
            search: None,
            matches: vec![],
            paste_confirm: None,
            bells_seen: 0,
            bell: false,
            flash: false,
        }
    }
    fn ring(&mut self, cx: &mut Context<Self>) {
        let rung = self.connection.bells.load(Ordering::Relaxed);
        if rung == self.bells_seen {
            return;
        }
        self.bells_seen = rung;
        if self.last_focus {
            self.flash = true;
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(150))
                    .await;
                let _ = this.update(cx, |pane, cx| {
                    pane.flash = false;
                    cx.notify();
                });
            })
            .detach();
        } else {
            self.bell = true;
            // Bounces the Dock icon once, only while Keepline is not the active app.
            crate::desktop::request_attention();
        }
    }
    /// The title the program set (OSC 0/2), e.g. a shell's "user@host: ~/dir" or vim's.
    pub fn program_title(&self) -> Option<String> {
        self.connection.state.lock().ok()?.vt.as_ref()?.title()
    }
    /// Dropped files arrive as their paths, escaped for a shell, like any terminal.
    fn drop_paths(&mut self, paths: &gpui::ExternalPaths, cx: &mut Context<Self>) {
        let text = crate::terminal_input::dropped_paths(paths.paths());
        if !text.is_empty() {
            self.prefill(&text, cx);
        }
    }
    fn sync_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focused = window.is_window_active() && self.focus.is_focused(window);
        if self.last_focus == focused {
            return;
        }
        self.last_focus = focused;
        // Notifications skip the terminal the user is already looking at.
        let watching = cx
            .try_global::<crate::desktop::Watching>()
            .and_then(|w| w.0.clone());
        if focused {
            self.bell = false;
            cx.set_global(crate::desktop::Watching(Some(self.terminal.clone())));
        } else if watching.as_deref() == Some(self.terminal.as_str()) {
            cx.set_global(crate::desktop::Watching(None));
        }
        let bytes = {
            let state = self.connection.state.lock().unwrap();
            state
                .vt
                .as_ref()
                .filter(|_| state.writable)
                .map(|v| crate::terminal_input::focus_bytes(v, focused))
        };
        match bytes {
            Some(Ok(b)) if !b.is_empty() => self.input(b, cx),
            Some(Err(e)) => {
                self.local_error = Some(e.to_string());
                cx.notify();
            }
            _ => (),
        }
        cx.notify(); // Invalidate the cached subtree when focus/activation changes.
    }
    fn report_mouse(
        &mut self,
        action: mouse::Action,
        button: Option<mouse::Button>,
        position: Point<Pixels>,
        modifiers: gpui::Modifiers,
        scale: f32,
        cx: &mut Context<Self>,
    ) -> bool {
        if modifiers.shift {
            return false;
        }
        let (Some(bounds), Some(metrics)) = (self.bounds, self.metrics) else {
            return false;
        };
        if !bounds.contains(&position) && action != mouse::Action::Release {
            return false;
        }
        let result = (|| {
            let state = self.connection.state.lock().unwrap();
            let Some(v) = state.vt.as_ref().filter(|_| state.writable) else {
                return Ok(None);
            };
            if !v.terminal().is_mouse_tracking()? {
                return Ok(None);
            }
            if self.mouse.is_none() {
                self.mouse = Some(crate::terminal_input::MouseEncoder::new()?);
            }
            let mut mods = Mods::empty();
            if modifiers.control {
                mods |= Mods::CTRL;
            }
            if modifiers.alt {
                mods |= Mods::ALT;
            }
            if modifiers.platform {
                mods |= Mods::SUPER;
            }
            let pos = mouse::Position {
                x: f32::from(position.x - bounds.left()).max(0.) * scale,
                y: f32::from(position.y - bounds.top()).max(0.) * scale,
            };
            let size = mouse::EncoderSize {
                screen_width: (f32::from(bounds.size.width) * scale).round() as u32,
                screen_height: (f32::from(bounds.size.height) * scale).round() as u32,
                cell_width: (f32::from(metrics.width) * scale).round().max(1.) as u32,
                cell_height: (f32::from(metrics.height) * scale).round().max(1.) as u32,
                padding_top: 0,
                padding_bottom: 0,
                padding_left: 0,
                padding_right: 0,
            };
            crate::terminal_input::mouse_bytes(
                self.mouse.as_mut().unwrap(),
                v,
                action,
                button,
                mods,
                pos,
                size,
            )
            .map(Some)
        })();
        match result {
            Ok(Some(bytes)) => {
                if !bytes.is_empty() {
                    self.input(bytes, cx);
                }
                cx.stop_propagation();
                true
            }
            Ok(None) => false,
            Err(e) => {
                self.local_error = Some(e.to_string());
                cx.notify();
                true
            }
        }
    }
    fn mouse_down(
        &mut self,
        event: &gpui::MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        if event.modifiers.platform
            && event.button == gpui::MouseButton::Left
            && let Some((y, x)) = self.cell_at(event.position)
            && let Some(link) = self.screen.link_at(y, x)
        {
            cx.open_url(&link.uri);
            cx.stop_propagation();
            return;
        }
        let button = match event.button {
            gpui::MouseButton::Left => mouse::Button::Left,
            gpui::MouseButton::Middle => mouse::Button::Middle,
            _ => mouse::Button::Right,
        };
        self.pressed_button = Some(event.button);
        if self.report_mouse(
            mouse::Action::Press,
            Some(button),
            event.position,
            event.modifiers,
            window.scale_factor(),
            cx,
        ) {
            return;
        }
        if event.button == gpui::MouseButton::Left {
            let cell = self.cell_at(event.position);
            // Double click selects a word, triple click the row, as in any terminal.
            self.text_selection = match event.click_count {
                2 => cell.map(|p| {
                    let (start, end) = self.word_at(p);
                    TextSelection::range(start, end)
                }),
                3.. => cell.map(|(row, _)| {
                    TextSelection::range((row, 0), (row, self.cols().saturating_sub(1)))
                }),
                _ => cell.map(TextSelection::Anchor),
            };
            self.selecting = event.click_count < 2;
            cx.notify();
        }
    }
    fn mouse_up(
        &mut self,
        event: &gpui::MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // The outside handler sees releases over tabs and other panes too.
        // Only capture a release for a gesture that started in this pane.
        if self.pressed_button != Some(event.button) {
            return;
        }
        self.pressed_button = None;
        let button = match event.button {
            gpui::MouseButton::Left => mouse::Button::Left,
            gpui::MouseButton::Middle => mouse::Button::Middle,
            _ => mouse::Button::Right,
        };
        self.report_mouse(
            mouse::Action::Release,
            Some(button),
            event.position,
            event.modifiers,
            window.scale_factor(),
            cx,
        );
        self.selecting = false;
    }
    fn mouse_move(
        &mut self,
        event: &gpui::MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let hover = self
            .cell_at(event.position)
            .filter(|(y, x)| self.screen.link_at(*y, *x).is_some());
        if hover != self.hover_link {
            self.hover_link = hover;
            cx.notify();
        }
        let button = event.pressed_button.map(|b| match b {
            gpui::MouseButton::Left => mouse::Button::Left,
            gpui::MouseButton::Middle => mouse::Button::Middle,
            _ => mouse::Button::Right,
        });
        if self.report_mouse(
            mouse::Action::Motion,
            button,
            event.position,
            event.modifiers,
            window.scale_factor(),
            cx,
        ) {
            return;
        }
        if self.selecting
            && let (Some(selection), Some(end)) = (self.text_selection, self.cell_at(event.position))
        {
            self.text_selection = Some(selection.drag_to(end));
            cx.notify();
        }
    }
    fn scroll_wheel(
        &mut self,
        event: &gpui::ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(m) = self.metrics else {
            return;
        };
        self.scroll_lines += event.delta.pixel_delta(m.height).y / m.height;
        let lines = self.scroll_lines.trunc().clamp(-100., 100.) as i32;
        self.scroll_lines -= lines as f32;
        if lines == 0 {
            return;
        }
        for _ in 0..lines.unsigned_abs() {
            if self.report_mouse(
                mouse::Action::Press,
                Some(if lines > 0 {
                    mouse::Button::Four
                } else {
                    mouse::Button::Five
                }),
                event.position,
                event.modifiers,
                window.scale_factor(),
                cx,
            ) {
                continue;
            }
            let result = {
                let mut state = self.connection.state.lock().unwrap();
                if let Some(v) = &mut state.vt {
                    match crate::terminal_input::alternate_scroll(v, lines > 0) {
                        Ok(Some(bytes)) if !event.modifiers.shift => Ok(Some(bytes)),
                        Ok(_) => {
                            v.scroll(keepline_core::terminal::libghostty_vt::terminal::ScrollViewport::Delta(if lines>0 {-1}else{1}));
                            Ok(None)
                        }
                        Err(e) => Err(e),
                    }
                } else {
                    Ok(None)
                }
            };
            match result {
                Ok(Some(b)) => self.input(b, cx),
                Err(e) => self.local_error = Some(e),
                _ => (),
            }
        }
        cx.stop_propagation();
        cx.notify();
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
    /// A user's own paste. Without bracketed paste each line would run as soon as it
    /// arrives, so a multi-line paste asks first instead of being refused, as Ghostty's
    /// paste protection does. Text Keepline places itself still goes through `prefill`.
    pub fn paste(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) else {
            return;
        };
        if !self.bracketed() && line_count(&text) > 1 && !text.contains('\x1b') {
            self.paste_confirm = Some(text);
            self.local_error = None;
            cx.notify();
        } else {
            self.prefill(&text, cx);
        }
    }
    fn confirm_paste(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = self.paste_confirm.take() {
            self.input(plain_paste(&text), cx);
        }
        cx.notify();
    }
    fn bracketed(&self) -> bool {
        self.connection
            .state
            .lock()
            .ok()
            .and_then(|s| {
                s.vt.as_ref()
                    .and_then(|v| v.terminal().mode(Mode::BRACKETED_PASTE).ok())
            })
            .unwrap_or(false)
    }
    pub fn copy(&self, cx: &mut Context<Self>) {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(selection::copied_text(
            &self.screen.rows,
            self.text_selection,
        )));
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
    fn cols(&self) -> u16 {
        self.requested_size.0.max(1)
    }
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        let rows = self.screen.rows.len() as u16;
        if rows > 0 {
            self.text_selection = Some(TextSelection::range((0, 0), (rows - 1, self.cols() - 1)));
            cx.notify();
        }
    }
    fn word_at(&self, (row, col): (u16, u16)) -> ((u16, u16), (u16, u16)) {
        let row_runs = self
            .screen
            .rows
            .get(row as usize)
            .map_or(&[][..], Vec::as_slice);
        let (start, end) = screen::word_around(row_runs, col);
        ((row, start), (row, end))
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
    /// Launch an Agent in this shell, then place `text` in the Agent's own input box
    /// without submitting it. Nothing is ever typed into the shell but the command.
    pub fn start_agent(&mut self, command: &str, text: Option<String>, cx: &mut Context<Self>) {
        self.handoff = Some(Handoff {
            command: Some(command.into()),
            text,
            stage: Stage::Shell,
            started: Instant::now(),
            released: false,
            tail: String::new(),
            still: 0,
        });
        self.watch_handoff(cx);
    }
    /// The Agent was started by hand; only the text still needs to reach its input box.
    pub fn place_text(&mut self, text: String, cx: &mut Context<Self>) {
        self.handoff = Some(Handoff {
            command: None,
            text: Some(text),
            stage: Stage::Agent,
            started: Instant::now(),
            released: true,
            tail: String::new(),
            still: 0,
        });
        self.watch_handoff(cx);
    }
    pub fn retry_handoff(&mut self, cx: &mut Context<Self>) {
        if let Some(h) = &mut self.handoff
            && matches!(h.stage, Stage::NotReady | Stage::Missing)
        {
            // After a missing command the user installs it, so run the command again.
            if h.stage == Stage::Missing {
                h.stage = Stage::Shell;
                h.released = false;
            } else {
                h.stage = Stage::Agent;
                h.released = true;
            }
            h.started = Instant::now();
            h.still = 0;
            self.watch_handoff(cx);
        }
    }
    pub fn dismiss_handoff(&mut self, cx: &mut Context<Self>) {
        self.handoff = None;
        cx.notify();
    }
    /// What the bar above this pane says, or None when there is nothing to report.
    pub fn handoff_state(&self) -> Option<HandoffState> {
        let h = self.handoff.as_ref()?;
        Some(match h.stage {
            Stage::Shell | Stage::Agent => HandoffState::Waiting,
            Stage::Placed => HandoffState::Placed,
            Stage::NotReady => HandoffState::NotReady,
            Stage::Missing => HandoffState::Missing,
        })
    }
    // A short-lived 250 ms check that ends with the handoff; idle panes keep no timer.
    fn watch_handoff(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(250))
                    .await;
                let busy = this
                    .update(cx, |pane, cx| {
                        pane.step_handoff(cx);
                        pane.handoff
                            .as_ref()
                            .is_some_and(|h| matches!(h.stage, Stage::Shell | Stage::Agent))
                    })
                    .unwrap_or(false);
                if !busy {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }
    fn step_handoff(&mut self, cx: &mut Context<Self>) {
        let Some(h) = self.handoff.as_mut() else {
            return;
        };
        if !matches!(h.stage, Stage::Shell | Stage::Agent) {
            return;
        }
        let (bracketed, tail) = {
            let Ok(state) = self.connection.state.lock() else {
                return;
            };
            if !state.writable || state.ended {
                return;
            }
            let Some(vt) = state.vt.as_ref() else {
                return;
            };
            (
                vt.terminal().mode(Mode::BRACKETED_PASTE).unwrap_or(false),
                // The whole viewport, minus blank rows under the content: a tall pane
                // with a shell at the top would otherwise show only blank rows here.
                vt.tail_text(usize::from(self.requested_size.1.max(40)))
                    .unwrap_or_default()
                    .trim_end()
                    .to_owned(),
            )
        };
        if tail == h.tail {
            h.still = h.still.saturating_add(1);
        } else {
            h.tail = tail;
            h.still = 0;
        }
        let elapsed = h.started.elapsed();
        match h.stage {
            Stage::Shell => {
                // A line editor that offers bracketed paste is ready; plain shells get 2 s.
                let ready = bracketed || (h.still >= 2 && elapsed.as_secs() >= 2);
                if ready {
                    let command = h.command.clone().unwrap_or_default();
                    h.stage = if h.text.is_some() {
                        Stage::Agent
                    } else {
                        Stage::Placed
                    };
                    h.started = Instant::now();
                    h.still = 0;
                    let mut bytes = command.into_bytes();
                    bytes.push(b'\r');
                    self.input(bytes, cx);
                    if self
                        .handoff
                        .as_ref()
                        .is_some_and(|h| h.stage == Stage::Placed)
                    {
                        // A resume has no text to place; the bar has nothing more to say.
                        self.handoff = None;
                    }
                } else if elapsed.as_secs() >= HANDOFF_LIMIT {
                    h.stage = Stage::NotReady;
                }
            }
            Stage::Agent => {
                // The shell drops bracketed paste when it runs the command; the Agent turns
                // it back on even for onboarding pages. Require a visible input prompt too;
                // a menu or a security notice is not ready to receive task text.
                if !bracketed {
                    h.released = true;
                }
                // Back at the shell prompt, bracketed paste returns too: without this
                // check the task text would land on the shell's command line.
                if command_missing(&h.tail) {
                    h.stage = Stage::Missing;
                } else if h.released && bracketed && h.still >= 2 && agent_input_ready(&h.tail) {
                    let text = h.text.clone().unwrap_or_default();
                    h.stage = Stage::Placed;
                    self.prefill(&text, cx);
                } else if elapsed.as_secs() >= HANDOFF_LIMIT {
                    h.stage = Stage::NotReady;
                }
            }
            Stage::Placed | Stage::NotReady | Stage::Missing => (),
        }
        cx.notify();
    }
    pub fn prefill(&mut self, command: &str, cx: &mut Context<Self>) {
        let bracketed = self.bracketed();
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
            self.paste(cx);
            cx.stop_propagation();
            return;
        }
        // The usual terminal shortcuts; Cmd+other keys still go to the program.
        if k.modifiers.platform && !k.modifiers.control && !k.modifiers.alt {
            let zoom = match k.key.as_str() {
                "=" | "+" => Some(Some(1)),
                "-" => Some(Some(-1)),
                "0" => Some(None),
                _ => None,
            };
            let handled = if let Some(step) = zoom {
                screen::zoom(step);
                cx.refresh_windows();
                true
            } else {
                match k.key.as_str() {
                    "a" => self.select_all(cx),
                    "f" => self.open_search(window, cx),
                    "k" => {
                        self.text_selection = None;
                        if let Err(e) = self.connection.send(Request::ClearHistory) {
                            self.local_error = Some(e.to_string());
                        }
                        cx.notify();
                    }
                    _ => (),
                }
                matches!(k.key.as_str(), "a" | "f" | "k")
            };
            if handled {
                cx.stop_propagation();
                return;
            }
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focus_observers {
            self.focus_observers = true;
            cx.on_focus(&self.focus, window, Self::sync_focus).detach();
            cx.on_blur(&self.focus, window, Self::sync_focus).detach();
            cx.observe_window_activation(window, Self::sync_focus)
                .detach();
        }
        self.sync_focus(window, cx);
        self.connection.dirty.store(false, Ordering::Release);
        let state = self.connection.state.lock().unwrap();
        if let Some(vt) = &state.vt {
            let result = (|| {
                if self.capture.is_none() {
                    self.capture = Some(screen::Capture::new()?);
                }
                self.capture.as_mut().unwrap().update(vt, &mut self.screen)
            })();
            match result {
                Ok(_) => {
                    self.cursor_anchor.observe(self.screen.cursor);
                }
                Err(e) => self.local_error = Some(e.to_string()),
            }
        }
        let error = self.local_error.clone().or(state.error.clone());
        let status = if state.ended {
            "进程已退出 · 待核对"
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
                    // Font metrics are logical pixels, independent of backing scale.
                    let key = (font.clone(), screen::font_size());
                    if pane.metrics_font.as_ref() != Some(&key) || pane.metrics.is_none() {
                        pane.metrics = screen::CellMetrics::measure(&font, window);
                        pane.metrics_font = Some(key);
                    }
                    let Some(metrics) = pane.metrics else { return };
                    screen::paint_cached(
                        &pane.screen,
                        &font,
                        bounds,
                        metrics,
                        pane.marked.is_empty(),
                        &mut pane.layout_cache,
                        window,
                        cx,
                    );
                    if let Some((y, x)) = pane.hover_link
                        && let Some(hover) = pane.screen.link_at(y, x)
                    {
                        for l in pane.screen.links.iter().filter(|l| l.uri == hover.uri) {
                            window.paint_quad(gpui::fill(
                                Bounds::new(
                                    gpui::point(
                                        bounds.left() + metrics.width * l.col as f32,
                                        bounds.top() + metrics.height * (l.row + 1) as f32
                                            - metrics.stroke,
                                    ),
                                    gpui::size(metrics.width * l.cols as f32, metrics.stroke),
                                ),
                                screen::hsla(screen::DEFAULT_FG),
                            ));
                        }
                    }
                    if let Some((start, end)) =
                        pane.text_selection.and_then(TextSelection::bounds)
                    {
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
                                T::color(T::FOCUS).opacity(0.25),
                            ));
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
                    let scale = window.scale_factor();
                    let cell = (
                        (f32::from(metrics.width) * scale).round().max(1.) as u32,
                        (f32::from(metrics.height) * scale).round().max(1.) as u32,
                    );
                    if (pane.requested_size, pane.requested_cell) != (dims, cell)
                        && pane
                            .connection
                            .send(Request::Resize {
                                cols: dims.0,
                                rows: dims.1,
                                cell_width: cell.0,
                                cell_height: cell.1,
                            })
                            .is_ok()
                    {
                        pane.requested_size = dims;
                        pane.requested_cell = cell;
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
            .on_mouse_down(gpui::MouseButton::Left,cx.listener(Self::mouse_down))
            .on_mouse_down(gpui::MouseButton::Middle,cx.listener(Self::mouse_down))
            .on_mouse_down(gpui::MouseButton::Right,cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(gpui::MouseButton::Left,cx.listener(Self::mouse_up))
            .on_mouse_up(gpui::MouseButton::Middle,cx.listener(Self::mouse_up))
            .on_mouse_up(gpui::MouseButton::Right,cx.listener(Self::mouse_up))
            .on_mouse_up_out(gpui::MouseButton::Left,cx.listener(Self::mouse_up))
            .on_mouse_up_out(gpui::MouseButton::Middle,cx.listener(Self::mouse_up))
            .on_mouse_up_out(gpui::MouseButton::Right,cx.listener(Self::mouse_up))
            .on_scroll_wheel(cx.listener(Self::scroll_wheel))
            .on_key_down(cx.listener(Self::key))
            .on_drop(cx.listener(|this, paths: &gpui::ExternalPaths, window, cx| {
                window.focus(&this.focus, cx);
                this.drop_paths(paths, cx);
            }))
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
            .when_some(self.paste_confirm.as_deref().map(line_count),|d,lines| {
                d.child(div().px(px(T::S12)).py(px(T::S8)).flex().items_center().gap(px(T::S12))
                    .bg(T::color(T::PANEL)).text_size(px(T::META))
                    .child(div().flex_1().text_color(T::color(T::SECONDARY))
                        .child(format!("这个程序没有启用安全粘贴：要粘贴的 {lines} 行会逐行立即执行。")))
                    .child(div().id("paste-anyway").cursor_pointer().text_color(T::color(T::LINK)).child(format!("仍然粘贴 {lines} 行"))
                        .on_click(cx.listener(|this,_,_,cx|this.confirm_paste(cx))))
                    .child(div().id("paste-cancel").cursor_pointer().child("取消")
                        .on_click(cx.listener(|this,_,_,cx|{this.paste_confirm=None;cx.notify();}))))
            })
            .when_some(self.search.clone(),|d,field| {
                let mut panel=div().p(px(T::S8)).flex().flex_col().gap(px(T::S4)).child(field)
                    .child(div().flex().gap(px(T::S8))
                        .child(div().id("find").cursor_pointer().child("查找").on_click(cx.listener(|this,_,_,cx|this.find_text(cx))))
                        .child(div().id("close-find").cursor_pointer().child("关闭搜索").on_click(cx.listener(|this,_,window,cx|{this.search=None;this.matches.clear();window.focus(&this.focus,cx);cx.notify();}))))
                    .child(format!("{} 条匹配（显示前 20 条）",self.matches.len()));
                for (row,line) in self.matches.iter().take(20){let row=*row;panel=panel.child(div().id(gpui::SharedString::from(format!("match-{row}"))).truncate().cursor_pointer().child(line.clone())
                    .on_click(cx.listener(move|this,_,_,cx|{if let Ok(mut s)=this.connection.state.lock()&& let Some(vt)=&mut s.vt{vt.scroll(keepline_core::terminal::libghostty_vt::terminal::ScrollViewport::Row(row));}this.search=None;cx.notify();})));}
                d.child(panel.id("search-results").max_h(px(T::SEARCH_HEIGHT)).overflow_y_scroll())
            })
            .child(div().flex_1().min_h_0().child(content))
            .when(self.flash, |d| {
                d.relative().child(
                    div()
                        .absolute()
                        .inset_0()
                        .border(px(T::S2))
                        .border_color(T::color(T::ACCENT)),
                )
            });
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

const HANDOFF_LIMIT: u64 = 60;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Shell,
    Agent,
    Placed,
    NotReady,
    /// The shell could not find the Agent command; nothing was placed.
    Missing,
}
struct Handoff {
    command: Option<String>,
    text: Option<String>,
    stage: Stage,
    started: Instant,
    released: bool,
    tail: String,
    still: u8,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HandoffState {
    Waiting,
    Placed,
    NotReady,
    Missing,
}
/// Both supported CLIs mark their input with an arrow. Bracketed paste alone
/// is not readiness: Claude also enables it on theme and security notice pages.
/// Use the last marker so an old menu above a new input does not block placement.
fn agent_input_ready(tail: &str) -> bool {
    let mut menu_footer = false;
    let selected = tail.lines().rev().find_map(|l| {
        let lower = l.to_ascii_lowercase();
        menu_footer |= lower.contains("enter to confirm")
            || lower.contains("enter to select")
            || lower.contains("enter to continue");
        let l = l.trim_start_matches(['│', ' ']);
        ["❯", "›", ">"].iter().find_map(|m| {
            l.strip_prefix(m)
                .filter(|s| s.is_empty() || s.starts_with(char::is_whitespace))
                .map(str::trim_start)
        })
    });
    // Claude 2.1.302 also uses unnumbered trust choices. Inspect only below
    // the current marker so a previous menu cannot poison a later input box.
    if menu_footer {
        return false;
    }
    let Some(selected) = selected else {
        return false;
    };
    if selected.is_empty() {
        return true;
    }
    let digits = selected.bytes().take_while(u8::is_ascii_digit).count();
    !(tail.contains("Choose the text style") || digits > 0 && selected[digits..].starts_with('.'))
}
/// zsh, bash and fish wording for a command that is not on PATH, near the prompt.
fn command_missing(tail: &str) -> bool {
    tail.lines()
        .rev()
        .filter(|l| !l.trim().is_empty())
        .take(4)
        .any(|l| l.contains("command not found") || l.contains("Unknown command"))
}
fn line_count(text: &str) -> usize {
    text.trim_end_matches(['\r', '\n']).lines().count()
}
/// What a terminal sends for a paste without bracketed paste: newlines become CR.
fn plain_paste(text: &str) -> Vec<u8> {
    text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
}
#[cfg(test)]
mod paste_tests {
    #[test]
    fn plain_multiline_paste_counts_lines_and_sends_cr() {
        assert_eq!(super::line_count("echo 一\necho 二\n"), 2);
        assert_eq!(super::line_count("single line\n"), 1);
        assert_eq!(super::plain_paste("a\r\nb\nc"), b"a\rb\rc");
    }
}
#[cfg(test)]
mod handoff_tests {
    #[test]
    fn onboarding_and_nonfirst_choices_do_not_receive_task_text() {
        // Real Claude 2.1.302 onboarding has no numeric options, and its preview
        // can put the selected row more than twenty lines above the bottom.
        let theme = format!(
            "Welcome to Claude Code\nChoose the text style that looks best with your terminal\n❯ ✔ Dark mode\n  Light mode\n{}",
            "preview row\n".repeat(25)
        );
        assert!(!super::agent_input_ready(&theme));
        assert!(!super::agent_input_ready(
            "Choose the text style that looks best with your terminal\n  ✔ Dark mode\n❯ Light mode"
        ));
        assert!(!super::agent_input_ready(
            "Sign in\n  1. With ChatGPT\n> 2. With Device Code\n  3. API key"
        ));
        assert!(!super::agent_input_ready(
            "Allow this command?\n› 3. No, cancel"
        ));
        assert!(super::agent_input_ready("› 写一个测试\n? for shortcuts"));
        assert!(super::agent_input_ready(&format!(
            "{theme}\n❯ \n? for shortcuts"
        )));
        assert!(super::agent_input_ready(
            "› 2. Cancel\n\n› Ask Codex to do anything"
        ));
    }
    #[test]
    fn bracketed_paste_pages_without_an_input_prompt_are_not_ready() {
        assert!(!super::agent_input_ready(
            "Security notes:\n1. Claude can make mistakes.\n2. Only use code you trust.\nPress Enter to continue…"
        ));
        assert!(!super::agent_input_ready(
            "Welcome to Codex\n>_ OpenAI Codex\nLoading…"
        ));
        assert!(!super::agent_input_ready("project% "));
        assert!(super::agent_input_ready("❯ \n? for shortcuts"));
        assert!(!super::agent_input_ready(
            "Accessing workspace:\n❯ No, exit\n  Yes, I trust this folder\nEnter to confirm · Esc to cancel"
        ));
        assert!(!super::agent_input_ready(
            "Accessing workspace:\n  No, exit\n❯ Yes, I trust this folder\nEnter to confirm · Esc to cancel"
        ));
        assert!(super::agent_input_ready(
            "❯ No, exit\nEnter to confirm · Esc to cancel\n❯ \n? for shortcuts"
        ));
    }
    #[test]
    fn a_missing_agent_command_is_reported_not_waited_on() {
        assert!(super::command_missing(
            "Agent-OS% codex\nzsh: command not found: codex\nAgent-OS% "
        ));
        assert!(super::command_missing(
            "$ claude\nbash: claude: command not found\n$ "
        ));
        assert!(!super::command_missing("› 写一个测试\n\n  ? for shortcuts"));
    }
    #[test]
    fn menus_block_placement_but_the_input_box_does_not() {
        assert!(!super::agent_input_ready(
            "Do you trust the files in this folder?\n\n❯ 1. Yes, proceed\n  2. No, exit\n"
        ));
        assert!(!super::agent_input_ready(
            "│ › 1. Yes, allow Codex to work in this folder\n"
        ));
        assert!(super::agent_input_ready(
            "› Ask Codex to do anything\n\n  ? for shortcuts\n"
        ));
        assert!(super::agent_input_ready(
            "> \n  1. plan written by the agent\n"
        ));
    }
}
