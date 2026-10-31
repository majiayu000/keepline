//! Session grouping, the 「你在这里」 card, the "needs you" queue and navigation into a session.
use super::overview::{quiet, solid};
use super::*;

/// Which finished turns the user has not looked at yet.
pub(super) struct Attention {
    /// Last live phase of each session whose agent runs in a Keepline terminal.
    pub(super) phases: HashMap<String, Phase>,
    /// Sessions whose turn ended while their pane was out of view, until it is shown.
    pub(super) unseen: HashSet<String>,
}
/// Conversations being resumed: the next launch's command, and the terminals opened for it.
pub(super) struct Resuming {
    /// The next launch resumes a conversation: its command, cwd and session id.
    pub(super) resume: Option<(String, String, String)>,
    /// Terminals opened to resume a session, so clicking it again returns there
    /// instead of opening another tab while the resume has not taken.
    pub(super) resumed: HashMap<String, String>,
}
impl Workbench {
    pub(super) fn open_session(
        &mut self,
        session: &AgentSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.search.hide();
        self.nav.task_detail = None;
        self.overview.here = None;
        self.nav.zoomed = None;
        let Some(terminal) = live_terminal(&self.workspace, session)
            .or_else(|| resume_terminal(&self.workspace, &self.resuming.resumed, session))
        else {
            if let Some(space) = session_space(&self.workspace, session) {
                self.nav.space = space.into();
            }
            self.resuming.resume = Some((
                self.workspace.resume_command(session),
                session.cwd.clone(),
                session.id.clone(),
            ));
            self.launch(false, session.task.clone(), cx);
            return;
        };
        let terminal_id = terminal.id.clone();
        if !self.focus_terminal(&terminal_id, window, cx) {
            let tab = Tab {
                id: id(),
                space: self.nav.space.clone(),
                default_task: session.task.clone(),
                title: String::new(),
                layout: Some(Layout::Leaf(terminal_id.clone())),
            };
            self.nav.tab = Some(tab.id.clone());
            self.nav.overview = false;
            self.edit(Edit::PutTab { tab }, cx);
        }
    }
    /// Shows the tab holding `terminal_id` and focuses its pane; false when no tab holds it.
    pub(super) fn focus_terminal(
        &mut self,
        terminal_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.nav.focused = Some(terminal_id.to_owned());
        self.nav.pending_focus = true;
        let Some(tab) = self.workspace.terminal_tab(terminal_id) else {
            return false;
        };
        self.nav.space = tab.space.clone();
        self.nav.tab = Some(tab.id.clone());
        self.nav.overview = false;
        self.sync_panes(cx);
        if let Some(pane) = self.layout.panes.get(terminal_id) {
            window.focus(&pane.focus_handle(cx), cx);
        }
        cx.notify();
        true
    }
    /// See `Workspace::session_title`; every surface names a conversation the same way.
    pub(super) fn session_title(&self, s: &AgentSession) -> String {
        self.workspace.session_title(s)
    }
    /// What hovering a conversation's row shows: where its name comes from, what it
    /// was first and last asked, and where it runs.
    pub(super) fn session_tip(&self, s: &AgentSession) -> Vec<String> {
        use keepline_core::content::short;
        let mut lines = vec![match self.workspace.session_naming(s) {
            Some((_, source)) => format!("名字来自{} · 双击改名", source.label(s.runtime)),
            None => "双击改名".into(),
        }];
        if let Some(c) = self.workspace.content.get(&s.id) {
            let line =
                |text: &str| short(&text.split_whitespace().collect::<Vec<_>>().join(" "), 120);
            if let Some(first) = c.asks.first().filter(|_| c.first_ask_known) {
                lines.push(format!("最初：{}", line(&first.text)));
            }
            // Only when it is not the first ask shown above.
            if let Some(last) = c
                .asks
                .last()
                .filter(|_| c.asks.len() > 1 || !c.first_ask_known)
            {
                lines.push(format!("最近：{}", line(&last.text)));
            }
        }
        lines.push(format!("{} · {}", s.runtime.name(), s.cwd));
        lines
    }
    /// Double-click names a conversation; an empty name hands it back to its CLI and
    /// first ask.
    pub(super) fn rename_session(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.save_session_name(cx);
        let Some(session) = self.workspace.agent_sessions.iter().find(|s| s.id == id) else {
            return;
        };
        let automatic = self.session_title(&AgentSession {
            name: String::new(),
            ..session.clone()
        });
        let current = session.name.clone();
        let field = cx.new(|cx| {
            Field::new("会话名", &current, cx)
                .inline()
                .placeholder(&automatic)
        });
        field.update(cx, |f, _| f.select_all());
        let events = cx.subscribe(&field, |this, _, event, cx| match event {
            crate::ui::field::FieldEvent::Submit => this.save_session_name(cx),
            crate::ui::field::FieldEvent::Cancel => {
                this.layout.renaming_session = None;
                cx.notify();
            }
        });
        // Clicking anywhere else keeps the name, as with tabs.
        let blur = cx.on_blur(&field.focus_handle(cx), window, |this, _, cx| {
            this.save_session_name(cx)
        });
        window.focus(&field.focus_handle(cx), cx);
        self.layout.renaming_session = Some((id.to_owned(), field, [events, blur]));
        cx.notify();
    }
    fn save_session_name(&mut self, cx: &mut Context<Self>) {
        let Some((id, field, _)) = self.layout.renaming_session.take() else {
            return;
        };
        let Some(session) = self.workspace.agent_sessions.iter().find(|s| s.id == id) else {
            return;
        };
        let name = field.read(cx).text.trim().to_owned();
        // Keeping the shown name unchanged must not pin today's automatic one.
        let name = if session.name.is_empty() && name == self.session_title(session) {
            String::new()
        } else {
            name
        };
        if name != session.name {
            self.edit(Edit::RenameSession { id, name }, cx);
        }
        cx.notify();
    }
    /// When its conversation last moved.
    pub(super) fn session_updated(&self, s: &AgentSession) -> u64 {
        self.workspace
            .content
            .get(&s.id)
            .map_or(s.updated, |c| c.updated)
    }
    pub(super) fn session_line(&self, s: &AgentSession) -> String {
        self.workspace
            .content
            .get(&s.id)
            .map(|c| keepline_core::content::short(&c.headline(), 100))
            .unwrap_or_else(|| "内容尚未索引".into())
    }
    pub(super) fn save_session_task(&mut self, session: &AgentSession, cx: &mut Context<Self>) {
        let Some(c) = self.workspace.content.get(&session.id) else {
            return;
        };
        if c.asks.is_empty() {
            self.error = Some("尾段没有用户原话，不能凭空建立待办".into());
            cx.notify();
            return;
        }
        let task = Task {
            id: id(),
            space: self.nav.space.clone(),
            title: self.session_title(session),
            description: c
                .asks
                .iter()
                .map(|q| q.text.as_str())
                .collect::<Vec<_>>()
                .join("\n\n追加要求："),
            acceptance: vec![],
            agent: session.runtime,
        };
        self.edit(
            Edit::SaveSessionTask {
                session: session.id.clone(),
                task,
            },
            cx,
        );
    }
    /// `in_pane`: the card sits on the session's own pane, not in the overview.
    /// Head (title, status, this turn), the facts that scroll, and the two decisions at
    /// the foot with the rest behind 更多 (specs/terminal-workbench/design/palette-here.html).
    pub(super) fn here_card(
        &self,
        session: &AgentSession,
        in_pane: bool,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let card = self
            .workspace
            .content
            .get(&session.id)
            .cloned()
            .unwrap_or_default();
        let h = card.here();
        let save = session.clone();
        let (group, _) = sidebar_status(&self.workspace, &self.attention.unseen, session);
        let mut meta = vec![session.runtime.executable().to_owned()];
        if card.turn_started > 0 && card.updated > card.turn_started {
            meta.push(format!(
                "本轮 {}",
                span_label(card.updated - card.turn_started)
            ));
        }
        meta.push(ago(self.session_updated(session), now()));
        let head = div()
            .flex_none()
            .px(px(T::S16))
            .pt(px(T::S14))
            .flex()
            .flex_col()
            .gap(px(T::S8))
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(T::META))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(T::color(T::muted()))
                            .child("你在这里"),
                    )
                    .child(div().flex_1())
                    .child(
                        button("dismiss-here", "×")
                            .h(px(T::PANE_ACTION))
                            .px(px(T::S6))
                            .text_color(T::color(T::muted()))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.overview.here = None;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .text_size(px(T::CARD_TITLE))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(T::color(T::primary()))
                    .line_clamp(3)
                    .child(self.session_title(session)),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(T::S6))
                    .text_size(px(T::META))
                    .text_color(T::color(T::muted()))
                    .font_features(T::tabular())
                    .child(chip(group))
                    .child(meta.join(" · ")),
            );
        // Only the body scrolls: the head and the decisions stay put.
        let mut panel = div()
            .id(gpui::SharedString::from(format!(
                "here-body-{}",
                session.id
            )))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(T::S16))
            .py(px(T::S12))
            .flex()
            .flex_col()
            .gap(px(T::S12));
        let fact = |name: &'static str, value: gpui::Div| {
            div()
                .flex()
                .gap(px(T::S10))
                .child(
                    div()
                        .w(px(T::HERE_FACT))
                        .flex_none()
                        .text_color(T::color(T::muted()))
                        .child(name),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_color(T::color(T::body()))
                        .child(value),
                )
        };
        // Each later ask on its own line, not run together with 「追加：」.
        let mut asks = h.ask.split("\n追加：");
        let mut ask = div()
            .flex()
            .flex_col()
            .gap(px(T::S2))
            .child(asks.next().unwrap_or_default().to_owned());
        for more in asks {
            ask = ask.child(
                div()
                    .text_color(T::color(T::secondary()))
                    .child(format!("追加 {more}")),
            );
        }
        // Only fields that say something: no evidence yet is not news, and the guessed
        // next step was the same few template lines for every session.
        let done = done_items(&card.evidence, &session.cwd);
        let mut facts = div()
            .flex()
            .flex_col()
            .gap(px(T::S8))
            .text_size(px(T::SMALL))
            .child(fact("你要它", ask));
        if !done.is_empty() {
            let mut list = div().flex().flex_col().gap(px(T::S2));
            for (text, times, file) in done {
                list = list.child(
                    div()
                        .when(file, |d| {
                            d.font_family(T::mono(cx).family)
                                .text_size(px(T::META))
                                .text_color(T::color(T::primary()))
                        })
                        .child(text)
                        .when(times > 1, |d| {
                            d.child(
                                div()
                                    .text_size(px(T::CHIP_SIZE))
                                    .text_color(T::color(T::muted()))
                                    .child(format!("改了 {times} 次")),
                            )
                        }),
                );
            }
            facts = facts.child(fact("做成了", list));
        }
        facts = facts.child(fact("停在", div().line_clamp(4).child(h.stopped)));
        panel = panel.child(facts);
        let (ok, files, failed) = evidence_tally(&card.evidence);
        if ok + files + failed > 0 {
            let total = (ok + files + failed) as f32;
            let failure = Group::Error.colors().0;
            let mut bar = div()
                .flex()
                .gap(px(T::S2))
                .h(px(T::EVIDENCE_BAR))
                .overflow_hidden();
            for (n, color) in [
                (ok, T::color(T::secondary())),
                (files, T::color(T::primary())),
                (failed, failure),
            ] {
                if n > 0 {
                    bar = bar.child(
                        div()
                            .h_full()
                            .w(gpui::relative(n as f32 / total))
                            .flex_shrink(1.)
                            .rounded(px(T::EVIDENCE_CELL_RADIUS))
                            .bg(color),
                    );
                }
            }
            let key = |name: &'static str, n: usize, color: gpui::Hsla| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(T::S4))
                    .child(
                        div()
                            .w(px(T::EVIDENCE_CELL_WIDTH))
                            .h(px(T::EVIDENCE_CELL_HEIGHT))
                            .rounded(px(T::EVIDENCE_CELL_RADIUS))
                            .bg(color),
                    )
                    .child(name)
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(T::color(T::primary()))
                            .child(n.to_string()),
                    )
            };
            panel = panel.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(T::S6))
                    .text_size(px(T::META))
                    .font_features(T::tabular())
                    .child(
                        div()
                            .flex()
                            .text_color(T::color(T::muted()))
                            .child(
                                div()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(T::color(T::secondary()))
                                    .child("留下的证据"),
                            )
                            .child(div().flex_1())
                            .child(format!("最近 {} 条", card.evidence.len())),
                    )
                    .child(bar)
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(T::S12))
                            .text_color(T::color(T::secondary()))
                            .child(key("命令成功", ok, T::color(T::secondary())))
                            .child(key("改文件", files, T::color(T::primary())))
                            .child(key("失败", failed, failure)),
                    ),
            );
        }
        let note = |text: String| {
            div()
                .text_size(px(T::META))
                .text_color(T::color(T::muted()))
                .child(text)
        };
        if card.partial_history {
            panel = panel.child(note(
                "仅覆盖已索引的历史尾段与之后的增量；没有证据不等于没做。".into(),
            ));
        }
        if let Some(binding) = session.binding.filter(|_| session.current) {
            panel = panel.child(note(format!("和这个终端对上靠的是：{}", binding.label())));
        }
        if let Some(h) = self
            .workspace
            .outcome_history
            .iter()
            .rev()
            .find(|h| h.session == session.id)
        {
            panel = panel.child(note(format!(
                "{} · {} → {} · 最近一次结局变化",
                h.reason,
                outcome_label(&h.before),
                outcome_label(&h.after)
            )));
        }
        if session.current && session.outcome != Outcome::Open {
            panel = panel.child(note(
                "结局已记录，Agent 仍在运行；若要停止，请在画面菜单选择停止 Agent。".into(),
            ));
        }
        let resume = session.clone();
        let command = self.workspace.resume_command(session);
        // Two decisions stay in sight: go back to it, and settle it. The rest waits in 更多.
        // On its own live pane there is nothing to go back to: you type in the terminal,
        // and settling it is the card's one solid button.
        let typing_here = in_pane && live_terminal(&self.workspace, session).is_some();
        let (settle_name, settle) = if session.outcome == Outcome::Open {
            ("标记完成", Outcome::Completed)
        } else {
            ("撤销结局", Outcome::Open)
        };
        let settle_id = session.id.clone();
        let more = self.overview.here_more.as_ref() == Some(&session.id);
        let more_id = session.id.clone();
        let settle_button = button(
            gpui::SharedString::from(format!("here-{settle_name}")),
            settle_name,
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.edit(
                Edit::SetOutcome {
                    id: settle_id.clone(),
                    outcome: settle.clone(),
                },
                cx,
            )
        }));
        let mut foot = div()
            .flex_none()
            .relative()
            .px(px(T::S16))
            .pt(px(T::S10))
            .pb(px(T::S12))
            .flex()
            .items_center()
            .gap(px(T::S6))
            .border_t(px(T::BORDER_WIDTH))
            .border_color(T::color(T::border()))
            .when(!typing_here, |d| {
                d.child(solid(button("here-continue", "继续")).on_click(
                    cx.listener(move |this, _, window, cx| this.open_session(&resume, window, cx)),
                ))
            })
            .child(if typing_here {
                solid(settle_button)
            } else {
                quiet(settle_button)
            })
            .child(div().flex_1())
            .child(
                button("here-more", "更多 ⋯")
                    .text_color(T::color(T::link()))
                    .when(more, |d| d.bg(T::color(T::button_hover())))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.overview.here_more = (!more).then(|| more_id.clone());
                        cx.notify();
                    })),
            );
        if more {
            let item = |id: &'static str, name: &'static str| {
                button(id, "")
                    .aria_label(name)
                    .w_full()
                    .px(px(T::S8))
                    .justify_start()
                    .gap(px(T::S8))
                    .child(name)
            };
            let fold = |this: &mut Self| this.overview.here_more = None;
            let hint: String = command.chars().take(T::HERE_COMMAND_CHARS).collect();
            let mut menu = div()
                .id("here-menu")
                .absolute()
                .right(px(T::S12))
                .bottom(gpui::relative(1.))
                .w(px(T::MENU_WIDTH))
                .p(px(T::S4))
                .flex()
                .flex_col()
                .gap(px(T::BORDER_WIDTH))
                .rounded(px(T::CARD_RADIUS))
                .bg(T::color(T::raised()))
                .border(px(T::BORDER_WIDTH))
                .border_color(T::color(T::control()))
                .shadow_lg()
                .child(
                    item("here-copy", "复制命令")
                        .child(div().flex_1())
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_size(px(T::CHIP_SIZE))
                                .text_color(T::color(T::muted()))
                                .font_family(T::mono(cx).family)
                                .child(if hint.len() < command.len() {
                                    format!("{hint}…")
                                } else {
                                    hint
                                }),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(command.clone()));
                            fold(this);
                            cx.notify();
                        })),
                )
                .child(item("save-here-task", "存为待办").on_click(cx.listener(
                    move |this, _, _, cx| {
                        fold(this);
                        this.save_session_task(&save, cx)
                    },
                )));
            let sid = session.id.clone();
            menu = menu.child(item("here-defer", "稍后再看…").on_click(cx.listener(
                move |this, _, window, cx| {
                    fold(this);
                    this.overview.here = Some(sid.clone());
                    this.nav.overview = false;
                    this.show_form(FormKind::Defer, window, cx);
                },
            )));
            // The other endings; 放弃 sits apart below a rule.
            for (name, outcome) in [
                ("标记完成", Outcome::Completed),
                ("撤销结局", Outcome::Open),
                ("放弃", Outcome::Abandoned),
            ]
            .into_iter()
            .filter(|(name, o)| *o != session.outcome && *name != settle_name)
            {
                let sid = session.id.clone();
                let abandon = outcome == Outcome::Abandoned;
                if abandon {
                    menu = menu.child(
                        div()
                            .mx(px(T::S2))
                            .my(px(T::S4))
                            .h(px(T::BORDER_WIDTH))
                            .bg(T::color(T::control())),
                    );
                }
                menu = menu.child(
                    item(
                        match name {
                            "标记完成" => "here-menu-标记完成",
                            "撤销结局" => "here-menu-撤销结局",
                            _ => "here-menu-放弃",
                        },
                        name,
                    )
                    .when(abandon, |d| d.text_color(Group::Error.colors().1))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        fold(this);
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
            foot = foot.child(menu);
        }
        div()
            .id(gpui::SharedString::from(format!(
                "here-card-{}",
                session.id
            )))
            .w(px(T::HERE_WIDTH))
            .max_w_full()
            .max_h(px(T::HERE_HEIGHT))
            .rounded(px(T::POPOVER_RADIUS))
            .bg(T::color(T::popover()))
            .text_color(T::color(T::primary()))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::control()))
            .shadow_lg()
            .flex()
            .flex_col()
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(|_, _, cx| cx.stop_propagation())
            // A click anywhere else puts it away; the defer form it opened still needs it.
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                if this.overview.here.is_some() && this.tasks.form.is_none() {
                    this.overview.here = None;
                    cx.notify();
                }
            }))
            .child(head)
            .child(panel)
            .child(foot)
    }
    /// A Keepline terminal ran it and is closed now; it resumes in a new pane.
    pub(super) fn session_closed(&self, s: &AgentSession) -> bool {
        s.terminal.is_some() && !self.session_open(s)
    }
    /// Its terminal is running in a tab: the sidebar lists only these.
    pub(super) fn session_open(&self, s: &AgentSession) -> bool {
        live_terminal(&self.workspace, s)
            .or_else(|| resume_terminal(&self.workspace, &self.resuming.resumed, s))
            .is_some_and(|t| self.workspace.terminal_tab(&t.id).is_some())
    }
    pub(super) fn session_hidden(&self, s: &AgentSession) -> Option<&str> {
        match self.workspace.content.get(&s.id) {
            Some(c) => c.hidden_for(s),
            // A hook can bind a new conversation before its first prompt writes anything.
            None if s.current && s.binding.is_some() => None,
            None => Some("缺少交互入口证据"),
        }
    }
    pub(super) fn enter_card(
        &mut self,
        s: &AgentSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.overview.card_menu = None;
        self.overview.filter_space = None;
        self.overview.changed_only = false;
        self.overview.closed_only = false;
        self.overview.show_stale =
            session_group(&self.workspace, s) == Group::Stale || self.overview.show_stale;
        self.overview.show_running =
            session_group(&self.workspace, s) == Group::Running || self.overview.show_running;
        if live_terminal(&self.workspace, s).is_some() {
            self.open_session(s, window, cx);
        } else {
            self.nav.overview = true;
            self.sync_panes(cx);
            self.overview.here = Some(s.id.clone());
            cx.notify();
        }
    }
    pub(super) fn session_actions(
        &self,
        session: &AgentSession,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let save = session.clone();
        let defer = session.id.clone();
        let mut menu = div()
            .id(gpui::SharedString::from(format!("actions-{}", session.id)))
            .w(px(T::MENU_WIDTH))
            .p(px(T::S6))
            .bg(T::color(T::raised()))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::control()))
            .rounded(px(T::CONTROL_RADIUS))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap(px(T::S4))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                button("card-save", "存为待办").on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.overview.card_menu = None;
                    this.save_session_task(&save, cx);
                })),
            )
            .child(button("card-defer", "稍后再看").on_click(cx.listener(
                move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.overview.card_menu = None;
                    this.overview.here = Some(defer.clone());
                    this.show_form(FormKind::Defer, window, cx);
                },
            )));
        for (label, outcome) in [
            ("标记完成", Outcome::Completed),
            ("放弃", Outcome::Abandoned),
            ("撤销结局", Outcome::Open),
        ]
        .into_iter()
        .filter(|(_, o)| *o != session.outcome)
        {
            let id = session.id.clone();
            menu = menu.child(
                button(gpui::SharedString::from(format!("card-{label}")), label).on_click(
                    cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.overview.card_menu = None;
                        this.edit(
                            Edit::SetOutcome {
                                id: id.clone(),
                                outcome: outcome.clone(),
                            },
                            cx,
                        );
                    }),
                ),
            );
        }
        menu
    }
    pub fn navigate_to(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.overview.cleanup.clear();
        self.tasks.form = None;
        self.search.hide();
        if let Some(session) = self
            .workspace
            .agent_sessions
            .iter()
            .find(|s| s.id == id)
            .cloned()
        {
            self.enter_card(&session, window, cx);
            // Report actual selected layout and focused pane, not a queued route intent.
            if !self.nav.overview
                && let Some(terminal) = live_terminal(&self.workspace, &session)
                && self.nav.focused.as_deref() == Some(terminal.id.as_str())
                && let Some(tab) = self.workspace.terminal_tab(&terminal.id)
                && self.nav.tab.as_deref() == Some(tab.id.as_str())
                && let Some(pane) = self.layout.panes.get(&terminal.id)
                && pane.focus_handle(cx).is_focused(window)
            {
                crate::desktop::route_applied(id, &terminal.id, &tab.id);
            }
        } else {
            self.error = Some("会话尚未载入，请刷新工作台".into());
            self.request(Request::Workspace, cx);
        }
    }
    /// Only Keepline's own terminals ask for attention; scanned-only sessions stay in the overview.
    pub(super) fn needs_you(&self) -> Vec<&AgentSession> {
        self.workspace
            .agent_sessions
            .iter()
            .filter(|s| {
                self.session_hidden(s).is_none()
                    && !self.session_closed(s)
                    && needs_attention(&self.workspace, s)
            })
            .collect()
    }
    pub(super) fn next_waiting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let needs = self.needs_you();
        let current = self
            .nav
            .focused
            .as_ref()
            .and_then(|id| self.workspace.current_agent(id))
            .map(|s| s.id.as_str())
            .or(self.overview.here.as_deref());
        if !needs.is_empty() {
            let index = needs
                .iter()
                .position(|s| Some(s.id.as_str()) == current)
                .map_or(0, |i| (i + 1) % needs.len());
            let session = needs[index].clone();
            self.enter_card(&session, window, cx);
        }
    }
}
/// The card's receipts by kind: commands that exited 0, file changes, and failures.
/// Test and commit receipts repeat their command's, so they are not counted again.
pub(super) fn evidence_tally(
    evidence: &[keepline_core::content::Evidence],
) -> (usize, usize, usize) {
    let count = |f: &dyn Fn(&keepline_core::content::Evidence) -> bool| {
        evidence.iter().filter(|e| f(e)).count()
    };
    (
        count(&|e| e.kind == "command" && e.exit_code == Some(0)),
        count(&|e| e.kind == "file" && e.exit_code == Some(0)),
        count(&|e| e.kind == "error"),
    )
}
/// 做成了: confirmed receipts, newest first, at most four. A file reads as its path
/// inside the session's directory, once, with how many times it changed.
pub(super) fn done_items(
    evidence: &[keepline_core::content::Evidence],
    cwd: &str,
) -> Vec<(String, usize, bool)> {
    let mut items: Vec<(String, usize, bool)> = Vec::new();
    for e in evidence.iter().rev().filter(|e| e.exit_code == Some(0)) {
        let (text, file) = match e.kind.as_str() {
            "file" => {
                let path = e.summary.trim_start_matches("文件修改：");
                let inside = Path::new(path)
                    .strip_prefix(cwd)
                    .ok()
                    .and_then(|p| p.to_str())
                    .filter(|p| !p.is_empty());
                (inside.unwrap_or(path).to_owned(), true)
            }
            "test" | "commit" => (keepline_core::content::short(&e.summary, 180), false),
            _ => continue,
        };
        match items.iter().position(|(t, _, _)| *t == text) {
            Some(i) => items[i].1 += 1,
            None if items.len() < 4 => items.push((text, 1, file)),
            None => {}
        }
    }
    items
}
