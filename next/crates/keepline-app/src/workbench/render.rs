//! The work view: top bar, sidebar, tabs, the pane grid and the footer.
use super::*;

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The overview and 逐个清理 take the main area beside the sidebar; the top bar,
        // sidebar and tab strip stay, so leaving is a click in the same place or Esc.
        let page = if self.tasks.form.is_some() {
            None
        } else if !self.overview.cleanup.is_empty() {
            Some(self.cleanup_view(cx))
        } else if self.nav.overview {
            Some(self.overview_view(window, cx))
        } else {
            None
        };
        if self.nav.pending_focus
            && page.is_some()
            && !self.search.is_open()
            && self.diagnostics.is_none()
            && self.settings.is_none()
        {
            window.focus(&self.overview.focus, cx);
            self.nav.pending_focus = false;
        }
        if self.nav.pending_focus
            && self.diagnostics.is_none()
            && self.settings.is_none()
            && self.tasks.form.is_none()
            && self.layout.renaming_tab.is_none()
            && self.layout.renaming_session.is_none()
            && self.layout.renaming_space.is_none()
            && !self.nav.overview
        {
            if let Some(pane) = self
                .nav
                .focused
                .as_ref()
                .and_then(|id| self.layout.panes.get(id))
            {
                window.focus(&pane.focus_handle(cx), cx);
            }
            self.nav.pending_focus = false;
        }
        let terminal_style = crate::ui::style::current(cx) == crate::ui::style::Style::Terminal;
        let mut root = div()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(T::color(T::app()))
            .text_color(T::color(T::primary()))
            .text_size(px(T::BODY_SIZE))
            .line_height(gpui::relative(T::LINE_HEIGHT))
            .font_family(T::UI_FONT)
            .when(terminal_style, |d| d.font(T::mono(cx)));
        let space_name = self
            .workspace
            .spaces
            .iter()
            .find(|s| s.id == self.nav.space)
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
                .bg(T::color(T::panel()))
                .border_b(px(T::BORDER_WIDTH))
                .border_color(T::color(T::border()))
                .child(
                    div()
                        .id("app-icon")
                        .role(gpui::accesskit::Role::Button)
                        .aria_label("换图标")
                        .flex_shrink_0()
                        .cursor_pointer()
                        .child(
                            gpui::img(cx.global::<Chosen>().icon.title_image()).size(px(T::LOGO)),
                        )
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                                // Drops below the title bar so the logo stays in view.
                                this.layout.icon_menu =
                                    Some(gpui::point(event.position.x, px(T::TOP)));
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        ),
                )
                .child(
                    div()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("Keepline"),
                )
                .child(label(format!("/ {space_name}")))
                // A toggle: the same spot goes into the overview and back out of it. Its
                // label and everything before it keep their width, so nothing shifts.
                .child({
                    let on = self.nav.overview || !self.overview.cleanup.is_empty();
                    let toggle = button("overview", "▦ 鸟瞰全部")
                        .aria_label(if on {
                            "回到工作区"
                        } else {
                            "鸟瞰全部"
                        })
                        .bg(T::color(if on { T::selection() } else { T::raised() }))
                        .when(on, |d| {
                            d.border_color(T::color(T::focus()))
                                .text_color(T::color(T::primary()))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if on {
                                this.leave_overview(cx);
                            } else {
                                this.show_overview(cx);
                            }
                        }));
                    self.record_toggle(toggle, cx)
                })
                .child(self.search_button(cx))
                .child(self.title_drag_area(cx))
                .when(self.store.slow(), |d| d.child(label("正在更新…")))
                .when(
                    matches!(
                        self.store.recognition().phase,
                        RecognitionPhase::Starting | RecognitionPhase::Recovering
                    ),
                    |d| d.child(label("正在识别会话…")),
                )
                // The host runs older code than this app. One that can hand over is updated
                // in place; an older one must be restarted, which ends every terminal.
                .when(crate::lifecycle::stale_host(), |d| {
                    d.child(
                        button(
                            "restart-host",
                            match (
                                self.restarting,
                                crate::lifecycle::stale_build().is_some_and(|b| b.handoff),
                            ) {
                                (true, _) => "正在更新后台…",
                                // Updated in place: every terminal keeps running.
                                (false, true) => "后台有更新 · 更新",
                                (false, false) => "后台有更新 · 重启",
                            },
                        )
                        .text_color(T::color(T::secondary()))
                        .on_click(cx.listener(|this, _, _, cx| {
                            if !this.restarting {
                                this.request_close(Closing::Host, cx);
                            }
                        })),
                    )
                })
                .when(!self.needs_you().is_empty(), |d| {
                    d.child(self.next_waiting_chip(cx))
                })
                .child(
                    button("add-task", "＋ 新待办").on_click(
                        cx.listener(|this, _, window, cx| this.show_quick_task(window, cx)),
                    ),
                ),
        );
        let sidebar = self.sidebar_view(space_name, cx);
        let current = self
            .workspace
            .tabs
            .iter()
            .find(|t| Some(&t.id) == self.nav.tab.as_ref());
        // Long conversation titles stay on one row: each tab is capped and elided, and
        // the strip scrolls sideways while 「＋」 and 「收起标签」 stay put.
        let mut strip = div()
            .id("tab-strip")
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .overflow_x_scroll();
        for (n, tab) in self
            .workspace
            .tabs
            .iter()
            .filter(|t| t.space == self.nav.space)
            .enumerate()
        {
            let id = tab.id.clone();
            // Over the overview no tab is shown; clicking one goes straight back to it.
            let active = page.is_none() && Some(&id) == self.nav.tab.as_ref();
            let ids = self.workspace.tab_terminals(&id);
            let needs: Vec<_> = self
                .workspace
                .agent_sessions
                .iter()
                .filter(|s| {
                    s.current
                        && s.terminal.as_ref().is_some_and(|id| ids.contains(id))
                        && s.outcome.needs_review(now())
                        && matches!(
                            session_group(&self.workspace, s),
                            Group::Waiting | Group::Error
                        )
                })
                .collect();
            let group = needs.first().map(|s| session_group(&self.workspace, s));
            // The tab's dot: the most urgent status among its panes; a tab of plain
            // shells keeps a faint neutral dot so the names stay aligned.
            let status = ids
                .iter()
                .filter_map(|id| self.pane_status(id))
                .max_by_key(|(g, attention)| {
                    (*attention, std::cmp::Reverse(super::layout::urgency(*g)))
                })
                .map(|(g, _)| g);
            if let Some((_, field, _)) =
                self.layout.renaming_tab.as_ref().filter(|(t, ..)| t == &id)
            {
                // Edited in place: the tab keeps its height, padding and icon, and only
                // its name turns editable, outlined so the edit is plain to see. A long
                // name gets wider room while typed; the strip scrolls to make space.
                strip = strip.child(
                    div()
                        .flex_shrink_0()
                        .min_w(px(T::TAB_RENAME_MIN))
                        .max_w(px(T::TAB_RENAME_MAX))
                        .h(px(T::TABS))
                        .px(px(T::S12 - T::BORDER_WIDTH))
                        .flex()
                        .items_center()
                        .gap(px(T::S6))
                        .bg(T::color(T::raised()))
                        .border(px(T::BORDER_WIDTH))
                        .border_color(T::color(T::focus()))
                        .text_size(px(T::MONO_SIZE))
                        .text_color(T::color(T::primary()))
                        .when(tab.default_task.is_some(), |d| d.child(task_icon()))
                        .child(div().flex_1().min_w_0().h_full().child(field.clone())),
                );
                continue;
            }
            let title = self.tab_name(tab, cx);
            let tip = self.tab_tip(&title, &ids);
            let dragged = super::tabs::DraggedTab {
                id: id.clone(),
                title: title.clone(),
            };
            let drop_before = id.clone();
            let group_name = gpui::SharedString::from(format!("tab-group-{id}"));
            // Shown on the current tab, and on the others while hovered, like a browser.
            let close = button(gpui::SharedString::from(format!("tab-close-{id}")), "×")
                .aria_label(format!("关闭标签 {title}"))
                .flex_shrink_0()
                .h(px(T::ICON_HEIGHT))
                .px(px(T::S4))
                .text_color(T::color(T::secondary()))
                .when(!active, |d| {
                    d.opacity(0.)
                        .group_hover(group_name.clone(), |s| s.opacity(1.))
                })
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click({
                    let id = id.clone();
                    cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.close_tab(&id, cx);
                    })
                });
            strip = strip.child(
                button(gpui::SharedString::from(format!("tab-{id}")), "")
                    .group(group_name)
                    .aria_label(title.clone())
                    .relative()
                    .flex_shrink_0()
                    .min_w(px(if terminal_style { 0. } else { T::TAB_MIN_WIDTH }))
                    .max_w(px(T::TAB_MAX_WIDTH))
                    .h(px(if terminal_style { T::FOOTER } else { T::TABS }))
                    .gap(px(T::S6))
                    .border_0()
                    .rounded_none()
                    .text_size(px(T::MONO_SIZE))
                    .text_color(T::color(if active { T::primary() } else { T::secondary() }))
                    .when(tab.default_task.is_some(), |d| d.child(task_icon()))
                    // Terminal style: `2:标题*` with the state as a symbol, like tmux.
                    .when(terminal_style, |d| {
                        d.when_some(status, |d, g| d.child(glyph(g)))
                            .when(active, |d| d.font_weight(gpui::FontWeight::SEMIBOLD))
                    })
                    .child(
                        div()
                            .flex_grow_1()
                            .min_w_0()
                            .truncate()
                            .child(if terminal_style {
                                format!("{}:{title}{}", n + 1, if active { "*" } else { "" })
                            } else {
                                title
                            }),
                    )
                    // Standard: a tab only calls out when a pane in it needs you.
                    .when_some(group.filter(|_| !terminal_style), |d, g| {
                        d.child(badge(g, needs.len()).font_features(T::tabular()))
                    })
                    .child(close)
                    // The current tab takes the pane's surface and opens into it: side
                    // hairlines, and it covers the strip's bottom edge.
                    .when(active && !terminal_style, |d| {
                        d.bg(T::color(T::pane()))
                            .border_l(px(T::BORDER_WIDTH))
                            .border_r(px(T::BORDER_WIDTH))
                            .border_color(T::color(T::border()))
                            .self_end()
                            .mb(px(-T::BORDER_WIDTH))
                            .h(px(T::TABS))
                    })
                    // The hover card would cover the right-click menu; dropping the
                    // builder while a menu is open also dismisses a visible card.
                    .when(self.layout.tab_menu.is_none(), |d| {
                        d.tooltip(move |_, cx| cx.new(|_| tip.clone()).into())
                    })
                    .on_mouse_down(gpui::MouseButton::Middle, {
                        let id = id.clone();
                        cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.close_tab(&id, cx);
                        })
                    })
                    // Dragging a tab onto another puts it right before that one.
                    .on_drag(dragged, |tab, _, _, cx| cx.new(|_| tab.clone()))
                    .drag_over::<super::tabs::DraggedTab>(|s, _, _, _| {
                        s.border_l(px(T::BORDER_WIDTH * 2.))
                            .border_color(T::color(T::accent()))
                    })
                    .on_drop(
                        cx.listener(move |this, tab: &super::tabs::DraggedTab, _, cx| {
                            if tab.id != drop_before {
                                this.edit(
                                    Edit::MoveTab {
                                        id: tab.id.clone(),
                                        before: Some(drop_before.clone()),
                                    },
                                    cx,
                                );
                            }
                        }),
                    )
                    .on_mouse_down(gpui::MouseButton::Right, {
                        let id = id.clone();
                        cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                            this.layout.tab_menu = Some((id.clone(), event.position));
                            cx.stop_propagation();
                            cx.notify();
                        })
                    })
                    .on_click(
                        cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                            if event.click_count() == 2 {
                                this.rename_tab(&id, window, cx);
                                return;
                            }
                            this.show_tab(&id, window, cx);
                        }),
                    ),
            );
        }
        let new_tab = button("new", "＋")
            .aria_label("新建终端标签")
            .flex_shrink_0()
            .border_0()
            .text_color(T::color(T::link()))
            .on_click(cx.listener(|this, _, _, cx| this.launch(false, None, cx)));
        // Terminal style puts the tabs in a status line under the panes, tmux-like:
        // the space, the tabs, then what needs you on the right.
        let tabs = if terminal_style {
            div()
                .h(px(T::FOOTER))
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap(px(T::S8))
                .px(px(T::S8))
                .border_t(px(T::BORDER_WIDTH))
                .border_color(T::color(T::border()))
                .bg(T::color(T::panel()))
                .text_size(px(T::MONO_SIZE))
                .child(label(space_name.to_owned()).flex_shrink_0())
                .child(strip.flex_1())
                .child(new_tab)
                .child(self.status_glyphs(cx))
        } else {
            div()
                .h(px(T::TABS))
                .flex_shrink_0()
                .flex()
                .items_center()
                .px(px(T::S6))
                .border_b(px(T::BORDER_WIDTH))
                .border_color(T::color(T::border()))
                .child(strip)
                .child(new_tab)
        };
        let (top_tabs, bottom_tabs) = if terminal_style {
            (None, Some(tabs))
        } else {
            (Some(tabs), None)
        };
        let mut main = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .children(top_tabs);
        // Indexing in progress is a top-bar note; only a lost link to the host is worth a
        // strip, in words the user can act on. A missing official status source is not:
        // logs and screens still tell the state. The menubar's failed refresh is the same
        // outage, so while the link is down it only adds to the strip's details.
        let status = cx
            .try_global::<crate::desktop::Status>()
            .map(|s| s.0.clone())
            .filter(|s| !s.is_empty());
        if self.store.cut_off() {
            let mut details = vec![self.store.recognition().detail.clone()];
            details.extend(status);
            main = main.child(self.notice_strip("后台连接断开，正在重新连接…", details, true, cx));
        } else if let Some(status) = status.filter(|_| {
            // A failed refresh while the link is down for a moment is the same blip.
            self.store.recognition().phase != RecognitionPhase::Interrupted
        }) {
            let (text, detail) = status.split_once('：').unwrap_or((&status, ""));
            let details = vec![detail.to_string()];
            main = main.child(self.notice_strip(text, details, false, cx));
        }
        if let Some(error) = &self.error {
            main = main.child(
                label(error.clone())
                    .px(px(T::S14))
                    .py(px(T::S6))
                    .text_color(Group::Error.colors().1),
            );
        }
        if let Some(form) = &self.tasks.form {
            main = main.child(self.form_panel(form, cx));
        } else if let Some(page) = page {
            main = main.child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .track_focus(&self.overview.focus)
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            if !this.search.is_open() {
                                window.focus(&this.overview.focus, cx);
                            }
                        }),
                    )
                    .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                        let k = &event.keystroke;
                        if !k.modifiers.modified() && this.cleanup_key(&k.key, window, cx) {
                            cx.stop_propagation();
                            return;
                        }
                        if k.key != "escape" || k.modifiers.modified() {
                            return;
                        }
                        // A card's menu closes first; 逐个清理 steps back to the overview.
                        if this.overview.card_menu.take().is_some() {
                        } else if !this.overview.cleanup.is_empty() {
                            this.overview.cleanup.clear();
                        } else {
                            this.leave_overview(cx);
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .child(page),
            );
        } else if let Some(task) = self
            .nav
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
                        .bg(T::color(T::todo()))
                        .border_b(px(T::BORDER_WIDTH))
                        .border_color(T::color(T::todo_border()))
                        .child(
                            div()
                                .px(px(T::S6))
                                .rounded(px(T::CHIP_RADIUS))
                                .text_size(px(T::CHIP_SIZE))
                                .bg(T::color(T::chip()))
                                .text_color(T::color(T::link()))
                                .child("待办"),
                        )
                        .child(
                            div()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(task.title.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(T::S6))
                                .font_features(T::tabular())
                                .when(!task.acceptance.is_empty(), |d| {
                                    d.child(evidence_cells(evidence, task.acceptance.len()))
                                })
                                .child(label(format!(
                                    "验收 {evidence} / {} 有证据",
                                    task.acceptance.len()
                                ))),
                        )
                        .child(div().flex_1())
                        .child(
                            button("prefill", "待办详情")
                                .h(px(T::FOOTER))
                                .text_color(T::color(T::link()))
                                .on_click({
                                    let tid = task.id.clone();
                                    cx.listener(move |this, _, _, cx| {
                                        this.nav.task_detail = Some(tid.clone());
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
            if let Some(zoomed) = &self.nav.zoomed {
                grid = grid.child(self.pane_view(zoomed, false, window, cx));
            } else if let Some(tree) = current.and_then(|t| t.layout.as_ref()) {
                grid = grid.child(self.layout_view(tree, vec![], count > 1, window, cx));
            } else {
                grid = grid.child(
                    button("empty-tab-new", "开一个空终端")
                        .on_click(cx.listener(|this, _, _, cx| this.launch(false, None, cx))),
                );
            }
            main = main.child(grid);
        }
        main = main.children(bottom_tabs);
        if let Some(peek) = &self.peek {
            main = main.relative().child(self.peek_view(peek, cx));
        }
        root = root
            .relative()
            .child(div().flex_1().min_h_0().flex().child(sidebar).child(main));
        if let Some(menu) = &self.layout.menu {
            root = root.child(self.menu_view(menu, window, cx));
        }
        if let Some(position) = self.layout.icon_menu {
            root = root.child(self.icon_menu_view(position, window, cx));
        }
        if let Some((id, position)) = &self.layout.tab_menu {
            root = root.child(self.tab_menu_view(id, *position, window, cx));
        }
        if let Some((id, position)) = &self.layout.space_menu {
            root = root.child(self.space_menu_view(id, *position, window, cx));
        }
        if let Some(closing) = &self.layout.confirm_close {
            root = root.child(self.confirm_close_view(closing, cx));
        }
        if self.search.is_open() {
            root = root.child(self.search_palette(cx));
        }
        if let Some(view) = &self.diagnostics {
            root = root.child(self.diagnostic_overlay(view.clone(), cx));
        }
        if let Some(view) = &self.settings {
            root = root.child(self.settings_overlay(view.clone(), cx));
        }
        root.on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
            if let Some(action) = crate::keymap::action(&event.keystroke, cx)
                && this.shortcut(action, window, cx)
            {
                cx.stop_propagation();
            }
        }))
        .on_mouse_move(cx.listener(Self::resize_drag))
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

impl Workbench {
    /// Shows a tab of the current space and puts input in its focused pane.
    pub(super) fn show_tab(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.nav.tab = Some(id.to_owned());
        if self.nav.overview || !self.overview.cleanup.is_empty() {
            self.leave_overview(cx);
        }
        self.sync_panes(cx);
        if let Some(p) = self
            .nav
            .focused
            .as_ref()
            .and_then(|id| self.layout.panes.get(id))
        {
            window.focus(&p.focus_handle(cx), cx);
        }
        cx.notify();
    }
    /// The workbench's keymap actions (crate::keymap); true when one ran. The focused
    /// terminal runs its own (查找、清屏、字号) before this sees them.
    pub(super) fn shortcut(
        &mut self,
        action: crate::keymap::Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        use crate::keymap::Action;
        let row: Vec<_> = self
            .workspace
            .tabs
            .iter()
            .filter(|t| t.space == self.nav.space)
            .map(|t| t.id.clone())
            .collect();
        let at = self
            .nav
            .tab
            .as_ref()
            .and_then(|t| row.iter().position(|r| r == t));
        let target = match action {
            Action::NewTab => {
                self.launch(false, None, cx);
                return true;
            }
            Action::Close => {
                self.close_focused(cx);
                return true;
            }
            Action::SplitRight | Action::SplitDown => {
                let pane = self.nav.tab.as_ref().and_then(|tab| {
                    let panes = self.workspace.tab_terminals(tab);
                    self.nav
                        .focused
                        .clone()
                        .filter(|id| panes.contains(id))
                        .or_else(|| panes.first().cloned())
                });
                match pane {
                    Some(id) => {
                        let label = if action == Action::SplitDown {
                            "向下分屏"
                        } else {
                            "向右分屏"
                        };
                        self.menu_action(&id, label, window, cx);
                    }
                    None => self.launch(false, None, cx),
                }
                return true;
            }
            Action::Search => {
                if self.search.is_open() {
                    self.close_search(cx);
                } else {
                    self.open_search(window, cx);
                }
                return true;
            }
            Action::Settings => {
                self.show_settings(window, cx);
                return true;
            }
            Action::NextWaiting => {
                self.next_waiting(window, cx);
                return true;
            }
            Action::ToggleSidebar => {
                self.toggle_sidebar(cx);
                return true;
            }
            Action::PrevPane | Action::NextPane => {
                self.focus_neighbour(action == Action::NextPane, window, cx);
                return true;
            }
            Action::PaneLeft | Action::PaneRight | Action::PaneUp | Action::PaneDown => {
                use keepline_core::layout::Direction;
                let to = match action {
                    Action::PaneLeft => Direction::Left,
                    Action::PaneRight => Direction::Right,
                    Action::PaneUp => Direction::Up,
                    _ => Direction::Down,
                };
                self.focus_toward(to, window, cx);
                return true;
            }
            Action::PrevTab => at.map(|i| (i + row.len() - 1) % row.len()),
            Action::NextTab => at.map(|i| (i + 1) % row.len()),
            Action::LastTab => row.len().checked_sub(1),
            Action::Tab(n) => Some(usize::from(n) - 1).filter(|i| *i < row.len()),
            _ => return false,
        };
        if let Some(id) = target.and_then(|i| row.get(i)).cloned() {
            self.show_tab(&id, window, cx);
        }
        true
    }
    /// ⌘[ / ⌘]: input moves to the previous or next pane of the tab in layout order,
    /// wrapping around, as Ghostty's goto_split does; a zoomed pane is restored first.
    fn focus_neighbour(&mut self, next: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.nav.tab.clone() else {
            return;
        };
        let panes: Vec<_> = self
            .workspace
            .tab_terminals(&tab)
            .into_iter()
            .filter(|id| self.layout.panes.contains_key(id))
            .collect();
        if panes.len() < 2 {
            return;
        }
        let at = self
            .nav
            .focused
            .as_ref()
            .and_then(|id| panes.iter().position(|p| p == id))
            .unwrap_or(0);
        let to = if next {
            (at + 1) % panes.len()
        } else {
            (at + panes.len() - 1) % panes.len()
        };
        self.focus_pane(panes[to].clone(), window, cx);
    }
    /// ⌘⌥ and an arrow: input moves to the pane on that side of the focused one, by
    /// where the panes sit; nothing happens at the tab's edge.
    fn focus_toward(
        &mut self,
        to: keepline_core::layout::Direction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.nav.tab.clone() else {
            return;
        };
        let Some(layout) = self
            .workspace
            .tabs
            .iter()
            .find(|t| t.id == tab)
            .and_then(|t| t.layout.clone())
        else {
            return;
        };
        let from = self
            .nav
            .focused
            .clone()
            .filter(|id| layout.contains(id))
            .or_else(|| layout.ids().first().cloned());
        if let Some(id) = from
            .and_then(|from| layout.neighbour(&from, to))
            .filter(|id| self.layout.panes.contains_key(id))
        {
            self.focus_pane(id, window, cx);
        }
    }
    fn focus_pane(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.nav.zoomed = None;
        self.nav.focused = Some(id.clone());
        self.sync_panes(cx);
        if let Some(pane) = self.layout.panes.get(&id) {
            window.focus(&pane.focus_handle(cx), cx);
        }
        cx.notify();
    }
    /// The top bar's count per status across every open session, grouped and hidden as
    /// the overview does; one opens the overview with that group shown.
    /// The terminal style's right end of the status line: `✕ 2  ◐ 1  ✓ 3`, each a way
    /// into 鸟瞰. Zero counts are left out.
    fn status_glyphs(&self, cx: &Context<Self>) -> gpui::Div {
        // The same sessions the top bar's 需要你 counts: those in Keepline's terminals.
        let mut counts = [self.needs_you().len(), 0, 0];
        const SHOWN: [Group; 3] = [Group::Error, Group::Working, Group::Done];
        for s in &self.workspace.agent_sessions {
            if !s.managed() || !s.outcome.needs_review(now()) || self.session_hidden(s).is_some() {
                continue;
            }
            match session_group(&self.workspace, s) {
                Group::Working | Group::Running => counts[1] += 1,
                Group::Done | Group::Review => counts[2] += 1,
                _ => {}
            }
        }
        let mut row = div()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(T::S10))
            .font_features(T::tabular());
        for (group, count) in SHOWN.into_iter().zip(counts).filter(|(_, n)| *n > 0) {
            row = row.child(
                div()
                    .id(gpui::SharedString::from(format!(
                        "status-{}",
                        group.label()
                    )))
                    .flex()
                    .gap(px(T::S4))
                    .cursor_pointer()
                    .child(glyph(group))
                    .child(count.to_string())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if group == Group::Working {
                            this.overview.show_running = true;
                        }
                        this.show_overview(cx);
                    })),
            );
        }
        row
    }
    /// 下一个等你的: the sessions that need you are waiting ones and failed ones, so the
    /// chip names and colors whichever it holds, keeping it consistent with the 等你 count.
    fn next_waiting_chip(&self, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        let needs = self.needs_you();
        let count = needs.len();
        let waiting = needs
            .iter()
            .any(|s| session_group(&self.workspace, s) == Group::Waiting);
        // One count for everything that needs you; the color says which kind leads.
        let group = if waiting {
            Group::Waiting
        } else {
            Group::Error
        };
        let text = "需要你";
        let (dot_color, ink, wash) = group.colors();
        let key = crate::keymap::current(cx)
            .chords(crate::keymap::Action::NextWaiting)
            .first()
            .map(|c| c.display());
        button("next-waiting", "")
            .aria_label(format!("{text}，共 {count} 个"))
            .h(px(T::NEXT_CHIP))
            .pl(px(T::S8))
            .pr(px(T::S10))
            .gap(px(T::S6))
            .bg(wash)
            .text_color(ink)
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .child(div().size(px(T::DOT)).rounded_full().bg(dot_color))
            .child(text)
            .child(div().font_features(T::tabular()).child(count.to_string()))
            .when_some(key, |d, key| {
                d.child(
                    div()
                        .px(px(T::S4))
                        .rounded(px(T::CHIP_RADIUS))
                        .border(px(T::BORDER_WIDTH))
                        .border_color(ink)
                        .opacity(T::KBD_OPACITY)
                        .text_size(px(T::KBD_SIZE))
                        .font_weight(gpui::FontWeight::NORMAL)
                        .child(key),
                )
            })
            .on_click(cx.listener(|this, _, window, cx| this.next_waiting(window, cx)))
    }
    /// Empty title-bar space moves the window and zooms it on a double press, as the
    /// system title bar did before the top bar replaced it.
    fn title_drag_area(&self, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        div()
            .id("title-drag")
            .flex_1()
            .h_full()
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _, _, _| this.title_drag = true),
            )
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, _, _, _| this.title_drag = false),
            )
            .on_mouse_move(cx.listener(|this, _, window, _| {
                if std::mem::take(&mut this.title_drag) {
                    window.start_window_move();
                }
            }))
            .on_click(|event, window, _| {
                if event.click_count() == 2 {
                    window.titlebar_double_click();
                }
            })
    }
}

