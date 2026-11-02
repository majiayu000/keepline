//! The work view: top bar, sidebar, tabs, the pane grid and the footer.
use super::*;

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Whenever the current tab changes (a click, the sidebar, a shortcut, a new tab),
        // scroll the strip so that tab is in view: the strip always shows where you are.
        if self.layout.revealed_tab != self.nav.tab {
            if let Some(ix) = self.nav.tab.as_ref().and_then(|id| {
                self.workspace
                    .tabs
                    .iter()
                    .filter(|t| t.space == self.nav.space)
                    .position(|t| &t.id == id)
            }) {
                self.layout.tab_scroll.scroll_to_item(ix);
            }
            self.layout.revealed_tab = self.nav.tab.clone();
        }
        self.open_pending_route(window, cx);
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
        // The main column's own title row, as T3 Code has it: where you are on the left,
        // what needs you and the overview on the right. The window's top edge belongs to
        // the sidebar; collapsed, its toggle stays beside the traffic lights right here.
        let collapsed = self.sidebar.collapsed;
        // Standard style puts the tabs in this row, so the current tab is not named a
        // second time; only the overview and a task's page, which no tab stands for.
        let here = if self.nav.overview || !self.overview.cleanup.is_empty() {
            Some("鸟瞰全部".to_owned())
        } else if let Some(task) = self
            .nav
            .task_detail
            .as_ref()
            .and_then(|id| self.workspace.tasks.iter().find(|t| &t.id == id))
        {
            Some(task.title.clone())
        } else if terminal_style {
            self.workspace
                .tabs
                .iter()
                .find(|t| Some(&t.id) == self.nav.tab.as_ref())
                .map(|t| self.tab_name(t, cx))
        } else {
            None
        };
        let crumb = div()
            .min_w_0()
            .flex_shrink_0()
            .max_w(px(T::TAB_MAX_WIDTH * 2.))
            .flex()
            .items_center()
            .gap(px(T::S8))
            .child(
                div()
                    .flex_shrink_0()
                    .text_color(T::color(T::muted()))
                    .child(space_name.to_owned()),
            )
            .when_some(here, |d, here| {
                d.child(
                    div()
                        .flex_shrink_0()
                        .text_color(T::color(T::muted()))
                        .child("/"),
                )
                .child(div().min_w_0().truncate().child(here))
            });
        // The right end of the title row: notes, what needs you, the overview, phones.
        let mut tail: Vec<gpui::AnyElement> = Vec::new();
        if self.store.slow() {
            tail.push(label("正在更新…").into_any_element());
        }
        if matches!(
            self.store.recognition().phase,
            RecognitionPhase::Starting | RecognitionPhase::Recovering
        ) {
            tail.push(label("正在识别会话…").into_any_element());
        }
        // The host runs older code than this app. One that can hand over is updated
        // in place; an older one must be restarted, which ends every terminal.
        if crate::lifecycle::stale_host() {
            tail.push(
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
                }))
                .into_any_element(),
            );
        }
        if !self.needs_you().is_empty() {
            tail.push(self.next_waiting_chip(cx).into_any_element());
        }
        // A toggle: the same spot goes into the overview and back out of it. Standard
        // style names it, so the way to every space's work is not a bare glyph.
        tail.push({
            let on = self.nav.overview || !self.overview.cleanup.is_empty();
            let toggle = icon_button(
                "overview",
                "icons/grid.svg",
                if on {
                    "回到工作区"
                } else {
                    "鸟瞰全部"
                },
            )
            .when(!terminal_style, |d| {
                d.w_auto()
                    .px(px(T::S8))
                    .gap(px(T::S6))
                    .text_size(px(T::SMALL))
                    .text_color(T::color(T::secondary()))
                    .child("鸟瞰")
            })
            .when(on, |d| d.bg(T::color(T::selection())))
            .tooltip(move |_, cx| {
                cx.new(|_| {
                    TextTip(
                        if on {
                            "回到工作区"
                        } else {
                            "鸟瞰全部"
                        }
                        .into(),
                    )
                })
                .into()
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                if on {
                    this.leave_overview(cx);
                } else {
                    this.show_overview(cx);
                }
            }));
            self.record_toggle(toggle, cx).into_any_element()
        });
        tail.extend(self.phones_button(cx).map(|b| b.into_any_element()));
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
            .when(!terminal_style, |d| d.gap(px(T::S2)))
            .overflow_x_scroll()
            .track_scroll(&self.layout.tab_scroll);
        // What each tab needs from you, in strip order, so a hidden one can be called out.
        let mut wants = Vec::new();
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
                            status_of(&self.workspace, s),
                            Status::Waiting | Status::Error
                        )
                })
                .collect();
            let group = needs.first().map(|s| status_of(&self.workspace, s));
            wants.push(group);
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
                    .max_w(px(T::TAB_MAX_WIDTH))
                    .h(px(if terminal_style { T::FOOTER } else { T::ROW }))
                    .gap(px(T::S6))
                    .border_0()
                    .when(terminal_style, |d| d.rounded_none())
                    .when(!terminal_style, |d| d.pl(px(T::S10)).pr(px(T::S4)))
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
                    // The current tab is a quiet filled block, like the sidebar's row.
                    .when(active && !terminal_style, |d| {
                        d.bg(T::color(T::selection()))
                            .font_weight(gpui::FontWeight::MEDIUM)
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
        let (left, right) = self.tab_overflow(&wants, cx);
        // The left marker floats over the strip so its coming and going never shifts
        // the tabs a click on either marker has just brought into view.
        let fade = right.is_some() && !terminal_style;
        let strip = div()
            .relative()
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .child(strip.flex_1())
            // Tabs past the right edge fade out instead of being cut mid-character.
            .when(fade, |d| {
                d.child(
                    div()
                        .absolute()
                        .right_0()
                        .top_0()
                        .h_full()
                        .w(px(T::S20 * 2.))
                        .bg(gpui::linear_gradient(
                            90.,
                            gpui::linear_color_stop(T::color(T::pane()).opacity(0.), 0.),
                            gpui::linear_color_stop(T::color(T::pane()), 1.),
                        )),
                )
            })
            .when_some(left, |d, hint| {
                d.child(
                    self.tab_edge(hint, false, terminal_style, cx)
                        .absolute()
                        .left_0()
                        .top_0(),
                )
            });
        let right = right.map(|hint| self.tab_edge(hint, true, terminal_style, cx));
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
                .children(right)
                .child(new_tab)
                .child(self.status_glyphs(cx))
        } else {
            // Standard: the tabs share the title row, T3 Code style. The strip keeps its
            // own width so the space after 「＋」 still moves the window.
            div()
                .h_full()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(T::S2))
                .child(strip)
                .children(right)
                .child(new_tab)
        };
        let (row_tabs, bottom_tabs) = if terminal_style {
            (None, Some(tabs))
        } else {
            (Some(tabs), None)
        };
        let header = div()
            .h(px(T::TOP))
            .flex_shrink_0()
            .pl(px(if collapsed {
                T::TRAFFIC_LIGHT_INSET
            } else {
                T::S16
            }))
            .pr(px(T::S14))
            .flex()
            .items_center()
            .gap(px(T::S10))
            .border_b(px(T::BORDER_WIDTH))
            .border_color(T::color(T::border()))
            .when(collapsed, |d| d.child(self.sidebar_toggle(cx)))
            .child(crumb)
            .when_some(row_tabs, |d, tabs| {
                d.child(
                    div()
                        .flex_shrink_0()
                        .w(px(T::BORDER_WIDTH))
                        .h(px(T::S16))
                        .bg(T::color(T::border())),
                )
                .child(tabs)
                .child(self.title_drag_area(cx).min_w(px(T::S20 * 2.)))
            })
            .when(terminal_style, |d| d.child(self.title_drag_area(cx)))
            .children(tail);
        let mut main = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            // One white surface under the title row and the panes; the sidebar's gray
            // beside it is the only other layer.
            .when(!terminal_style, |d| d.bg(T::color(T::pane())))
            .child(header);
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
                    .text_color(Tone::Danger.ink()),
            );
        }
        if let Some(form) = &self.tasks.form {
            main = main.child(self.form_panel(form, window, cx));
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
                        // The board's drawer walks its column with j/k or the arrows.
                        if this.overview.drawer.is_some() && !k.modifiers.modified() {
                            let by = match k.key.as_str() {
                                "j" | "down" => 1,
                                "k" | "up" => -1,
                                _ => 0,
                            };
                            if by != 0 {
                                this.step_drawer(by, cx);
                                cx.stop_propagation();
                                return;
                            }
                        }
                        if k.key != "escape" || k.modifiers.modified() {
                            return;
                        }
                        // A card's menu closes first, then the drawer; 逐个清理 steps back
                        // to the overview.
                        if this.overview.card_menu.take().is_some()
                            || this.overview.drawer.take().is_some()
                        {
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
                let evidence = task.proven();
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
                grid = grid.child(self.empty_tab(cx));
            }
            main = main.child(grid);
        }
        main = main.children(bottom_tabs);
        if let Some(peek) = &self.peek {
            main = main.relative().child(self.peek_view(peek, cx));
        }
        root = root
            .relative()
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .children(sidebar)
                    .child(main),
            )
            .child(self.statusbar_view(cx));
        // 标记完成 runs leave their panes for the sidebar, so they draw over everything
        // but the menus.
        if let Some(overlay) = self.completing_overlay(window) {
            root = root.child(overlay);
        }
        if let Some(menu) = &self.layout.menu {
            root = root.child(self.menu_view(menu, window, cx));
        }
        if let Some(position) = self.sidebar.view_menu {
            root = root.child(self.view_menu_view(position, window, cx));
        }
        if self.phones.open {
            root = root.child(self.phones_panel(window, cx));
        }
        root = root.children(self.statusbar_popover(cx));
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
    /// Tabs hidden past each edge of the strip. Read from the last layout, so one more
    /// frame is drawn whenever the new layout moved tabs across an edge.
    fn tab_overflow(
        &self,
        wants: &[Option<Status>],
        cx: &mut Context<Self>,
    ) -> (Option<TabEdge>, Option<TabEdge>) {
        let (left, right) = hidden_tabs(&self.layout.tab_scroll, wants.len());
        let shown = (left.len(), right.len());
        let scroll = self.layout.tab_scroll.clone();
        let count = wants.len();
        // The display link sleeps while idle, so a next-frame callback may never come;
        // a short timer checks the fresh layout instead.
        cx.spawn(async move |view, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(16))
                .await;
            let (left, right) = hidden_tabs(&scroll, count);
            if (left.len(), right.len()) != shown {
                view.update(cx, |_, cx| cx.notify()).ok();
            }
        })
        .detach();
        let edge = |mut hidden: Vec<usize>, nearest_last: bool| {
            if nearest_last {
                hidden.reverse();
            }
            let nearest = *hidden.first()?;
            let urgent = hidden.iter().copied().find(|&ix| wants[ix].is_some());
            Some(TabEdge {
                count: hidden.len(),
                reveal: urgent.unwrap_or(nearest),
                group: urgent.and_then(|ix| wants[ix]),
            })
        };
        (edge(left, true), edge(right, false))
    }
    /// The marker at one edge of the strip: how many tabs are out of sight that way,
    /// tinted when one of them is waiting on you. A click brings that tab into view.
    fn tab_edge(
        &self,
        edge: TabEdge,
        right: bool,
        terminal_style: bool,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let text = match (right, terminal_style) {
            (true, true) => format!("还有 {} 个 ›", edge.count),
            (false, true) => format!("‹ {} 个", edge.count),
            (true, false) => format!("+{}", edge.count),
            (false, false) => format!("‹ {}", edge.count),
        };
        let reveal = edge.reveal;
        button(
            if right {
                "tabs-more-right"
            } else {
                "tabs-more-left"
            },
            "",
        )
        .aria_label(if right {
            format!("右侧还有 {} 个标签", edge.count)
        } else {
            format!("左侧还有 {} 个标签", edge.count)
        })
        .flex_shrink_0()
        .h_full()
        .px(px(T::S8))
        .gap(px(T::S4))
        // Terminal style: a full-height block at the strip's edge. Standard: a small
        // count beside the faded edge, never covering a tab's name.
        .when(terminal_style, |d| {
            d.rounded_none()
                .when(right, |d| d.border_l(px(T::BORDER_WIDTH)))
                .when(!right, |d| d.border_r(px(T::BORDER_WIDTH)))
                .border_color(T::color(T::border()))
        })
        .when(!terminal_style, |d| d.font_features(T::tabular()))
        .bg(match edge.group {
            Some(g) => g.colors().2,
            None if terminal_style => T::color(T::raised()),
            None => T::color(T::pane()),
        })
        .text_size(px(T::MONO_SIZE))
        .text_color(match edge.group {
            Some(g) => g.colors().1,
            None => T::color(T::primary()),
        })
        .when_some(edge.group, |d, g| d.child(dot(g)))
        .child(text)
        .on_click(cx.listener(move |this, _, _, cx| {
            this.reveal_tab(reveal, right);
            cx.notify();
        }))
    }
    /// Scrolls the strip just far enough to show one tab whole at the given edge.
    pub(super) fn reveal_tab(&self, ix: usize, right: bool) {
        let scroll = &self.layout.tab_scroll;
        let (view, max) = (scroll.bounds(), scroll.max_offset().x);
        let Some(tab) = scroll.bounds_for_item(ix) else {
            return;
        };
        // Set directly rather than through scroll_to_item, so the next redraw already
        // counts the tabs from the new position.
        let x = if right {
            view.right() - tab.right()
        } else {
            view.left() - tab.left()
        };
        let y = scroll.offset().y;
        scroll.set_offset(gpui::point(x.clamp(-max, px(0.)), y));
    }
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
        const SHOWN: [Status; 3] = [Status::Error, Status::Running, Status::Review];
        for s in &self.workspace.agent_sessions {
            if !s.managed() || !s.outcome.needs_review(now()) || self.session_hidden(s).is_some() {
                continue;
            }
            match status_of(&self.workspace, s) {
                g if g.working() => counts[1] += 1,
                Status::Review => counts[2] += 1,
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
                        if group == Status::Running {
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
            .any(|s| status_of(&self.workspace, s) == Status::Waiting);
        // One count for everything that needs you; the color says which kind leads.
        let group = if waiting {
            Status::Waiting
        } else {
            Status::Error
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
    /// The sidebar button beside the traffic lights: on the sidebar's top row while it is
    /// open, on the main title row once it is folded away, so it never moves.
    pub(super) fn sidebar_toggle(&self, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        // T3 Code's mark: the panel with a chevron saying which way a click moves it.
        icon_button(
            "sidebar-toggle",
            if self.sidebar.collapsed {
                "icons/sidebar-open.svg"
            } else {
                "icons/sidebar.svg"
            },
            if self.sidebar.collapsed {
                "展开左栏"
            } else {
                "收起左栏"
            },
        )
        .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx)))
    }
    /// Empty title-bar space moves the window and zooms it on a double press, as the
    /// system title bar did before the top bar replaced it.
    pub(super) fn title_drag_area(&self, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
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
pub(super) fn evidence_cells(filled: usize, total: usize) -> gpui::Div {
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
    status: Option<Status>,
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
                    session_status(&self.workspace, &self.attention.unseen, s),
                )
            })
            .max_by_key(|(_, (_, attention))| *attention)
            .map(|(s, (g, _))| {
                let line = self.attention_text(s, g).and_then(|t| {
                    let l = t.lines().map(str::trim).find(|l| !l.is_empty())?;
                    Some(l.chars().take(T::TAB_TIP_REASON_CHARS).collect::<String>())
                });
                (Some(g), line)
            })
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
    /// Why the session needs you, as a short phrase from the reasons rules (`批准
    /// WebSearch`, `要重新登录`, a question's title). `None` when no rule knows the text or
    /// the rule has nothing to add: the state word alone says it, the full text is in
    /// [`Self::attention_text`].
    pub(super) fn attention_reason(&self, s: &AgentSession, g: Status) -> Option<String> {
        use keepline_core::reason::{Kind, phrase};
        let kind = match g {
            Status::Waiting => Kind::Waiting,
            Status::Error => Kind::Error,
            _ => return None,
        };
        let text = self.attention_text(s, g)?;
        Some(phrase(kind, &text).text).filter(|p| !p.is_empty())
    }
    /// The whole text behind 等你回答 / 出错了: the question or approval, the error.
    pub(super) fn attention_text(&self, s: &AgentSession, g: Status) -> Option<String> {
        let content = self.workspace.content.get(&s.id);
        let text = match g {
            Status::Waiting => content.map(|c| c.waiting.clone()).unwrap_or_default(),
            Status::Error => content
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
        Some(text).filter(|t| !t.trim().is_empty())
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
        let (_, ink, bg) = Tone::Warn.colors();
        let details: Vec<String> = details.into_iter().filter(|d| !d.is_empty()).collect();
        let message = div()
            .id("connection-notice")
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(T::S8))
            .child(
                div()
                    .size(px(T::DOT))
                    .flex_shrink_0()
                    .rounded_full()
                    .bg(Tone::Warn.dot()),
            )
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

impl Workbench {
    /// A new terminal in the shown space with `agent` started in it.
    pub(super) fn run_agent_here(&mut self, agent: Agent, cx: &mut Context<Self>) {
        self.tasks.last_agent = Some(agent);
        self.tasks.chosen_agent = Some(agent);
        self.launch(false, None, cx);
    }
    /// The Agent an empty tab offers first: the last one picked, else the default.
    pub(super) fn first_agent(&self) -> Agent {
        self.tasks
            .last_agent
            .unwrap_or(self.workspace.settings.default_agent)
    }
    /// A tab with no terminal yet, a new space's first view: where work starts, one solid
    /// button for the Agent used last, the other Agents and a bare shell as quiet links.
    fn empty_tab(&self, cx: &Context<Self>) -> gpui::Div {
        let space = self
            .workspace
            .spaces
            .iter()
            .find(|s| s.id == self.nav.space);
        let first = self.first_agent();
        let link = |id: String, text: String| {
            button(gpui::SharedString::from(id), text)
                .border_0()
                .px(px(T::S4))
                .text_color(T::color(T::link()))
        };
        let mut others = vec![];
        for agent in Agent::all().into_iter().filter(|a| *a != first) {
            others.push(
                link(
                    format!("empty-tab-{}", agent.executable()),
                    agent.name().to_owned(),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.run_agent_here(agent, cx)))
                .into_any_element(),
            );
        }
        others.push(
            link("empty-tab-new".into(), "空终端".into())
                .on_click(cx.listener(|this, _, _, cx| this.launch(false, None, cx)))
                .into_any_element(),
        );
        let split = crate::keymap::current(cx)
            .chords(crate::keymap::Action::SplitRight)
            .first()
            .map(|c| c.display());
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(T::S12))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(T::S4))
                    .child(
                        div()
                            .text_size(px(T::CARD_TITLE))
                            .text_color(T::color(T::primary()))
                            .child(match space {
                                Some(s) => format!("在 {} 里开始", s.name),
                                None => "在这里开始".to_owned(),
                            }),
                    )
                    .when_some(space, |d, s| d.child(label(super::settings::tilde(&s.cwd)))),
            )
            .child(
                super::overview::solid(button(
                    gpui::SharedString::from(format!("empty-tab-{}", first.executable())),
                    format!("运行 {}", first.name()),
                ))
                .px(px(T::S16))
                .on_click(cx.listener(move |this, _, _, cx| this.run_agent_here(first, cx))),
            )
            .child(
                div()
                    .max_w(px(T::HERE_WIDTH * 1.5))
                    .flex()
                    .flex_wrap()
                    .justify_center()
                    .items_center()
                    .gap(px(T::S4))
                    .text_size(px(T::SMALL))
                    .children(others.into_iter().enumerate().flat_map(|(i, b)| {
                        (i > 0)
                            .then(|| label("·").into_any_element())
                            .into_iter()
                            .chain(std::iter::once(b))
                    })),
            )
            .when_some(split, |d, keys| {
                d.child(label(format!("打开后可以右键分屏，或按 {keys} 向右分屏")))
            })
    }
}

/// Tabs scrolled out of sight past one edge of the strip.
pub(super) struct TabEdge {
    count: usize,
    /// The tab a click brings into view: the nearest one waiting on you, else the nearest.
    reveal: usize,
    group: Option<Status>,
}

/// Indexes of the tabs wholly or partly past the left and right edges of the strip,
/// from its last layout. A tab cut off at the edge counts as hidden.
pub(super) fn hidden_tabs(scroll: &gpui::ScrollHandle, count: usize) -> (Vec<usize>, Vec<usize>) {
    let view = scroll.bounds();
    let shift = scroll.offset().x;
    let slack = px(T::BORDER_WIDTH);
    let (mut left, mut right) = (Vec::new(), Vec::new());
    for ix in 0..count {
        let Some(tab) = scroll.bounds_for_item(ix) else {
            break;
        };
        if tab.left() + shift < view.left() - slack {
            left.push(ix);
        } else if tab.right() + shift > view.right() + slack {
            right.push(ix);
        }
    }
    (left, right)
}
