//! Todos: detail, handoff bar, agent picker and the sidebar quick capture.
use super::*;

/// Todo editing: the open form, the sidebar quick capture and the Agent choice.
pub(super) struct TaskUi {
    pub(super) chosen_agent: Option<Agent>,
    /// The task whose 开始 is waiting for an Agent choice; true when it is "再开一个会话".
    pub(super) picking: Option<(String, bool)>,
    pub(super) last_agent: Option<Agent>,
    pub(super) form: Option<Form>,
    /// One-line capture in the sidebar; the full form is its "展开".
    pub(super) quick_task: Option<(Entity<Field>, gpui::Subscription)>,
    /// The sidebar's todo list starts folded; sessions come first.
    pub(super) todos_open: bool,
    pub(super) editing_task: Option<String>,
    pub(super) start_after_save: Option<String>,
    /// Agent commands the login shell finds; None until asked, or when it cannot tell.
    pub(super) found: Option<HashSet<String>>,
    pub(super) probing: bool,
    /// Criteria whose older evidence is unfolded in the todo detail.
    pub(super) expanded: HashSet<String>,
    /// The criterion whose 「挂一条证据」 list is open.
    pub(super) attaching: Option<String>,
}
impl Workbench {
    /// The line above a pane that Keepline is starting an Agent in for a task.
    pub(super) fn handoff_bar(
        &self,
        terminal: &str,
        task: &Task,
        state: crate::terminal::pane::HandoffState,
        cx: &Context<Self>,
    ) -> gpui::Div {
        use crate::terminal::pane::HandoffState;
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
                agent.name()
            ),
            HandoffState::Placed => "已放入待办内容，检查后在 Agent 里按回车发送".to_owned(),
            HandoffState::NotReady => "Agent 还没准备好，内容没有放进去".to_owned(),
            HandoffState::Missing => format!(
                "找不到 {} 命令，内容没有放进去。先安装它或把它加进 PATH，再点「再试一次」",
                agent.name()
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
            .bg(T::color(T::todo()))
            .child(label(message).text_color(if stuck {
                Group::Waiting.colors().1
            } else {
                T::color(T::secondary())
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
                        if let Some(p) = this.layout.panes.get(&retry) {
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
                    if let Some(p) = this.layout.panes.get(&terminal) {
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
        self.tasks.picking = Some((task, another));
        cx.notify();
    }
    pub(super) fn choose_agent(&mut self, agent: Agent, cx: &mut Context<Self>) {
        let Some((task, another)) = self.tasks.picking.take() else {
            return;
        };
        self.tasks.last_agent = Some(agent);
        self.tasks.chosen_agent = Some(agent);
        let tab = self
            .workspace
            .tabs
            .iter()
            .find(|t| t.default_task.as_ref() == Some(&task))
            .map(|t| t.id.clone());
        match tab.filter(|_| another) {
            Some(tab) => {
                self.nav.tab = Some(tab);
                self.sync_panes(cx);
                self.launch(true, Some(task), cx);
            }
            None => self.launch(false, Some(task), cx),
        }
    }
    pub(super) fn agent_picker(&self, task: &str, cx: &Context<Self>) -> Option<gpui::Div> {
        if self.tasks.picking.as_ref().map(|(t, _)| t.as_str()) != Some(task) {
            return None;
        }
        let first = self
            .tasks
            .last_agent
            .unwrap_or(self.workspace.settings.default_agent);
        let order =
            std::iter::once(first).chain(Agent::all().into_iter().filter(move |a| *a != first));
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
                    agent.name(),
                )
                .h(px(T::ICON_HEIGHT))
                .px(px(T::S6))
                .text_size(px(T::META))
                .when(agent == first, |b| {
                    b.bg(T::color(T::accent()))
                        .text_color(T::color(T::on_accent()))
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
                    this.tasks.picking = None;
                    cx.notify();
                })),
            ),
        )
    }
    pub(super) fn show_quick_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.nav.overview = false;
        self.tasks.todos_open = true;
        self.tasks.form = None;
        self.nav.task_detail = None;
        self.sync_panes(cx);
        let field = cx.new(|cx| Field::new("要做什么（回车保存，Esc 取消）", "", cx).compact());
        let subscription = cx.subscribe(&field, |this, _, event, cx| match event {
            crate::ui::field::FieldEvent::Submit => this.save_quick_task(false, cx),
            crate::ui::field::FieldEvent::Cancel => {
                this.tasks.quick_task = None;
                cx.notify();
            }
        });
        window.focus(&field.focus_handle(cx), cx);
        self.tasks.quick_task = Some((field, subscription));
        cx.notify();
    }
    /// Only the title is needed to capture; the space and Agent come from where you are.
    pub(super) fn save_quick_task(&mut self, start: bool, cx: &mut Context<Self>) {
        let Some((field, _)) = &self.tasks.quick_task else {
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
            space: self.nav.space.clone(),
            title,
            description: String::new(),
            acceptance: vec![],
            agent: self.workspace.settings.default_agent,
        };
        if start {
            self.tasks.start_after_save = Some(task.id.clone());
        }
        self.tasks.quick_task = None;
        self.edit(Edit::AddTask(task), cx);
    }
    pub(super) fn expand_quick_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = self
            .tasks
            .quick_task
            .take()
            .map(|(f, _)| f.read(cx).text.clone())
            .unwrap_or_default();
        self.show_form(FormKind::Task, window, cx);
        if let Some(field) = self.tasks.form.as_ref().and_then(|f| f.fields.first()) {
            field.update(cx, |f, cx| {
                f.set_text(&title);
                cx.notify();
            });
        }
    }
    pub(super) fn quick_task_row(&self, cx: &Context<Self>) -> Option<gpui::Div> {
        let (field, _) = self.tasks.quick_task.as_ref()?;
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
                                .bg(T::color(T::accent()))
                                .text_color(T::color(T::on_accent()))
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
                                    this.tasks.quick_task = None;
                                    cx.notify();
                                })),
                        ),
                ),
        )
    }
}
