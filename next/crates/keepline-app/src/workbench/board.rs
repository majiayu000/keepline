//! 鸟瞰 as a board (specs/overview-kanban/kanban.html): one column per thing you might do
//! next, left to right in the order work moves. Each card carries one colored mark, its
//! space's gradient; everything else stays neutral so the status colors read first.
use super::*;
use crate::ui::icons::agent_icon;
use gpui::{
    Animation, AnimationExt, Hsla, Transformation, linear_color_stop, linear_gradient, relative,
    svg,
};
use keepline_core::model::Agent;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum Column {
    Todo,
    Running,
    Needs,
    Review,
    Done,
    Stale,
}

/// Each column keeps GPUI's measured item heights and its own scroll anchor.
pub(super) struct ColumnList {
    pub(super) state: gpui::ListState,
    ids: Vec<String>,
    revision: u64,
}
impl ColumnList {
    fn sync(&mut self, ids: &[String], revision: u64) {
        let prefix = self.ids.iter().zip(ids).take_while(|(a, b)| a == b).count();
        let suffix = self.ids[prefix..]
            .iter()
            .rev()
            .zip(ids[prefix..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        if prefix + suffix != self.ids.len() || self.ids.len() != ids.len() {
            self.state
                .splice(prefix..self.ids.len() - suffix, ids.len() - prefix - suffix);
            self.ids = ids.to_vec();
        }
        if self.revision != revision {
            self.state.remeasure_items(0..self.state.item_count());
            self.revision = revision;
        }
    }
}
impl Column {
    fn label(self) -> &'static str {
        match self {
            Self::Todo => "待办",
            Self::Running => Group::Running.label(),
            Self::Needs => "要你处理",
            Self::Review => Group::Review.label(),
            Self::Done => "已收尾",
            Self::Stale => Group::Stale.label(),
        }
    }
    fn id(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::Running => "running",
            Self::Needs => "needs",
            Self::Review => "review",
            Self::Done => "done",
            Self::Stale => "stale",
        }
    }
}

/// Which column an open or just-finished session belongs in; `None` keeps it off the board.
pub(super) fn column_of(w: &Workspace, s: &AgentSession, now: u64) -> Option<Column> {
    if !s.outcome.needs_review(now) {
        return (s.outcome == Outcome::Completed && completed_within(w, &s.id, now, DAY))
            .then_some(Column::Done);
    }
    if live_phase(w, s) == Some(Phase::Idle) && fresh(w, s) {
        // Just opened, no turn yet: nothing to put on a card.
        return None;
    }
    Some(match board_group(w, s) {
        Group::Running | Group::Working => Column::Running,
        Group::Waiting | Group::Error | Group::Paused => Column::Needs,
        Group::Stale => Column::Stale,
        _ => Column::Review,
    })
}
/// A live Agent in a Keepline terminal reads as the sidebar shows it: working, waiting on
/// you or idle. Every other session keeps its inbox group.
pub(super) fn board_group(w: &Workspace, s: &AgentSession) -> Group {
    match live_phase(w, s) {
        Some(Phase::Working) => Group::Running,
        Some(Phase::Waiting) => Group::Waiting,
        Some(Phase::Idle) => Group::Review,
        None => session_group(w, s),
    }
}
const DAY: u64 = 86_400;
fn completed_within(w: &Workspace, id: &str, now: u64, window: u64) -> bool {
    w.outcome_history
        .iter()
        .rev()
        .find(|h| h.session == id)
        .is_some_and(|h| h.after == Outcome::Completed && now.saturating_sub(h.at) < window)
}

/// Four-color gradients for a space's avatar, picked by its id so a space keeps its look.
/// No pure red or orange as a base: those belong to 出错 and 等你回答.
pub(super) fn mesh_index(seed: &str) -> usize {
    seed.bytes().fold(2166136261u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(16777619)
    }) as usize
        % T::BOARD_MESHES.len()
}
/// A round avatar in the space's gradient: three fading washes over a base, each layer
/// rounded itself since content masks are square.
pub(super) fn mesh(index: usize, size: f32) -> gpui::Div {
    let [a, b, c, base] = T::BOARD_MESHES[index % T::BOARD_MESHES.len()];
    let wash = |hex: u32, angle: f32, reach: f32| {
        let solid = T::color(hex);
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .rounded_full()
            .bg(linear_gradient(
                angle,
                linear_color_stop(solid, 0.),
                linear_color_stop(solid.opacity(0.), reach),
            ))
    };
    div()
        .relative()
        .size(px(size))
        .flex_shrink_0()
        .rounded_full()
        .bg(T::color(base))
        .child(wash(a, 135., 0.62))
        .child(wash(b, 225., 0.58))
        .child(wash(c, 0., 0.6))
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .rounded_full()
                .border(px(T::BORDER_WIDTH))
                .border_color(gpui::white().opacity(0.12)),
        )
}

