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
    /// What each queued session became while this round of 逐个清理 ran, by session id.
    pub(super) cleanup_results: HashMap<String, Outcome>,
    pub(super) show_running: bool,
    pub(super) show_review: bool,
    /// The overview's scroll, so a group's pill can bring that group into view.
    pub(super) scroll: gpui::ScrollHandle,
    pub(super) show_hidden: bool,
    pub(super) card_menu: Option<String>,
    /// Opened by coming back after a while, not by the user: titled 「欢迎回来」.
    pub(super) welcome: bool,
    /// Holds keyboard focus while the overview shows, so Esc returns to the panes.
    pub(super) focus: gpui::FocusHandle,
    /// Where the top bar's toggle was drawn, for native click evidence.
    #[cfg(feature = "visual-test")]
    pub(super) toggle_at: Option<gpui::Bounds<gpui::Pixels>>,
}
impl Workbench {
    /// Swaps the panes for the overview in the same window: top bar, sidebar and tabs stay.
    pub(super) fn show_overview(&mut self, cx: &mut Context<Self>) {
        self.overview.closed_only = false;
        self.overview.welcome = false;
        self.overview.cleanup.clear();
        self.nav.overview = true;
        self.nav.task_detail = None;
        self.nav.pending_focus = true;
        self.sync_panes(cx);
        self.request(Request::Workspace, cx);
    }
    /// 逐个清理 over these sessions, in order.
    pub(super) fn start_cleanup(&mut self, queue: Vec<String>, cx: &mut Context<Self>) {
        self.overview.cleanup = queue;
        self.overview.cleanup_index = 0;
        self.overview.cleanup_results.clear();
        self.overview.here = None;
        cx.notify();
    }
    /// Notes where the toggle is drawn so native evidence can press the real button.
    #[cfg(feature = "visual-test")]
    pub(super) fn record_toggle(
        &self,
        button: gpui::Stateful<gpui::Div>,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let entity = cx.entity();
        button.relative().child(
            gpui::canvas(
                |_, _, _| (),
                move |bounds, _, _, cx| {
                    entity.update(cx, |this, _| this.overview.toggle_at = Some(bounds));
                },
            )
            .absolute()
            .size_full(),
        )
    }
    #[cfg(not(feature = "visual-test"))]
    pub(super) fn record_toggle(
        &self,
        button: gpui::Stateful<gpui::Div>,
        _: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        button
    }
    /// Back to the tab that was in view, its focused pane taking the keyboard again.
    pub(super) fn leave_overview(&mut self, cx: &mut Context<Self>) {
        self.overview.closed_only = false;
        self.overview.welcome = false;
        self.overview.cleanup.clear();
        self.overview.card_menu = None;
        self.nav.overview = false;
        self.nav.pending_focus = true;
        self.sync_panes(cx);
        cx.notify();
    }
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
        // Search sits in the top bar and the way back is the same 「鸟瞰全部」 toggle, the
        // tab strip or Esc, so the header keeps only what this page is.
        let header = div()
            .flex()
            .flex_wrap()
            .items_end()
            .gap(px(T::S16))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(T::S4))
                    .child(
                        div()
                            .text_size(px(T::TITLE))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(T::color(T::primary()))
                            .child(if closed_only {
                                "已关闭的会话"
                            } else if self.overview.welcome {
                                "欢迎回来"
                            } else {
                                "鸟瞰全部"
                            }),
                    )
                    .child(label(if closed_only {
                        let quiet = open
                            .iter()
                            .filter(|s| !closed_recently(&self.workspace, s))
                            .count();
                        format!(
                            "{} 个 · 终端已关，对话还在，点卡片在新画面里接着聊{} · Esc 回到工作区",
                            open.len(),
                            if quiet > 0 {
                                format!(" · 其中 {quiet} 个很久没动，左栏不再计数")
                            } else {
                                String::new()
                            }
                        )
                    } else {
                        format!(
                            "{} 个会话还没收尾 · 按要你做什么分好了组 · Esc 回到工作区",
                            open.len()
                        )
                    })),
            )
            .child(div().flex_1())
            .child(self.navigation_filters(cx));
        let mut before: Vec<gpui::AnyElement> = vec![header.into_any_element()];
        // Closed ones pile up: walk them one at a time instead of card menus.
        if closed_only && !open.is_empty() {
            let queue: Vec<_> = open.iter().map(|s| s.id.clone()).collect();
            before.push(
                div()
                    .flex()
                    .child(solid(button("cleanup-closed", "逐个清理 →")).on_click(
                        cx.listener(move |this, _, _, cx| this.start_cleanup(queue.clone(), cx)),
                    ))
                    .into_any_element(),
            );
        }
        if let Some(error) = self.error.clone() {
            before.push(
                div()
                    .text_color(Group::Error.colors().1)
                    .child(error)
                    .into_any_element(),
            );
        }
        // The sidebar stays beside the overview.
        let available = f32::from(window.viewport_size().width) - T::SIDEBAR - T::OVERVIEW_X * 2.;
        let card_width = |min: f32| {
            let columns = ((available + T::S10) / (min + T::S10)).floor().max(1.);
            ((available - T::S10 * (columns - 1.)) / columns).floor()
        };
        let mut middle: Vec<gpui::AnyElement> = vec![];
        if !closed_only {
            middle.push(
                self.todo_section(card_width(T::TODO_CARD_MIN), cx)
                    .into_any_element(),
            );
        }
        let at_now = now();
        let grouped: Vec<(Group, Vec<&AgentSession>)> = Group::ALL
            .into_iter()
            .map(|g| {
                (
                    g,
                    open.iter()
                        .copied()
                        .filter(|s| session_group(&self.workspace, s) == g)
                        .collect(),
                )
            })
            .collect();
        // Empty groups get no section: their pill above already reads 0.
        let shown: Vec<_> = grouped.iter().filter(|(_, s)| !s.is_empty()).collect();
        let first_group = before.len() + 1 + middle.len();
        let place: Vec<(Group, usize)> = shown
            .iter()
            .enumerate()
            .map(|(i, (g, _))| (*g, first_group + i))
            .collect();
        let width = card_width(T::CARD_MIN);
        let sections: Vec<gpui::AnyElement> = shown
            .iter()
            .map(|(group, sessions)| {
                self.group_section(*group, sessions, width, at_now, cx)
                    .into_any_element()
            })
            .collect();
        let dist = self.distribution(&grouped, &place, cx);
        let mut view = div()
            .id("overview-scroll")
            .track_scroll(&self.overview.scroll)
            .size_full()
            .overflow_y_scroll()
            .font_features(super::diagnostics::tabular())
            .flex()
            .flex_col()
            .gap(px(T::S20))
            .px(px(T::OVERVIEW_X))
            .py(px(T::S20))
            .children(before)
            .child(dist)
            .children(middle)
            .children(sections);
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
                .border_color(T::color(T::border()))
                .pt(px(T::S16))
                .child(
                    button(
                        "closed-toggle",
                        format!(
                            "稍后再看 {deferred} · 已完成 {completed} · 已放弃 {abandoned} · {}",
                            if self.overview.show_closed {
                                "收起"
                            } else {
                                "展开"
                            }
                        ),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.overview.show_closed = !this.overview.show_closed;
                        cx.notify();
                    })),
                ),
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
            .text_color(T::color(T::secondary()))
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
                        .border_color(T::color(T::border()))
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

