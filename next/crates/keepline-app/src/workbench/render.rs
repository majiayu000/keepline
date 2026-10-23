//! The work view: top bar, sidebar, tabs, the pane grid and the footer.
use super::*;

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.pending_focus && self.form.is_none() && !self.overview {
            if let Some(pane) = self.focused.as_ref().and_then(|id| self.panes.get(id)) {
                window.focus(&pane.focus_handle(cx), cx);
            }
            self.pending_focus = false;
        }
        let mut root = div()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(T::color(T::APP))
            .text_color(T::color(T::PRIMARY))
            .text_size(px(T::BODY_SIZE))
            .line_height(gpui::relative(T::LINE_HEIGHT))
            .font_family(T::UI_FONT);
        // Full pages keep a title-bar row of their own for the traffic lights and dragging.
        let full_page = if self.search_open {
            Some(self.search_view(cx))
        } else if !self.cleanup.is_empty() && self.form.is_none() {
            Some(self.cleanup_view(cx))
        } else if self.overview && self.form.is_none() {
            Some(self.overview_view(window, cx))
        } else {
            None
        };
        if let Some(view) = full_page {
            return root
                .child(
                    div()
                        .h(px(T::TOP))
                        .flex_shrink_0()
                        .flex()
                        .child(self.title_drag_area(cx)),
                )
                .child(div().flex_1().min_h_0().flex().flex_col().child(view));
        }
        let space_name = self
            .workspace
            .spaces
            .iter()
            .find(|s| s.id == self.space)
            .map(|s| s.name.as_str())
            .unwrap_or("工作空间");
        root = root.child(
            div()
                .h(px(T::TOP))
                .flex_shrink_0()
                .pl(px(T::TRAFFIC_LIGHT_INSET))
                .pr(px(T::S14))
                .flex()
                .items_center()
                .gap(px(T::S16))
                .bg(T::color(T::PANEL))
                .border_b(px(T::BORDER_WIDTH))
                .border_color(T::color(T::BORDER))
                .child(
                    div()
                        .size(px(T::LOGO))
                        .rounded(px(T::CHIP_RADIUS))
                        .bg(T::color(T::CONTROL)),
                )
                .child(
                    div()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("Keepline"),
                )
                .child(label(format!("/ {space_name}")))
                .child(
                    button("overview", "▦ 鸟瞰全部")
                        .bg(T::color(T::RAISED))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.overview = true;
                            this.sync_panes(cx);
                            this.request(Request::Workspace, cx);
                        })),
                )
                .when(!self.needs_you().is_empty(), |d| {
                    d.child(
                        button(
                            "next-waiting",
                            format!("下一个等你的 · {} →", self.needs_you().len()),
                        )
                        .text_color(Group::Waiting.colors().1)
                        .on_click(cx.listener(|this, _, window, cx| this.next_waiting(window, cx))),
                    )
                })
                .child(self.search_button(cx))
                .child(self.title_drag_area(cx))
                .when(self.busy, |d| d.child(label("正在更新…")))
                .child(
                    button("add-task", "＋ 新待办").on_click(
                        cx.listener(|this, _, window, cx| this.show_quick_task(window, cx)),
                    ),
                )
                .child(self.settings_button(cx))
                .child(
                    button("refresh", "↻")
                        .aria_label("刷新")
                        .w(px(T::ROW))
                        .px(px(T::S4))
                        .on_click(
                            cx.listener(|this, _, _, cx| this.request(Request::Workspace, cx)),
                        ),
                ),
        );
        let sidebar = self.sidebar_view(space_name, cx);
        let current = self
            .workspace
            .tabs
            .iter()
            .find(|t| Some(&t.id) == self.tab.as_ref() && !t.closed);
        let mut tabs = div()
            .h(px(T::TABS))
            .flex_shrink_0()
            .flex()
            .items_center()
            .px(px(T::S6))
            .border_b(px(T::BORDER_WIDTH))
            .border_color(T::color(T::BORDER));
        for tab in self
            .workspace
            .tabs
            .iter()
            .filter(|t| !t.closed && t.space == self.space)
        {
            let id = tab.id.clone();
            let active = Some(&id) == self.tab.as_ref();
            let ids = self.workspace.tab_terminals(&id);
            let needs: Vec<_> = self
                .workspace
                .agent_sessions
                .iter()
                .filter(|s| {
                    s.current
                        && s.terminal.as_ref().is_some_and(|id| ids.contains(id))
                        && s.outcome.needs_review(now())
                        && matches!(self.group(s), Group::Waiting | Group::Error)
                })
                .collect();
            let group = needs.first().map(|s| self.group(s));
            tabs = tabs.child(
                button(gpui::SharedString::from(format!("tab-{id}")), "")
                    .aria_label(self.workspace.tab_title(tab))
                    .h(px(T::TABS))
                    .gap(px(T::S6))
                    .border_0()
                    .rounded_none()
                    .text_size(px(T::MONO_SIZE))
                    .text_color(T::color(if active { T::PRIMARY } else { T::SECONDARY }))
                    .when(active, |d| d.bg(T::color(T::RAISED)))
                    .when(tab.default_task.is_some(), |d| d.child(task_icon()))
                    .child(self.workspace.tab_title(tab))
                    .when_some(group, |d, g| d.child(dot(g)).child(badge(g, needs.len())))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.tab = Some(id.clone());
                        this.sync_panes(cx);
                        if let Some(p) = this.focused.as_ref().and_then(|id| this.panes.get(id)) {
                            window.focus(&p.focus_handle(cx), cx);
                        }
                        cx.notify();
                    })),
            );
        }
        tabs = tabs
            .child(
                button("new", "＋")
                    .aria_label("新建终端标签")
                    .border_0()
                    .text_color(T::color(T::LINK))
                    .on_click(cx.listener(|this, _, _, cx| this.launch(false, None, cx))),
            )
            .child(div().flex_1())
            .child(
                button("close-tab", "收起标签")
                    .border_0()
                    .text_size(px(T::META))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(id) = this.tab.clone() {
                            this.edit(Edit::CloseTab { id }, cx);
                        }
                    })),
            );
        let mut main = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(tabs);
        if self.recognition.phase != RecognitionPhase::Healthy {
            main = main.child(
                label(self.recognition.detail.clone())
                    .px(px(T::S14))
                    .py(px(T::S6))
                    .text_color(Group::Waiting.colors().1),
            );
        }
        if let Some(status) = cx
            .try_global::<crate::desktop::Status>()
            .filter(|s| !s.0.is_empty())
        {
            main = main.child(
                label(status.0.clone())
                    .px(px(T::S14))
                    .text_color(Group::Waiting.colors().1),
            );
        }
        if let Some(error) = &self.error {
            main = main.child(
                label(error.clone())
                    .px(px(T::S14))
                    .py(px(T::S6))
                    .text_color(Group::Error.colors().1),
            );
        }
        if let Some(form) = &self.form {
            main = main.child(self.form_panel(form, cx));
        } else if let Some(task) = self
            .task_detail
            .as_ref()
            .and_then(|id| self.workspace.tasks.iter().find(|t| &t.id == id))
        {
            main = main.child(self.task_detail_view(task, cx));
        } else {
            if let Some(task) = current
                .and_then(|t| t.default_task.as_ref())
                .and_then(|id| self.workspace.tasks.iter().find(|t| &t.id == id))
            {
                let evidence = task
                    .acceptance
                    .iter()
                    .filter(|a| a.evidence.is_some())
                    .count();
                main = main.child(
                    div()
                        .h(px(T::TODO_HEIGHT))
                        .flex_shrink_0()
                        .px(px(T::S14))
                        .flex()
                        .items_center()
                        .gap(px(T::S14))
                        .bg(T::color(T::TODO))
                        .border_b(px(T::BORDER_WIDTH))
                        .border_color(T::color(T::TODO_BORDER))
                        .child(
                            div()
                                .px(px(T::S6))
                                .rounded(px(T::CHIP_RADIUS))
                                .text_size(px(T::CHIP_SIZE))
                                .bg(T::color(T::CHIP))
                                .text_color(T::color(T::LINK))
                                .child("待办"),
                        )
                        .child(
                            div()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(task.title.clone()),
                        )
                        .child(label(format!(
                            "验收 {evidence} / {} 有证据",
                            task.acceptance.len()
                        )))
                        .child(div().flex_1())
                        .child(
                            button("prefill", "待办详情")
                                .h(px(T::FOOTER))
                                .text_color(T::color(T::LINK))
                                .on_click({
                                    let tid = task.id.clone();
                                    cx.listener(move |this, _, _, cx| {
                                        this.task_detail = Some(tid.clone());
                                        cx.notify();
                                    })
                                }),
                        ),
                );
            }
            let ids = current
                .map(|t| self.workspace.tab_terminals(&t.id))
                .unwrap_or_default();
            let count = ids.len();
            let mut grid = div().flex_1().min_h_0().min_w_0().relative();
            if let Some(zoomed) = &self.zoomed {
                grid = grid.child(self.pane_view(zoomed, false, window, cx));
            } else if let Some(tree) = current.and_then(|t| t.layout.as_ref()) {
                grid = grid.child(self.layout_view(tree, vec![], count > 1, window, cx));
            } else {
                grid = grid.child(
                    button("empty-tab-new", "开一个空终端")
                        .on_click(cx.listener(|this, _, _, cx| this.launch(true, None, cx))),
                );
            }
            main = main.child(grid);
        }
        root = root
            .relative()
            .child(div().flex_1().min_h_0().flex().child(sidebar).child(main));
        if let Some(menu) = &self.menu {
            root = root.child(self.menu_view(menu, window, cx));
        }
        root.on_mouse_move(cx.listener(Self::resize_drag))
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, _, _, cx| this.finish_drag(cx)),
            )
            .on_mouse_up_out(
                gpui::MouseButton::Left,
                cx.listener(|this, _, _, cx| this.finish_drag(cx)),
            )
    }
}
