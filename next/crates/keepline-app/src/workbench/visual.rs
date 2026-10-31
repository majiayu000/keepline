//! Fixture-driven layouts and pages for native visual evidence.
use super::*;

/// The space menu's state, the rename field's text and focus, each space's name and cwd.
#[cfg(feature = "visual-test")]
pub type VisualSpaceState = (bool, Option<(String, bool)>, Vec<(String, String)>);
impl Workbench {
    /// The host's socket, whether its connection strip shows, and 「重新连接」 itself.
    #[cfg(feature = "visual-test")]
    pub fn visual_socket(&self) -> PathBuf {
        self.store.socket().to_owned()
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_cut_off(&self) -> bool {
        self.store.cut_off()
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_reconnect(&mut self, cx: &mut Context<Self>) {
        self.reconnect(cx);
    }
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
            && let Some(pane) = self.layout.panes.get(&id)
        {
            window.focus(&pane.focus_handle(cx), cx);
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
            name: String::new(),
        };
        self.open_session(&session, window, cx);
        (self.workspace.tabs.len(), self.nav.tab.clone())
    }
    /// The recorded directory of the shown tab's first terminal.
    #[cfg(feature = "visual-test")]
    pub fn visual_shown_cwd(&self) -> String {
        let tab = self.nav.tab.as_ref().expect("a shown tab");
        let id = self.workspace.tab_terminals(tab)[0].clone();
        let terminal = self
            .workspace
            .terminals
            .iter()
            .find(|t| t.id == id)
            .unwrap();
        terminal.cwd.clone()
    }
    /// The settings panel when open: (recording, a taken chord waiting, panel focused).
    #[cfg(feature = "visual-test")]
    pub fn visual_settings(&self, window: &Window, cx: &gpui::App) -> Option<(bool, bool, bool)> {
        let view = self.settings.as_ref()?.read(cx);
        Some((
            view.recording.is_some(),
            view.taken.is_some(),
            view.focus.is_focused(window),
        ))
    }
    /// Starts recording a chord for the action named as in keys.json, as its key button does.
    #[cfg(feature = "visual-test")]
    pub fn visual_settings_record(&mut self, name: &str, cx: &mut Context<Self>) {
        let action = crate::keymap::Keymap::actions()
            .find(|a| a.name() == name)
            .unwrap();
        let view = self.settings.clone().expect("settings open");
        view.update(cx, |v, cx| v.record(action, cx));
    }
    /// 取消 under a taken chord, or 全部恢复默认.
    #[cfg(feature = "visual-test")]
    pub fn visual_settings_press(&mut self, button: &str, cx: &mut Context<Self>) {
        let view = self.settings.clone().expect("settings open");
        view.update(cx, |v, cx| match button {
            "取消" => {
                v.taken = None;
                cx.notify();
            }
            "全部恢复默认" => v.reset(None, cx),
            _ => panic!("no button {button}"),
        });
    }
    /// Whether a terminal pane holds input focus.
    #[cfg(feature = "visual-test")]
    pub fn visual_pane_focused(&self, window: &Window, cx: &gpui::App) -> bool {
        self.layout
            .panes
            .values()
            .any(|p| p.focus_handle(cx).is_focused(window))
    }
    /// The shown tab's panes in layout order and the one whose handle really holds
    /// input focus (not just the one the workbench remembers).
    #[cfg(feature = "visual-test")]
    pub fn visual_pane_focus(
        &self,
        window: &Window,
        cx: &gpui::App,
    ) -> (Vec<String>, Option<String>) {
        let order = self
            .nav
            .tab
            .as_ref()
            .map(|t| self.workspace.tab_terminals(t))
            .unwrap_or_default();
        let focused = order
            .iter()
            .find(|id| {
                self.layout
                    .panes
                    .get(*id)
                    .is_some_and(|p| p.focus_handle(cx).is_focused(window))
            })
            .cloned();
        assert_eq!(
            focused.is_some(),
            focused == self.nav.focused,
            "the remembered pane is the focused one"
        );
        (order, focused)
    }
    /// The current space's tabs in strip order, the shown one, and its pane count.
    #[cfg(feature = "visual-test")]
    pub fn visual_tab_row(&self) -> (Vec<String>, Option<String>, usize) {
        let row = self
            .workspace
            .tabs
            .iter()
            .filter(|t| t.space == self.nav.space)
            .map(|t| t.id.clone())
            .collect();
        let panes = self
            .nav
            .tab
            .as_ref()
            .map_or(0, |t| self.workspace.tab_terminals(t).len());
        (row, self.nav.tab.clone(), panes)
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
        if !overview
            && let Some(pane) = self
                .nav
                .tab
                .as_ref()
                .and_then(|id| self.workspace.tab_terminals(id).first().cloned())
                .and_then(|id| self.layout.panes.get(&id))
        {
            window.focus(&pane.focus_handle(cx), cx);
        }
        cx.notify();
        window.refresh();
    }
    /// The first pane of the shown tab.
    #[cfg(feature = "visual-test")]
    pub fn visual_decorations(&self, cx: &Context<Self>) -> (Vec<String>, Vec<String>) {
        let tab = self.nav.tab.as_ref().expect("a shown tab");
        let id = self.workspace.tab_terminals(tab)[0].clone();
        let lines = self.pane_decorations(&id, cx);
        (lines.title, lines.tip)
    }
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
        self.tasks.attaching = None;
        self.tasks.expanded.clear();
        self.overview.cleanup.clear();
        self.overview.here = None;
        self.overview.here_more = None;
        self.search.hide();
        self.nav.overview = matches!(name, "Overview" | "OverviewEnd" | "Closed");
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
            // The filled form, the line being written last; the login shell's answer is
            // fixed so the run does not depend on what this machine has installed.
            "TodoNew" | "TodoNewBare" | "TodoNewChecks" => {
                let all: HashSet<String> = Agent::all()
                    .into_iter()
                    .map(|a| a.executable().to_owned())
                    .collect();
                self.tasks.found = Some(all.clone());
                self.show_form(FormKind::Task, window, cx);
                let mut task = self
                    .workspace
                    .tasks
                    .iter()
                    .find(|t| t.id == "task0")
                    .cloned()
                    .unwrap();
                match name {
                    "TodoNewBare" => task.acceptance.clear(),
                    // Every check at once: no title after a save, a repeated line, a space
                    // whose folder is gone and an Agent the shell cannot find.
                    "TodoNewChecks" => {
                        task.title.clear();
                        let repeat = task.acceptance[1].description.clone();
                        task.acceptance.push(AcceptanceCriterion {
                            id: "repeat".into(),
                            description: repeat,
                            evidence: vec![],
                        });
                        if let Some(gone) = self
                            .workspace
                            .spaces
                            .iter()
                            .find(|s| !Path::new(&s.cwd).is_dir())
                        {
                            task.space = gone.id.clone();
                        }
                        if let Some(missing) = Agent::all().into_iter().last() {
                            task.agent = missing;
                            self.tasks.found = Some(
                                all.into_iter()
                                    .filter(|e| e != missing.executable())
                                    .collect(),
                            );
                        }
                    }
                    _ => (),
                }
                self.fill_task_form(&task, window, cx);
                if let Some(form) = &mut self.tasks.form {
                    form.tried = name == "TodoNewChecks";
                    if let Some(last) = form.criteria.last() {
                        window.focus(&last.field.focus_handle(cx), cx);
                    }
                }
            }
            "TodoDetail" | "TodoDetailAttach" | "TodoDetailFull" => {
                self.visual_attach_evidence(name == "TodoDetailFull", cx);
                self.nav.task_detail = Some("task0".into());
                if name == "TodoDetailAttach" {
                    self.tasks.attaching = self
                        .workspace
                        .tasks
                        .iter()
                        .find(|t| t.id == "task0")
                        .and_then(|t| t.acceptance.get(1))
                        .map(|c| c.id.clone());
                }
            }
            // A todo settled without criteria or sessions.
            "TodoDetailDone" => {
                self.edit(
                    Edit::SetTaskOutcome {
                        id: "task2".into(),
                        outcome: Outcome::Completed,
                    },
                    cx,
                );
                self.nav.task_detail = Some("task2".into());
            }
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
            // The left column's presets, set in memory so the evidence run writes no choice.
            "SidebarClassic" | "SidebarStatus" | "SidebarStatusOpen" | "SidebarDirectory"
            | "SidebarRail" | "SidebarWasm" | "SidebarWasmOpen" | "SidebarWasmClosed" => {
                self.sidebar.preset = match name {
                    "SidebarStatus" | "SidebarStatusOpen" => "status",
                    "SidebarDirectory" => "directory",
                    // The example WebAssembly plugin, installed into the scratch config.
                    "SidebarWasm" | "SidebarWasmOpen" | "SidebarWasmClosed" => "wasm-status",
                    _ => "classic",
                }
                .into();
                self.sidebar.collapsed = name == "SidebarRail";
                self.sidebar.toggled.clear();
                if name == "SidebarStatusOpen" {
                    self.sidebar.toggled.insert("status:rest".into());
                }
                if name == "SidebarWasmOpen" || name == "SidebarWasmClosed" {
                    for section in ["rest", "closed"] {
                        self.sidebar
                            .toggled
                            .insert(format!("wasm:wasm-status:{section}"));
                    }
                }
                // Closes the tab's last pane, a finished Agent, so the plugin has a closed
                // session to place. Run it last: the fixture keeps the pane closed.
                if name == "SidebarWasmClosed"
                    && let Some(last) = self.workspace.tab_terminals("feedback").pop()
                {
                    self.close_now(Closing::Pane(last), cx);
                }
            }
            // The two looks, set in memory so the evidence run writes no choice.
            "StyleStandard" | "StyleTerminal" => {
                use crate::ui::style::Style;
                cx.set_global(if name == "StyleTerminal" {
                    Style::Terminal
                } else {
                    Style::Standard
                });
                self.nav.focused = Some("p0".into());
            }
            "PaneMenu" => {
                self.nav.focused = Some("p0".into());
                self.layout.menu = Some(PaneMenu {
                    terminal: "p0".into(),
                    position: gpui::point(px(T::WINDOW_WIDTH / 2.), px(T::TOP * 3.)),
                    page: MenuPage::Main,
                });
            }
            // Runs the first action a real pane offers, as a click in its menu would.
            "ActionRun" => {
                let target = self.workspace.tabs.iter().find_map(|tab| {
                    let id = self.workspace.tab_terminals(&tab.id).first()?.clone();
                    Some((tab.space.clone(), tab.id.clone(), id))
                });
                if let Some((space, tab, terminal)) = target {
                    self.nav.space = space;
                    self.nav.tab = Some(tab);
                    self.nav.focused = Some(terminal.clone());
                    self.sync_panes(cx);
                    if let Some(action) = self.actions_for(&terminal).first().copied() {
                        self.run_action(&terminal, action, cx);
                    }
                }
            }
            // The palette empty, narrowed to 出错, with a query, and with one that finds
            // nothing but whose halves do.
            "SearchEmpty" | "SearchGroup" | "Search" | "SearchNone" => {
                self.nav.focused = Some("p0".into());
                self.open_search(window, cx);
                if name == "SearchGroup" {
                    self.search.pick_group(Some(Group::Error));
                }
                if name == "Search" || name == "SearchNone" {
                    let query = match name {
                        "SearchNone" => "反馈报销".into(),
                        _ => {
                            std::env::var("KEEPLINE_SEARCH_QUERY").unwrap_or_else(|_| "反馈".into())
                        }
                    };
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
            // The overview's foot: the folded groups below the cards.
            "OverviewEnd" => self.overview.scroll.scroll_to_bottom(),
            "Overview" | "Menubar" => (),
            // `Dense-{status|directory|wasm}-{standard|terminal}-{light|dark}[-open|-menu]`:
            // the denser presets over sessions shaped like sidebar-density.html's.
            dense if dense.starts_with("Dense-") => {
                use crate::ui::{appearance::Appearance, style::Style};
                let parts: Vec<&str> = dense.split('-').collect();
                self.visual_dense_fixture();
                self.sidebar.preset = match parts[1] {
                    "wasm" => "wasm-status",
                    other => other,
                }
                .into();
                self.sidebar.collapsed = false;
                self.sidebar.toggled.clear();
                if parts.get(4) == Some(&"open") {
                    for key in ["status:rest", "wasm:wasm-status:rest"] {
                        self.sidebar.toggled.insert(key.into());
                    }
                }
                cx.set_global(if parts[2] == "terminal" {
                    Style::Terminal
                } else {
                    Style::Standard
                });
                let _ = crate::ui::appearance::choose(
                    if parts[3] == "dark" {
                        Appearance::Dark
                    } else {
                        Appearance::Light
                    },
                    cx,
                );
                self.nav.focused = Some("dense-t2".into());
                self.layout.space_menu = (parts.get(4) == Some(&"menu"))
                    .then(|| ("s0".into(), gpui::point(px(T::S20), px(T::TOP * 2.))));
                self.layout.space_switch = true;
            }
            _ => panic!("unknown visual page"),
        }
        cx.notify();
        window.refresh();
    }
    /// Sessions in space s0 shaped like sidebar-density.html's: two need you (one asks,
    /// one failed), two run, four are done, three stopped and a few went quiet, over five
    /// folders. Kept in memory only, in a tab of their own, and the revision is pushed
    /// past the host's so its snapshots leave them be.
    #[cfg(feature = "visual-test")]
    fn visual_dense_fixture(&mut self) {
        use keepline_core::content::{Content, Evidence, Quote, Source, Turn};
        if self.workspace.terminals.iter().any(|t| t.id == "dense-t0") {
            return;
        }
        let Some(template) = self.workspace.agent_sessions.first().cloned() else {
            return;
        };
        let Some(shell) = self.workspace.terminals.first().cloned() else {
            return;
        };
        // Only these sessions in the column: the fixture's own step aside.
        for c in self.workspace.content.values_mut() {
            c.hidden_reason = "visual".into();
        }
        let home = std::env::var("HOME").unwrap_or_default();
        let dir = |d: &str| match d {
            "knowledge-base" => format!("{home}/Documents/knowledge-base"),
            "grok-build" => format!("{home}/Desktop/code/AI/grok-build"),
            d => format!("{home}/Desktop/code/AI/tools/{d}"),
        };
        let now = now();
        // (title, folder, turn, live, seconds ago, what it says)
        let rows: Vec<(&str, &str, Turn, bool, u64, &str)> = vec![
            (
                "这个终止 agent 中断取消",
                "keepline",
                Turn::Error,
                false,
                240,
                "managed_session 测试超时：timed out waiting for Settled after 30s",
            ),
            (
                "这个目录的 grok 的聊天记录",
                "grok-build",
                Turn::Waiting,
                true,
                720,
                "找不到 grok 的会话目录，要我去 ~/.grok 下面再找一遍吗？",
            ),
            (
                "这个诊断的 ui 感觉很丑啊",
                "keepline",
                Turn::Running,
                true,
                2467,
                "S=/private/tmp/claude-502/-Users-lifcc-Desktop; cd \"$S\" && cargo build -p keepline-app 2>&1 | tail -20",
            ),
            (
                "标签命名依旧有问题",
                "keepline",
                Turn::Running,
                true,
                192,
                "文件修改：next/crates/keepline-app/src/workbench/tabs.rs",
            ),
            (
                "左侧的会话 有什么好的命名",
                "keepline",
                Turn::Completed,
                true,
                7200,
                "标签用第一句话命名",
            ),
            (
                "这个 keepline 的 gpu 占用",
                "keepline",
                Turn::Completed,
                true,
                10800,
                "只重画变化的区域",
            ),
            (
                "为什么有的会话 在 keepline 里",
                "cli",
                Turn::Completed,
                true,
                18000,
                "外部会话标为只读",
            ),
            (
                "cli 是否支持 workflow 呢",
                "cli",
                Turn::Completed,
                true,
                86400,
                "说明了 workflow 命令",
            ),
            (
                "我准备 重启一下 mac keepline",
                "keepline",
                Turn::Interrupted,
                false,
                7200,
                "等你确认是否重启",
            ),
            (
                "这个库 有什么可以优化的",
                "bench",
                Turn::Interrupted,
                false,
                21600,
                "列了 12 条，没开始改",
            ),
            (
                "看这里 选中一个内容后",
                "bench",
                Turn::Interrupted,
                false,
                86400,
                "复现了选区跳动",
            ),
            (
                "整理知识库目录",
                "knowledge-base",
                Turn::Completed,
                false,
                40 * 86400,
                "整理好了",
            ),
            (
                "知识库去重",
                "knowledge-base",
                Turn::Completed,
                false,
                41 * 86400,
                "去掉了 30 条",
            ),
            (
                "bench 跑分脚本",
                "bench",
                Turn::Completed,
                false,
                50 * 86400,
                "加了脚本",
            ),
        ];
        let mut layout: Option<keepline_core::layout::Layout> = None;
        for (i, (title, folder, turn, live, ago, says)) in rows.into_iter().enumerate() {
            let (sid, tid) = (format!("dense-s{i}"), format!("dense-t{i}"));
            let at = now - ago;
            self.workspace.terminals.push(Terminal {
                id: tid.clone(),
                space: "s0".into(),
                cwd: dir(folder),
                ..shell.clone()
            });
            self.workspace.agent_sessions.push(AgentSession {
                id: sid.clone(),
                cli_session_id: sid.clone(),
                terminal: Some(tid.clone()),
                current: live,
                task: None,
                outcome: Outcome::Open,
                updated: at,
                cwd: dir(folder),
                name: title.into(),
                ..template.clone()
            });
            if !live {
                self.resuming.resumed.insert(sid.clone(), tid.clone());
            }
            let record = |kind: &str, summary: &str| Evidence {
                kind: kind.into(),
                summary: summary.into(),
                exit_code: None,
                at: now - 20,
                source: Source::default(),
            };
            self.workspace.content.insert(
                sid,
                Content {
                    interactive: true,
                    first_ask_known: true,
                    title: title.into(),
                    asks: vec![Quote {
                        text: title.into(),
                        at,
                        source: Source::default(),
                    }],
                    turn,
                    turn_started: at,
                    updated: if turn == Turn::Running { now } else { at },
                    waiting: if turn == Turn::Waiting {
                        says.into()
                    } else {
                        String::new()
                    },
                    error: if turn == Turn::Error {
                        says.into()
                    } else {
                        String::new()
                    },
                    completed_claim: turn == Turn::Completed,
                    last_reply: if matches!(turn, Turn::Completed | Turn::Interrupted) {
                        says.into()
                    } else {
                        String::new()
                    },
                    evidence: match turn {
                        Turn::Running if says.starts_with("文件修改") => {
                            vec![record("file", says)]
                        }
                        Turn::Running => vec![record("command", says)],
                        _ => vec![],
                    },
                    ..Default::default()
                },
            );
            let leaf = keepline_core::layout::Layout::Leaf(tid);
            layout = Some(match layout {
                None => leaf,
                Some(first) => keepline_core::layout::Layout::Split {
                    axis: keepline_core::layout::Axis::Down,
                    ratio: 5000,
                    first: Box::new(first),
                    second: Box::new(leaf),
                },
            });
        }
        // Two closed since: their terminals are gone, so they count under 已关闭.
        for i in 0..2 {
            let sid = format!("dense-closed-{i}");
            self.workspace.agent_sessions.push(AgentSession {
                id: sid.clone(),
                cli_session_id: sid.clone(),
                terminal: Some(format!("dense-gone-{i}")),
                current: false,
                task: None,
                outcome: Outcome::Open,
                updated: now - 3600,
                // No terminal left to place it: the space is found by its folder.
                cwd: self
                    .workspace
                    .spaces
                    .iter()
                    .find(|s| s.id == "s0")
                    .map(|s| s.cwd.clone())
                    .unwrap_or_default(),
                name: format!("已关闭的会话 {i}"),
                ..template.clone()
            });
            self.workspace.content.insert(
                sid,
                Content {
                    interactive: true,
                    turn: Turn::Completed,
                    turn_started: now - 3600,
                    updated: now - 3600,
                    ..Default::default()
                },
            );
        }
        self.workspace.tabs.push(Tab {
            id: "dense".into(),
            space: "s0".into(),
            default_task: None,
            title: "左栏样例".into(),
            layout,
        });
        self.workspace.revision = u64::MAX / 2;
    }
    /// Attaches the fixture sessions' own records to task0's criteria through a real
    /// edit: a passing test and a file change on the first, a failed test on the third;
    /// with `full`, passing records on all three.
    #[cfg(feature = "visual-test")]
    fn visual_attach_evidence(&mut self, full: bool, cx: &mut Context<Self>) {
        let Some(mut task) = self
            .workspace
            .tasks
            .iter()
            .find(|t| t.id == "task0")
            .cloned()
        else {
            return;
        };
        let records = super::task_detail::attachable(&self.workspace, "task0", None);
        let find = |needle: &str, ok: bool| {
            records
                .iter()
                .find(|e| e.record.summary.contains(needle) && e.record.proves() == ok)
                .cloned()
        };
        let mut sets: Vec<Vec<Option<CriterionEvidence>>> = vec![
            vec![find("tab_title：", true), find("tabs.rs", true)],
            vec![],
            vec![find("-p keepline-app：", false)],
        ];
        if full {
            sets[1] = vec![
                find("tab_title_survives_exit", true),
                find("model.rs", true),
            ];
            sets[2].push(find("213 passed", true));
        }
        for (c, set) in task.acceptance.iter_mut().zip(sets) {
            c.evidence = set.into_iter().flatten().collect();
        }
        self.edit(Edit::UpdateTask(task), cx);
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
    /// Tab titles in strip order for the shown space, and the spaces left.
    #[cfg(feature = "visual-test")]
    pub fn visual_tab_titles(&self) -> Vec<String> {
        self.workspace
            .tabs
            .iter()
            .filter(|t| t.space == self.nav.space)
            .map(|t| t.title.clone())
            .collect()
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_page_tab(&mut self, title: &str, _: &mut Window, cx: &mut Context<Self>) {
        let tab = self
            .workspace
            .tabs
            .iter()
            .find(|t| t.title == title)
            .expect("tab")
            .id
            .clone();
        self.nav.tab = Some(tab);
        self.nav.focused = None;
        self.sync_panes(cx);
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_move_pane(&mut self, title: &str, cx: &mut Context<Self>) {
        let pane = self.nav.focused.clone().expect("a focused pane");
        let to = self
            .workspace
            .tabs
            .iter()
            .find(|t| t.title == title)
            .map(|t| t.id.clone());
        self.move_pane(&pane, to.as_deref(), cx);
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_tab_layout(&self, title: &str) -> usize {
        self.workspace
            .tabs
            .iter()
            .find(|t| t.title == title)
            .map_or(0, |t| self.workspace.tab_terminals(&t.id).len())
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_open_space_menu(&mut self, id: &str, cx: &mut Context<Self>) {
        self.layout.space_menu =
            Some((id.into(), gpui::point(px(T::S20), px(T::TOP + 4. * T::ROW))));
        self.layout.space_removing = false;
        cx.notify();
    }
    /// Names this space's tabs in strip order, for scripts that address them by name.
    #[cfg(feature = "visual-test")]
    pub fn visual_name_tabs(&mut self, names: &[&str], cx: &mut Context<Self>) {
        let tabs: Vec<_> = self
            .workspace
            .tabs
            .iter()
            .filter(|t| t.space == self.nav.space)
            .cloned()
            .collect();
        for (tab, name) in tabs.into_iter().zip(names) {
            self.edit(
                Edit::PutTab {
                    tab: Tab {
                        title: (*name).into(),
                        ..tab
                    },
                },
                cx,
            );
        }
    }
    /// Adds a space with a fixed id beside the current one.
    #[cfg(feature = "visual-test")]
    pub fn visual_add_space(&mut self, id: &str, cwd: &str, cx: &mut Context<Self>) {
        self.edit(
            Edit::AddSpace(Space {
                id: id.into(),
                name: id.into(),
                cwd: cwd.into(),
            }),
            cx,
        );
    }
    /// Whether the pane menu shows 「为什么是这个状态」.
    #[cfg(feature = "visual-test")]
    pub fn visual_menu_why(&self) -> bool {
        self.layout
            .menu
            .as_ref()
            .is_some_and(|m| matches!(m.page, MenuPage::Why))
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_spaces(&self) -> Vec<String> {
        self.workspace.spaces.iter().map(|s| s.id.clone()).collect()
    }
    /// Whether the space menu is open, the rename field's text and focus if any, and
    /// each space's name and directory.
    #[cfg(feature = "visual-test")]
    pub fn visual_space_state(&self, window: &Window, cx: &gpui::App) -> VisualSpaceState {
        (
            self.layout.space_menu.is_some(),
            self.layout.renaming_space.as_ref().map(|(_, field, _)| {
                (
                    field.read(cx).text.clone(),
                    field.focus_handle(cx).is_focused(window),
                )
            }),
            self.workspace
                .spaces
                .iter()
                .map(|s| (s.name.clone(), s.cwd.clone()))
                .collect(),
        )
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_space_menu_point(&self, index: usize) -> gpui::Point<gpui::Pixels> {
        let (_, position) = self
            .layout
            .space_menu
            .as_ref()
            .expect("right mouse opened space menu");
        *position + gpui::point(px(T::S20), px(T::S6 + T::ROW * (index as f32 + 0.5)))
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_space_rename_type(&mut self, text: &str, cx: &mut Context<Self>) {
        if let Some((_, field, _)) = &self.layout.renaming_space {
            field.update(cx, |f, cx| {
                f.set_text(text);
                cx.notify();
            });
        }
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_rename_first_space(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.workspace.spaces.first().map(|s| s.id.clone()) {
            self.rename_space(&id, window, cx);
        }
    }
    /// Points the first space at a folder that is gone, as a deleted worktree does.
    #[cfg(feature = "visual-test")]
    pub fn visual_lose_space_dir(&mut self, cx: &mut Context<Self>) {
        if let Some(space) = self.workspace.spaces.first() {
            let cwd = "/nonexistent/keepline-space-evidence".into();
            self.edit(
                Edit::UpdateSpace(Space {
                    cwd,
                    ..space.clone()
                }),
                cx,
            );
        }
    }
}