/// Last-activity ages bucket by week in 很久没动's mini chart.
const WEEK: u64 = 7 * 86_400;

/// The one solid control of a view.
pub(super) fn solid(b: gpui::Stateful<gpui::Div>) -> gpui::Stateful<gpui::Div> {
    b.bg(T::color(T::accent()))
        .text_color(T::color(T::on_accent()))
        .font_weight(gpui::FontWeight::SEMIBOLD)
}
/// An ordinary button: a quiet fill, no outline.
pub(super) fn quiet(b: gpui::Stateful<gpui::Div>) -> gpui::Stateful<gpui::Div> {
    b.bg(T::color(T::button()))
        .text_color(T::color(T::primary()))
}
/// A section's title line: status dot, name, count, a note and actions at the right.
fn section_head(
    group: Option<Group>,
    title: &str,
    count: usize,
    note: Option<&str>,
    right: Vec<gpui::AnyElement>,
) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(T::S8))
        .when_some(group, |d, g| d.child(dot(g)))
        .child(
            div()
                .text_size(px(T::CARD_TITLE))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(group.map_or(T::color(T::primary()), |g| g.colors().1))
                .child(title.to_owned()),
        )
        .child(
            div()
                .text_color(T::color(T::muted()))
                .child(count.to_string()),
        )
        .when_some(note, |d, note| {
            d.child(label(note.to_owned()).text_color(T::color(T::muted())))
        })
        .child(div().flex_1())
        .child(div().flex().gap(px(T::S6)).children(right))
}
/// The row a folded group shows instead of its cards.
fn folded_row(content: Vec<gpui::AnyElement>, right: Vec<gpui::AnyElement>) -> gpui::Div {
    div()
        .h(px(T::FOLDED_ROW))
        .px(px(T::S14))
        .flex()
        .items_center()
        .gap(px(T::S10))
        .rounded(px(T::CARD_RADIUS))
        .border(px(T::BORDER_WIDTH))
        .border_color(T::color(T::border()))
        .children(content)
        .child(div().flex_1())
        .child(div().flex().gap(px(T::S6)).children(right))
}
fn weeks_label(weeks: u64) -> String {
    if weeks == 0 {
        "不到 1 周".into()
    } else {
        format!("{weeks} 周前")
    }
}

