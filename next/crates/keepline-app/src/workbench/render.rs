//! The work view: top bar, sidebar, tabs, the pane grid and the footer.
use super::*;

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.nav.pending_focus
            && self.tasks.form.is_none()
            && self.layout.renaming_tab.is_none()
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
        let full_page = if !self.overview.cleanup.is_empty() && self.tasks.form.is_none() {
            Some(self.cleanup_view(cx))
        } else if self.nav.overview && self.tasks.form.is_none() {
            Some(self.overview_view(window, cx))
        } else {
            None
        };
        if let Some(view) = full_page {
            return self
                .with_search(root.relative(), cx)
                .child(
                    div()
                        .h(px(T::TOP))
                        .flex_shrink_0()
                        .flex()
                        .child(self.title_drag_area(cx)),
                )
                .child(div().flex_1().min_h_0().flex().flex_col().child(view))
                .when(self.search.is_open(), |d| d.child(self.search_palette(cx)));
        }
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
                .bg(T::color(T::PANEL))
                .border_b(px(T::BORDER_WIDTH))
                .border_color(T::color(T::BORDER))
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
                .child(
                    button("overview", "▦ 鸟瞰全部")
                        .bg(T::color(T::RAISED))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.nav.overview = true;
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
                        .text_color(T::color(T::SECONDARY))
                        .on_click(cx.listener(|this, _, _, cx| {
                            if !this.restarting {
                                this.request_close(Closing::Host, cx);
                            }
                        })),
                    )
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
        for tab in self
            .workspace
            .tabs
            .iter()
            .filter(|t| t.space == self.nav.space)
        {
            let id = tab.id.clone();
            let active = Some(&id) == self.nav.tab.as_ref();
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
                        .bg(T::color(T::RAISED))
                        .border(px(T::BORDER_WIDTH))
                        .border_color(T::color(T::FOCUS))
                        .text_size(px(T::MONO_SIZE))
                        .text_color(T::color(T::PRIMARY))
                        .when(tab.default_task.is_some(), |d| d.child(task_icon()))
                        .child(div().flex_1().min_w_0().h_full().child(field.clone())),
                );
                continue;
            }
            let title = self.tab_name(tab, cx);
            let tip = self.tab_tip(&title, &ids);
            let group_name = gpui::SharedString::from(format!("tab-group-{id}"));
            // Shown on the current tab, and on the others while hovered, like a browser.
            let close = button(gpui::SharedString::from(format!("tab-close-{id}")), "×")
                .aria_label(format!("关闭标签 {title}"))
                .flex_shrink_0()
                .h(px(T::ICON_HEIGHT))
                .px(px(T::S4))
                .text_color(T::color(T::SECONDARY))
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
                    .flex_shrink_0()
                    .max_w(px(T::TAB_MAX_WIDTH))
                    .h(px(T::TABS))
                    .gap(px(T::S6))
                    .border_0()
                    .rounded_none()
                    .text_size(px(T::MONO_SIZE))
                    .text_color(T::color(if active { T::PRIMARY } else { T::SECONDARY }))
                    .when(active, |d| d.bg(T::color(T::RAISED)))
                    .when(tab.default_task.is_some(), |d| d.child(task_icon()))
                    .child(div().min_w_0().truncate().child(title))
                    .when_some(group, |d, g| d.child(dot(g)).child(badge(g, needs.len())))
                    .child(close)
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
                            this.nav.tab = Some(id.clone());
                            this.sync_panes(cx);
                            if let Some(p) = this
                                .nav
                                .focused
                                .as_ref()
                                .and_then(|id| this.layout.panes.get(id))
                            {
                                window.focus(&p.focus_handle(cx), cx);
                            }
                            cx.notify();
                        }),
                    ),
            );
        }
        let tabs = div()
            .h(px(T::TABS))
            .flex_shrink_0()
            .flex()
            .items_center()
            .px(px(T::S6))
            .border_b(px(T::BORDER_WIDTH))
            .border_color(T::color(T::BORDER))
            .child(strip)
            .child(
                button("new", "＋")
                    .aria_label("新建终端标签")
                    .flex_shrink_0()
                    .border_0()
                    .text_color(T::color(T::LINK))
                    .on_click(cx.listener(|this, _, _, cx| this.launch(false, None, cx))),
            );
        let mut main = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(tabs);
        // Indexing in progress is a top-bar note; only a stopped recognition is worth a
        // banner, in words the user can act on. A missing official status source is not:
        // logs and screens still tell the state.
        if self.store.recognition().phase == RecognitionPhase::Interrupted {
            main = main.child(
                label(
                    "会话识别暂停了，左栏和鸟瞰里的状态可能不是最新的。重开 Keepline 通常能恢复。",
                )
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
        if let Some(form) = &self.tasks.form {
            main = main.child(self.form_panel(form, cx));
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
        if let Some(closing) = &self.layout.confirm_close {
            root = root.child(self.confirm_close_view(closing, cx));
        }
        if self.search.is_open() {
            root = root.child(self.search_palette(cx));
        }
        self.with_search(root, cx)
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                let k = &event.keystroke;
                if k.key == "w"
                    && k.modifiers.platform
                    && !k.modifiers.shift
                    && !k.modifiers.control
                    && !k.modifiers.alt
                {
                    this.close_focused(cx);
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
    /// ⌘P opens the search palette from anywhere; ⌘K stays the terminal's clear.
    fn with_search(&self, root: gpui::Div, cx: &Context<Self>) -> gpui::Div {
        root.on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
            let k = &event.keystroke;
            if k.key == "p"
                && k.modifiers.platform
                && !k.modifiers.shift
                && !k.modifiers.control
                && !k.modifiers.alt
            {
                if this.search.is_open() {
                    this.close_search(cx);
                } else {
                    this.open_search(window, cx);
                }
                cx.stop_propagation();
            }
        }))
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

/// A plain hover note.
pub(super) struct TextTip(pub(super) &'static str);
impl Render for TextTip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .max_w(px(T::TAB_TIP_WIDTH))
            .p(px(T::S8))
            .rounded(px(T::CONTROL_RADIUS))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::CARD_BORDER))
            .bg(T::color(T::RAISED))
            .shadow_lg()
            .text_size(px(T::SMALL))
            .text_color(T::color(T::PRIMARY))
            .child(self.0)
    }
}
/// A tab's full name, what its agent is doing and where, shown on hover since the tab
/// itself is capped and elided.
#[derive(Clone)]
struct TabTip {
    title: String,
    status: Option<Group>,
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
        let status = sessions
            .iter()
            .map(|s| sidebar_status(&self.workspace, &self.attention.unseen, s))
            .max_by_key(|(_, attention)| *attention)
            .map(|(g, _)| g);
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
            cwd,
        }
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
            .border_color(T::color(T::CARD_BORDER))
            .bg(T::color(T::RAISED))
            .shadow_lg()
            .text_size(px(T::SMALL))
            .text_color(T::color(T::PRIMARY))
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
            .when_some(self.cwd.clone(), |d, cwd| d.child(label(cwd)))
    }
}
