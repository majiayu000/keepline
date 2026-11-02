use crate::{
    connection::Connection,
    terminal::screen::{self, Screen},
    ui::{theme::Theme as T, tone::Tone},
};
use futures::{StreamExt, channel::mpsc::unbounded};
use gpui::{
    App, Bounds, Context, ElementInputHandler, EntityInputHandler, FocusHandle, Focusable,
    KeyDownEvent, Pixels, Point, UTF16Selection, Window, canvas, div, prelude::*, px,
};
use keepline_core::terminal::{agent_input_ready, command_missing};
use keepline_core::{
    protocol::{Reply, Request, rpc},
    terminal::{
        Vt,
        libghostty_vt::{
            key::Mods,
            mouse,
            selection::gesture::Autoscroll,
            terminal::{Mode, ScrollViewport},
        },
    },
};
use std::{ops::Range, path::Path, sync::atomic::Ordering, time::Instant};

/// The strip along a pane's right edge that grabs the scrollbar, and the thumb's width.
const SCROLLBAR_HIT: f32 = 10.;
const SCROLLBAR_WIDTH: f32 = 6.;
/// Wheel reports sent to a program for one scroll event.
const WHEEL_REPORTS: usize = 10;
/// How often output may re-ask the host for the program's directory.
const CWD_REFRESH_SECS: u64 = 5;
/// The floating ask stays up this long after the last scroll through history.
const ASK_FLOAT_MS: u64 = 1500;
/// Output re-reads where the asks are at most this often; any other repaint, every
/// few seconds (history may have dropped its oldest rows, moving the rest up).
const ASK_LOCATE_MS: u64 = 500;
const ASK_RELOCATE_MS: u64 = 3000;
/// A shown cell with text: where its middle is in the window, its color and its width.
pub type Glyph = (gpui::Point<Pixels>, u32, u16);

