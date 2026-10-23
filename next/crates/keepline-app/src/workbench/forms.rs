//! The editor form: todo, space, defer and settings.
use super::*;

impl Workbench {
    pub(super) fn show_form(
        &mut self,
        kind: FormKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.overview = false;
        self.editing_task = None;
        self.sync_panes(cx);
        let fields = match kind {
            FormKind::Settings => vec![
                cx.new(|cx| {
                    Field::new(
                        "很久没动（天，1–365）",
                        &self.workspace.settings.stale_days.to_string(),
                        cx,
                    )
                }),
                cx.new(|cx| {
                    Field::new(
                        "离开多久显示鸟瞰（分钟，1–1440）",
                        &self.workspace.settings.away_minutes.to_string(),
                        cx,
                    )
                }),
            ],
            FormKind::Space => vec![
                cx.new(|cx| Field::new("空间名称", "", cx)),
                cx.new(|cx| Field::new("工作目录（绝对路径）", "", cx)),
            ],
            FormKind::Defer | FormKind::TaskDefer => vec![
                cx.new(|cx| Field::new("暂存原因（必填）", "", cx)),
                cx.new(|cx| Field::new("多少小时后再看（必填）", "24", cx)),
            ],
            FormKind::Task => vec![
                cx.new(|cx| Field::new("要做什么", "", cx)),
                cx.new(|cx| Field::new("说明", "", cx)),
                cx.new(|cx| Field::new("验收标准（每行一条）", "", cx)),
            ],
        };
        window.focus(&fields[0].focus_handle(cx), cx);
        self.form = Some(Form {
            notifications: self.workspace.settings.notifications,
            kind,
            fields,
            agent: self.workspace.settings.default_agent,
            space: self.space.clone(),
        });
        cx.notify();
    }
    pub(super) fn save_form(&mut self, start: bool, cx: &mut Context<Self>) {
        let Some(form) = self.form.take() else {
            return;
        };
        let values: Vec<String> = form
            .fields
            .iter()
            .map(|f| f.read(cx).text.clone())
            .collect();
        let edit = match form.kind {
            FormKind::Settings => {
                let days = values[0]
                    .trim()
                    .parse::<u32>()
                    .ok()
                    .filter(|n| (1..=365).contains(n));
                let minutes = values[1]
                    .trim()
                    .parse::<u32>()
                    .ok()
                    .filter(|n| (1..=1440).contains(n));
                let (Some(stale_days), Some(away_minutes)) = (days, minutes) else {
                    self.error = Some("请填写有效天数和分钟数".into());
                    self.form = Some(form);
                    return;
                };
                Edit::Settings(Settings {
                    notifications: form.notifications,
                    stale_days,
                    away_minutes,
                    default_agent: form.agent,
                })
            }

            FormKind::Defer | FormKind::TaskDefer => {
                let hours = values[1]
                    .trim()
                    .parse::<u64>()
                    .ok()
                    .filter(|h| *h > 0 && *h <= 24 * 365);
                if values[0].trim().is_empty() || hours.is_none() {
                    self.error = Some("请填写暂存原因和 1–8760 小时的再看时间".into());
                    self.form = Some(form);
                    return;
                }
                let outcome = Outcome::Deferred {
                    reason: values[0].clone(),
                    until: now() + hours.unwrap() * 3600,
                };
                if matches!(form.kind, FormKind::TaskDefer) {
                    let Some(id) = self.task_detail.clone() else {
                        return;
                    };
                    Edit::SetTaskOutcome { id, outcome }
                } else {
                    let Some(id) = self.here.clone() else {
                        return;
                    };
                    Edit::SetOutcome { id, outcome }
                }
            }
            FormKind::Space => Edit::AddSpace(Space {
                id: id(),
                name: values[0].clone(),
                cwd: values[1].clone(),
            }),
            FormKind::Task => {
                let task_id = self.editing_task.clone().unwrap_or_else(id);
                if start {
                    self.start_after_save = Some(task_id.clone());
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
                            self.editing_task
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
                if self.editing_task.take().is_some() {
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
            .border_color(T::color(T::CONTROL))
            .rounded(px(T::POPOVER_RADIUS))
            .bg(T::color(T::HEADER))
            .flex()
            .flex_col()
            .gap(px(T::S16))
            .child(div().text_size(px(T::FORM_TITLE)).child(match form.kind {
                FormKind::Task if self.editing_task.is_some() => "编辑待办",
                FormKind::Task => "新建待办",
                FormKind::Settings => "设置",
                FormKind::Space => "新建空间",
                _ => "暂存",
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
                            d.bg(T::color(T::CHIP)).text_color(T::color(T::LINK))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(f) = &mut this.form {
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
        if matches!(form.kind, FormKind::Task | FormKind::Settings) {
            let mut agents = div().flex().gap(px(T::S8));
            for agent in [Agent::Codex, Agent::Claude] {
                agents = agents.child(
                    button(agent.executable(), agent.executable())
                        .when(form.agent == agent, |d| {
                            d.bg(T::color(T::CHIP)).text_color(T::color(T::LINK))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(f) = &mut this.form {
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
        if matches!(form.kind, FormKind::Settings) {
            panel = panel
                .child(
                    button(
                        "notifications-toggle",
                        if form.notifications {
                            "✓ 系统通知已开启"
                        } else {
                            "系统通知关闭（点击开启）"
                        },
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(form) = &mut this.form {
                            form.notifications = !form.notifications;
                        }
                        cx.notify();
                    })),
                )
                .child(label(
                    "保存开启后由 macOS 请求通知权限；默认关闭，退出 App 后不接收通知。",
                ));
            if let Some(status) = cx.try_global::<crate::desktop::Status>() {
                panel = panel.child(label(status.0.clone()));
            }
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
                            .bg(T::color(T::ACCENT))
                            .text_color(T::color(T::ON_ACCENT))
                            .on_click(cx.listener(|this, _, _, cx| this.save_form(true, cx))),
                    )
                })
                .child(
                    button("cancel", "取消").on_click(cx.listener(|this, _, _, cx| {
                        this.form = None;
                        cx.notify();
                    })),
                ),
        )
    }
}
