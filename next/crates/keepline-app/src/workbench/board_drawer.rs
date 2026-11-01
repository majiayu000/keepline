//! The board's moving parts: a card's detail drawer, dragging a card where dropping it is
//! an action, and the note at the bottom that says what happened, with 撤销.
use super::board::{Column, agent_mark, board_group, column_icon, mesh};
use super::*;
use gpui::{Animation, AnimationExt, relative};
use std::time::Duration;

/// What a card carries while it is dragged: a session, or a todo not handed out yet.
#[derive(Clone)]
pub(super) struct DraggedCard {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) from: Column,
}
impl gpui::Render for DraggedCard {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // Lifted off the board: a raised fill and a deep shadow.
        div()
            .w(px(T::BOARD_COLUMN_MIN))
            .px(px(T::S12))
            .py(px(T::S10))
            .rounded(px(T::CARD_RADIUS))
            .bg(T::color(T::raised()))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::control()))
            .shadow_lg()
            .text_size(px(T::CARD_TITLE))
            .text_color(T::color(T::primary()))
            .line_clamp(2)
            .child(self.title.clone())
    }
}

/// Dropping is an action, so only these moves exist: start a todo, accept finished work,
/// or send accepted work back.
pub(super) fn legal(from: Column, to: Column) -> bool {
    matches!(
        (from, to),
        (Column::Todo, Column::Running)
            | (Column::Review, Column::Done)
            | (Column::Done, Column::Review)
    )
}
/// Why a card cannot go where it was dropped, named by where it came from.
pub(super) fn refusal(from: Column) -> &'static str {
    match from {
        Column::Needs => "要你处理的会话得先在终端里回答它",
        Column::Running => "在跑的状态由 Agent 自己决定",
        Column::Review => "做完了只能拖到「已收尾」，表示验收通过",
        Column::Todo => "待办只能拖到「在跑」来开始",
        Column::Done => "已收尾只能拖回「做完了」",
        Column::Stale => "很久没动的会话用「逐个清理」处理",
    }
}

/// The note at the bottom of the board; `undo` puts that outcome back on that session.
#[derive(Clone)]
pub(super) struct Toast {
    pub(super) seq: u64,
    pub(super) text: String,
    pub(super) undo: Option<(String, Outcome)>,
    pub(super) refused: bool,
}
const TOAST_FOR: Duration = Duration::from_millis(4200);

/// Overshoots a little and settles, like a spring.
fn spring(t: f32) -> f32 {
    let (c1, c3) = (1.4_f32, 2.4_f32);
    1. + c3 * (t - 1.).powi(3) + c1 * (t - 1.).powi(2)
}

