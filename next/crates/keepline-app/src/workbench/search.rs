//! Top-bar search, filters and the "needs you" queue.
use super::*;

impl Workbench {
    pub(super) fn settings_button(&self, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        button("settings", "⚙")
            .aria_label("设置")
            .w(px(T::ROW))
            .px(px(T::S4))
            .on_click(
                cx.listener(|this, _, window, cx| this.show_form(FormKind::Settings, window, cx)),
            )
    }
    pub(super) fn matches_filter(&self, s: &AgentSession) -> bool {
        self.filter_space
            .as_deref()
            .is_none_or(|space| session_space(&self.workspace, s) == Some(space))
            && (!self.changed_only || changed_since(&self.workspace, s, self.since))
    }
    pub(super) fn search_button(&self, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        div()
            .id("global-search")
            .w(px(T::SEARCH_WIDTH))
            .min_w(px(T::SEARCH_MIN))
            .child(self.search.clone())
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.search_open = true;
                    window.focus(&this.search.focus_handle(cx), cx);
                    cx.notify();
                }),
            )
    }
    pub(super) fn navigation_filters(&self, cx: &Context<Self>) -> gpui::Div {
        let mut row = div().flex().flex_wrap().gap(px(T::S8)).child(
            button(
                "all-spaces",
                if self.filter_space.is_none() {
                    "● 全部空间"
                } else {
                    "全部空间"
                },
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.filter_space = None;
                cx.notify();
            })),
        );
        for space in &self.workspace.spaces {
            let id = space.id.clone();
            row = row.child(
                button(
                    gpui::SharedString::from(format!("filter-{id}")),
                    format!(
                        "{}{}",
                        if self.filter_space.as_ref() == Some(&id) {
                            "● "
                        } else {
                            ""
                        },
                        space.name
                    ),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.filter_space = Some(id.clone());
                    cx.notify();
                })),
            );
        }
        row.child(
            button(
                "only-changed",
                if self.changed_only {
                    "✓ 只看有变化"
                } else {
                    "只看有变化"
                },
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.changed_only = !this.changed_only;
                cx.notify();
            })),
        )
    }
    pub(super) fn search_view(&self, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        let query = self.search.read(cx).text.trim().to_lowercase();
        let mut view = div()
            .id("search-results")
            .size_full()
            .overflow_y_scroll()
            .p(px(T::S20))
            .flex()
            .flex_col()
            .gap(px(T::S12))
            .child(
                div()
                    .flex()
                    .gap(px(T::S12))
                    .child(div().flex_1().child(self.search.clone()))
                    .child(button("close-search", "关闭搜索").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.search_open = false;
                            cx.notify();
                        },
                    ))),
            );
        if query.is_empty() {
            return view.child(label(
                "按标题、项目或用户原话查找；搜索只使用本地已索引内容。",
            ));
        }
        let mut count = 0;
        for task in &self.workspace.tasks {
            if !format!("{} {}", task.title, task.description)
                .to_lowercase()
                .contains(&query)
            {
                continue;
            }
            let id = task.id.clone();
            let space = task.space.clone();
            count += 1;
            view = view.child(
                button(
                    gpui::SharedString::from(format!("search-task-{id}")),
                    format!("待办 · {}", task.title),
                )
                .justify_start()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.search_open = false;
                    this.overview = false;
                    this.space = space.clone();
                    this.task_detail = Some(id.clone());
                    cx.notify();
                })),
            );
        }
        for session in &self.workspace.agent_sessions {
            if self.session_hidden(session).is_some() {
                continue;
            }
            let quote = self
                .workspace
                .content
                .get(&session.id)
                .and_then(|c| {
                    c.asks
                        .iter()
                        .find(|q| q.text.to_lowercase().contains(&query))
                })
                .map(|q| q.text.clone());
            if quote.is_none()
                && !format!(
                    "{} {} {}",
                    self.session_title(session),
                    session.cli_session_id,
                    session.cwd
                )
                .to_lowercase()
                .contains(&query)
            {
                continue;
            }
            let open = session.clone();
            count += 1;
            let snippet = quote
                .map(|q| q.chars().take(160).collect::<String>())
                .unwrap_or_else(|| session.cwd.clone());
            view = view.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(T::S4))
                    .child(
                        button(
                            gpui::SharedString::from(format!("search-session-{}", session.id)),
                            format!("会话 · {}", self.session_title(session)),
                        )
                        .justify_start()
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.search_open = false;
                                this.enter_card(&open, window, cx);
                            },
                        )),
                    )
                    .child(label(snippet)),
            );
        }
        view.child(label(format!(
            "共 {count} 项；未索引历史不会出现在搜索中。"
        )))
    }
    /// Only Keepline's own terminals ask for attention; scanned-only sessions stay in the overview.
    pub(super) fn needs_you(&self) -> Vec<&AgentSession> {
        self.workspace
            .agent_sessions
            .iter()
            .filter(|s| {
                s.managed()
                    && s.outcome.needs_review(now())
                    && self.session_hidden(s).is_none()
                    && matches!(self.group(s), Group::Waiting | Group::Error)
            })
            .collect()
    }
    pub(super) fn next_waiting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let needs = self.needs_you();
        let current = self
            .focused
            .as_ref()
            .and_then(|id| self.workspace.current_agent(id))
            .map(|s| s.id.as_str())
            .or(self.here.as_deref());
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
