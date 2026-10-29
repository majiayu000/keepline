//! Session grouping, the 「你在这里」 card, the "needs you" queue and navigation into a session.
use super::*;

/// Which finished turns the user has not looked at yet.
pub(super) struct Attention {
    /// Last live phase of each session whose agent runs in a Keepline terminal.
    pub(super) phases: HashMap<String, Phase>,
    /// Sessions whose turn ended while their pane was out of view, until it is shown.
    pub(super) unseen: HashSet<String>,
}
/// Conversations being resumed: the next launch's command, and the terminals opened for it.
pub(super) struct Resuming {
    /// The next launch resumes a conversation: its command, cwd and session id.
    pub(super) resume: Option<(String, String, String)>,
    /// Terminals opened to resume a session, so clicking it again returns there
    /// instead of opening another tab while the resume has not taken.
    pub(super) resumed: HashMap<String, String>,
}
impl Workbench {
    pub(super) fn open_session(
        &mut self,
        session: &AgentSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.search.hide();
        self.nav.task_detail = None;
        self.overview.here = None;
        self.nav.zoomed = None;
        let Some(terminal) = live_terminal(&self.workspace, session)
            .or_else(|| resume_terminal(&self.workspace, &self.resuming.resumed, session))
        else {
            if let Some(space) = session_space(&self.workspace, session) {
                self.nav.space = space.into();
            }
            self.resuming.resume = Some((
                session.runtime.resume_command(&session.cli_session_id),
                session.cwd.clone(),
                session.id.clone(),
            ));
            self.launch(false, session.task.clone(), cx);
            return;
        };
        let terminal_id = terminal.id.clone();
        if !self.focus_terminal(&terminal_id, window, cx) {
            let tab = Tab {
                id: id(),
                space: self.nav.space.clone(),
                default_task: session.task.clone(),
                title: String::new(),
                layout: Some(Layout::Leaf(terminal_id.clone())),
            };
            self.nav.tab = Some(tab.id.clone());
            self.nav.overview = false;
            self.edit(Edit::PutTab { tab }, cx);
        }
    }
    /// Shows the tab holding `terminal_id` and focuses its pane; false when no tab holds it.
    pub(super) fn focus_terminal(
        &mut self,
        terminal_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.nav.focused = Some(terminal_id.to_owned());
        self.nav.pending_focus = true;
        let Some(tab) = self.workspace.terminal_tab(terminal_id) else {
            return false;
        };
        self.nav.space = tab.space.clone();
        self.nav.tab = Some(tab.id.clone());
        self.nav.overview = false;
        self.sync_panes(cx);
        if let Some(pane) = self.layout.panes.get(terminal_id) {
            window.focus(&pane.focus_handle(cx), cx);
        }
        cx.notify();
        true
    }
    pub(super) fn session_title(&self, s: &AgentSession) -> String {
        if let Some(c) = self
            .workspace
            .content
            .get(&s.id)
            .filter(|c| !c.title.is_empty())
        {
            return keepline_core::content::topic(&c.title);
        }
        if self
            .workspace
            .content
            .get(&s.id)
            .is_some_and(|c| !c.first_ask_known)
        {
            return format!(
                "最初原话未覆盖 · {}",
                &s.cli_session_id.chars().take(8).collect::<String>()
            );
        }
        s.task
            .as_ref()
            .and_then(|id| self.workspace.tasks.iter().find(|t| &t.id == id))
            .map(|t| t.title.clone())
            .unwrap_or_else(|| match self.workspace.content.get(&s.id) {
                None if s.current => format!("{} · 新会话", s.runtime.executable()),
                _ => format!("{} · {}", s.runtime.executable(), s.cli_session_id),
            })
    }
    /// When its conversation last moved.
    pub(super) fn session_updated(&self, s: &AgentSession) -> u64 {
        self.workspace
            .content
            .get(&s.id)
            .map_or(s.updated, |c| c.updated)
    }
    pub(super) fn session_line(&self, s: &AgentSession) -> String {
        self.workspace
            .content
            .get(&s.id)
            .map(|c| keepline_core::content::short(&c.headline(), 100))
            .unwrap_or_else(|| "内容尚未索引".into())
    }
    pub(super) fn save_session_task(&mut self, session: &AgentSession, cx: &mut Context<Self>) {
        let Some(c) = self.workspace.content.get(&session.id) else {
            return;
        };
        if c.asks.is_empty() {
            self.error = Some("尾段没有用户原话，不能凭空建立待办".into());
            cx.notify();
            return;
        }
        let task = Task {
            id: id(),
            space: self.nav.space.clone(),
            title: self.session_title(session),
            description: c
                .asks
                .iter()
                .map(|q| q.text.as_str())
                .collect::<Vec<_>>()
                .join("\n\n追加要求："),
            acceptance: vec![],
            agent: session.runtime,
        };
        self.edit(
            Edit::SaveSessionTask {
                session: session.id.clone(),
                task,
            },
            cx,
        );
    }
    /// `in_pane`: the card sits on the session's own pane, not in the overview.
    pub(super) fn here_card(
        &self,
        session: &AgentSession,
        in_pane: bool,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let card = self
            .workspace
            .content
            .get(&session.id)
            .cloned()
            .unwrap_or_default();
        let h = card.here();
        let save = session.clone();
        // Only the body scrolls: the title and × stay put.
        let mut panel = div()
            .id(gpui::SharedString::from(format!(
                "here-body-{}",
                session.id
            )))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(T::S16))
            .pb(px(T::S16))
            .flex()
            .flex_col()
            .gap(px(T::S12));
        // Only fields that say something: no evidence yet is not news, and the guessed
        // next step was the same few template lines for every session.
        let done = (h.done != keepline_core::content::NO_DONE).then_some(h.done);
        for (name, value) in [
            ("你要它", Some(h.ask)),
            ("做成了", done),
            ("停在", Some(h.stopped)),
        ]
        .into_iter()
        .filter_map(|(name, value)| Some((name, value?)))
        {
            panel = panel.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(T::S4))
                    .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(name))
                    .child(div().text_size(px(T::SMALL)).child(value)),
            );
        }
        if card.partial_history {
            panel = panel.child(
                div()
                    .text_size(px(T::META))
                    .text_color(T::color(T::SECONDARY))
                    .child("仅覆盖已索引的历史尾段与之后的增量；没有证据不等于没做。"),
            );
        }
        if let Some(binding) = session.binding.filter(|_| session.current) {
            panel = panel.child(
                div()
                    .text_size(px(T::META))
                    .text_color(T::color(T::SECONDARY))
                    .child(format!("和这个终端对上靠的是：{}", binding.label())),
            );
        }
        let resume = session.clone();
        let command = session.runtime.resume_command(&session.cli_session_id);
        // Two decisions stay in sight: go back to it, and settle it. The rest waits in 更多.
        // On its own live pane there is nothing to go back to: you type in the terminal.
        let typing_here = in_pane && live_terminal(&self.workspace, session).is_some();
        let (settle_name, settle) = if session.outcome == Outcome::Open {
            ("标记完成", Outcome::Completed)
        } else {
            ("撤销结局", Outcome::Open)
        };
        let settle_id = session.id.clone();
        let more = self.overview.here_more.as_ref() == Some(&session.id);
        let more_id = session.id.clone();
        panel = panel.child(
            div()
                .flex()
                .gap(px(T::S6))
                .when(!typing_here, |d| {
                    d.child(
                        button("here-continue", "继续")
                            .bg(T::color(T::ACCENT))
                            .text_color(T::color(T::ON_ACCENT))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_session(&resume, window, cx)
                            })),
                    )
                })
                .child(
                    button(
                        gpui::SharedString::from(format!("here-{settle_name}")),
                        settle_name,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.edit(
                            Edit::SetOutcome {
                                id: settle_id.clone(),
                                outcome: settle.clone(),
                            },
                            cx,
                        )
                    })),
                )
                .child(div().flex_1())
                .child(
                    button("here-more", if more { "收起" } else { "更多 ⋯" })
                        .border_0()
                        .text_color(T::color(T::LINK))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.overview.here_more = (!more).then(|| more_id.clone());
                            cx.notify();
                        })),
                ),
        );
        let mut actions = div().flex().flex_wrap().gap(px(T::S6));
        actions = actions
            .child(button("here-copy", "复制命令").on_click(move |_, _, cx| {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(command.clone()))
            }))
            .child(
                button("save-here-task", "存为待办")
                    .on_click(cx.listener(move |this, _, _, cx| this.save_session_task(&save, cx))),
            );
        let sid = session.id.clone();
        actions = actions.child(button("here-defer", "稍后再看…").on_click(cx.listener(
            move |this, _, window, cx| {
                this.overview.here = Some(sid.clone());
                this.nav.overview = false;
                this.show_form(FormKind::Defer, window, cx);
            },
        )));
        for (name, outcome) in [
            ("标记完成", Outcome::Completed),
            ("放弃", Outcome::Abandoned),
            ("撤销结局", Outcome::Open),
        ]
        .into_iter()
        .filter(|(name, o)| *o != session.outcome && *name != settle_name)
        {
            let sid = session.id.clone();
            actions = actions.child(
                button(gpui::SharedString::from(format!("here-{name}")), name).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.edit(
                            Edit::SetOutcome {
                                id: sid.clone(),
                                outcome: outcome.clone(),
                            },
                            cx,
                        )
                    }),
                ),
            );
        }
        if more {
            panel = panel.child(actions);
        }
        for h in self
            .workspace
            .outcome_history
            .iter()
            .rev()
            .filter(|h| h.session == session.id)
            .take(3)
        {
            panel = panel.child(label(format!(
                "{} · {} → {}",
                h.reason,
                outcome_label(&h.before),
                outcome_label(&h.after)
            )));
        }
        if session.current && session.outcome != Outcome::Open {
            panel = panel.child(label(
                "结局已记录，Agent 仍在运行；若要停止，请在画面菜单选择停止 Agent。",
            ));
        }
        div()
            .id(gpui::SharedString::from(format!(
                "here-card-{}",
                session.id
            )))
            .w(px(T::HERE_WIDTH))
            .max_w_full()
            .max_h(px(T::HERE_HEIGHT))
            .overflow_hidden()
            .rounded(px(T::POPOVER_RADIUS))
            .bg(T::color(T::POPOVER))
            .text_color(T::color(T::PRIMARY))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::CONTROL))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap(px(T::S12))
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(|_, _, cx| cx.stop_propagation())
            // A click anywhere else puts it away; the defer form it opened still needs it.
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                if this.overview.here.is_some() && this.tasks.form.is_none() {
                    this.overview.here = None;
                    cx.notify();
                }
            }))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .justify_between()
                    .items_center()
                    .px(px(T::S16))
                    .pt(px(T::S16))
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("你在这里"),
                    )
                    .child(
                        button("dismiss-here", "×").on_click(cx.listener(|this, _, _, cx| {
                            this.overview.here = None;
                            cx.notify();
                        })),
                    ),
            )
            .child(panel)
    }
    /// A Keepline terminal ran it and is closed now; it resumes in a new pane.
    pub(super) fn session_closed(&self, s: &AgentSession) -> bool {
        s.terminal.is_some() && !self.session_open(s)
    }
    /// Its terminal is running in a tab: the sidebar lists only these.
    pub(super) fn session_open(&self, s: &AgentSession) -> bool {
        live_terminal(&self.workspace, s)
            .or_else(|| resume_terminal(&self.workspace, &self.resuming.resumed, s))
            .is_some_and(|t| self.workspace.terminal_tab(&t.id).is_some())
    }
    pub(super) fn session_hidden(&self, s: &AgentSession) -> Option<&str> {
        match self.workspace.content.get(&s.id) {
            Some(c) => c.hidden_for(s),
            // A hook can bind a new conversation before its first prompt writes anything.
            None if s.current && s.binding.is_some() => None,
            None => Some("缺少交互入口证据"),
        }
    }
    pub(super) fn enter_card(
        &mut self,
        s: &AgentSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.overview.card_menu = None;
        self.overview.filter_space = None;
        self.overview.changed_only = false;
        self.overview.closed_only = false;
        self.overview.show_stale =
            session_group(&self.workspace, s) == Group::Stale || self.overview.show_stale;
        self.overview.show_running =
            session_group(&self.workspace, s) == Group::Running || self.overview.show_running;
        if live_terminal(&self.workspace, s).is_some() {
            self.open_session(s, window, cx);
        } else {
            self.nav.overview = true;
            self.sync_panes(cx);
            self.overview.here = Some(s.id.clone());
            cx.notify();
        }
    }
    pub(super) fn session_actions(
        &self,
        session: &AgentSession,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let save = session.clone();
        let defer = session.id.clone();
        let mut menu = div()
            .id(gpui::SharedString::from(format!("actions-{}", session.id)))
            .w(px(T::MENU_WIDTH))
            .p(px(T::S6))
            .bg(T::color(T::RAISED))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::CONTROL))
            .rounded(px(T::CONTROL_RADIUS))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap(px(T::S4))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                button("card-save", "存为待办").on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.overview.card_menu = None;
                    this.save_session_task(&save, cx);
                })),
            )
            .child(button("card-defer", "稍后再看").on_click(cx.listener(
                move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.overview.card_menu = None;
                    this.overview.here = Some(defer.clone());
                    this.show_form(FormKind::Defer, window, cx);
                },
            )));
        for (label, outcome) in [
            ("标记完成", Outcome::Completed),
            ("放弃", Outcome::Abandoned),
            ("撤销结局", Outcome::Open),
        ]
        .into_iter()
        .filter(|(_, o)| *o != session.outcome)
        {
            let id = session.id.clone();
            menu = menu.child(
                button(gpui::SharedString::from(format!("card-{label}")), label).on_click(
                    cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.overview.card_menu = None;
                        this.edit(
                            Edit::SetOutcome {
                                id: id.clone(),
                                outcome: outcome.clone(),
                            },
                            cx,
                        );
                    }),
                ),
            );
        }
        menu
    }
    pub fn navigate_to(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.overview.cleanup.clear();
        self.tasks.form = None;
        self.search.hide();
        if let Some(session) = self
            .workspace
            .agent_sessions
            .iter()
            .find(|s| s.id == id)
            .cloned()
        {
            self.enter_card(&session, window, cx);
            // Report actual selected layout and focused pane, not a queued route intent.
            if !self.nav.overview
                && let Some(terminal) = live_terminal(&self.workspace, &session)
                && self.nav.focused.as_deref() == Some(terminal.id.as_str())
                && let Some(tab) = self.workspace.terminal_tab(&terminal.id)
                && self.nav.tab.as_deref() == Some(tab.id.as_str())
                && let Some(pane) = self.layout.panes.get(&terminal.id)
                && pane.focus_handle(cx).is_focused(window)
            {
                crate::desktop::route_applied(id, &terminal.id, &tab.id);
            }
        } else {
            self.error = Some("会话尚未载入，请刷新工作台".into());
            self.request(Request::Workspace, cx);
        }
    }
    /// Only Keepline's own terminals ask for attention; scanned-only sessions stay in the overview.
    pub(super) fn needs_you(&self) -> Vec<&AgentSession> {
        self.workspace
            .agent_sessions
            .iter()
            .filter(|s| {
                self.session_hidden(s).is_none()
                    && !self.session_closed(s)
                    && needs_attention(&self.workspace, s)
            })
            .collect()
    }
    pub(super) fn next_waiting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let needs = self.needs_you();
        let current = self
            .nav
            .focused
            .as_ref()
            .and_then(|id| self.workspace.current_agent(id))
            .map(|s| s.id.as_str())
            .or(self.overview.here.as_deref());
        if !needs.is_empty() {
            let index = needs
                .iter()
                .position(|s| Some(s.id.as_str()) == current)
                .map_or(0, |i| (i + 1) % needs.len());
            let session = needs[index].clone();
            self.enter_card(&session, window, cx);
        }
    }
}