pub struct Pane {
    terminal: String,
    socket: std::path::PathBuf,
    /// Wakes this pane on terminal output; every connection it opens shares it.
    wake: futures::channel::mpsc::UnboundedSender<()>,
    connection: Connection,
    /// A dropped stream is being reattached; the delay doubles while it keeps failing.
    reconnecting: bool,
    reconnect_delay: u64,
    focus: FocusHandle,
    screen: Screen,
    capture: Option<screen::Capture>,
    layout_cache: screen::LayoutCache,
    metrics_font: Option<(gpui::Font, f32, f32)>,
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
    /// The selection gesture; the selection itself lives on the terminal, so it stays
    /// on its text as output or the scrollbar moves it.
    selecting: Option<crate::terminal::selection::Selecting>,
    /// Backing scale of the last paint, for drags reported outside the pane.
    scale: f32,
    mouse: Option<crate::terminal::terminal_input::MouseEncoder>,
    pressed_button: Option<gpui::MouseButton>,
    scroll_lines: f32,
    scroll_cols: f32,
    /// The viewport is in history; a chip offers the way back to the live screen.
    scrolled_back: bool,
    /// The viewport's first row, the scrollable rows and the shown rows, read each frame;
    /// rows count from the top of history, as search matches do.
    viewport: (usize, usize, usize),
    /// The scrollbar thumb is being dragged.
    dragging_scrollbar: bool,
    hover_link: Option<(u16, u16)>,
    /// The path word under the pointer, and where it points when that exists on disk.
    hover_word: Option<screen::Link>,
    hover_path: Option<Hovered>,
    /// The modifiers last seen, so links are underlined only while ⌘ is held, and where
    /// the pointer is, for the note saying what a click would do.
    modifiers: gpui::Modifiers,
    pointer: Option<Point<Pixels>>,
    /// The program's working directory, asked from the host when a relative path is hovered.
    cwd: Option<std::path::PathBuf>,
    /// When the footer last asked the host for that directory; output re-asks at most
    /// every few seconds, and an idle pane asks nothing.
    cwd_asked: Option<Instant>,
    /// An ask held back by the throttle is still to come.
    cwd_due: bool,
    focus_observers: bool,
    last_focus: bool,
    /// The focused cursor's blink phase; a keystroke shows it and restarts the count.
    blink_on: bool,
    /// Bumped to retire the running blink timer.
    blink_epoch: u64,
    /// When the user last sent input; its echo is drawn without waiting for the shared tick.
    typed: Option<Instant>,
    search: Option<gpui::Entity<crate::ui::field::Field>>,
    matches: Vec<keepline_core::terminal::SearchMatch>,
    /// The query `matches` came from; Enter on an edited query searches again.
    searched: Option<String>,
    _search_events: Option<gpui::Subscription>,
    /// The match the view moved to; ↑/↓ and Enter step through them.
    current_match: Option<usize>,
    /// A multi-line paste waiting for the user's go-ahead (no bracketed paste).
    paste_confirm: Option<String>,
    bells_seen: u64,
    /// The program rang while the user looked elsewhere; the header shows it until focus.
    pub bell: bool,
    /// A bell in the pane the user is looking at flashes its border once.
    flash: bool,
    /// A short note in the corner that clears itself, e.g. how to select when the
    /// program took the mouse.
    toast: Option<gpui::SharedString>,
    toast_seq: u64,
    /// The Agent running in this terminal, as the workbench last saw it.
    pub agent: Option<keepline_core::model::Agent>,
    /// Shares the tab with other panes: unless it holds focus, it fades and draws no
    /// cursor, so only one cursor is in sight.
    pub split: bool,
    /// Shown in the peek overlay and only looked at: it does not resize the terminal
    /// (that would reflow the Agent's screen for its own tab) and keeps the bottom rows
    /// in view when taller. The first key, paste or click in it clears this, so the
    /// overlay takes the size: the client used last decides the size.
    pub peek: bool,
    /// The scheme this pane first painted its live program with. Agents such as Codex
    /// tint their own bands from the background they saw at start, which a later switch
    /// cannot repaint, so the pane says so until dismissed.
    started_under: Option<keepline_core::terminal::Scheme>,
    tint_note_dismissed: bool,
    /// What this terminal's conversation was asked, oldest first, as the workbench last
    /// saw it, and where each sits in history once found.
    asks: Vec<String>,
    ask_rows: Vec<Option<usize>>,
    /// When the asks were last looked for, and the history's length then.
    asks_located: Option<(Instant, usize)>,
    locating_asks: bool,
    pinned: keepline_core::prompt_pin::Pinned,
    /// When the viewport last moved through history, for the floating ask.
    scrolled_at: Option<Instant>,
}
/// A hovered path word that exists on disk.
#[derive(Clone, Debug, PartialEq)]
pub struct Hovered {
    pub path: std::path::PathBuf,
    /// The line and column the word named, as in `pane.rs:651:7`.
    pub at: Option<(u32, Option<u32>)>,
    /// The whole word named it, not one piece of a word broken over rows, so every
    /// piece is underlined.
    pub whole: bool,
    /// The program's directory when hovered, for copying the path relative to it.
    pub cwd: Option<std::path::PathBuf>,
}
/// What the pane asks of the workbench around it.
pub enum PaneEvent {
    /// Run `command` in a new terminal tab started in `cwd`.
    Run { command: String, cwd: String },
    /// The turn's ask scrolled out of sight or back, for the header's 「↑ 你问」.
    PinnedAsk,
}
impl gpui::EventEmitter<PaneEvent> for Pane {}
impl Focusable for Pane {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Pane {
    #[cfg(feature = "visual-test")]
    pub fn visual_type(&mut self, text: &str, cx: &mut Context<Self>) {
        self.input(text.as_bytes().to_vec(), cx);
    }
    /// The visible screen as text, for checking what reached the program.
    #[cfg(feature = "visual-test")]
    pub fn visual_text(&self) -> String {
        let state = self.connection.state.lock().unwrap();
        state
            .vt
            .as_ref()
            .and_then(|vt| {
                vt.tail_text(usize::from(self.requested_size.1.max(40)))
                    .ok()
            })
            .unwrap_or_default()
    }
    /// The rows the viewport shows now, scrolled back or not.
    #[cfg(feature = "visual-test")]
    pub fn visual_text_shown(&self) -> String {
        let rows: Vec<String> = self
            .screen
            .rows
            .iter()
            .map(|row| row.iter().map(|r| r.text.as_str()).collect::<String>())
            .collect();
        rows.join("\n")
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_bounds(&self) -> Option<Bounds<Pixels>> {
        self.bounds
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_viewport(&self) -> (usize, usize, usize) {
        self.viewport
    }
    /// Where each ask was found, and what is pinned: the newest ask's text and whether
    /// it scrolls back, and the text of the ask whose answer is being read.
    #[cfg(feature = "visual-test")]
    pub fn visual_pinned_ask(
        &self,
    ) -> (Vec<Option<usize>>, Option<(String, bool)>, Option<String>) {
        (
            self.ask_rows.clone(),
            self.pinned_latest(),
            self.pinned
                .viewed
                .and_then(|(i, _)| self.asks.get(i).cloned()),
        )
    }
    /// Scrolls history so `row` is the top row, as the scrollbar would.
    #[cfg(feature = "visual-test")]
    pub fn visual_scroll_row(&mut self, row: usize, cx: &mut Context<Self>) {
        self.scroll_to(ScrollViewport::Row(row), cx);
    }
    /// Opens search with `query` and runs it, as typing it and pressing Enter does.
    #[cfg(feature = "visual-test")]
    pub fn visual_search(&mut self, query: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.open_search(window, cx);
        let field = self.search.clone().unwrap();
        field.update(cx, |f, cx| {
            f.text = query.into();
            cx.notify();
        });
        self.find_text(cx);
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_matches(&self) -> (Option<usize>, Vec<usize>) {
        (
            self.current_match,
            self.matches.iter().map(|m| m.row).collect(),
        )
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_step_match(&mut self, step: isize, cx: &mut Context<Self>) {
        self.step_match(step, cx);
    }
    /// The window point at the middle of cell (`row`, `col`).
    #[cfg(feature = "visual-test")]
    pub fn visual_cell(&self, row: u16, col: u16) -> Option<gpui::Point<Pixels>> {
        let (bounds, metrics) = (self.bounds?, self.metrics?);
        Some(gpui::point(
            bounds.left() + metrics.width * (col as f32 + 0.5),
            bounds.top() + metrics.height * (row as f32 + 0.5),
        ))
    }
    /// The selected columns (inclusive) on each viewport row, as drawn.
    #[cfg(feature = "visual-test")]
    pub fn visual_selection_cols(&self) -> Vec<Option<(u16, u16)>> {
        self.screen.selection.clone()
    }
    /// ⌘= / ⌘- / ⌘0 on the terminal text, as the shortcuts do.
    #[cfg(feature = "visual-test")]
    pub fn visual_zoom(&mut self, step: Option<i8>, cx: &mut Context<Self>) {
        screen::zoom(step).unwrap();
        cx.refresh_windows();
    }
    /// The viewport rows the selection is drawn on, and the text it would copy.
    #[cfg(feature = "visual-test")]
    pub fn visual_selection(&self) -> (Vec<u16>, Option<String>) {
        self.visual_selection_drawn().0
    }
    /// Also the text drawn on those rows.
    #[cfg(feature = "visual-test")]
    pub fn visual_selection_drawn(&self) -> ((Vec<u16>, Option<String>), Vec<String>) {
        let drawn = (self.screen.selection.iter().zip(&self.screen.rows))
            .filter(|(s, _)| s.is_some())
            .map(|(_, row)| {
                row.iter()
                    .map(|r| r.text.as_str())
                    .collect::<String>()
                    .trim()
                    .to_owned()
            })
            .collect();
        let rows = (self.screen.selection.iter().enumerate())
            .filter(|(_, s)| s.is_some())
            .map(|(y, _)| y as u16)
            .collect();
        let text = self.selected_text();
        ((rows, text), drawn)
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_error(&self) -> Option<String> {
        self.local_error
            .clone()
            .or_else(|| self.connection.state.lock().ok()?.error.clone())
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_select_hint(&mut self, cx: &mut Context<Self>) {
        self.show_toast(SELECT_HINT, cx);
    }
    /// Where the link whose target starts with `prefix` is drawn, and its cell count.
    #[cfg(feature = "visual-test")]
    pub fn visual_link(&self, prefix: &str) -> Option<(gpui::Point<Pixels>, u16)> {
        let (bounds, metrics) = (self.bounds?, self.metrics?);
        let link = self
            .screen
            .links
            .iter()
            .find(|l| l.uri.starts_with(prefix))?;
        Some((
            gpui::point(
                bounds.left() + metrics.width * (link.col as f32 + 0.5),
                bounds.top() + metrics.height * (link.row as f32 + 0.5),
            ),
            link.cols,
        ))
    }
    /// Whether the viewport shows the live screen rather than scrollback.
    #[cfg(feature = "visual-test")]
    pub fn visual_at_bottom(&self) -> bool {
        let state = self.connection.state.lock().unwrap();
        state
            .vt
            .as_ref()
            .is_some_and(|v| v.terminal().viewport_active().unwrap_or(false))
    }
    /// Where the path word `word` is drawn, and the file it points to once hovered.
    #[cfg(feature = "visual-test")]
    pub fn visual_path(
        &self,
        word: &str,
    ) -> Option<(gpui::Point<Pixels>, Option<std::path::PathBuf>)> {
        let (bounds, metrics) = (self.bounds?, self.metrics?);
        // The last piece, so a path broken over rows is hovered where it ends.
        let link = self.screen.paths.iter().rfind(|l| l.uri == word)?;
        let hovered = (self.hover_word.as_ref() == Some(link))
            .then(|| self.shown_path().cloned())
            .flatten();
        Some((
            gpui::point(
                bounds.left() + metrics.width * (link.col as f32 + 0.5),
                bounds.top() + metrics.height * (link.row as f32 + 0.5),
            ),
            hovered,
        ))
    }
    /// The note shown beside the pointer for a click on what it is over.
    #[cfg(feature = "visual-test")]
    pub fn visual_hint(&self) -> Option<String> {
        self.click_hint()
    }
    /// The program reads the mouse itself, so a plain drag goes to it, not to selection.
    pub fn takes_mouse(&self) -> bool {
        self.connection.state.lock().is_ok_and(|s| {
            s.vt.as_ref()
                .is_some_and(|v| v.terminal().is_mouse_tracking().unwrap_or(false))
        })
    }
    /// Whether the program in this pane asked for mouse reports, and where to click it.
    #[cfg(feature = "visual-test")]
    pub fn visual_mouse_app(&self) -> (bool, Option<gpui::Point<Pixels>>) {
        let state = self.connection.state.lock().unwrap();
        let tracking = state
            .vt
            .as_ref()
            .is_some_and(|v| v.terminal().is_mouse_tracking().unwrap_or(false));
        (tracking, self.bounds.map(|b| b.center()))
    }
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
        let connection = Connection::new(socket, id, takeover, tx.clone());
        cx.spawn(async move |this, cx| {
            while rx.next().await.is_some() {
                // Coalesce bursts at 60 Hz; no timer exists when the terminal is idle.
                // Output nobody is typing into waits for the shared tick instead, so
                // several busy panes repaint in one frame rather than one frame each.
                let echo = this
                    .read_with(cx, |pane, _| {
                        pane.typed
                            .is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(300))
                    })
                    .unwrap_or(false);
                cx.background_executor()
                    .timer(if echo {
                        std::time::Duration::from_millis(16)
                    } else {
                        until_shared_tick()
                    })
                    .await;
                while rx.try_recv().is_ok() {}
                if this
                    .update(cx, |pane, cx| {
                        pane.ring(cx);
                        pane.recover(cx);
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
            socket: socket.to_owned(),
            wake: tx,
            connection,
            reconnecting: false,
            reconnect_delay: 0,
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
            selecting: crate::terminal::selection::Selecting::new().ok(),
            scale: 1.,
            mouse: None,
            pressed_button: None,
            scroll_lines: 0.,
            scroll_cols: 0.,
            scrolled_back: false,
            viewport: (0, 0, 0),
            dragging_scrollbar: false,
            hover_link: None,
            hover_word: None,
            hover_path: None,
            modifiers: gpui::Modifiers::default(),
            pointer: None,
            cwd: None,
            cwd_asked: None,
            cwd_due: false,
            focus_observers: false,
            last_focus: false,
            blink_on: true,
            blink_epoch: 0,
            typed: None,
            search: None,
            matches: vec![],
            searched: None,
            _search_events: None,
            current_match: None,
            paste_confirm: None,
            bells_seen: 0,
            bell: false,
            flash: false,
            toast: None,
            toast_seq: 0,
            agent: None,
            split: false,
            peek: false,
            started_under: None,
            tint_note_dismissed: false,
            asks: Vec::new(),
            ask_rows: Vec::new(),
            asks_located: None,
            locating_asks: false,
            pinned: Default::default(),
            scrolled_at: None,
        }
    }
    /// Attaches again after the host dropped this pane's stream; the snapshot restores
    /// the screen, and input works again once the pane owns the terminal.
    fn recover(&mut self, cx: &mut Context<Self>) {
        let (dropped, failed, live) =
            self.connection
                .state
                .lock()
                .map_or((false, false, false), |s| {
                    (
                        s.dropped,
                        s.error.is_some() && !s.ended,
                        s.vt.is_some() && s.error.is_none(),
                    )
                });
        if live {
            self.reconnect_delay = 0;
        }
        // Once dropped, every failed attempt retries until one restores the screen: an
        // attempt made during the same burst is dropped again before its snapshot lands.
        let dropped = dropped || (self.reconnect_delay > 0 && failed);
        if !dropped || self.reconnecting {
            return;
        }
        self.reconnecting = true;
        let delay = self.reconnect_delay.max(100);
        self.reconnect_delay = (delay * 2).min(5000);
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(delay))
                .await;
            let _ = this.update(cx, |pane, cx| {
                pane.connection =
                    Connection::new(&pane.socket, &pane.terminal, false, pane.wake.clone());
                pane.reconnecting = false;
                pane.bells_seen = 0;
                pane.requested_cell = (0, 0);
                pane.local_error = None;
                cx.notify();
            });
        })
        .detach();
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
    /// The window point at the middle of each shown cell with text, with its color and
    /// width in cells, row by row; and the cell size. For 标记完成's
    /// dissolve, which turns the text into specks.
    pub fn glyph_cells(&self) -> (Vec<Glyph>, gpui::Size<Pixels>) {
        let (Some(bounds), Some(m)) = (self.bounds, self.metrics) else {
            return (vec![], gpui::size(px(0.), px(0.)));
        };
        let mut cells = Vec::new();
        for (row, line) in self.screen.cells.iter().enumerate() {
            let runs = self.screen.rows.get(row).map(Vec::as_slice).unwrap_or(&[]);
            for &(col, width, ch) in line {
                if ch.is_whitespace() || ch == '\0' {
                    continue;
                }
                let fg = runs
                    .iter()
                    .find(|r| col >= r.col && col < r.col + r.cols)
                    .map_or(self.screen.background ^ T::INVERT_RGB, |r| r.fg);
                let x = bounds.left() + m.width * (col as f32 + width.max(1) as f32 / 2.);
                let y = bounds.top() + m.height * (row as f32 + 0.5);
                cells.push((gpui::point(x, y), fg, width.max(1)));
            }
        }
        (cells, gpui::size(m.width, m.height))
    }
    pub fn program_title(&self) -> Option<String> {
        self.connection.state.lock().ok()?.vt.as_ref()?.title()
    }
    /// Dropped files arrive as their paths, escaped for a shell, like any terminal.
    fn drop_paths(&mut self, paths: &gpui::ExternalPaths, cx: &mut Context<Self>) {
        let text = crate::terminal::terminal_input::dropped_paths(paths.paths());
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
        self.restart_blink(cx);
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
                .map(|v| crate::terminal::terminal_input::focus_bytes(v, focused))
        };
        match bytes {
            // A focus report is not typing; it leaves the scrollback where it is.
            Some(Ok(b)) if !b.is_empty() => self.send_input(b, false, cx),
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
                self.mouse = Some(crate::terminal::terminal_input::MouseEncoder::new()?);
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
            crate::terminal::terminal_input::mouse_bytes(
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
                    self.pointer(bytes, cx);
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
        self.take_size(cx);
        if event.modifiers.platform
            && event.button == gpui::MouseButton::Left
            && let Some((y, x)) = self.cell_at(event.position)
            && let Some(link) = self.screen.link_at(y, x)
        {
            cx.open_url(&link.uri);
            cx.stop_propagation();
            return;
        }
        // ⌘ opens it as iTerm2 and Ghostty do; ⌘⌥ shows it in Finder instead.
        if event.modifiers.platform
            && event.button == gpui::MouseButton::Left
            && let Some(target) = self.path_target()
        {
            self.open_target(&target, event.modifiers.alt, cx);
            cx.stop_propagation();
            return;
        }
        let button = match event.button {
            gpui::MouseButton::Left => mouse::Button::Left,
            gpui::MouseButton::Middle => mouse::Button::Middle,
            _ => mouse::Button::Right,
        };
        self.pressed_button = Some(event.button);
        // The thumb is Keepline's own, even over a program that took the mouse.
        if event.button == gpui::MouseButton::Left && self.on_scrollbar(event.position) {
            self.dragging_scrollbar = true;
            self.scroll_to_pointer(event.position, cx);
            cx.stop_propagation();
            return;
        }
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
        if event.button == gpui::MouseButton::Left
            && let Some(at) = self.local(event.position)
        {
            // Ghostty's gesture counts clicks itself: cell, word, then line.
            self.scale = window.scale_factor();
            let state = self.connection.state.clone();
            if let Ok(state) = state.lock()
                && let Some(vt) = &state.vt
                && let Some(grid) = grid(vt, self.metrics, self.scale)
                && let Some(sel) = &mut self.selecting
            {
                let _ = sel.press(vt.terminal(), grid, at);
            }
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
        if std::mem::take(&mut self.dragging_scrollbar) {
            return;
        }
        let button = match event.button {
            gpui::MouseButton::Left => mouse::Button::Left,
            gpui::MouseButton::Middle => mouse::Button::Middle,
            _ => mouse::Button::Right,
        };
        let reported = self.report_mouse(
            mouse::Action::Release,
            Some(button),
            event.position,
            event.modifiers,
            window.scale_factor(),
            cx,
        );
        if !reported
            && event.button == gpui::MouseButton::Left
            && self.selecting.as_ref().is_some_and(|s| s.active)
        {
            let at = self.local(event.position).unwrap_or_default();
            let state = self.connection.state.clone();
            if let Ok(state) = state.lock()
                && let Some(vt) = &state.vt
                && let Some(grid) = grid(vt, self.metrics, self.scale)
                && let Some(sel) = &mut self.selecting
            {
                sel.release(vt.terminal(), grid, at);
            }
            // Copy on select, as iTerm2 and Ghostty do; a plain click selects nothing.
            if self.selected_text().is_some() {
                self.copy(cx);
            }
            cx.notify();
        }
    }
    fn mouse_move(
        &mut self,
        event: &gpui::MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pointer = Some(event.position);
        self.set_modifiers(event.modifiers, cx);
        let hover = self
            .cell_at(event.position)
            .filter(|(y, x)| self.screen.link_at(*y, *x).is_some());
        if hover != self.hover_link {
            self.hover_link = hover;
            cx.notify();
        }
        let word = self
            .cell_at(event.position)
            .filter(|_| hover.is_none())
            .and_then(|(y, x)| self.screen.path_at(y, x).cloned());
        self.hover_word(word, cx);
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
        ) && button == Some(mouse::Button::Left)
            && self.toast.as_deref() != Some(SELECT_HINT)
        {
            self.show_toast(SELECT_HINT, cx);
        }
    }
    /// Window-wide, so a selection drag keeps extending after it leaves the pane.
    fn drag_selection(&mut self, event: &gpui::MouseMoveEvent, cx: &mut Context<Self>) {
        if self.dragging_scrollbar {
            if event.pressed_button == Some(gpui::MouseButton::Left) {
                self.scroll_to_pointer(event.position, cx);
            }
            return;
        }
        if event.pressed_button != Some(gpui::MouseButton::Left)
            || !self.selecting.as_ref().is_some_and(|s| s.active)
        {
            return;
        }
        let Some(at) = self.local(event.position) else {
            return;
        };
        let state = self.connection.state.clone();
        let Ok(state) = state.lock() else { return };
        let (Some(vt), Some(sel)) = (&state.vt, &mut self.selecting) else {
            return;
        };
        let Some(grid) = grid(vt, self.metrics, self.scale) else {
            return;
        };
        let scroll = sel.drag(vt.terminal(), grid, at);
        let start = matches!(scroll, Ok(Autoscroll::Up | Autoscroll::Down)) && !sel.autoscrolling;
        sel.autoscrolling |= start;
        drop(state);
        if start {
            self.autoscroll(cx);
        }
        cx.notify();
    }
    /// While a selection drag sits above or below the pane, scroll and extend it.
    fn autoscroll(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
                let keep = this.update(cx, |this, cx| {
                    let state = this.connection.state.clone();
                    let Ok(mut state) = state.lock() else {
                        return false;
                    };
                    let (Some(vt), Some(sel)) = (&mut state.vt, &mut this.selecting) else {
                        return false;
                    };
                    let Some(grid) = grid(vt, this.metrics, this.scale) else {
                        return false;
                    };
                    let delta = match sel.autoscroll(vt.terminal()) {
                        Ok(Autoscroll::Up) if sel.active => -1,
                        Ok(Autoscroll::Down) if sel.active => 1,
                        _ => {
                            sel.autoscrolling = false;
                            return false;
                        }
                    };
                    vt.scroll(ScrollViewport::Delta(delta));
                    let _ = sel.tick(vt.terminal(), grid);
                    cx.notify();
                    true
                });
                if !matches!(keep, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
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
        if event.touch_phase == gpui::TouchPhase::Started {
            (self.scroll_cols, self.scroll_lines) = (0., 0.);
        }
        // Trackpad pixels add up across events; a wheel notch moves three rows, like Ghostty.
        let (dx, dy) = match event.delta {
            gpui::ScrollDelta::Pixels(p) => (p.x / m.width, p.y / m.height),
            gpui::ScrollDelta::Lines(l) => (l.x * 3., l.y * 3.),
        };
        // Sums of fractions drift just below whole rows (5 x 0.2 = 0.99999994).
        let whole = |v: f32| (v + v.signum() * 1e-3).trunc().clamp(-100., 100.);
        self.scroll_lines += dy;
        self.scroll_cols += dx;
        let lines = whole(self.scroll_lines) as i32;
        let cols = whole(self.scroll_cols) as i32;
        self.scroll_lines -= lines as f32;
        self.scroll_cols -= cols as f32;
        if lines == 0 && cols == 0 {
            return;
        }
        // Sideways scrolling only means something to a program that tracks the mouse.
        for _ in 0..cols.unsigned_abs().min(WHEEL_REPORTS as u32) {
            self.report_mouse(
                mouse::Action::Press,
                Some(if cols > 0 {
                    mouse::Button::Six
                } else {
                    mouse::Button::Seven
                }),
                event.position,
                event.modifiers,
                window.scale_factor(),
                cx,
            );
        }
        // A fling is up to 100 lines; a program that took the mouse gets a few reports,
        // not a queue-filling burst.
        let mut reported = 0;
        for _ in 0..lines.unsigned_abs() {
            if reported == WHEEL_REPORTS {
                break;
            }
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
                reported += 1;
                continue;
            }
            let result = {
                let mut state = self.connection.state.lock().unwrap();
                if let Some(v) = &mut state.vt {
                    match crate::terminal::terminal_input::alternate_scroll(v, lines > 0) {
                        Ok(Some(bytes)) if !event.modifiers.shift => Ok(Some(bytes)),
                        Ok(_) => {
                            v.scroll(ScrollViewport::Delta(if lines > 0 { -1 } else { 1 }));
                            Ok(None)
                        }
                        Err(e) => Err(e),
                    }
                } else {
                    Ok(None)
                }
            };
            match result {
                // Arrow keys for a full-screen program: keep the selection.
                Ok(Some(b)) => self.send_input(b, false, cx),
                Err(e) => self.local_error = Some(e),
                _ => (),
            }
        }
        cx.stop_propagation();
        cx.notify();
    }
    /// The thumb shows while the view is up in history, along the pane's right edge.
    fn scrollbar_shown(&self) -> bool {
        let (top, total, len) = self.viewport;
        total > len && top + len < total
    }
    fn on_scrollbar(&self, point: Point<Pixels>) -> bool {
        self.scrollbar_shown()
            && self
                .bounds
                .is_some_and(|b| b.contains(&point) && point.x >= b.right() - px(SCROLLBAR_HIT))
    }
    /// Centers the view on the history row under the pointer's height in the pane.
    fn scroll_to_pointer(&mut self, point: Point<Pixels>, cx: &mut Context<Self>) {
        let (Some(b), (_, total, len)) = (self.bounds, self.viewport) else {
            return;
        };
        let at = ((point.y - b.top()) / b.size.height).clamp(0., 1.);
        let row = (at * total as f32) as usize;
        if let Ok(mut s) = self.connection.state.lock()
            && let Some(vt) = &mut s.vt
        {
            vt.scroll(ScrollViewport::Row(row.saturating_sub(len / 2)));
            if let Ok(v) = vt.viewport() {
                self.viewport = v;
            }
        }
        cx.notify();
    }
    fn scroll_to(&mut self, to: ScrollViewport, cx: &mut Context<Self>) {
        if let Ok(mut state) = self.connection.state.lock()
            && let Some(vt) = &mut state.vt
        {
            vt.scroll(to);
        }
        cx.notify();
    }
    /// What the conversation in this terminal was asked, oldest first. New asks are
    /// looked for on the next repaint.
    pub fn set_asks(&mut self, asks: Vec<String>, cx: &mut Context<Self>) {
        if self.asks != asks {
            self.asks = asks;
            self.ask_rows.clear();
            self.asks_located = None;
            cx.notify();
        }
    }
    /// How this terminal's CLI draws a sent ask, unless it pins the turn's ask itself.
    fn prompt_line(&self) -> Option<&'static keepline_core::agent::PromptLine> {
        let prompt = &self.agent?.spec()?.prompt;
        (!prompt.pins_own).then_some(prompt)
    }
    /// The newest ask while its row is above the viewport, and whether it can be
    /// scrolled back to.
    pub fn pinned_latest(&self) -> Option<(String, bool)> {
        let (i, row) = self.pinned.latest?;
        Some((self.asks.get(i)?.clone(), row.is_some()))
    }
    /// Shows the newest ask's row at the top.
    pub fn back_to_latest_ask(&mut self, cx: &mut Context<Self>) {
        if let Some((_, Some(row))) = self.pinned.latest {
            self.back_to_ask(row, cx);
        }
    }
    fn back_to_ask(&mut self, row: usize, cx: &mut Context<Self>) {
        self.scroll_to(ScrollViewport::Row(row), cx);
    }
    /// Works out what to pin from where the asks were last found, and looks for them
    /// again once output may have moved them. Runs on each repaint.
    fn refresh_pinned(&mut self, cx: &mut Context<Self>) {
        let choice = crate::ui::pinned_ask::current(cx);
        let was = self.pinned.latest.is_some();
        let prompt = self.prompt_line().filter(|_| choice.any());
        match prompt {
            Some(prompt) if !self.asks.is_empty() => {
                self.pinned = if self.ask_rows.len() == self.asks.len() {
                    keepline_core::prompt_pin::pinned(&self.ask_rows, self.viewport)
                } else {
                    Default::default()
                };
                let due = match self.asks_located {
                    None => true,
                    Some((at, total)) => {
                        let ms = at.elapsed().as_millis() as u64;
                        (total != self.viewport.1 && ms >= ASK_LOCATE_MS) || ms >= ASK_RELOCATE_MS
                    }
                };
                if due && !self.locating_asks {
                    self.locate_asks(prompt, cx);
                }
            }
            _ => self.pinned = Default::default(),
        }
        if was != self.pinned.latest.is_some() {
            cx.emit(PaneEvent::PinnedAsk);
        }
    }
    fn locate_asks(
        &mut self,
        prompt: &'static keepline_core::agent::PromptLine,
        cx: &mut Context<Self>,
    ) {
        self.locating_asks = true;
        let asks = self.asks.clone();
        let keys: Vec<_> = asks
            .iter()
            .map(|a| keepline_core::prompt_pin::key(a))
            .collect();
        let state = self.connection.state.clone();
        let job = cx.background_spawn(async move {
            let state = state.lock().ok()?;
            let vt = state.vt.as_ref()?;
            let total = vt.viewport().ok()?.1;
            Some((vt.find_prompts(&keys, prompt).ok()?, total))
        });
        cx.spawn(async move |this, cx| {
            let found = job.await;
            let _ = this.update(cx, |this, cx| {
                this.locating_asks = false;
                // The asks changed while looking: the next repaint looks again.
                if this.asks != asks {
                    return;
                }
                let Some((rows, total)) = found else { return };
                this.asks_located = Some((Instant::now(), total));
                if this.ask_rows != rows {
                    this.ask_rows = rows;
                    cx.notify();
                }
            });
        })
        .detach();
    }
    /// The pinned ask along the terminal's top, and the chip naming the ask whose answer
    /// is being read in history; each as the settings choose.
    fn ask_overlays(&self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let choice = crate::ui::pinned_ask::current(cx);
        let first_line = |i: usize| {
            self.asks
                .get(i)
                .and_then(|a| a.lines().map(str::trim).find(|l| !l.is_empty()))
                .map(|l| keepline_core::content::short(l, 200))
        };
        let mut out = Vec::new();
        let bar = self
            .pinned
            .latest
            .filter(|_| choice.bar)
            .and_then(|(i, row)| Some((first_line(i)?, row)));
        let bar_shown = bar.is_some();
        // Exactly the terminal's first row, marked as the CLI marks a sent ask.
        let row_height = self
            .metrics
            .map_or(px(T::MONO_SIZE * T::LINE_HEIGHT), |m| m.height);
        if let Some((text, row)) = bar {
            let mark = self
                .prompt_line()
                .and_then(|p| p.markers.first())
                .map_or("›".to_owned(), Clone::clone);
            out.push(
                div()
                    .id("pinned-ask")
                    .absolute()
                    .top_0()
                    .left_0()
                    .right(px(SCROLLBAR_HIT))
                    .h(row_height)
                    .pr(px(T::S8))
                    .flex()
                    .items_center()
                    .gap(px(T::S8))
                    .bg(T::color(T::button()))
                    .font(T::mono(cx))
                    .text_size(px(T::MONO_SIZE))
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_color(T::color(T::muted()))
                            .child(mark),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(T::color(T::primary()))
                            .child(text),
                    )
                    .when_some(row, |d, row| {
                        d.cursor_pointer()
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_size(px(T::META))
                                    .text_color(T::color(T::muted()))
                                    .child("回到这句 ↑"),
                            )
                            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                                cx.stop_propagation()
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.back_to_ask(row, cx);
                                window.focus(&this.focus, cx);
                            }))
                    })
                    .into_any_element(),
            );
        }
        let reading = self.scrolled_back
            && self
                .scrolled_at
                .is_some_and(|at| at.elapsed().as_millis() < u128::from(ASK_FLOAT_MS));
        let float = self
            .pinned
            .viewed
            .or(self
                .pinned
                .latest
                .filter(|_| !bar_shown)
                .and_then(|(i, row)| Some((i, row?))))
            .filter(|_| choice.float && reading)
            .and_then(|(i, row)| Some((first_line(i)?, row)));
        if let Some((text, row)) = float {
            out.push(
                div()
                    .absolute()
                    .top(if bar_shown {
                        row_height + px(T::S6)
                    } else {
                        px(T::S8)
                    })
                    .left_0()
                    .right(px(SCROLLBAR_HIT))
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .id("viewed-ask")
                            .max_w(gpui::relative(0.7))
                            .px(px(T::S12))
                            .py(px(T::S2))
                            .rounded(px(T::S12))
                            .bg(T::color(T::raised()))
                            .border(px(T::BORDER_WIDTH))
                            .border_color(T::color(T::card_border()))
                            .shadow_sm()
                            .text_size(px(T::META))
                            .text_color(T::color(T::secondary()))
                            .truncate()
                            .cursor_pointer()
                            .child(format!("在看 › {text}"))
                            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                                cx.stop_propagation()
                            })
                            .on_click(cx.listener(move |this, _, _, cx| this.back_to_ask(row, cx))),
                    )
                    .into_any_element(),
            );
        }
        out
    }
    /// The program's working directory as the host last reported it.
    pub fn cwd(&self) -> Option<&std::path::Path> {
        self.cwd.as_deref()
    }
    /// Asks the host where the program is now, for the footer; throttled, so a `cd` shows
    /// within a few seconds of the next output.
    fn refresh_cwd(&mut self, cx: &mut Context<Self>) {
        let window = std::time::Duration::from_secs(CWD_REFRESH_SECS);
        if let Some(at) = self.cwd_asked
            && at.elapsed() < window
        {
            // This output may be a `cd`'s last before the shell sits at its prompt: ask
            // once the window ends instead of waiting for output that may never come.
            if !self.cwd_due {
                self.cwd_due = true;
                let wait = window.saturating_sub(at.elapsed());
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(wait).await;
                    this.update(cx, |pane, cx| {
                        pane.cwd_due = false;
                        pane.refresh_cwd(cx);
                    })
                    .ok();
                })
                .detach();
            }
            return;
        }
        self.cwd_asked = Some(Instant::now());
        let socket = self.socket.clone();
        let id = self.terminal.clone();
        cx.spawn(async move |this, cx| {
            let reply = cx
                .background_spawn(async move { rpc(&socket, &Request::TerminalCwd { id }) })
                .await;
            if let Ok(Reply::Cwd(cwd)) = reply {
                this.update(cx, |pane, cx| {
                    let cwd = std::path::PathBuf::from(cwd);
                    if pane.cwd.as_ref() != Some(&cwd) {
                        pane.cwd = Some(cwd);
                        cx.notify();
                    }
                })
                .ok();
            }
        })
        .detach();
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
        let text = match paste_action(cx.read_from_clipboard().as_ref()) {
            PasteAction::Text(text) => text,
            // Claude Code and Codex read an image off the clipboard themselves on Ctrl+V.
            // Sent whatever runs here, as libghostty-swift does: a pane is not always
            // known to hold an Agent, and a shell only takes it as quoted-insert.
            PasteAction::Image => return self.input(vec![0x16], cx),
            PasteAction::Nothing => return,
        };
        if !self.bracketed() && line_count(&text) > 1 && !text.contains('\x1b') {
            self.paste_confirm = Some(text);
            self.local_error = None;
            cx.notify();
        } else {
            self.prefill(&text, cx);
        }
    }
    fn show_toast(&mut self, text: impl Into<gpui::SharedString>, cx: &mut Context<Self>) {
        self.toast = Some(text.into());
        self.toast_seq += 1;
        let seq = self.toast_seq;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(3))
                .await;
            let _ = this.update(cx, |pane, cx| {
                // A newer note restarts the clock instead of being cut short.
                if pane.toast_seq == seq {
                    pane.toast = None;
                    cx.notify();
                }
            });
        })
        .detach();
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
    /// The selected text, unwrapped as Ghostty copies it, even where it has scrolled
    /// out of view; None when nothing is selected.
    fn selected_text(&self) -> Option<String> {
        let state = self.connection.state.lock().ok()?;
        state
            .vt
            .as_ref()?
            .selection_text()
            .ok()
            .flatten()
            .filter(|t| !t.is_empty())
    }
    /// Copies the selection; with nothing selected, the visible screen.
    /// Says what was copied in the corner, away from the text being read. Copying the
    /// whole screen is called out, as nothing was selected.
    pub fn copy(&mut self, cx: &mut Context<Self>) {
        let selected = self.selected_text();
        let whole_screen = selected.is_none();
        let text = selected.unwrap_or_else(|| {
            let lines: Vec<String> = self
                .screen
                .rows
                .iter()
                .map(|row| {
                    let text: String = row.iter().map(|run| run.text.as_str()).collect();
                    text.trim_end().to_owned()
                })
                .collect();
            lines.join("\n")
        });
        let note = copied_note(&text, whole_screen);
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        self.show_toast(note, cx);
    }
    /// Checks a hovered path word on disk. A relative one also asks the host where the
    /// program is now, then checks again, so a `cd` since the last hover is followed.
    fn hover_word(&mut self, word: Option<screen::Link>, cx: &mut Context<Self>) {
        if word == self.hover_word {
            return;
        }
        self.hover_word = word.clone();
        self.check_hover_path(cx);
        // Asked for absolute paths too: the menu copies a path relative to it.
        let Some(word) = word else { return };
        let socket = self.socket.clone();
        let id = self.terminal.clone();
        cx.spawn(async move |this, cx| {
            let reply = cx
                .background_spawn(async move { rpc(&socket, &Request::TerminalCwd { id }) })
                .await;
            if let Ok(Reply::Cwd(cwd)) = reply {
                this.update(cx, |pane, cx| {
                    pane.cwd = Some(cwd.into());
                    if pane.hover_word.as_ref() == Some(&word) {
                        pane.check_hover_path(cx);
                    }
                })
                .ok();
            }
        })
        .detach();
    }
    /// The whole word when it names a file, else a piece of a broken one that does.
    fn check_hover_path(&mut self, cx: &mut Context<Self>) {
        let cwd = self.cwd.as_deref();
        let found = self.hover_word.as_ref().and_then(|w| {
            [(Some(w.uri.as_str()), true), (w.alone.as_deref(), false)]
                .into_iter()
                .find_map(|(word, whole)| {
                    let word = word?;
                    let path = screen::resolve_path(word, cwd).filter(|p| p.exists())?;
                    Some(Hovered {
                        path,
                        at: screen::split_position(word).1,
                        whole,
                        cwd: cwd.map(Path::to_owned),
                    })
                })
        });
        if found != self.hover_path {
            self.hover_path = found;
            cx.notify();
        }
    }
    /// The hovered path that exists, while its word is still on screen where it was.
    fn shown_target(&self) -> Option<&Hovered> {
        let word = self.hover_word.as_ref()?;
        (self.screen.path_at(word.row, word.col) == Some(word))
            .then_some(self.hover_path.as_ref())
            .flatten()
    }
    #[cfg(feature = "visual-test")]
    fn shown_path(&self) -> Option<&std::path::PathBuf> {
        self.shown_target().map(|t| &t.path)
    }
    /// The path under the pointer, for a click or the pane menu.
    pub fn path_target(&self) -> Option<Hovered> {
        self.shown_target().cloned()
    }
    /// While ⌘ is held over a link or path: what a click does, and how to only show it.
    fn click_hint(&self) -> Option<String> {
        if !self.modifiers.platform {
            return None;
        }
        if self.hover_link.is_some() {
            return Some("⌘ 点击在浏览器打开".into());
        }
        let target = self.shown_target()?;
        Some(super::open_path::hint(&Self::open_plan(
            target,
            self.modifiers.alt,
        )))
    }
    fn set_modifiers(&mut self, modifiers: gpui::Modifiers, cx: &mut Context<Self>) {
        let shows = |m: &gpui::Modifiers| (m.platform, m.platform && m.alt);
        if shows(&modifiers) != shows(&self.modifiers) {
            cx.notify();
        }
        self.modifiers = modifiers;
    }
    fn open_plan(target: &Hovered, reveal: bool) -> super::open_path::Plan {
        super::open_path::plan(
            &target.path,
            target.at,
            crate::ui::terminal_text::current().editor,
            reveal,
            super::open_path::installed,
        )
    }
    /// What clicking `target` would do, in the words of the menu entry.
    pub fn open_title(target: &Hovered) -> String {
        use super::open_path::Plan;
        match Self::open_plan(target, false) {
            Plan::Reveal { .. } => "在访达中显示（可执行文件不运行）".into(),
            Plan::OpenFolder => "在访达中打开".into(),
            Plan::OpenFile => "打开".into(),
            Plan::Editor { name, line, .. } => format!("在 {name} 打开第 {line} 行"),
            Plan::Terminal { line, .. } => format!("用 $EDITOR 打开第 {line} 行"),
        }
    }
    /// Opens `target` by the rules in `open_path`; `reveal` only shows it in Finder.
    pub fn open_target(&mut self, target: &Hovered, reveal: bool, cx: &mut Context<Self>) {
        use super::open_path::Plan;
        match Self::open_plan(target, reveal) {
            Plan::Reveal { executable } => {
                cx.reveal_path(&target.path);
                if executable {
                    self.show_toast("可执行文件只在访达中显示，不会运行", cx);
                }
            }
            Plan::OpenFolder | Plan::OpenFile => cx.open_with_system(&target.path),
            Plan::Editor {
                name,
                program,
                args,
                ..
            } => {
                let task = cx.background_spawn(async move {
                    let mut child = std::process::Command::new(program)
                        .args(args)
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()?;
                    // The tool hands the file to the editor and exits; reap it.
                    child.wait().map(|_| ())
                });
                cx.spawn(async move |this, cx| {
                    if let Err(e) = task.await {
                        this.update(cx, |pane, cx| {
                            pane.show_toast(format!("没能用 {name} 打开：{e}"), cx)
                        })
                        .ok();
                    }
                })
                .detach();
            }
            Plan::Terminal { command, .. } => {
                let cwd = target
                    .path
                    .parent()
                    .unwrap_or(Path::new("/"))
                    .to_string_lossy()
                    .into_owned();
                cx.emit(PaneEvent::Run { command, cwd });
            }
        }
    }
    /// Copies `text` and says so in the corner.
    pub fn copy_note(&mut self, text: String, note: &'static str, cx: &mut Context<Self>) {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        self.show_toast(note, cx);
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
    /// A window point relative to the pane, in logical pixels; may lie outside it.
    fn local(&self, point: Point<Pixels>) -> Option<(f32, f32)> {
        let bounds = self.bounds?;
        Some((
            f32::from(point.x - bounds.left()),
            f32::from(point.y - bounds.top()),
        ))
    }
    /// Everything the terminal holds, scrollback included, as Ghostty's select_all.
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        if let Ok(state) = self.connection.state.lock()
            && let Some(vt) = &state.vt
        {
            let t = vt.terminal();
            let _ = t
                .select_all()
                .and_then(|sel| t.set_selection(sel.as_ref()).map(|_| ()));
        }
        cx.notify();
    }
    fn clear_selection(&mut self) {
        if let Ok(state) = self.connection.state.lock()
            && let Some(vt) = &state.vt
        {
            let _ = vt.terminal().set_selection(None);
        }
    }
    pub fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let field = cx.new(|cx| crate::ui::field::Field::new("搜索滚动内容", "", cx));
        window.focus(&field.focus_handle(cx), cx);
        // Enter finds, then steps to the next match; the panel's buttons do the same.
        let events = cx.subscribe(&field, |this, _, event, cx| {
            if let crate::ui::field::FieldEvent::Submit = event {
                if this.matches.is_empty() || this.searched != this.search_query(cx) {
                    this.find_text(cx);
                } else {
                    this.step_match(1, cx);
                }
            }
        });
        self.search = Some(field);
        self._search_events = Some(events);
        self.matches.clear();
        self.current_match = None;
        self.searched = None;
        cx.notify();
    }
    fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search = None;
        self._search_events = None;
        self.matches.clear();
        self.current_match = None;
        self.searched = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }
    fn search_query(&self, cx: &App) -> Option<String> {
        self.search
            .as_ref()
            .map(|f| f.read(cx).text.trim().to_owned())
    }
    /// Moves to the next (1) or previous (-1) match, wrapping around, and centers it.
    fn step_match(&mut self, step: isize, cx: &mut Context<Self>) {
        if self.matches.is_empty() {
            return;
        }
        let n = self.matches.len() as isize;
        let next = match self.current_match {
            Some(i) => (i as isize + step).rem_euclid(n),
            // The first step from nothing lands on the nearest match above the bottom.
            None if step < 0 => n - 1,
            None => 0,
        } as usize;
        self.show_match(next, cx);
    }
    fn show_match(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(m) = self.matches.get(i) else { return };
        let row = m.row.saturating_sub(self.viewport.2 / 2);
        if let Ok(mut s) = self.connection.state.lock()
            && let Some(vt) = &mut s.vt
        {
            vt.scroll(ScrollViewport::Row(row));
            if let Ok(v) = vt.viewport() {
                self.viewport = v;
            }
        }
        self.current_match = Some(i);
        cx.notify();
    }
    fn find_text(&mut self, cx: &mut Context<Self>) {
        let Some(query) = self.search_query(cx) else {
            return;
        };
        self.searched = Some(query.clone());
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
                    Ok(found) => {
                        this.matches = found;
                        this.current_match = None;
                        // Newest first, as the bottom of the screen is closest.
                        this.step_match(-1, cx);
                    }
                    Err(e) => this.local_error = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }
    /// Like Ghostty, typing also drops the selection.
    fn input(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        self.clear_selection();
        self.follow_input(cx);
        self.send_input(bytes, false, cx);
    }
    fn follow_input(&mut self, cx: &mut Context<Self>) {
        self.scroll_lines = 0.;
        let moved = self.connection.state.lock().is_ok_and(|mut state| {
            state.vt.as_mut().is_some_and(|vt| {
                let away = !vt.terminal().viewport_active().unwrap_or(true);
                if away {
                    vt.scroll(ScrollViewport::Bottom);
                }
                away
            })
        });
        // A key the program does not echo would otherwise leave the old frame drawn.
        if moved {
            cx.notify();
        }
    }
    /// Mouse reports may be dropped by the host while the program is behind on input.
    fn pointer(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        self.send_input(bytes, true, cx);
    }
    /// Ghostty's 600 ms phase. The timer runs only while this pane holds focus in the
    /// active window, so idle panes keep no timer.
    fn restart_blink(&mut self, cx: &mut Context<Self>) {
        self.blink_epoch += 1;
        if !self.blink_on {
            self.blink_on = true;
            cx.notify();
        }
        if !self.last_focus {
            return;
        }
        let epoch = self.blink_epoch;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(600))
                    .await;
                let running = this.update(cx, |pane, cx| {
                    if pane.blink_epoch != epoch {
                        return false;
                    }
                    let blinks = crate::ui::terminal_text::current().cursor_blink
                        && pane.screen.cursor_blinking;
                    let next = !blinks || !pane.blink_on;
                    if next != pane.blink_on {
                        pane.blink_on = next;
                        cx.notify();
                    }
                    true
                });
                if !matches!(running, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }
    /// A peek that is used, not just looked at, sizes the terminal to itself.
    fn take_size(&mut self, cx: &mut Context<Self>) {
        if self.peek {
            self.peek = false;
            cx.notify();
        }
    }
    fn send_input(&mut self, bytes: Vec<u8>, lossy: bool, cx: &mut Context<Self>) {
        self.take_size(cx);
        self.typed = Some(Instant::now());
        self.restart_blink(cx);
        if let Err(error) = self.connection.send(Request::Input { bytes, lossy }) {
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
        // Keymap shortcuts: the terminal's own run here, the workbench's pass up without
        // reaching the program; any other Cmd key still goes to it.
        if let Some(action) = crate::keymap::action(k, cx) {
            if !action.in_terminal() {
                return;
            }
            use crate::keymap::Action;
            match action {
                Action::ZoomIn | Action::ZoomOut | Action::ZoomReset => {
                    if let Err(error) = screen::zoom(match action {
                        Action::ZoomIn => Some(1),
                        Action::ZoomOut => Some(-1),
                        _ => None,
                    }) {
                        self.local_error = Some(error);
                    }
                    cx.refresh_windows();
                }
                Action::SelectAll => self.select_all(cx),
                Action::Find => self.open_search(window, cx),
                Action::ScrollTop => self.scroll_to(ScrollViewport::Top, cx),
                Action::ScrollBottom => self.scroll_to(ScrollViewport::Bottom, cx),
                Action::PageUp | Action::PageDown => {
                    let rows = self.screen.rows.len().max(1) as isize;
                    let rows = if action == Action::PageUp {
                        -rows
                    } else {
                        rows
                    };
                    self.scroll_to(ScrollViewport::Delta(rows), cx);
                }
                Action::Clear => {
                    self.clear_selection();
                    if let Err(e) = self.connection.send(Request::ClearHistory) {
                        self.local_error = Some(e.to_string());
                    }
                    cx.notify();
                }
                _ => (),
            }
            cx.stop_propagation();
            return;
        }
        if !self.marked.is_empty() {
            return;
        }
        let Some(key) = crate::terminal::keyboard::physical_key(&k.key) else {
            return;
        };
        // Ordinary text belongs to the IME/text-input path, avoiding duplicate commits.
        if !k.modifiers.control
            && !k.modifiers.alt
            && crate::terminal::keyboard::is_text_key(&k.key)
        {
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
            crate::terminal::keyboard::encode(vt, key, &k.key, mods)
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
/// The pane's cell grid for the selection gesture, once its cells have been measured.
fn grid(
    vt: &Vt,
    metrics: Option<screen::CellMetrics>,
    scale: f32,
) -> Option<crate::terminal::selection::Grid> {
    let metrics = metrics?;
    let t = vt.terminal();
    Some(crate::terminal::selection::Grid {
        cols: t.cols().ok()?,
        rows: t.rows().ok()?,
        cell_w: f32::from(metrics.width),
        cell_h: f32::from(metrics.height),
        scale,
    })
}
impl Pane {
    /// Repaint this terminal's defaults after a theme switch. The display copy changes
    /// here; the host's copy, which programs query, only from the writing pane.
    fn sync_scheme(&mut self) {
        let scheme = T::scheme();
        let writable = {
            let Ok(mut state) = self.connection.state.lock() else {
                return;
            };
            if state.scheme == Some(scheme) {
                return;
            }
            let Some(vt) = state.vt.as_mut() else {
                return;
            };
            if let Err(e) = vt.set_scheme(scheme) {
                self.local_error = Some(e.to_string());
                return;
            }
            state.scheme = Some(scheme);
            state.writable && !state.ended
        };
        if writable {
            let _ = self.connection.send(Request::Scheme(scheme));
        }
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
        self.refresh_cwd(cx);
        self.connection.dirty.store(false, Ordering::Release);
        self.sync_scheme();
        let shared = self.connection.state.clone();
        let state = shared.lock().unwrap();
        if let Some(vt) = &state.vt {
            let result = (|| {
                if self.capture.is_none() {
                    self.capture = Some(screen::Capture::new()?);
                }
                self.capture.as_mut().unwrap().update(vt, &mut self.screen)
            })();
            self.scrolled_back = !vt.terminal().viewport_active().unwrap_or(true);
            let top = self.viewport.0;
            self.viewport = vt.viewport().unwrap_or(self.viewport);
            if self.scrolled_back && self.viewport.0 != top {
                self.scrolled_at = Some(Instant::now());
                cx.spawn(async move |this, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(ASK_FLOAT_MS + 50))
                        .await;
                    let _ = this.update(cx, |_, cx| cx.notify());
                })
                .detach();
            }
            self.refresh_pinned(cx);
            match result {
                Ok(_) => {
                    self.cursor_anchor.observe(self.screen.cursor);
                }
                Err(e) => self.local_error = Some(e.to_string()),
            }
        }
        // While reattaching, the status line says so; the broken stream's error is noise.
        let error = self
            .local_error
            .clone()
            .or(state.error.clone())
            .filter(|_| !state.dropped && self.reconnect_delay == 0);
        if self.started_under.is_none() && state.vt.is_some() && !state.ended {
            self.started_under = Some(T::scheme());
        }
        let tint_stale = self.agent.is_some()
            && !state.ended
            && !self.tint_note_dismissed
            && self.started_under.is_some_and(|s| s != T::scheme());
        let status = if state.ended {
            "进程已退出"
        } else if state.dropped || self.reconnect_delay > 0 {
            "连接断开 · 正在重连…"
        } else if state.vt.is_none() {
            "正在恢复终端…"
        } else if state.writable {
            "终端已连接"
        } else {
            "只读 · 可明确接管"
        };
        let writable = state.writable;
        drop(state);
        // OSC 52: the program copied something. Only the writable client owns the clipboard.
        if let Some(text) = self
            .connection
            .clipboard
            .lock()
            .ok()
            .and_then(|mut c| c.take())
            && writable
        {
            // The program copied on its own (e.g. an Agent's /copy); say so, or the
            // user can't tell anything happened.
            let note = format!("程序{}", copied_note(&text, false));
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
            self.show_toast(note, cx);
        }
        let entity = cx.entity();
        let content = canvas(
            |_, _, _| (),
            move |bounds, _, window, cx| {
                let drags = entity.clone();
                window.on_mouse_event(move |e: &gpui::MouseMoveEvent, phase, _, cx| {
                    if phase == gpui::DispatchPhase::Bubble {
                        drags.update(cx, |pane, cx| pane.drag_selection(e, cx));
                    }
                });
                entity.update(cx, |pane, cx| {
                    // A peek draws at the terminal's own size; one taller than the overlay
                    // slides up so the prompt at its bottom stays in view.
                    let bounds = match (pane.peek, pane.metrics) {
                        (true, Some(m)) => {
                            let tall = m.height * pane.screen.rows.len() as f32;
                            let over = (tall - bounds.size.height).max(px(0.));
                            Bounds::new(bounds.origin - gpui::point(px(0.), over), bounds.size)
                        }
                        _ => bounds,
                    };
                    pane.bounds = Some(bounds);
                    pane.scale = window.scale_factor();
                    window.handle_input(
                        &pane.focus,
                        ElementInputHandler::new(bounds, cx.entity()),
                        cx,
                    );
                    let font = T::terminal_mono(cx);
                    // Font metrics are logical pixels, independent of backing scale.
                    let key = (
                        font.clone(),
                        screen::font_size(),
                        crate::ui::terminal_text::line_height(),
                    );
                    if pane.metrics_font.as_ref() != Some(&key) || pane.metrics.is_none() {
                        pane.metrics = screen::CellMetrics::measure(&font, window);
                        pane.metrics_font = Some(key);
                    }
                    let Some(metrics) = pane.metrics else { return };
                    let focused = pane.focus.is_focused(window);
                    let look = crate::ui::terminal_text::current();
                    let cursor = if !pane.marked.is_empty() {
                        screen::CursorPaint::Hidden
                    } else if focused {
                        if pane.blink_on {
                            screen::CursorPaint::Shown
                        } else {
                            screen::CursorPaint::Hidden
                        }
                    } else {
                        match look.unfocused_cursor {
                            crate::ui::terminal_text::UnfocusedCursor::Hidden => {
                                screen::CursorPaint::Hidden
                            }
                            crate::ui::terminal_text::UnfocusedCursor::Hollow => {
                                screen::CursorPaint::Hollow
                            }
                        }
                    };
                    screen::paint_cached(
                        &pane.screen,
                        &font,
                        bounds,
                        metrics,
                        cursor,
                        &mut pane.layout_cache,
                        window,
                        cx,
                    );
                    // Underlined only while ⌘ is held, when a click would open it.
                    let hovered: Vec<&screen::Link> = match pane.hover_link {
                        _ if !pane.modifiers.platform => Vec::new(),
                        Some((y, x)) => pane.screen.link_at(y, x).map_or(Vec::new(), |hover| {
                            pane.screen
                                .links
                                .iter()
                                .filter(|l| l.uri == hover.uri)
                                .collect()
                        }),
                        None => match (pane.shown_target(), pane.hover_word.as_ref()) {
                            (Some(target), Some(word)) if target.whole => pane
                                .screen
                                .paths
                                .iter()
                                .filter(|l| l.start == word.start && l.uri == word.uri)
                                .collect(),
                            (Some(_), Some(word)) => vec![word],
                            _ => Vec::new(),
                        },
                    };
                    for l in hovered {
                        window.paint_quad(gpui::fill(
                            Bounds::new(
                                gpui::point(
                                    bounds.left() + metrics.width * l.col as f32,
                                    bounds.top() + metrics.height * (l.row + 1) as f32
                                        - metrics.stroke,
                                ),
                                gpui::size(metrics.width * l.cols as f32, metrics.stroke),
                            ),
                            crate::ui::theme::hsla(T::terminal_fg()),
                        ));
                    }
                    let (top, total, len) = pane.viewport;
                    // Matches in view, the current one stronger; drawn over the text, so
                    // translucent, the current one also outlined.
                    // The amber T::matched() washes into the popover; over terminal text
                    // the pure hue is needed to stay visible in both schemes.
                    let matched = crate::ui::theme::hsla(T::SEARCH_HIT);
                    for (i, m) in pane.matches.iter().enumerate() {
                        if m.row < top || m.row >= top + len {
                            continue;
                        }
                        let current = pane.current_match == Some(i);
                        let at = Bounds::new(
                            gpui::point(
                                bounds.left() + metrics.width * m.col as f32,
                                bounds.top() + metrics.height * (m.row - top) as f32,
                            ),
                            gpui::size(metrics.width * m.cols as f32, metrics.height),
                        );
                        window.paint_quad(gpui::fill(
                            at,
                            matched.opacity(if current { 0.55 } else { 0.25 }),
                        ));
                        if current {
                            window.paint_quad(gpui::outline(at, matched, gpui::BorderStyle::Solid));
                        }
                    }
                    if pane.scrollbar_shown() {
                        let h = bounds.size.height;
                        let thumb = (h * (len as f32 / total as f32)).max(px(T::S20));
                        let y = (h - thumb) * (top as f32 / (total - len).max(1) as f32);
                        window.paint_quad(
                            gpui::fill(
                                Bounds::new(
                                    gpui::point(
                                        bounds.right() - px(SCROLLBAR_WIDTH + T::S2),
                                        bounds.top() + y,
                                    ),
                                    gpui::size(px(SCROLLBAR_WIDTH), thumb),
                                ),
                                T::color(T::secondary()).opacity(0.6),
                            )
                            .corner_radii(px(SCROLLBAR_WIDTH / 2.)),
                        );
                    }
                    for (row, cols) in pane.screen.selection.iter().enumerate() {
                        let Some((left, right)) = *cols else { continue };
                        window.paint_quad(gpui::fill(
                            Bounds::new(
                                gpui::point(
                                    bounds.left() + metrics.width * left as f32,
                                    bounds.top() + metrics.height * row as f32,
                                ),
                                gpui::size(
                                    metrics.width * (right + 1).saturating_sub(left) as f32,
                                    metrics.height,
                                ),
                            ),
                            T::color(T::focus()).opacity(0.25),
                        ));
                    }
                    // Ghostty's unfocused-split-opacity: the program's own cursor (Claude
                    // and Codex draw one as reversed text) recedes with the rest.
                    if pane.split && !focused && look.unfocused_dim > 0. {
                        window.paint_quad(gpui::fill(
                            bounds,
                            crate::ui::theme::hsla(pane.screen.background)
                                .opacity(look.unfocused_dim),
                        ));
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
                    if !pane.peek
                        && (pane.requested_size, pane.requested_cell) != (dims, cell)
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
            .bg(T::color(T::pane()))
            .track_focus(&self.focus)
            // Right-click is never reported to the program: it opens Keepline's pane
            // menu even when a full-screen Agent has taken the mouse.
            .on_mouse_down(gpui::MouseButton::Left,cx.listener(Self::mouse_down))
            .on_mouse_down(gpui::MouseButton::Middle,cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_modifiers_changed(cx.listener(|this, e: &gpui::ModifiersChangedEvent, _, cx| {
                this.set_modifiers(e.modifiers, cx)
            }))
            .on_mouse_up(gpui::MouseButton::Left,cx.listener(Self::mouse_up))
            .on_mouse_up(gpui::MouseButton::Middle,cx.listener(Self::mouse_up))
            .on_mouse_up_out(gpui::MouseButton::Left,cx.listener(Self::mouse_up))
            .on_mouse_up_out(gpui::MouseButton::Middle,cx.listener(Self::mouse_up))
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
                        .text_color(T::color(T::secondary()))
                        .child(status),
                )
            })
            .when_some(self.paste_confirm.as_deref().map(line_count),|d,lines| {
                d.child(div().px(px(T::S12)).py(px(T::S8)).flex().items_center().gap(px(T::S12))
                    .bg(T::color(T::panel())).text_size(px(T::META))
                    .child(div().flex_1().text_color(T::color(T::secondary()))
                        .child(format!("这个程序没有启用安全粘贴：要粘贴的 {lines} 行会逐行立即执行。")))
                    .child(div().id("paste-anyway").cursor_pointer().text_color(T::color(T::link())).child(format!("仍然粘贴 {lines} 行"))
                        .on_click(cx.listener(|this,_,_,cx|this.confirm_paste(cx))))
                    .child(div().id("paste-cancel").cursor_pointer().child("取消")
                        .on_click(cx.listener(|this,_,_,cx|{this.paste_confirm=None;cx.notify();}))))
            })
            .when(tint_stale, |d| {
                d.child(
                    div()
                        .px(px(T::S12))
                        .py(px(T::S4))
                        .flex()
                        .items_center()
                        .gap(px(T::S12))
                        .text_size(px(T::META))
                        .child(div().flex_1().text_color(T::color(T::secondary())).child(
                            "这个终端在切换外观前就开着，程序自己画的底色要退出重开（如 codex resume）才会跟上。",
                        ))
                        .child(
                            div()
                                .id("tint-note-dismiss")
                                .cursor_pointer()
                                .text_color(T::color(T::link()))
                                .child("知道了")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.tint_note_dismissed = true;
                                    cx.notify();
                                })),
                        ),
                )
            })
            .when_some(self.search.clone(), |d, field| {
                let count = self.matches.len();
                let at = self.current_match.map_or(0, |i| i + 1);
                let link = |id: &'static str, text: &'static str| {
                    div().id(id).cursor_pointer().text_color(T::color(T::link())).child(text)
                };
                let mut panel = div()
                    .p(px(T::S8))
                    .flex()
                    .flex_col()
                    .gap(px(T::S4))
                    .child(field)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(T::S12))
                            .child(link("find", "查找").on_click(cx.listener(|this, _, _, cx| this.find_text(cx))))
                            .when(count > 0, |d| {
                                d.child(link("prev-match", "↑ 上一个").on_click(cx.listener(|this, _, _, cx| this.step_match(-1, cx))))
                                    .child(link("next-match", "↓ 下一个").on_click(cx.listener(|this, _, _, cx| this.step_match(1, cx))))
                            })
                            .child(div().text_color(T::color(T::secondary())).child(if self.searched.is_none() {
                                "按回车查找".to_owned()
                            } else if count == 0 {
                                "没有匹配".to_owned()
                            } else {
                                format!("第 {at} / {count} 处")
                            }))
                            .child(div().flex_1())
                            .child(link("close-find", "关闭搜索").on_click(cx.listener(|this, _, window, cx| this.close_search(window, cx)))),
                    );
                // The newest 20 matches, newest first; clicking one goes to it.
                for (i, m) in self.matches.iter().enumerate().rev().take(20) {
                    let current = self.current_match == Some(i);
                    panel = panel.child(
                        div()
                            .id(gpui::SharedString::from(format!("match-{i}")))
                            .flex_shrink_0()
                            .truncate()
                            .cursor_pointer()
                            .when(current, |d| d.text_color(T::color(T::primary())).font_weight(gpui::FontWeight::SEMIBOLD))
                            .when(!current, |d| d.text_color(T::color(T::secondary())))
                            .child(m.line.clone())
                            .on_click(cx.listener(move |this, _, _, cx| this.show_match(i, cx))),
                    );
                }
                d.child(panel.id("search-results").max_h(px(T::SEARCH_HEIGHT)).overflow_y_scroll())
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(content)
                    .children(self.ask_overlays(cx))
                    .when(self.scrolled_back || self.toast.is_some(), |d| {
                        // Floats over the grid so it never resizes the terminal; clear of
                        // the scrollbar along the right edge. A toast stacks above the
                        // "回到底部" button instead of covering it.
                        let toast = self.toast.clone().map(|toast| {
                            div()
                                .px(px(T::S8))
                                .py(px(T::S4))
                                .rounded(px(T::CONTROL_RADIUS))
                                .bg(T::color(T::raised()))
                                .border(px(T::BORDER_WIDTH))
                                .border_color(T::color(T::card_border()))
                                .text_size(px(T::META))
                                .text_color(T::color(T::secondary()))
                                .child(toast)
                        });
                        let bottom = self.scrolled_back.then(|| {
                            div()
                                .id("scroll-to-bottom")
                                .px(px(T::S8))
                                .py(px(T::S4))
                                .rounded(px(T::CHIP_RADIUS))
                                .bg(T::color(T::accent()))
                                .text_size(px(T::META))
                                .text_color(T::color(T::on_accent()))
                                .cursor_pointer()
                                .child("回到底部 ↓")
                                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                                    cx.stop_propagation()
                                })
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.scroll_to(ScrollViewport::Bottom, cx);
                                    window.focus(&this.focus, cx);
                                }))
                        });
                        d.child(
                            div()
                                .absolute()
                                .right(px(T::S20))
                                .bottom(px(T::S8))
                                .flex()
                                .flex_col()
                                .items_end()
                                .gap(px(T::S4))
                                .children(toast)
                                .children(bottom),
                        )
                    }),
            )
            .when_some(self.click_hint().zip(self.pointer), |d, (hint, at)| {
                // Below and right of the pointer, so the word itself stays readable.
                d.child(
                    gpui::deferred(
                        gpui::anchored()
                            .position(at + gpui::point(px(T::S12), px(T::S16)))
                            .snap_to_window_with_margin(px(T::S8))
                            .child(
                                div()
                                    .id("click-hint")
                                    .px(px(T::S8))
                                    .py(px(T::S2))
                                    .rounded(px(T::CONTROL_RADIUS))
                                    .bg(T::color(T::raised()))
                                    .border(px(T::BORDER_WIDTH))
                                    .border_color(T::color(T::card_border()))
                                    .text_size(px(T::META))
                                    .text_color(T::color(T::secondary()))
                                    .child(hint),
                            ),
                    )
                    .priority(2),
                )
            })
            .when(self.flash, |d| {
                d.relative().child(
                    div()
                        .absolute()
                        .inset_0()
                        .border(px(T::S2))
                        .border_color(T::color(T::accent())),
                )
            });
        if let Some(error) = error {
            view = view.child(
                div()
                    .px(px(T::S12))
                    .py(px(T::S4))
                    .text_size(px(T::META))
                    .text_color(Tone::Danger.ink())
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
        // Composition is drawn at the cursor, so it needs the input line in view too.
        if !text.is_empty() {
            self.follow_input(cx);
        }
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
fn line_count(text: &str) -> usize {
    text.trim_end_matches(['\r', '\n']).lines().count()
}
/// "已复制 3 行" / "已复制 12 字"; a screen copied for lack of a selection says so.
fn copied_note(text: &str, whole_screen: bool) -> String {
    let lines = text.lines().count();
    if whole_screen {
        return format!("没选中文字，已复制整屏 {lines} 行");
    }
    if lines > 1 {
        format!("已复制 {lines} 行")
    } else {
        format!("已复制 {} 字", text.chars().count())
    }
}
const SELECT_HINT: &str = "这个程序接管了鼠标：按住 Shift 再拖动来选中文字";

#[derive(Debug, PartialEq)]
enum PasteAction {
    Text(String),
    /// Only an image: the program fetches it from the clipboard on Ctrl+V.
    Image,
    Nothing,
}
fn paste_action(item: Option<&gpui::ClipboardItem>) -> PasteAction {
    let Some(item) = item else {
        return PasteAction::Nothing;
    };
    if let Some(text) = item.text() {
        return PasteAction::Text(text);
    }
    let image = item
        .entries()
        .iter()
        .any(|e| matches!(e, gpui::ClipboardEntry::Image(_)));
    if image {
        PasteAction::Image
    } else {
        PasteAction::Nothing
    }
}

/// What a terminal sends for a paste without bracketed paste: newlines become CR.
fn plain_paste(text: &str) -> Vec<u8> {
    text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
}
#[cfg(test)]
mod paste_tests {
    use super::{PasteAction, copied_note};
    use gpui::{ClipboardItem, Image, ImageFormat};

    #[test]
    fn an_image_goes_to_the_program_as_ctrl_v() {
        let image = ClipboardItem::new_image(&Image::from_bytes(ImageFormat::Png, vec![1]));
        assert_eq!(super::paste_action(Some(&image)), PasteAction::Image);
    }
    #[test]
    fn copied_note_counts_lines_or_characters_and_flags_a_whole_screen_copy() {
        assert_eq!(copied_note("中文ab", false), "已复制 4 字");
        assert_eq!(copied_note("1\n2\n3", false), "已复制 3 行");
        assert_eq!(copied_note("a\nb", true), "没选中文字，已复制整屏 2 行");
    }
    #[test]
    fn text_wins_over_an_image_and_an_empty_clipboard_does_nothing() {
        let text = ClipboardItem::new_string("hi".into());
        assert_eq!(
            super::paste_action(Some(&text)),
            PasteAction::Text("hi".into())
        );
        assert_eq!(super::paste_action(None), PasteAction::Nothing);
    }
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
/// Time left until the next boundary of one process-wide 50 ms clock: background panes
/// that wake at different moments all flush on the same boundary, in the same frame.
fn until_shared_tick() -> std::time::Duration {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    const TICK: u128 = 50_000;
    let micros = START.get_or_init(Instant::now).elapsed().as_micros();
    std::time::Duration::from_micros((TICK - micros % TICK) as u64)
}
