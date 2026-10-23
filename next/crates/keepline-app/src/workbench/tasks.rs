//! Todos: detail, handoff bar, agent picker and the sidebar quick capture.
use super::*;

impl Workbench {
    pub(super) fn task_detail_view(
        &self,
        task: &Task,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let tid = task.id.clone();
        let mut view = div()
            .id("task-detail")
            .w(px(T::DETAIL_WIDTH))
            .max_w_full()
            .m(px(T::S20))
            .bg(T::color(T::HEADER))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::CONTROL))
            .rounded(px(T::POPOVER_RADIUS))
            .overflow_y_scroll()
            .p(px(T::S20))
            .flex()
            .flex_col()
            .gap(px(T::S12))
            .child(div().text_size(px(T::TITLE)).child(task.title.clone()))
            .child(label(format!(
                "{} · {}",
                task.agent.executable(),
                match self
                    .workspace
                    .task_outcomes
                    .get(&tid)
                    .unwrap_or(&Outcome::Open)
                {
                    Outcome::Open => "未完成",
                    Outcome::Completed => "已完成",
                    Outcome::Deferred { .. } => "暂存",
                    Outcome::Abandoned => "已放弃",
                }
            )))
            .child(
                label(
                    self.workspace
                        .spaces
                        .iter()
                        .find(|s| s.id == task.space)
                        .map_or(String::new(), |s| format!("{} · {}", s.name, s.cwd)),
                )
                .truncate(),
            )
            .child(label("说明").mt(px(T::S8)))
            .child(task.description.clone());
        view = view.child(
            label(format!(
                "验收标准 · {} / {} 项有机械证据",
                task.acceptance
                    .iter()
                    .filter(|c| c.evidence.is_some())
                    .count(),
                task.acceptance.len()
            ))
            .mt(px(T::S12)),
        );
        for criterion in &task.acceptance {
            view = view.child(
                div()
                    .p(px(T::S8))
                    .border_b(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::BORDER))
                    .child(format!(
                        "{}  {}",
                        if criterion.evidence.is_some() {
                            "☑"
                        } else {
                            "□"
                        },
                        criterion.description
                    ))
                    .when_some(criterion.evidence.clone(), |d, source| {
                        d.child(label(source))
                    }),
            );
        }
        view = view.child(label("关联会话").mt(px(T::S12)));
        for session in self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| s.task.as_ref() == Some(&tid))
        {
            let session = session.clone();
            view = view.child(
                button(
                    gpui::SharedString::from(format!("task-session-{}", session.id)),
                    "",
                )
                .h(px(T::TASK_ROW))
                .justify_start()
                .gap(px(T::S8))
                .child(dot(self.group(&session)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().truncate().child(self.session_title(&session)))
                        .child(label(format!(
                            "{} · {}",
                            session.runtime.executable(),
                            self.group(&session).label()
                        ))),
                )
                .child(label("打开 →").text_color(T::color(T::LINK)))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.task_detail = None;
                    this.enter_card(&session, window, cx);
                })),
            );
        }
        let mut actions = div().flex().flex_wrap().gap(px(T::S8));
        // Offer only real changes: no 撤销结局 without an outcome, no repeat of the current one.
        for (name, outcome) in [
            ("标记完成", Outcome::Completed),
            ("放弃", Outcome::Abandoned),
            ("撤销结局", Outcome::Open),
        ]
        .into_iter()
        .filter(|(_, o)| {
            o != self
                .workspace
                .task_outcomes
                .get(&task.id)
                .unwrap_or(&Outcome::Open)
        }) {
            let tid = tid.clone();
            actions = actions.child(button(name, name).on_click(cx.listener(
                move |this, _, _, cx| {
                    this.edit(
                        Edit::SetTaskOutcome {
                            id: tid.clone(),
                            outcome: outcome.clone(),
                        },
                        cx,
                    )
                },
            )));
        }
        actions = actions
            .child(button("task-defer", "暂存").on_click(
                cx.listener(|this, _, window, cx| this.show_form(FormKind::TaskDefer, window, cx)),
            ))
            .child(button("task-edit", "编辑").on_click({
                let task = task.clone();
                cx.listener(move |this, _, window, cx| this.edit_task_form(&task, window, cx))
            }))
            .child(
                button("task-another", "再开一个会话")
                    .bg(T::color(T::ACCENT))
                    .text_color(T::color(T::WHITE))
                    .on_click({
                        let tid = tid.clone();
                        cx.listener(move |this, _, _, cx| this.ask_agent(tid.clone(), true, cx))
                    }),
            )
            .child(
                button("task-close", "返回画面").on_click(cx.listener(|this, _, _, cx| {
                    this.task_detail = None;
                    cx.notify();
                })),
            );
        view.child(actions)
            .children(self.agent_picker(&task.id, cx))
    }
    /// The line above a pane that Keepline is starting an Agent in for a task.
    pub(super) fn handoff_bar(
        &self,
        terminal: &str,
        task: &Task,
        state: crate::pane::HandoffState,
        cx: &Context<Self>,
    ) -> gpui::Div {
        use crate::pane::HandoffState;
        let terminal = terminal.to_owned();
        let agent = self
            .workspace
            .current_agent(&terminal)
            .map(|s| s.runtime)
            .unwrap_or(task.agent);
        let message = match state {
            HandoffState::Waiting => format!(
                "为待办「{}」启动 {}，就绪后放入待办内容",
                task.title,
                agent_name(agent)
            ),
            HandoffState::Placed => "已放入待办内容，检查后在 Agent 里按回车发送".to_owned(),
            HandoffState::NotReady => "Agent 还没准备好，内容没有放进去".to_owned(),
            HandoffState::Missing => format!(
                "找不到 {} 命令，内容没有放进去。先安装它或把它加进 PATH，再点「再试一次」",
                agent_name(agent)
            ),
        };
        let stuck = matches!(state, HandoffState::NotReady | HandoffState::Missing);
        let text = task.prompt();
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(T::S6))
            .p(px(T::S6))
            .bg(T::color(T::TODO))
            .child(label(message).text_color(if stuck {
                Group::Waiting.colors().1
            } else {
                T::color(T::SECONDARY)
            }))
            .when(task.acceptance.is_empty(), |d| {
                let task = task.clone();
                d.child(label("还没写验收标准").text_color(Group::Waiting.colors().1))
                    .child(button("handoff-criteria", "补写").on_click(cx.listener(
                        move |this, _, window, cx| this.edit_task_form(&task, window, cx),
                    )))
            })
            .when(stuck, |d| {
                let retry = terminal.clone();
                d.child(button("handoff-retry", "再试一次").on_click(cx.listener(
                    move |this, _, _, cx| {
                        if let Some(p) = this.panes.get(&retry) {
                            p.update(cx, |p, cx| p.retry_handoff(cx));
                        }
                    },
                )))
            })
            .child(
                button("handoff-copy", "复制内容").on_click(cx.listener(move |_, _, _, cx| {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone()))
                })),
            )
            .child(button("handoff-dismiss", "收起").on_click({
                let terminal = terminal.clone();
                cx.listener(move |this, _, _, cx| {
                    if let Some(p) = this.panes.get(&terminal) {
                        p.update(cx, |p, cx| p.dismiss_handoff(cx));
                    }
                })
            }))
            .when(self.workspace.current_agent(&terminal).is_none(), |d| {
                d.child(button("handoff-unlink", "不关联待办").on_click(cx.listener(
                    move |this, _, _, cx| {
                        if this.workspace.current_agent(&terminal).is_none() {
                            this.edit(
                                Edit::SetPendingTask {
                                    terminal: terminal.clone(),
                                    task: None,
                                },
                                cx,
                            );
                        }
                    },
                )))
            })
    }
    /// 开始 asks which Agent first, unless the task already has a pane to return to.
    pub(super) fn ask_agent(&mut self, task: String, another: bool, cx: &mut Context<Self>) {
        if !another && self.workspace.task_terminal(&task).is_some() {
            self.launch(false, Some(task), cx);
            return;
        }
        self.picking = Some((task, another));
        cx.notify();
    }
    pub(super) fn choose_agent(&mut self, agent: Agent, cx: &mut Context<Self>) {
        let Some((task, another)) = self.picking.take() else {
            return;
        };
        self.last_agent = Some(agent);
        self.chosen_agent = Some(agent);
        let tab = self
            .workspace
            .tabs
            .iter()
            .find(|t| t.default_task.as_ref() == Some(&task))
            .map(|t| t.id.clone());
        match tab.filter(|_| another) {
            Some(tab) => {
                self.tab = Some(tab);
                self.launch(true, Some(task), cx);
            }
            None => self.launch(false, Some(task), cx),
        }
    }
    pub(super) fn agent_picker(&self, task: &str, cx: &Context<Self>) -> Option<gpui::Div> {
        if self.picking.as_ref().map(|(t, _)| t.as_str()) != Some(task) {
            return None;
        }
        let first = self
            .last_agent
            .unwrap_or(self.workspace.settings.default_agent);
        let order = if first == Agent::Codex {
            [Agent::Codex, Agent::Claude]
        } else {
            [Agent::Claude, Agent::Codex]
        };
        let mut row = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(T::S4))
            .px(px(T::S8))
            .pb(px(T::S6))
            .child(
                label(format!(
                    "「{}」用哪个 Agent",
                    self.workspace
                        .tasks
                        .iter()
                        .find(|t| t.id == task)
                        .map(|t| keepline_core::content::short(&t.title, 12))
                        .unwrap_or_default()
                ))
                .text_size(px(T::META)),
            );
        for agent in order {
            row = row.child(
                button(
                    gpui::SharedString::from(format!("pick-{}-{task}", agent.executable())),
                    agent_name(agent),
                )
                .h(px(T::ICON_HEIGHT))
                .px(px(T::S6))
                .text_size(px(T::META))
                .when(agent == first, |b| {
                    b.bg(T::color(T::ACCENT)).text_color(T::color(T::WHITE))
                })
                .on_click(cx.listener(move |this, _, _, cx| this.choose_agent(agent, cx))),
            );
        }
        Some(
            row.child(
                button(
                    gpui::SharedString::from(format!("pick-cancel-{task}")),
                    "取消",
                )
                .h(px(T::ICON_HEIGHT))
                .px(px(T::S6))
                .text_size(px(T::META))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.picking = None;
                    cx.notify();
                })),
            ),
        )
    }
    pub(super) fn edit_task_form(
        &mut self,
        task: &Task,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_form(FormKind::Task, window, cx);
        self.editing_task = Some(task.id.clone());
        if let Some(f) = &mut self.form {
            f.space = task.space.clone();
            f.agent = task.agent;
            for (field, value) in f.fields.iter().zip([
                task.title.clone(),
                task.description.clone(),
                task.acceptance
                    .iter()
                    .map(|c| c.description.clone())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ]) {
                field.update(cx, |f, cx| {
                    f.set_text(&value);
                    cx.notify();
                });
            }
        }
    }
    pub(super) fn show_quick_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.overview = false;
        self.form = None;
        self.task_detail = None;
        self.sync_panes(cx);
        let field = cx.new(|cx| Field::new("要做什么（回车保存，Esc 取消）", "", cx).compact());
        let subscription = cx.subscribe(&field, |this, _, event, cx| match event {
            crate::field::FieldEvent::Submit => this.save_quick_task(false, cx),
            crate::field::FieldEvent::Cancel => {
                this.quick_task = None;
                cx.notify();
            }
        });
        window.focus(&field.focus_handle(cx), cx);
        self.quick_task = Some((field, subscription));
        cx.notify();
    }
    /// Only the title is needed to capture; the space and Agent come from where you are.
    pub(super) fn save_quick_task(&mut self, start: bool, cx: &mut Context<Self>) {
        let Some((field, _)) = &self.quick_task else {
            return;
        };
        let title = field.read(cx).text.trim().to_owned();
        if title.is_empty() {
            self.error = Some("先写要做什么".into());
            cx.notify();
            return;
        }
        let task = Task {
            id: id(),
            space: self.space.clone(),
            title,
            description: String::new(),
            acceptance: vec![],
            agent: self.workspace.settings.default_agent,
        };
        if start {
            self.start_after_save = Some(task.id.clone());
        }
        self.quick_task = None;
        self.edit(Edit::AddTask(task), cx);
    }
    pub(super) fn expand_quick_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = self
            .quick_task
            .take()
            .map(|(f, _)| f.read(cx).text.clone())
            .unwrap_or_default();
        self.show_form(FormKind::Task, window, cx);
        if let Some(field) = self.form.as_ref().and_then(|f| f.fields.first()) {
            field.update(cx, |f, cx| {
                f.set_text(&title);
                cx.notify();
            });
        }
    }
    pub(super) fn quick_task_row(&self, cx: &Context<Self>) -> Option<gpui::Div> {
        let (field, _) = self.quick_task.as_ref()?;
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(T::S6))
                .px(px(T::S8))
                .pb(px(T::S8))
                .child(field.clone())
                .child(
                    div()
                        .flex()
                        .gap(px(T::S4))
                        .child(
                            button("quick-save-start", "保存并开始")
                                .h(px(T::ICON_HEIGHT))
                                .px(px(T::S6))
                                .text_size(px(T::META))
                                .bg(T::color(T::ACCENT))
                                .text_color(T::color(T::WHITE))
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.save_quick_task(true, cx)),
                                ),
                        )
                        .child(
                            button("quick-save", "保存")
                                .h(px(T::ICON_HEIGHT))
                                .px(px(T::S6))
                                .text_size(px(T::META))
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.save_quick_task(false, cx)),
                                ),
                        )
                        .child(
                            button("quick-expand", "展开")
                                .h(px(T::ICON_HEIGHT))
                                .px(px(T::S6))
                                .text_size(px(T::META))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.expand_quick_task(window, cx)
                                })),
                        )
                        .child(
                            button("quick-cancel", "取消")
                                .h(px(T::ICON_HEIGHT))
                                .px(px(T::S6))
                                .text_size(px(T::META))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.quick_task = None;
                                    cx.notify();
                                })),
                        ),
                ),
        )
    }
}