/// A branch's kind from its prefix, colored like a label: feat, fix, perf, docs...
fn branch_hue(branch: &str) -> Option<u32> {
    Some(match branch.split('/').next()? {
        "feat" | "feature" => T::BRANCH_FEAT,
        "fix" | "hotfix" | "bugfix" => T::BRANCH_FIX,
        "perf" => T::BRANCH_PERF,
        "docs" | "copy" | "style" => T::BRANCH_DOCS,
        "ops" | "ci" | "chore" | "build" => T::BRANCH_OPS,
        "refactor" | "test" | "db" => T::BRANCH_REFACTOR,
        _ => return None,
    })
}

/// A rounded label with an outline in its own hue and an optional dot.
fn pill(
    text: impl Into<gpui::SharedString>,
    edge: Hsla,
    dot: Option<Hsla>,
    ink: Hsla,
) -> gpui::Div {
    div()
        .h(px(PILL))
        .px(px(T::S8))
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap(px(T::BOARD_PILL_GAP))
        .rounded(px(PILL / 2.))
        .border(px(T::BORDER_WIDTH))
        .border_color(edge)
        .text_size(px(T::META))
        .text_color(ink)
        .when_some(dot, |d, c| {
            d.child(
                div()
                    .size(px(T::BOARD_PILL_DOT))
                    .rounded_full()
                    .bg(c)
                    .flex_shrink_0(),
            )
        })
        .child(
            div()
                .max_w(px(T::BOARD_PILL_MAX))
                .truncate()
                .child(text.into()),
        )
}
const PILL: f32 = T::BOARD_PILL;
const COLUMN_MIN: f32 = T::BOARD_COLUMN_MIN;
const CARD_GAP: f32 = T::BOARD_CARD_SPACING;
const AVATAR: f32 = T::BOARD_AVATAR;
const MARK: f32 = T::BOARD_MARK;
const HEAD_ICON: f32 = T::BOARD_HEAD_ICON;

/// The CLI's mark in the muted text color, or its initial when it has none.
pub(super) fn agent_mark(agent: Agent, color: Hsla) -> gpui::AnyElement {
    match agent_icon(agent) {
        Some(path) => svg()
            .path(path)
            .size(px(MARK))
            .flex_shrink_0()
            .text_color(color)
            .into_any_element(),
        None => div()
            .text_size(px(T::CHIP_SIZE))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(color)
            .child(agent.name().chars().next().unwrap_or('?').to_string())
            .into_any_element(),
    }
}

/// How long it has waited, as four rising bars that fill and warm up with the wait.
fn wait_bars(waited: u64, hue: Hsla) -> gpui::Div {
    let filled = match waited {
        0..300 => 1,
        300..1800 => 2,
        1800..3600 => 3,
        _ => 4,
    };
    let lit = if filled >= 3 {
        hue
    } else {
        T::color(T::secondary())
    };
    div()
        .size(px(PILL))
        .flex_shrink_0()
        .flex()
        .items_end()
        .justify_center()
        .gap(px(T::BOARD_BAR_GAP))
        .pb(px(T::BOARD_BAR_INSET))
        .rounded(px(T::BOARD_BAR_FRAME_RADIUS))
        .border(px(T::BORDER_WIDTH))
        .border_color(T::color(T::card_border()))
        .children((0..4).map(|i| {
            div()
                .w(px(T::BOARD_BAR_WIDTH))
                .h(px(T::BOARD_BAR_BASE + T::BOARD_BAR_STEP * i as f32))
                .rounded(px(T::BOARD_BAR_RADIUS))
                .bg(if i < filled {
                    lit
                } else {
                    T::color(T::border())
                })
        }))
}

