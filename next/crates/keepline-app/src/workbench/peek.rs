//! Peek: a session's live screen floated over the current tab, after Herdr App's peek.
//! An approval on it is answered from its title bar; 「↗ 打开」 switches to it for real.
use super::*;

pub(super) struct Peek {
    pub(super) terminal: String,
    pub(super) session: String,
    /// Its own attachment, in peek mode: it never resizes the terminal. Closing the peek
    /// drops it and the attachment with it; nothing keeps running.
    pub(super) pane: Entity<Pane>,
    /// An answer sent to the host and not replied to yet.
    pub(super) answering: bool,
    /// Why the host turned the last answer down, e.g. it was answered in the pane already.
    pub(super) note: Option<String>,
}

impl Workbench {
    /// A sidebar row: a session running out of sight floats up for a look; one already
    /// on screen, or with nothing live to look at, opens as before.
    pub(super) fn peek_or_open(
        &mut self,
        session: &AgentSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let hidden = live_terminal(&self.workspace, session)
            .map(|t| t.id.clone())
            .filter(|t| !self.layout.panes.contains_key(t));
        match hidden {
            Some(terminal) => self.open_peek(session, terminal, window, cx),
            None => {
                self.close_peek(cx);
                self.open_session(session, window, cx);
            }
        }
    }
    fn open_peek(
        &mut self,
        session: &AgentSession,
        terminal: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.peek.as_ref().is_none_or(|p| p.terminal != terminal) {
            let pane = cx.new(|cx| {
                let mut pane = Pane::new(self.store.socket(), &terminal, false, cx);
                pane.peek = true;
                pane
            });
            cx.observe(&pane, |_, _, cx| cx.notify()).detach();
            self.peek = Some(Peek {
                terminal,
                session: session.id.clone(),
                pane,
                answering: false,
                note: None,
            });
        }
        if let Some(peek) = &self.peek {
            window.focus(&peek.pane.focus_handle(cx), cx);
        }
        cx.notify();
    }
    pub(super) fn close_peek(&mut self, cx: &mut Context<Self>) {
        if self.peek.take().is_some() {
            self.nav.pending_focus = true;
            cx.notify();
        }
    }
    /// A tab that now shows the peeked terminal takes it over: two attachments to one
    /// terminal would leave the tab's read-only.
    pub(super) fn drop_shown_peek(&mut self, shown: &[String]) {
        if self.peek.as_ref().is_some_and(|p| shown.contains(&p.terminal)) {
            self.peek = None;
        }
    }
    /// The approval question the peeked session's screen shows, if any.
    fn peek_approval(&self) -> Option<String> {
        let peek = self.peek.as_ref()?;
        self.workspace
            .content
            .get(&peek.session)
            .map(|c| c.approval.clone())
            .filter(|a| !a.is_empty())
    }
    fn answer_peek(&mut self, allow: bool, cx: &mut Context<Self>) {
        let Some(question) = self.peek_approval() else {
            return;
        };
        let Some(peek) = self.peek.as_mut().filter(|p| !p.answering) else {
            return;
        };
        peek.answering = true;
        peek.note = None;
        let terminal = peek.terminal.clone();
        let task = cx.background_spawn(self.store.send(Request::AnswerApproval {
            id: terminal.clone(),
            question,
            allow,
        }));
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if let Some(peek) = this.peek.as_mut().filter(|p| p.terminal == terminal) {
                    peek.answering = false;
                    peek.note = result.err().map(|e| e.to_string());
                }
                cx.notify();
            })
        })
        .detach();
        cx.notify();
    }
    pub(super) fn peek_view(&self, peek: &Peek, cx: &Context<Self>) -> gpui::AnyElement {
        let session = self
            .workspace
            .agent_sessions
            .iter()
            .find(|s| s.id == peek.session);
        let group = session.map(|s| session_group(&self.workspace, s));
        let title = session
            .map(|s| self.session_title(s))
            .unwrap_or_else(|| "终端".into());
        let approval = self.peek_approval();
        let state = match (&approval, group) {
            (Some(_), _) => Some("等你批准".to_owned()),
            (None, Some(g)) => Some(g.label().to_owned()),
            _ => None,
        };
        let open = session.cloned();
        let bar = div()
            .h(px(T::ROW + T::S8))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(T::S8))
            .px(px(T::S12))
            .border_b(px(T::BORDER_WIDTH))
            .border_color(T::color(T::border()))
            .bg(T::color(T::panel()))
            .when_some(group, |d, g| d.child(dot(g)))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(px(T::MONO_SIZE))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(title),
            )
            .when_some(state, |d, s| {
                d.child(
                    label(s)
                        .flex_shrink_0()
                        .text_size(px(T::CHIP_SIZE))
                        .text_color(group.map_or(T::color(T::muted()), |g| g.colors().1)),
                )
            })
            .child(div().flex_1())
            // Only an approval the screen rules recognize gets the two answers; anything
            // else (a choice, an unrecognized prompt) is answered by typing or in its tab.
            .when(approval.is_some(), |d| {
                let busy = peek.answering;
                d.child(
                    button("peek-allow", "允许一次")
                        .bg(T::color(T::raised()))
                        .text_color(T::color(T::link()))
                        .when(busy, |b| b.opacity(0.5))
                        .on_click(cx.listener(|this, _, _, cx| this.answer_peek(true, cx))),
                )
                .child(
                    button("peek-deny", "拒绝")
                        .when(busy, |b| b.opacity(0.5))
                        .on_click(cx.listener(|this, _, _, cx| this.answer_peek(false, cx))),
                )
            })
            .child(
                button("peek-open", "↗ 打开").on_click(cx.listener(move |this, _, window, cx| {
                    this.close_peek(cx);
                    if let Some(s) = &open {
                        this.open_session(s, window, cx);
                    }
                })),
            )
            .child(
                button("peek-close", "✕")
                    .aria_label("关闭偷看")
                    .on_click(cx.listener(|this, _, _, cx| this.close_peek(cx))),
            );
        div()
            .id("peek")
            .absolute()
            .inset_0()
            .m(px(T::S12))
            .occlude()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(T::PANE_RADIUS))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::control()))
            .bg(T::color(T::pane()))
            .shadow_lg()
            // Esc closes the peek and never reaches the Agent, where it would mean 「No」.
            .capture_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                let k = &event.keystroke;
                if k.key == "escape" && !k.modifiers.modified() {
                    this.close_peek(cx);
                    cx.stop_propagation();
                }
            }))
            .child(bar)
            .when_some(peek.note.clone(), |d, note| {
                d.child(
                    label(note)
                        .px(px(T::S12))
                        .py(px(T::S4))
                        .text_size(px(T::META))
                        .text_color(Group::Error.colors().1),
                )
            })
            .child(div().flex_1().min_h_0().pl(px(T::S4)).child(peek.pane.clone()))
            .into_any_element()
    }
}

