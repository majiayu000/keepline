//! Panes, their split layout, resizing and pane menus.
use super::*;

/// The most a terminal's text opened in an editor holds: the host's own read limit.
const SCROLLBACK_ROWS: usize = 10_000;

/// The visible panes and what is open over them: menus, drags and pending closes.
pub(super) struct LayoutState {
    pub(super) panes: HashMap<String, Entity<Pane>>,
    pub(super) menu: Option<PaneMenu>,
    pub(super) split_target: Option<(String, Axis)>,
    /// A plugin action waiting for its new pane: the command and the directory to run it in.
    pub(super) pending_command: Option<(String, String)>,
    pub(super) drag: Option<Vec<bool>>,
    pub(super) split_bounds: HashMap<Vec<bool>, gpui::Bounds<gpui::Pixels>>,
    /// A tab being renamed in place after a double-click.
    pub(super) renaming_tab: Option<(String, Entity<Field>, [gpui::Subscription; 2])>,
    /// A sidebar conversation being renamed in place after a double-click.
    pub(super) renaming_session: Option<(String, Entity<Field>, [gpui::Subscription; 2])>,
    /// A space renamed in place in the sidebar; it carries the space so a just-added
    /// one can be named before the host's reply lists it.
    pub(super) renaming_space: Option<(Space, Entity<Field>, [gpui::Subscription; 2])>,
    /// A space's right-click menu: the space and where it was opened.
    pub(super) space_menu: Option<(String, gpui::Point<gpui::Pixels>)>,
    /// 删除空间 was pressed once in the open space menu; the second press removes it.
    pub(super) space_removing: bool,
    /// A tab's right-click menu: the tab and where it was opened.
    pub(super) tab_menu: Option<(String, gpui::Point<gpui::Pixels>)>,
    /// The title-bar logo's menu for choosing the app icon, where it was opened.
    pub(super) icon_menu: Option<gpui::Point<gpui::Pixels>>,
    /// A close that would stop a running Agent, waiting for the user to confirm it.
    pub(super) confirm_close: Option<Closing>,
    /// Interrupted terminals already reopened in place once; a failed try keeps the button.
    pub(super) reopened: HashSet<String>,
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
                .border_color(T::color(T::border()))
                .bg(T::color(T::pane()))
                .child(label("已中断（后台重启，原进程已结束）"))
                .child(
                    button(
                        gpui::SharedString::from(format!("reopen-{id}")),
                        "在这里重新开一个终端",
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.reopen(reopen.clone(), cx))),
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
        let session = self.pane_session(id);
        let status = self.pane_status(id).map(|(g, _)| g);
        // The conversation's title names the pane, like its tab; its latest reply is
        // already on screen below, so that line only shows on hover. Without an Agent,
        // the title the program set.
        let title = session
            .map(|s| self.session_title(s))
            .or_else(|| pane.read(cx).program_title())
            .unwrap_or_else(|| "空终端".into());
        let rang = pane.read(cx).bell;
        // What the Agent's adapter says about one it sees here but cannot read.
        let unread = self.store.recognition().terminal_notes.get(id).cloned();
        let takeover = id.to_owned();
        let focus = pane.focus_handle(cx);
        let focus_id = id.to_owned();
        let menu_id = id.to_owned();
        let close_id = id.to_owned();
        let other_owner = pane.read(cx).has_other_owner();
        let card_id = session.map(|s| s.id.clone());
        let terminal_style = crate::ui::style::current(cx) == crate::ui::style::Style::Terminal;
        let decorated = self.pane_decorations(id, cx);
        // Where the program runs left the footer for the title's hover card.
        let tip: Vec<String> = session
            .map(|s| self.session_line(s))
            .into_iter()
            .chain(self.pane_place(id, session, cx))
            .chain(decorated.tip)
            .filter(|l| !l.is_empty())
            .collect();
        let story = session.zip(status).and_then(|(s, g)| self.pane_story(s, g));
        let review = session.filter(|s| {
            s.outcome == Outcome::Open && session_group(&self.workspace, s) == Group::Review
        });
        let line = T::color(if focused { T::focus() } else { T::border() });
        // The status and title, then the meta and the actions: two groups so the terminal
        // style can set them into its frame line with the line showing between them.
        let left = div()
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(T::S8))
            .when_some(status, |d, g| {
                d.child(if terminal_style {
                    div()
                        .flex()
                        .gap(px(T::S4))
                        .text_size(px(T::MONO_SIZE))
                        .text_color(g.colors().1)
                        .child(glyph(g))
                        .child(g.label())
                } else {
                    state_chip(g)
                })
            })
            .when(rang, |d| d.child(label("🔔")))
            .child(
                div()
                    .id(gpui::SharedString::from(format!("pane-title-{id}")))
                    .when(!terminal_style, |d| d.flex_1())
                    .min_w_0()
                    .truncate()
                    .text_size(px(T::MONO_SIZE))
                    .text_color(T::color(T::primary()))
                    .child(title)
                    .when(!tip.is_empty(), |d| {
                        let line = gpui::SharedString::from(tip.join("\n"));
                        d.tooltip(move |_, cx| {
                            cx.new(|_| super::render::TextTip(line.clone())).into()
                        })
                    }),
            );
        let right = div()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(T::S8))
            .when_some(unread.clone().map(|n| (n.note, n.why)), |d, (note, why)| {
                d.child(
                    div()
                        .id(gpui::SharedString::from(format!("unread-{id}")))
                        .child(label(note))
                        .tooltip(move |_, cx| {
                            cx.new(|_| super::render::TextTip(why.clone().into()))
                                .into()
                        }),
                )
            })
            // What the plugins put here, before the Agent and its time.
            .children(decorated.title.into_iter().map(|line| {
                div()
                    .flex_shrink_0()
                    .text_size(px(T::META))
                    .text_color(T::color(T::muted()))
                    .child(line)
            }))
            .when(unread.is_none(), |d| d.child(self.pane_meta(session, cx)))
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
                        .pane_action()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(p) = this.layout.panes.get(&target) {
                                p.update(cx, |p, cx| p.place_text(text.clone(), cx));
                            }
                        })),
                    )
                },
            )
            .when_some(review, |d, s| {
                let accept = s.id.clone();
                d.child(
                    button(
                        gpui::SharedString::from(format!("accept-pane-{id}")),
                        "标记完成",
                    )
                    .pane_action()
                    // The step this pane waits on you for; the rest stay text.
                    .bg(T::color(T::button()))
                    .text_color(T::color(T::primary()))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.edit(
                            Edit::SetOutcome {
                                id: accept.clone(),
                                outcome: Outcome::Completed,
                            },
                            cx,
                        )
                    })),
                )
            })
            // One step per pane: 标记完成 when it is due, else 看进度.
            .when_some(session.filter(|_| review.is_none()), |d, s| {
                let show = s.id.clone();
                d.child(
                    button(
                        gpui::SharedString::from(format!("here-pane-{id}")),
                        "看进度",
                    )
                    .pane_action()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.overview.here = Some(show.clone());
                        cx.notify();
                    })),
                )
            })
            .child(
                button(gpui::SharedString::from(format!("close-pane-{id}")), "×")
                    .aria_label("关闭画面")
                    .pane_action()
                    .text_color(T::color(T::muted()))
                    .on_click(cx.listener(move |this, _, _, cx| this.close_pane(&close_id, cx))),
            );
        let story = story.map(|story| {
            div()
                .flex_shrink_0()
                .px(px(T::S10))
                .py(px(T::S2))
                .truncate()
                .text_size(px(T::META))
                .text_color(T::color(T::secondary()))
                .when(!terminal_style, |d| {
                    d.border_b(px(T::BORDER_WIDTH))
                        .border_color(T::color(T::border()))
                })
                .child(story)
        });
        let body = div()
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
                            .and_then(|tid| self.workspace.tasks.iter().find(|t| &t.id == tid)),
                    ),
                |d, (state, task)| d.child(self.handoff_bar(id, task, state, cx)),
            )
            .child(pane.clone().cached(gpui::StyleRefinement {
                size: gpui::SizeRefinement {
                    width: Some(gpui::relative(1.).into()),
                    height: Some(gpui::relative(1.).into()),
                },
                ..Default::default()
            }));
        let footer = other_owner.then(|| self.pane_footer(id, takeover, cx));
        let mut pane_box = div()
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
            .overflow_hidden();
        if terminal_style {
            // The title sits on the frame's top line, tmux / Zellij style: the header's
            // upper half is the gap around the pane, its lower half the pane, and each
            // group covers the line with the same two tones.
            let half = T::PANE_HEADER / 2.;
            let split = gpui::linear_gradient(
                180.,
                gpui::linear_color_stop(T::color(T::app()), 0.5),
                gpui::linear_color_stop(T::color(T::pane()), 0.5),
            );
            let frame = |d: gpui::Div| d.bg(line);
            pane_box = pane_box
                .child(
                    div()
                        .h(px(T::PANE_HEADER))
                        .flex_shrink_0()
                        .relative()
                        .flex()
                        .items_center()
                        .px(px(T::S8))
                        .bg(split)
                        .when(borders, |d| {
                            d.child(frame(
                                div()
                                    .absolute()
                                    .left_0()
                                    .right_0()
                                    .top(px(half))
                                    .h(px(T::BORDER_WIDTH)),
                            ))
                            .child(frame(
                                div()
                                    .absolute()
                                    .left_0()
                                    .top(px(half))
                                    .bottom_0()
                                    .w(px(T::BORDER_WIDTH)),
                            ))
                            .child(frame(
                                div()
                                    .absolute()
                                    .right_0()
                                    .top(px(half))
                                    .bottom_0()
                                    .w(px(T::BORDER_WIDTH)),
                            ))
                        })
                        .child(left.flex_shrink_1().px(px(T::S6)).bg(split))
                        .child(div().flex_1())
                        .child(right.px(px(T::S6)).bg(split)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .flex_col()
                        .bg(T::color(T::pane()))
                        .when(borders, |d| {
                            d.border_l(px(T::BORDER_WIDTH))
                                .border_r(px(T::BORDER_WIDTH))
                                .border_b(px(T::BORDER_WIDTH))
                                .border_color(line)
                        })
                        .children(story)
                        .child(body)
                        .children(footer),
                );
        } else {
            pane_box = pane_box
                .rounded(px(T::PANE_RADIUS))
                .border(px(if borders { T::BORDER_WIDTH } else { 0. }))
                .border_color(line)
                .bg(T::color(T::pane()))
                .child(
                    div()
                        .h(px(T::PANE_HEADER))
                        .flex_shrink_0()
                        .px(px(T::S10))
                        .flex()
                        .items_center()
                        .gap(px(T::S8))
                        // GPUI 只按矩形裁剪子元素，标题栏不跟着圆角会从外框圆角外露出方角
                        .rounded_t(px(T::PANE_RADIUS - T::BORDER_WIDTH))
                        .bg(T::color(if focused {
                            T::header_focused()
                        } else {
                            T::header()
                        }))
                        .border_b(px(T::BORDER_WIDTH))
                        .border_color(T::color(T::border()))
                        .child(left.flex_1())
                        .child(right),
                )
                .children(story)
                .child(body)
                .children(footer)
                // Over the border rather than widening it: a wider border on focus would
                // shrink the grid and resize the program each time focus moves.
                .when(borders && focused, |d| {
                    d.child(
                        div()
                            .absolute()
                            .inset_0()
                            .rounded(px(T::PANE_RADIUS))
                            .border(px(T::FOCUS_RING))
                            .border_color(T::color(T::focus())),
                    )
                });
        }
        pane_box
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
                                        d.bg(T::color(T::focus()))
                                    })
                                    .group_hover("divider", |d| d.bg(T::color(T::focus()))),
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
    /// Moves a pane, process and all, beside the panes of another tab or into a tab of
    /// its own, and follows it there. Its old tab closes with its last pane.
    pub(super) fn move_pane(&mut self, terminal: &str, to: Option<&str>, cx: &mut Context<Self>) {
        let tab = match to {
            Some(to) => {
                let Some(mut tab) = self.workspace.tabs.iter().find(|t| t.id == to).cloned() else {
                    return;
                };
                let Some(last) = tab.layout.as_ref().and_then(|l| l.ids().last().cloned()) else {
                    return;
                };
                if !tab
                    .layout
                    .as_mut()
                    .is_some_and(|l| l.split(&last, terminal.to_owned(), Axis::Right))
                {
                    return;
                }
                tab
            }
            None => Tab {
                id: super::id(),
                space: self.nav.space.clone(),
                default_task: None,
                title: String::new(),
                layout: Some(Layout::Leaf(terminal.to_owned())),
            },
        };
        self.nav.zoomed = None;
        self.nav.tab = Some(tab.id.clone());
        self.nav.focused = Some(terminal.to_owned());
        self.nav.pending_focus = true;
        self.edit(Edit::PutTab { tab }, cx);
    }
    /// The terminal's text, scrollback included, saved under the host's private runtime
    /// directory and opened with `$EDITOR` in a new tab, as Herdr does. The host reads it,
    /// so a pane's own screen is never locked for it; one file per terminal, replaced.
    fn scrollback_in_editor(&mut self, id: &str, cx: &mut Context<Self>) {
        let socket = self.store.socket().to_owned();
        let terminal = id.to_owned();
        let cwd = self
            .workspace
            .terminal_tab(id)
            .and_then(|t| self.workspace.spaces.iter().find(|s| s.id == t.space))
            .map(|s| s.cwd.clone())
            .unwrap_or_default();
        let task = cx.background_spawn(async move {
            let Reply::Text(text) = rpc(
                &socket,
                &Request::ReadScreen {
                    id: terminal.clone(),
                    rows: SCROLLBACK_ROWS,
                },
            )?
            else {
                return Err(std::io::Error::other("后台没有返回文字"));
            };
            let dir = socket
                .parent()
                .ok_or_else(|| std::io::Error::other("运行目录未知"))?
                .join("scrollback");
            std::fs::create_dir_all(&dir)?;
            let path = dir.join(format!("{terminal}.txt"));
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&path)?;
            std::io::Write::write_all(&mut file, text.trim_end().as_bytes())?;
            Ok(path)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| match result {
                Ok(path) => {
                    // Through sh so the user's $EDITOR applies whatever their shell is.
                    let command = format!(
                        "sh -c '${{EDITOR:-vi}} \"$1\"' sh {}",
                        shell_quote(&path.to_string_lossy())
                    );
                    this.layout.pending_command = Some((command, cwd));
                    this.launch(false, None, cx);
                }
                Err(e) => {
                    this.error = Some(format!("在编辑器里看滚动内容失败：{e}"));
                    cx.notify();
                }
            })
        })
        .detach();
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
            "在编辑器里看滚动内容" => self.scrollback_in_editor(id, cx),
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
            "挂到待办…" | "为这个待办开一个会话" | "移到标签…" | "为什么是这个状态" => {
                if let Some(mut menu) = menu {
                    menu.page = match action {
                        "挂到待办…" => MenuPage::Assign,
                        "移到标签…" => MenuPage::Move,
                        "为什么是这个状态" => MenuPage::Why,
                        _ => MenuPage::Sessions,
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
                    "在编辑器里看滚动内容",
                    "移到标签…",
                    "为什么是这个状态",
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
                // Plugin actions this pane offers, after the built-ins.
                let offered = self.actions_for(&menu.terminal);
                if !offered.is_empty() {
                    entries.push(
                        label("插件")
                            .h(px(T::ROW))
                            .px(px(T::S10))
                            .flex()
                            .items_center()
                            .border_t(px(T::BORDER_WIDTH))
                            .border_color(T::color(T::border()))
                            .into_any_element(),
                    );
                }
                for action in offered {
                    let terminal = menu.terminal.clone();
                    entries.push(
                        button(
                            gpui::SharedString::from(format!("menu-action-{}", action.id)),
                            action.title.clone(),
                        )
                        .w_full()
                        .justify_start()
                        .border_0()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.run_action(&terminal, action, cx);
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
                for agent in Agent::all() {
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
            MenuPage::Why => {
                let hints = &self.store.recognition().terminal_hints;
                for (i, line) in state_reasons(
                    &self.workspace,
                    &menu.terminal,
                    hints.get(&menu.terminal).map(String::as_str),
                    now(),
                )
                .into_iter()
                .enumerate()
                {
                    entries.push(
                        div()
                            .id(gpui::SharedString::from(format!("why-{i}")))
                            .w_full()
                            .px(px(T::S10))
                            .py(px(T::S4))
                            .text_size(px(if i == 0 { T::META } else { T::SMALL }))
                            .text_color(T::color(if i == 0 {
                                T::secondary()
                            } else {
                                T::primary()
                            }))
                            .child(line)
                            .into_any_element(),
                    );
                }
            }
            MenuPage::Move => {
                let own = self
                    .workspace
                    .terminal_tab(&menu.terminal)
                    .map(|t| t.id.clone());
                let targets: Vec<_> = self
                    .workspace
                    .tabs
                    .iter()
                    .filter(|t| t.space == self.nav.space && Some(&t.id) != own.as_ref())
                    .map(|t| (Some(t.id.clone()), self.tab_name(t, cx)))
                    .chain([(None, "新标签".to_owned())])
                    .collect();
                for (tab, name) in targets {
                    let terminal = menu.terminal.clone();
                    entries.push(
                        button(
                            gpui::SharedString::from(format!(
                                "move-to-{}",
                                tab.as_deref().unwrap_or("new")
                            )),
                            name,
                        )
                        .w_full()
                        .justify_start()
                        .border_0()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.layout.menu = None;
                            this.move_pane(&terminal, tab.as_deref(), cx);
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
        let tabs = self.sibling_tabs(id);
        let at = tabs.iter().position(|t| t == id).unwrap_or(0);
        let mut entries: Vec<gpui::AnyElement> =
            [("向左移", -1, at > 0), ("向右移", 1, at + 1 < tabs.len())]
                .into_iter()
                .map(|(name, step, enabled)| {
                    let id = id.to_owned();
                    button(gpui::SharedString::from(format!("tab-menu-{name}")), name)
                        .w_full()
                        .justify_start()
                        .border_0()
                        .when(!enabled, |d| d.opacity(0.4))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if enabled {
                                this.layout.tab_menu = None;
                                this.step_tab(&id, step, cx);
                            }
                        }))
                        .into_any_element()
                })
                .collect();
        entries.extend([
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
        ]);
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
                    .hover(|s| s.bg(T::color(T::button_hover())))
                    .child(gpui::img(icon.title_image()).size(px(T::LOGO)))
                    .child(div().flex_1().child(icon.label()))
                    .when(icon == current, |d| {
                        d.child(div().text_color(T::color(T::link())).child("✓"))
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
    /// Asks before a close stops running Agents or takes several splits with it; a click
    /// outside cancels.
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
        let splits = splits_closed(&self.workspace, closing);
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
            .chain((shells > 0).then(|| format!("另有 {shells} 个普通终端会停止。")));
        let lines = (splits > 0)
            .then(|| format!("这个标签有 {splits} 个分屏，会一起关闭。"))
            .into_iter()
            .chain(lines)
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
                    .bg(T::color(T::raised()))
                    .border(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::control()))
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
    pub(super) fn popup(
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
                    .bg(T::color(T::raised()))
                    .border(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::control()))
                    .rounded(px(T::CONTROL_RADIUS))
                    .shadow_lg()
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .children(entries),
            )
            .into_any_element()
    }
}
/// Applies `icon` to the title bar and the Dock, and remembers it.
/// One look for the pane header's text actions: link-colored, filled only on hover.
trait PaneAction {
    fn pane_action(self) -> Self;
}
impl PaneAction for gpui::Stateful<gpui::Div> {
    fn pane_action(self) -> Self {
        self.h(px(T::PANE_ACTION))
            .px(px(T::S6))
            .rounded(px(T::PANE_ACTION_RADIUS))
            .text_size(px(T::SMALL))
            .text_color(T::color(T::link()))
    }
}
/// The pane header's status: the group's wash, its ink and its dot.
fn state_chip(group: Group) -> gpui::Div {
    let (dot, ink, wash) = group.colors();
    div()
        .flex_shrink_0()
        .h(px(T::STATE_CHIP))
        .px(px(T::S6))
        .flex()
        .items_center()
        .gap(px(T::PULSE_GAP))
        .rounded(px(T::STATE_CHIP_RADIUS))
        .bg(wash)
        .text_color(ink)
        .text_size(px(T::META))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .child(div().size(px(T::DOT)).rounded_full().bg(dot))
        .child(group.label())
}
/// Lower is more urgent: what a tab's dot shows when its panes disagree.
pub(super) fn urgency(group: Group) -> u8 {
    match group {
        Group::Waiting => 0,
        Group::Error => 1,
        Group::Done => 2,
        Group::Review => 3,
        Group::Working => 4,
        Group::Running => 5,
        Group::Paused => 6,
        Group::Ready => 7,
        Group::Stale => 8,
    }
}
/// `3:12`, or `1:03:12` past an hour.
fn turn_clock(seconds: u64) -> String {
    let (h, m, s) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}
/// A running turn's clock is redrawn once a second while one is on screen, and not at all
/// otherwise; one pending redraw serves every pane.
static TURN_TICK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
const TURN_TICK_EVERY: Duration = Duration::from_secs(1);
/// Each directory's branch with when it was read.
type Branches = HashMap<String, (std::time::Instant, Option<String>)>;
/// Branches by directory, read off the render path and reread at most this often.
static BRANCHES: std::sync::LazyLock<std::sync::Mutex<Branches>> =
    std::sync::LazyLock::new(Default::default);
const BRANCH_TTL: Duration = Duration::from_secs(10);
/// The branch checked out where `dir` sits, from `.git/HEAD` (a linked worktree's `.git`
/// file points at its own HEAD); a detached HEAD shows its short commit.
fn read_branch(dir: &Path) -> Option<String> {
    for d in dir.ancestors() {
        let dot = d.join(".git");
        let git_dir = if dot.is_dir() {
            dot
        } else if dot.is_file() {
            let text = std::fs::read_to_string(&dot).ok()?;
            d.join(text.trim().strip_prefix("gitdir:")?.trim())
        } else {
            continue;
        };
        let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
        let head = head.trim();
        return Some(match head.strip_prefix("ref: refs/heads/") {
            Some(branch) => branch.to_owned(),
            None => head.chars().take(7).collect(),
        });
    }
    None
}
impl Workbench {
    /// The conversation a pane shows: its current one, else the latest it ran.
    pub(super) fn pane_session(&self, id: &str) -> Option<&AgentSession> {
        self.workspace.current_agent(id).or_else(|| {
            self.workspace
                .agent_sessions
                .iter()
                .filter(|s| s.terminal.as_deref() == Some(id))
                .max_by_key(|s| s.updated)
        })
    }
    /// A pane's status as the sidebar shows it, and whether it asks for attention; a
    /// conversation already closed out and no longer live shows none.
    pub(super) fn pane_status(&self, id: &str) -> Option<(Group, bool)> {
        let s = self.pane_session(id)?;
        if live_terminal(&self.workspace, s).is_none() && !s.outcome.needs_review(now()) {
            return None;
        }
        Some(sidebar_status(&self.workspace, &self.attention.unseen, s))
    }
    /// `claude · 5 分钟`, or `claude · 本轮 3:12` while a turn runs in a live terminal.
    fn pane_meta(&self, session: Option<&AgentSession>, cx: &Context<Self>) -> gpui::Div {
        let meta = div()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(T::S6))
            .text_size(px(T::META))
            .text_color(T::color(T::muted()))
            .font_features(T::tabular());
        let Some(s) = session else {
            return meta.child("Shell");
        };
        let content = self.workspace.content.get(&s.id);
        let started = content
            .filter(|c| c.turn == keepline_core::content::Turn::Running && c.turn_started > 0)
            .filter(|_| live_terminal(&self.workspace, s).is_some())
            .map(|c| c.turn_started);
        let when = match started {
            Some(at) => {
                self.tick_turn(cx);
                format!("本轮 {}", turn_clock(now().saturating_sub(at)))
            }
            None => age(content.map_or(s.updated, |c| c.updated)),
        };
        meta.child(s.runtime.executable().to_string())
            .child(div().opacity(T::ZERO_OPACITY).child("·"))
            .child(when)
    }
    fn tick_turn(&self, cx: &Context<Self>) {
        use std::sync::atomic::Ordering;
        if TURN_TICK.swap(true, Ordering::Relaxed) {
            return;
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TURN_TICK_EVERY).await;
            TURN_TICK.store(false, Ordering::Relaxed);
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }
    /// The cached branch for `dir`; a missing or old entry is read in the background and
    /// redraws the window only if it changed.
    pub(super) fn git_branch(&self, dir: &str, cx: &Context<Self>) -> Option<String> {
        let mut map = BRANCHES.lock().ok()?;
        let old = match map.get(dir) {
            Some((at, branch)) if at.elapsed() < BRANCH_TTL => return branch.clone(),
            Some((_, branch)) => branch.clone(),
            None => None,
        };
        // Marked as read now, so the redraws before the read lands do not read again.
        map.insert(dir.to_owned(), (std::time::Instant::now(), old.clone()));
        drop(map);
        let dir = dir.to_owned();
        cx.spawn(async move |this, cx| {
            let at = PathBuf::from(&dir);
            let branch = cx.background_spawn(async move { read_branch(&at) }).await;
            let changed = BRANCHES.lock().is_ok_and(|mut map| {
                let before = map.insert(dir, (std::time::Instant::now(), branch.clone()));
                before.and_then(|(_, b)| b) != branch
            });
            if changed {
                let _ = this.update(cx, |_, cx| cx.notify());
            }
        })
        .detach();
        old
    }
    /// `~/code/app · main`: where the pane's program is and on which branch.
    fn pane_place(
        &self,
        id: &str,
        session: Option<&AgentSession>,
        cx: &Context<Self>,
    ) -> Option<String> {
        let cwd = self
            .layout
            .panes
            .get(id)
            .and_then(|p| p.read(cx).cwd().map(|c| c.to_string_lossy().into_owned()))
            .or_else(|| session.map(|s| s.cwd.clone()).filter(|c| !c.is_empty()))
            .or_else(|| {
                self.workspace
                    .terminals
                    .iter()
                    .find(|t| t.id == id)
                    .map(|t| t.cwd.clone())
            })
            .filter(|c| !c.is_empty())?;
        Some(match self.git_branch(&cwd, cx) {
            Some(b) => format!("{} · {b}", home_relative(&cwd)),
            None => home_relative(&cwd),
        })
    }
    /// `停在 测试超时 · 你要它 修好登录`: what the conversation is at and what it was asked.
    fn pane_story(&self, s: &AgentSession, g: Group) -> Option<String> {
        let at = self.row_detail(s, g).map(|d| {
            if d.starts_with("停在") || d.starts_with("正在") {
                d
            } else {
                format!("停在 {d}")
            }
        });
        let ask = self
            .workspace
            .content
            .get(&s.id)
            .and_then(|c| c.asks.first())
            // A title taken from the first ask already says it.
            .filter(|q| {
                let title = self.session_title(s);
                let title = title.trim_end_matches('…').trim();
                title.is_empty() || !q.text.trim().starts_with(title)
            })
            .map(|q| format!("你要它 {}", keepline_core::content::short(&q.text, 80)));
        let parts: Vec<String> = at.into_iter().chain(ask).collect();
        (!parts.is_empty()).then(|| parts.join(" · "))
    }
    /// Shown only while another window types into this pane: the way to take it back.
    fn pane_footer(&self, id: &str, takeover: String, cx: &Context<Self>) -> gpui::Div {
        div()
            .h(px(T::FOOTER))
            .flex_shrink_0()
            .px(px(T::S10))
            .flex()
            .items_center()
            .border_t(px(T::BORDER_WIDTH))
            .border_color(T::color(T::border()))
            .text_size(px(T::PANE_FOOTER_SIZE))
            .text_color(T::color(T::muted()))
            .child(div().flex_1().child("另一个窗口在输入"))
            .child(
                button(
                    gpui::SharedString::from(format!("takeover-{id}")),
                    "点击接管输入",
                )
                .flex_shrink_0()
                .h(px(T::ICON_HEIGHT))
                .px(px(T::S6))
                .text_size(px(T::PANE_FOOTER_SIZE))
                .text_color(T::color(T::link()))
                .on_click(cx.listener(move |this, _, window, cx| {
                    let pane = cx.new(|cx| Pane::new(this.store.socket(), &takeover, true, cx));
                    window.focus(&pane.focus_handle(cx), cx);
                    cx.observe(&pane, |_, _, cx| cx.notify()).detach();
                    this.layout.panes.insert(takeover.clone(), pane);
                    cx.notify();
                })),
            )
    }
}
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
    fn turn_clock_reads_minutes_and_hours() {
        assert_eq!(super::turn_clock(0), "0:00");
        assert_eq!(super::turn_clock(192), "3:12");
        assert_eq!(super::turn_clock(3792), "1:03:12");
    }
    #[test]
    fn branch_comes_from_head_in_a_repo_or_a_linked_worktree() {
        let root = std::env::temp_dir().join(format!("kl-branch-{}", std::process::id()));
        let repo = root.join("repo");
        let nested = repo.join("a/b");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(repo.join(".git/HEAD"), "ref: refs/heads/feat/ui-main\n").unwrap();
        assert_eq!(super::read_branch(&nested).as_deref(), Some("feat/ui-main"));
        let linked = root.join("linked");
        let gitdir = repo.join(".git/worktrees/linked");
        std::fs::create_dir_all(&linked).unwrap();
        std::fs::create_dir_all(&gitdir).unwrap();
        std::fs::write(gitdir.join("HEAD"), "0123456789abcdef\n").unwrap();
        std::fs::write(
            linked.join(".git"),
            format!("gitdir: {}\n", gitdir.display()),
        )
        .unwrap();
        assert_eq!(super::read_branch(&linked).as_deref(), Some("0123456"));
        assert_eq!(super::read_branch(&root), None);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
