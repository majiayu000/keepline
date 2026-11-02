//! The window's bottom edge: the host's link and the sessions on the left, the usage
//! limits, diagnostics, reminders and settings on the right. One fixed-width quota button
//! however many CLIs there are; its popover lists them all.
use super::*;
use crate::ui::widgets::gauge;
use keepline_core::quota::{NEAR, OVER, Quota, SHOWN_FOR, Window as QuotaWindow};

/// A reading older than this is marked `*`: the CLI has not been used since.
const STALE_AFTER: u64 = 3600;

/// Which of the bar's popovers is open.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum StatusPop {
    Sessions,
    Quota,
}

impl Workbench {
    pub(super) fn statusbar_view(&self, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        let open = self.statusbar;
        let cut = self.store.cut_off();
        let detail = self.store.recognition().detail.clone();
        let host = item("status-host", false)
            .child(div().size(px(T::DOT)).rounded_full().bg(if cut {
                Group::Error.colors().0
            } else {
                Group::Running.colors().0
            }))
            .child(
                div()
                    .text_color(if cut {
                        Group::Error.colors().1
                    } else {
                        T::color(T::secondary())
                    })
                    .child(if cut {
                        "后台断开，正在重连"
                    } else {
                        "后台"
                    }),
            )
            .tooltip(super::diagnostics::tip(vec![
                if cut {
                    "和后台 keeplined 的连接断了，侧栏和看板停在断开那一刻".into()
                } else {
                    "后台 keeplined 在运行，按事件更新，不定时轮询".into()
                },
                detail,
            ]));
        let (working, needs) = self.bar_counts();
        let mut sessions = item("status-sessions", open == Some(StatusPop::Sessions))
            .aria_label(format!("{working} 个工作中，{needs} 个等你"))
            .on_click(cx.listener(|this, _, _, cx| this.toggle_status(StatusPop::Sessions, cx)));
        if working == 0 && needs == 0 {
            sessions = sessions.child(
                div()
                    .text_color(T::color(T::muted()))
                    .child("没有进行中的会话"),
            );
        }
        if working > 0 {
            sessions = sessions.child(
                div()
                    .text_color(Group::Running.colors().1)
                    .child(format!("{working} 工作中")),
            );
        }
        if working > 0 && needs > 0 {
            sessions = sessions.child(div().text_color(T::color(T::muted())).child("·"));
        }
        if needs > 0 {
            sessions = sessions.child(
                div()
                    .text_color(Group::Waiting.colors().1)
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(format!("{needs} 等你")),
            );
        }
        div()
            .id("statusbar")
            .h(px(T::STATUSBAR))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(T::S2))
            .px(px(T::S6))
            .border_t(px(T::BORDER_WIDTH))
            .border_color(T::color(T::border()))
            .bg(T::color(T::panel()))
            .text_size(px(T::CHIP_SIZE))
            .font_features(T::tabular())
            .whitespace_nowrap()
            .child(host)
            .child(sessions)
            .child(div().flex_1())
            .child(self.quota_button(cx))
            .child(divider())
            .child(
                item("diagnose-host", false)
                    .aria_label("本机诊断")
                    .child("本机诊断")
                    .on_click(cx.listener(|this, _, window, cx| this.show_diagnostics(window, cx))),
            )
            // Notifications, sound and the overview's time limits live in settings;
            // this opens them on the 通知 page.
            .child(
                item("open-reminders", false)
                    .aria_label("提醒")
                    .child("提醒")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.show_settings_page(super::settings::Page::Notifications, window, cx)
                    })),
            )
            .child(
                item("open-settings", false)
                    .aria_label("设置")
                    .child("设置")
                    .on_click(cx.listener(|this, _, window, cx| this.show_settings(window, cx))),
            )
    }

    pub(super) fn toggle_status(&mut self, pop: StatusPop, cx: &mut Context<Self>) {
        self.statusbar = if self.statusbar == Some(pop) {
            None
        } else {
            Some(pop)
        };
        cx.notify();
    }

    /// The sessions the sidebar would show: working ones and those that need you.
    fn status_sessions(&self) -> (Vec<&AgentSession>, Vec<&AgentSession>) {
        let needs = self.needs_you();
        let working = self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| {
                s.current
                    && self.session_hidden(s).is_none()
                    && !self.session_closed(s)
                    && matches!(
                        session_group(&self.workspace, s),
                        Group::Working | Group::Running
                    )
                    && !needs.iter().any(|n| n.id == s.id)
            })
            .collect();
        (working, needs)
    }

    fn bar_counts(&self) -> (usize, usize) {
        let (working, needs) = self.status_sessions();
        (working.len(), needs.len())
    }

    /// The tightest CLI and its tightest window, then `+N` for the others.
    fn quota_button(&self, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        let now = now();
        let readings = tightest_first(&self.workspace, now);
        let on = self.statusbar == Some(StatusPop::Quota);
        let mut b = item("status-quota", on)
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::control()))
            .when(!on, |d| d.bg(T::color(T::pane())))
            .on_click(cx.listener(|this, _, _, cx| this.toggle_status(StatusPop::Quota, cx)));
        let Some((agent, quota, window)) = readings.first().copied() else {
            return b
                .aria_label("额度：还没有读数")
                .child(div().text_color(T::color(T::muted())).child("额度"))
                .child(caret());
        };
        let stale = now.saturating_sub(quota.at) > STALE_AFTER;
        b = b.aria_label(format!(
            "额度：{} {} 已用 {}%",
            agent.name(),
            window.map_or(String::new(), |w| w.label()),
            window.map_or(0, |w| w.used)
        ));
        b = b.child(div().text_color(T::color(T::secondary())).child(format!(
            "{}{}",
            agent.key(),
            if stale { " *" } else { "" }
        )));
        if let Some(w) = window {
            let (fill, ink) = colors(w, stale);
            b = b
                .child(div().text_color(T::color(T::muted())).child(w.label()))
                .child(
                    div()
                        .w(px(T::STATUS_GAUGE))
                        .child(gauge(share(w), fill, T::METER + 2.)),
                )
                .child(
                    div()
                        .text_color(ink)
                        .when(w.used >= NEAR && !stale, |d| {
                            d.font_weight(gpui::FontWeight::SEMIBOLD)
                        })
                        .child(format!("{}%", w.used)),
                );
            if w.used >= NEAR && w.resets_at > now {
                b = b.child(
                    div()
                        .text_color(T::color(T::muted()))
                        .child(left(w.resets_at, now)),
                );
            }
        }
        if readings.len() > 1 {
            b = b.child(
                div()
                    .px(px(T::S4))
                    .rounded_full()
                    .bg(T::color(T::chip()))
                    .text_color(T::color(T::secondary()))
                    .child(format!("+{}", readings.len() - 1)),
            );
        }
        b.child(caret())
    }

    /// The open popover over the bar; a click outside closes it.
    pub(super) fn statusbar_popover(&self, cx: &Context<Self>) -> Option<gpui::AnyElement> {
        let pop = self.statusbar?;
        let panel = div()
            .id("status-popover")
            .absolute()
            .bottom(px(T::STATUSBAR + T::S4))
            .p(px(T::S12))
            .flex()
            .flex_col()
            .gap(px(T::S6))
            .bg(T::color(T::popover()))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::control()))
            .rounded(px(T::CONTROL_RADIUS))
            .shadow_lg()
            .text_size(px(T::SMALL))
            .font_features(T::tabular())
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation());
        let panel = match pop {
            StatusPop::Quota => self.quota_popover(panel.right(px(T::S6)).w(px(T::STATUS_POP))),
            StatusPop::Sessions => {
                self.sessions_popover(panel.left(px(T::S6)).w(px(T::STATUS_POP)), cx)
            }
        };
        Some(
            gpui::deferred(
                div()
                    .absolute()
                    .inset_0()
                    .occlude()
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.statusbar = None;
                            cx.notify();
                        }),
                    )
                    .child(panel),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    fn quota_popover(&self, mut panel: gpui::Stateful<gpui::Div>) -> gpui::Stateful<gpui::Div> {
        let now = now();
        let readings = tightest_first(&self.workspace, now);
        panel = panel.child(heading("额度", "按用得最多排序 · 只读本机记录"));
        if readings.is_empty() {
            panel = panel.child(label("这一周用过的 Agent 都还没留下额度记录"));
        }
        for (agent, quota, _) in &readings {
            let stale = now.saturating_sub(quota.at) > STALE_AFTER;
            let mut head = div().flex().items_baseline().gap(px(T::S6)).child(
                div().font_weight(gpui::FontWeight::SEMIBOLD).child(format!(
                    "{}{}",
                    agent.name(),
                    if stale { " *" } else { "" }
                )),
            );
            if !quota.plan.is_empty() {
                head = head.child(label(quota.plan.clone()));
            }
            head = head
                .child(div().flex_1())
                .child(label(format!("读数 {}", ago(quota.at, now))));
            let mut block = div()
                .flex()
                .flex_col()
                .gap(px(T::S4))
                .py(px(T::S4))
                .child(head);
            for w in &quota.windows {
                let (fill, ink) = colors(w, stale);
                let reset = w.reset(now);
                block = block.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(T::S8))
                        .text_size(px(T::CHIP_SIZE))
                        .child(
                            div()
                                .w(px(T::S20 + T::S10))
                                .text_color(T::color(T::muted()))
                                .child(w.label()),
                        )
                        .child(div().w(px(T::STATUS_POP_GAUGE)).child(gauge(
                            if reset { 0. } else { share(w) },
                            fill,
                            T::METER + 2.,
                        )))
                        .child(
                            div()
                                .w(px(T::S20 + T::S20))
                                .flex()
                                .justify_end()
                                .text_color(ink)
                                .child(if reset {
                                    "已重置".into()
                                } else {
                                    format!("{}%", w.used)
                                }),
                        )
                        .child(div().flex_1())
                        .child(label(if reset {
                            "等下次使用时更新".into()
                        } else if w.resets_at == 0 {
                            String::new()
                        } else {
                            format!(
                                "{} 后 · {} 重置",
                                left(w.resets_at, now),
                                upcoming(w.resets_at, now)
                            )
                        })),
                );
            }
            panel = panel.child(
                block
                    .border_t(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::border())),
            );
        }
        let missing = unread_agents(&self.workspace, &readings, now);
        if !missing.is_empty() {
            panel = panel.child(
                label(format!("本机没有额度记录：{}", missing.join(" · ")))
                    .pt(px(T::S4))
                    .border_t(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::border())),
            );
        }
        panel.child(label(
            "只读各家在本机留下的记录，不联网。* 表示读数超过 1 小时",
        ))
    }

    fn sessions_popover(
        &self,
        mut panel: gpui::Stateful<gpui::Div>,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let (working, needs) = self.status_sessions();
        if working.is_empty() && needs.is_empty() {
            return panel.child(label("没有工作中或等你的会话"));
        }
        let now = now();
        for (title, group, list) in [
            (format!("等你 {}", needs.len()), Group::Waiting, &needs),
            (
                format!("工作中 {}", working.len()),
                Group::Running,
                &working,
            ),
        ] {
            if list.is_empty() {
                continue;
            }
            panel = panel.child(
                div()
                    .text_size(px(T::CHIP_SIZE))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(group.colors().1)
                    .child(title),
            );
            for s in list.iter() {
                let g = session_group(&self.workspace, s);
                let session = (*s).clone();
                panel = panel.child(
                    div()
                        .id(gpui::SharedString::from(format!("status-session-{}", s.id)))
                        .h(px(T::ROW))
                        .px(px(T::S6))
                        .mx(px(-T::S6))
                        .flex()
                        .items_center()
                        .gap(px(T::S8))
                        .rounded(px(T::CHIP_RADIUS))
                        .cursor_pointer()
                        .hover(|d| d.bg(T::color(T::button_hover())))
                        .child(
                            div()
                                .size(px(T::DOT))
                                .flex_shrink_0()
                                .rounded_full()
                                .bg(g.colors().0),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(self.session_title(s)),
                        )
                        .child(label(format!(
                            "{} · {}",
                            s.runtime.name(),
                            ago(s.updated, now)
                        )))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.statusbar = None;
                            this.enter_card(&session, window, cx);
                        })),
                );
            }
        }
        panel
    }

    /// The pane header's warning when the CLI running there has used up a window.
    pub(super) fn quota_alert(&self, session: &AgentSession) -> Option<String> {
        let now = now();
        let (_, quota) = keepline_core::quota::latest(&self.workspace, now)
            .into_iter()
            .find(|(a, _)| *a == session.runtime)?;
        let w = quota
            .windows
            .iter()
            .filter(|w| !w.reset(now) && w.used >= OVER)
            .max_by_key(|w| w.used)?;
        Some(if w.resets_at > now {
            format!(
                "{} {}额度已用 {}%，{}后重置",
                session.runtime.name(),
                w.label(),
                w.used,
                left(w.resets_at, now)
            )
        } else {
            format!(
                "{} {}额度已用 {}%",
                session.runtime.name(),
                w.label(),
                w.used
            )
        })
    }
}