pub(super) fn column_icon(column: Column) -> gpui::AnyElement {
    animated_column_icon(column, false)
}
fn animated_column_icon(column: Column, animate: bool) -> gpui::AnyElement {
    let icon =
        |path: &'static str, color: Hsla| svg().path(path).size(px(HEAD_ICON)).text_color(color);
    let frame = || div().relative().size(px(HEAD_ICON)).flex_shrink_0();
    let overlay = |e: gpui::AnyElement| div().absolute().top_0().left_0().child(e);
    if !animate {
        match column {
            Column::Running => {
                return frame()
                    .child(icon(
                        "icons/ring.svg",
                        Group::Running.colors().0.opacity(0.25),
                    ))
                    .child(overlay(
                        icon("icons/arc.svg", Group::Running.colors().0).into_any_element(),
                    ))
                    .into_any_element();
            }
            Column::Needs => {
                return icon("icons/target.svg", Group::Waiting.colors().0).into_any_element();
            }
            _ => (),
        }
    }
    match column {
        Column::Todo => icon("icons/dashed.svg", T::color(T::muted())).into_any_element(),
        Column::Stale => icon("icons/dashed.svg", Group::Stale.colors().0).into_any_element(),
        Column::Review => icon("icons/half.svg", Group::Review.colors().0).into_any_element(),
        Column::Running => {
            let hue = Group::Running.colors().0;
            frame()
                .child(icon("icons/ring.svg", hue.opacity(0.25)))
                .child(overlay(
                    icon("icons/arc.svg", hue)
                        .with_animation(
                            "board-running-spin",
                            Animation::new(Duration::from_millis(1600)).repeat(),
                            |s, t| {
                                s.with_transformation(Transformation::rotate(gpui::percentage(t)))
                            },
                        )
                        .into_any_element(),
                ))
                .into_any_element()
        }
        Column::Needs => {
            let hue = Group::Waiting.colors().0;
            frame()
                .child(overlay(
                    icon("icons/ring.svg", hue)
                        .with_animation(
                            "board-needs-halo",
                            Animation::new(Duration::from_millis(2000)).repeat(),
                            move |s, t| {
                                let k = 0.7 + 0.8 * t;
                                s.with_transformation(Transformation::scale(gpui::size(k, k)))
                                    .text_color(hue.opacity(0.7 * (1. - t)))
                            },
                        )
                        .into_any_element(),
                ))
                .child(icon("icons/target.svg", hue))
                .into_any_element()
        }
        Column::Done => frame()
            .rounded_full()
            .bg(Group::Done.colors().0)
            .flex()
            .items_center()
            .justify_center()
            .child(icon("icons/check.svg", T::color(T::terminal_bg())))
            .into_any_element(),
    }
}