/// `total` small cells for the acceptance criteria, `filled` of them with evidence;
/// neutral tones, since color is kept for session status.
fn evidence_cells(filled: usize, total: usize) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(T::S2))
        .children((0..total).map(|i| {
            div()
                .w(px(T::EVIDENCE_CELL_WIDTH))
                .h(px(T::EVIDENCE_CELL_HEIGHT))
                .rounded(px(T::EVIDENCE_CELL_RADIUS))
                .bg(T::color(if i < filled {
                    T::secondary()
                } else {
                    T::control()
                }))
        }))
}
/// A plain hover note.
pub(super) struct TextTip(pub(super) gpui::SharedString);
impl Render for TextTip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .max_w(px(T::TAB_TIP_WIDTH))
            .p(px(T::S8))
            .rounded(px(T::CONTROL_RADIUS))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::card_border()))
            .bg(T::color(T::raised()))
            .shadow_lg()
            .text_size(px(T::SMALL))
            .text_color(T::color(T::primary()))
            .child(self.0.clone())
    }
}
/// A tab's full name, what its agent is doing and where, shown on hover since the tab
/// itself is capped and elided.
#[derive(Clone)]
struct TabTip {
    title: String,
    status: Option<Group>,
    /// Why the tab asks for you: the error or the question, so the dot is never a riddle.
    reason: Option<String>,
    cwd: Option<String>,
}
impl Workbench {
    fn tab_tip(&self, title: &str, ids: &[String]) -> TabTip {
        let sessions: Vec<_> = self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| s.current && s.terminal.as_ref().is_some_and(|id| ids.contains(id)))
            .collect();
        let (status, reason) = sessions
            .iter()
            .map(|s| {
                (
                    s,
                    sidebar_status(&self.workspace, &self.attention.unseen, s),
                )
            })
            .max_by_key(|(_, (_, attention))| *attention)
            .map(|(s, (g, _))| (Some(g), self.attention_reason(s, g)))
            .unwrap_or_default();
        let first = ids
            .iter()
            .find(|id| Some(*id) == self.nav.focused.as_ref())
            .or(ids.first());
        let cwd = first
            .and_then(|id| self.workspace.terminals.iter().find(|t| &t.id == id))
            .map(|t| home_relative(&t.cwd));
        TabTip {
            title: title.to_owned(),
            status,
            reason,
            cwd,
        }
    }
    pub(super) fn attention_reason(&self, s: &AgentSession, g: Group) -> Option<String> {
        let content = self.workspace.content.get(&s.id);
        let text = match g {
            Group::Waiting => content.map(|c| c.waiting.clone()).unwrap_or_default(),
            Group::Error => content
                .map(|c| c.error.clone())
                .filter(|e| !e.is_empty())
                .or_else(|| {
                    let t = self
                        .workspace
                        .terminals
                        .iter()
                        .find(|t| s.terminal.as_ref() == Some(&t.id))?;
                    t.error
                        .clone()
                        .or_else(|| t.exit_code.map(|c| format!("终端退出码 {c}")))
                })
                .unwrap_or_default(),
            _ => return None,
        };
        let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
        Some(line.chars().take(T::TAB_TIP_REASON_CHARS).collect())
    }
}
impl Render for TabTip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .max_w(px(T::TAB_TIP_WIDTH))
            .p(px(T::S8))
            .flex()
            .flex_col()
            .gap(px(T::S4))
            .rounded(px(T::CONTROL_RADIUS))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::card_border()))
            .bg(T::color(T::raised()))
            .shadow_lg()
            .text_size(px(T::SMALL))
            .text_color(T::color(T::primary()))
            .child(self.title.clone())
            .when_some(self.status, |d, g| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(T::S6))
                        .child(dot(g))
                        .child(label(g.label())),
                )
            })
            .when_some(self.reason.clone(), |d, reason| {
                d.child(div().text_color(T::color(T::secondary())).child(reason))
            })
            .when_some(self.cwd.clone(), |d, cwd| d.child(label(cwd)))
    }
}