/// Every CLI with a reading, the one closest to its limit first, with that window.
fn tightest_first(
    w: &Workspace,
    now: u64,
) -> Vec<(keepline_core::agent::Agent, &Quota, Option<&QuotaWindow>)> {
    let mut out: Vec<_> = keepline_core::quota::latest(w, now)
        .into_iter()
        .map(|(a, q)| (a, q, tightest(q, now)))
        .collect();
    out.sort_by_key(|(_, _, w)| std::cmp::Reverse(w.map_or(0, |w| w.used)));
    out
}

fn tightest(q: &Quota, now: u64) -> Option<&QuotaWindow> {
    q.windows
        .iter()
        .filter(|w| !w.reset(now))
        .max_by_key(|w| w.used)
}

/// CLIs used this week that never left a reading here (Cursor, Qoder).
fn unread_agents(
    w: &Workspace,
    readings: &[(keepline_core::agent::Agent, &Quota, Option<&QuotaWindow>)],
    now: u64,
) -> Vec<&'static str> {
    let mut out: Vec<keepline_core::agent::Agent> = Vec::new();
    for s in &w.agent_sessions {
        if now.saturating_sub(s.updated) <= SHOWN_FOR
            && !readings.iter().any(|(a, ..)| *a == s.runtime)
            && !out.contains(&s.runtime)
        {
            out.push(s.runtime);
        }
    }
    out.into_iter().map(|a| a.name()).collect()
}

