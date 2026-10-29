//! Fixture-driven layouts and pages for native visual evidence.
use super::*;

impl Workbench {
    #[cfg(feature = "visual-test")]
    pub fn visual_layout(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.nav.overview = false;
        self.nav.zoomed = None;
        self.layout.menu = None;
        if name == "eight" {
            let mut leaves = self
                .workspace
                .terminals
                .iter()
                .map(|t| Layout::Leaf(t.id.clone()))
                .collect::<Vec<_>>();
            while leaves.len() > 1 {
                leaves = leaves
                    .chunks(2)
                    .map(|pair| {
                        if pair.len() == 1 {
                            pair[0].clone()
                        } else {
                            Layout::Split {
                                axis: if pair[0].ids().len() == 1 {
                                    Axis::Down
                                } else {
                                    Axis::Right
                                },
                                ratio: 5000,
                                first: Box::new(pair[0].clone()),
                                second: Box::new(pair[1].clone()),
                            }
                        }
                    })
                    .collect();
            }
            if let Some(mut tab) = self
                .workspace
                .tabs
                .iter()
                .find(|t| t.id == "eight")
                .cloned()
            {
                tab.layout = leaves.pop();
                if let Ok(Reply::Workspace(w)) = rpc(
                    self.store.socket(),
                    &Request::Edit {
                        revision: self.workspace.revision,
                        edit: Edit::PutTab { tab },
                    },
                ) {
                    self.workspace = w;
                }
            }
        }
        self.nav.tab = Some(if name == "menu" { "nested" } else { name }.into());
        self.sync_panes(cx);
        if let Some(id) = self
            .nav
            .tab
            .as_ref()
            .and_then(|id| self.workspace.tab_terminals(id).first().cloned())
        {
            if let Some(pane) = self.layout.panes.get(&id) {
                window.focus(&pane.focus_handle(cx), cx);
            }
        }
        cx.notify();
        window.refresh();
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_bounds(&self) -> gpui::Bounds<gpui::Pixels> {
        *self
            .layout
            .split_bounds
            .get(&Vec::new())
            .expect("root splitter bounds")
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_menu_point(&self, index: usize) -> gpui::Point<gpui::Pixels> {
        let menu = self
            .layout
            .menu
            .as_ref()
            .expect("right mouse opened pane menu");
        menu.position + gpui::point(px(T::S20), px(T::S6 + T::ROW * (index as f32 + 0.5)))
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_tab_menu_point(&self, index: usize) -> gpui::Point<gpui::Pixels> {
        let (_, position) = self
            .layout
            .tab_menu
            .as_ref()
            .expect("right mouse opened tab menu");
        *position + gpui::point(px(T::S20), px(T::S6 + T::ROW * (index as f32 + 0.5)))
    }
    /// Whether the tab menu is open, and the rename field's text and focus if any.
    #[cfg(feature = "visual-test")]
    pub fn visual_tab_state(
        &self,
        window: &Window,
        cx: &gpui::App,
    ) -> (bool, Option<(String, bool)>) {
        (
            self.layout.tab_menu.is_some(),
            self.layout.renaming_tab.as_ref().map(|(_, field, _)| {
                (
                    field.read(cx).text.clone(),
                    field.focus_handle(cx).is_focused(window),
                )
            }),
        )
    }
    /// Types into the open rename field as an input method would, caret at the end.
    #[cfg(feature = "visual-test")]
    pub fn visual_rename_type(&mut self, text: &str, cx: &mut Context<Self>) {
        if let Some((_, field, _)) = &self.layout.renaming_tab {
            field.update(cx, |f, cx| {
                f.set_text(text);
                cx.notify();
            });
        }
    }
    /// The name the first tab of this space shows.
    #[cfg(feature = "visual-test")]
    pub fn visual_first_tab_name(&self, cx: &gpui::App) -> String {
        self.workspace
            .tabs
            .iter()
            .find(|t| t.space == self.nav.space)
            .map(|t| self.tab_name(t, cx))
            .unwrap_or_default()
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_new_tab(&mut self, cx: &mut Context<Self>) {
        self.launch(false, None, cx);
    }
    /// Opens a conversation whose resume cannot take (unknown CLI id, no live terminal),
    /// as clicking a 出错了 row does; returns the tab count and the current tab after.
    #[cfg(feature = "visual-test")]
    pub fn visual_open_failed_session(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (usize, Option<String>) {
        let session = AgentSession {
            id: "visual-failed".into(),
            runtime: Agent::Codex,
            cli_session_id: "visual-missing".into(),
            terminal: None,
            current: false,
            task: None,
            outcome: Outcome::Open,
            updated: 1,
            cwd: std::env::temp_dir().display().to_string(),
            transcript: String::new(),
            binding: None,
        };
        self.open_session(&session, window, cx);
        (self.workspace.tabs.len(), self.nav.tab.clone())
    }
    /// The recorded directory of the shown tab's first terminal.
    #[cfg(feature = "visual-test")]
    pub fn visual_shown_cwd(&self) -> String {
        let tab = self.nav.tab.as_ref().expect("a shown tab");
        let id = self.workspace.tab_terminals(tab)[0].clone();
        let terminal = self.workspace.terminals.iter().find(|t| t.id == id).unwrap();
        terminal.cwd.clone()
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_tabs(&self) -> (usize, Option<String>) {
        (self.workspace.tabs.len(), self.nav.tab.clone())
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_check_panes(&self, count: usize) {
        assert_eq!(self.layout.panes.len(), count);
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_check_saved_ratio(&self) {
        let Reply::Workspace(w) = rpc(self.store.socket(), &Request::Workspace).unwrap() else {
            panic!("workspace")
        };
        let local = self
            .workspace
            .tabs
            .iter()
            .find(|t| t.id == "nested")
            .unwrap();
        assert_eq!(w.tabs.iter().find(|t| t.id == "nested").unwrap(), local);
        assert!(
            matches!(local.layout,Some(Layout::Split{ratio,..}) if (6600..6800).contains(&ratio))
        );
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_confirming_close(&self) -> bool {
        self.layout.confirm_close.is_some()
    }
    /// The dialog's 取消.
    #[cfg(feature = "visual-test")]
    pub fn visual_cancel_close(&mut self, cx: &mut Context<Self>) {
        self.layout.confirm_close = None;
        cx.notify();
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_split_down(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.nav.focused.clone().unwrap();
        self.menu_action(&id, "向下分屏", window, cx);
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_close_focused(&mut self, cx: &mut Context<Self>) {
        let id = self.nav.focused.clone().unwrap();
        self.close_pane(&id, cx);
    }
    /// Closes the shown tab, as its × does; the neighbour that takes over is returned.
    #[cfg(feature = "visual-test")]
    pub fn visual_close_tab(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let id = self.nav.tab.clone().unwrap();
        self.close_tab(&id, cx);
        let tab = self.nav.tab.clone()?;
        for id in attachable_ids(&self.workspace, self.workspace.tab_terminals(&tab)) {
            assert!(
                self.layout.panes.contains_key(&id),
                "the neighbour's panes attach as soon as it takes over"
            );
        }
        Some(tab)
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_assert_new_shell(&self, count: usize, window: &Window, cx: &gpui::App) {
        self.visual_check_panes(count);
        let id = self.nav.focused.as_ref().unwrap();
        assert!(
            self.layout.panes[id].focus_handle(cx).is_focused(window),
            "new split must receive input focus"
        );
        let terminal = self
            .workspace
            .terminals
            .iter()
            .find(|t| &t.id == id)
            .unwrap();
        assert_eq!(terminal.argv, vec!["/bin/sh", "-l"]);
        assert_eq!(
            std::fs::canonicalize(&terminal.cwd).unwrap(),
            std::fs::canonicalize(&self.workspace.terminals[0].cwd).unwrap()
        );
        assert!(self.workspace.agent_sessions.is_empty());
    }
    #[cfg(feature = "visual-test")]
    /// Of the two shells this run opened, `closed` were closed and must have stopped.
    pub fn visual_check_closed_processes(&self, closed: usize) {
        let extras: Vec<_> = self
            .workspace
            .terminals
            .iter()
            .filter(|t| !t.id.starts_with('p'))
            .collect();
        assert_eq!(extras.len(), 2);
        let stopped = extras
            .iter()
            .filter(|t| {
                t.state != ProcessState::Running
                    && unsafe { libc::kill(t.pid.unwrap() as i32, 0) } != 0
            })
            .count();
        assert_eq!(stopped, closed, "closing a pane stops its terminal");
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_marked_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Bounds<gpui::Pixels>> {
        if let Some(pane) = self
            .nav
            .tab
            .as_ref()
            .and_then(|id| self.workspace.tab_terminals(id).first().cloned())
            .and_then(|id| self.layout.panes.get(&id))
        {
            return Some(pane.update(cx, |pane, cx| pane.verify_marked_overlay(window, cx)));
        }
        None
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_view(&mut self, overview: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.nav.overview = overview;
        self.sync_panes(cx);
        if !overview {
            if let Some(pane) = self
                .nav
                .tab
                .as_ref()
                .and_then(|id| self.workspace.tab_terminals(id).first().cloned())
                .and_then(|id| self.layout.panes.get(&id))
            {
                window.focus(&pane.focus_handle(cx), cx);
            }
        }
        cx.notify();
        window.refresh();
    }
    /// The first pane of the shown tab.
    #[cfg(feature = "visual-test")]
    pub fn visual_shown_pane(&self) -> Entity<Pane> {
        let tab = self.nav.tab.as_ref().expect("a shown tab");
        let id = self
            .workspace
            .tab_terminals(tab)
            .first()
            .cloned()
            .expect("a pane");
        self.layout.panes[&id].clone()
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_pane_menu_open(&self) -> bool {
        self.layout.menu.is_some()
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_here_open(&self) -> bool {
        self.overview.here.is_some()
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_restart_host(&mut self, cx: &mut Context<Self>) {
        self.close_now(Closing::Host, cx);
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_page(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.tasks.form = None;
        self.tasks.quick_task = None;
        self.tasks.picking = None;
        self.nav.task_detail = None;
        self.overview.cleanup.clear();
        self.overview.here = None;
        self.overview.here_more = None;
        self.search.hide();
        self.nav.overview = matches!(name, "Overview" | "Closed");
        self.overview.closed_only = name == "Closed";
        self.nav.zoomed = None;
        self.nav.space = "s0".into();
        self.nav.tab = Some(
            if name == "TodoStart" {
                "start"
            } else {
                "feedback"
            }
            .into(),
        );
        self.overview.show_running = false;
        self.sync_panes(cx);
        match name {
            "Main" | "HereClickOut" => {
                self.overview.here = self.workspace.current_agent("p0").map(|s| s.id.clone())
            }
            // The sidebar with the space's todos unfolded above its sessions.
            "SidebarTodos" => self.tasks.todos_open = true,
            // The card open on the focused waiting pane; the harness then types a key.
            "HereTyped" => {
                self.overview.here = self.workspace.current_agent("p0").map(|s| s.id.clone());
                self.nav.focused = Some("p0".into());
                if let Some(pane) = self.layout.panes.get("p0") {
                    window.focus(&pane.focus_handle(cx), cx);
                }
            }
            // The close dialog over a waiting Agent, and over one whose turn is under way.
            // The top bar's entry for an older host, and its restart dialog.
            "RestartHost" => {
                crate::lifecycle::visual_stale_host();
                self.layout.confirm_close = Some(Closing::Host);
            }
            "CloseIdle" | "CloseWorking" => {
                if name == "CloseWorking"
                    && let Some(s) = self.workspace.current_agent("p0").map(|s| s.id.clone())
                    && let Some(c) = self.workspace.content.get_mut(&s)
                {
                    c.turn = keepline_core::content::Turn::Running;
                }
                self.layout.confirm_close = Some(Closing::Pane("p0".into()));
            }
            // An Agent reply in Markdown, as the pane header shows it.
            "Markdown" => {
                for c in self.workspace.content.values_mut() {
                    if c.turn == keepline_core::content::Turn::Completed {
                        c.last_reply =
                            "**建议都接收**，见 `a.rs`\n\n| 投稿 | 建议处理 |\n|---|---|\n| 3 | 接收 |"
                                .into();
                    }
                }
            }
            "SelectHint" => {
                if let Some(pane) = self.layout.panes.get("p0") {
                    pane.update(cx, |p, cx| p.visual_select_hint(cx));
                }
            }
            "HereMore" => {
                self.overview.here = self.workspace.current_agent("p0").map(|s| s.id.clone());
                self.overview.here_more = self.overview.here.clone();
            }
            "TodoNew" => {
                self.show_form(FormKind::Task, window, cx);
                let task = self
                    .workspace
                    .tasks
                    .iter()
                    .find(|t| t.id == "task1")
                    .unwrap();
                for (field, text) in self.tasks.form.as_ref().unwrap().fields.iter().zip([
                    task.title.clone(),
                    task.description.clone(),
                    task.acceptance
                        .iter()
                        .map(|a| a.description.clone())
                        .collect::<Vec<_>>()
                        .join("\n"),
                ]) {
                    field.update(cx, |field, cx| {
                        field.set_text(&text);
                        cx.notify();
                    });
                }
            }
            "TodoDetail" => self.nav.task_detail = Some("task0".into()),
            "TodoStart" => {
                let text = self
                    .workspace
                    .tasks
                    .iter()
                    .find(|t| t.id == "task1")
                    .unwrap()
                    .prompt();
                let command = self
                    .workspace
                    .tasks
                    .iter()
                    .find(|t| t.id == "task1")
                    .unwrap()
                    .agent
                    .command();
                self.layout.panes["pstart"]
                    .update(cx, |pane, cx| pane.start_agent(&command, Some(text), cx));
            }
            "Cleanup" => {
                self.overview.cleanup = self
                    .workspace
                    .agent_sessions
                    .iter()
                    .filter(|s| session_group(&self.workspace, s) == Group::Stale)
                    .map(|s| s.id.clone())
                    .collect();
                self.overview.cleanup_index = 0;
                assert!(!self.overview.cleanup.is_empty());
            }
            "QuickTask" => {
                self.show_quick_task(window, cx);
                if let Some((field, _)) = &self.tasks.quick_task {
                    field.update(cx, |f, cx| {
                        f.set_text("给离线模式补一组断网回归测试");
                        cx.notify();
                    });
                }
            }
            "ScreenWait" => {
                self.overview.here = self.workspace.current_agent("p1").map(|s| s.id.clone())
            }
            "StartPick" => self.tasks.picking = Some(("task2".into(), false)),
            // The logo's menu, then the menu after choosing the mascot from it.
            "IconMenu" => {
                self.layout.icon_menu = Some(gpui::point(px(T::TRAFFIC_LIGHT_INSET), px(T::TOP)))
            }
            "IconMascot" => {
                layout::choose_icon(AppIcon::Mascot, cx);
                self.layout.icon_menu = Some(gpui::point(px(T::TRAFFIC_LIGHT_INSET), px(T::TOP)));
            }
            "SearchEmpty" | "Search" => {
                self.open_search(window, cx);
                if name == "Search" {
                    let query =
                        std::env::var("KEEPLINE_SEARCH_QUERY").unwrap_or_else(|_| "反馈".into());
                    self.search.field().update(cx, |f, cx| {
                        f.set_text(&query);
                        cx.notify();
                    });
                }
            }
            // Two sessions whose terminal is gone, as after closing their panes.
            "Closed" => {
                for s in self.workspace.agent_sessions.iter_mut().take(2) {
                    s.terminal = Some("closed-visual".into());
                }
            }
            "Overview" | "Menubar" => (),
            _ => panic!("unknown visual page"),
        }
        cx.notify();
        window.refresh();
    }
    /// The overview toggle's centre, as last drawn.
    #[cfg(feature = "visual-test")]
    pub fn visual_toggle_point(&self) -> gpui::Point<gpui::Pixels> {
        self.overview.toggle_at.expect("toggle drawn").center()
    }
    /// Overview shown, the tab in view, the space filter, and who holds the keyboard:
    /// the overview itself or a pane of the shown tab.
    #[cfg(feature = "visual-test")]
    pub fn visual_overview_state(
        &self,
        window: &Window,
        cx: &gpui::App,
    ) -> (bool, Option<String>, Option<String>, &'static str) {
        let focus = if self.overview.focus.is_focused(window) {
            "overview"
        } else if self
            .layout
            .panes
            .values()
            .any(|p| p.focus_handle(cx).contains_focused(window, cx))
        {
            "pane"
        } else {
            "none"
        };
        (
            self.nav.overview,
            self.nav.tab.clone(),
            self.overview.filter_space.clone(),
            focus,
        )
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_spaces(&self) -> Vec<String> {
        self.workspace.spaces.iter().map(|s| s.id.clone()).collect()
    }
}
