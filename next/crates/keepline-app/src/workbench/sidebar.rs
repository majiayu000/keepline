//! The left column: spaces, the current space's todos and the "needs you" list.
use super::*;

impl Workbench {
    pub(super) fn sidebar_view(
        &self,
        space_name: &str,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let mut sidebar = div()
            .id("sidebar")
            .w(px(T::SIDEBAR))
            .flex_shrink_0()
            .h_full()
            .overflow_y_scroll()
            .px(px(T::S8))
            .py(px(T::S12))
            .flex()
            .flex_col()
            .gap(px(T::S2))
            .bg(T::color(T::PANEL))
            .border_r(px(T::BORDER_WIDTH))
            .border_color(T::color(T::BORDER))
            .child(label("空间").px(px(T::S8)).mb(px(T::S4)));
        for space in &self.workspace.spaces {
            let id = space.id.clone();
            let active = self.space == id;
            let sessions: Vec<_> = self
                .workspace
                .agent_sessions
                .iter()
                .filter(|s| {
                    self.session_hidden(s).is_none()
                        && session_space(&self.workspace, s) == Some(id.as_str())
                })
                .collect();
            let count = sessions.len();
            let waiting = sessions
                .iter()
                .filter(|s| {
                    s.managed() && s.outcome.needs_review(now()) && self.group(s) == Group::Waiting
                })
                .count();
            let errors = sessions
                .iter()
                .filter(|s| {
                    s.managed() && s.outcome.needs_review(now()) && self.group(s) == Group::Error
                })
                .count();
            sidebar = sidebar.child(
                button(gpui::SharedString::from(format!("space-{id}")), "")
                    .aria_label(space.name.clone())
                    .gap(px(T::S8))
                    .child(dot(if errors > 0 {
                        Group::Error
                    } else if waiting > 0 {
                        Group::Waiting
                    } else {
                        sessions
                            .iter()
                            .filter(|s| s.outcome.needs_review(now()))
                            .map(|s| self.group(s))
                            .find(|g| !matches!(g, Group::Waiting | Group::Error))
                            .unwrap_or(Group::Stale)
                    }))
                    .child(div().min_w_0().truncate().child(space.name.clone()))
                    .justify_start()
                    .px(px(T::S8))
                    .border_0()
                    .when(active, |s| s.bg(T::color(T::RAISED)))
                    .child(div().flex_1())
                    .when(waiting > 0, |d| d.child(badge(Group::Waiting, waiting)))
                    .when(errors > 0, |d| d.child(badge(Group::Error, errors)))
                    .child(label(format!("共 {count}")).text_size(px(T::CHIP_SIZE)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.space = id.clone();
                        this.tab = this
                            .workspace
                            .tabs
                            .iter()
                            .find(|t| t.space == id && !t.closed)
                            .map(|t| t.id.clone());
                        this.sync_panes(cx);
                        cx.notify();
                    })),
            );
        }
        sidebar = sidebar
            .child(
                button("add-space", "＋ 空间")
                    .border_0()
                    .justify_start()
                    .text_color(T::color(T::MUTED))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.show_form(FormKind::Space, window, cx)
                    })),
            )
            .child(
                div()
                    .mt(px(T::S14))
                    .pt(px(T::S12))
                    .px(px(T::S8))
                    .mb(px(T::S4))
                    .flex()
                    .items_center()
                    .child(label(format!("{space_name} 的待办")).flex_1())
                    .child(
                        button("sidebar-new-task", "＋")
                            .aria_label("新待办")
                            .border_0()
                            .text_color(T::color(T::LINK))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.show_quick_task(window, cx)),
                            ),
                    ),
            )
            .children(self.quick_task_row(cx));
        // First open: say what the empty pane is for before asking for a task.
        if self.quick_task.is_none() && !self.workspace.tasks.iter().any(|t| t.space == self.space)
        {
            sidebar = sidebar.child(
            div()
                .px(px(T::S8))
                .flex()
                .flex_col()
                .items_start()
                .gap(px(T::S6))
                .child(label(
                    "还没有待办。可以直接在右边的终端里运行 codex 或 claude，Keepline 会自动跟踪；也可以先记一条待办。",
                ))
                .child(
                    button("empty-new-task", "＋ 新建待办")
                        .h(px(T::ICON_HEIGHT))
                        .px(px(T::S6))
                        .text_size(px(T::META))
                        .text_color(T::color(T::LINK))
                        .on_click(
                            cx.listener(|this, _, window, cx| this.show_quick_task(window, cx)),
                        ),
                ),
        );
        }
        for task in self
            .workspace
            .tasks
            .iter()
            .filter(|t| t.space == self.space)
        {
            let tid = task.id.clone();
            let start = tid.clone();
            let count = self
                .workspace
                .agent_sessions
                .iter()
                .filter(|s| s.task.as_ref() == Some(&tid))
                .count();
            let outcome = self
                .workspace
                .task_outcomes
                .get(&tid)
                .unwrap_or(&Outcome::Open);
            let state = if *outcome == Outcome::Open {
                if count == 0 {
                    "未开始".into()
                } else {
                    format!("未完成 · {count} 个会话")
                }
            } else {
                outcome_label(outcome)
            };
            sidebar = sidebar.child(
                div()
                    .id(gpui::SharedString::from(format!("task-{tid}")))
                    .h(px(T::TASK_ROW))
                    .px(px(T::S8))
                    .flex()
                    .items_center()
                    .gap(px(T::S8))
                    .rounded(px(T::CONTROL_RADIUS))
                    .cursor_pointer()
                    .hover(|s| s.bg(T::color(T::RAISED)))
                    .child(
                        div()
                            .size(px(T::S8))
                            .flex_shrink_0()
                            .rounded_full()
                            .border(px(T::BORDER_WIDTH))
                            .border_color(T::color(T::LINK))
                            .when(count > 0, |d| d.bg(T::color(T::ACCENT))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(T::MONO_SIZE))
                                    .child(task.title.clone()),
                            )
                            .child(label(state).text_size(px(T::CHIP_SIZE))),
                    )
                    .child(
                        button(
                            gpui::SharedString::from(format!("start-task-{tid}")),
                            "开始",
                        )
                        .h(px(T::ICON_HEIGHT))
                        .px(px(T::S6))
                        .text_size(px(T::META))
                        .text_color(T::color(T::LINK))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.ask_agent(start.clone(), false, cx);
                        })),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.task_detail = Some(tid.clone());
                        cx.notify();
                    })),
            );
            if let Some(picker) = self.agent_picker(&task.id, cx) {
                sidebar = sidebar.child(picker);
            }
        }
        let needs = self.needs_you();
        sidebar = sidebar.child(
            label(format!("需要你 · {}", needs.len()))
                .mt(px(T::S14))
                .pt(px(T::S12))
                .px(px(T::S8))
                .mb(px(T::S4)),
        );
        for session in needs.into_iter().take(6) {
            let open = session.clone();
            let g = self.group(session);
            sidebar = sidebar.child(
                div()
                    .id(gpui::SharedString::from(format!("need-{}", session.id)))
                    .h(px(T::ROW))
                    .px(px(T::S8))
                    .flex()
                    .items_center()
                    .gap(px(T::S8))
                    .rounded(px(T::CONTROL_RADIUS))
                    .cursor_pointer()
                    .hover(|s| s.bg(T::color(T::RAISED)))
                    .child(dot(g))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(T::MONO_SIZE))
                            .child(self.session_title(session)),
                    )
                    .child(
                        label(
                            session_space(&self.workspace, session)
                                .and_then(|id| self.workspace.spaces.iter().find(|p| p.id == id))
                                .map(|p| p.name.clone())
                                .unwrap_or_default(),
                        )
                        .text_size(px(T::CHIP_SIZE))
                        .max_w(px(T::SIDEBAR / T::S4))
                        .truncate(),
                    )
                    // Straight to its pane; an exited one resumes in a new pane.
                    .on_click(
                        cx.listener(move |this, _, window, cx| {
                            this.open_session(&open, window, cx)
                        }),
                    ),
            );
        }
        sidebar
    }
}