fn share(w: &QuotaWindow) -> f64 {
    f64::from(w.used.min(100)) / 100.
}

/// The bar's fill and the number's ink: green, orange from 80%, red from 95%; gray once stale.
fn colors(w: &QuotaWindow, stale: bool) -> (gpui::Hsla, gpui::Hsla) {
    match w.used {
        _ if stale => (T::color(T::muted()), T::color(T::muted())),
        u if u >= OVER => (Group::Error.colors().0, Group::Error.colors().1),
        u if u >= NEAR => (Group::Waiting.colors().0, Group::Waiting.colors().1),
        _ => (Group::Running.colors().0, T::color(T::body())),
    }
}

/// Time left as the bar has room for: `2d4h`, `4h20m`, `12m`.
pub(super) fn left(at: u64, now: u64) -> String {
    let s = at.saturating_sub(now);
    let (d, h, m) = (s / 86_400, s % 86_400 / 3600, s % 3600 / 60);
    if d > 0 {
        format!("{d}d{h}h")
    } else if h > 0 {
        format!("{h}h{m}m")
    } else {
        format!("{}m", m.max(1))
    }
}

/// One clickable spot on the bar, filled while its popover is open.
fn item(id: &'static str, on: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .role(gpui::accesskit::Role::Button)
        .h(px(T::STATUSBAR - T::S6))
        .px(px(T::S8))
        .flex()
        .items_center()
        .gap(px(T::S6))
        .rounded(px(T::CHIP_RADIUS))
        .text_color(T::color(T::secondary()))
        .cursor_pointer()
        .hover(|s| s.bg(T::color(T::button_hover())))
        .when(on, |d| {
            d.bg(T::color(T::selection()))
                .text_color(T::color(T::primary()))
        })
}

fn divider() -> gpui::Div {
    div()
        .w(px(T::BORDER_WIDTH))
        .h(px(T::S14))
        .mx(px(T::S4))
        .bg(T::color(T::border()))
}

fn caret() -> gpui::Div {
    div()
        .text_size(px(T::S8))
        .text_color(T::color(T::muted()))
        .child("▲")
}

fn heading(title: &'static str, note: &'static str) -> gpui::Div {
    div()
        .flex()
        .items_baseline()
        .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(title))
        .child(div().flex_1())
        .child(label(note))
}

#[cfg(test)]
mod tests {
    use super::left;

    #[test]
    fn time_left_fits_the_bar() {
        assert_eq!(left(1000 + 2 * 86_400 + 4 * 3600 + 59, 1000), "2d4h");
        assert_eq!(left(1000 + 4 * 3600 + 20 * 60, 1000), "4h20m");
        assert_eq!(left(1000 + 30, 1000), "1m");
        assert_eq!(left(500, 1000), "1m");
    }
}
