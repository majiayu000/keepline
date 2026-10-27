//! 鸟瞰: every open session as grouped cards.
use super::*;

/// The overview's filters and folds, the 「你在这里」 card and 逐个清理's queue.
pub(super) struct OverviewState {
    pub(super) since: u64,
    pub(super) filter_space: Option<String>,
    pub(super) changed_only: bool,
    /// Only this space's sessions whose terminal was closed, from the sidebar's 「已关闭」.
    pub(super) closed_only: bool,
    pub(super) here: Option<String>,
    /// The session whose 「你在这里」 card shows its secondary actions.
    pub(super) here_more: Option<String>,
    pub(super) show_stale: bool,
    pub(super) show_closed: bool,
    pub(super) cleanup: Vec<String>,
    pub(super) cleanup_index: usize,
    pub(super) show_running: bool,
    pub(super) show_hidden: bool,
    pub(super) card_menu: Option<String>,
}
impl Workbench {
    pub(super) fn overview_view(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let open: Vec<_> = self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| {
                s.outcome.needs_review(now())
                    && self.session_hidden(s).is_none()
                    && self.matches_filter(s)
            })
            .collect();
        let closed_only = self.overview.closed_only;
        let header = div()
            .flex()
            .items_center()
            .gap(px(T::S16))
            .child(
                div()
                    .flex_1()
                    .child(
                        div()
                            .text_size(px(T::TITLE))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(if closed_only {
                                "已关闭的会话"
                            } else {
                                "欢迎回来"
                            }),
                    )
                    .child(label(if closed_only {
                        format!(
                            "{} 个 · 终端已关，对话还在，点卡片在新画面里接着聊",
                            open.len()
                        )
                    } else {
                        format!("{} 个未闭环会话 · 按日志回合与机械证据分组", open.len())
                    })),
            )
            .child(self.search_button(cx))
            .child(
                button("back", "← 回到工作区")
                    .text_color(T::color(T::LINK))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.overview.closed_only = false;
                        this.nav.overview = false;
                        this.sync_panes(cx);
                        cx.notify();
                    })),
            );
        let mut view = div()
            .id("overview-scroll")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(T::S20))
            .px(px(T::OVERVIEW_X))
            .py(px(T::S20))
            .child(header)
            .child(self.navigation_filters(cx))
            .when_some(self.error.clone(), |d, error| {
                d.child(div().text_color(Group::Error.colors().1).child(error))
            });
        let available = f32::from(window.viewport_size().width) - T::OVERVIEW_X * 2.;
        let columns = ((available + T::S10) / (T::CARD_MIN + T::S10))
            .floor()
            .max(1.);
        let card_width = ((available - T::S10 * (columns - 1.)) / columns).floor();
        let unstarted: Vec<_> = self
            .workspace
            .tasks
            .iter()
            .filter(|t| {
                self.workspace
                    .task_outcomes
                    .get(&t.id)
                    .unwrap_or(&Outcome::Open)
                    .needs_review(now())
                    && self
                        .overview
                        .filter_space
                        .as_ref()
                        .is_none_or(|id| id == &t.space)
                    && !self.overview.changed_only
                    && !self
                        .workspace
                        .agent_sessions
                        .iter()
                        .any(|s| s.task.as_ref() == Some(&t.id))
            })
            .collect();
        let mut todo_cards = div().flex().flex_wrap().gap(px(T::S10));
        for task in &unstarted {
            let id = task.id.clone();
            todo_cards = todo_cards.child(
                div()
                    .w(px(card_width - T::S8))
                    .min_w(px(T::CARD_MIN))
                    .p(px(T::S12))
                    .rounded(px(T::CARD_RADIUS))
                    .bg(T::color(T::TODO))
                    .border(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::CARD_BORDER))
                    .flex()
                    .flex_col()
                    .gap(px(T::S8))
                    .child(
                        div()
                            .truncate()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(task.title.clone()),
                    )
                    .child(label(format!(
                        "{} · {} 条验收标准",
                        task.agent.executable(),
                        task.acceptance.len()
                    )))
                    .child(
                        button(gpui::SharedString::from(format!("start-{id}")), "开始 →")
                            .bg(T::color(T::ACCENT))
                            .text_color(T::color(T::ON_ACCENT))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.ask_agent(id.clone(), false, cx)
                            })),
                    )
                    .children(self.agent_picker(&task.id, cx)),
            );
        }
        view = view.when(!closed_only, |d| {
            d.child(
                div()
                    .p(px(T::S16))
                    .rounded(px(T::POPOVER_RADIUS))
                    .border(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::TODO_BORDER))
                    .bg(T::color(T::SLOT))
                    .flex()
                    .flex_col()
                    .gap(px(T::S10))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(T::S10))
                            .child(
                                div()
                                    .text_color(T::color(T::LINK))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("☐ 待办 · 还没开始"),
                            )
                            .child(label(unstarted.len().to_string()))
                            .child(label("命令只预填，按回车才启动"))
                            .child(div().flex_1())
                            .child(button("overview-new-task", "＋ 新待办").on_click(
                                cx.listener(|this, _, window, cx| this.show_quick_task(window, cx)),
                            )),
                    )
                    .child(todo_cards),
            )
        });
        for group in Group::ALL {
            // Closed sessions are few and you came for them: nothing stays folded.
            if group == Group::Running && !closed_only {
                let count = open
                    .iter()
                    .filter(|s| session_group(&self.workspace, s) == Group::Running)
                    .count();
                view = view.child(
                    button(
                        "toggle-running",
                        format!(
                            "进行中 {count} · {}",
                            if self.overview.show_running {
                                "收起"
                            } else {
                                "展开"
                            }
                        ),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.overview.show_running = !this.overview.show_running;
                        cx.notify();
                    })),
                );
                if !self.overview.show_running {
                    continue;
                }
            }
            if group == Group::Stale && !closed_only {
                let count = open
                    .iter()
                    .filter(|s| session_group(&self.workspace, s) == Group::Stale)
                    .count();
                view = view.child(
                    button(
                        "toggle-stale",
                        format!(
                            "很久没动 {count} · {}",
                            if self.overview.show_stale {
                                "收起"
                            } else {
                                "展开核对"
                            }
                        ),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.overview.show_stale = !this.overview.show_stale;
                        cx.notify();
                    })),
                );
                if count > 0 {
                    view = view.child(button("cleanup", "逐个清理 →").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.overview.cleanup = this
                                .workspace
                                .agent_sessions
                                .iter()
                                .filter(|s| {
                                    this.session_hidden(s).is_none()
                                        && session_group(&this.workspace, s) == Group::Stale
                                })
                                .map(|s| s.id.clone())
                                .collect();
                            this.overview.cleanup_index = 0;
                            this.overview.here = None;
                            cx.notify();
                        },
                    )));
                }
                if !self.overview.show_stale {
                    continue;
                }
            }
            let sessions: Vec<_> = open
                .iter()
                .filter(|s| session_group(&self.workspace, s) == group)
                .collect();
            if closed_only && sessions.is_empty() {
                continue;
            }
            let mut cards = div().flex().flex_wrap().gap(px(T::S10));
            for session in &sessions {
                let session = (**session).clone();
                let enter = session.clone();
                let primary = session.clone();
                let menu_id = session.id.clone();
                let meta = format!(
                    "{} · {} · {}{}{}",
                    if session.terminal.is_none() {
                        "外部·只读"
                    } else {
                        "托管"
                    },
                    session.runtime.executable(),
                    session_space(&self.workspace, &session)
                        .and_then(|id| self.workspace.spaces.iter().find(|s| s.id == id))
                        .map_or("外部", |s| s.name.as_str()),
                    if self
                        .workspace
                        .outcome_history
                        .iter()
                        .rev()
                        .find(|h| h.session == session.id)
                        .is_some_and(|h| h.reason == "暂存到期")
                    {
                        " · 到期"
                    } else {
                        ""
                    },
                    if changed_since(&self.workspace, &session, self.overview.since) {
                        " · 新"
                    } else {
                        ""
                    }
                );
                let primary_continue =
                    closed_only || matches!(group, Group::Waiting | Group::Error | Group::Paused);
                cards = cards.child(
                    div()
                        .id(gpui::SharedString::from(format!("card-{}", session.id)))
                        .role(gpui::accesskit::Role::Button)
                        .aria_label(gpui::SharedString::from(format!(
                            "会话：{}",
                            self.session_title(&session)
                        )))
                        .relative()
                        .w(px(card_width))
                        .min_w(px(T::CARD_MIN))
                        .p(px(T::S12))
                        .rounded(px(T::CARD_RADIUS))
                        .border(px(T::BORDER_WIDTH))
                        .border_color(T::color(T::CARD_BORDER))
                        .bg(T::color(T::HEADER))
                        .flex()
                        .flex_col()
                        .gap(px(T::S6))
                        .cursor_pointer()
                        .hover(|s| s.bg(T::color(T::RAISED)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.enter_card(&enter, window, cx)
                        }))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(T::S6))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .text_size(px(T::CARD_TITLE))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .truncate()
                                        .child(self.session_title(&session)),
                                )
                                .child(
                                    button(
                                        gpui::SharedString::from(format!("more-{}", session.id)),
                                        "⋯",
                                    )
                                    .border_0()
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            cx.stop_propagation();
                                            this.overview.here = None;
                                            this.overview.card_menu =
                                                if this.overview.card_menu.as_ref()
                                                    == Some(&menu_id)
                                                {
                                                    None
                                                } else {
                                                    Some(menu_id.clone())
                                                };
                                            cx.notify();
                                        },
                                    )),
                                ),
                        )
                        .child(label(meta).truncate())
                        .child(
                            div()
                                .text_size(px(T::MONO_SIZE))
                                .line_clamp(3)
                                .text_color(group.colors().1)
                                .child(self.session_line(&session)),
                        )
                        .child(
                            div().flex().justify_end().child(
                                button(
                                    gpui::SharedString::from(format!("primary-{}", session.id)),
                                    if closed_only {
                                        "接着聊 →"
                                    } else if primary_continue {
                                        "继续 →"
                                    } else {
                                        "看进度"
                                    },
                                )
                                .text_color(T::color(T::LINK))
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        cx.stop_propagation();
                                        this.overview.card_menu = None;
                                        if primary_continue {
                                            this.open_session(&primary, window, cx);
                                        } else {
                                            this.overview.here = Some(primary.id.clone());
                                            cx.notify();
                                        }
                                    },
                                )),
                            ),
                        )
                        .when(self.overview.card_menu.as_ref() == Some(&session.id), |d| {
                            d.child(
                                gpui::deferred(
                                    self.session_actions(&session, cx)
                                        .absolute()
                                        .top(px(T::ROW + T::S12))
                                        .right(px(T::S12))
                                        .occlude(),
                                )
                                .priority(1),
                            )
                        })
                        .when(self.overview.here.as_ref() == Some(&session.id), |d| {
                            d.child(self.here_card(&session, false, cx))
                        }),
                );
            }
            let section = div()
                .flex()
                .flex_col()
                .gap(px(T::S10))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(T::S8))
                        .child(dot(group))
                        .child(
                            div()
                                .text_color(group.colors().1)
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(group.label()),
                        )
                        .child(label(sessions.len().to_string()))
                        .when(sessions.is_empty(), |d| d.child(label("暂无会话"))),
                )
                .when(!sessions.is_empty(), |d| d.child(cards));
            view = view.child(section);
        }
        if closed_only {
            return view.when(open.is_empty(), |d| {
                d.child(label(
                    "这里没有关掉的会话。关掉画面后，它的对话会留在这里，随时能接着聊。",
                ))
            });
        }
        let completed = self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| s.outcome == Outcome::Completed && self.session_hidden(s).is_none())
            .count();
        let abandoned = self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| s.outcome == Outcome::Abandoned && self.session_hidden(s).is_none())
            .count();
        let deferred = self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| {
                matches!(s.outcome, Outcome::Deferred { .. }) && self.session_hidden(s).is_none()
            })
            .count();
        view = view.child(
            div()
                .border_t(px(T::BORDER_WIDTH))
                .border_color(T::color(T::BORDER))
                .pt(px(T::S16))
                .child(button("closed-toggle",format!("暂存 {deferred} · 已结束：完成 {completed} / 放弃 {abandoned} · 展开/收起")).on_click(cx.listener(|this, _, _, cx| {this.overview.show_closed = !this.overview.show_closed;cx.notify();}))),
        );
        if self.overview.show_closed {
            for s in self
                .workspace
                .agent_sessions
                .iter()
                .filter(|s| !s.outcome.needs_review(now()) && self.session_hidden(s).is_none())
            {
                let sid = s.id.clone();
                view = view.child(
                    div()
                        .p(px(T::S8))
                        .child(
                            button(
                                gpui::SharedString::from(format!("closed-{sid}")),
                                format!(
                                    "{} · {}",
                                    self.session_title(s),
                                    outcome_label(&s.outcome)
                                ),
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.overview.here = Some(sid.clone());
                                    cx.notify();
                                },
                            )),
                        )
                        .when(self.overview.here.as_ref() == Some(&s.id), |d| {
                            d.child(self.here_card(s, false, cx))
                        }),
                );
            }
        }
        let hidden: Vec<_> = self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| self.session_hidden(s).is_some())
            .collect();
        view = view.child(
            button(
                "toggle-hidden",
                format!(
                    "已隐藏 {} 个自动化会话 · {}",
                    hidden.len(),
                    if self.overview.show_hidden {
                        "收起"
                    } else {
                        "展开查看"
                    }
                ),
            )
            .border_0()
            .justify_start()
            .text_color(T::color(T::SECONDARY))
            .on_click(cx.listener(|this, _, _, cx| {
                this.overview.show_hidden = !this.overview.show_hidden;
                cx.notify();
            })),
        );
        if self.overview.show_hidden {
            for s in hidden {
                let sid = s.id.clone();
                view = view.child(
                    div()
                        .p(px(T::S8))
                        .border_b(px(T::BORDER_WIDTH))
                        .border_color(T::color(T::BORDER))
                        .child(
                            button(
                                gpui::SharedString::from(format!("hidden-{}", s.id)),
                                self.session_title(s),
                            )
                            .border_0()
                            .justify_start()
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.overview.here = Some(sid.clone());
                                    cx.notify();
                                },
                            )),
                        )
                        .child(label(format!(
                            "{} · {} · {} · {}",
                            self.session_hidden(s).unwrap_or(""),
                            s.runtime.executable(),
                            s.cli_session_id,
                            s.cwd
                        )))
                        .when(self.overview.here.as_ref() == Some(&s.id), |d| {
                            d.child(self.here_card(s, false, cx))
                        }),
                );
            }
        }
        view
    }
}
