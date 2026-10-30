//! The left column: spaces, the current space's sessions and its (folded) todos.
use super::*;

/// Rows before the list hands off to the overview.
const SIDEBAR_SESSIONS: usize = 12;
/// Row order by the shown state: what needs you, then live work, then the rest.
const SIDEBAR_ORDER: [Group; 9] = [
    Group::Waiting,
    Group::Error,
    Group::Done,
    Group::Working,
    Group::Running,
    Group::Review,
    Group::Paused,
    Group::Ready,
    Group::Stale,
];
pub(super) fn sidebar_rank(g: Group, attention: bool) -> (bool, usize) {
    (
        !attention,
        SIDEBAR_ORDER
            .iter()
            .position(|o| *o == g)
            .unwrap_or(SIDEBAR_ORDER.len()),
    )
}
/// A closed session the sidebar still counts: no outcome yet and not gone quiet past
/// the stale days; older ones wait in the overview's 很久没动.
pub(super) fn closed_recently(w: &Workspace, s: &AgentSession) -> bool {
    s.outcome.needs_review(now()) && session_group(w, s) != Group::Stale
}

impl Workbench {
    pub(super) fn sidebar_view(
        &self,
        space_name: &str,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let mut sidebar = div()
            .id("sidebar-content")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(T::S8))
            .py(px(T::S12))
            .flex()
            .flex_col()
            .gap(px(T::S2))
            .child(label("空间").px(px(T::S8)).mb(px(T::S4)));
        for space in &self.workspace.spaces {
            let id = space.id.clone();
            // Over the overview the sidebar's spaces are its space filter.
            let active = if self.nav.overview {
                self.overview.filter_space.as_ref() == Some(&id)
            } else {
                self.nav.space == id
            };
            let sessions: Vec<_> = self
                .workspace
                .agent_sessions
                .iter()
                .filter(|s| {
                    self.session_hidden(s).is_none()
                        && self.session_open(s)
                        && session_space(&self.workspace, s) == Some(id.as_str())
                })
                .collect();
            let count = sessions.len();
            let waiting = sessions
                .iter()
                .filter(|s| {
                    s.managed()
                        && s.outcome.needs_review(now())
                        && session_group(&self.workspace, s) == Group::Waiting
                })
                .count();
            let errors = sessions
                .iter()
                .filter(|s| {
                    s.managed()
                        && s.outcome.needs_review(now())
                        && session_group(&self.workspace, s) == Group::Error
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
                            .map(|s| session_group(&self.workspace, s))
                            .find(|g| !matches!(g, Group::Waiting | Group::Error))
                            .unwrap_or(Group::Stale)
                    }))
                    .child(div().min_w_0().truncate().child(space.name.clone()))
                    // A deleted worktree leaves the space pointing nowhere; new terminals
                    // fall back to home, and a double-click fixes the directory.
                    .when(!Path::new(&space.cwd).is_dir(), |d| {
                        d.child(label("目录不在了").text_size(px(T::CHIP_SIZE)))
                    })
                    .justify_start()
                    .px(px(T::S8))
                    .border_0()
                    .when(active, |s| s.bg(T::color(T::raised())))
                    .child(div().flex_1())
                    .when(waiting > 0, |d| d.child(badge(Group::Waiting, waiting)))
                    .when(errors > 0, |d| d.child(badge(Group::Error, errors)))
                    .child(label(format!("共 {count}")).text_size(px(T::CHIP_SIZE)))
                    .on_click(
                        cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                            if event.click_count() == 2 {
                                this.tasks.editing_space = Some(id.clone());
                                this.show_form(FormKind::Space, window, cx);
                                return;
                            }
                            if this.nav.overview {
                                this.overview.filter_space = Some(id.clone());
                            }
                            this.nav.space = id.clone();
                            this.nav.tab = this
                                .workspace
                                .tabs
                                .iter()
                                .find(|t| t.space == id)
                                .map(|t| t.id.clone());
                            this.sync_panes(cx);
                            cx.notify();
                        }),
                    ),
            );
        }
        sidebar = sidebar.child(
            button("add-space", "＋ 空间")
                .border_0()
                .justify_start()
                .text_color(T::color(T::muted()))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.tasks.editing_space = None;
                    this.show_form(FormKind::Space, window, cx)
                })),
        );
        // Sorted by what each row shows. Within a state, newest started first: the
        // host keeps sessions in creation order, while `updated` ticks with every
        // output line and would move rows under the pointer.
        // Only sessions open in a tab; closed ones live in the overview, ready to resume.
        let in_space: Vec<_> = self
            .workspace
            .agent_sessions
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                self.session_hidden(s).is_none()
                    && session_space(&self.workspace, s) == Some(self.nav.space.as_str())
            })
            .collect();
        let closed = in_space
            .iter()
            .filter(|(_, s)| self.session_closed(s) && closed_recently(&self.workspace, s))
            .count();
        let mut sessions: Vec<_> = in_space
            .into_iter()
            .filter(|(_, s)| self.session_open(s))
            .map(|(i, s)| {
                let (g, attention) = sidebar_status(&self.workspace, &self.attention.unseen, s);
                (s, g, attention, i)
            })
            .collect();
        sessions.sort_by_key(|(_, g, attention, i)| {
            (sidebar_rank(*g, *attention), std::cmp::Reverse(*i))
        });
        let current = self
            .nav
            .focused
            .as_ref()
            .and_then(|id| self.workspace.current_agent(id))
            .map(|s| s.id.clone());
        // The session you are in stays listed even when others outrank it.
        if let Some(i) = sessions
            .iter()
            .position(|(s, ..)| current.as_deref() == Some(s.id.as_str()))
            .filter(|i| *i >= SIDEBAR_SESSIONS)
        {
            let row = sessions.remove(i);
            sessions.insert(SIDEBAR_SESSIONS - 1, row);
        }
        // Agents running in a tab here that no evidence has tied to a conversation yet:
        // listed rather than missing, so a CLI Keepline cannot read is never silent.
        let mut unread: Vec<_> = self
            .store
            .recognition()
            .terminal_hints
            .iter()
            .filter(|(terminal, _)| {
                self.workspace.current_agent(terminal).is_none()
                    && self
                        .workspace
                        .terminal_tab(terminal)
                        .is_some_and(|tab| tab.space == self.nav.space)
            })
            .map(|(terminal, hint)| (terminal.clone(), super::layout::unread_agent(hint)))
            .collect();
        unread.sort_by(|a, b| a.0.cmp(&b.0));
        sidebar = sidebar.child(self.todos_view(space_name, cx));
        sidebar = sidebar.child(
            label(format!("会话 · {}", sessions.len() + unread.len()))
                .mt(px(T::S14))
                .pt(px(T::S12))
                .px(px(T::S8))
                .mb(px(T::S4)),
        );
        if sessions.is_empty() && unread.is_empty() {
            sidebar = sidebar.child(
                label("没有打开的会话。在终端里运行 codex 或 claude 就会出现在这里").px(px(T::S8)),
            );
        }
        let more = sessions.len().saturating_sub(SIDEBAR_SESSIONS);
        let seen = crate::ui::status_colors::StatusColors::get();
        for (session, g, attention, _) in sessions.into_iter().take(SIDEBAR_SESSIONS) {
            let open = session.clone();
            sidebar =
                sidebar.child(
                    div()
                        .id(gpui::SharedString::from(format!("session-{}", session.id)))
                        .h(px(T::ROW))
                        .px(px(T::S8))
                        .flex()
                        .items_center()
                        .gap(px(T::S8))
                        .rounded(px(T::CONTROL_RADIUS))
                        .cursor_pointer()
                        .when(current.as_deref() == Some(session.id.as_str()), |d| {
                            d.bg(T::color(T::raised()))
                        })
                        .hover(|s| s.bg(T::color(T::raised())))
                        .child(if attention { dot(g) } else { seen_dot(g) })
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(px(T::MONO_SIZE))
                                .when(!attention, |d| d.text_color(T::color(seen.seen_title())))
                                .child(self.session_title(session)),
                        )
                        // Rows that all wait for your next step tell apart by how long they have.
                        .child(
                            label(if g == Group::Ready && !attention {
                                match age(self.session_updated(session)) {
                                    a if a == "刚刚" => a,
                                    a => format!("{a}前"),
                                }
                            } else {
                                g.label().to_owned()
                            })
                            .text_size(px(T::CHIP_SIZE))
                            .text_color(if attention {
                                g.colors().1
                            } else {
                                T::color(seen.seen_status())
                            }),
                        )
                        // Straight to its pane; an exited one resumes in a new pane.
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_session(&open, window, cx)
                        })),
                );
        }
        for (terminal, (note, why)) in unread {
            let focused = self.nav.focused.as_deref() == Some(terminal.as_str());
            sidebar = sidebar.child(
                div()
                    .id(gpui::SharedString::from(format!("unread-row-{terminal}")))
                    .h(px(T::ROW))
                    .px(px(T::S8))
                    .flex()
                    .items_center()
                    .gap(px(T::S8))
                    .rounded(px(T::CONTROL_RADIUS))
                    .cursor_pointer()
                    .when(focused, |d| d.bg(T::color(T::raised())))
                    .hover(|s| s.bg(T::color(T::raised())))
                    .child(seen_dot(Group::Stale))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(T::MONO_SIZE))
                            .text_color(T::color(T::muted()))
                            .child(note),
                    )
                    .tooltip(move |_, cx| {
                        cx.new(|_| super::render::TextTip(why.clone().into()))
                            .into()
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.focus_terminal(&terminal, window, cx);
                    })),
            );
        }
        if more > 0 {
            sidebar = sidebar.child(
                button(
                    "sidebar-more-sessions",
                    format!("另有 {more} 个 · 鸟瞰全部"),
                )
                .border_0()
                .justify_start()
                .text_size(px(T::META))
                .text_color(T::color(T::link()))
                .on_click(cx.listener(|this, _, _, cx| this.show_overview(cx))),
            );
        }
        if closed > 0 {
            sidebar = sidebar.child(
                button(
                    "sidebar-closed-sessions",
                    format!("已关闭 {closed} 个 · 点开接着聊"),
                )
                .border_0()
                .justify_start()
                .text_size(px(T::META))
                .text_color(T::color(T::muted()))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.show_overview(cx);
                    this.overview.closed_only = true;
                    this.overview.changed_only = false;
                    this.overview.filter_space = Some(this.nav.space.clone());
                })),
            );
        }
        div()
            .id("sidebar")
            .w(px(T::SIDEBAR))
            .h_full()
            .min_h_0()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(T::color(T::panel()))
            .border_r(px(T::BORDER_WIDTH))
            .border_color(T::color(T::border()))
            .child(sidebar)
            .child(
                div()
                    .flex_shrink_0()
                    .border_t(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::border()))
                    .px(px(T::S8))
                    .py(px(T::S8))
                    .flex()
                    .gap(px(T::S4))
                    .child(
                        button("diagnose-host", "本机诊断")
                            .flex_1()
                            .justify_start()
                            .border_0()
                            .on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.show_diagnostics(window, cx)
                                }),
                            ),
                    )
                    .child(button("open-settings", "设置").border_0().on_click(
                        cx.listener(|this, _, window, cx| this.show_settings(window, cx)),
                    )),
            )
    }
    /// The space's todos: what is still to be handed to an Agent, above what is running.
    fn todos_view(&self, space_name: &str, cx: &Context<Self>) -> gpui::Div {
        let mut sidebar = div().flex().flex_col();
        let todo_count = self
            .workspace
            .tasks
            .iter()
            .filter(|t| t.space == self.nav.space)
            .count();
        let open = self.tasks.todos_open;
        sidebar = sidebar
            .child(
                div()
                    .id("todos-toggle")
                    .mt(px(T::S14))
                    .pt(px(T::S12))
                    .px(px(T::S8))
                    .mb(px(T::S4))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .child(
                        label(format!(
                            "{} {space_name} 的待办 · {todo_count}",
                            if open { "▾" } else { "▸" }
                        ))
                        .flex_1(),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.tasks.todos_open = !this.tasks.todos_open;
                        cx.notify();
                    }))
                    .child(
                        button("sidebar-new-task", "＋")
                            .aria_label("新待办")
                            .border_0()
                            .text_color(T::color(T::link()))
                            .on_click(cx.listener(|this, _, window, cx| {
                                // The header toggles on click; ＋ must only open it.
                                cx.stop_propagation();
                                this.show_quick_task(window, cx)
                            })),
                    ),
            )
            .children(self.quick_task_row(cx));
        if !open {
            return sidebar;
        }
        // First open: say what the empty pane is for before asking for a task.
        if self.tasks.quick_task.is_none()
            && !self
                .workspace
                .tasks
                .iter()
                .any(|t| t.space == self.nav.space)
        {
            sidebar = sidebar.child(
                div()
                    .px(px(T::S8))
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap(px(T::S6))
                    .child(label(
                        "还没有待办。先记下要做的事，再点「开始」派给 agent。",
                    ))
                    .child(
                        button("empty-new-task", "＋ 新建待办")
                            .h(px(T::ICON_HEIGHT))
                            .px(px(T::S6))
                            .text_size(px(T::META))
                            .text_color(T::color(T::link()))
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
            .filter(|t| t.space == self.nav.space)
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
                    .hover(|s| s.bg(T::color(T::raised())))
                    .child(
                        div()
                            .size(px(T::S8))
                            .flex_shrink_0()
                            .rounded_full()
                            .border(px(T::BORDER_WIDTH))
                            .border_color(T::color(T::link()))
                            .when(count > 0, |d| d.bg(T::color(T::accent()))),
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
                        .text_color(T::color(T::link()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.ask_agent(start.clone(), false, cx);
                        })),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.nav.task_detail = Some(tid.clone());
                        cx.notify();
                    })),
            );
            if let Some(picker) = self.agent_picker(&task.id, cx) {
                sidebar = sidebar.child(picker);
            }
        }
        sidebar
    }
}