impl Workbench {
    pub(super) fn show_toast(
        &mut self,
        text: String,
        undo: Option<(String, Outcome)>,
        refused: bool,
        cx: &mut Context<Self>,
    ) {
        self.overview.toast_seq += 1;
        let seq = self.overview.toast_seq;
        self.overview.toast = Some(Toast {
            seq,
            text,
            undo,
            refused,
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TOAST_FOR).await;
            let _ = this.update(cx, |this, cx| {
                if this.overview.toast.as_ref().is_some_and(|t| t.seq == seq) {
                    this.overview.toast = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }
    pub(super) fn drop_card(&mut self, card: &DraggedCard, to: Column, cx: &mut Context<Self>) {
        self.overview.drop_over = None;
        if card.from == to {
            cx.notify();
            return;
        }
        if !legal(card.from, to) {
            self.show_toast(refusal(card.from).into(), None, true, cx);
            return;
        }
        let (outcome, before, text) = match to {
            Column::Running => {
                self.ask_agent(card.id.clone(), false, cx);
                self.show_toast(
                    format!("选一个 Agent 开始「{}」", card.title),
                    None,
                    false,
                    cx,
                );
                return;
            }
            Column::Done => (
                Outcome::Completed,
                Outcome::Open,
                format!("「{}」已验收，移到已收尾", card.title),
            ),
            _ => (
                Outcome::Open,
                Outcome::Completed,
                format!("「{}」退回做完了", card.title),
            ),
        };
        self.edit(
            Edit::SetOutcome {
                id: card.id.clone(),
                outcome,
            },
            cx,
        );
        self.show_toast(text, Some((card.id.clone(), before)), false, cx);
    }
    pub(super) fn toast_view(&self, cx: &Context<Self>) -> Option<gpui::AnyElement> {
        let toast = self.overview.toast.clone()?;
        let hue = if toast.refused {
            Group::Waiting.colors().0
        } else {
            Group::Done.colors().0
        };
        let note = div()
            .id(gpui::SharedString::from(format!(
                "board-toast-{}",
                toast.seq
            )))
            .role(gpui::accesskit::Role::Status)
            .relative()
            .h(px(T::ROW + T::S10))
            .pl(px(T::S14))
            .pr(px(T::S6))
            .flex()
            .items_center()
            .gap(px(T::S12))
            .rounded(px(T::CONTROL_RADIUS))
            .bg(T::color(T::raised()))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::control()))
            .shadow_lg()
            .text_size(px(T::SMALL))
            .text_color(T::color(T::primary()))
            .child(div().size(px(T::DOT)).rounded_full().bg(hue))
            .child(toast.text.clone())
            .when_some(toast.undo.clone(), |d, (id, outcome)| {
                d.child(
                    button("board-undo", "撤销")
                        .h(px(T::SEGMENT_HEIGHT))
                        .bg(T::color(T::button()))
                        .text_color(T::color(T::primary()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.overview.toast = None;
                            this.edit(
                                Edit::SetOutcome {
                                    id: id.clone(),
                                    outcome: outcome.clone(),
                                },
                                cx,
                            );
                        })),
                )
            })
            .with_animation(
                gpui::SharedString::from(format!("board-toast-in-{}", toast.seq)),
                Animation::new(Duration::from_millis(420)).with_easing(spring),
                |d, t| d.top(px(T::S20 * (1. - t))).opacity(t.clamp(0., 1.)),
            );
        Some(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom(px(T::S20))
                .flex()
                .justify_center()
                .child(note)
                .into_any_element(),
        )
    }

    /// The drawer's session, still on the board.
    fn drawer_session(&self) -> Option<(Column, &AgentSession)> {
        let id = self.overview.drawer.as_ref()?;
        let at = now();
        self.board_sessions(at)
            .into_iter()
            .find(|(_, s)| &s.id == id)
    }
    /// The next or previous card in the drawer's column, wrapping around.
    pub(super) fn step_drawer(&mut self, by: isize, cx: &mut Context<Self>) {
        let Some((column, session)) = self.drawer_session() else {
            return;
        };
        let list = self.column_sessions(column, now());
        let Some(i) = list.iter().position(|s| s.id == session.id) else {
            return;
        };
        let n = list.len() as isize;
        let next = list[((i as isize + by) % n + n) as usize % list.len()]
            .id
            .clone();
        self.overview.drawer = Some(next);
        cx.notify();
    }
    pub(super) fn drawer_view(&self, cx: &Context<Self>) -> Option<gpui::AnyElement> {
        let (column, s) = self.drawer_session()?;
        let at = now();
        let list = self.column_sessions(column, at);
        let index = list.iter().position(|x| x.id == s.id).unwrap_or(0);
        let content = self.workspace.content.get(&s.id);
        let group = board_group(&self.workspace, s);
        let status = if column == Column::Done {
            "已收尾"
        } else {
            group.label()
        };
        let said = content
            .map(|c| {
                [&c.waiting, &c.error, &c.last_reply]
                    .into_iter()
                    .find(|t| !t.trim().is_empty())
                    .cloned()
                    .unwrap_or_default()
            })
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| self.session_line(s));
        let space_id = session_space(&self.workspace, s).unwrap_or("外部");
        let space = self
            .workspace
            .spaces
            .iter()
            .find(|p| p.id == space_id)
            .map_or("外部", |p| p.name.as_str());
        let live = live_terminal(&self.workspace, s).is_some();
        let open = s.clone();
        let small = |id: &'static str, text: &'static str| {
            button(id, text)
                .h(px(T::ICON_HEIGHT))
                .px(px(T::S6))
                .text_color(T::color(T::secondary()))
        };
        let head = div()
            .flex()
            .items_center()
            .gap(px(T::S8))
            .text_size(px(T::SMALL))
            .text_color(T::color(T::muted()))
            .child(column_icon(column))
            .child(agent_mark(s.runtime, T::color(T::secondary())))
            .child(
                div()
                    .text_color(T::color(T::secondary()))
                    .child(s.cli_session_id.chars().take(8).collect::<String>()),
            )
            .child(format!("{} / {}", index + 1, list.len()))
            .child(
                small("drawer-up", "↑")
                    .aria_label("上一个")
                    .on_click(cx.listener(|this, _, _, cx| this.step_drawer(-1, cx))),
            )
            .child(
                small("drawer-down", "↓")
                    .aria_label("下一个")
                    .on_click(cx.listener(|this, _, _, cx| this.step_drawer(1, cx))),
            )
            .child(div().flex_1())
            .child(
                small("drawer-close", "✕")
                    .aria_label("关闭")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.overview.drawer = None;
                        cx.notify();
                    })),
            );
        let primary = gpui::SharedString::from(match (column, live) {
            (Column::Needs, true) => "打开终端回答 →",
            (_, true) => "打开终端 →",
            _ => "接着聊 →",
        });
        let mut actions = div().flex().gap(px(T::S8)).child(
            super::overview::solid(button("drawer-open", primary)).on_click(cx.listener(
                move |this, _, window, cx| {
                    this.overview.drawer = None;
                    this.open_session(&open, window, cx);
                },
            )),
        );
        let move_to = match column {
            Column::Review => Some((Column::Done, "验收通过")),
            Column::Done => Some((Column::Review, "退回做完了")),
            _ => None,
        };
        if let Some((to, text)) = move_to {
            let card = DraggedCard {
                id: s.id.clone(),
                title: self.session_title(s),
                from: column,
            };
            actions = actions.child(
                super::overview::quiet(button("drawer-move", text))
                    .on_click(cx.listener(move |this, _, _, cx| this.drop_card(&card, to, cx))),
            );
        }
        let asks: Vec<gpui::AnyElement> = content
            .map(|c| c.asks.iter().rev().take(3).collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .map(|q| {
                fact_row(
                    "你",
                    q.text.lines().next().unwrap_or("").to_owned(),
                    short_ago(q.at, at),
                )
            })
            .collect();
        let history: Vec<gpui::AnyElement> = self
            .workspace
            .outcome_history
            .iter()
            .rev()
            .filter(|h| h.session == s.id)
            .take(4)
            .map(|h| {
                fact_row(
                    "结局",
                    format!(
                        "{} → {}{}",
                        outcome_label(&h.before),
                        outcome_label(&h.after),
                        if h.reason.is_empty() {
                            String::new()
                        } else {
                            format!(" · {}", h.reason)
                        }
                    ),
                    short_ago(h.at, at),
                )
            })
            .collect();
        let mut main = div()
            .id("drawer-main")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .px(px(T::S20))
            .py(px(T::S14))
            .flex()
            .flex_col()
            .gap(px(T::S14))
            .child(head)
            .child(
                div()
                    .text_size(px(T::TITLE))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(T::color(T::primary()))
                    .child(self.session_title(s)),
            )
            .child(
                div()
                    .p(px(T::S12))
                    .rounded(px(T::CARD_RADIUS))
                    .bg(T::color(T::popover()))
                    .border(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::card_border()))
                    .flex()
                    .flex_col()
                    .gap(px(T::S6))
                    .child(label("Agent 最后说").text_color(T::color(T::muted())))
                    .child(
                        div()
                            .text_size(px(T::BODY_SIZE))
                            .line_height(relative(1.6))
                            .text_color(T::color(T::body()))
                            .line_clamp(10)
                            .child(said),
                    ),
            )
            .child(actions);
        if !asks.is_empty() {
            main = main.child(section("你问过")).children(asks);
        }
        if !history.is_empty() {
            main = main.child(section("经过")).children(history);
        }
        let mut props = vec![
            prop("状态", column_icon(column), status.to_owned()),
            prop(
                "Agent",
                agent_mark(s.runtime, T::color(T::secondary())),
                s.runtime.name().to_owned(),
            ),
            prop(
                "空间",
                mesh(self.space_mesh(space_id), T::BOARD_MARK).into_any_element(),
                space.to_owned(),
            ),
        ];
        if let Some(branch) = self.git_branch(&s.cwd, cx) {
            props.push(prop("分支", div().into_any_element(), branch));
        }
        props.push(prop(
            "回合",
            div().into_any_element(),
            content.map_or(0, |c| c.asks.len()).to_string(),
        ));
        props.push(prop(
            "最后动静",
            div().into_any_element(),
            short_ago(self.session_updated(s), at),
        ));
        props.push(prop(
            "终端",
            div().into_any_element(),
            if s.terminal.is_none() {
                "外部 · 只读".to_owned()
            } else if live {
                "Keepline 里在开".to_owned()
            } else {
                "已关闭".to_owned()
            },
        ));
        props.push(prop("目录", div().into_any_element(), s.cwd.clone()));
        let drawer = div()
            .id("board-drawer")
            .occlude()
            .absolute()
            .top_0()
            .bottom_0()
            .right_0()
            .w(relative(0.6))
            .max_w(px(T::DETAIL_WIDTH * 1.4))
            .flex()
            .bg(T::color(T::panel()))
            .border_l(px(T::BORDER_WIDTH))
            .border_color(T::color(T::border()))
            .shadow_lg()
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(main)
            .child(
                div()
                    .w(px(T::TODO_DETAIL_SIDE))
                    .flex_shrink_0()
                    .h_full()
                    .border_l(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::border()))
                    .pt(px(T::S20 * 2.))
                    .px(px(T::S14))
                    .flex()
                    .flex_col()
                    .gap(px(T::S4))
                    .children(props),
            )
            .with_animation(
                "board-drawer-in",
                Animation::new(Duration::from_millis(450)).with_easing(spring),
                |d, t| d.right(relative(-0.6 * (1. - t))),
            );
        Some(
            div()
                .id("board-scrim")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(T::color(T::scrim()).opacity(T::BOARD_SCRIM))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.overview.drawer = None;
                    cx.notify();
                }))
                .child(drawer)
                .into_any_element(),
        )
    }
}

