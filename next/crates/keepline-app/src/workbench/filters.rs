//! The overview's space and "only changed" filters.
use super::*;

impl Workbench {
    pub(super) fn matches_filter(&self, s: &AgentSession) -> bool {
        self.overview
            .filter_space
            .as_deref()
            .is_none_or(|space| session_space(&self.workspace, s) == Some(space))
            && (!self.overview.changed_only
                || changed_since(&self.workspace, s, self.overview.since))
            && (!self.overview.closed_only || self.session_closed(s))
    }
    pub(super) fn navigation_filters(&self, cx: &Context<Self>) -> gpui::Div {
        let mut row = div().flex().flex_wrap().gap(px(T::S8)).child(
            button(
                "all-spaces",
                if self.overview.filter_space.is_none() {
                    "● 全部空间"
                } else {
                    "全部空间"
                },
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.overview.filter_space = None;
                cx.notify();
            })),
        );
        for space in &self.workspace.spaces {
            let id = space.id.clone();
            row = row.child(
                button(
                    gpui::SharedString::from(format!("filter-{id}")),
                    format!(
                        "{}{}",
                        if self.overview.filter_space.as_ref() == Some(&id) {
                            "● "
                        } else {
                            ""
                        },
                        space.name
                    ),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.overview.filter_space = Some(id.clone());
                    cx.notify();
                })),
            );
        }
        if self.overview.closed_only {
            row = row.child(
                button("only-closed", "✓ 只看已关闭的").on_click(cx.listener(|this, _, _, cx| {
                    this.overview.closed_only = false;
                    cx.notify();
                })),
            );
        }
        row.child(
            button(
                "only-changed",
                if self.overview.changed_only {
                    "✓ 只看有变化"
                } else {
                    "只看有变化"
                },
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.overview.changed_only = !this.overview.changed_only;
                cx.notify();
            })),
        )
    }
}