impl Workbench {
    /// Spaces take the gradients in their sidebar order, so the first six never share one;
    /// a session outside every space hashes its key.
    pub(super) fn space_mesh(&self, space: &str) -> usize {
        self.workspace
            .spaces
            .iter()
            .position(|s| s.id == space)
            .unwrap_or_else(|| mesh_index(space))
    }
    pub(super) fn board_sessions(&self, now: u64) -> Vec<(Column, &AgentSession)> {
        self.workspace
            .agent_sessions
            .iter()
            .filter(|s| {
                self.session_hidden(s).is_none()
                    && self.matches_filter(s)
                    && self.overview.agent.is_none_or(|a| a == s.runtime)
            })
            .filter_map(|s| column_of(&self.workspace, s, now).map(|c| (c, s)))
            .collect()
    }
    fn board_todos(&self) -> Vec<&Task> {
        self.workspace
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
                    && self.overview.agent.is_none_or(|a| a == t.agent)
                    && !self.overview.changed_only
                    && !self
                        .workspace
                        .agent_sessions
                        .iter()
                        .any(|s| s.task.as_ref() == Some(&t.id))
            })
            .collect()
    }
    /// One column's sessions in board order.
    pub(super) fn column_sessions(&self, c: Column, at: u64) -> Vec<&AgentSession> {
        let mut list: Vec<_> = self
            .board_sessions(at)
            .into_iter()
            .filter(|(col, _)| *col == c)
            .map(|(_, s)| s)
            .collect();
        self.sort_column(c, &mut list);
        list
    }
    fn sort_column(&self, c: Column, list: &mut [&AgentSession]) {
        // An Agent holding a live terminal for you comes first, then what has waited
        // longest; elsewhere the latest comes first.
        if c == Column::Needs {
            list.sort_by_key(|s| {
                (
                    !needs_attention(&self.workspace, s),
                    self.session_updated(s),
                )
            });
        } else {
            list.sort_by_key(|s| std::cmp::Reverse(self.session_updated(s)));
        }
    }
    pub(super) fn board_view(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let at = now();
        let sessions = self.board_sessions(at);
        let todos = self.board_todos();
        let open = sessions.iter().filter(|(c, _)| *c != Column::Done).count();
        let stale = sessions.iter().filter(|(c, _)| *c == Column::Stale).count();
        let empty = sessions.is_empty() && todos.is_empty();
        let mut grouped: HashMap<Column, Vec<&AgentSession>> = HashMap::new();
        for (column, session) in sessions {
            grouped.entry(column).or_default().push(session);
        }
        let todo_ids: Vec<_> = todos.iter().map(|t| t.id.clone()).collect();
        let mut columns = vec![
            self.column(
                Column::Todo,
                todo_ids.len(),
                self.card_list(Column::Todo, todo_ids, cx),
                window.is_window_active(),
                cx,
            )
            .into_any_element(),
        ];
        let mut shown = vec![Column::Running, Column::Needs, Column::Review, Column::Done];
        if self.overview.show_stale {
            shown.push(Column::Stale);
        }
        for c in shown {
            let mut list = grouped.remove(&c).unwrap_or_default();
            self.sort_column(c, &mut list);
            if c == Column::Running && !list.is_empty() {
                self.tick_turn(cx);
            }
            let ids = list.iter().map(|s| s.id.clone()).collect();
            columns.push(
                self.column(
                    c,
                    list.len(),
                    self.card_list(c, ids, cx),
                    window.is_window_active(),
                    cx,
                )
                .into_any_element(),
            );
        }
        div()
            .id("overview-scroll")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .font_features(super::diagnostics::tabular())
            .child(self.board_header(open, stale, empty, cx))
            .when_some(self.error.clone(), |d, error| {
                d.child(
                    div()
                        .px(px(T::OVERVIEW_X))
                        .pb(px(T::S8))
                        .text_color(Group::Error.colors().1)
                        .child(error),
                )
            })
            .child(
                div()
                    .id("board")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .gap(px(T::S14))
                    .px(px(T::OVERVIEW_X))
                    .pb(px(T::S16))
                    .overflow_x_scroll()
                    .children(columns),
            )
            .children(self.drawer_view(cx))
            .children(self.toast_view(cx))
    }
    fn board_header(
        &self,
        open: usize,
        stale: usize,
        empty: bool,
        cx: &Context<Self>,
    ) -> gpui::Div {
        let mut agents: Vec<(Agent, usize)> = vec![];
        for s in self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| self.session_hidden(s).is_none() && s.outcome.needs_review(now()))
        {
            match agents.iter_mut().find(|(a, _)| *a == s.runtime) {
                Some((_, n)) => *n += 1,
                None => agents.push((s.runtime, 1)),
            }
        }
        agents.sort_by_key(|(a, _)| a.name());
        let chips = agents.into_iter().map(|(agent, n)| {
            let on = self.overview.agent == Some(agent);
            let dim = self.overview.agent.is_some() && !on;
            div()
                .id(gpui::SharedString::from(format!(
                    "agent-filter-{}",
                    agent.key()
                )))
                .role(gpui::accesskit::Role::Button)
                .aria_label(gpui::SharedString::from(format!("只看 {}", agent.name())))
                .h(px(T::SEGMENT_HEIGHT))
                .pl(px(T::S6))
                .pr(px(T::S8))
                .flex()
                .items_center()
                .gap(px(T::S6))
                .rounded(px(T::SEGMENT_RADIUS))
                .border(px(T::BORDER_WIDTH))
                .border_color(T::color(if on { T::control() } else { T::card_border() }))
                .when(on, |d| d.bg(T::color(T::raised())))
                .when(dim, |d| d.opacity(0.5))
                .text_size(px(T::SMALL))
                .text_color(T::color(if on { T::primary() } else { T::body() }))
                .cursor_pointer()
                .hover(|s| s.border_color(T::color(T::control())))
                .child(agent_mark(agent, T::color(T::secondary())))
                .child(agent.name())
                .child(div().text_color(T::color(T::muted())).child(n.to_string()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.overview.agent = if this.overview.agent == Some(agent) {
                        None
                    } else {
                        Some(agent)
                    };
                    cx.notify();
                }))
        });
        let link = |id: &'static str, text: String| {
            button(id, text)
                .h(px(T::SEGMENT_HEIGHT))
                .px(px(T::S8))
                .text_color(T::color(T::secondary()))
        };
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(T::S10))
            .px(px(T::OVERVIEW_X))
            .pt(px(T::S20))
            .pb(px(T::S14))
            .child(
                div()
                    .text_size(px(T::TITLE))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(T::color(T::primary()))
                    .child(if self.overview.welcome {
                        "欢迎回来"
                    } else {
                        "鸟瞰全部"
                    }),
            )
            .child(
                // An empty board says what lands in these columns, not just 「0 个」.
                label(if empty {
                    "还没有会话：在终端里运行 claude 或 codex，它们会按进度出现在下面几列 · Esc 回到工作区".to_owned()
                } else {
                    format!("{open} 个没收尾 · Esc 回到工作区")
                })
                .text_color(T::color(T::muted())),
            )
            .child(div().w(px(T::S8)))
            .children(chips)
            .child(div().flex_1())
            .when(stale > 0 || self.overview.show_stale, |d| {
                d.child(
                    link(
                        "board-stale",
                        if self.overview.show_stale {
                            format!("收起很久没动 {stale}")
                        } else {
                            format!("很久没动 {stale}")
                        },
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.overview.show_stale = !this.overview.show_stale;
                        cx.notify();
                    })),
                )
            })
            .child(
                link("board-closed", "稍后再看与已结束".into()).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.overview.show_closed = true;
                        cx.notify();
                    },
                )),
            )
            .child(self.navigation_filters(cx))
    }
    fn column(
        &self,
        column: Column,
        count: usize,
        cards: impl IntoElement,
        active: bool,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        use super::board_drawer::{DraggedCard, legal};
        // The slot stays above the virtual viewport, so it remains visible even
        // when the receiving column is scrolled away from its first item.
        let slot = (cx.has_active_drag()
            && self
                .overview
                .drop_over
                .is_some_and(|(over, from)| over == column && legal(from, column)))
        .then(|| {
            div()
                .flex_shrink_0()
                .mb(px(CARD_GAP))
                .rounded(px(T::CARD_RADIUS))
                .border(px(T::BORDER_WIDTH))
                .border_dashed()
                .border_color(T::color(T::control()))
                .bg(T::color(T::raised()))
                .with_animation(
                    gpui::SharedString::from(format!("slot-{}", column.id())),
                    Animation::new(Duration::from_millis(260)).with_easing(gpui::ease_out_quint()),
                    |d, t| d.h(px(T::BOARD_SLOT * t)),
                )
        });
        div()
            .id(gpui::SharedString::from(format!("drop-{}", column.id())))
            .rounded(px(T::CARD_RADIUS))
            .drag_over::<DraggedCard>(move |s, card, _, _| {
                if card.from == column {
                    s
                } else if legal(card.from, column) {
                    s.bg(T::color(T::raised()))
                } else {
                    s.opacity(T::DISABLED_OPACITY)
                }
            })
            .on_drag_move::<DraggedCard>(cx.listener(
                move |this, e: &gpui::DragMoveEvent<DraggedCard>, _, cx| {
                    let inside = e.bounds.contains(&e.event.position);
                    let from = e.drag(cx).from;
                    let now_over = this.overview.drop_over.map(|(c, _)| c);
                    if inside && now_over != Some(column) {
                        this.overview.drop_over = Some((column, from));
                        cx.notify();
                    } else if !inside && now_over == Some(column) {
                        this.overview.drop_over = None;
                        cx.notify();
                    }
                },
            ))
            .on_drop(
                cx.listener(move |this, card: &DraggedCard, _, cx| {
                    this.drop_card(card, column, cx)
                }),
            )
            .flex_1()
            .min_w(px(COLUMN_MIN))
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(T::ROW + T::S6))
                    .px(px(T::S4))
                    .flex()
                    .items_center()
                    .gap(px(T::S8))
                    .child(animated_column_icon(column, active && count > 0))
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(T::color(T::primary()))
                            .child(column.label()),
                    )
                    .child(
                        div()
                            .text_color(T::color(T::muted()))
                            .child(count.to_string()),
                    )
                    .child(div().flex_1())
                    .when(column == Column::Stale && count > 0, |d| {
                        d.child(
                            button("cleanup", "逐个清理 →")
                                .h(px(T::SEGMENT_HEIGHT))
                                .px(px(T::S8))
                                .text_color(T::color(T::link()))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let at = now();
                                    let queue = this
                                        .board_sessions(at)
                                        .into_iter()
                                        .filter(|(c, _)| *c == Column::Stale)
                                        .map(|(_, s)| s.id.clone())
                                        .collect();
                                    this.start_cleanup(queue, cx);
                                })),
                        )
                    }),
            )
            .children(slot)
            .child(
                div()
                    .id(gpui::SharedString::from(format!("column-{}", column.id())))
                    .flex_1()
                    .min_h_0()
                    .child(cards),
            )
    }
    fn card_list(&self, column: Column, ids: Vec<String>, cx: &Context<Self>) -> impl IntoElement {
        let state = {
            let mut columns = self.overview.columns.borrow_mut();
            let cached = columns.entry(column).or_insert_with(|| ColumnList {
                state: gpui::ListState::new(
                    ids.len() + usize::from(column == Column::Todo),
                    gpui::ListAlignment::Top,
                    px(T::CARD_ESTIMATE),
                ),
                ids: ids.clone(),
                revision: self.workspace.revision,
            });
            cached.sync(&ids, self.workspace.revision);
            cached.state.clone()
        };
        let count = ids.len() + usize::from(column == Column::Todo);
        let entity = cx.entity();
        gpui::list(state, move |index, window, app| {
            let animate = window.is_window_active();
            entity.update(app, |this, cx| {
                let card = if column == Column::Todo {
                    if let Some(id) = ids.get(index) {
                        this.todo_card(
                            this.workspace
                                .tasks
                                .iter()
                                .find(|t| &t.id == id)
                                .expect("board row belongs to the current workspace"),
                            cx,
                        )
                        .into_any_element()
                    } else {
                        button("overview-new-task", "＋ 新待办")
                            .justify_start()
                            .text_color(T::color(T::secondary()))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.show_quick_task(window, cx)),
                            )
                            .into_any_element()
                    }
                } else {
                    this.board_card(
                        this.workspace
                            .agent_sessions
                            .iter()
                            .find(|s| s.id == ids[index])
                            .expect("board row belongs to the current workspace"),
                        column,
                        now(),
                        animate,
                        cx,
                    )
                    .into_any_element()
                };
                div()
                    .pb(px(if index + 1 < count { CARD_GAP } else { 0. }))
                    .child(card)
                    .into_any_element()
            })
        })
        .size_full()
        .pb(px(T::S20 * 2.))
    }
    fn todo_card(&self, task: &Task, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        let id = task.id.clone();
        let space = self
            .workspace
            .spaces
            .iter()
            .find(|s| s.id == task.space)
            .map_or("外部", |s| s.name.as_str());
        card_frame(gpui::SharedString::from(format!("board-todo-{id}")))
            .on_drag(
                super::board_drawer::DraggedCard {
                    id: id.clone(),
                    title: task.title.clone(),
                    from: Column::Todo,
                },
                |card, _, _, cx| cx.new(|_| card.clone()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(T::S6))
                    .text_size(px(T::META))
                    .text_color(T::color(T::muted()))
                    .child(agent_mark(task.agent, T::color(T::muted())))
                    .child("还没开始")
                    .child(div().flex_1())
                    .child(
                        mesh(self.space_mesh(&task.space), AVATAR)
                            .id(gpui::SharedString::from(format!("todo-space-{id}")))
                            .tooltip(super::diagnostics::tip(vec![space.to_owned()])),
                    ),
            )
            .child(card_title(task.title.clone()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(T::S6))
                    .child(pill(
                        task.agent.name(),
                        T::color(T::card_border()),
                        None,
                        T::color(T::body()),
                    ))
                    .child(
                        div()
                            .text_size(px(T::META))
                            .text_color(T::color(T::muted()))
                            .child(format!("{} 条验收", task.acceptance.len())),
                    )
                    .child(div().flex_1())
                    .child(
                        button(gpui::SharedString::from(format!("start-{id}")), "开始 →")
                            .h(px(PILL + 4.))
                            .px(px(T::S10))
                            .bg(T::color(T::button()))
                            .text_color(T::color(T::primary()))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.ask_agent(id.clone(), false, cx)
                            })),
                    ),
            )
            .children(self.agent_picker(&task.id, cx))
    }
    fn board_card(
        &self,
        session: &AgentSession,
        column: Column,
        now: u64,
        animate: bool,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let enter = session.clone();
        let menu_id = session.id.clone();
        let group = board_group(&self.workspace, session);
        let content = self.workspace.content.get(&session.id);
        let space_id = session_space(&self.workspace, session).unwrap_or("外部");
        let space_name = self
            .workspace
            .spaces
            .iter()
            .find(|s| s.id == space_id)
            .map_or("外部", |s| s.name.as_str());
        // The id's tail: ids share prefixes (dates, `visual-`), their ends differ.
        let alnum: Vec<char> = session
            .cli_session_id
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();
        let short: String = alnum[alnum.len().saturating_sub(4)..]
            .iter()
            .collect::<String>()
            .to_uppercase();
        let running_since = (column == Column::Running)
            .then(|| self.turn_started(session))
            .flatten();
        let when = match running_since {
            Some(at) => super::layout::turn_clock(now.saturating_sub(at)),
            None => short_ago(self.session_updated(session), now),
        };
        let branch = self.git_branch(&session.cwd, cx);
        let asks = content.map_or(0, |c| c.asks.len());
        let hover_group = gpui::SharedString::from(format!("board-card-{}", session.id));

        let status_pill = |g: Group| {
            let (dot, ink, _) = g.colors();
            pill(g.label(), dot.opacity(0.42), Some(dot), ink)
        };
        let branch_pill = branch.as_ref().map(|b| {
            let hue = branch_hue(b).map_or(T::color(T::muted()), T::color);
            pill(
                b.rsplit('/').next().unwrap_or(b).to_owned(),
                hue.opacity(0.4),
                Some(hue),
                T::color(T::body()),
            )
        });
        let mut left: Vec<gpui::AnyElement> = vec![];
        match column {
            Column::Needs => {
                let (dot, _, _) = group.colors();
                left.push(
                    wait_bars(now.saturating_sub(self.session_updated(session)), dot)
                        .into_any_element(),
                );
                left.push(status_pill(group).into_any_element());
            }
            Column::Running => {
                let (dot, ink, _) = Group::Running.colors();
                left.push(pill(when.clone(), dot.opacity(0.42), Some(dot), ink).into_any_element());
                left.extend(branch_pill.map(|p| p.into_any_element()));
            }
            Column::Stale => left.push(status_pill(Group::Stale).into_any_element()),
            _ => left.extend(branch_pill.map(|p| p.into_any_element())),
        }
        let title = self.session_title(session);
        // What it asks, unless that only repeats the title (an interrupted turn says nothing).
        let quote = (column == Column::Needs)
            .then(|| self.session_line(session))
            .filter(|q| !q.trim().is_empty() && q.trim() != title.trim());

        card_frame(gpui::SharedString::from(format!("card-{}", session.id)))
            .group(hover_group.clone())
            .aria_label(gpui::SharedString::from(format!(
                "会话：{}",
                self.session_title(session)
            )))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.overview.card_menu = None;
                this.overview.drawer = Some(enter.id.clone());
                cx.notify();
            }))
            .on_drag(
                super::board_drawer::DraggedCard {
                    id: session.id.clone(),
                    title: title.clone(),
                    from: column,
                },
                |card, _, _, cx| cx.new(|_| card.clone()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(T::S6))
                    .text_size(px(T::META))
                    .text_color(T::color(T::muted()))
                    .child(agent_mark(session.runtime, T::color(T::muted())))
                    .child(div().text_color(T::color(T::secondary())).child(short))
                    .child(
                        svg()
                            .path("icons/clock.svg")
                            .size(px(T::BOARD_CLOCK))
                            .text_color(T::color(T::muted())),
                    )
                    .child(when)
                    .child(div().flex_1())
                    .child(
                        button(
                            gpui::SharedString::from(format!("more-{}", session.id)),
                            "⋯",
                        )
                        .h(px(AVATAR))
                        .px(px(T::S4))
                        .text_color(T::color(T::muted()))
                        .invisible()
                        .group_hover(hover_group.clone(), |s| s.visible())
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
                    )
                    .child(
                        mesh(self.space_mesh(space_id), AVATAR)
                            .id(gpui::SharedString::from(format!("space-{}", session.id)))
                            .tooltip(super::diagnostics::tip(vec![space_name.to_owned()])),
                    ),
            )
            .child(card_title(title.clone()))
            .when_some(quote, |d, q| {
                d.child(
                    div()
                        .text_size(px(T::SMALL))
                        .text_color(T::color(T::secondary()))
                        .line_clamp(1)
                        .child(q),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(T::S6))
                    .children(left)
                    .child(div().flex_1())
                    .when(column == Column::Done, |d| {
                        d.child(
                            div()
                                .text_size(px(T::META))
                                .text_color(Group::Done.colors().1)
                                .child("✓ 已完成"),
                        )
                    })
                    .when(asks > 0, |d| {
                        d.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(T::S4))
                                .text_size(px(T::META))
                                .text_color(T::color(T::muted()))
                                .child(
                                    svg()
                                        .path("icons/turn.svg")
                                        .size(px(MARK))
                                        .text_color(T::color(T::muted())),
                                )
                                .child(asks.to_string()),
                        )
                    }),
            )
            .when(column == Column::Running && animate, |d| d.child(sweep()))
            .when(self.overview.card_menu.as_ref() == Some(&session.id), |d| {
                d.child(
                    gpui::deferred(
                        self.session_actions(session, cx)
                            .absolute()
                            .top(px(T::ROW + T::S6))
                            .right(px(T::S8))
                            .occlude(),
                    )
                    .priority(1),
                )
            })
            .when(self.overview.here.as_ref() == Some(&session.id), |d| {
                d.child(self.here_card(session, false, cx))
            })
    }
}