impl Workbench {
    /// One line under the tabs for a connection problem: a sentence the user can act on,
    /// the raw errors on hover, and 「重新连接」 when the host itself is unreachable.
    fn notice_strip(
        &self,
        text: &str,
        details: Vec<String>,
        reconnect: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (_, ink, bg) = Group::Waiting.colors();
        let details: Vec<String> = details.into_iter().filter(|d| !d.is_empty()).collect();
        let message = div()
            .id("connection-notice")
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(T::S8))
            .child(dot(Group::Waiting))
            .child(div().truncate().child(text.to_string()))
            .when(!details.is_empty(), |d| {
                d.tooltip(diagnostics::tip(details))
            });
        div()
            .flex()
            .items_center()
            .gap(px(T::S8))
            .px(px(T::S14))
            .py(px(T::S2))
            .bg(bg)
            .text_size(px(T::META))
            .text_color(ink)
            .child(message)
            .child(div().flex_1())
            .when(reconnect, |d| {
                let text = if self.reconnecting {
                    "连接中…"
                } else {
                    "重新连接"
                };
                d.child(
                    button("reconnect-host", text)
                        .flex_shrink_0()
                        .text_color(ink)
                        .on_click(cx.listener(|this, _, _, cx| this.reconnect(cx))),
                )
            })
    }
    /// Starts a host when none answers; the status link then resubscribes on its own.
    pub(super) fn reconnect(&mut self, cx: &mut Context<Self>) {
        if self.reconnecting {
            return;
        }
        self.reconnecting = true;
        cx.notify();
        let socket = self.store.socket().to_owned();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    if crate::lifecycle::connected_host(&socket)? {
                        return Ok(());
                    }
                    let root = socket
                        .parent()
                        .ok_or_else(|| std::io::Error::other("找不到运行目录"))?;
                    crate::lifecycle::spawn_host(root, false)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.reconnecting = false;
                if let Err(e) = result {
                    this.error = Some(format!("没能启动后台：{e}"));
                }
                cx.notify();
            });
        })
        .detach();
    }
}
