//! Session grouping, the 「你在这里」 card and navigation into a session.
use super::*;

impl Workbench {
    pub(super) fn open_session(
        &mut self,
        session: &AgentSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.search_open = false;
        self.task_detail = None;
        self.here = None;
        self.zoomed = None;
        let Some(terminal) = live_terminal(&self.workspace, session) else {
            if let Some(space) = session_space(&self.workspace, session) {
                self.space = space.into();
            }
            self.resume = Some((
                session.runtime.resume_command(&session.cli_session_id),
                session.cwd.clone(),
            ));
            self.launch(false, session.task.clone(), cx);
            return;
        };
        let terminal_id = terminal.id.clone();
        self.focused = Some(terminal_id.clone());
        self.pending_focus = true;
        if let Some(tab) = self.workspace.terminal_tab(&terminal.id) {
            self.space = tab.space.clone();
            self.tab = Some(tab.id.clone());
            self.overview = false;
            if tab.closed {
                let mut tab = tab.clone();
                tab.closed = false;
                self.edit(Edit::PutTab { tab }, cx);
            } else {
                self.sync_panes(cx);
                if let Some(pane) = self.panes.get(&terminal_id) {
                    window.focus(&pane.focus_handle(cx), cx);
                }
                cx.notify();
            }
        } else {
            let tab = Tab {
                id: id(),
                space: self.space.clone(),
                default_task: session.task.clone(),
                title: "恢复视图".into(),
                closed: false,
                layout: Some(Layout::Leaf(terminal_id.clone())),
            };
            self.tab = Some(tab.id.clone());
            self.overview = false;
            self.edit(Edit::PutTab { tab }, cx);
        }
    }
    pub(super) fn group(&self, s: &AgentSession) -> Group {
        use keepline_core::content::{Content, Inbox};
        match self
            .workspace
            .content
            .get(&s.id)
            .cloned()
            .unwrap_or_else(|| Content {
                updated: s.updated,
                ..Default::default()
            })
            .group(s, &self.workspace, now())
        {
            Inbox::Waiting => Group::Waiting,
            Inbox::Error => Group::Error,
            Inbox::Review => Group::Review,
            Inbox::Running => Group::Running,
            Inbox::Stale => Group::Stale,
            _ => Group::Paused,
        }
    }
    pub(super) fn session_title(&self, s: &AgentSession) -> String {
        if let Some(c) = self
            .workspace
            .content
            .get(&s.id)
            .filter(|c| !c.title.is_empty())
        {
            return c.title.clone();
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
            .unwrap_or_else(|| format!("{} · {}", s.runtime.executable(), s.cli_session_id))
    }
    pub(super) fn session_line(&self, s: &AgentSession) -> String {
        self.workspace
            .content
            .get(&s.id)
            .map(|c| {
                keepline_core::content::short(
                    &c.here()
                        .stopped
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                    100,
                )
            })
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
            space: self.space.clone(),
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
    pub(super) fn here_card(
        &self,
        session: &AgentSession,
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
        let mut panel = div()
            .id(gpui::SharedString::from(format!(
                "here-card-{}",
                session.id
            )))
            .w(px(T::HERE_WIDTH))
            .max_w_full()
            .max_h(px(T::HERE_HEIGHT))
            .overflow_y_scroll()
            .p(px(T::S16))
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
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("你在这里"),
                    )
                    .child(
                        button("dismiss-here", "×").on_click(cx.listener(|this, _, _, cx| {
                            this.here = None;
                            cx.notify();
                        })),
                    ),
            );
        for (name, value) in [
            ("你要它", h.ask),
            ("做成了", h.done),
            ("停在", h.stopped),
            ("下一步·推断", h.next),
        ] {
            panel = panel.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(T::S4))
                    .when(name == "下一步·推断", |d| {
                        d.bg(T::color(T::INFERENCE)).p(px(T::S8))
                    })
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
        let resume = session.clone();
        let command = session.runtime.resume_command(&session.cli_session_id);
        // Two decisions stay in sight: go back to it, and settle it. The rest waits in 更多.
        let (settle_name, settle) = if session.outcome == Outcome::Open {
            ("已验收", Outcome::Completed)
        } else {
            ("撤销结局", Outcome::Open)
        };
        let settle_id = session.id.clone();
        let more = self.here_more.as_ref() == Some(&session.id);
        let more_id = session.id.clone();
        panel = panel.child(
            div()
                .flex()
                .gap(px(T::S6))
                .child(
                    button("here-continue", "继续")
                        .bg(T::color(T::ACCENT))
                        .text_color(T::color(T::ON_ACCENT))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_session(&resume, window, cx)
                        })),
                )
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
                            this.here_more = (!more).then(|| more_id.clone());
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
        actions = actions.child(button("here-defer", "暂存…").on_click(cx.listener(
            move |this, _, window, cx| {
                this.here = Some(sid.clone());
                this.overview = false;
                this.show_form(FormKind::Defer, window, cx);
            },
        )));
        for (name, outcome) in [
            ("已验收", Outcome::Completed),
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
        panel
    }
    pub(super) fn session_hidden(&self, s: &AgentSession) -> Option<&str> {
        self.workspace
            .content
            .get(&s.id)
            .map_or(Some("缺少交互入口证据"), |c| c.hidden_for(s))
    }
    pub(super) fn enter_card(
        &mut self,
        s: &AgentSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.card_menu = None;
        self.filter_space = None;
        self.changed_only = false;
        self.show_stale = self.group(s) == Group::Stale || self.show_stale;
        self.show_running = self.group(s) == Group::Running || self.show_running;
        if live_terminal(&self.workspace, s).is_some() {
            self.open_session(s, window, cx);
        } else {
            self.overview = true;
            self.sync_panes(cx);
            self.here = Some(s.id.clone());
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
                    this.card_menu = None;
                    this.save_session_task(&save, cx);
                })),
            )
            .child(button("card-defer", "暂存").on_click(cx.listener(
                move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.card_menu = None;
                    this.here = Some(defer.clone());
                    this.show_form(FormKind::Defer, window, cx);
                },
            )));
        for (label, outcome) in [
            ("已验收", Outcome::Completed),
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
                        this.card_menu = None;
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
        self.cleanup.clear();
        self.form = None;
        self.search_open = false;
        if let Some(session) = self
            .workspace
            .agent_sessions
            .iter()
            .find(|s| s.id == id)
            .cloned()
        {
            self.enter_card(&session, window, cx);
            // Report actual selected layout and focused pane, not a queued route intent.
            if !self.overview
                && let Some(terminal) = live_terminal(&self.workspace, &session)
                && self.focused.as_deref() == Some(terminal.id.as_str())
                && let Some(tab) = self.workspace.terminal_tab(&terminal.id)
                && !tab.closed
                && self.tab.as_deref() == Some(tab.id.as_str())
                && let Some(pane) = self.panes.get(&terminal.id)
                && pane.focus_handle(cx).is_focused(window)
            {
                crate::desktop::route_applied(id, &terminal.id, &tab.id);
            }
        } else {
            self.error = Some("会话尚未载入，请刷新工作台".into());
            self.request(Request::Workspace, cx);
        }
    }
}
