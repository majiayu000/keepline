//! Panes, their split layout, resizing and pane menus.
use super::*;

impl Workbench {
    pub(super) fn pane_view(
        &self,
        id: &str,
        borders: bool,
        window: &Window,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        if interrupted(&self.workspace, id) {
            let reopen = id.to_owned();
            let close = id.to_owned();
            return div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(T::S12))
                .p(px(T::S20))
                .rounded(px(T::PANE_RADIUS))
                .border(px(if borders { T::BORDER_WIDTH } else { 0. }))
                .border_color(T::color(T::BORDER))
                .bg(T::color(T::PANE))
                .child(label("已中断（后台重启，原进程已结束）"))
                .child(
                    button(
                        gpui::SharedString::from(format!("reopen-{id}")),
                        "在这里重新开一个终端",
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.launch_in_slot(true, None, Some(reopen.clone()), cx)
                    })),
                )
                .child(
                    button(
                        gpui::SharedString::from(format!("close-interrupted-{id}")),
                        "关闭",
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.close_pane(&close, cx))),
                )
                .into_any_element();
        }
        let pane = self.panes.get(id).unwrap();
        let focused = pane.focus_handle(cx).is_focused(window);
        let session = self.workspace.current_agent(id).or_else(|| {
            self.workspace
                .agent_sessions
                .iter()
                .filter(|s| s.terminal.as_deref() == Some(id))
                .max_by_key(|s| s.updated)
        });
        let group = session.map(|s| self.group(s));
        let title = session
            .map(|s| self.session_line(s))
            .unwrap_or_else(|| "空终端".into());
        let hint = self.recognition.terminal_hints.get(id).cloned();
        let takeover = id.to_owned();
        let focus = pane.focus_handle(cx);
        let focus_id = id.to_owned();
        let menu_id = id.to_owned();
        let more_id = id.to_owned();
        let close_id = id.to_owned();
        let split_id = id.to_owned();
        let other_owner = pane.read(cx).has_other_owner();
        div()
            .id(gpui::SharedString::from(format!("pane-{id}")))
            .relative()
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.focused = Some(focus_id.clone());
                    window.focus(&focus, cx);
                    cx.notify();
                }),
            )
            .on_mouse_down(
                gpui::MouseButton::Right,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    this.menu = Some(PaneMenu {
                        terminal: menu_id.clone(),
                        position: event.position,
                        page: MenuPage::Main,
                    });
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(T::PANE_RADIUS))
            .border(px(if borders { T::BORDER_WIDTH } else { 0. }))
            .border_color(T::color(if focused { T::FOCUS } else { T::BORDER }))
            .bg(T::color(T::PANE))
            .child(
                div()
                    .h(px(T::PANE_HEADER))
                    .flex_shrink_0()
                    .px(px(T::S10))
                    .flex()
                    .items_center()
                    .gap(px(T::S8))
                    .bg(T::color(if focused {
                        T::HEADER_FOCUSED
                    } else {
                        T::HEADER
                    }))
                    .border_b(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::BORDER))
                    .when_some(group, |d, g| d.child(chip(g)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(T::MONO_SIZE))
                            .child(hint.clone().unwrap_or(title)),
                    )
                    .when(hint.is_none(), |d| {
                        d.child(label(
                            session
                                .map(|s| {
                                    format!(
                                        "{} · {}",
                                        s.runtime.executable(),
                                        age(self
                                            .workspace
                                            .content
                                            .get(&s.id)
                                            .map_or(s.updated, |c| c.updated))
                                    )
                                })
                                .unwrap_or_else(|| "Shell".into()),
                        ))
                    })
                    .when(other_owner, |d| {
                        d.child(
                            button(
                                gpui::SharedString::from(format!("takeover-{id}")),
                                "接管输入",
                            )
                            .h(px(T::ICON_HEIGHT))
                            .px(px(T::S8))
                            .text_size(px(T::META))
                            .text_color(T::color(T::LINK))
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    let pane =
                                        cx.new(|cx| Pane::new(&this.socket, &takeover, true, cx));
                                    window.focus(&pane.focus_handle(cx), cx);
                                    cx.observe(&pane, |_, _, cx| cx.notify()).detach();
                                    this.panes.insert(takeover.clone(), pane);
                                    cx.notify();
                                },
                            )),
                        )
                    })
                    .when_some(
                        session
                            .filter(|_| pane.read(cx).handoff_state().is_none())
                            .and_then(|_| self.workspace.terminal_tab(id))
                            .and_then(|t| t.default_task.as_ref())
                            .and_then(|tid| self.workspace.tasks.iter().find(|t| &t.id == tid))
                            .map(|t| t.prompt()),
                        |d, text| {
                            let target = id.to_owned();
                            d.child(
                                button(
                                    gpui::SharedString::from(format!("place-task-{id}")),
                                    "放入待办内容",
                                )
                                .h(px(T::ICON_HEIGHT))
                                .px(px(T::S6))
                                .text_size(px(T::META))
                                .text_color(T::color(T::LINK))
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        if let Some(p) = this.panes.get(&target) {
                                            p.update(cx, |p, cx| p.place_text(text.clone(), cx));
                                        }
                                    },
                                )),
                            )
                        },
                    )
                    .when_some(session, |d, s| {
                        let show = s.id.clone();
                        d.child(
                            button(
                                gpui::SharedString::from(format!("here-pane-{id}")),
                                "你在这里",
                            )
                            .h(px(T::ICON_HEIGHT))
                            .px(px(T::S6))
                            .text_color(T::color(T::LINK))
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.here = Some(show.clone());
                                    cx.notify();
                                },
                            )),
                        )
                    })
                    .child(
                        button(gpui::SharedString::from(format!("split-pane-{id}")), "◫")
                            .aria_label("分屏")
                            .w(px(T::ICON_WIDTH))
                            .h(px(T::ICON_HEIGHT))
                            .px(px(T::S2))
                            .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                                this.menu = Some(PaneMenu {
                                    terminal: split_id.clone(),
                                    position: event.position(),
                                    page: MenuPage::Main,
                                });
                                cx.notify();
                            })),
                    )
                    .child(
                        button(gpui::SharedString::from(format!("menu-{id}")), "⋯")
                            .aria_label("画面菜单")
                            .h(px(T::ICON_HEIGHT))
                            .px(px(T::S6))
                            .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                                this.menu = Some(PaneMenu {
                                    terminal: more_id.clone(),
                                    position: event.position(),
                                    page: MenuPage::Main,
                                });
                                cx.notify();
                            })),
                    )
                    .child(
                        button(gpui::SharedString::from(format!("close-pane-{id}")), "×")
                            .aria_label("关闭画面")
                            .h(px(T::ICON_HEIGHT))
                            .px(px(T::S6))
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.close_pane(&close_id, cx)),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .px(px(T::S14))
                    .py(px(T::S10))
                    .when_some(
                        pane.read(cx)
                            .handoff_state()
                            .filter(|_| self.workspace.pending_tasks.get(id) != Some(&None))
                            .zip(
                                self.workspace
                                    .terminal_tab(id)
                                    .and_then(|t| t.default_task.as_ref())
                                    .and_then(|tid| {
                                        self.workspace.tasks.iter().find(|t| &t.id == tid)
                                    }),
                            ),
                        |d, (state, task)| d.child(self.handoff_bar(id, task, state, cx)),
                    )
                    .child(pane.clone().cached(gpui::StyleRefinement {
                        size: gpui::SizeRefinement {
                            width: Some(gpui::relative(1.).into()),
                            height: Some(gpui::relative(1.).into()),
                        },
                        ..Default::default()
                    })),
            )
            .when_some(
                session.filter(|s| self.here.as_ref() == Some(&s.id)),
                |d, s| {
                    d.child(
                        div()
                            .absolute()
                            .top(px(T::PANE_HEADER + T::S10))
                            .right(px(T::S10))
                            .left(px(T::S10))
                            .bottom(px(T::S10))
                            .flex()
                            .justify_end()
                            .items_start()
                            .overflow_hidden()
                            .child(self.here_card(s, cx).max_h_full()),
                    )
                },
            )
            .into_any_element()
    }
    pub(super) fn layout_view(
        &self,
        tree: &Layout,
        path: Vec<bool>,
        borders: bool,
        window: &Window,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        match tree {
            Layout::Leaf(id) => self.pane_view(id, borders, window, cx),
            Layout::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let down = *axis == Axis::Down;
                let mut left = path.clone();
                left.push(false);
                let mut right = path.clone();
                right.push(true);
                let drag_path = path.clone();
                let entity = cx.entity();
                let bounds_path = path.clone();
                div()
                    .size_full()
                    .min_w_0()
                    .min_h_0()
                    .relative()
                    .flex()
                    .when(down, |d| d.flex_col())
                    .child(
                        gpui::canvas(
                            |_, _, _| (),
                            move |bounds, _, _, cx| {
                                entity.update(cx, |this, _| {
                                    this.split_bounds.insert(bounds_path.clone(), bounds);
                                });
                            },
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .min_h_0()
                            .flex_basis(px(0.))
                            .flex_grow(*ratio as f32)
                            .when(down, |d| d.w_full())
                            .when(!down, |d| d.h_full())
                            .child(self.layout_view(first, left, true, window, cx)),
                    )
                    .child(
                        div()
                            .id(gpui::SharedString::from(format!("divider-{path:?}")))
                            .flex_shrink_0()
                            .when(down, |d| {
                                d.h(px(T::S6))
                                    .w_full()
                                    .cursor(gpui::CursorStyle::ResizeUpDown)
                            })
                            .when(!down, |d| {
                                d.w(px(T::S6))
                                    .h_full()
                                    .cursor(gpui::CursorStyle::ResizeLeftRight)
                            })
                            .bg(T::color(T::BORDER))
                            .hover(|d| d.bg(T::color(T::FOCUS)))
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    if !this.busy {
                                        this.drag = Some(drag_path.clone());
                                        this.menu = None;
                                    }
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .min_h_0()
                            .flex_basis(px(0.))
                            .flex_grow((10000 - *ratio) as f32)
                            .when(down, |d| d.w_full())
                            .when(!down, |d| d.h_full())
                            .child(self.layout_view(second, right, true, window, cx)),
                    )
                    .into_any_element()
            }
        }
    }
    pub(super) fn resize_drag(
        &mut self,
        event: &gpui::MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = &self.drag else { return };
        let Some(bounds) = self.split_bounds.get(path) else {
            return;
        };
        let Some(Layout::Split { axis, ratio, .. }) = self
            .workspace
            .tabs
            .iter_mut()
            .find(|t| Some(&t.id) == self.tab.as_ref())
            .and_then(|t| t.layout.as_mut())
            .and_then(|l| l.at_mut(path))
        else {
            return;
        };
        let fraction = if *axis == Axis::Right {
            (event.position.x - bounds.left()) / bounds.size.width
        } else {
            (event.position.y - bounds.top()) / bounds.size.height
        };
        *ratio = (fraction.clamp(0.05, 0.95) * 10000.).round() as u16;
        cx.notify();
    }
    pub(super) fn finish_drag(&mut self, cx: &mut Context<Self>) {
        if self.drag.take().is_none() {
            return;
        }
        if let Some(tab) = self
            .workspace
            .tabs
            .iter()
            .find(|t| Some(&t.id) == self.tab.as_ref())
            .cloned()
        {
            self.edit(Edit::PutTab { tab }, cx);
        }
    }
    pub(super) fn menu_action(
        &mut self,
        id: &str,
        action: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let menu = self.menu.take();
        match action {
            "向右分屏" | "向下分屏" => {
                self.zoomed = None;
                self.split_target = Some((
                    id.into(),
                    if action == "向右分屏" {
                        Axis::Right
                    } else {
                        Axis::Down
                    },
                ));
                self.launch(true, None, cx);
            }
            "放大" | "还原" => {
                self.zoomed = if self.zoomed.as_deref() == Some(id) {
                    None
                } else {
                    Some(id.into())
                };
                self.sync_panes(cx);
                if let Some(pane) = self.panes.get(id) {
                    window.focus(&pane.focus_handle(cx), cx);
                }
            }
            "关闭画面" => self.close_pane(id, cx),
            "复制" => {
                if let Some(pane) = self.panes.get(id) {
                    pane.update(cx, |p, cx| p.copy(cx));
                }
            }
            "粘贴" => {
                if let Some(pane) = self.panes.get(id) {
                    pane.update(cx, |p, cx| p.paste(cx));
                    window.focus(&pane.focus_handle(cx), cx);
                }
            }
            "搜索滚动内容" => {
                if let Some(pane) = self.panes.get(id) {
                    pane.update(cx, |p, cx| p.open_search(window, cx));
                }
            }
            "中断（Ctrl-C）" => {
                if let Some(pane) = self.panes.get(id) {
                    pane.update(cx, |p, cx| p.interrupt(cx));
                }
            }
            "停止 Agent" => self.request(Request::StopAgent { id: id.into() }, cx),
            "取消关联" => {
                if let Some(session) = self.workspace.current_agent(id) {
                    self.edit(
                        Edit::AssignTask {
                            id: session.id.clone(),
                            task: None,
                        },
                        cx,
                    );
                }
            }
            "挂到待办…" | "为这个待办开一个会话" => {
                if let Some(mut menu) = menu {
                    menu.page = if action == "挂到待办…" {
                        MenuPage::Assign
                    } else {
                        MenuPage::Sessions
                    };
                    self.menu = Some(menu);
                }
            }
            _ => (),
        }
        cx.notify();
    }
    pub(super) fn menu_view(
        &self,
        menu: &PaneMenu,
        window: &Window,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let session = self.workspace.current_agent(&menu.terminal);
        let task = self
            .workspace
            .terminal_tab(&menu.terminal)
            .and_then(|t| t.default_task.clone());
        let mut entries = Vec::new();
        match menu.page {
            MenuPage::Main => {
                for title in [
                    "向右分屏",
                    "向下分屏",
                    if self.zoomed.as_deref() == Some(&menu.terminal) {
                        "还原"
                    } else {
                        "放大"
                    },
                    "为这个待办开一个会话",
                    "复制",
                    "粘贴",
                    "搜索滚动内容",
                    "挂到待办…",
                    "取消关联",
                    "中断（Ctrl-C）",
                    "停止 Agent",
                    "关闭画面",
                ] {
                    let enabled = match title {
                        "为这个待办开一个会话" => task.is_some(),
                        "挂到待办…" | "停止 Agent" => session.is_some(),
                        "取消关联" => session.is_some_and(|s| s.task.is_some()),
                        _ => true,
                    };
                    let terminal = menu.terminal.clone();
                    let action = title.to_owned();
                    entries.push(
                        button(gpui::SharedString::from(format!("menu-{title}")), title)
                            .w_full()
                            .justify_start()
                            .border_0()
                            .when(!enabled, |d| d.opacity(0.4))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if enabled {
                                    this.menu_action(&terminal, &action, window, cx);
                                }
                            }))
                            .into_any_element(),
                    );
                }
            }
            MenuPage::Sessions => {
                for agent in [Agent::Codex, Agent::Claude] {
                    let terminal = menu.terminal.clone();
                    let task = task.clone();
                    entries.push(
                        button(
                            gpui::SharedString::from(format!("new-{}", agent.executable())),
                            agent_name(agent),
                        )
                        .w_full()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.menu = None;
                            this.chosen_agent = Some(agent);
                            this.split_target = Some((terminal.clone(), Axis::Right));
                            this.launch(true, task.clone(), cx);
                        }))
                        .into_any_element(),
                    );
                }
            }
            MenuPage::Assign => {
                if let Some(session) = session {
                    for task in &self.workspace.tasks {
                        let sid = session.id.clone();
                        let tid = task.id.clone();
                        entries.push(
                            button(
                                gpui::SharedString::from(format!("assign-{tid}")),
                                task.title.clone(),
                            )
                            .w_full()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.menu = None;
                                this.edit(
                                    Edit::AssignTask {
                                        id: sid.clone(),
                                        task: Some(tid.clone()),
                                    },
                                    cx,
                                );
                            }))
                            .into_any_element(),
                        );
                    }
                }
            }
        }
        let height = px(T::ROW * entries.len() as f32 + T::S12);
        let viewport = window.viewport_size();
        div()
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.menu = None;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("pane-menu")
                    .absolute()
                    .left(
                        menu.position
                            .x
                            .min(viewport.width - px(T::MENU_WIDTH))
                            .max(px(0.)),
                    )
                    .top(menu.position.y.min(viewport.height - height).max(px(0.)))
                    .w(px(T::MENU_WIDTH))
                    .p(px(T::S6))
                    .bg(T::color(T::RAISED))
                    .border(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::CONTROL))
                    .rounded(px(T::CONTROL_RADIUS))
                    .shadow_lg()
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .children(entries),
            )
            .into_any_element()
    }
}
