//! 逐个清理: one stale session at a time.
use super::*;

impl Workbench {
    pub(super) fn cleanup_view(&self, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        let mut page = div()
            .w_full()
            .max_w(px(T::CLEANUP_WIDTH))
            .flex()
            .flex_col()
            .gap(px(T::S16))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(T::S12))
                    .child(
                        button("cleanup-exit", "← 返回鸟瞰")
                            .border_0()
                            .text_color(T::color(T::LINK))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.overview.cleanup.clear();
                                this.nav.overview = true;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(T::FORM_TITLE))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("清理很久没动的会话"),
                    )
                    .child(div().flex_1())
                    .child(
                        label(format!(
                            "第 {} / {} 个",
                            (self.overview.cleanup_index + 1).min(self.overview.cleanup.len()),
                            self.overview.cleanup.len()
                        ))
                        .text_color(T::color(T::SECONDARY)),
                    ),
            )
            .child(
                div()
                    .h(px(T::S4))
                    .w_full()
                    .rounded_full()
                    .bg(T::color(T::CONTROL))
                    .child(div().h_full().rounded_full().bg(T::color(T::ACCENT)).w(
                        gpui::relative(
                            self.overview.cleanup_index as f32
                                / self.overview.cleanup.len().max(1) as f32,
                        ),
                    )),
            );
        if let Some(s) = self
            .overview
            .cleanup
            .get(self.overview.cleanup_index)
            .and_then(|id| self.workspace.agent_sessions.iter().find(|s| &s.id == id))
        {
            let h = self.workspace.content.get(&s.id).map(|c| c.here());
            if let Some(h) = h {
                let mut card = div()
                    .p(px(T::S20))
                    .rounded(px(T::CARD_RADIUS))
                    .bg(T::color(T::POPOVER))
                    .flex()
                    .flex_col()
                    .gap(px(T::S16))
                    .child(
                        div()
                            .text_size(px(T::TITLE))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(self.session_title(s)),
                    )
                    .child(
                        label(format!(
                            "{} · {} · {}",
                            s.runtime.executable(),
                            age(self
                                .workspace
                                .content
                                .get(&s.id)
                                .map_or(s.updated, |c| c.updated)),
                            s.cwd
                        ))
                        .text_color(T::color(T::SECONDARY)),
                    );
                // The title already is the first ask when nothing better names the session.
                let title = self.session_title(s);
                // As in the 你在这里 card: no empty evidence line, no template next step.
                for (name, text) in [("你要它", h.ask), ("做成了", h.done), ("停在", h.stopped)]
                    .into_iter()
                    .filter(|(name, text)| {
                        (*name != "你要它" || text.trim() != title.trim())
                            && (*name != "做成了" || text != keepline_core::content::NO_DONE)
                    })
                {
                    card = card.child(
                        div()
                            .flex()
                            .gap(px(T::S16))
                            .py(px(T::S8))
                            .child(
                                label(name)
                                    .w(px(T::HERE_LABEL))
                                    .flex_shrink_0()
                                    .text_color(T::color(T::SECONDARY)),
                            )
                            .child(div().flex_1().child(text)),
                    );
                }
                page = page.child(card);
            }
            let mut actions = div().flex().gap(px(T::S8));
            let resume = s.clone();
            actions = actions.child(
                button("cleanup-continue", "继续")
                    .h(px(T::CLEANUP_ACTION_HEIGHT))
                    .flex_1()
                    .justify_center()
                    .bg(T::color(T::ACCENT))
                    .text_color(T::color(T::ON_ACCENT))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.overview.cleanup.clear();
                        this.open_session(&resume, window, cx);
                    })),
            );
            {
                let (name, outcome) = ("完成", Outcome::Completed);
                let sid = s.id.clone();
                actions = actions.child(
                    button(name, name)
                        .h(px(T::CLEANUP_ACTION_HEIGHT))
                        .flex_1()
                        .justify_center()
                        .bg(T::color(T::POPOVER))
                        .text_color(T::color(T::PRIMARY))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.edit(
                                Edit::SetOutcome {
                                    id: sid.clone(),
                                    outcome: outcome.clone(),
                                },
                                cx,
                            )
                        })),
                );
            }
            let sid = s.id.clone();
            actions = actions.child(
                button("cleanup-defer", "稍后再看…")
                    .h(px(T::CLEANUP_ACTION_HEIGHT))
                    .flex_1()
                    .justify_center()
                    .bg(T::color(T::POPOVER))
                    .text_color(T::color(T::PRIMARY))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.overview.here = Some(sid.clone());
                        this.show_form(FormKind::Defer, window, cx);
                    })),
            );
            let sid = s.id.clone();
            actions = actions.child(
                button("cleanup-abandon", "放弃")
                    .h(px(T::CLEANUP_ACTION_HEIGHT))
                    .flex_1()
                    .justify_center()
                    .bg(T::color(T::POPOVER))
                    .border_color(T::color(T::CONTROL))
                    .text_color(T::color(T::PRIMARY))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.edit(
                            Edit::SetOutcome {
                                id: sid.clone(),
                                outcome: Outcome::Abandoned,
                            },
                            cx,
                        )
                    })),
            );
            page = page.child(actions);
        } else {
            page = page.child("这一轮已处理完，可以返回鸟瞰。");
        }
        page = page.child(
            div()
                .flex()
                .gap(px(T::S8))
                .child(
                    button("cleanup-previous", "上一个")
                        .text_color(T::color(T::PRIMARY))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.overview.cleanup_index =
                                this.overview.cleanup_index.saturating_sub(1);
                            cx.notify();
                        })),
                )
                .child(
                    button("cleanup-skip", "跳过")
                        .text_color(T::color(T::PRIMARY))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.overview.cleanup_index =
                                (this.overview.cleanup_index + 1).min(this.overview.cleanup.len());
                            cx.notify();
                        })),
                ),
        );
        div()
            .id("cleanup-page")
            .size_full()
            .bg(T::color(T::APP))
            .text_color(T::color(T::PRIMARY))
            .overflow_y_scroll()
            .p(px(T::S20))
            .flex()
            .flex_col()
            .items_center()
            .child(page)
    }
}
