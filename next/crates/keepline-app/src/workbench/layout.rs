//! Panes, their split layout, resizing and pane menus.
use super::*;

/// The visible panes and what is open over them: menus, drags and pending closes.
pub(super) struct LayoutState {
    pub(super) panes: HashMap<String, Entity<Pane>>,
    pub(super) menu: Option<PaneMenu>,
    pub(super) split_target: Option<(String, Axis)>,
    pub(super) drag: Option<Vec<bool>>,
    pub(super) split_bounds: HashMap<Vec<bool>, gpui::Bounds<gpui::Pixels>>,
    /// A tab being renamed in place after a double-click.
    pub(super) renaming_tab: Option<(String, Entity<Field>, [gpui::Subscription; 2])>,
    /// A tab's right-click menu: the tab and where it was opened.
    pub(super) tab_menu: Option<(String, gpui::Point<gpui::Pixels>)>,
    /// The title-bar logo's menu for choosing the app icon, where it was opened.
    pub(super) icon_menu: Option<gpui::Point<gpui::Pixels>>,
    /// A close that would stop a running Agent, waiting for the user to confirm it.
    pub(super) confirm_close: Option<Closing>,
    /// Interrupted terminals already reopened in place once; a failed try keeps the button.
    pub(super) reopened: HashSet<String>,
}
/// The host's note on an Agent it sees but cannot read, in the user's words, and why.
fn unread_agent(hint: &str) -> (String, &'static str) {
    if hint == keepline_core::protocol::CLAUDE_TRANSCRIPT_OFF {
        return (
            "claude · 没有保存记录".into(),
            "这个 Claude 继承了 CLAUDE_CODE_CHILD_SESSION，自认为是别的会话的子会话，不写对话记录，Keepline 读不到也统计不了。关掉这个终端重新打开，再运行 claude。",
        );
    }
    let why = "这个 Codex 连着共用的后台，Keepline 只看得到它忙不忙，读不到对话。想在这里看到对话内容，下次用 codex --no-daemon 启动。";
    match hint
        .strip_prefix("仅显示状态：")
        .and_then(|rest| rest.split_once('；'))
    {
        Some((state, _)) => (format!("codex · {state} · 看不到对话"), why),
        None => ("codex · 看不到对话".into(), why),
    }
}
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
        // Panes attach in sync_panes; a leaf drawn before that (a tab switch whose
        // reply has not arrived) stays blank for a frame instead of taking the app down.
        let Some(pane) = self.layout.panes.get(id) else {
            return div().size_full().into_any_element();
        };
        let focused = pane.focus_handle(cx).is_focused(window);
        let session = self.workspace.current_agent(id).or_else(|| {
            self.workspace
                .agent_sessions
                .iter()
                .filter(|s| s.terminal.as_deref() == Some(id))
                .max_by_key(|s| s.updated)
        });
        let group = session.map(|s| session_group(&self.workspace, s));
        // The conversation's title names the pane, like its tab; its latest reply is
        // already on screen below, so that line only shows on hover. Without an Agent,
        // the title the program set.
        let title = session
            .map(|s| self.session_title(s))
            .or_else(|| pane.read(cx).program_title())
            .unwrap_or_else(|| "空终端".into());
        let rang = pane.read(cx).bell;
        let hint = self.store.recognition().terminal_hints.get(id).cloned();
        let takeover = id.to_owned();
        let focus = pane.focus_handle(cx);
        let focus_id = id.to_owned();
        let menu_id = id.to_owned();
        let close_id = id.to_owned();
        let other_owner = pane.read(cx).has_other_owner();
        let card_id = session.map(|s| s.id.clone());
        div()
            .id(gpui::SharedString::from(format!("pane-{id}")))
            .relative()
            // Typing into the terminal is how you carry on; its card steps out of the way.
            // Captured on the way down, so the keystroke still reaches the terminal.
            .capture_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                if !event.keystroke.modifiers.platform
                    && card_id.is_some()
                    && this.overview.here == card_id
                {
                    this.overview.here = None;
                    cx.notify();
                }
            }))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.nav.focused = Some(focus_id.clone());
                    window.focus(&focus, cx);
                    cx.notify();
                }),
            )
            .on_mouse_down(
                gpui::MouseButton::Right,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    this.layout.menu = Some(PaneMenu {
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
                    .when(rang, |d| d.child(label("🔔")))
                    .child(
                        div()
                            .id(gpui::SharedString::from(format!("pane-title-{id}")))
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(T::MONO_SIZE))
                            .child(title)
                            .when_some(session.map(|s| self.session_line(s)), |d, line| {
                                let line = gpui::SharedString::from(line);
                                d.tooltip(move |_, cx| {
                                    cx.new(|_| super::render::TextTip(line.clone())).into()
                                })
                            }),
                    )
                    .when_some(hint.as_deref().map(unread_agent), |d, (note, why)| {
                        d.child(
                            div()
                                .id(gpui::SharedString::from(format!("unread-{id}")))
                                .child(label(note))
                                .tooltip(move |_, cx| {
                                    cx.new(|_| super::render::TextTip(why.into())).into()
                                }),
                        )
                    })
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
                                    let pane = cx.new(|cx| {
                                        Pane::new(this.store.socket(), &takeover, true, cx)
                                    });
                                    window.focus(&pane.focus_handle(cx), cx);
                                    cx.observe(&pane, |_, _, cx| cx.notify()).detach();
                                    this.layout.panes.insert(takeover.clone(), pane);
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
                                    "放入待办",
                                )
                                .h(px(T::ICON_HEIGHT))
                                .px(px(T::S6))
                                .text_size(px(T::META))
                                .text_color(T::color(T::LINK))
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        if let Some(p) = this.layout.panes.get(&target) {
                                            p.update(cx, |p, cx| p.place_text(text.clone(), cx));
                                        }
                                    },
                                )),
                            )
                        },
                    )
                    .when_some(
                        session.filter(|s| {
                            s.outcome == Outcome::Open
                                && session_group(&self.workspace, s) == Group::Review
                        }),
                        |d, s| {
                            let accept = s.id.clone();
                            d.child(
                                button(
                                    gpui::SharedString::from(format!("accept-pane-{id}")),
                                    "标记完成",
                                )
                                .h(px(T::ICON_HEIGHT))
                                .px(px(T::S6))
                                .text_color(T::color(T::LINK))
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.edit(
                                            Edit::SetOutcome {
                                                id: accept.clone(),
                                                outcome: Outcome::Completed,
                                            },
                                            cx,
                                        )
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
                                "看进度",
                            )
                            .h(px(T::ICON_HEIGHT))
                            .px(px(T::S6))
                            .text_color(T::color(T::LINK))
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.overview.here = Some(show.clone());
                                    cx.notify();
                                },
                            )),
                        )
                    })
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
                session.filter(|s| self.overview.here.as_ref() == Some(&s.id)),
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
                            .child(self.here_card(s, true, cx).max_h_full()),
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
                                    this.layout.split_bounds.insert(bounds_path.clone(), bounds);
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
                        // Both panes draw their own border, so the gutter stays empty and
                        // only shows a thin handle on hover or while dragging.
                        div()
                            .id(gpui::SharedString::from(format!("divider-{path:?}")))
                            .group("divider")
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
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
                            .child(
                                div()
                                    .rounded_full()
                                    .when(down, |d| d.h(px(T::S2)).w(px(T::S20 * 2.)))
                                    .when(!down, |d| d.w(px(T::S2)).h(px(T::S20 * 2.)))
                                    .when(self.layout.drag.as_ref() == Some(&path), |d| {
                                        d.bg(T::color(T::FOCUS))
                                    })
                                    .group_hover("divider", |d| d.bg(T::color(T::FOCUS))),
                            )
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    if !this.store.busy() {
                                        this.layout.drag = Some(drag_path.clone());
                                        this.layout.menu = None;
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
        let Some(path) = &self.layout.drag else {
            return;
        };
        let Some(bounds) = self.layout.split_bounds.get(path) else {
            return;
        };
        let Some(Layout::Split { axis, ratio, .. }) = self
            .workspace
            .tabs
            .iter_mut()
            .find(|t| Some(&t.id) == self.nav.tab.as_ref())
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
        if self.layout.drag.take().is_none() {
            return;
        }
        if let Some(tab) = self
            .workspace
            .tabs
            .iter()
            .find(|t| Some(&t.id) == self.nav.tab.as_ref())
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
        let menu = self.layout.menu.take();
        match action {
            "向右分屏" | "向下分屏" => {
                self.nav.zoomed = None;
                self.layout.split_target = Some((
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
                self.nav.zoomed = if self.nav.zoomed.as_deref() == Some(id) {
                    None
                } else {
                    Some(id.into())
                };
                self.sync_panes(cx);
                if let Some(pane) = self.layout.panes.get(id) {
                    window.focus(&pane.focus_handle(cx), cx);
                }
            }
            "关闭画面" => self.close_pane(id, cx),
            "复制" => {
                if let Some(pane) = self.layout.panes.get(id) {
                    pane.update(cx, |p, cx| p.copy(cx));
                }
            }
            "粘贴" => {
                if let Some(pane) = self.layout.panes.get(id) {
                    pane.update(cx, |p, cx| p.paste(cx));
                    window.focus(&pane.focus_handle(cx), cx);
                }
            }
            "搜索滚动内容" => {
                if let Some(pane) = self.layout.panes.get(id) {
                    pane.update(cx, |p, cx| p.open_search(window, cx));
                }
            }
            "中断（Ctrl-C）" => {
                if let Some(pane) = self.layout.panes.get(id) {
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
                    self.layout.menu = Some(menu);
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
                    if self.nav.zoomed.as_deref() == Some(&menu.terminal) {
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
                // A program that took the mouse gets plain drags; say how to select instead.
                if self
                    .layout
                    .panes
                    .get(&menu.terminal)
                    .is_some_and(|p| p.read(cx).takes_mouse())
                {
                    entries.push(
                        label("选中文字：按住 Shift 再拖动")
                            .h(px(T::ROW))
                            .px(px(T::S10))
                            .flex()
                            .items_center()
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
                            agent.name(),
                        )
                        .w_full()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.layout.menu = None;
                            this.tasks.chosen_agent = Some(agent);
                            this.layout.split_target = Some((terminal.clone(), Axis::Right));
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
                                this.layout.menu = None;
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
        let position = menu.position;
        self.popup(position, entries, window, cx, |this| {
            this.layout.menu = None
        })
    }
    /// A tab's right-click menu.
    pub(super) fn tab_menu_view(
        &self,
        id: &str,
        position: gpui::Point<gpui::Pixels>,
        window: &Window,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let rename = id.to_owned();
        let close = id.to_owned();
        let entries = vec![
            button("tab-menu-rename", "重命名")
                .w_full()
                .justify_start()
                .border_0()
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.layout.tab_menu = None;
                    this.rename_tab(&rename, window, cx);
                }))
                .into_any_element(),
            button("tab-menu-close", "关闭标签")
                .w_full()
                .justify_start()
                .border_0()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.close_tab(&close, cx);
                }))
                .into_any_element(),
        ];
        self.popup(position, entries, window, cx, |this| {
            this.layout.tab_menu = None
        })
    }
    /// Ink ring or mascot: the title bar and the Dock tile change at once, and the
    /// choice is remembered for the next launch.
    pub(super) fn icon_menu_view(
        &self,
        position: gpui::Point<gpui::Pixels>,
        window: &Window,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let current = cx.global::<Chosen>().icon;
        let entries = AppIcon::ALL
            .into_iter()
            .map(|icon| {
                div()
                    .id(gpui::SharedString::from(format!(
                        "app-icon-{}",
                        icon.label()
                    )))
                    .role(gpui::accesskit::Role::Button)
                    .aria_label(icon.label())
                    .h(px(T::ROW))
                    .px(px(T::S8))
                    .flex()
                    .items_center()
                    .gap(px(T::S8))
                    .rounded(px(T::CONTROL_RADIUS))
                    .text_size(px(T::SMALL))
                    .cursor_pointer()
                    .hover(|s| s.bg(T::color(T::BUTTON_HOVER)))
                    .child(gpui::img(icon.title_image()).size(px(T::LOGO)))
                    .child(div().flex_1().child(icon.label()))
                    .when(icon == current, |d| {
                        d.child(div().text_color(T::color(T::LINK)).child("✓"))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.layout.icon_menu = None;
                        choose_icon(icon, cx);
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect();
        self.popup(position, entries, window, cx, |this| {
            this.layout.icon_menu = None
        })
    }
    /// Asks before a close stops running Agents; a click outside cancels.
    pub(super) fn confirm_close_view(
        &self,
        closing: &Closing,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let agents = self.running_agents(closing);
        let working = agents.iter().any(|(_, p)| *p == Some(Phase::Working));
        let host = matches!(closing, Closing::Host);
        let what = match closing {
            Closing::Tab(_) => "关闭这个标签",
            Closing::Pane(_) => "关闭这个画面",
            Closing::Host => "重启后台",
        };
        let shells = if host {
            self.running_terminals().len().saturating_sub(agents.len())
        } else {
            0
        };
        let lines = agents
            .iter()
            .map(|(title, phase)| match phase {
                Some(Phase::Working) if host => format!("「{title}」还在工作，重启会打断这一轮。"),
                Some(Phase::Working) => format!("「{title}」还在工作，关闭会打断这一轮。"),
                Some(Phase::Waiting) => format!(
                    "「{title}」在等你回答，{}后它会退出。",
                    if host { "重启" } else { "关闭" }
                ),
                _ => format!(
                    "「{title}」这一轮已经结束，{}后它会退出。",
                    if host { "重启" } else { "关闭" }
                ),
            })
            .chain((shells > 0).then(|| format!("另有 {shells} 个普通终端会停止。")))
            .collect::<Vec<_>>()
            .join("\n");
        let confirmed = closing.clone();
        div()
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.layout.confirm_close = None;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("confirm-close")
                    .w(px(T::HERE_WIDTH))
                    .p(px(T::S16))
                    .flex()
                    .flex_col()
                    .gap(px(T::S12))
                    .bg(T::color(T::RAISED))
                    .border(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::CONTROL))
                    .rounded(px(T::CARD_RADIUS))
                    .shadow_lg()
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .text_size(px(T::CARD_TITLE))
                            .child(if host {
                                "重启后台，换上新版本？".to_owned()
                            } else {
                                format!("{}{what}？", if working { "停止并" } else { "" })
                            }),
                    )
                    .when(host, |d| {
                        d.child(label(
                            "后台还在跑旧版本，新改动要重启后台才生效。重启会停止所有终端，Keepline 会自动重新打开。",
                        ))
                    })
                    .child(label(lines))
                    .child(label(if host {
                        "对话记录会保留，重启后可以原地重开终端，或从左栏「已关闭」里接着聊。"
                    } else {
                        "对话记录会保留，可以从左栏底部「已关闭」里接着聊。"
                    }))
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(T::S8))
                            .child(button("confirm-close-cancel", "取消").on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.layout.confirm_close = None;
                                    cx.notify();
                                },
                            )))
                            .child(
                                button(
                                    "confirm-close-stop",
                                    if host {
                                        "重启后台"
                                    } else if working {
                                        "停止并关闭"
                                    } else {
                                        "关闭"
                                    },
                                )
                                .when(working || host, |b| b.text_color(Group::Error.colors().1))
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.close_now(confirmed.clone(), cx);
                                    },
                                )),
                            ),
                    ),
            )
            .into_any_element()
    }
    /// A floating menu at `position`; a click outside it runs `dismiss`.
    fn popup(
        &self,
        position: gpui::Point<gpui::Pixels>,
        entries: Vec<gpui::AnyElement>,
        window: &Window,
        cx: &Context<Self>,
        dismiss: fn(&mut Self),
    ) -> gpui::AnyElement {
        let height = px(T::ROW * entries.len() as f32 + T::S12);
        let viewport = window.viewport_size();
        div()
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    dismiss(this);
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("pane-menu")
                    .absolute()
                    .left(
                        position
                            .x
                            .min(viewport.width - px(T::MENU_WIDTH))
                            .max(px(0.)),
                    )
                    .top(position.y.min(viewport.height - height).max(px(0.)))
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
/// Applies `icon` to the title bar and the Dock, and remembers it.
pub(super) fn choose_icon(icon: AppIcon, cx: &mut gpui::App) {
    crate::desktop::set_dock_icon(icon.dock_png());
    let file = cx.global::<Chosen>().file.clone();
    if let Err(e) = icon.save(&file) {
        eprintln!("记住图标选择失败: {e}");
    }
    cx.set_global(Chosen { icon, file });
}
#[cfg(test)]
mod tests {
    #[test]
    fn unread_agents_speak_plainly() {
        let note = |hint: &str| super::unread_agent(hint).0;
        assert_eq!(
            note("仅显示状态：工作中；看对话内容请用 codex --no-daemon"),
            "codex · 工作中 · 看不到对话"
        );
        assert_eq!(
            note("未关联：看对话内容请用 codex --no-daemon"),
            "codex · 看不到对话"
        );
        assert_eq!(
            note(keepline_core::protocol::CLAUDE_TRANSCRIPT_OFF),
            "claude · 没有保存记录"
        );
    }
}
