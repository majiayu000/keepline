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
    /// The overview's scope as one segmented switch. 「全部空间」 clears the space the
    /// sidebar picked; that space and 只看已关闭的 show as pressed segments with ×;
    /// 「只看有变化」 toggles on its own.
    pub(super) fn navigation_filters(&self, cx: &Context<Self>) -> gpui::Div {
        let segment = |id: &'static str, text: String, pressed: bool| {
            button(id, text)
                .h(px(T::SEGMENT_HEIGHT))
                .px(px(T::S10))
                .rounded(px(T::SEGMENT_RADIUS))
                .text_color(T::color(if pressed {
                    T::primary()
                } else {
                    T::secondary()
                }))
                .when(pressed, |b| b.bg(T::color(T::popover())))
        };
        let mut row = div()
            .flex()
            .p(px(T::S2))
            .rounded(px(T::SEGMENT_TRACK_RADIUS))
            .bg(T::color(T::raised()))
            .child(
                segment(
                    "all-spaces",
                    "全部空间".into(),
                    self.overview.filter_space.is_none(),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.overview.filter_space = None;
                    cx.notify();
                })),
            );
        // The sidebar's spaces filter the overview; the switch only names and clears it.
        if let Some(space) = self
            .overview
            .filter_space
            .as_ref()
            .and_then(|id| self.workspace.spaces.iter().find(|s| &s.id == id))
        {
            row = row.child(
                segment("filter-space", format!("{} ×", space.name), true)
                    .aria_label(format!("取消只看 {}", space.name))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.overview.filter_space = None;
                        cx.notify();
                    })),
            );
        }
        if self.overview.closed_only {
            row = row.child(
                segment("only-closed", "只看已关闭的 ×".into(), true).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.overview.closed_only = false;
                        cx.notify();
                    },
                )),
            );
        }
        row.child(
            segment(
                "only-changed",
                "只看有变化".into(),
                self.overview.changed_only,
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.overview.changed_only = !this.overview.changed_only;
                cx.notify();
            })),
        )
    }
}