fn section(text: &'static str) -> gpui::Div {
    div()
        .pt(px(T::S4))
        .text_size(px(T::SMALL))
        .text_color(T::color(T::muted()))
        .child(text)
}
fn fact_row(who: &'static str, text: String, when: String) -> gpui::AnyElement {
    div()
        .flex()
        .items_start()
        .gap(px(T::S10))
        .text_size(px(T::SMALL))
        .child(
            div()
                .flex_shrink_0()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(T::color(T::body()))
                .child(who),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_color(T::color(T::secondary()))
                .line_clamp(2)
                .child(text),
        )
        .child(
            div()
                .flex_shrink_0()
                .text_color(T::color(T::muted()))
                .child(when),
        )
        .into_any_element()
}
fn prop(key: &'static str, mark: gpui::AnyElement, value: String) -> gpui::AnyElement {
    div()
        .min_h(px(T::ROW))
        .flex()
        .items_center()
        .gap(px(T::S8))
        .text_size(px(T::SMALL))
        .child(
            div()
                .w(px(T::S20 * 3.))
                .flex_shrink_0()
                .text_color(T::color(T::muted()))
                .child(key),
        )
        .child(mark)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(T::color(T::body()))
                .child(value),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_moves_that_are_actions_drop() {
        assert!(legal(Column::Todo, Column::Running));
        assert!(legal(Column::Review, Column::Done));
        assert!(legal(Column::Done, Column::Review));
        for from in [Column::Needs, Column::Running, Column::Stale] {
            for to in [
                Column::Todo,
                Column::Running,
                Column::Needs,
                Column::Review,
                Column::Done,
            ] {
                assert!(!legal(from, to), "{from:?} → {to:?}");
            }
        }
        assert!(!legal(Column::Todo, Column::Done));
    }

    #[test]
    fn a_refused_drop_says_why() {
        assert!(refusal(Column::Needs).contains("回答"));
        assert!(refusal(Column::Running).contains("Agent"));
    }

    #[test]
    fn the_spring_settles_where_it_aims() {
        assert!(spring(0.).abs() < 1e-6);
        assert!((spring(1.) - 1.).abs() < 1e-6);
        assert!(
            (0..=20).any(|i| spring(i as f32 / 20.) > 1.),
            "it overshoots"
        );
    }
}
