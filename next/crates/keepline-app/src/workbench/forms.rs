//! The editor form: todo, space and defer.
use super::*;

impl Workbench {
    pub(super) fn show_form(
        &mut self,
        kind: FormKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.nav.overview = false;
        self.tasks.editing_task = None;
        self.sync_panes(cx);
        let editing = self
            .tasks
            .editing_space
            .as_ref()
            .and_then(|id| self.workspace.spaces.iter().find(|s| &s.id == id))
            .cloned();
        let fields = match kind {
            FormKind::Space => {
                let (name, cwd) = editing.map_or_else(Default::default, |s| (s.name, s.cwd));
                vec![
                    cx.new(|cx| Field::new("空间名称", &name, cx)),
                    cx.new(|cx| Field::new("工作目录（绝对路径）", &cwd, cx)),
                ]
            }
            FormKind::Defer | FormKind::TaskDefer => vec![
                cx.new(|cx| Field::new("为什么稍后再看（必填）", "", cx)),
                cx.new(|cx| Field::new("多少小时后再看（必填）", "24", cx)),
            ],
            FormKind::Task => vec![
                cx.new(|cx| Field::new("要做什么", "", cx)),
                cx.new(|cx| Field::new("说明", "", cx)),
                cx.new(|cx| Field::new("验收标准（每行一条）", "", cx)),
            ],
        };
        window.focus(&fields[0].focus_handle(cx), cx);
        self.tasks.form = Some(Form {
            kind,
            fields,
            agent: self.workspace.settings.default_agent,
            space: self.nav.space.clone(),
        });
        cx.notify();
    }
    pub(super) fn save_form(&mut self, start: bool, cx: &mut Context<Self>) {
        let Some(form) = self.tasks.form.take() else {
            return;
        };
        let values: Vec<String> = form
            .fields
            .iter()
            .map(|f| f.read(cx).text.clone())
            .collect();
        let edit = match form.kind {
            FormKind::Defer | FormKind::TaskDefer => {
                let hours = values[1]
                    .trim()
                    .parse::<u64>()
                    .ok()
                    .filter(|h| *h > 0 && *h <= 24 * 365);
                if values[0].trim().is_empty() || hours.is_none() {
                    self.error = Some("请写明为什么稍后再看，并填 1–8760 小时后再看".into());
                    self.tasks.form = Some(form);
                    return;
                }
                let outcome = Outcome::Deferred {
                    reason: values[0].clone(),
                    until: now() + hours.unwrap() * 3600,
                };
                if matches!(form.kind, FormKind::TaskDefer) {
                    let Some(id) = self.nav.task_detail.clone() else {
                        return;
                    };
                    Edit::SetTaskOutcome { id, outcome }
                } else {
                    let Some(id) = self.overview.here.clone() else {
                        return;
                    };
                    Edit::SetOutcome { id, outcome }
                }
            }
            FormKind::Space => {
                let space = Space {
                    id: self.tasks.editing_space.clone().unwrap_or_else(id),
                    name: values[0].trim().into(),
                    cwd: values[1].trim().into(),
                };
                if !Path::new(&space.cwd).is_dir() {
                    self.error = Some(format!("目录不存在：{}", space.cwd));
                    self.tasks.form = Some(form);
                    return;
                }
                if self.tasks.editing_space.take().is_some() {
                    Edit::UpdateSpace(space)
                } else {
                    Edit::AddSpace(space)
                }
            }
            FormKind::Task => {
                let task_id = self.tasks.editing_task.clone().unwrap_or_else(id);
                if start {
                    self.tasks.start_after_save = Some(task_id.clone());
                }
                let task = Task {
                    id: task_id,
                    space: form.space.clone(),
                    title: values[0].clone(),
                    description: values[1].clone(),
                    acceptance: values[2]
                        .lines()
                        .filter(|line| !line.trim().is_empty())
                        .map(|line| {
                            self.tasks
                                .editing_task
                                .as_ref()
                                .and_then(|id| self.workspace.tasks.iter().find(|t| &t.id == id))
                                .and_then(|t| {
                                    t.acceptance.iter().find(|c| c.description == line.trim())
                                })
                                .cloned()
                                .unwrap_or_else(|| AcceptanceCriterion {
                                    id: id(),
                                    description: line.trim().into(),
                                    evidence: None,
                                })
                        })
                        .collect(),
                    agent: form.agent,
                };
                if self.tasks.editing_task.take().is_some() {
                    Edit::UpdateTask(task)
                } else {
                    Edit::AddTask(task)
                }
            }
        };
        self.edit(edit, cx);
    }
    pub(super) fn form_panel(&self, form: &Form, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        let mut panel = div()
            .id("editor-form")
            .m(px(T::S20))
            .p(px(T::S20))
            .w(px(T::FORM_WIDTH))
            .max_w_full()
            .overflow_y_scroll()
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::control()))
            .rounded(px(T::POPOVER_RADIUS))
            .bg(T::color(T::header()))
            .flex()
            .flex_col()
            .gap(px(T::S16))
            .child(div().text_size(px(T::FORM_TITLE)).child(match form.kind {
                FormKind::Task if self.tasks.editing_task.is_some() => "编辑待办",
                FormKind::Task => "新建待办",
                FormKind::Space if self.tasks.editing_space.is_some() => "编辑空间",
                FormKind::Space => "新建空间",
                _ => "稍后再看",
            }));
        for (index, field) in form.fields.iter().enumerate() {
            panel = panel.child(field.clone());
            if matches!(form.kind, FormKind::Task) && index == 0 {
                let mut projects = div().flex().flex_wrap().gap(px(T::S6));
                for space in &self.workspace.spaces {
                    let sid = space.id.clone();
                    projects = projects.child(
                        button(
                            gpui::SharedString::from(format!("project-{sid}")),
                            space.name.clone(),
                        )
                        .when(form.space == sid, |d| {
                            d.bg(T::color(T::chip())).text_color(T::color(T::link()))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(f) = &mut this.tasks.form {
                                f.space = sid.clone();
                            }
                            cx.notify();
                        })),
                    );
                }
                panel = panel.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(T::S6))
                        .child(label("项目"))
                        .child(projects),
                );
            }
        }
        if matches!(form.kind, FormKind::Task) {
            let mut agents = div().flex().gap(px(T::S8));
            for agent in Agent::ALL {
                agents = agents.child(
                    button(agent.executable(), agent.executable())
                        .when(form.agent == agent, |d| {
                            d.bg(T::color(T::chip())).text_color(T::color(T::link()))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(f) = &mut this.tasks.form {
                                f.agent = agent;
                            }
                            cx.notify();
                        })),
                );
            }
            panel = panel.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(T::S6))
                    .child(label("默认 Agent"))
                    .child(agents),
            );
        }
        panel.child(
            div()
                .flex()
                .gap(px(T::S8))
                .child(
                    button(
                        "save",
                        if matches!(form.kind, FormKind::Task) {
                            "保存为待办"
                        } else {
                            "保存"
                        },
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.save_form(false, cx))),
                )
                .when(matches!(form.kind, FormKind::Task), |d| {
                    d.child(
                        button("save-start", "保存并开始")
                            .bg(T::color(T::accent()))
                            .text_color(T::color(T::on_accent()))
                            .on_click(cx.listener(|this, _, _, cx| this.save_form(true, cx))),
                    )
                })
                .child(
                    button("cancel", "取消").on_click(cx.listener(|this, _, _, cx| {
                        this.tasks.form = None;
                        this.tasks.editing_space = None;
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
        self.tasks.editing_task = Some(task.id.clone());
        if let Some(f) = &mut self.tasks.form {
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
}
