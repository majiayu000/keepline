//! The left column: spaces, the current space's sessions and its (folded) todos.
use super::*;

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

/// A space's row figures: its open sessions, those waiting on you and those failed, and
/// the state its dot shows.
struct SpaceCounts {
    count: usize,
    waiting: usize,
    errors: usize,
    group: Group,
}

impl Workbench {
    /// 鸟瞰 narrowed to this space's closed sessions, ready to resume.
    pub(super) fn show_closed(&mut self, cx: &mut Context<Self>) {
        self.show_overview(cx);
        self.overview.closed_only = true;
        self.overview.changed_only = false;
        self.overview.filter_space = Some(self.nav.space.clone());
    }
    pub(super) fn sidebar_view(
        &self,
        space_name: &str,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        if self.sidebar.collapsed {
            return self.sidebar_rail(cx);
        }
        let dense = self.sidebar.dense();
        let mut sidebar = div()
            .id("sidebar-content")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(T::S8))
            .py(px(if dense { T::S8 } else { T::S12 }))
            .flex()
            .flex_col()
            .gap(px(T::S2))
            .font_features(super::diagnostics::tabular());
        if dense {
            sidebar = sidebar.child(self.space_line(cx));
        } else {
            sidebar = sidebar.child(
                div()
                    .flex()
                    .px(px(T::S8))
                    .mb(px(T::S4))
                    .child(label("空间").flex_1())
                    .child(label(self.workspace.spaces.len().to_string())),
            );
        }
        for space in self.workspace.spaces.iter().filter(|_| !dense) {
            let id = space.id.clone();
            let active = self.space_active(&id);
            let SpaceCounts {
                count,
                waiting,
                errors,
                group,
            } = self.space_counts(&id);
            if let Some((_, field, _)) = self
                .layout
                .renaming_space
                .as_ref()
                .filter(|(s, ..)| s.id == id)
            {
                // Named in place, like a tab: the row keeps its height and dot, and only
                // the name turns editable, outlined so the edit is plain to see.
                sidebar = sidebar.child(
                    div()
                        .h(px(T::ROW))
                        .px(px(T::S8 - T::BORDER_WIDTH))
                        .flex()
                        .items_center()
                        .gap(px(T::S8))
                        .rounded(px(T::CONTROL_RADIUS))
                        .bg(T::color(T::raised()))
                        .border(px(T::BORDER_WIDTH))
                        .border_color(T::color(T::focus()))
                        .text_size(px(T::SMALL))
                        .child(dot(group))
                        .child(div().flex_1().min_w_0().h_full().child(field.clone())),
                );
                continue;
            }
            let missing = !Path::new(&space.cwd).is_dir();
            sidebar = sidebar.child(
                button(gpui::SharedString::from(format!("space-{id}")), "")
                    .aria_label(space.name.clone())
                    .gap(px(T::S8))
                    .child(dot(group))
                    .child(div().min_w_0().truncate().child(space.name.clone()))
                    // The branch the space's directory is on, as herdr's sidebar shows it.
                    .when_some(self.git_branch(&space.cwd, cx), |d, branch| {
                        d.child(
                            label(branch)
                                .min_w_0()
                                .max_w(px(T::SIDEBAR * 0.4))
                                .truncate()
                                .text_size(px(T::CHIP_SIZE))
                                .text_color(T::color(T::muted())),
                        )
                    })
                    .when_some(
                        self.git_divergence(&space.cwd, cx)
                            .and_then(super::divergence::divergence_label),
                        |d, counts| {
                            d.child(
                                label(counts)
                                    .flex_shrink_0()
                                    .text_size(px(T::CHIP_SIZE))
                                    .text_color(T::color(T::muted()))
                                    .font_features(T::tabular()),
                            )
                        },
                    )
                    // A deleted worktree leaves the space pointing nowhere; new terminals
                    // fall back to home, and a click here picks the folder again.
                    .when(missing, |d| {
                        let id = id.clone();
                        d.child(
                            button(
                                gpui::SharedString::from(format!("space-missing-{id}")),
                                "目录不在了",
                            )
                            .aria_label(format!("重新选择 {} 的目录", space.name))
                            .flex_shrink_0()
                            .h(px(T::ICON_HEIGHT))
                            .px(px(T::S4))
                            .border_0()
                            .rounded(px(T::CHIP_RADIUS))
                            .bg(T::color(T::chip()))
                            .text_size(px(T::CHIP_SIZE))
                            .text_color(T::color(T::secondary()))
                            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                                cx.stop_propagation()
                            })
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    this.choose_space_dir(Some(id.clone()), window, cx);
                                },
                            )),
                        )
                    })
                    .justify_start()
                    .px(px(T::S8))
                    .border_0()
                    .when(active, |s| s.bg(T::color(T::raised())))
                    .child(div().flex_1())
                    .when(waiting > 0, |d| d.child(badge(Group::Waiting, waiting)))
                    .when(errors > 0, |d| d.child(badge(Group::Error, errors)))
                    .child(
                        label(count.to_string())
                            .flex_shrink_0()
                            .text_size(px(T::CHIP_SIZE))
                            .text_color(T::color(T::muted())),
                    )
                    .on_mouse_down(gpui::MouseButton::Right, {
                        let id = id.clone();
                        cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                            this.layout.space_menu = Some((id.clone(), event.position));
                            this.layout.space_switch = false;
                            this.layout.space_removing = false;
                            cx.stop_propagation();
                            cx.notify();
                        })
                    })
                    .on_click(
                        cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                            if event.click_count() == 2 {
                                this.rename_space(&id, window, cx);
                                return;
                            }
                            this.pick_space(&id, cx);
                        }),
                    ),
            );
        }
        if !dense {
            sidebar = sidebar.child(
                button("add-space", "＋ 空间")
                    .border_0()
                    .justify_start()
                    .text_color(T::color(T::muted()))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.choose_space_dir(None, window, cx)),
                    ),
            );
        }
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
        // A WASM preset is handed the newest closed sessions too, to place as it likes.
        let mut ended: Vec<_> = in_space
            .iter()
            .filter(|(_, s)| self.session_closed(s))
            .map(|(_, s)| {
                let (g, attention) = sidebar_status(&self.workspace, &self.attention.unseen, s);
                (*s, g, attention)
            })
            .collect();
        ended.sort_by_key(|(s, ..)| std::cmp::Reverse(self.session_updated(s)));
        let ended = super::sidebar_wasm::Closed {
            total: ended.len(),
            rows: ended
                .into_iter()
                .take(super::sidebar_wasm::CLOSED_SHOWN)
                .collect(),
        };
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
        // Agents running in a tab here that no evidence has tied to a conversation yet:
        // listed rather than missing, so a CLI Keepline cannot read is never silent.
        let mut unread: Vec<_> = self
            .store
            .recognition()
            .terminal_notes
            .iter()
            .filter(|(terminal, _)| {
                self.workspace.current_agent(terminal).is_none()
                    && self
                        .workspace
                        .terminal_tab(terminal)
                        .is_some_and(|tab| tab.space == self.nav.space)
            })
            .map(|(terminal, n)| (terminal.clone(), (n.note.clone(), n.why.clone())))
            .collect();
        unread.sort_by(|a, b| a.0.cmp(&b.0));
        sidebar = sidebar.child(self.todos_view(space_name, dense, cx));
        if sessions.is_empty() && unread.is_empty() {
            sidebar = sidebar.child(
                label("没有打开的会话。在终端里运行 codex 或 claude 就会出现在这里")
                    .mt(px(T::S14))
                    .px(px(T::S8)),
            );
        }
        let listed = sessions.into_iter().map(|(s, g, a, _)| (s, g, a)).collect();
        let (rows, more, owns_closed) = self.session_list(listed, &ended, current.as_deref(), cx);
        sidebar = sidebar.children(rows);
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
        if closed > 0 && !owns_closed {
            sidebar = sidebar.child(
                button(
                    "sidebar-closed-sessions",
                    if dense {
                        format!("已关闭 {closed} · ⌘P 找回")
                    } else {
                        format!("已关闭 {closed} 个 · 点开接着聊")
                    },
                )
                .border_0()
                .justify_start()
                .when(dense, |d| {
                    d.mt(px(T::S8)).px(px(T::S8)).font_family(T::UI_FONT)
                })
                .text_size(px(T::META))
                .text_color(T::color(T::muted()))
                .on_click(cx.listener(|this, _, _, cx| this.show_closed(cx))),
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
            .child(
                div()
                    .flex_shrink_0()
                    .px(px(T::S8))
                    .pt(px(T::S8))
                    .flex()
                    .items_center()
                    .gap(px(T::S4))
                    .child(self.preset_switch(cx).flex_1())
                    .child(
                        button("sidebar-collapse", "‹")
                            .border_0()
                            .aria_label("收起左栏")
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
                    ),
            )
            .child(sidebar)
            .when_some(self.quota_view(), |d, quota| d.child(quota))
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
                    // Notifications, sound and the overview's time limits live in settings;
                    // this opens them on the 通知 page.
                    .child(
                        button("open-reminders", "提醒")
                            .border_0()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.show_settings_page(
                                    super::settings::Page::Notifications,
                                    window,
                                    cx,
                                )
                            })),
                    )
                    .child(button("open-settings", "设置").border_0().on_click(
                        cx.listener(|this, _, window, cx| this.show_settings(window, cx)),
                    )),
            )
    }
    /// The accounts' usage limits above the footer, one row per CLI used this week, from
    /// what its own logs last recorded. Hovering a row gives the plan, the reading's age and
    /// when each window resets.
    fn quota_view(&self) -> Option<gpui::Div> {
        let now = now();
        let readings = keepline_core::quota::latest(&self.workspace, now);
        if readings.is_empty() {
            return None;
        }
        let mut block = div()
            .flex_shrink_0()
            .border_t(px(T::BORDER_WIDTH))
            .border_color(T::color(T::border()))
            .px(px(T::S10))
            .py(px(T::S6))
            .flex()
            .flex_col()
            .gap(px(T::S6))
            .text_size(px(T::CHIP_SIZE))
            .font_features(T::tabular());
        for (agent, quota) in readings {
            let mut lines = vec![format!("{} 额度", agent.name())];
            lines.push(match quota.plan.as_str() {
                "" => format!("读数 {}", ago(quota.at, now)),
                plan => format!("{plan} · 读数 {}", ago(quota.at, now)),
            });
            let mut row = div()
                .id(gpui::SharedString::from(format!("quota-{}", agent.key())))
                .flex()
                .items_center()
                .gap(px(T::S8))
                .child(
                    div()
                        .w(px(T::QUOTA_NAME))
                        .flex_shrink_0()
                        .truncate()
                        .text_color(T::color(T::secondary()))
                        .child(agent.key()),
                );
            for window in quota.windows.iter().take(2) {
                lines.push(if window.reset(now) {
                    format!("{} 已重置，等这家下次使用时更新", window.label())
                } else if window.resets_at == 0 {
                    format!("{} 已用 {}%", window.label(), window.used)
                } else {
                    format!(
                        "{} 已用 {}%，{} 重置",
                        window.label(),
                        window.used,
                        upcoming(window.resets_at, now)
                    )
                });
                row = row.child(quota_meter(window, now));
            }
            if quota.windows.len() == 1 {
                row = row.child(div().flex_1());
            }
            lines.push("来自它自己的会话记录，Keepline 不联网查询".into());
            block = block.child(row.tooltip(super::diagnostics::tip(lines)));
        }
        Some(block)
    }
    /// Whether a space is the one shown; over the overview the spaces are its filter.
    fn space_active(&self, id: &str) -> bool {
        if self.nav.overview {
            self.overview.filter_space.as_deref() == Some(id)
        } else {
            self.nav.space == id
        }
    }
    /// A space's open sessions, how many wait on you or failed, and the state its dot shows.
    fn space_counts(&self, id: &str) -> SpaceCounts {
        let sessions: Vec<_> = self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| {
                self.session_hidden(s).is_none()
                    && self.session_open(s)
                    && session_space(&self.workspace, s) == Some(id)
            })
            .collect();
        let needs = |g: Group| {
            sessions
                .iter()
                .filter(|s| {
                    s.managed()
                        && s.outcome.needs_review(now())
                        && session_group(&self.workspace, s) == g
                })
                .count()
        };
        let (waiting, errors) = (needs(Group::Waiting), needs(Group::Error));
        let group = if errors > 0 {
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
        };
        SpaceCounts {
            count: sessions.len(),
            waiting,
            errors,
            group,
        }
    }
    /// Shows a space: its first tab, or over the overview, its sessions only.
    fn pick_space(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.nav.overview {
            self.overview.filter_space = Some(id.to_owned());
        }
        self.nav.space = id.to_owned();
        self.nav.tab = self
            .workspace
            .tabs
            .iter()
            .find(|t| t.space == id)
            .map(|t| t.id.clone());
        self.sync_panes(cx);
        cx.notify();
    }
    /// The dense presets' top line: the space shown, `▾` opening the space menu (switch,
    /// add, rename, folder, remove), a badge when another space needs you, and the
    /// space's todos as a small `待办 3` that unfolds them below.
    fn space_line(&self, cx: &Context<Self>) -> gpui::Div {
        let shown = if self.nav.overview {
            self.overview.filter_space.clone()
        } else {
            Some(self.nav.space.clone())
        };
        let space = shown
            .as_ref()
            .and_then(|id| self.workspace.spaces.iter().find(|s| &s.id == id));
        let line = div()
            .flex()
            .items_center()
            .gap(px(T::S4))
            .pb(px(T::S4))
            .font_family(T::UI_FONT);
        if let Some((_, field, _)) = &self.layout.renaming_space {
            return line.child(
                div()
                    .flex_1()
                    .h(px(T::ROW))
                    .px(px(T::S8 - T::BORDER_WIDTH))
                    .rounded(px(T::CONTROL_RADIUS))
                    .bg(T::color(T::raised()))
                    .border(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::focus()))
                    .text_size(px(T::SMALL))
                    .child(field.clone()),
            );
        }
        let elsewhere = self
            .workspace
            .spaces
            .iter()
            .filter(|s| shown.as_deref() != Some(s.id.as_str()))
            .map(|s| self.space_counts(&s.id))
            .map(|c| c.waiting + c.errors)
            .sum::<usize>();
        let name = space.map_or_else(|| "全部空间".to_owned(), |s| s.name.clone());
        let todos = self
            .workspace
            .tasks
            .iter()
            .filter(|t| t.space == self.nav.space)
            .count();
        let open = self.tasks.todos_open;
        line.child(
            button("space-switch", "")
                .aria_label(format!("空间 {name}，点开切换或新建"))
                .min_w_0()
                .px(px(T::S8))
                .gap(px(T::S6))
                .justify_start()
                .border_0()
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(T::color(T::primary()))
                        .child(name),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(T::CHIP_SIZE))
                        .text_color(T::color(T::muted()))
                        .child("▾"),
                )
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                        this.layout.space_menu = Some((this.nav.space.clone(), event.position));
                        this.layout.space_switch = true;
                        this.layout.space_removing = false;
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ),
        )
        .when(elsewhere > 0, |d| {
            d.child(
                badge(Group::Error, elsewhere)
                    .flex_shrink_0()
                    .font_family(T::mono(cx).family),
            )
        })
        .child(div().flex_1())
        .child(
            button("todos-toggle", format!("待办 {todos}"))
                .aria_label(if open { "收起待办" } else { "展开待办" })
                .flex_shrink_0()
                .h(px(T::ICON_HEIGHT))
                .px(px(T::S6))
                .border_0()
                .text_size(px(T::CHIP_SIZE))
                .text_color(T::color(if open { T::primary() } else { T::secondary() }))
                .when(open, |d| d.bg(T::color(T::raised())))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.tasks.todos_open = !this.tasks.todos_open;
                    cx.notify();
                })),
        )
        .child(
            button("sidebar-new-task", "＋")
                .aria_label("新待办")
                .flex_shrink_0()
                .h(px(T::ICON_HEIGHT))
                .px(px(T::S6))
                .border_0()
                .text_size(px(T::CHIP_SIZE))
                .text_color(T::color(T::link()))
                .on_click(cx.listener(|this, _, window, cx| this.show_quick_task(window, cx))),
        )
    }
    /// The folded column: a way back, and how many sessions in this space need you.
    fn sidebar_rail(&self, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        let needs = self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| {
                self.session_hidden(s).is_none()
                    && self.session_open(s)
                    && session_space(&self.workspace, s) == Some(self.nav.space.as_str())
            })
            .map(|s| sidebar_status(&self.workspace, &self.attention.unseen, s))
            .filter(|(g, attention)| *attention && matches!(g, Group::Waiting | Group::Error))
            .count();
        div()
            .id("sidebar")
            .w(px(T::SIDEBAR_RAIL))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(T::S8))
            .py(px(T::S8))
            .bg(T::color(T::panel()))
            .border_r(px(T::BORDER_WIDTH))
            .border_color(T::color(T::border()))
            .child(
                button("sidebar-expand", "›")
                    .border_0()
                    .aria_label("展开左栏")
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
            )
            .when(needs > 0, |d| d.child(badge(Group::Waiting, needs)))
            .child(div().flex_1())
            .child(
                button("open-settings", "⚙")
                    .border_0()
                    .aria_label("设置")
                    .on_click(cx.listener(|this, _, window, cx| this.show_settings(window, cx))),
            )
    }
    /// The space's todos: what is still to be handed to an Agent, above what is running.
    /// In the dense presets the header lives in the space line (`待办 3`), so only the
    /// quick-add field and, once opened, the list come from here.
    fn todos_view(&self, space_name: &str, dense: bool, cx: &Context<Self>) -> gpui::Div {
        let mut sidebar = div().flex().flex_col();
        let todo_count = self
            .workspace
            .tasks
            .iter()
            .filter(|t| t.space == self.nav.space)
            .count();
        let open = self.tasks.todos_open;
        if dense {
            sidebar = sidebar
                .when(open || self.tasks.quick_task.is_some(), |d| d.pt(px(T::S6)))
                .children(self.quick_task_row(cx));
        } else {
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
        }
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
impl Workbench {
    /// Turns the space's name in the sidebar into a field: Enter or a click elsewhere
    /// keeps it, Esc puts the old name back.
    pub(super) fn rename_space(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(space) = self.workspace.spaces.iter().find(|s| s.id == id).cloned() else {
            return;
        };
        self.start_space_rename(space, window, cx);
    }
    fn start_space_rename(&mut self, space: Space, window: &mut Window, cx: &mut Context<Self>) {
        self.save_space_name(cx);
        let field = cx.new(|cx| Field::new("空间名称", &space.name, cx).inline());
        field.update(cx, |f, _| f.select_all());
        let events = cx.subscribe(&field, |this, _, event, cx| match event {
            crate::ui::field::FieldEvent::Submit => {
                this.save_space_name(cx);
                this.nav.pending_focus = true;
            }
            crate::ui::field::FieldEvent::Cancel => {
                this.layout.renaming_space = None;
                this.nav.pending_focus = true;
                cx.notify();
            }
        });
        let blur = cx.on_blur(&field.focus_handle(cx), window, |this, _, cx| {
            this.save_space_name(cx)
        });
        window.focus(&field.focus_handle(cx), cx);
        self.layout.renaming_space = Some((space, field, [events, blur]));
        cx.notify();
    }
    fn save_space_name(&mut self, cx: &mut Context<Self>) {
        let Some((mut space, field, _)) = self.layout.renaming_space.take() else {
            return;
        };
        let name = field.read(cx).text.trim().to_owned();
        // An empty or unchanged name leaves the space as it was.
        if !name.is_empty() && name != space.name {
            space.name = name;
            self.edit(Edit::UpdateSpace(space), cx);
        }
        cx.notify();
    }
    /// Asks macOS for a folder: for `target` it becomes that space's directory; with no
    /// target it starts a new space named after the folder, its name ready to edit.
    pub(super) fn choose_space_dir(
        &mut self,
        target: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.layout.space_menu = None;
        let picked = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("选择".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(dir) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update_in(cx, |this, window, cx| {
                let cwd = dir.to_string_lossy().into_owned();
                if let Some(target) = target {
                    if let Some(space) = this.workspace.spaces.iter().find(|s| s.id == target) {
                        this.edit(
                            Edit::UpdateSpace(Space {
                                cwd,
                                ..space.clone()
                            }),
                            cx,
                        );
                    }
                    return;
                }
                let space = Space {
                    id: id(),
                    name: dir
                        .file_name()
                        .map_or_else(|| cwd.clone(), |n| n.to_string_lossy().into_owned()),
                    cwd,
                };
                this.edit(Edit::AddSpace(space.clone()), cx);
                this.nav.space = space.id.clone();
                this.nav.tab = None;
                this.sync_panes(cx);
                this.start_space_rename(space, window, cx);
            });
        })
        .detach();
    }
    /// From the space menu: the first press asks, the second removes the space and
    /// closes its tabs; the host refuses while it still has unsettled todos.
    pub(super) fn remove_space(&mut self, space: &str, cx: &mut Context<Self>) {
        if !self.layout.space_removing {
            self.layout.space_removing = true;
            cx.notify();
            return;
        }
        self.layout.space_removing = false;
        self.layout.space_menu = None;
        if self.nav.space == space
            && let Some(other) = self.workspace.spaces.iter().find(|s| s.id != space)
        {
            self.nav.space = other.id.clone();
            self.nav.tab = self
                .workspace
                .tabs
                .iter()
                .find(|t| t.space == other.id)
                .map(|t| t.id.clone());
            self.sync_panes(cx);
        }
        self.edit(Edit::RemoveSpace { id: space.into() }, cx);
        cx.notify();
    }
    /// A space's right-click menu.
    pub(super) fn space_menu_view(
        &self,
        id: &str,
        position: gpui::Point<gpui::Pixels>,
        window: &Window,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let rename = id.to_owned();
        let folder = id.to_owned();
        let remove = id.to_owned();
        let tabs = self.workspace.tabs.iter().filter(|t| t.space == id).count();
        let last = self.workspace.spaces.len() <= 1;
        // From the dense presets' space line: every space to switch to, and a new one.
        let mut picks = vec![];
        if self.layout.space_switch {
            for space in &self.workspace.spaces {
                let pick = space.id.clone();
                let SpaceCounts {
                    count,
                    waiting,
                    errors,
                    group,
                } = self.space_counts(&space.id);
                picks.push(
                    button(
                        gpui::SharedString::from(format!("space-pick-{}", space.id)),
                        "",
                    )
                    .aria_label(space.name.clone())
                    .w_full()
                    .px(px(T::S8))
                    .gap(px(T::S8))
                    .justify_start()
                    .border_0()
                    .when(self.space_active(&space.id), |d| {
                        d.bg(T::color(T::button_hover()))
                    })
                    .child(dot(group))
                    .child(div().min_w_0().truncate().child(space.name.clone()))
                    .when(!Path::new(&space.cwd).is_dir(), |d| {
                        d.child(
                            label("目录不在了")
                                .flex_shrink_0()
                                .text_size(px(T::CHIP_SIZE))
                                .text_color(T::color(T::muted())),
                        )
                    })
                    .child(div().flex_1())
                    .when(waiting > 0, |d| d.child(badge(Group::Waiting, waiting)))
                    .when(errors > 0, |d| d.child(badge(Group::Error, errors)))
                    .child(
                        label(count.to_string())
                            .flex_shrink_0()
                            .text_size(px(T::CHIP_SIZE))
                            .text_color(T::color(T::muted())),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.layout.space_menu = None;
                        this.pick_space(&pick, cx);
                    }))
                    .into_any_element(),
                );
            }
            picks.push(
                button("space-menu-add", "＋ 新空间…")
                    .w_full()
                    .justify_start()
                    .border_0()
                    .text_color(T::color(T::link()))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.choose_space_dir(None, window, cx)),
                    )
                    .into_any_element(),
            );
        }
        let name = self
            .workspace
            .spaces
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.name.clone())
            .unwrap_or_default();
        let mut entries = picks;
        entries.extend([
            button(
                "space-menu-rename",
                if self.layout.space_switch {
                    format!("重命名「{name}」")
                } else {
                    "重命名".to_owned()
                },
            )
            .w_full()
            .justify_start()
            .border_0()
            .on_click(cx.listener(move |this, _, window, cx| {
                this.layout.space_menu = None;
                this.rename_space(&rename, window, cx);
            }))
            .into_any_element(),
            button("space-menu-folder", "更换目录…")
                .w_full()
                .justify_start()
                .border_0()
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.choose_space_dir(Some(folder.clone()), window, cx);
                }))
                .into_any_element(),
            // Two presses: the first says what goes with it, the second removes it.
            button(
                "space-menu-remove",
                if !self.layout.space_removing {
                    "删除空间".to_owned()
                } else if tabs > 0 {
                    format!("确认删除，并关掉 {tabs} 个标签")
                } else {
                    "确认删除".to_owned()
                },
            )
            .w_full()
            .justify_start()
            .border_0()
            .text_color(Group::Error.colors().1)
            .when(last, |d| d.opacity(0.4))
            .on_click(cx.listener(move |this, _, _, cx| {
                if !last {
                    this.remove_space(&remove, cx);
                }
            }))
            .into_any_element(),
        ]);
        self.popup(position, entries, window, cx, |this| {
            this.layout.space_menu = None
        })
    }
}
/// One window: its name and percentage over a thin bar, colored as it fills up. A window
/// that rolled over since the reading says so instead of showing an old number.
fn quota_meter(window: &keepline_core::quota::Window, now: u64) -> gpui::Div {
    use keepline_core::quota::{NEAR, OVER};
    let reset = window.reset(now);
    let fill = match window.used {
        _ if reset => T::color(T::muted()),
        u if u >= OVER => Group::Error.colors().0,
        u if u >= NEAR => Group::Waiting.colors().0,
        _ => Group::Running.colors().0,
    };
    let (number, share) = if reset {
        ("已重置".to_string(), 0.)
    } else {
        (
            format!("{}%", window.used),
            f32::from(window.used.min(100)) / 100.,
        )
    };
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(T::S2))
        .child(
            div()
                .flex()
                .justify_between()
                .child(div().text_color(T::color(T::muted())).child(window.label()))
                .child(
                    div()
                        .text_color(match window.used {
                            _ if reset => T::color(T::muted()),
                            u if u < NEAR => T::color(T::body()),
                            _ => fill,
                        })
                        .child(number),
                ),
        )
        .child(
            div()
                .h(px(T::QUOTA_BAR))
                .rounded_full()
                .bg(T::color(T::chip()))
                .child(
                    div()
                        .h_full()
                        .w(gpui::relative(share))
                        .rounded_full()
                        .bg(fill),
                ),
        )
}
