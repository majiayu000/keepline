//! The todo detail: where it stands, acceptance with evidence per criterion, its sessions.
use super::forms::{plain, solid};
use super::*;
use keepline_core::content::Evidence;

/// Where a todo stands, as the detail's status names it. Outcomes come from the user;
/// 进行中 and 验收齐了 are read from the linked sessions and the attached evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TaskStatus {
    NotStarted,
    Working,
    Proven,
    Completed,
    Deferred,
    Abandoned,
}
impl TaskStatus {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::NotStarted => "未开始",
            Self::Working => "进行中",
            Self::Proven => "验收齐了",
            Self::Completed => "已完成",
            Self::Deferred => "稍后再看",
            Self::Abandoned => "已放弃",
        }
    }
}
pub(super) fn task_status(w: &Workspace, task: &Task) -> TaskStatus {
    match w.task_outcomes.get(&task.id).unwrap_or(&Outcome::Open) {
        Outcome::Completed => TaskStatus::Completed,
        Outcome::Abandoned => TaskStatus::Abandoned,
        Outcome::Deferred { .. } => TaskStatus::Deferred,
        Outcome::Open if !task.acceptance.is_empty() && task.proven() == task.acceptance.len() => {
            TaskStatus::Proven
        }
        Outcome::Open
            if w.agent_sessions
                .iter()
                .any(|s| s.task.as_ref() == Some(&task.id)) =>
        {
            TaskStatus::Working
        }
        Outcome::Open => TaskStatus::NotStarted,
    }
}
/// Records from the todo's sessions that a criterion can hold, newest first, each with the
/// session it came from; ones already on `criterion` are left out.
pub(super) fn attachable(
    w: &Workspace,
    task: &str,
    criterion: Option<&AcceptanceCriterion>,
) -> Vec<CriterionEvidence> {
    let mut records: Vec<CriterionEvidence> = w
        .agent_sessions
        .iter()
        .filter(|s| s.task.as_deref() == Some(task))
        .filter_map(|s| w.content.get(&s.id).map(|c| (s, c)))
        .flat_map(|(s, c)| {
            c.evidence
                .iter()
                .filter(|e| e.attachable())
                .map(|e| CriterionEvidence {
                    session: s.id.clone(),
                    record: e.clone(),
                })
        })
        .filter(|e| criterion.is_none_or(|c| !c.evidence.contains(e)))
        .collect();
    records.sort_by_key(|e| std::cmp::Reverse(e.record.at));
    records
}
fn kind_name(e: &Evidence) -> &'static str {
    match e.kind.as_str() {
        "test" => "测试",
        "commit" => "提交",
        _ => "文件",
    }
}
/// How the record ended: an exit code, or what a file change and a commit did.
fn result_name(e: &Evidence) -> String {
    match (e.kind.as_str(), e.exit_code) {
        ("file", Some(0)) => "已写入".into(),
        ("commit", Some(0)) => "已提交".into(),
        (_, Some(code)) => format!("exit {code}"),
        _ => "未知".into(),
    }
}
fn hm(at: u64) -> String {
    clock(at).chars().take(5).collect()
}
/// The ring before a status: empty, half or full as the todo moves along.
fn ring(status: TaskStatus) -> gpui::Div {
    let ink = T::color(T::secondary());
    let base = div()
        .size(px(T::DOT + 1.))
        .flex_shrink_0()
        .rounded_full()
        .border(px(T::CARET_WIDTH))
        .border_color(ink)
        .overflow_hidden();
    match status {
        TaskStatus::NotStarted | TaskStatus::Deferred | TaskStatus::Abandoned => {
            base.border_color(T::color(T::muted()))
        }
        TaskStatus::Working => base.flex().child(div().w_1_2().h_full().bg(ink)),
        TaskStatus::Proven | TaskStatus::Completed => base.bg(ink),
    }
}
fn panel() -> gpui::Div {
    div()
        .min_w_0()
        .flex()
        .flex_col()
        .bg(T::color(T::popover()))
        .border(px(T::BORDER_WIDTH))
        .border_color(T::color(T::card_border()))
        .rounded(px(T::POPOVER_RADIUS))
        .overflow_hidden()
}
fn panel_head(title: &str, count: String) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(T::S8))
        .px(px(T::S16))
        .py(px(T::S12))
        .border_b(px(T::BORDER_WIDTH))
        .border_color(T::color(T::border()))
        .child(
            div()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(title.to_owned()),
        )
        .child(
            div()
                .text_size(px(T::SMALL))
                .text_color(T::color(T::muted()))
                .child(count),
        )
}
fn link(
    id: impl Into<gpui::ElementId>,
    text: impl Into<gpui::SharedString>,
) -> gpui::Stateful<gpui::Div> {
    button(id, text)
        .h(px(T::ICON_WIDTH))
        .px(px(T::S6))
        .text_color(T::color(T::link()))
}
fn muted(text: impl Into<gpui::SharedString>) -> gpui::Div {
    div()
        .text_size(px(T::META))
        .text_color(T::color(T::muted()))
        .child(text.into())
}