#[cfg(feature = "visual-test")]
impl Workbench {
    pub fn visual_peek_show_tab(&mut self, title: &str, cx: &mut Context<Self>) {
        let tab = self.workspace.tabs.iter().find(|t| t.title == title).expect("tab");
        self.nav.space = tab.space.clone();
        self.nav.tab = Some(tab.id.clone());
        self.nav.overview = false;
        self.nav.focused = None;
        self.sync_panes(cx);
        cx.notify();
    }
    /// The sidebar row's click for the session in `terminal`; whether a peek opened.
    pub fn visual_peek_row(&mut self, terminal: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(s) = self
            .workspace
            .agent_sessions
            .iter()
            .find(|s| s.current && s.terminal.as_deref() == Some(terminal))
            .cloned()
        else {
            return false;
        };
        self.peek_or_open(&s, window, cx);
        self.peek.is_some()
    }
    /// The peeked terminal, the approval it offers to answer, and the host's last refusal.
    pub fn visual_peek_state(&self) -> Option<(String, Option<String>, Option<String>)> {
        let p = self.peek.as_ref()?;
        Some((p.terminal.clone(), self.peek_approval(), p.note.clone()))
    }
    pub fn visual_peek_answer(&mut self, allow: bool, cx: &mut Context<Self>) {
        self.answer_peek(allow, cx);
    }
    pub fn visual_peek_focused(&self, window: &Window, cx: &gpui::App) -> bool {
        self.peek
            .as_ref()
            .is_some_and(|p| p.pane.focus_handle(cx).is_focused(window))
    }
}

#[cfg(feature = "visual-test")]
impl Workbench {
    pub fn visual_pins(&self) -> Vec<String> {
        self.sidebar.pinned.clone()
    }
}