impl Workbench {
    /// A group was asked for from the distribution: unfold it and bring it to the top.
    fn reveal_group(&mut self, group: Group, at: usize, cx: &mut Context<Self>) {
        match group {
            Group::Running => self.overview.show_running = true,
            Group::Stale => self.overview.show_stale = true,
            Group::Review => self.overview.show_review = true,
            _ => (),
        }
        self.overview.scroll.scroll_to_top_of_item(at);
        cx.notify();
    }
    fn group_folded(&self, group: Group) -> bool {
        !self.overview.closed_only
            && match group {
                Group::Running => !self.overview.show_running,
                Group::Stale => !self.overview.show_stale,
                Group::Review => !self.overview.show_review,
                _ => false,
            }
    }
    /// One segment per group with sessions, weighted by count, then a pill per group.
    pub(super) fn distribution(
        &self,
        grouped: &[(Group, Vec<&AgentSession>)],
        place: &[(Group, usize)],
        cx: &Context<Self>,
    ) -> gpui::Div {
        let at = |g: Group| place.iter().find(|(p, _)| *p == g).map(|(_, i)| *i);
        let total: usize = grouped.iter().map(|(_, s)| s.len()).sum();
        let filled: Vec<_> = grouped.iter().filter(|(_, s)| !s.is_empty()).collect();
        let mut bar = div()
            .id("overview-distribution")
            .flex()
            .gap(px(T::S2))
            .h(px(T::DIST_BAR))
            .aria_label(gpui::SharedString::from(format!(
                "{total} 个会话按状态分布"
            )));
        let last = filled.len().saturating_sub(1);
        for (i, (group, sessions)) in filled.iter().enumerate() {
            let (group, n) = (*group, sessions.len());
            let hue = group.colors().0;
            let mut segment = div()
                .id(gpui::SharedString::from(format!("dist-{i}")))
                .h_full()
                .min_w(px(T::S4))
                .flex_grow(n as f32)
                .flex_basis(gpui::relative(0.))
                .cursor_pointer()
                .tooltip(super::diagnostics::tip(vec![
                    group.label().to_owned(),
                    format!("{n} 个 · {:.0}%", n as f32 / total.max(1) as f32 * 100.),
                ]));
            segment = if group == Group::Stale {
                segment
                    .bg(gpui::pattern_slash(hue, T::S2, T::S6))
                    .border(px(T::BORDER_WIDTH))
                    .border_color(hue)
            } else {
                segment.bg(hue)
            };
            if i == 0 {
                segment = segment.rounded_l(px(T::CHIP_RADIUS));
            }
            if i == last {
                segment = segment.rounded_r(px(T::CHIP_RADIUS));
            }
            if let Some(at) = at(group) {
                segment = segment
                    .on_click(cx.listener(move |this, _, _, cx| this.reveal_group(group, at, cx)));
            }
            bar = bar.child(segment);
        }
        let mut legend = div().flex().flex_wrap().gap(px(T::S6));
        for (i, (group, sessions)) in grouped.iter().enumerate() {
            let (group, n) = (*group, sessions.len());
            let empty = n == 0;
            let mut pill = div()
                .id(gpui::SharedString::from(format!("pill-{i}")))
                .role(gpui::accesskit::Role::Button)
                .aria_label(gpui::SharedString::from(format!("{} {n}", group.label())))
                .h(px(T::PILL_HEIGHT))
                .px(px(T::S10))
                .flex()
                .items_center()
                .gap(px(T::S6))
                .rounded(px(T::PILL_RADIUS))
                .border(px(T::BORDER_WIDTH))
                .border_color(T::color(T::border()))
                .text_size(px(T::SMALL))
                .text_color(T::color(if empty { T::muted() } else { T::body() }))
                .child(dot(group))
                .child(group.label())
                .child(if empty {
                    div().child(n.to_string())
                } else {
                    div()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(T::color(T::primary()))
                        .child(n.to_string())
                });
            if empty {
                pill = pill.border_dashed();
            } else if let Some(at) = at(group) {
                pill = pill
                    .cursor_pointer()
                    .hover(|s| s.bg(T::color(T::raised())))
                    .on_click(cx.listener(move |this, _, _, cx| this.reveal_group(group, at, cx)));
            }
            legend = legend.child(pill);
        }
        div()
            .flex()
            .flex_col()
            .gap(px(T::S10))
            .when(total > 0, |d| d.child(bar))
            .child(legend)
    }
    /// Todos not handed to an Agent yet: a light row each, 「开始」 at its right.
    fn todo_section(&self, width: f32, cx: &Context<Self>) -> gpui::Div {
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
        let mut cards = div().flex().flex_wrap().gap(px(T::S10));
        for task in &unstarted {
            let id = task.id.clone();
            cards = cards.child(
                div()
                    .w(px(width))
                    .min_w(px(T::TODO_CARD_MIN))
                    .px(px(T::S14))
                    .py(px(T::S12))
                    .rounded(px(T::CARD_RADIUS))
                    .bg(T::color(T::popover()))
                    .border(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::card_border()))
                    .flex()
                    .flex_col()
                    .gap(px(T::S8))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(T::S12))
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
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(T::color(T::primary()))
                                            .child(task.title.clone()),
                                    )
                                    .child(
                                        label(format!(
                                            "{} · {} 条验收标准",
                                            task.agent.executable(),
                                            task.acceptance.len()
                                        ))
                                        .text_color(T::color(T::muted())),
                                    ),
                            )
                            .child(
                                quiet(button(
                                    gpui::SharedString::from(format!("start-{id}")),
                                    "开始 →",
                                ))
                                .flex_shrink_0()
                                .on_click(cx.listener(
                                    move |this, _, _, cx| this.ask_agent(id.clone(), false, cx),
                                )),
                            ),
                    )
                    .children(self.agent_picker(&task.id, cx)),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(px(T::S10))
            .child(section_head(
                None,
                "待办 · 还没开始",
                unstarted.len(),
                Some("命令只预填，按回车才启动"),
                vec![
                    quiet(button("overview-new-task", "＋ 新待办"))
                        .on_click(
                            cx.listener(|this, _, window, cx| this.show_quick_task(window, cx)),
                        )
                        .into_any_element(),
                ],
            ))
            .when(!unstarted.is_empty(), |d| d.child(cards))
    }
    /// A group's head, its folded summary when it is folded, and its cards otherwise.
    fn group_section(
        &self,
        group: Group,
        sessions: &[&AgentSession],
        width: f32,
        now: u64,
        cx: &Context<Self>,
    ) -> gpui::Div {
        let folded = self.group_folded(group);
        let foldable = !self.overview.closed_only
            && matches!(group, Group::Running | Group::Stale | Group::Review);
        let note = match group {
            Group::Error => Some("先看这些：Agent 停在了报错上"),
            Group::Review => Some("等你核对后标记完成"),
            Group::Stale if !self.overview.closed_only => Some("左栏不再计数"),
            _ => None,
        };
        let mut section = div().flex().flex_col().gap(px(T::S10)).child(section_head(
            Some(group),
            group.label(),
            sessions.len(),
            note,
            vec![],
        ));
        if foldable {
            let toggle_id = match group {
                Group::Running => "toggle-running",
                Group::Stale => "toggle-stale",
                _ => "toggle-review",
            };
            let open_label = if group == Group::Stale {
                "展开核对"
            } else {
                "展开"
            };
            let toggle = quiet(button(toggle_id, if folded { open_label } else { "收起" }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    let flag = match group {
                        Group::Running => &mut this.overview.show_running,
                        Group::Stale => &mut this.overview.show_stale,
                        _ => &mut this.overview.show_review,
                    };
                    *flag = !*flag;
                    cx.notify();
                }));
            let mut right = vec![toggle.into_any_element()];
            let content: Vec<gpui::AnyElement> = match group {
                Group::Review => {
                    let mut counts = [("最近 24 小时", 0), ("最近 7 天", 0), ("更早", 0)];
                    for s in sessions {
                        let bucket = recency(self.session_updated(s), now);
                        if let Some(c) = counts.iter_mut().find(|(name, _)| *name == bucket) {
                            c.1 += 1;
                        }
                    }
                    vec![
                        label(
                            counts
                                .iter()
                                .map(|(name, n)| format!("{name} {n}"))
                                .collect::<Vec<_>>()
                                .join(" · "),
                        )
                        .text_size(px(T::SMALL))
                        .text_color(T::color(T::muted()))
                        .into_any_element(),
                    ]
                }
                Group::Running => vec![
                    label("不用管，跑完会挪到「做完了」")
                        .text_size(px(T::SMALL))
                        .text_color(T::color(T::muted()))
                        .into_any_element(),
                ],
                _ => {
                    right.push(
                        solid(button("cleanup", "逐个清理 →"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let queue = this
                                    .workspace
                                    .agent_sessions
                                    .iter()
                                    .filter(|s| {
                                        this.session_hidden(s).is_none()
                                            && session_group(&this.workspace, s) == Group::Stale
                                    })
                                    .map(|s| s.id.clone())
                                    .collect();
                                this.start_cleanup(queue, cx);
                            }))
                            .into_any_element(),
                    );
                    self.age_chart(sessions, now)
                }
            };
            section = section.child(folded_row(content, right));
        }
        if folded {
            return section;
        }
        let mut cards = div().flex().flex_wrap().gap(px(T::S10));
        for session in sessions {
            cards = cards.child(self.session_card(group, session, width, now, cx));
        }
        section.child(cards)
    }
    /// Weekly buckets of how long each stale session has been quiet, oldest at the right.
    fn age_chart(&self, sessions: &[&AgentSession], now: u64) -> Vec<gpui::AnyElement> {
        let weeks: Vec<u64> = sessions
            .iter()
            .map(|s| now.saturating_sub(self.session_updated(s)) / WEEK)
            .collect();
        let (Some(&lo), Some(&hi)) = (weeks.iter().min(), weeks.iter().max()) else {
            return vec![];
        };
        let span = ((hi - lo + 1) as usize).min(T::AGE_CHART_WEEKS);
        let mut counts = vec![0usize; span];
        for w in &weeks {
            counts[((w - lo) as usize).min(span - 1)] += 1;
        }
        let peak = counts.iter().copied().max().unwrap_or(1).max(1);
        let capped = (hi - lo + 1) as usize > span;
        let mut chart = div()
            .id("stale-ages")
            .flex()
            .items_end()
            .gap(px(T::S2))
            .h(px(T::AGE_CHART_HEIGHT))
            .aria_label("按最后活动时间分布：每格一周");
        let hue = Group::Stale.colors().0;
        for (i, n) in counts.iter().enumerate() {
            let week = lo + i as u64;
            let when = if capped && i == span - 1 {
                format!("{week} 周及更早")
            } else {
                weeks_label(week)
            };
            chart = chart.child(
                div()
                    .id(gpui::SharedString::from(format!("stale-age-{i}")))
                    .w(px(T::AGE_BAR_WIDTH))
                    .h(px(
                        (T::AGE_CHART_HEIGHT * *n as f32 / peak as f32).max(T::S2)
                    ))
                    .rounded(px(T::BORDER_WIDTH))
                    .bg(hue)
                    .opacity(if *n == 0 { 0.25 } else { 0.7 })
                    .tooltip(super::diagnostics::tip(vec![when, format!("{n} 个")])),
            );
        }
        let right = if capped {
            format!("{} 周以上", lo + span as u64 - 1)
        } else {
            weeks_label(hi)
        };
        vec![
            chart.into_any_element(),
            label(format!("按最后活动：{} ← → {right}", weeks_label(lo)))
                .text_size(px(T::SMALL))
                .text_color(T::color(T::muted()))
                .into_any_element(),
        ]
    }
    fn session_card(
        &self,
        group: Group,
        session: &AgentSession,
        width: f32,
        now: u64,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let closed_only = self.overview.closed_only;
        let session = session.clone();
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
        // The last thing said, in body text; a muted lead says what it is.
        let line = if self.workspace.content.contains_key(&session.id) {
            const LEAD: &str = "最后一句  ";
            gpui::StyledText::new(format!("{LEAD}{}", self.session_line(&session))).with_highlights(
                [(
                    0..LEAD.len(),
                    gpui::HighlightStyle {
                        color: Some(T::color(T::muted())),
                        ..Default::default()
                    },
                )],
            )
        } else {
            gpui::StyledText::new(self.session_line(&session))
        };
        let primary_continue =
            closed_only || matches!(group, Group::Waiting | Group::Error | Group::Paused);
        div()
            .id(gpui::SharedString::from(format!("card-{}", session.id)))
            .role(gpui::accesskit::Role::Button)
            .aria_label(gpui::SharedString::from(format!(
                "会话：{}",
                self.session_title(&session)
            )))
            .relative()
            .w(px(width))
            .min_w(px(T::CARD_MIN))
            .min_h(px(T::CARD_MIN_HEIGHT))
            .px(px(T::S14))
            .pt(px(T::S12))
            .pb(px(T::S10))
            .rounded(px(T::CARD_RADIUS))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::card_border()))
            .bg(T::color(T::popover()))
            .flex()
            .flex_col()
            .gap(px(T::S6))
            .cursor_pointer()
            .hover(|s| s.border_color(T::color(T::control())))
            .on_click(cx.listener(move |this, _, window, cx| this.enter_card(&enter, window, cx)))
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
                            .text_color(T::color(T::primary()))
                            .truncate()
                            .child(self.session_title(&session)),
                    )
                    .child(
                        button(
                            gpui::SharedString::from(format!("more-{}", session.id)),
                            "⋯",
                        )
                        .h(px(T::ICON_HEIGHT))
                        .px(px(T::S6))
                        .text_color(T::color(T::muted()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.overview.here = None;
                            this.overview.card_menu =
                                if this.overview.card_menu.as_ref() == Some(&menu_id) {
                                    None
                                } else {
                                    Some(menu_id.clone())
                                };
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(T::S6))
                    .text_size(px(T::META))
                    .text_color(T::color(T::muted()))
                    .child(div().flex_1().min_w_0().truncate().child(meta))
                    .child(
                        div()
                            .flex_shrink_0()
                            .child(ago(self.session_updated(&session), now)),
                    ),
            )
            .child(
                div()
                    .text_size(px(T::SMALL))
                    .text_color(T::color(T::body()))
                    .line_clamp(3)
                    .child(line),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(T::S8))
                    .child(chip(group).font_weight(gpui::FontWeight::SEMIBOLD))
                    .child(div().flex_1())
                    .child(
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
                        .h(px(T::SEGMENT_HEIGHT))
                        .px(px(T::S8))
                        .text_color(T::color(T::link()))
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
            })
    }
}