impl Workbench {
    pub(super) fn task_detail_view(
        &self,
        task: &Task,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let status = task_status(&self.workspace, task);
        let sessions: Vec<AgentSession> = self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| s.task.as_ref() == Some(&task.id))
            .cloned()
            .collect();
        let mut left = div().flex_1().min_w_0().flex().flex_col().gap(px(T::S16));
        if !task.description.trim().is_empty() {
            left = left.child(
                panel()
                    .child(
                        panel_head("说明", String::new())
                            .child(div().flex_1())
                            .child(link("task-edit-description", "编辑").on_click({
                                let task = task.clone();
                                cx.listener(move |this, _, window, cx| {
                                    this.edit_task_form(&task, window, cx)
                                })
                            })),
                    )
                    .child(
                        div()
                            .px(px(T::S16))
                            .py(px(T::S12))
                            .text_color(T::color(T::body()))
                            .child(task.description.trim().to_owned()),
                    ),
            );
        }
        left = left.child(self.criteria_panel(task, cx));
        div()
            .id("task-detail")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(T::OVERVIEW_X))
            .py(px(T::S20))
            .child(
                div()
                    .w(px(T::TODO_DETAIL_WIDTH))
                    .max_w_full()
                    .mx_auto()
                    .flex()
                    .flex_col()
                    .gap(px(T::S16))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(T::S8))
                            .text_size(px(T::SMALL))
                            .text_color(T::color(T::secondary()))
                            .child(button("task-close", "← 返回画面").on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.nav.task_detail = None;
                                    cx.notify();
                                },
                            )))
                            .child("·")
                            .child("待办"),
                    )
                    .child(self.detail_head(task, status, !sessions.is_empty(), cx))
                    .children(self.agent_picker(&task.id, cx))
                    .when(!task.acceptance.is_empty(), |d| {
                        d.child(self.progress(task, status, &sessions))
                    })
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap(px(T::S16))
                            .child(left)
                            .child(
                                div()
                                    .w(px(T::TODO_DETAIL_SIDE))
                                    .flex_shrink_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(T::S16))
                                    .child(self.sessions_panel(task, &sessions, cx))
                                    .when(!sessions.is_empty(), |d| {
                                        d.child(muted(
                                        "标记完成不会停掉关联会话；它们各自的结局仍在鸟瞰里处理。",
                                    ))
                                    }),
                            ),
                    ),
            )
    }
    /// Title, status and where it runs; the main button follows the state: back to its
    /// pane while one runs, 标记完成 once every criterion is proven, 开始 with no session.
    fn detail_head(
        &self,
        task: &Task,
        status: TaskStatus,
        has_sessions: bool,
        cx: &Context<Self>,
    ) -> gpui::Div {
        let tid = task.id.clone();
        let outcome = self
            .workspace
            .task_outcomes
            .get(&tid)
            .cloned()
            .unwrap_or(Outcome::Open);
        let space = self.workspace.spaces.iter().find(|s| s.id == task.space);
        let sep = || div().opacity(0.5).child("·");
        let mut meta = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(T::S8))
            .text_size(px(T::SMALL))
            .text_color(T::color(T::secondary()))
            .child(
                div()
                    .h(px(T::STATE_CHIP))
                    .px(px(T::S8))
                    .flex()
                    .items_center()
                    .gap(px(T::S6))
                    .rounded(px(T::STATE_CHIP_RADIUS))
                    .bg(T::color(T::chip()))
                    .text_color(T::color(T::primary()))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(ring(status))
                    .child(status.label()),
            );
        if outcome == Outcome::Open {
            meta = meta.child(
                div().flex().gap(px(T::S4)).child("结局").child(
                    div()
                        .text_color(T::color(T::primary()))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child("未定"),
                ),
            );
        }
        if let Outcome::Deferred { reason, until } = &outcome {
            meta = meta
                .child(sep())
                .child(format!("{} 再看 · {reason}", upcoming(*until, now())));
        }
        meta = meta.child(sep()).child(task.agent.name());
        if let Some(space) = space {
            meta = meta.child(sep()).child(space.name.clone());
            if outcome == Outcome::Open {
                meta = meta.child(sep()).child(
                    div()
                        .font(T::mono(cx))
                        .text_size(px(T::META))
                        .min_w_0()
                        .truncate()
                        .child(home_relative(&space.cwd)),
                );
            }
        }
        let settled = matches!(status, TaskStatus::Completed | TaskStatus::Abandoned);
        let running = self.workspace.task_terminal(&tid).is_some();
        let outcome_button = |name: &'static str, to: Outcome| {
            let tid = tid.clone();
            button(name, name).on_click(cx.listener(move |this, _, _, cx| {
                this.edit(
                    Edit::SetTaskOutcome {
                        id: tid.clone(),
                        outcome: to.clone(),
                    },
                    cx,
                )
            }))
        };
        let back = || {
            let tid = tid.clone();
            button("task-back", "回到画面 →").on_click(cx.listener(move |this, _, _, cx| {
                this.nav.task_detail = None;
                this.ask_agent(tid.clone(), false, cx);
            }))
        };
        let another = || {
            let tid = tid.clone();
            button("task-another", "再开一个会话 ▾")
                .on_click(cx.listener(move |this, _, _, cx| this.ask_agent(tid.clone(), true, cx)))
        };
        let edit = {
            let task = task.clone();
            button("task-edit", "编辑").on_click(
                cx.listener(move |this, _, window, cx| this.edit_task_form(&task, window, cx)),
            )
        };
        let divider = || {
            div()
                .w(px(T::BORDER_WIDTH))
                .h(px(T::S20))
                .mx(px(T::S2))
                .bg(T::color(T::border()))
        };
        let mut actions = div().flex().flex_wrap().items_center().gap(px(T::S6));
        if settled {
            // A settled todo has nothing to push forward: only undo and edit.
            actions = actions
                .child(plain(outcome_button("撤销结局", Outcome::Open)))
                .child(edit);
        } else {
            if status == TaskStatus::Proven {
                actions = actions.child(solid(outcome_button("标记完成", Outcome::Completed)));
                if running {
                    actions = actions.child(plain(back()));
                }
                actions = actions.child(plain(another()));
            } else if running {
                actions = actions
                    .child(solid(back()))
                    .child(plain(another()))
                    .child(divider())
                    .child(plain(outcome_button("标记完成", Outcome::Completed)));
            } else {
                let start = tid.clone();
                actions = actions.child(solid(button("task-start", "开始").on_click(
                    cx.listener(move |this, _, _, cx| this.ask_agent(start.clone(), false, cx)),
                )));
                if has_sessions {
                    actions = actions.child(plain(another()));
                }
                actions = actions
                    .child(divider())
                    .child(plain(outcome_button("标记完成", Outcome::Completed)));
            }
            if status == TaskStatus::Proven {
                actions = actions.child(divider());
            }
            if let Outcome::Deferred { .. } = outcome {
                actions = actions.child(outcome_button("撤销结局", Outcome::Open));
            } else {
                actions = actions.child(button("task-defer", "稍后再看…").on_click(cx.listener(
                    |this, _, window, cx| this.show_form(FormKind::TaskDefer, window, cx),
                )));
            }
            actions = actions
                .child(outcome_button("放弃", Outcome::Abandoned))
                .child(edit);
        }
        div()
            .flex()
            .flex_wrap()
            .items_start()
            .gap(px(T::S16))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(T::S8))
                    .child(
                        div()
                            .text_size(px(T::TITLE))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(task.title.clone()),
                    )
                    .child(meta),
            )
            .child(actions)
    }
    /// The 「n / m 有证据」 strip: one cell per criterion, filled once evidence proves it.
    fn progress(&self, task: &Task, status: TaskStatus, sessions: &[AgentSession]) -> gpui::Div {
        let proven = task.proven();
        let total = task.acceptance.len();
        let note = if status == TaskStatus::Proven {
            let last = task
                .acceptance
                .iter()
                .flat_map(|c| c.evidence.iter())
                .filter(|e| e.record.proves())
                .map(|e| e.record.at)
                .max()
                .unwrap_or_default();
            let done = sessions
                .iter()
                .filter(|s| session_group(&self.workspace, s) == Group::Review)
                .count();
            [
                format!(
                    "最后一条证据 {}{}",
                    hm(last),
                    if !sessions.is_empty() && done == sessions.len() {
                        format!(" · {done} 个会话都做完了")
                    } else {
                        String::new()
                    }
                ),
                "核对后点「标记完成」".to_owned(),
            ]
        } else {
            [
                "证据只认成功的测试、提交和文件改动".to_owned(),
                "Agent 自己说「做完了」不算".to_owned(),
            ]
        };
        div()
            .flex()
            .items_center()
            .gap(px(T::S16))
            .px(px(T::S16))
            .py(px(T::S14))
            .bg(T::color(T::popover()))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::card_border()))
            .rounded(px(T::POPOVER_RADIUS))
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(T::S4))
                    .font_features(T::tabular())
                    .child(
                        div()
                            .text_size(px(T::PROGRESS_NUMBER))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(proven.to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(T::FORM_TITLE))
                            .text_color(T::color(T::muted()))
                            .child(format!("/ {total}")),
                    )
                    .child(
                        div()
                            .ml(px(T::S6))
                            .text_size(px(T::SMALL))
                            .text_color(T::color(T::secondary()))
                            .child("有证据"),
                    ),
            )
            .child(div().flex_1().min_w_0().flex().gap(px(T::S4)).children(
                task.acceptance.iter().enumerate().map(|(i, c)| {
                    let ok = c.proven();
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(T::S4))
                        .child(
                            div()
                                .h(px(T::PROGRESS_CELL))
                                .rounded(px(T::PROGRESS_CELL_RADIUS))
                                .when(ok, |d| d.bg(T::color(T::secondary())))
                                .when(!ok, |d| {
                                    d.bg(T::color(T::app()))
                                        .border(px(T::BORDER_WIDTH))
                                        .border_color(T::color(T::control()))
                                }),
                        )
                        .child(
                            muted(format!(
                                "{} · {}",
                                i + 1,
                                keepline_core::content::short(&c.description, 16)
                            ))
                            .truncate()
                            .when(ok, |d| d.text_color(T::color(T::secondary()))),
                        )
                }),
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_end()
                    .text_size(px(T::SMALL))
                    .text_color(T::color(T::secondary()))
                    .children(note),
            )
    }
    fn criteria_panel(&self, task: &Task, cx: &Context<Self>) -> gpui::Div {
        let total = task.acceptance.len();
        let mut p = panel().child(panel_head(
            "验收标准",
            if total == 0 {
                "0".into()
            } else {
                format!("{} / {total} 有证据", task.proven())
            },
        ));
        if total == 0 {
            let edit = task.clone();
            return p.child(
                div()
                    .px(px(T::S16))
                    .py(px(T::S14))
                    .flex()
                    .items_center()
                    .gap(px(T::S8))
                    .text_color(T::color(T::muted()))
                    .child("没有写验收标准，这条待办靠你自己判断完成")
                    .child(div().flex_1())
                    .child(link("task-criteria-add", "补写").on_click(cx.listener(
                        move |this, _, window, cx| this.edit_task_form(&edit, window, cx),
                    ))),
            );
        }
        for (i, criterion) in task.acceptance.iter().enumerate() {
            p = p.child(self.criterion_row(task, i, criterion, i + 1 == total, cx));
        }
        p
    }
    fn criterion_row(
        &self,
        task: &Task,
        i: usize,
        criterion: &AcceptanceCriterion,
        last: bool,
        cx: &Context<Self>,
    ) -> gpui::Div {
        let ok = criterion.proven();
        // Newest first: what proves it, and the attempts that did not.
        let mut proofs: Vec<(usize, &CriterionEvidence)> = criterion
            .evidence
            .iter()
            .enumerate()
            .filter(|(_, e)| e.record.proves())
            .collect();
        let mut fails: Vec<(usize, &CriterionEvidence)> = criterion
            .evidence
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.record.proves())
            .collect();
        proofs.sort_by_key(|e| std::cmp::Reverse(e.1.record.at));
        fails.sort_by_key(|e| std::cmp::Reverse(e.1.record.at));
        let count = match (proofs.len(), fails.len()) {
            (0, 0) => "0 条".to_owned(),
            (0, f) => format!("0 条 · {f} 次失败"),
            (n, 0) => format!("{n} 条证据"),
            (n, f) => format!("{n} 条证据 · {f} 次失败"),
        };
        let expanded = self.tasks.expanded.contains(&criterion.id);
        let mut body = div().flex().flex_col().gap(px(T::S2)).mt(px(T::S4));
        let available = attachable(&self.workspace, &task.id, Some(criterion));
        if expanded {
            for (at, e) in proofs.iter().chain(fails.iter()) {
                body = body.child(self.evidence_row(task, i, *at, e, false, true, cx));
            }
        } else if let Some((at, e)) = proofs.first() {
            body = body.child(self.evidence_row(task, i, *at, e, false, false, cx));
            let mut more = Vec::new();
            if let Some((_, next)) = proofs.get(1) {
                more.push(format!(
                    "另 {} 条：{} · {}",
                    proofs.len() - 1,
                    keepline_core::content::short(&next.record.summary, 28),
                    hm(next.record.at)
                ));
            }
            if let Some((_, f)) = fails.first() {
                more.push(format!(
                    "{} {} 次失败：{} · {}",
                    if f.record.at < e.record.at {
                        "更早"
                    } else {
                        "之后又有"
                    },
                    fails.len(),
                    result_name(&f.record),
                    hm(f.record.at)
                ));
            }
            if !more.is_empty() {
                let cid = criterion.id.clone();
                body = body.child(
                    button(("evidence-more", i), more.join("；"))
                        .h(px(T::ICON_HEIGHT))
                        .px(px(T::S8))
                        .justify_start()
                        .text_size(px(T::META))
                        .text_color(T::color(T::link()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.tasks.expanded.insert(cid.clone());
                            cx.notify();
                        })),
                );
            }
        } else if let Some((at, e)) = fails.first() {
            body = body.child(self.evidence_row(task, i, *at, e, true, false, cx));
        } else {
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(T::S8))
                    .px(px(T::S8))
                    .py(px(T::S4))
                    .text_size(px(T::SMALL))
                    .text_color(T::color(T::muted()))
                    .child(if available.is_empty() {
                        "还没有证据 · 关联会话里还没有测试、提交或文件改动".to_owned()
                    } else {
                        format!("还没有证据 · 关联会话里有 {} 条记录可以挂", available.len())
                    }),
            );
        }
        let cid = criterion.id.clone();
        let picking = self.tasks.attaching.as_ref() == Some(&criterion.id);
        body = body.child(
            div()
                .flex()
                .gap(px(T::S4))
                .when(!available.is_empty() || picking, |d| {
                    let cid = cid.clone();
                    d.child(
                        link(
                            ("evidence-attach", i),
                            if picking {
                                "收起"
                            } else {
                                "挂一条证据 ▾"
                            },
                        )
                        .text_size(px(T::META))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.tasks.attaching = if this.tasks.attaching.as_ref() == Some(&cid) {
                                None
                            } else {
                                Some(cid.clone())
                            };
                            cx.notify();
                        })),
                    )
                })
                .when(expanded, |d| {
                    let cid = cid.clone();
                    d.child(
                        link(("evidence-fold", i), "收起较早的证据")
                            .text_size(px(T::META))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.tasks.expanded.remove(&cid);
                                cx.notify();
                            })),
                    )
                }),
        );
        if picking {
            body = body.child(self.evidence_picker(task, i, &available, cx));
        }
        div()
            .flex()
            .gap(px(T::S10))
            .pl(px(T::S14))
            .pr(px(T::S16))
            .py(px(T::S12))
            .when(!last, |d| {
                d.border_b(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::border()))
            })
            .child(
                div()
                    .size(px(T::CRITERION_BOX))
                    .mt(px(T::S2))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(T::CHIP_RADIUS))
                    .border(px(T::CARET_WIDTH))
                    .border_color(T::color(T::control()))
                    .text_size(px(T::CHIP_SIZE))
                    .text_color(T::color(T::on_accent()))
                    .when(ok, |d| {
                        d.bg(T::color(T::secondary()))
                            .border_color(T::color(T::secondary()))
                            .child("✓")
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(T::S8))
                            .child(muted((i + 1).to_string()).text_size(px(T::SMALL)))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_color(T::color(T::primary()))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child(criterion.description.clone()),
                            )
                            .child(muted(count).flex_shrink_0()),
                    )
                    .child(body),
            )
    }
    /// One attached record: its kind, what it says, how it ended, when, and where from.
    #[allow(clippy::too_many_arguments)]
    fn evidence_row(
        &self,
        task: &Task,
        criterion: usize,
        index: usize,
        e: &CriterionEvidence,
        last_try: bool,
        removable: bool,
        cx: &Context<Self>,
    ) -> gpui::Div {
        let failed = !e.record.proves();
        let session = self
            .workspace
            .agent_sessions
            .iter()
            .find(|s| s.id == e.session)
            .cloned();
        let from = session
            .as_ref()
            .map_or_else(|| "已关闭的会话".to_owned(), |s| self.session_title(s));
        let path = e.record.source.path.clone();
        let key = format!("{criterion}-{index}");
        div()
            .flex()
            .flex_col()
            .px(px(T::S8))
            .py(px(T::S4))
            .rounded(px(T::CONTROL_RADIUS))
            .hover(|s| s.bg(T::color(T::raised())))
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(T::S10))
                    .text_size(px(T::SMALL))
                    .child(
                        muted(kind_name(&e.record))
                            .w(px(T::EVIDENCE_KIND))
                            .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font(T::mono(cx))
                            .text_size(px(T::META))
                            .text_color(T::color(if failed { T::muted() } else { T::body() }))
                            .child(e.record.summary.clone()),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(T::CHIP_SIZE))
                            .text_color(if failed {
                                Group::Error.colors().1
                            } else {
                                T::color(T::secondary())
                            })
                            .when(failed, |d| d.font_weight(gpui::FontWeight::SEMIBOLD))
                            .child(result_name(&e.record)),
                    )
                    .child(muted(hm(e.record.at)).flex_shrink_0()),
            )
            .child(
                div()
                    .pl(px(T::EVIDENCE_KIND + T::S10))
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(T::S4))
                    .text_size(px(T::CHIP_SIZE))
                    .text_color(T::color(T::muted()))
                    .child(
                        div()
                            .id(gpui::SharedString::from(format!("evidence-from-{key}")))
                            .cursor_pointer()
                            .hover(|s| s.text_color(T::color(T::link())))
                            .child(format!("来自「{from}」"))
                            .when_some(session, |d, s| {
                                d.on_click(cx.listener(move |this, _, window, cx| {
                                    this.nav.task_detail = None;
                                    this.enter_card(&s, window, cx);
                                }))
                            }),
                    )
                    .when(last_try, |d| d.child("· 最近一次尝试，不算证据"))
                    .child("·")
                    .child(
                        div()
                            .id(gpui::SharedString::from(format!("evidence-source-{key}")))
                            .cursor_pointer()
                            .text_color(T::color(T::link()))
                            .child("看原文 ↗")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                // The session log itself, opened as text where the record sits.
                                if let Err(e) = std::process::Command::new("open")
                                    .args(["-t", &path])
                                    .spawn()
                                {
                                    this.error = Some(format!("无法打开会话原文：{e}"));
                                    cx.notify();
                                }
                            })),
                    )
                    .when(removable || last_try, |d| {
                        let (tid, cid, gone) = (
                            task.id.clone(),
                            task.acceptance[criterion].id.clone(),
                            e.clone(),
                        );
                        d.child("·").child(
                            div()
                                .id(gpui::SharedString::from(format!("evidence-remove-{key}")))
                                .cursor_pointer()
                                .hover(|s| s.text_color(T::color(T::link())))
                                .child("移除")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.change_criterion(
                                        &tid,
                                        &cid,
                                        |c| c.evidence.retain(|x| x != &gone),
                                        cx,
                                    )
                                })),
                        )
                    }),
            )
    }
    /// The records the todo's sessions left that this criterion can hold; a click attaches one.
    fn evidence_picker(
        &self,
        task: &Task,
        criterion: usize,
        available: &[CriterionEvidence],
        cx: &Context<Self>,
    ) -> gpui::Div {
        let mut list = div()
            .mt(px(T::S4))
            .flex()
            .flex_col()
            .p(px(T::S4))
            .rounded(px(T::CONTROL_RADIUS))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::card_border()))
            .bg(T::color(T::pane()));
        for (n, e) in available.iter().take(12).enumerate() {
            let failed = !e.record.proves();
            let from = self
                .workspace
                .agent_sessions
                .iter()
                .find(|s| s.id == e.session)
                .map(|s| self.session_title(s))
                .unwrap_or_default();
            let (tid, cid, attach) = (
                task.id.clone(),
                task.acceptance[criterion].id.clone(),
                e.clone(),
            );
            list = list.child(
                div()
                    .id(("evidence-pick", n))
                    .flex()
                    .items_baseline()
                    .gap(px(T::S10))
                    .px(px(T::S8))
                    .py(px(T::S4))
                    .rounded(px(T::CHIP_RADIUS))
                    .cursor_pointer()
                    .hover(|s| s.bg(T::color(T::raised())))
                    .text_size(px(T::SMALL))
                    .child(
                        muted(kind_name(&e.record))
                            .w(px(T::EVIDENCE_KIND))
                            .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font(T::mono(cx))
                            .text_size(px(T::META))
                            .child(e.record.summary.clone()),
                    )
                    .child(
                        muted(if failed {
                            format!("{} · 失败，只记作尝试", result_name(&e.record))
                        } else {
                            result_name(&e.record)
                        })
                        .flex_shrink_0()
                        .when(failed, |d| d.text_color(Group::Error.colors().1)),
                    )
                    .child(
                        muted(format!(
                            "{} · {}",
                            hm(e.record.at),
                            keepline_core::content::short(&from, 10)
                        ))
                        .flex_shrink_0(),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.tasks.attaching = None;
                        this.change_criterion(&tid, &cid, |c| c.evidence.push(attach.clone()), cx)
                    })),
            );
        }
        list
    }
    /// Changes one criterion of the todo as it is now and saves the whole todo.
    fn change_criterion(
        &mut self,
        task: &str,
        criterion: &str,
        change: impl FnOnce(&mut AcceptanceCriterion),
        cx: &mut Context<Self>,
    ) {
        let Some(mut t) = self.workspace.tasks.iter().find(|t| t.id == task).cloned() else {
            return;
        };
        if let Some(c) = t.acceptance.iter_mut().find(|c| c.id == criterion) {
            change(c);
            self.edit(Edit::UpdateTask(t), cx);
        }
    }
    fn sessions_panel(
        &self,
        task: &Task,
        sessions: &[AgentSession],
        cx: &Context<Self>,
    ) -> gpui::Div {
        let mut p = panel().child(panel_head("关联会话", sessions.len().to_string()));
        if sessions.is_empty() {
            let tid = task.id.clone();
            return p.child(
                div()
                    .px(px(T::S16))
                    .py(px(T::S14))
                    .flex()
                    .items_center()
                    .gap(px(T::S8))
                    .text_color(T::color(T::muted()))
                    .child("没有会话做过这条待办")
                    .child(div().flex_1())
                    .child(link("task-session-start", "再开一个会话 ▾").on_click(
                        cx.listener(move |this, _, _, cx| this.ask_agent(tid.clone(), false, cx)),
                    )),
            );
        }
        let at = now();
        let count = sessions.len();
        for (n, session) in sessions.iter().enumerate() {
            let group = session_group(&self.workspace, session);
            let updated = self
                .workspace
                .content
                .get(&session.id)
                .map(|c| c.updated)
                .filter(|u| *u > 0)
                .unwrap_or(session.updated);
            let open = session.clone();
            p = p.child(
                div()
                    .id(gpui::SharedString::from(format!(
                        "task-session-{}",
                        session.id
                    )))
                    .flex()
                    .items_center()
                    .gap(px(T::S10))
                    .px(px(T::S16))
                    .py(px(T::S10))
                    .cursor_pointer()
                    .hover(|s| s.bg(T::color(T::raised())))
                    .when(n + 1 < count, |d| {
                        d.border_b(px(T::BORDER_WIDTH))
                            .border_color(T::color(T::border()))
                    })
                    .child(dot(group))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(T::S2))
                            .child(
                                div()
                                    .truncate()
                                    .text_color(T::color(T::primary()))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child(self.session_title(session)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap(px(T::S6))
                                    .text_size(px(T::META))
                                    .text_color(T::color(T::muted()))
                                    .child(
                                        div()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(group.colors().1)
                                            .child(group.label()),
                                    )
                                    .child(session.runtime.name())
                                    .child(ago(updated, at))
                                    .when(
                                        self.workspace
                                            .content
                                            .get(&session.id)
                                            .is_some_and(|c| c.waking()),
                                        |d| d.child("等待唤醒"),
                                    ),
                            ),
                    )
                    .child(muted("打开 →").text_color(T::color(T::link())))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.nav.task_detail = None;
                        this.enter_card(&open, window, cx);
                    })),
            );
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keepline_core::content::Content;

    fn record(kind: &str, exit_code: Option<i32>, at: u64) -> Evidence {
        Evidence {
            kind: kind.into(),
            summary: kind.into(),
            exit_code,
            at,
            source: Default::default(),
        }
    }
    #[test]
    fn status_follows_outcome_sessions_and_evidence() {
        let mut w = super::super::tests::fixture();
        let mut task = Task {
            id: "task".into(),
            space: "space".into(),
            title: "标签命名".into(),
            description: String::new(),
            acceptance: vec![AcceptanceCriterion {
                id: "c".into(),
                description: "测试通过".into(),
                evidence: vec![],
            }],
            agent: Agent::Codex,
        };
        w.tasks.push(task.clone());
        assert_eq!(task_status(&w, &task), TaskStatus::NotStarted);
        w.agent_sessions[0].task = Some(task.id.clone());
        assert_eq!(task_status(&w, &task), TaskStatus::Working);
        let sid = w.agent_sessions[0].id.clone();
        w.content.insert(
            sid.clone(),
            Content {
                evidence: vec![
                    record("command", Some(0), 1),
                    record("test", Some(101), 2),
                    record("file", Some(0), 3),
                ],
                ..Default::default()
            },
        );
        // Plain commands are never offered; the newest record comes first.
        let offered = attachable(&w, &task.id, None);
        assert_eq!(
            offered.iter().map(|e| e.record.at).collect::<Vec<_>>(),
            [3, 2]
        );
        // A failed test on the criterion is an attempt: still 进行中.
        task.acceptance[0].evidence.push(offered[1].clone());
        assert_eq!(task_status(&w, &task), TaskStatus::Working);
        task.acceptance[0].evidence.push(offered[0].clone());
        assert_eq!(task_status(&w, &task), TaskStatus::Proven);
        assert!(attachable(&w, &task.id, Some(&task.acceptance[0])).is_empty());
        w.task_outcomes.insert(task.id.clone(), Outcome::Completed);
        assert_eq!(task_status(&w, &task), TaskStatus::Completed);
    }
}