fn card_frame(id: gpui::SharedString) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .role(gpui::accesskit::Role::Button)
        .relative()
        .flex_shrink_0()
        .px(px(T::S12))
        .py(px(T::S10))
        .rounded(px(T::CARD_RADIUS))
        .bg(T::color(T::popover()))
        .border(px(T::BORDER_WIDTH))
        .border_color(T::color(T::card_border()))
        .flex()
        .flex_col()
        .gap(px(T::BOARD_CARD_GAP))
        .cursor_pointer()
        .hover(|s| s.border_color(T::color(T::control())))
}
fn card_title(text: impl Into<gpui::SharedString>) -> gpui::Div {
    div()
        .text_size(px(T::CARD_TITLE))
        .line_height(relative(1.4))
        .text_color(T::color(T::primary()))
        .line_clamp(2)
        .child(text.into())
}
/// A comet of the running hue that runs along a working card's bottom edge.
fn sweep() -> gpui::Div {
    let hue = Group::Running.colors().0;
    div()
        .absolute()
        .bottom_0()
        .left(px(T::S10))
        .right(px(T::S10))
        .h(px(T::BORDER_WIDTH))
        .overflow_hidden()
        .child(
            div()
                .absolute()
                .top_0()
                .h_full()
                .w(relative(0.4))
                .bg(linear_gradient(
                    90.,
                    linear_color_stop(hue.opacity(0.), 0.),
                    linear_color_stop(hue, 1.),
                ))
                .with_animation(
                    "board-sweep",
                    Animation::new(Duration::from_millis(2400)).repeat(),
                    |d, t| d.left(relative(-0.4 + 1.4 * t)),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branches_color_by_their_prefix() {
        assert_eq!(branch_hue("feat/peek"), Some(T::BRANCH_FEAT));
        assert_eq!(branch_hue("fix/timer"), branch_hue("hotfix/x"));
        assert_eq!(branch_hue("main"), None);
        assert_eq!(branch_hue("wip-thing"), None);
    }

    #[test]
    fn a_space_keeps_its_gradient() {
        assert_eq!(mesh_index("keepline"), mesh_index("keepline"));
        let distinct: std::collections::HashSet<_> = ["a", "b", "c", "d", "e", "f", "g", "h"]
            .iter()
            .map(|s| mesh_index(s))
            .collect();
        assert!(distinct.len() > 2, "spaces spread over the palettes");
    }
}

#[cfg(test)]
mod virtual_column_tests {
    use super::*;
    fn column(ids: &[&str], extra: usize) -> ColumnList {
        ColumnList {
            state: gpui::ListState::new(
                ids.len() + extra,
                gpui::ListAlignment::Top,
                px(T::CARD_ESTIMATE),
            ),
            ids: ids.iter().map(|s| s.to_string()).collect(),
            revision: 1,
        }
    }
    #[test]
    fn session_arrival_and_removal_preserve_the_visible_session() {
        let mut list = column(&["a", "b", "c", "d"], 0);
        list.state.scroll_to(gpui::ListOffset {
            item_ix: 2,
            offset_in_item: px(T::S12),
        });
        list.sync(&["new", "a", "b", "c", "d"].map(String::from), 2);
        let top = list.state.logical_scroll_top();
        assert_eq!(list.ids[top.item_ix], "c");
        assert_eq!(top.offset_in_item, px(T::S12));
        list.sync(&["b", "c", "d"].map(String::from), 3);
        let top = list.state.logical_scroll_top();
        assert_eq!(list.ids[top.item_ix], "c");
        assert_eq!(top.offset_in_item, px(T::S12));
    }
    #[test]
    fn content_updates_keep_the_scroll_anchor() {
        let mut list = column(&["a", "b", "c"], 0);
        list.state.scroll_to(gpui::ListOffset {
            item_ix: 1,
            offset_in_item: px(T::S8),
        });
        list.sync(&["a", "b", "c"].map(String::from), 2);
        let top = list.state.logical_scroll_top();
        assert_eq!(top.item_ix, 1);
        assert_eq!(top.offset_in_item, px(T::S8));
        assert_eq!(list.state.item_count(), 3);
    }
    #[test]
    fn todo_add_row_survives_filtering_to_empty_and_back() {
        let mut list = column(&["a", "b"], 1);
        list.sync(&[], 2);
        assert_eq!(list.state.item_count(), 1);
        list.sync(&["c".to_string()], 3);
        assert_eq!(list.state.item_count(), 2);
        assert_eq!(list.ids, ["c"]);
    }
}
