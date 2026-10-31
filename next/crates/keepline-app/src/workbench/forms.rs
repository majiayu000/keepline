//! The editor form: todo and defer.
use super::*;
use crate::ui::field::FieldEvent;

/// One acceptance criterion line in the todo form. `id` stays with the line while its
/// text is edited or moved, so evidence already attached to it is kept.
pub(super) struct CriterionRow {
    pub(super) id: String,
    pub(super) field: Entity<Field>,
    _events: [gpui::Subscription; 2],
}
/// A criterion line being dragged to a new place, drawn under the pointer by its text.
#[derive(Clone)]
pub(super) struct DraggedCriterion {
    index: usize,
    text: String,
}
impl gpui::Render for DraggedCriterion {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .h(px(T::CRITERION_ROW))
            .max_w(px(T::TODO_PREVIEW_WIDTH))
            .px(px(T::S12))
            .flex()
            .items_center()
            .rounded(px(T::CONTROL_RADIUS))
            .bg(T::color(T::raised()))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::card_border()))
            .text_color(T::color(T::primary()))
            .opacity(0.9)
            .child(div().truncate().child(self.text.clone()))
    }
}
/// What stops the todo form: a missing title blocks both saves, a space whose directory
/// is gone blocks 「保存并开始」, and an Agent the login shell cannot find is said up front.
pub(super) struct FormChecks {
    pub(super) title_missing: bool,
    pub(super) dir_missing: bool,
    pub(super) agent_missing: bool,
}
/// The commands among `names` that a login shell finds, functions and aliases included,
/// as a new terminal would; None when the shell is not one we can ask or it does not answer.
fn login_commands(names: &[String]) -> Option<HashSet<String>> {
    use std::process::{Command, Stdio};
    let shell = std::env::var("SHELL")
        .ok()
        .filter(|s| s.ends_with("/zsh") || s.ends_with("/bash"))?;
    let script: String = names
        .iter()
        .map(|n| {
            let q = shell_quote(n);
            format!("command -v {q} >/dev/null 2>&1 && printf 'keepline-found:%s\\n' {q};")
        })
        .collect();
    let mut child = Command::new(shell)
        .args(["-l", "-i", "-c", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50))
            }
            _ => {
                let _ = child.kill();
                return None;
            }
        }
    }
    let mut out = String::new();
    std::io::Read::read_to_string(&mut child.stdout.take()?, &mut out).ok()?;
    Some(
        out.lines()
            .filter_map(|l| l.strip_prefix("keepline-found:"))
            .map(str::to_owned)
            .collect(),
    )
}
/// A small segmented-control button: raised when chosen.
fn segment(id: impl Into<gpui::ElementId>, chosen: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .role(gpui::accesskit::Role::Button)
        .h(px(T::SEGMENT_HEIGHT))
        .px(px(T::S12))
        .flex()
        .items_center()
        .gap(px(T::S6))
        .rounded(px(T::SEGMENT_RADIUS))
        .text_size(px(T::SMALL))
        .cursor_pointer()
        .text_color(T::color(T::secondary()))
        .hover(|s| s.text_color(T::color(T::primary())))
        .when(chosen, |d| {
            d.bg(T::color(T::popover()))
                .text_color(T::color(T::primary()))
                .font_weight(gpui::FontWeight::MEDIUM)
                .shadow_sm()
        })
}
fn segments() -> gpui::Div {
    div()
        .flex()
        .flex_wrap()
        .p(px(T::S2))
        .rounded(px(T::SEGMENT_TRACK_RADIUS))
        .bg(T::color(T::raised()))
}
/// A form label with an optional grey note after it and one at the right end.
fn form_label(text: &str, note: Option<&str>, right: Option<String>) -> gpui::Div {
    div()
        .flex()
        .items_baseline()
        .gap(px(T::S8))
        .text_size(px(T::SMALL))
        .text_color(T::color(T::secondary()))
        .child(text.to_owned())
        .when_some(note, |d, n| {
            d.child(
                div()
                    .text_size(px(T::META))
                    .text_color(T::color(T::muted()))
                    .child(n.to_owned()),
            )
        })
        .when_some(right, |d, r| {
            d.child(
                div()
                    .ml_auto()
                    .text_size(px(T::META))
                    .text_color(T::color(T::muted()))
                    .child(r),
            )
        })
}
fn hint(text: impl Into<gpui::SharedString>) -> gpui::Div {
    div()
        .text_size(px(T::META))
        .text_color(T::color(T::muted()))
        .child(text.into())
}
/// A primary or plain button that cannot be pressed now.
fn disabled(b: gpui::Stateful<gpui::Div>) -> gpui::Stateful<gpui::Div> {
    b.opacity(T::DISABLED_OPACITY).cursor_default()
}
pub(super) fn solid(b: gpui::Stateful<gpui::Div>) -> gpui::Stateful<gpui::Div> {
    b.bg(T::color(T::accent()))
        .text_color(T::color(T::on_accent()))
        .font_weight(gpui::FontWeight::SEMIBOLD)
}
pub(super) fn plain(b: gpui::Stateful<gpui::Div>) -> gpui::Stateful<gpui::Div> {
    b.bg(T::color(T::button()))
        .text_color(T::color(T::primary()))
}

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
        let fields = match kind {
            FormKind::Defer | FormKind::TaskDefer => vec![
                cx.new(|cx| Field::new("为什么稍后再看（必填）", "", cx)),
                cx.new(|cx| Field::new("多少小时后再看（必填）", "24", cx)),
            ],
            FormKind::Task => vec![
                cx.new(|cx| {
                    Field::new("要做什么", "", cx)
                        .compact()
                        .placeholder("要做什么")
                }),
                cx.new(|cx| Field::new("说明（可选 · 原样交给 Agent）", "", cx)),
            ],
        };
        let watch = fields
            .iter()
            .map(|f| cx.observe(f, |_, _, cx| cx.notify()))
            .collect();
        window.focus(&fields[0].focus_handle(cx), cx);
        self.tasks.form = Some(Form {
            kind,
            fields,
            agent: self.workspace.settings.default_agent,
            space: self.nav.space.clone(),
            criteria: vec![],
            tried: false,
            _watch: watch,
        });
        if matches!(kind, FormKind::Task) {
            self.probe_agents(cx);
        }
        cx.notify();
    }
    /// Asks the login shell once which Agent commands it has, in the background.
    fn probe_agents(&mut self, cx: &mut Context<Self>) {
        if self.tasks.found.is_some() || self.tasks.probing {
            return;
        }
        self.tasks.probing = true;
        let names: Vec<String> = Agent::all()
            .into_iter()
            .map(|a| a.executable().to_owned())
            .collect();
        cx.spawn(async move |this, cx| {
            let found = cx
                .background_executor()
                .spawn(async move { login_commands(&names) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.tasks.found = found;
                cx.notify();
            });
        })
        .detach();
    }
    /// Puts a criterion line at `at` (clamped), focused when `focus`.
    pub(super) fn add_criterion(
        &mut self,
        at: usize,
        criterion: Option<&AcceptanceCriterion>,
        focus: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = criterion.map_or("", |c| c.description.as_str());
        let field = cx.new(|cx| {
            Field::new("验收标准", text, cx)
                .inline()
                .placeholder("写一条完成时怎么核对…")
        });
        // Enter on a written line opens the next one below it.
        let events = cx.subscribe_in(&field, window, |this, field, event, window, cx| {
            if let FieldEvent::Submit = event
                && let Some(form) = &this.tasks.form
                && let Some(i) = form.criteria.iter().position(|r| &r.field == field)
                && !field.read(cx).text.trim().is_empty()
            {
                this.add_criterion(i + 1, None, true, window, cx);
            }
        });
        let watch = cx.observe(&field, |_, _, cx| cx.notify());
        if focus {
            window.focus(&field.focus_handle(cx), cx);
        }
        if let Some(form) = &mut self.tasks.form {
            let at = at.min(form.criteria.len());
            form.criteria.insert(
                at,
                CriterionRow {
                    id: criterion.map_or_else(id, |c| c.id.clone()),
                    field,
                    _events: [events, watch],
                },
            );
        }
        cx.notify();
    }
    /// Drops the dragged line right before line `before`, or last.
    fn move_criterion(&mut self, from: usize, before: usize, cx: &mut Context<Self>) {
        let Some(form) = &mut self.tasks.form else {
            return;
        };
        if from >= form.criteria.len() || before == from || before == from + 1 {
            return;
        }
        let row = form.criteria.remove(from);
        let to = if before > from { before - 1 } else { before };
        form.criteria.insert(to.min(form.criteria.len()), row);
        cx.notify();
    }
    /// The todo the form would save now; duplicate criteria are merged with their evidence.
    pub(super) fn form_task(&self, form: &Form, cx: &gpui::App) -> Task {
        let existing = self
            .tasks
            .editing_task
            .as_ref()
            .and_then(|id| self.workspace.tasks.iter().find(|t| &t.id == id));
        let acceptance = form
            .criteria
            .iter()
            .map(|row| {
                let description = row.field.read(cx).text.trim().to_owned();
                existing
                    .and_then(|t| t.acceptance.iter().find(|c| c.id == row.id))
                    .map(|c| AcceptanceCriterion {
                        description: description.clone(),
                        ..c.clone()
                    })
                    .unwrap_or_else(|| AcceptanceCriterion {
                        id: row.id.clone(),
                        description: description.clone(),
                        evidence: vec![],
                    })
            })
            .collect();
        Task {
            id: self.tasks.editing_task.clone().unwrap_or_default(),
            space: form.space.clone(),
            title: form.fields[0].read(cx).text.trim().to_owned(),
            description: form.fields[1].read(cx).text.clone(),
            acceptance: merge_criteria(acceptance),
            agent: form.agent,
        }
    }
    pub(super) fn form_checks(&self, form: &Form, cx: &gpui::App) -> FormChecks {
        FormChecks {
            title_missing: form.fields[0].read(cx).text.trim().is_empty(),
            dir_missing: self
                .workspace
                .spaces
                .iter()
                .find(|s| s.id == form.space)
                .is_some_and(|s| !Path::new(&s.cwd).is_dir()),
            agent_missing: self
                .tasks
                .found
                .as_ref()
                .is_some_and(|f| !f.contains(form.agent.executable())),
        }
    }
    pub(super) fn save_form(&mut self, start: bool, cx: &mut Context<Self>) {
        let Some(mut form) = self.tasks.form.take() else {
            return;
        };
        let edit = match form.kind {
            FormKind::Defer | FormKind::TaskDefer => {
                let values: Vec<String> = form
                    .fields
                    .iter()
                    .map(|f| f.read(cx).text.clone())
                    .collect();
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
            FormKind::Task => {
                let checks = self.form_checks(&form, cx);
                if checks.title_missing || (start && checks.dir_missing) {
                    form.tried = true;
                    self.tasks.form = Some(form);
                    cx.notify();
                    return;
                }
                let mut task = self.form_task(&form, cx);
                if task.id.is_empty() {
                    task.id = id();
                }
                if start {
                    self.tasks.start_after_save = Some(task.id.clone());
                }
                if self.tasks.editing_task.take().is_some() {
                    Edit::UpdateTask(task)
                } else {
                    Edit::AddTask(task)
                }
            }
        };
        self.edit(edit, cx);
    }
    pub(super) fn form_panel(
        &self,
        form: &Form,
        window: &Window,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        if !matches!(form.kind, FormKind::Task) {
            return self.defer_panel(form, cx);
        }
        let checks = self.form_checks(form, cx);
        let draft = self.form_task(form, cx);
        div()
            .id("editor-form")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(T::OVERVIEW_X))
            .py(px(T::S20))
            // ⌘⏎ from any field saves and starts, as the button says.
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                let k = &event.keystroke;
                if k.key == "enter" && k.modifiers.platform {
                    this.save_form(true, cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .w(px(T::TODO_FORM_WIDTH))
                    .max_w_full()
                    .mx_auto()
                    .flex()
                    .items_start()
                    .gap(px(T::S16))
                    .child(self.form_card(form, &checks, window, cx))
                    .child(self.form_preview(form, &draft, cx)),
            )
    }
    fn form_card(
        &self,
        form: &Form,
        checks: &FormChecks,
        window: &Window,
        cx: &Context<Self>,
    ) -> gpui::Div {
        let error = Group::Error.colors().1;
        let title_bad = form.tried && checks.title_missing;
        let card = div()
            .flex_1()
            .min_w_0()
            .p(px(T::S20))
            .flex()
            .flex_col()
            .gap(px(T::S16))
            .bg(T::color(T::popover()))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::card_border()))
            .rounded(px(T::POPOVER_RADIUS))
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(T::S10))
                    .child(
                        div()
                            .text_size(px(T::FORM_TITLE))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(if self.tasks.editing_task.is_some() {
                                "编辑待办"
                            } else {
                                "新建待办"
                            }),
                    )
                    .child(hint("保存后出现在左栏「待办」和鸟瞰顶部"))
                    .child(div().flex_1())
                    .child(
                        button("form-close", "×")
                            .w(px(T::ICON_WIDTH))
                            .h(px(T::ICON_WIDTH))
                            .px_0()
                            .text_color(T::color(T::muted()))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.tasks.form = None;
                                this.tasks.editing_task = None;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(T::S6))
                    .child(form_label("要做什么", None, None))
                    .child(
                        div()
                            .rounded(px(T::CONTROL_RADIUS))
                            .when(title_bad, |d| {
                                d.border(px(T::BORDER_WIDTH)).border_color(error)
                            })
                            .child(form.fields[0].clone()),
                    )
                    .when(title_bad, |d| d.child(hint("先写要做什么").text_color(error))),
            )
            .child(self.form_spaces(form, checks, cx))
            .child(form.fields[1].clone())
            .child(self.form_criteria(form, window, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(T::S6))
                    .child(form_label("默认用哪个 Agent", Some("开始时还能改"), None))
                    .child(Agent::all().into_iter().fold(segments(), |row, agent| {
                        row.child(
                            segment(
                                gpui::SharedString::from(format!("agent-{}", agent.executable())),
                                form.agent == agent,
                            )
                            .aria_label(agent.name())
                            .child(agent.name())
                            .child(
                                div()
                                    .font(T::mono(cx))
                                    .text_size(px(T::META))
                                    .text_color(T::color(T::muted()))
                                    .child(agent.executable()),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(f) = &mut this.tasks.form {
                                    f.agent = agent;
                                }
                                cx.notify();
                            })),
                        )
                    }))
                    .when(checks.agent_missing, |d| {
                        d.child(
                            hint(format!(
                                "找不到 {} 命令：开始后会停在「找不到命令」。先安装或加进 PATH，或换一个 Agent",
                                form.agent.executable()
                            ))
                            .text_color(error),
                        )
                    }),
            );
        let blocked = checks.title_missing;
        let no_start = blocked || checks.dir_missing;
        let save = button("save", "保存为待办");
        let start = button("save-start", "")
            .gap(px(T::S6))
            .child("保存并开始")
            .child(
                div()
                    .px(px(T::S4))
                    .rounded(px(T::CHIP_RADIUS))
                    .border(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::on_accent()))
                    .opacity(T::KBD_OPACITY)
                    .text_size(px(T::KBD_SIZE))
                    .child("⌘⏎"),
            )
            .aria_label("保存并开始");
        card.child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(T::S8))
                .pt(px(T::S16))
                .border_t(px(T::BORDER_WIDTH))
                .border_color(T::color(T::border()))
                .child(
                    hint("都能再编辑；验收标准改字不会丢掉已挂的证据")
                        .text_size(px(T::SMALL))
                        .mr_auto(),
                )
                .child(
                    button("cancel", "取消").on_click(cx.listener(|this, _, _, cx| {
                        this.tasks.form = None;
                        this.tasks.editing_task = None;
                        cx.notify();
                    })),
                )
                .child(if blocked {
                    disabled(plain(save))
                } else {
                    plain(save).on_click(cx.listener(|this, _, _, cx| this.save_form(false, cx)))
                })
                .child(if no_start {
                    disabled(solid(start))
                } else {
                    solid(start).on_click(cx.listener(|this, _, _, cx| this.save_form(true, cx)))
                }),
        )
    }
    /// The space choice; a space whose directory is gone says so and offers to repoint it.
    fn form_spaces(&self, form: &Form, checks: &FormChecks, cx: &Context<Self>) -> gpui::Div {
        let error = Group::Error.colors().1;
        let cwd = self
            .workspace
            .spaces
            .iter()
            .find(|s| s.id == form.space)
            .map(|s| home_relative(&s.cwd))
            .unwrap_or_default();
        let mut row = segments();
        for space in &self.workspace.spaces {
            let sid = space.id.clone();
            let missing = !Path::new(&space.cwd).is_dir();
            row = row.child(
                segment(
                    gpui::SharedString::from(format!("project-{sid}")),
                    form.space == sid,
                )
                .aria_label(space.name.clone())
                .child(space.name.clone())
                .when(missing, |d| {
                    d.child(
                        div()
                            .px(px(T::S4))
                            .rounded(px(T::CHIP_RADIUS))
                            .bg(T::color(T::chip()))
                            .text_size(px(T::CHIP_SIZE))
                            .text_color(T::color(T::muted()))
                            .font_weight(gpui::FontWeight::NORMAL)
                            .child("目录不在了"),
                    )
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(f) = &mut this.tasks.form {
                        f.space = sid.clone();
                    }
                    cx.notify();
                })),
            );
        }
        let space = form.space.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(T::S6))
            .child(form_label(
                "在哪个空间做",
                None,
                Some("开始时在这个目录开终端".into()),
            ))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(T::S12))
                    .child(row)
                    .child(
                        hint(cwd)
                            .font(T::mono(cx))
                            .min_w_0()
                            .truncate()
                            .when(checks.dir_missing, |d| d.text_color(error)),
                    ),
            )
            .when(checks.dir_missing, |d| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(T::S8))
                        .child(
                            hint("这个空间的目录不在了：可以先保存为待办，改好目录再开始")
                                .text_color(error),
                        )
                        .child(
                            button("form-space-dir", "改空间目录")
                                .h(px(T::ICON_HEIGHT))
                                .px(px(T::S6))
                                .text_color(T::color(T::link()))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.choose_space_dir(Some(space.clone()), window, cx)
                                })),
                        ),
                )
            })
    }
    /// The criteria checklist: drag a line by its grip, Enter opens the next line, × drops it.
    fn form_criteria(&self, form: &Form, window: &Window, cx: &Context<Self>) -> gpui::Div {
        let warn = Group::Paused.colors().1;
        let texts: Vec<String> = form
            .criteria
            .iter()
            .map(|r| r.field.read(cx).text.trim().to_owned())
            .collect();
        let written = texts.iter().filter(|t| !t.is_empty()).count();
        let mut list = div()
            .flex()
            .flex_col()
            .rounded(px(T::CONTROL_RADIUS))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::field_edge()))
            .bg(T::color(T::panel()))
            .overflow_hidden();
        for (i, row) in form.criteria.iter().enumerate() {
            let same = (!texts[i].is_empty())
                .then(|| texts[..i].iter().position(|t| t == &texts[i]))
                .flatten();
            let focused = row.field.focus_handle(cx).is_focused(window);
            let dragged = DraggedCriterion {
                index: i,
                text: texts[i].clone(),
            };
            list = list.child(
                div()
                    .id(("criterion", i))
                    .h(px(T::CRITERION_ROW))
                    .flex()
                    .items_center()
                    .gap(px(T::S8))
                    .pl(px(T::S6))
                    .pr(px(T::S8))
                    .border_b(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::border()))
                    .when(focused, |d| d.bg(T::color(T::popover())))
                    .when(same.is_some(), |d| {
                        d.border_l(px(T::FOCUS_RING)).border_color(warn)
                    })
                    .drag_over::<DraggedCriterion>(|s, _, _, _| {
                        s.border_t(px(T::FOCUS_RING))
                            .border_color(T::color(T::accent()))
                    })
                    .on_drop(cx.listener(move |this, d: &DraggedCriterion, _, cx| {
                        this.move_criterion(d.index, i, cx)
                    }))
                    .child(
                        div()
                            .id(("criterion-grip", i))
                            .w(px(T::CRITERION_GRIP))
                            .flex_shrink_0()
                            .text_size(px(T::CHIP_SIZE))
                            .text_color(T::color(T::muted()))
                            .cursor_grab()
                            .child("⋮⋮")
                            .on_drag(dragged, |d, _, _, cx| cx.new(|_| d.clone())),
                    )
                    .child(
                        div()
                            .w(px(T::CRITERION_NUMBER))
                            .flex_shrink_0()
                            .flex()
                            .justify_end()
                            .text_size(px(T::SMALL))
                            .text_color(T::color(T::muted()))
                            .font_features(T::tabular())
                            .child((i + 1).to_string()),
                    )
                    .child(div().flex_1().min_w_0().h_full().child(row.field.clone()))
                    .when_some(same, |d, j| {
                        d.child(
                            hint(format!("和第 {} 条相同，保存时合并", j + 1))
                                .flex_shrink_0()
                                .text_color(warn),
                        )
                    })
                    .when(same.is_none() && focused, |d| {
                        d.child(hint("回车加下一条").flex_shrink_0())
                    })
                    .child(
                        button(("criterion-delete", i), "×")
                            .aria_label(format!("删除第 {} 条", i + 1))
                            .w(px(T::ICON_HEIGHT))
                            .h(px(T::ICON_HEIGHT))
                            .px_0()
                            .flex_shrink_0()
                            .text_color(T::color(T::muted()))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(f) = &mut this.tasks.form
                                    && i < f.criteria.len()
                                {
                                    f.criteria.remove(i);
                                }
                                cx.notify();
                            })),
                    ),
            );
        }
        let count = form.criteria.len();
        list = list.child(
            div()
                .id("criterion-add")
                .h(px(T::CRITERION_ROW))
                .flex()
                .items_center()
                .gap(px(T::S8))
                .pl(px(T::S6))
                .pr(px(T::S8))
                .cursor_text()
                .drag_over::<DraggedCriterion>(|s, _, _, _| {
                    s.border_t(px(T::FOCUS_RING))
                        .border_color(T::color(T::accent()))
                })
                .on_drop(cx.listener(move |this, d: &DraggedCriterion, _, cx| {
                    this.move_criterion(d.index, count, cx)
                }))
                .child(div().w(px(T::CRITERION_GRIP)).flex_shrink_0())
                .child(
                    div()
                        .w(px(T::CRITERION_NUMBER))
                        .flex_shrink_0()
                        .flex()
                        .justify_end()
                        .text_color(T::color(T::muted()))
                        .child("＋"),
                )
                .child(div().text_color(T::color(T::muted())).child(if count == 0 {
                    "写一条完成时怎么核对…"
                } else {
                    "再加一条…"
                }))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.add_criterion(count, None, true, window, cx)
                })),
        );
        div()
            .flex()
            .flex_col()
            .gap(px(T::S6))
            .child(form_label(
                "验收标准",
                Some("可选 · 也会交给 Agent，完成时逐条核对"),
                (written > 0).then(|| format!("{written} 条")),
            ))
            .child(list)
            .child(if written == 0 && !form.fields[0].read(cx).text.trim().is_empty() {
                hint("没有验收标准，完成时没有东西可核对；开始后待办信息条会一直提示「补写」")
                    .text_color(warn)
            } else {
                hint("每条最好能被一条命令、一次测试或一处文件改动证明；Keepline 会从关联会话里找这类记录当证据。")
            })
    }
    /// What 「保存并开始」 then does, and the exact text placed in the Agent's input box.
    fn form_preview(&self, form: &Form, draft: &Task, cx: &Context<Self>) -> gpui::Div {
        let space = self
            .workspace
            .spaces
            .iter()
            .find(|s| s.id == form.space)
            .map_or_else(String::new, |s| s.name.clone());
        let agent = form.agent;
        // A missing title still shows where it will go, rather than a blank first line.
        let text = if draft.title.is_empty() {
            Task {
                title: "（先写要做什么）".into(),
                ..draft.clone()
            }
            .prompt()
        } else {
            draft.prompt()
        };
        let step = |n: &str, you: bool, body: gpui::AnyElement| {
            div()
                .flex()
                .items_start()
                .gap(px(T::S8))
                .child(
                    div()
                        .size(px(T::STEP_DOT))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .border(px(T::BORDER_WIDTH))
                        .border_color(T::color(T::control()))
                        .text_size(px(T::CHIP_SIZE))
                        .text_color(T::color(T::secondary()))
                        .when(you, |d| {
                            d.bg(T::color(T::accent()))
                                .border_color(T::color(T::accent()))
                                .text_color(T::color(T::on_accent()))
                        })
                        .child(n.to_owned()),
                )
                .child(div().flex_1().min_w_0().child(body))
        };
        let code = |s: String| {
            div()
                .px(px(T::S4))
                .rounded(px(T::CHIP_RADIUS))
                .border(px(T::BORDER_WIDTH))
                .border_color(T::color(T::border()))
                .bg(T::color(T::app()))
                .font(T::mono(cx))
                .text_size(px(T::SMALL))
                .text_color(T::color(T::primary()))
                .child(s)
        };
        let copy = text.clone();
        div()
            .w(px(T::TODO_PREVIEW_WIDTH))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(T::color(T::popover()))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::card_border()))
            .rounded(px(T::POPOVER_RADIUS))
            .overflow_hidden()
            .child(
                div()
                    .px(px(T::S16))
                    .pt(px(T::S14))
                    .pb(px(T::S12))
                    .flex()
                    .flex_col()
                    .gap(px(T::S10))
                    .border_b(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::border()))
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("保存并开始之后"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(T::S8))
                            .text_size(px(T::SMALL))
                            .child(step(
                                "1",
                                false,
                                format!("在「{space}」新开一个画面，标签就叫待办标题")
                                    .into_any_element(),
                            ))
                            .child(step(
                                "2",
                                false,
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .items_center()
                                    .gap(px(T::S4))
                                    .child("运行")
                                    .child(code(agent.command()))
                                    .child("，等它的输入框出现")
                                    .into_any_element(),
                            ))
                            .child(step(
                                "3",
                                false,
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .child(format!("把下面这段放进 {} 的输入框，", agent.name()))
                                    .child(
                                        div().font_weight(gpui::FontWeight::BOLD).child("不发送"),
                                    )
                                    .into_any_element(),
                            ))
                            .child(step(
                                "4",
                                true,
                                "你检查、可以改，按回车才发给 Agent".into_any_element(),
                            )),
                    ),
            )
            .child(
                div()
                    .px(px(T::S16))
                    .pt(px(T::S12))
                    .pb(px(T::S14))
                    .flex()
                    .flex_col()
                    .gap(px(T::S8))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .text_size(px(T::SMALL))
                            .text_color(T::color(T::secondary()))
                            .child("放进输入框的内容")
                            .child(div().flex_1())
                            .child(
                                button("form-copy", "复制")
                                    .h(px(T::ICON_WIDTH))
                                    .px(px(T::S6))
                                    .text_color(T::color(T::link()))
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                            copy.clone(),
                                        ))
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .p(px(T::S10))
                            .rounded(px(T::CONTROL_RADIUS))
                            .border(px(T::BORDER_WIDTH))
                            .border_color(T::color(T::border()))
                            .bg(T::color(T::app()))
                            .font(T::mono(cx))
                            .text_size(px(T::SMALL))
                            .text_color(T::color(T::body()))
                            .flex()
                            .gap(px(T::S6))
                            .child(div().text_color(T::color(T::muted())).child("›"))
                            .child(div().flex_1().min_w_0().child(text)),
                    ),
            )
            .child(
                div()
                    .px(px(T::S16))
                    .py(px(T::S10))
                    .flex()
                    .flex_wrap()
                    .gap(px(T::S12))
                    .border_t(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::border()))
                    .child(hint("不是 shell 参数，引号和换行原样保留"))
                    .child(hint("同一待办再点「开始」会切回这个画面")),
            )
    }
    fn defer_panel(&self, form: &Form, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
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
            .child(div().text_size(px(T::FORM_TITLE)).child("稍后再看"));
        for field in &form.fields {
            panel = panel.child(field.clone());
        }
        panel.child(
            div()
                .flex()
                .gap(px(T::S8))
                .child(
                    button("save", "保存")
                        .on_click(cx.listener(|this, _, _, cx| this.save_form(false, cx))),
                )
                .child(
                    button("cancel", "取消").on_click(cx.listener(|this, _, _, cx| {
                        this.tasks.form = None;
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
        self.fill_task_form(task, window, cx);
        self.tasks.editing_task = Some(task.id.clone());
    }
    /// Puts a todo's title, description, space, Agent and criteria into the open form.
    pub(super) fn fill_task_form(
        &mut self,
        task: &Task,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(f) = &mut self.tasks.form else {
            return;
        };
        f.space = task.space.clone();
        f.agent = task.agent;
        f.criteria.clear();
        for (field, value) in f
            .fields
            .clone()
            .iter()
            .zip([task.title.clone(), task.description.clone()])
        {
            field.update(cx, |f, cx| {
                f.set_text(&value);
                cx.notify();
            });
        }
        for (i, criterion) in task.acceptance.iter().enumerate() {
            self.add_criterion(i, Some(criterion), false, window, cx);
        }
    }
}
