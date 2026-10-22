use crate::{
    field::Field,
    pane::Pane,
    theme::{Group, Theme as T},
};
use gpui::{Context, Entity, Focusable, Window, div, prelude::*, px};
use keepline_core::{model::*, protocol::*};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub struct Workbench {
    since: u64,
    away: Option<u64>,
    filter_space: Option<String>,
    changed_only: bool,
    search_open: bool,
    search: Entity<Field>,
    socket: PathBuf,
    workspace: Workspace,
    space: String,
    tab: Option<String>,
    panes: HashMap<String, Entity<Pane>>,
    focused: Option<String>,
    pending_focus: bool,
    zoomed: Option<String>,
    menu: Option<PaneMenu>,
    split_target: Option<(String, Axis)>,
    chosen_agent: Option<Agent>,
    /// The task whose 开始 is waiting for an Agent choice; true when it is "再开一个会话".
    picking: Option<(String, bool)>,
    last_agent: Option<Agent>,
    resume: Option<(String, String)>,
    here: Option<String>,
    show_stale: bool,
    show_closed: bool,
    cleanup: Vec<String>,
    cleanup_index: usize,
    show_running: bool,
    show_hidden: bool,
    card_menu: Option<String>,
    drag: Option<Vec<bool>>,
    split_bounds: HashMap<Vec<bool>, gpui::Bounds<gpui::Pixels>>,
    overview: bool,
    busy: bool,
    error: Option<String>,
    form: Option<Form>,
    /// One-line capture in the sidebar; the full form is its "展开".
    quick_task: Option<(Entity<Field>, gpui::Subscription)>,
    task_detail: Option<String>,
    editing_task: Option<String>,
    start_after_save: Option<String>,
    recognition: RecognitionStatus,
    _recognition_connection: crate::connection::RecognitionConnection,
}
#[derive(Clone)]
struct PaneMenu {
    terminal: String,
    position: gpui::Point<gpui::Pixels>,
    page: MenuPage,
}
#[derive(Clone, Copy)]
enum MenuPage {
    Main,
    Sessions,
    Assign,
}
struct Form {
    notifications: bool,
    kind: FormKind,
    fields: Vec<Entity<Field>>,
    agent: Agent,
    space: String,
}
#[derive(Clone, Copy)]
enum FormKind {
    Settings,
    Space,
    Task,
    Defer,
    TaskDefer,
}
fn id() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn session_space<'a>(w: &'a Workspace, s: &AgentSession) -> Option<&'a str> {
    s.task
        .as_ref()
        .and_then(|id| w.tasks.iter().find(|t| &t.id == id))
        .map(|t| t.space.as_str())
        .or_else(|| {
            s.terminal
                .as_ref()
                .and_then(|id| w.terminals.iter().find(|t| &t.id == id))
                .map(|t| t.space.as_str())
        })
        .or_else(|| {
            w.spaces
                .iter()
                .filter(|p| std::path::Path::new(&s.cwd).starts_with(&p.cwd))
                .max_by_key(|p| p.cwd.len())
                .map(|p| p.id.as_str())
        })
}
fn live_terminal<'a>(w: &'a Workspace, s: &AgentSession) -> Option<&'a Terminal> {
    if !s.current {
        return None;
    }
    s.terminal.as_ref().and_then(|id| {
        w.terminals
            .iter()
            .find(|t| &t.id == id && t.state == ProcessState::Running)
    })
}
fn changed_since(w: &Workspace, s: &AgentSession, since: u64) -> bool {
    since > 0 && w.content.get(&s.id).map_or(s.updated, |c| c.updated) > since
}
fn show_on_return(left: u64, at: u64, minutes: u32) -> bool {
    left > 0 && at.saturating_sub(left) > u64::from(minutes) * 60
}
fn record_left(socket: &std::path::Path, at: u64) {
    if let Err(e) = rpc(socket, &Request::ClientLeft { at }) {
        eprintln!("记录离开时间失败: {e}");
    }
}
fn interrupted(workspace: &Workspace, id: &str) -> bool {
    workspace
        .terminals
        .iter()
        .any(|t| t.id == id && t.state == ProcessState::Interrupted)
}
fn attachable_ids(workspace: &Workspace, ids: Vec<String>) -> Vec<String> {
    ids.into_iter()
        .filter(|id| !interrupted(workspace, id))
        .collect()
}
fn age(at: u64) -> String {
    let minutes = now().saturating_sub(at) / 60;
    if minutes == 0 {
        "刚刚".into()
    } else if minutes < 60 {
        format!("{minutes} 分钟")
    } else if minutes < 1440 {
        format!("{} 小时", minutes / 60)
    } else {
        format!("{} 天", minutes / 1440)
    }
}
fn badge(group: Group, count: usize) -> gpui::Div {
    let (_, ink, bg) = group.colors();
    div()
        .px(px(T::S4))
        .rounded(px(T::CHIP_RADIUS))
        .text_size(px(T::CHIP_SIZE))
        .text_color(ink)
        .bg(bg)
        .child(count.to_string())
}
fn task_icon() -> gpui::Div {
    div()
        .size(px(T::CHIP_SIZE))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(T::S2))
        .border(px(T::BORDER_WIDTH))
        .border_color(T::color(T::LINK))
        .text_color(T::color(T::LINK))
        .text_size(px(T::CHIP_SIZE))
        .child("✓")
}
fn button(
    id: impl Into<gpui::ElementId>,
    text: impl Into<gpui::SharedString>,
) -> gpui::Stateful<gpui::Div> {
    let text = text.into();
    div()
        .id(id)
        .role(gpui::accesskit::Role::Button)
        .aria_label(text.clone())
        .h(px(T::ROW))
        .px(px(T::S12))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(T::CONTROL_RADIUS))
        .border(px(T::BORDER_WIDTH))
        .border_color(T::color(T::CONTROL))
        .text_size(px(T::SMALL))
        .text_color(T::color(T::BODY))
        .cursor_pointer()
        .hover(|s| s.border_color(T::color(T::ACCENT)))
        .child(text)
}
fn outcome_label(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Open => "未结束".into(),
        Outcome::Completed => "已完成".into(),
        Outcome::Abandoned => "已放弃".into(),
        Outcome::Deferred { reason, until } => format!(
            "暂存：{} · {} 小时后再看",
            reason,
            until.saturating_sub(now()).div_ceil(3600)
        ),
    }
}
fn agent_name(agent: Agent) -> &'static str {
    match agent {
        Agent::Codex => "Codex",
        Agent::Claude => "Claude Code",
    }
}
fn label(text: impl Into<gpui::SharedString>) -> gpui::Div {
    div()
        .text_size(px(T::META))
        .text_color(T::color(T::SECONDARY))
        .child(text.into())
}
fn dot(group: Group) -> gpui::Div {
    div()
        .size(px(T::DOT))
        .flex_shrink_0()
        .rounded_full()
        .bg(group.colors().0)
}
fn chip(group: Group) -> gpui::Div {
    div()
        .px(px(T::S6))
        .py(px(T::BORDER_WIDTH))
        .rounded(px(T::CHIP_RADIUS))
        .text_size(px(T::CHIP_SIZE))
        .text_color(group.colors().1)
        .bg(group.colors().2)
        .child(group.label())
}

impl Workbench {
    #[cfg(feature = "visual-test")]
    pub fn visual_layout(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.overview = false;
        self.zoomed = None;
        self.menu = None;
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
                    &self.socket,
                    &Request::Edit {
                        revision: self.workspace.revision,
                        edit: Edit::PutTab { tab },
                    },
                ) {
                    self.workspace = w;
                }
            }
        }
        self.tab = Some(if name == "menu" { "nested" } else { name }.into());
        self.sync_panes(cx);
        if let Some(id) = self
            .tab
            .as_ref()
            .and_then(|id| self.workspace.tab_terminals(id).first().cloned())
        {
            if let Some(pane) = self.panes.get(&id) {
                window.focus(&pane.focus_handle(cx), cx);
            }
        }
        cx.notify();
        window.refresh();
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_bounds(&self) -> gpui::Bounds<gpui::Pixels> {
        *self
            .split_bounds
            .get(&Vec::new())
            .expect("root splitter bounds")
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_menu_point(&self, index: usize) -> gpui::Point<gpui::Pixels> {
        let menu = self.menu.as_ref().expect("right mouse opened pane menu");
        menu.position + gpui::point(px(T::S20), px(T::S6 + T::ROW * (index as f32 + 0.5)))
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_check_panes(&self, count: usize) {
        assert_eq!(self.panes.len(), count);
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_check_saved_ratio(&self) {
        let Reply::Workspace(w) = rpc(&self.socket, &Request::Workspace).unwrap() else {
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
    pub fn visual_split_down(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.focused.clone().unwrap();
        self.menu_action(&id, "向下分屏", window, cx);
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_close_focused(&mut self, cx: &mut Context<Self>) {
        let id = self.focused.clone().unwrap();
        self.close_pane(&id, cx);
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_assert_new_shell(&self, count: usize, window: &Window, cx: &gpui::App) {
        self.visual_check_panes(count);
        let id = self.focused.as_ref().unwrap();
        assert!(
            self.panes[id].focus_handle(cx).is_focused(window),
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
    pub fn visual_check_closed_processes(&self) {
        let extras: Vec<_> = self
            .workspace
            .terminals
            .iter()
            .filter(|t| !t.id.starts_with('p'))
            .collect();
        assert_eq!(extras.len(), 2);
        for t in extras {
            assert_eq!(t.state, ProcessState::Running);
            assert_eq!(unsafe { libc::kill(t.pid.unwrap() as i32, 0) }, 0);
        }
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_marked_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Bounds<gpui::Pixels>> {
        if let Some(pane) = self
            .tab
            .as_ref()
            .and_then(|id| self.workspace.tab_terminals(id).first().cloned())
            .and_then(|id| self.panes.get(&id))
        {
            return Some(pane.update(cx, |pane, cx| pane.verify_marked_overlay(window, cx)));
        }
        None
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_view(&mut self, overview: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.overview = overview;
        self.sync_panes(cx);
        if !overview {
            if let Some(pane) = self
                .tab
                .as_ref()
                .and_then(|id| self.workspace.tab_terminals(id).first().cloned())
                .and_then(|id| self.panes.get(&id))
            {
                window.focus(&pane.focus_handle(cx), cx);
            }
        }
        cx.notify();
        window.refresh();
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_page(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.form = None;
        self.quick_task = None;
        self.picking = None;
        self.task_detail = None;
        self.cleanup.clear();
        self.here = None;
        self.search_open = false;
        self.overview = name == "Overview";
        self.zoomed = None;
        self.space = "s0".into();
        self.tab = Some(
            if name == "TodoStart" {
                "start"
            } else {
                "feedback"
            }
            .into(),
        );
        self.show_running = false;
        self.sync_panes(cx);
        match name {
            "Main" => self.here = self.workspace.current_agent("p0").map(|s| s.id.clone()),
            "TodoNew" => {
                self.show_form(FormKind::Task, window, cx);
                let task = self
                    .workspace
                    .tasks
                    .iter()
                    .find(|t| t.id == "task1")
                    .unwrap();
                for (field, text) in self.form.as_ref().unwrap().fields.iter().zip([
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
            "TodoDetail" => self.task_detail = Some("task0".into()),
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
                self.panes["pstart"]
                    .update(cx, |pane, cx| pane.start_agent(&command, Some(text), cx));
            }
            "Cleanup" => {
                self.cleanup = self
                    .workspace
                    .agent_sessions
                    .iter()
                    .filter(|s| self.group(s) == Group::Stale)
                    .map(|s| s.id.clone())
                    .collect();
                self.cleanup_index = 0;
                assert!(!self.cleanup.is_empty());
            }
            "QuickTask" => {
                self.show_quick_task(window, cx);
                if let Some((field, _)) = &self.quick_task {
                    field.update(cx, |f, cx| {
                        f.set_text("给离线模式补一组断网回归测试");
                        cx.notify();
                    });
                }
            }
            "ScreenWait" => self.here = self.workspace.current_agent("p1").map(|s| s.id.clone()),
            "StartPick" => self.picking = Some(("task2".into(), false)),
            "Overview" | "Menubar" => (),
            _ => panic!("unknown visual page"),
        }
        cx.notify();
        window.refresh();
    }
    pub fn new(
        socket: PathBuf,
        workspace: Workspace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let space = workspace
            .spaces
            .first()
            .map(|s| s.id.clone())
            .unwrap_or_default();
        let tab = workspace
            .tabs
            .iter()
            .find(|t| !t.closed && t.space == space)
            .map(|t| t.id.clone());
        let (status_tx, mut status_rx) = futures::channel::mpsc::unbounded();
        let recognition_connection =
            crate::connection::RecognitionConnection::new(&socket, status_tx);
        cx.spawn(async move |this, cx| {
            use futures::StreamExt;
            while let Some(status) = status_rx.next().await {
                if this
                    .update(cx, |this, cx| {
                        let revision = status.workspace_revision;
                        this.recognition = status;
                        if revision > this.workspace.revision {
                            this.request(Request::Workspace, cx);
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        cx.observe_global::<crate::desktop::Status>(|_, cx| cx.notify())
            .detach();
        let since = workspace.last_left;
        let overview = show_on_return(since, now(), workspace.settings.away_minutes);
        let search = cx.new(|cx| Field::new("跳到会话、待办或原话", "", cx).compact());
        cx.observe(&search, |_, _, cx| cx.notify()).detach();
        cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() {
                if let Some(left) = this.away.take() {
                    if show_on_return(left, now(), this.workspace.settings.away_minutes) {
                        this.since = left;
                        this.overview = true;
                        this.sync_panes(cx);
                    }
                    cx.notify();
                }
            } else if this.away.is_none() {
                let at = now();
                this.away = Some(at);
                let socket = this.socket.clone();
                cx.background_spawn(async move {
                    record_left(&socket, at);
                })
                .detach();
            }
        })
        .detach();
        cx.on_release(|this, _| record_left(&this.socket, this.away.unwrap_or_else(now)))
            .detach();
        let mut this = Self {
            since,
            away: None,
            filter_space: None,
            changed_only: false,
            search_open: false,
            search,
            socket,
            workspace,
            space,
            tab,
            panes: HashMap::new(),
            focused: None,
            pending_focus: true,
            zoomed: None,
            menu: None,
            split_target: None,
            chosen_agent: None,
            picking: None,
            last_agent: None,
            resume: None,
            here: None,
            show_stale: false,
            show_closed: false,
            cleanup: vec![],
            cleanup_index: 0,
            show_running: false,
            show_hidden: false,
            card_menu: None,
            drag: None,
            split_bounds: HashMap::new(),
            overview,
            busy: false,
            error: None,
            form: None,
            quick_task: None,
            task_detail: None,
            editing_task: None,
            start_after_save: None,
            recognition: RecognitionStatus::default(),
            _recognition_connection: recognition_connection,
        };
        this.sync_panes(cx);
        if this.workspace.terminals.is_empty() && !this.overview {
            this.launch(false, None, cx);
        }
        this
    }
    fn sync_panes(&mut self, cx: &mut Context<Self>) {
        let ids = if self.overview {
            Vec::new()
        } else {
            self.workspace
                .tabs
                .iter()
                .find(|t| Some(&t.id) == self.tab.as_ref() && !t.closed)
                .map(|t| self.workspace.tab_terminals(&t.id))
                .unwrap_or_default()
        };
        if self.zoomed.as_ref().is_some_and(|id| !ids.contains(id)) {
            self.zoomed = None;
        }
        let ids = if let Some(id) = &self.zoomed {
            vec![id.clone()]
        } else {
            ids
        };
        let ids = attachable_ids(&self.workspace, ids);
        if self.focused.as_ref().is_none_or(|id| !ids.contains(id)) {
            self.focused = ids.first().cloned();
            self.pending_focus = true;
        }
        self.panes.retain(|id, _| ids.contains(id));
        for id in ids {
            if !self.panes.contains_key(&id) {
                let pane = cx.new(|cx| Pane::new(&self.socket, &id, false, cx));
                cx.observe(&pane, |_, _, cx| cx.notify()).detach();
                self.panes.insert(id, pane);
            }
        }
    }
    fn request(&mut self, request: Request, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.error = None;
        cx.notify();
        let socket = self.socket.clone();
        let task = cx.background_spawn(async move { rpc(&socket, &request) });
        cx.spawn(async move |this, cx| {
            let reply = task.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                let received_workspace = matches!(&reply, Ok(Reply::Workspace(_)));
                match reply {
                    Ok(Reply::Workspace(w)) => {
                        if let Some(id) = this.cleanup.get(this.cleanup_index) {
                            let old = this.workspace.agent_sessions.iter().find(|s| &s.id == id);
                            let new = w.agent_sessions.iter().find(|s| &s.id == id);
                            if old.zip(new).is_some_and(|(a, b)| {
                                a.outcome != b.outcome && b.outcome != Outcome::Open
                            }) {
                                this.cleanup_index += 1;
                            }
                        }
                        this.workspace = w;
                        this.sync_panes(cx);
                        if let Some(task) = this.start_after_save.take() {
                            this.launch(false, Some(task), cx);
                        }
                    }
                    Ok(_) => (),
                    Err(e) => {
                        this.start_after_save = None;
                        this.error = Some(e.to_string());
                    }
                }
                if received_workspace
                    && this.recognition.workspace_revision > this.workspace.revision
                {
                    this.request(Request::Workspace, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn edit(&mut self, edit: Edit, cx: &mut Context<Self>) {
        self.request(
            Request::Edit {
                revision: self.workspace.revision,
                edit,
            },
            cx,
        );
    }
    fn launch(&mut self, split: bool, task_id: Option<String>, cx: &mut Context<Self>) {
        if !split
            && self.resume.is_none()
            && let Some(task) = task_id.as_deref()
            && let Some(terminal) = self.workspace.task_terminal(task)
        {
            let id = terminal.id.clone();
            if let Some(mut tab) = self.workspace.terminal_tab(&id).cloned() {
                self.space = tab.space.clone();
                self.tab = Some(tab.id.clone());
                self.focused = Some(id);
                self.pending_focus = true;
                self.overview = false;
                self.task_detail = None;
                if tab.closed {
                    tab.closed = false;
                    self.edit(Edit::PutTab { tab }, cx);
                } else {
                    self.sync_panes(cx);
                    cx.notify();
                }
                return;
            }
        }
        if let Some(task) = task_id
            .as_ref()
            .and_then(|id| self.workspace.tasks.iter().find(|t| &t.id == id))
        {
            self.space = task.space.clone();
        }
        self.task_detail = None;
        self.launch_in_slot(split, task_id, None, cx);
    }
    fn close_pane(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(mut tab) = self.workspace.terminal_tab(id).cloned() else {
            return;
        };
        tab.layout = tab.layout.take().and_then(|l| l.without(id));
        self.menu = None;
        self.zoomed = None;
        self.edit(Edit::PutTab { tab }, cx);
    }
    fn launch_in_slot(
        &mut self,
        mut split: bool,
        task_id: Option<String>,
        replace: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        let previous = replace
            .as_ref()
            .and_then(|id| self.workspace.terminals.iter().find(|t| &t.id == id))
            .cloned();
        if replace.is_some()
            && !previous
                .as_ref()
                .is_some_and(|t| t.state == ProcessState::Interrupted)
        {
            return;
        }
        if !split
            && let Some(task_id) = &task_id
            && let Some(tab) = self
                .workspace
                .tabs
                .iter()
                .find(|t| t.default_task.as_ref() == Some(task_id))
        {
            self.tab = Some(tab.id.clone());
            split = true;
        }
        let source = self.split_target.take().or_else(|| {
            if !split {
                return None;
            }
            self.focused
                .clone()
                .filter(|id| {
                    self.workspace
                        .terminal_tab(id)
                        .is_some_and(|t| Some(&t.id) == self.tab.as_ref())
                })
                .or_else(|| {
                    self.tab
                        .as_ref()
                        .and_then(|id| self.workspace.tab_terminals(id).first().cloned())
                })
                .map(|id| (id, Axis::Right))
        });
        let Some(space) = self
            .workspace
            .spaces
            .iter()
            .find(|s| {
                s.id == previous
                    .as_ref()
                    .map(|t| &t.space)
                    .unwrap_or(&self.space)
                    .as_str()
            })
            .cloned()
        else {
            return;
        };
        let agent = self.chosen_agent.take();
        let resume = self.resume.take();
        let resuming = resume.is_some();
        // The command runs in the new shell; task text only ever reaches the Agent's input.
        let start: Option<(String, Option<String>)> =
            resume.as_ref().map(|r| (r.0.clone(), None)).or_else(|| {
                task_id
                    .as_ref()
                    .and_then(|id| self.workspace.tasks.iter().find(|t| &t.id == id))
                    .map(|task| {
                        let agent = agent.unwrap_or(task.agent);
                        (agent.command(), Some(task.prompt()))
                    })
            });
        let mut tab = if split {
            let Some(tab) = self
                .workspace
                .tabs
                .iter()
                .find(|t| Some(&t.id) == self.tab.as_ref())
                .cloned()
            else {
                return;
            };
            tab
        } else {
            let title = String::new();
            Tab {
                id: id(),
                space: space.id.clone(),
                default_task: task_id.clone(),
                title,
                closed: false,
                layout: None,
            }
        };
        let terminal = id();
        tab.closed = false;
        if let Some(old) = replace {
            if !tab
                .layout
                .as_mut()
                .is_some_and(|l| l.replace(&old, &terminal))
            {
                return;
            }
        } else if let Some(tree) = &mut tab.layout {
            let Some((target, axis)) = &source else {
                return;
            };
            if !tree.split(target, terminal.clone(), *axis) {
                return;
            }
        } else {
            tab.layout = Some(Layout::Leaf(terminal.clone()));
        }
        let task_cwd = task_id
            .as_ref()
            .and_then(|id| self.workspace.tasks.iter().find(|t| &t.id == id))
            .and_then(|t| self.workspace.spaces.iter().find(|s| s.id == t.space))
            .map(|s| s.cwd.clone());
        let cwd = resume.map(|r| r.1).or(task_cwd).unwrap_or_else(|| {
            previous
                .map(|t| t.cwd)
                .or_else(|| {
                    source.as_ref().and_then(|(id, _)| {
                        self.workspace
                            .terminals
                            .iter()
                            .find(|t| &t.id == id)
                            .map(|t| t.cwd.clone())
                    })
                })
                .unwrap_or(space.cwd)
        });
        let cwd_source = source
            .as_ref()
            .and_then(|(id, _)| {
                self.workspace
                    .terminals
                    .iter()
                    .find(|t| &t.id == id && t.state == ProcessState::Running)
            })
            .filter(|_| task_id.is_none() && !resuming)
            .map(|t| t.id.clone());
        let mut request = Request::Launch {
            id: terminal.clone(),
            space: space.id,
            argv: vec![
                std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into()),
                "-l".into(),
            ],
            cwd,
            cols: 100,
            rows: 30,
        };
        let socket = self.socket.clone();
        let tab_id = tab.id.clone();
        self.busy = true;
        self.error = None;
        cx.notify();
        let job = cx.background_spawn(async move {
            if let Some(id) = cwd_source {
                let Reply::Cwd(current) = rpc(&socket, &Request::TerminalCwd { id })? else {
                    return Err(std::io::Error::other("terminal cwd response missing"));
                };
                if let Request::Launch { cwd, .. } = &mut request {
                    *cwd = current;
                }
            }
            let Reply::Workspace(w) = rpc(&socket, &request)? else {
                return Err(std::io::Error::other("launch response missing"));
            };
            rpc(
                &socket,
                &Request::Edit {
                    revision: w.revision,
                    edit: Edit::PutTab { tab },
                },
            )
        });
        cx.spawn(async move |this, cx| {
            let reply = job.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match reply {
                    Ok(Reply::Workspace(w)) => {
                        this.workspace = w;
                        this.tab = Some(tab_id);
                        this.zoomed = None;
                        this.menu = None;
                        this.focused = Some(terminal.clone());
                        this.pending_focus = true;
                        this.overview = false;
                        this.sync_panes(cx);
                        if let (Some((command, text)), Some(pane)) =
                            (&start, this.panes.get(&terminal))
                        {
                            pane.update(cx, |pane, cx| pane.start_agent(command, text.clone(), cx));
                        }
                        if this.recognition.workspace_revision > this.workspace.revision {
                            this.request(Request::Workspace, cx);
                        }
                    }
                    Ok(_) => this.error = Some("缺少工作台响应".into()),
                    Err(e) => this.error = Some(e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn cleanup_view(&self, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        let mut page = div()
            .w_full()
            .max_w(px(T::CLEANUP_WIDTH))
            .flex()
            .flex_col()
            .gap(px(T::S16))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(T::S12))
                    .child(
                        button("cleanup-exit", "← 返回鸟瞰")
                            .border_0()
                            .text_color(T::color(T::ACCENT))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.cleanup.clear();
                                this.overview = true;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(T::FORM_TITLE))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("清理很久没动的会话"),
                    )
                    .child(div().flex_1())
                    .child(
                        label(format!(
                            "第 {} / {} 个",
                            (self.cleanup_index + 1).min(self.cleanup.len()),
                            self.cleanup.len()
                        ))
                        .text_color(T::color(T::POPOVER_SECONDARY)),
                    ),
            )
            .child(
                div()
                    .h(px(T::S4))
                    .w_full()
                    .rounded_full()
                    .bg(T::color(T::POPOVER_BORDER))
                    .child(div().h_full().rounded_full().bg(T::color(T::ACCENT)).w(
                        gpui::relative(
                            self.cleanup_index as f32 / self.cleanup.len().max(1) as f32,
                        ),
                    )),
            );
        if let Some(s) = self
            .cleanup
            .get(self.cleanup_index)
            .and_then(|id| self.workspace.agent_sessions.iter().find(|s| &s.id == id))
        {
            let h = self.workspace.content.get(&s.id).map(|c| c.here());
            if let Some(h) = h {
                let mut card = div()
                    .p(px(T::S20))
                    .rounded(px(T::CARD_RADIUS))
                    .bg(T::color(T::WHITE))
                    .flex()
                    .flex_col()
                    .gap(px(T::S16))
                    .child(
                        div()
                            .text_size(px(T::TITLE))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(self.session_title(s)),
                    )
                    .child(
                        label(format!(
                            "{} · {} · {}",
                            s.runtime.executable(),
                            age(self
                                .workspace
                                .content
                                .get(&s.id)
                                .map_or(s.updated, |c| c.updated)),
                            s.cwd
                        ))
                        .text_color(T::color(T::POPOVER_SECONDARY)),
                    );
                for (name, text) in [
                    ("你要它", h.ask),
                    ("做成了", h.done),
                    ("停在", h.stopped),
                    ("下一步·推断", h.next),
                ] {
                    card = card.child(
                        div()
                            .flex()
                            .gap(px(T::S16))
                            .py(px(T::S8))
                            .when(name == "下一步·推断", |d| {
                                d.bg(T::color(T::INFERENCE))
                            })
                            .child(
                                label(name)
                                    .w(px(T::HERE_LABEL))
                                    .flex_shrink_0()
                                    .text_color(T::color(T::POPOVER_SECONDARY)),
                            )
                            .child(div().flex_1().child(text)),
                    );
                }
                page = page.child(card);
            }
            let mut actions = div().flex().gap(px(T::S8));
            let resume = s.clone();
            actions = actions.child(
                button("cleanup-continue", "继续")
                    .h(px(T::CLEANUP_ACTION_HEIGHT))
                    .flex_1()
                    .justify_center()
                    .bg(T::color(T::ACCENT))
                    .text_color(T::color(T::WHITE))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.cleanup.clear();
                        this.open_session(&resume, window, cx);
                    })),
            );
            {
                let (name, outcome) = ("完成", Outcome::Completed);
                let sid = s.id.clone();
                actions = actions.child(
                    button(name, name)
                        .h(px(T::CLEANUP_ACTION_HEIGHT))
                        .flex_1()
                        .justify_center()
                        .bg(T::color(T::WHITE))
                        .text_color(T::color(T::POPOVER_TEXT))
                        .on_click(cx.listener(move |this, _, _, cx| {
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
            let sid = s.id.clone();
            actions = actions.child(
                button("cleanup-defer", "暂存…")
                    .h(px(T::CLEANUP_ACTION_HEIGHT))
                    .flex_1()
                    .justify_center()
                    .text_color(T::color(T::ACCENT))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.here = Some(sid.clone());
                        this.show_form(FormKind::Defer, window, cx);
                    })),
            );
            let sid = s.id.clone();
            actions = actions.child(
                button("cleanup-abandon", "放弃")
                    .h(px(T::CLEANUP_ACTION_HEIGHT))
                    .flex_1()
                    .justify_center()
                    .bg(T::color(T::WHITE))
                    .border_color(T::color(T::POPOVER_BORDER))
                    .text_color(T::color(T::POPOVER_TEXT))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.edit(
                            Edit::SetOutcome {
                                id: sid.clone(),
                                outcome: Outcome::Abandoned,
                            },
                            cx,
                        )
                    })),
            );
            page = page.child(actions);
        } else {
            page = page.child("这一轮已处理完，可以返回鸟瞰。");
        }
        page = page.child(
            div()
                .flex()
                .gap(px(T::S8))
                .child(
                    button("cleanup-previous", "上一个")
                        .text_color(T::color(T::POPOVER_TEXT))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.cleanup_index = this.cleanup_index.saturating_sub(1);
                            cx.notify();
                        })),
                )
                .child(
                    button("cleanup-skip", "跳过")
                        .text_color(T::color(T::POPOVER_TEXT))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.cleanup_index = (this.cleanup_index + 1).min(this.cleanup.len());
                            cx.notify();
                        })),
                ),
        );
        div()
            .id("cleanup-page")
            .size_full()
            .bg(T::color(T::CLEANUP_BACKGROUND))
            .text_color(T::color(T::POPOVER_TEXT))
            .overflow_y_scroll()
            .p(px(T::S20))
            .flex()
            .flex_col()
            .items_center()
            .child(page)
    }
    fn task_detail_view(&self, task: &Task, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        let tid = task.id.clone();
        let mut view = div()
            .id("task-detail")
            .w(px(T::DETAIL_WIDTH))
            .max_w_full()
            .m(px(T::S20))
            .bg(T::color(T::HEADER))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::CONTROL))
            .rounded(px(T::POPOVER_RADIUS))
            .overflow_y_scroll()
            .p(px(T::S20))
            .flex()
            .flex_col()
            .gap(px(T::S12))
            .child(div().text_size(px(T::TITLE)).child(task.title.clone()))
            .child(label(format!(
                "{} · {}",
                task.agent.executable(),
                match self
                    .workspace
                    .task_outcomes
                    .get(&tid)
                    .unwrap_or(&Outcome::Open)
                {
                    Outcome::Open => "未完成",
                    Outcome::Completed => "已完成",
                    Outcome::Deferred { .. } => "暂存",
                    Outcome::Abandoned => "已放弃",
                }
            )))
            .child(
                label(
                    self.workspace
                        .spaces
                        .iter()
                        .find(|s| s.id == task.space)
                        .map_or(String::new(), |s| format!("{} · {}", s.name, s.cwd)),
                )
                .truncate(),
            )
            .child(label("说明").mt(px(T::S8)))
            .child(task.description.clone());
        view = view.child(
            label(format!(
                "验收标准 · {} / {} 项有机械证据",
                task.acceptance
                    .iter()
                    .filter(|c| c.evidence.is_some())
                    .count(),
                task.acceptance.len()
            ))
            .mt(px(T::S12)),
        );
        for criterion in &task.acceptance {
            view = view.child(
                div()
                    .p(px(T::S8))
                    .border_b(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::BORDER))
                    .child(format!(
                        "{}  {}",
                        if criterion.evidence.is_some() {
                            "☑"
                        } else {
                            "□"
                        },
                        criterion.description
                    ))
                    .when_some(criterion.evidence.clone(), |d, source| {
                        d.child(label(source))
                    }),
            );
        }
        view = view.child(label("关联会话").mt(px(T::S12)));
        for session in self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| s.task.as_ref() == Some(&tid))
        {
            let session = session.clone();
            view = view.child(
                button(
                    gpui::SharedString::from(format!("task-session-{}", session.id)),
                    "",
                )
                .h(px(T::TASK_ROW))
                .justify_start()
                .gap(px(T::S8))
                .child(dot(self.group(&session)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().truncate().child(self.session_title(&session)))
                        .child(label(format!(
                            "{} · {}",
                            session.runtime.executable(),
                            self.group(&session).label()
                        ))),
                )
                .child(label("打开 →").text_color(T::color(T::LINK)))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.task_detail = None;
                    this.enter_card(&session, window, cx);
                })),
            );
        }
        let mut actions = div().flex().flex_wrap().gap(px(T::S8));
        // Offer only real changes: no 撤销结局 without an outcome, no repeat of the current one.
        for (name, outcome) in [
            ("标记完成", Outcome::Completed),
            ("放弃", Outcome::Abandoned),
            ("撤销结局", Outcome::Open),
        ]
        .into_iter()
        .filter(|(_, o)| {
            o != self
                .workspace
                .task_outcomes
                .get(&task.id)
                .unwrap_or(&Outcome::Open)
        }) {
            let tid = tid.clone();
            actions = actions.child(button(name, name).on_click(cx.listener(
                move |this, _, _, cx| {
                    this.edit(
                        Edit::SetTaskOutcome {
                            id: tid.clone(),
                            outcome: outcome.clone(),
                        },
                        cx,
                    )
                },
            )));
        }
        actions = actions
            .child(button("task-defer", "暂存").on_click(
                cx.listener(|this, _, window, cx| this.show_form(FormKind::TaskDefer, window, cx)),
            ))
            .child(button("task-edit", "编辑").on_click({
                let task = task.clone();
                cx.listener(move |this, _, window, cx| this.edit_task_form(&task, window, cx))
            }))
            .child(
                button("task-another", "再开一个会话")
                    .bg(T::color(T::ACCENT))
                    .text_color(T::color(T::WHITE))
                    .on_click({
                        let tid = tid.clone();
                        cx.listener(move |this, _, _, cx| this.ask_agent(tid.clone(), true, cx))
                    }),
            )
            .child(
                button("task-close", "返回画面").on_click(cx.listener(|this, _, _, cx| {
                    this.task_detail = None;
                    cx.notify();
                })),
            );
        view.child(actions)
            .children(self.agent_picker(&task.id, cx))
    }
    /// The line above a pane that Keepline is starting an Agent in for a task.
    fn handoff_bar(
        &self,
        terminal: &str,
        task: &Task,
        state: crate::pane::HandoffState,
        cx: &Context<Self>,
    ) -> gpui::Div {
        use crate::pane::HandoffState;
        let terminal = terminal.to_owned();
        let agent = self
            .workspace
            .current_agent(&terminal)
            .map(|s| s.runtime)
            .unwrap_or(task.agent);
        let message = match state {
            HandoffState::Waiting => format!(
                "为待办「{}」启动 {}，就绪后放入待办内容",
                task.title,
                agent_name(agent)
            ),
            HandoffState::Placed => "已放入待办内容，检查后在 Agent 里按回车发送".to_owned(),
            HandoffState::NotReady => "Agent 还没准备好，内容没有放进去".to_owned(),
            HandoffState::Missing => format!(
                "找不到 {} 命令，内容没有放进去。先安装它或把它加进 PATH，再点「再试一次」",
                agent_name(agent)
            ),
        };
        let stuck = matches!(state, HandoffState::NotReady | HandoffState::Missing);
        let text = task.prompt();
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(T::S6))
            .p(px(T::S6))
            .bg(T::color(T::TODO))
            .child(label(message).text_color(if stuck {
                Group::Waiting.colors().1
            } else {
                T::color(T::SECONDARY)
            }))
            .when(task.acceptance.is_empty(), |d| {
                let task = task.clone();
                d.child(label("还没写验收标准").text_color(Group::Waiting.colors().1))
                    .child(button("handoff-criteria", "补写").on_click(cx.listener(
                        move |this, _, window, cx| this.edit_task_form(&task, window, cx),
                    )))
            })
            .when(stuck, |d| {
                let retry = terminal.clone();
                d.child(button("handoff-retry", "再试一次").on_click(cx.listener(
                    move |this, _, _, cx| {
                        if let Some(p) = this.panes.get(&retry) {
                            p.update(cx, |p, cx| p.retry_handoff(cx));
                        }
                    },
                )))
            })
            .child(
                button("handoff-copy", "复制内容").on_click(cx.listener(move |_, _, _, cx| {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone()))
                })),
            )
            .child(button("handoff-dismiss", "收起").on_click({
                let terminal = terminal.clone();
                cx.listener(move |this, _, _, cx| {
                    if let Some(p) = this.panes.get(&terminal) {
                        p.update(cx, |p, cx| p.dismiss_handoff(cx));
                    }
                })
            }))
            .when(self.workspace.current_agent(&terminal).is_none(), |d| {
                d.child(button("handoff-unlink", "不关联待办").on_click(cx.listener(
                    move |this, _, _, cx| {
                        if this.workspace.current_agent(&terminal).is_none() {
                            this.edit(
                                Edit::SetPendingTask {
                                    terminal: terminal.clone(),
                                    task: None,
                                },
                                cx,
                            );
                        }
                    },
                )))
            })
    }
    /// 开始 asks which Agent first, unless the task already has a pane to return to.
    fn ask_agent(&mut self, task: String, another: bool, cx: &mut Context<Self>) {
        if !another && self.workspace.task_terminal(&task).is_some() {
            self.launch(false, Some(task), cx);
            return;
        }
        self.picking = Some((task, another));
        cx.notify();
    }
    fn choose_agent(&mut self, agent: Agent, cx: &mut Context<Self>) {
        let Some((task, another)) = self.picking.take() else {
            return;
        };
        self.last_agent = Some(agent);
        self.chosen_agent = Some(agent);
        let tab = self
            .workspace
            .tabs
            .iter()
            .find(|t| t.default_task.as_ref() == Some(&task))
            .map(|t| t.id.clone());
        match tab.filter(|_| another) {
            Some(tab) => {
                self.tab = Some(tab);
                self.launch(true, Some(task), cx);
            }
            None => self.launch(false, Some(task), cx),
        }
    }
    fn agent_picker(&self, task: &str, cx: &Context<Self>) -> Option<gpui::Div> {
        if self.picking.as_ref().map(|(t, _)| t.as_str()) != Some(task) {
            return None;
        }
        let first = self
            .last_agent
            .unwrap_or(self.workspace.settings.default_agent);
        let order = if first == Agent::Codex {
            [Agent::Codex, Agent::Claude]
        } else {
            [Agent::Claude, Agent::Codex]
        };
        let mut row = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(T::S4))
            .px(px(T::S8))
            .pb(px(T::S6))
            .child(label("选 Agent").text_size(px(T::META)));
        for agent in order {
            row = row.child(
                button(
                    gpui::SharedString::from(format!("pick-{}-{task}", agent.executable())),
                    agent_name(agent),
                )
                .h(px(T::ICON_HEIGHT))
                .px(px(T::S6))
                .text_size(px(T::META))
                .when(agent == first, |b| {
                    b.bg(T::color(T::ACCENT)).text_color(T::color(T::WHITE))
                })
                .on_click(cx.listener(move |this, _, _, cx| this.choose_agent(agent, cx))),
            );
        }
        Some(
            row.child(
                button(
                    gpui::SharedString::from(format!("pick-cancel-{task}")),
                    "取消",
                )
                .h(px(T::ICON_HEIGHT))
                .px(px(T::S6))
                .text_size(px(T::META))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.picking = None;
                    cx.notify();
                })),
            ),
        )
    }
    fn edit_task_form(&mut self, task: &Task, window: &mut Window, cx: &mut Context<Self>) {
        self.show_form(FormKind::Task, window, cx);
        self.editing_task = Some(task.id.clone());
        if let Some(f) = &mut self.form {
            f.space = task.space.clone();
            f.agent = task.agent;
            for (field, value) in f.fields.iter().zip([
                task.title.clone(),
                task.description.clone(),
                task.acceptance
                    .iter()
                    .map(|c| c.description.clone())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ]) {
                field.update(cx, |f, cx| {
                    f.set_text(&value);
                    cx.notify();
                });
            }
        }
    }
    fn show_quick_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.overview = false;
        self.form = None;
        self.task_detail = None;
        self.sync_panes(cx);
        let field = cx.new(|cx| Field::new("要做什么（回车保存，Esc 取消）", "", cx).compact());
        let subscription = cx.subscribe(&field, |this, _, event, cx| match event {
            crate::field::FieldEvent::Submit => this.save_quick_task(false, cx),
            crate::field::FieldEvent::Cancel => {
                this.quick_task = None;
                cx.notify();
            }
        });
        window.focus(&field.focus_handle(cx), cx);
        self.quick_task = Some((field, subscription));
        cx.notify();
    }
    /// Only the title is needed to capture; the space and Agent come from where you are.
    fn save_quick_task(&mut self, start: bool, cx: &mut Context<Self>) {
        let Some((field, _)) = &self.quick_task else {
            return;
        };
        let title = field.read(cx).text.trim().to_owned();
        if title.is_empty() {
            self.error = Some("先写要做什么".into());
            cx.notify();
            return;
        }
        let task = Task {
            id: id(),
            space: self.space.clone(),
            title,
            description: String::new(),
            acceptance: vec![],
            agent: self.workspace.settings.default_agent,
        };
        if start {
            self.start_after_save = Some(task.id.clone());
        }
        self.quick_task = None;
        self.edit(Edit::AddTask(task), cx);
    }
    fn expand_quick_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = self
            .quick_task
            .take()
            .map(|(f, _)| f.read(cx).text.clone())
            .unwrap_or_default();
        self.show_form(FormKind::Task, window, cx);
        if let Some(field) = self.form.as_ref().and_then(|f| f.fields.first()) {
            field.update(cx, |f, cx| {
                f.set_text(&title);
                cx.notify();
            });
        }
    }
    fn quick_task_row(&self, cx: &Context<Self>) -> Option<gpui::Div> {
        let (field, _) = self.quick_task.as_ref()?;
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(T::S6))
                .px(px(T::S8))
                .pb(px(T::S8))
                .child(field.clone())
                .child(
                    div()
                        .flex()
                        .gap(px(T::S4))
                        .child(
                            button("quick-save-start", "保存并开始")
                                .h(px(T::ICON_HEIGHT))
                                .px(px(T::S6))
                                .text_size(px(T::META))
                                .bg(T::color(T::ACCENT))
                                .text_color(T::color(T::WHITE))
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.save_quick_task(true, cx)),
                                ),
                        )
                        .child(
                            button("quick-save", "保存")
                                .h(px(T::ICON_HEIGHT))
                                .px(px(T::S6))
                                .text_size(px(T::META))
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.save_quick_task(false, cx)),
                                ),
                        )
                        .child(
                            button("quick-expand", "展开")
                                .h(px(T::ICON_HEIGHT))
                                .px(px(T::S6))
                                .text_size(px(T::META))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.expand_quick_task(window, cx)
                                })),
                        )
                        .child(
                            button("quick-cancel", "取消")
                                .h(px(T::ICON_HEIGHT))
                                .px(px(T::S6))
                                .text_size(px(T::META))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.quick_task = None;
                                    cx.notify();
                                })),
                        ),
                ),
        )
    }
    fn show_form(&mut self, kind: FormKind, window: &mut Window, cx: &mut Context<Self>) {
        self.overview = false;
        self.editing_task = None;
        self.sync_panes(cx);
        let fields = match kind {
            FormKind::Settings => vec![
                cx.new(|cx| {
                    Field::new(
                        "很久没动（天，1–365）",
                        &self.workspace.settings.stale_days.to_string(),
                        cx,
                    )
                }),
                cx.new(|cx| {
                    Field::new(
                        "离开多久显示鸟瞰（分钟，1–1440）",
                        &self.workspace.settings.away_minutes.to_string(),
                        cx,
                    )
                }),
            ],
            FormKind::Space => vec![
                cx.new(|cx| Field::new("空间名称", "", cx)),
                cx.new(|cx| Field::new("工作目录（绝对路径）", "", cx)),
            ],
            FormKind::Defer | FormKind::TaskDefer => vec![
                cx.new(|cx| Field::new("暂存原因（必填）", "", cx)),
                cx.new(|cx| Field::new("多少小时后再看（必填）", "24", cx)),
            ],
            FormKind::Task => vec![
                cx.new(|cx| Field::new("要做什么", "", cx)),
                cx.new(|cx| Field::new("说明", "", cx)),
                cx.new(|cx| Field::new("验收标准（每行一条）", "", cx)),
            ],
        };
        window.focus(&fields[0].focus_handle(cx), cx);
        self.form = Some(Form {
            notifications: self.workspace.settings.notifications,
            kind,
            fields,
            agent: self.workspace.settings.default_agent,
            space: self.space.clone(),
        });
        cx.notify();
    }
    fn save_form(&mut self, start: bool, cx: &mut Context<Self>) {
        let Some(form) = self.form.take() else {
            return;
        };
        let values: Vec<String> = form
            .fields
            .iter()
            .map(|f| f.read(cx).text.clone())
            .collect();
        let edit = match form.kind {
            FormKind::Settings => {
                let days = values[0]
                    .trim()
                    .parse::<u32>()
                    .ok()
                    .filter(|n| (1..=365).contains(n));
                let minutes = values[1]
                    .trim()
                    .parse::<u32>()
                    .ok()
                    .filter(|n| (1..=1440).contains(n));
                let (Some(stale_days), Some(away_minutes)) = (days, minutes) else {
                    self.error = Some("请填写有效天数和分钟数".into());
                    self.form = Some(form);
                    return;
                };
                Edit::Settings(Settings {
                    notifications: form.notifications,
                    stale_days,
                    away_minutes,
                    default_agent: form.agent,
                })
            }

            FormKind::Defer | FormKind::TaskDefer => {
                let hours = values[1]
                    .trim()
                    .parse::<u64>()
                    .ok()
                    .filter(|h| *h > 0 && *h <= 24 * 365);
                if values[0].trim().is_empty() || hours.is_none() {
                    self.error = Some("请填写暂存原因和 1–8760 小时的再看时间".into());
                    self.form = Some(form);
                    return;
                }
                let outcome = Outcome::Deferred {
                    reason: values[0].clone(),
                    until: now() + hours.unwrap() * 3600,
                };
                if matches!(form.kind, FormKind::TaskDefer) {
                    let Some(id) = self.task_detail.clone() else {
                        return;
                    };
                    Edit::SetTaskOutcome { id, outcome }
                } else {
                    let Some(id) = self.here.clone() else {
                        return;
                    };
                    Edit::SetOutcome { id, outcome }
                }
            }
            FormKind::Space => Edit::AddSpace(Space {
                id: id(),
                name: values[0].clone(),
                cwd: values[1].clone(),
            }),
            FormKind::Task => {
                let task_id = self.editing_task.clone().unwrap_or_else(id);
                if start {
                    self.start_after_save = Some(task_id.clone());
                }
                let task = Task {
                    id: task_id,
                    space: form.space.clone(),
                    title: values[0].clone(),
                    description: values[1].clone(),
                    acceptance: values[2]
                        .lines()
                        .filter(|line| !line.trim().is_empty())
                        .map(|line| {
                            self.editing_task
                                .as_ref()
                                .and_then(|id| self.workspace.tasks.iter().find(|t| &t.id == id))
                                .and_then(|t| {
                                    t.acceptance.iter().find(|c| c.description == line.trim())
                                })
                                .cloned()
                                .unwrap_or_else(|| AcceptanceCriterion {
                                    id: id(),
                                    description: line.trim().into(),
                                    evidence: None,
                                })
                        })
                        .collect(),
                    agent: form.agent,
                };
                if self.editing_task.take().is_some() {
                    Edit::UpdateTask(task)
                } else {
                    Edit::AddTask(task)
                }
            }
        };
        self.edit(edit, cx);
    }
    fn open_session(
        &mut self,
        session: &AgentSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.search_open = false;
        self.task_detail = None;
        self.here = None;
        self.zoomed = None;
        let Some(terminal) = live_terminal(&self.workspace, session) else {
            if let Some(space) = session_space(&self.workspace, session) {
                self.space = space.into();
            }
            self.resume = Some((
                session.runtime.resume_command(&session.cli_session_id),
                session.cwd.clone(),
            ));
            self.launch(false, session.task.clone(), cx);
            return;
        };
        let terminal_id = terminal.id.clone();
        self.focused = Some(terminal_id.clone());
        self.pending_focus = true;
        if let Some(tab) = self.workspace.terminal_tab(&terminal.id) {
            self.space = tab.space.clone();
            self.tab = Some(tab.id.clone());
            self.overview = false;
            if tab.closed {
                let mut tab = tab.clone();
                tab.closed = false;
                self.edit(Edit::PutTab { tab }, cx);
            } else {
                self.sync_panes(cx);
                if let Some(pane) = self.panes.get(&terminal_id) {
                    window.focus(&pane.focus_handle(cx), cx);
                }
                cx.notify();
            }
        } else {
            let tab = Tab {
                id: id(),
                space: self.space.clone(),
                default_task: session.task.clone(),
                title: "恢复视图".into(),
                closed: false,
                layout: Some(Layout::Leaf(terminal_id.clone())),
            };
            self.tab = Some(tab.id.clone());
            self.overview = false;
            self.edit(Edit::PutTab { tab }, cx);
        }
    }
}
impl Workbench {
    fn group(&self, s: &AgentSession) -> Group {
        use keepline_core::content::{Content, Inbox};
        match self
            .workspace
            .content
            .get(&s.id)
            .cloned()
            .unwrap_or_else(|| Content {
                updated: s.updated,
                ..Default::default()
            })
            .group(s, &self.workspace, now())
        {
            Inbox::Waiting => Group::Waiting,
            Inbox::Error => Group::Error,
            Inbox::Review => Group::Review,
            Inbox::Running => Group::Running,
            Inbox::Stale => Group::Stale,
            _ => Group::Paused,
        }
    }
    fn session_title(&self, s: &AgentSession) -> String {
        if let Some(c) = self
            .workspace
            .content
            .get(&s.id)
            .filter(|c| !c.title.is_empty())
        {
            return c.title.clone();
        }
        if self
            .workspace
            .content
            .get(&s.id)
            .is_some_and(|c| !c.first_ask_known)
        {
            return format!(
                "最初原话未覆盖 · {}",
                &s.cli_session_id.chars().take(8).collect::<String>()
            );
        }
        s.task
            .as_ref()
            .and_then(|id| self.workspace.tasks.iter().find(|t| &t.id == id))
            .map(|t| t.title.clone())
            .unwrap_or_else(|| format!("{} · {}", s.runtime.executable(), s.cli_session_id))
    }
    fn session_line(&self, s: &AgentSession) -> String {
        self.workspace
            .content
            .get(&s.id)
            .map(|c| {
                keepline_core::content::short(
                    &c.here()
                        .stopped
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                    100,
                )
            })
            .unwrap_or_else(|| "内容尚未索引".into())
    }
    fn save_session_task(&mut self, session: &AgentSession, cx: &mut Context<Self>) {
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
            space: self.space.clone(),
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
    fn here_card(&self, session: &AgentSession, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        let card = self
            .workspace
            .content
            .get(&session.id)
            .cloned()
            .unwrap_or_default();
        let h = card.here();
        let save = session.clone();
        let mut panel = div()
            .id(gpui::SharedString::from(format!(
                "here-card-{}",
                session.id
            )))
            .w(px(T::HERE_WIDTH))
            .max_w_full()
            .max_h(px(T::HERE_HEIGHT))
            .overflow_y_scroll()
            .p(px(T::S16))
            .rounded(px(T::POPOVER_RADIUS))
            .bg(T::color(T::WHITE))
            .text_color(T::color(T::POPOVER_TEXT))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::POPOVER_BORDER))
            .flex()
            .flex_col()
            .gap(px(T::S12))
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("你在这里"),
                    )
                    .child(
                        button("dismiss-here", "×")
                            .text_color(T::color(T::POPOVER_TEXT))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.here = None;
                                cx.notify();
                            })),
                    ),
            );
        for (name, value) in [
            ("你要它", h.ask),
            ("做成了", h.done),
            ("停在", h.stopped),
            ("下一步·推断", h.next),
        ] {
            panel = panel.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(T::S4))
                    .when(name == "下一步·推断", |d| {
                        d.bg(T::color(T::INFERENCE)).p(px(T::S8))
                    })
                    .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(name))
                    .child(div().text_size(px(T::SMALL)).child(value)),
            );
        }
        if card.partial_history {
            panel = panel.child(
                div()
                    .text_size(px(T::META))
                    .text_color(T::color(T::POPOVER_SECONDARY))
                    .child("仅覆盖已索引的历史尾段与之后的增量；没有证据不等于没做。"),
            );
        }
        let resume = session.clone();
        let command = session.runtime.resume_command(&session.cli_session_id);
        panel = panel.child(
            div()
                .flex()
                .gap(px(T::S6))
                .child(
                    button("here-continue", "继续")
                        .bg(T::color(T::ACCENT))
                        .text_color(T::color(T::WHITE))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_session(&resume, window, cx)
                        })),
                )
                .child(
                    button("here-copy", "复制命令")
                        .text_color(T::color(T::POPOVER_TEXT))
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(command.clone()))
                        }),
                ),
        );
        panel = panel.child(
            button("save-here-task", "存为待办")
                .text_color(T::color(T::POPOVER_TEXT))
                .on_click(cx.listener(move |this, _, _, cx| this.save_session_task(&save, cx))),
        );
        let mut actions = div().flex().flex_wrap().gap(px(T::S6));
        let sid = session.id.clone();
        actions = actions.child(
            button("here-defer", "暂存…")
                .text_color(T::color(T::POPOVER_TEXT))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.here = Some(sid.clone());
                    this.overview = false;
                    this.show_form(FormKind::Defer, window, cx);
                })),
        );
        for (name, outcome) in [
            ("已验收", Outcome::Completed),
            ("放弃", Outcome::Abandoned),
            ("撤销结局", Outcome::Open),
        ]
        .into_iter()
        .filter(|(_, o)| *o != session.outcome)
        {
            let sid = session.id.clone();
            actions = actions.child(
                button(gpui::SharedString::from(format!("here-{name}")), name)
                    .text_color(T::color(T::POPOVER_TEXT))
                    .on_click(cx.listener(move |this, _, _, cx| {
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
        for h in self
            .workspace
            .outcome_history
            .iter()
            .rev()
            .filter(|h| h.session == session.id)
            .take(3)
        {
            panel = panel.child(
                label(format!(
                    "{} · {} → {}",
                    h.reason,
                    outcome_label(&h.before),
                    outcome_label(&h.after)
                ))
                .text_color(T::color(T::POPOVER_TEXT)),
            );
        }
        if session.current && session.outcome != Outcome::Open {
            panel = panel.child(label(
                "结局已记录，Agent 仍在运行；若要停止，请在画面菜单选择停止 Agent。",
            ));
        }
        panel.child(actions)
    }
    fn session_hidden(&self, s: &AgentSession) -> Option<&str> {
        self.workspace
            .content
            .get(&s.id)
            .map_or(Some("缺少交互入口证据"), |c| c.hidden_for(s))
    }
    fn enter_card(&mut self, s: &AgentSession, window: &mut Window, cx: &mut Context<Self>) {
        self.card_menu = None;
        self.filter_space = None;
        self.changed_only = false;
        self.show_stale = self.group(s) == Group::Stale || self.show_stale;
        self.show_running = self.group(s) == Group::Running || self.show_running;
        if live_terminal(&self.workspace, s).is_some() {
            self.open_session(s, window, cx);
        } else {
            self.overview = true;
            self.sync_panes(cx);
            self.here = Some(s.id.clone());
            cx.notify();
        }
    }
    fn session_actions(
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
            .bg(T::color(T::RAISED))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::CONTROL))
            .rounded(px(T::CONTROL_RADIUS))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap(px(T::S4))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                button("card-save", "存为待办").on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.card_menu = None;
                    this.save_session_task(&save, cx);
                })),
            )
            .child(button("card-defer", "暂存").on_click(cx.listener(
                move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.card_menu = None;
                    this.here = Some(defer.clone());
                    this.show_form(FormKind::Defer, window, cx);
                },
            )));
        for (label, outcome) in [
            ("已验收", Outcome::Completed),
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
                        this.card_menu = None;
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
        self.cleanup.clear();
        self.form = None;
        self.search_open = false;
        if let Some(session) = self
            .workspace
            .agent_sessions
            .iter()
            .find(|s| s.id == id)
            .cloned()
        {
            self.enter_card(&session, window, cx);
            // Report actual selected layout and focused pane, not a queued route intent.
            if !self.overview
                && let Some(terminal) = live_terminal(&self.workspace, &session)
                && self.focused.as_deref() == Some(terminal.id.as_str())
                && let Some(tab) = self.workspace.terminal_tab(&terminal.id)
                && !tab.closed
                && self.tab.as_deref() == Some(tab.id.as_str())
                && let Some(pane) = self.panes.get(&terminal.id)
                && pane.focus_handle(cx).is_focused(window)
            {
                crate::desktop::route_applied(id, &terminal.id, &tab.id);
            }
        } else {
            self.error = Some("会话尚未载入，请刷新工作台".into());
            self.request(Request::Workspace, cx);
        }
    }
    fn settings_button(&self, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        button("settings", "⚙")
            .aria_label("设置")
            .w(px(T::ROW))
            .px(px(T::S4))
            .on_click(
                cx.listener(|this, _, window, cx| this.show_form(FormKind::Settings, window, cx)),
            )
    }
    fn matches_filter(&self, s: &AgentSession) -> bool {
        self.filter_space
            .as_deref()
            .is_none_or(|space| session_space(&self.workspace, s) == Some(space))
            && (!self.changed_only || changed_since(&self.workspace, s, self.since))
    }
    fn search_button(&self, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        div()
            .id("global-search")
            .w(px(T::SEARCH_WIDTH))
            .min_w(px(T::SEARCH_MIN))
            .child(self.search.clone())
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.search_open = true;
                    window.focus(&this.search.focus_handle(cx), cx);
                    cx.notify();
                }),
            )
    }
    fn navigation_filters(&self, cx: &Context<Self>) -> gpui::Div {
        let mut row = div().flex().flex_wrap().gap(px(T::S8)).child(
            button(
                "all-spaces",
                if self.filter_space.is_none() {
                    "● 全部空间"
                } else {
                    "全部空间"
                },
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.filter_space = None;
                cx.notify();
            })),
        );
        for space in &self.workspace.spaces {
            let id = space.id.clone();
            row = row.child(
                button(
                    gpui::SharedString::from(format!("filter-{id}")),
                    format!(
                        "{}{}",
                        if self.filter_space.as_ref() == Some(&id) {
                            "● "
                        } else {
                            ""
                        },
                        space.name
                    ),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.filter_space = Some(id.clone());
                    cx.notify();
                })),
            );
        }
        row.child(
            button(
                "only-changed",
                if self.changed_only {
                    "✓ 只看有变化"
                } else {
                    "只看有变化"
                },
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.changed_only = !this.changed_only;
                cx.notify();
            })),
        )
    }
    fn search_view(&self, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        let query = self.search.read(cx).text.trim().to_lowercase();
        let mut view = div()
            .id("search-results")
            .size_full()
            .overflow_y_scroll()
            .p(px(T::S20))
            .flex()
            .flex_col()
            .gap(px(T::S12))
            .child(
                div()
                    .flex()
                    .gap(px(T::S12))
                    .child(div().flex_1().child(self.search.clone()))
                    .child(button("close-search", "关闭搜索").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.search_open = false;
                            cx.notify();
                        },
                    ))),
            );
        if query.is_empty() {
            return view.child(label(
                "按标题、项目或用户原话查找；搜索只使用本地已索引内容。",
            ));
        }
        let mut count = 0;
        for task in &self.workspace.tasks {
            if !format!("{} {}", task.title, task.description)
                .to_lowercase()
                .contains(&query)
            {
                continue;
            }
            let id = task.id.clone();
            let space = task.space.clone();
            count += 1;
            view = view.child(
                button(
                    gpui::SharedString::from(format!("search-task-{id}")),
                    format!("待办 · {}", task.title),
                )
                .justify_start()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.search_open = false;
                    this.overview = false;
                    this.space = space.clone();
                    this.task_detail = Some(id.clone());
                    cx.notify();
                })),
            );
        }
        for session in &self.workspace.agent_sessions {
            if self.session_hidden(session).is_some() {
                continue;
            }
            let quote = self
                .workspace
                .content
                .get(&session.id)
                .and_then(|c| {
                    c.asks
                        .iter()
                        .find(|q| q.text.to_lowercase().contains(&query))
                })
                .map(|q| q.text.clone());
            if quote.is_none()
                && !format!(
                    "{} {} {}",
                    self.session_title(session),
                    session.cli_session_id,
                    session.cwd
                )
                .to_lowercase()
                .contains(&query)
            {
                continue;
            }
            let open = session.clone();
            count += 1;
            let snippet = quote
                .map(|q| q.chars().take(160).collect::<String>())
                .unwrap_or_else(|| session.cwd.clone());
            view = view.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(T::S4))
                    .child(
                        button(
                            gpui::SharedString::from(format!("search-session-{}", session.id)),
                            format!("会话 · {}", self.session_title(session)),
                        )
                        .justify_start()
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.search_open = false;
                                this.enter_card(&open, window, cx);
                            },
                        )),
                    )
                    .child(label(snippet)),
            );
        }
        view.child(label(format!(
            "共 {count} 项；未索引历史不会出现在搜索中。"
        )))
    }
    /// Only Keepline's own terminals ask for attention; scanned-only sessions stay in the overview.
    fn needs_you(&self) -> Vec<&AgentSession> {
        self.workspace
            .agent_sessions
            .iter()
            .filter(|s| {
                s.managed()
                    && s.outcome.needs_review(now())
                    && self.session_hidden(s).is_none()
                    && matches!(self.group(s), Group::Waiting | Group::Error)
            })
            .collect()
    }
    fn next_waiting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let needs = self.needs_you();
        let current = self
            .focused
            .as_ref()
            .and_then(|id| self.workspace.current_agent(id))
            .map(|s| s.id.as_str())
            .or(self.here.as_deref());
        if !needs.is_empty() {
            let index = needs
                .iter()
                .position(|s| Some(s.id.as_str()) == current)
                .map_or(0, |i| (i + 1) % needs.len());
            let session = needs[index].clone();
            self.enter_card(&session, window, cx);
        }
    }
    fn overview_view(&self, window: &Window, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        let open: Vec<_> = self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| {
                s.outcome.needs_review(now())
                    && self.session_hidden(s).is_none()
                    && self.matches_filter(s)
            })
            .collect();
        let header = div()
            .flex()
            .items_center()
            .gap(px(T::S16))
            .child(
                div()
                    .flex_1()
                    .child(
                        div()
                            .text_size(px(T::TITLE))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("欢迎回来"),
                    )
                    .child(label(format!(
                        "{} 个未闭环会话 · 按日志回合与机械证据分组",
                        open.len()
                    ))),
            )
            .child(self.settings_button(cx))
            .child(self.search_button(cx))
            .child(
                button("overview-refresh", "刷新")
                    .on_click(cx.listener(|this, _, _, cx| this.request(Request::Workspace, cx))),
            )
            .child(
                button("back", "← 回到工作区")
                    .text_color(T::color(T::LINK))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.overview = false;
                        this.sync_panes(cx);
                        cx.notify();
                    })),
            );
        let mut view = div()
            .id("overview-scroll")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(T::S20))
            .px(px(T::OVERVIEW_X))
            .py(px(T::S20))
            .child(header)
            .child(self.navigation_filters(cx))
            .when_some(self.error.clone(), |d, error| {
                d.child(div().text_color(Group::Error.colors().1).child(error))
            });
        let available = f32::from(window.viewport_size().width) - T::OVERVIEW_X * 2.;
        let columns = ((available + T::S10) / (T::CARD_MIN + T::S10))
            .floor()
            .max(1.);
        let card_width = ((available - T::S10 * (columns - 1.)) / columns).floor();
        let unstarted: Vec<_> = self
            .workspace
            .tasks
            .iter()
            .filter(|t| {
                self.workspace
                    .task_outcomes
                    .get(&t.id)
                    .unwrap_or(&Outcome::Open)
                    .needs_review(now())
                    && self.filter_space.as_ref().is_none_or(|id| id == &t.space)
                    && !self.changed_only
                    && !self
                        .workspace
                        .agent_sessions
                        .iter()
                        .any(|s| s.task.as_ref() == Some(&t.id))
            })
            .collect();
        let mut todo_cards = div().flex().flex_wrap().gap(px(T::S10));
        for task in &unstarted {
            let id = task.id.clone();
            todo_cards = todo_cards.child(
                div()
                    .w(px(card_width - T::S8))
                    .min_w(px(T::CARD_MIN))
                    .p(px(T::S12))
                    .rounded(px(T::CARD_RADIUS))
                    .bg(T::color(T::TODO))
                    .border(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::CARD_BORDER))
                    .flex()
                    .flex_col()
                    .gap(px(T::S8))
                    .child(
                        div()
                            .truncate()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(task.title.clone()),
                    )
                    .child(label(format!(
                        "{} · {} 条验收标准",
                        task.agent.executable(),
                        task.acceptance.len()
                    )))
                    .child(
                        button(gpui::SharedString::from(format!("start-{id}")), "开始 →")
                            .bg(T::color(T::ACCENT))
                            .text_color(T::color(T::WHITE))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.ask_agent(id.clone(), false, cx)
                            })),
                    )
                    .children(self.agent_picker(&task.id, cx)),
            );
        }
        view = view.child(
            div()
                .p(px(T::S16))
                .rounded(px(T::POPOVER_RADIUS))
                .border(px(T::BORDER_WIDTH))
                .border_color(T::color(T::TODO_BORDER))
                .bg(T::color(T::SLOT))
                .flex()
                .flex_col()
                .gap(px(T::S10))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(T::S10))
                        .child(
                            div()
                                .text_color(T::color(T::LINK))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child("☐ 待办 · 还没开始"),
                        )
                        .child(label(unstarted.len().to_string()))
                        .child(label("命令只预填，按回车才启动"))
                        .child(div().flex_1())
                        .child(button("overview-new-task", "＋ 新待办").on_click(
                            cx.listener(|this, _, window, cx| this.show_quick_task(window, cx)),
                        )),
                )
                .child(todo_cards),
        );
        for group in Group::ALL {
            if group == Group::Running {
                let count = open
                    .iter()
                    .filter(|s| self.group(s) == Group::Running)
                    .count();
                view = view.child(
                    button(
                        "toggle-running",
                        format!(
                            "进行中 {count} · {}",
                            if self.show_running {
                                "收起"
                            } else {
                                "展开"
                            }
                        ),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_running = !this.show_running;
                        cx.notify();
                    })),
                );
                if !self.show_running {
                    continue;
                }
            }
            if group == Group::Stale {
                let count = open
                    .iter()
                    .filter(|s| self.group(s) == Group::Stale)
                    .count();
                view = view.child(
                    button(
                        "toggle-stale",
                        format!(
                            "很久没动 {count} · {}",
                            if self.show_stale {
                                "收起"
                            } else {
                                "展开核对"
                            }
                        ),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_stale = !this.show_stale;
                        cx.notify();
                    })),
                );
                if count > 0 {
                    view = view.child(button("cleanup", "逐个清理 →").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.cleanup = this
                                .workspace
                                .agent_sessions
                                .iter()
                                .filter(|s| {
                                    this.session_hidden(s).is_none()
                                        && this.group(s) == Group::Stale
                                })
                                .map(|s| s.id.clone())
                                .collect();
                            this.cleanup_index = 0;
                            this.here = None;
                            cx.notify();
                        },
                    )));
                }
                if !self.show_stale {
                    continue;
                }
            }
            let sessions: Vec<_> = open.iter().filter(|s| self.group(s) == group).collect();
            let mut cards = div().flex().flex_wrap().gap(px(T::S10));
            for session in &sessions {
                let session = (**session).clone();
                let enter = session.clone();
                let primary = session.clone();
                let menu_id = session.id.clone();
                let meta = format!(
                    "{} · {} · {}{}{}",
                    if session.terminal.is_none() {
                        "外部·只读"
                    } else {
                        "托管"
                    },
                    session.runtime.executable(),
                    session_space(&self.workspace, &session)
                        .and_then(|id| self.workspace.spaces.iter().find(|s| s.id == id))
                        .map_or("外部", |s| s.name.as_str()),
                    if self
                        .workspace
                        .outcome_history
                        .iter()
                        .rev()
                        .find(|h| h.session == session.id)
                        .is_some_and(|h| h.reason == "暂存到期")
                    {
                        " · 到期"
                    } else {
                        ""
                    },
                    if changed_since(&self.workspace, &session, self.since) {
                        " · 新"
                    } else {
                        ""
                    }
                );
                let primary_continue =
                    matches!(group, Group::Waiting | Group::Error | Group::Paused);
                cards = cards.child(
                    div()
                        .id(gpui::SharedString::from(format!("card-{}", session.id)))
                        .role(gpui::accesskit::Role::Button)
                        .aria_label(gpui::SharedString::from(format!(
                            "会话：{}",
                            self.session_title(&session)
                        )))
                        .relative()
                        .w(px(card_width))
                        .min_w(px(T::CARD_MIN))
                        .p(px(T::S12))
                        .rounded(px(T::CARD_RADIUS))
                        .border(px(T::BORDER_WIDTH))
                        .border_color(T::color(T::CARD_BORDER))
                        .bg(T::color(T::HEADER))
                        .flex()
                        .flex_col()
                        .gap(px(T::S6))
                        .cursor_pointer()
                        .hover(|s| s.bg(T::color(T::RAISED)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.enter_card(&enter, window, cx)
                        }))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(T::S6))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .text_size(px(T::CARD_TITLE))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .truncate()
                                        .child(self.session_title(&session)),
                                )
                                .child(
                                    button(
                                        gpui::SharedString::from(format!("more-{}", session.id)),
                                        "⋯",
                                    )
                                    .border_0()
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            cx.stop_propagation();
                                            this.here = None;
                                            this.card_menu =
                                                if this.card_menu.as_ref() == Some(&menu_id) {
                                                    None
                                                } else {
                                                    Some(menu_id.clone())
                                                };
                                            cx.notify();
                                        },
                                    )),
                                ),
                        )
                        .child(label(meta).truncate())
                        .child(
                            div()
                                .text_size(px(T::MONO_SIZE))
                                .line_clamp(3)
                                .text_color(group.colors().1)
                                .child(self.session_line(&session)),
                        )
                        .child(
                            div().flex().justify_end().child(
                                button(
                                    gpui::SharedString::from(format!("primary-{}", session.id)),
                                    if primary_continue {
                                        "继续 →"
                                    } else {
                                        "你在这里"
                                    },
                                )
                                .text_color(T::color(T::LINK))
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        cx.stop_propagation();
                                        this.card_menu = None;
                                        if primary_continue {
                                            this.open_session(&primary, window, cx);
                                        } else {
                                            this.here = Some(primary.id.clone());
                                            cx.notify();
                                        }
                                    },
                                )),
                            ),
                        )
                        .when(self.card_menu.as_ref() == Some(&session.id), |d| {
                            d.child(
                                gpui::deferred(
                                    self.session_actions(&session, cx)
                                        .absolute()
                                        .top(px(T::ROW + T::S12))
                                        .right(px(T::S12))
                                        .occlude(),
                                )
                                .priority(1),
                            )
                        })
                        .when(self.here.as_ref() == Some(&session.id), |d| {
                            d.child(self.here_card(&session, cx))
                        }),
                );
            }
            let section = div()
                .flex()
                .flex_col()
                .gap(px(T::S10))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(T::S8))
                        .child(dot(group))
                        .child(
                            div()
                                .text_color(group.colors().1)
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(group.label()),
                        )
                        .child(label(sessions.len().to_string()))
                        .when(sessions.is_empty(), |d| d.child(label("暂无会话"))),
                )
                .when(!sessions.is_empty(), |d| d.child(cards));
            view = view.child(section);
        }
        let completed = self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| s.outcome == Outcome::Completed && self.session_hidden(s).is_none())
            .count();
        let abandoned = self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| s.outcome == Outcome::Abandoned && self.session_hidden(s).is_none())
            .count();
        let deferred = self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| {
                matches!(s.outcome, Outcome::Deferred { .. }) && self.session_hidden(s).is_none()
            })
            .count();
        view = view.child(
            div()
                .border_t(px(T::BORDER_WIDTH))
                .border_color(T::color(T::BORDER))
                .pt(px(T::S16))
                .child(button("closed-toggle",format!("暂存 {deferred} · 已结束：完成 {completed} / 放弃 {abandoned} · 展开/收起")).on_click(cx.listener(|this, _, _, cx| {this.show_closed = !this.show_closed;cx.notify();}))),
        );
        if self.show_closed {
            for s in self
                .workspace
                .agent_sessions
                .iter()
                .filter(|s| !s.outcome.needs_review(now()) && self.session_hidden(s).is_none())
            {
                let sid = s.id.clone();
                view = view.child(
                    div()
                        .p(px(T::S8))
                        .child(
                            button(
                                gpui::SharedString::from(format!("closed-{sid}")),
                                format!(
                                    "{} · {}",
                                    self.session_title(s),
                                    outcome_label(&s.outcome)
                                ),
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.here = Some(sid.clone());
                                    cx.notify();
                                },
                            )),
                        )
                        .when(self.here.as_ref() == Some(&s.id), |d| {
                            d.child(self.here_card(s, cx))
                        }),
                );
            }
        }
        let hidden: Vec<_> = self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| self.session_hidden(s).is_some())
            .collect();
        view = view.child(
            button(
                "toggle-hidden",
                format!(
                    "已隐藏 {} 个自动化会话 · {}",
                    hidden.len(),
                    if self.show_hidden {
                        "收起"
                    } else {
                        "展开查看"
                    }
                ),
            )
            .border_0()
            .justify_start()
            .text_color(T::color(T::SECONDARY))
            .on_click(cx.listener(|this, _, _, cx| {
                this.show_hidden = !this.show_hidden;
                cx.notify();
            })),
        );
        if self.show_hidden {
            for s in hidden {
                let sid = s.id.clone();
                view = view.child(
                    div()
                        .p(px(T::S8))
                        .border_b(px(T::BORDER_WIDTH))
                        .border_color(T::color(T::BORDER))
                        .child(
                            button(
                                gpui::SharedString::from(format!("hidden-{}", s.id)),
                                self.session_title(s),
                            )
                            .border_0()
                            .justify_start()
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.here = Some(sid.clone());
                                    cx.notify();
                                },
                            )),
                        )
                        .child(label(format!(
                            "{} · {} · {} · {}",
                            self.session_hidden(s).unwrap_or(""),
                            s.runtime.executable(),
                            s.cli_session_id,
                            s.cwd
                        )))
                        .when(self.here.as_ref() == Some(&s.id), |d| {
                            d.child(self.here_card(s, cx))
                        }),
                );
            }
        }
        view
    }

    fn pane_view(
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
            .border_color(T::color(if focused { T::ACCENT } else { T::BORDER }))
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
}
impl Workbench {
    fn layout_view(
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
                            .hover(|d| d.bg(T::color(T::ACCENT)))
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
    fn resize_drag(
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
    fn finish_drag(&mut self, cx: &mut Context<Self>) {
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
    fn menu_action(&mut self, id: &str, action: &str, window: &mut Window, cx: &mut Context<Self>) {
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
    fn menu_view(&self, menu: &PaneMenu, window: &Window, cx: &Context<Self>) -> gpui::AnyElement {
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
        if self.search_open {
            return root.child(self.search_view(cx));
        }
        if !self.cleanup.is_empty() && self.form.is_none() {
            return root.child(self.cleanup_view(cx));
        }
        if self.overview && self.form.is_none() {
            return root.child(self.overview_view(window, cx));
        }
        let space_name = self
            .workspace
            .spaces
            .iter()
            .find(|s| s.id == self.space)
            .map(|s| s.name.as_str())
            .unwrap_or("工作空间");
        root =
            root.child(
                div()
                    .h(px(T::TOP))
                    .flex_shrink_0()
                    .px(px(T::S14))
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
                            .bg(T::color(T::ACCENT)),
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
                    .child(self.search_button(cx))
                    .child(div().flex_1())
                    .child(button("add-task", "＋ 新待办").on_click(
                        cx.listener(|this, _, window, cx| this.show_quick_task(window, cx)),
                    ))
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
        let mut sidebar = div()
            .id("sidebar")
            .w(px(T::SIDEBAR))
            .flex_shrink_0()
            .h_full()
            .overflow_y_scroll()
            .px(px(T::S8))
            .py(px(T::S12))
            .flex()
            .flex_col()
            .gap(px(T::S2))
            .bg(T::color(T::PANEL))
            .border_r(px(T::BORDER_WIDTH))
            .border_color(T::color(T::BORDER))
            .child(label("空间").px(px(T::S8)).mb(px(T::S4)));
        for space in &self.workspace.spaces {
            let id = space.id.clone();
            let active = self.space == id;
            let sessions: Vec<_> = self
                .workspace
                .agent_sessions
                .iter()
                .filter(|s| {
                    self.session_hidden(s).is_none()
                        && session_space(&self.workspace, s) == Some(id.as_str())
                })
                .collect();
            let count = sessions.len();
            let waiting = sessions
                .iter()
                .filter(|s| {
                    s.managed() && s.outcome.needs_review(now()) && self.group(s) == Group::Waiting
                })
                .count();
            let errors = sessions
                .iter()
                .filter(|s| {
                    s.managed() && s.outcome.needs_review(now()) && self.group(s) == Group::Error
                })
                .count();
            sidebar = sidebar.child(
                button(gpui::SharedString::from(format!("space-{id}")), "")
                    .aria_label(space.name.clone())
                    .gap(px(T::S8))
                    .child(dot(if errors > 0 {
                        Group::Error
                    } else if waiting > 0 {
                        Group::Waiting
                    } else {
                        sessions
                            .iter()
                            .filter(|s| s.outcome.needs_review(now()))
                            .map(|s| self.group(s))
                            .find(|g| !matches!(g, Group::Waiting | Group::Error))
                            .unwrap_or(Group::Stale)
                    }))
                    .child(div().min_w_0().truncate().child(space.name.clone()))
                    .justify_start()
                    .px(px(T::S8))
                    .border_0()
                    .when(active, |s| s.bg(T::color(T::RAISED)))
                    .child(div().flex_1())
                    .when(waiting > 0, |d| d.child(badge(Group::Waiting, waiting)))
                    .when(errors > 0, |d| d.child(badge(Group::Error, errors)))
                    .child(label(count.to_string()).text_size(px(T::CHIP_SIZE)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.space = id.clone();
                        this.tab = this
                            .workspace
                            .tabs
                            .iter()
                            .find(|t| t.space == id && !t.closed)
                            .map(|t| t.id.clone());
                        this.sync_panes(cx);
                        cx.notify();
                    })),
            );
        }
        sidebar = sidebar
            .child(
                button("add-space", "＋ 空间")
                    .border_0()
                    .justify_start()
                    .text_color(T::color(T::MUTED))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.show_form(FormKind::Space, window, cx)
                    })),
            )
            .child(
                div()
                    .mt(px(T::S14))
                    .pt(px(T::S12))
                    .px(px(T::S8))
                    .mb(px(T::S4))
                    .flex()
                    .items_center()
                    .child(label(format!("{space_name} 的待办")).flex_1())
                    .child(
                        button("sidebar-new-task", "＋")
                            .aria_label("新待办")
                            .border_0()
                            .text_color(T::color(T::LINK))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.show_quick_task(window, cx)),
                            ),
                    ),
            )
            .children(self.quick_task_row(cx));
        // First open: say what the empty pane is for before asking for a task.
        if self.quick_task.is_none() && !self.workspace.tasks.iter().any(|t| t.space == self.space)
        {
            sidebar = sidebar.child(
                div()
                    .px(px(T::S8))
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap(px(T::S6))
                    .child(label(
                        "还没有待办。可以直接在右边的终端里运行 codex 或 claude，Keepline 会自动跟踪；也可以先记一条待办。",
                    ))
                    .child(
                        button("empty-new-task", "＋ 新建待办")
                            .h(px(T::ICON_HEIGHT))
                            .px(px(T::S6))
                            .text_size(px(T::META))
                            .text_color(T::color(T::LINK))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.show_quick_task(window, cx)),
                            ),
                    ),
            );
        }
        for task in self
            .workspace
            .tasks
            .iter()
            .filter(|t| t.space == self.space)
        {
            let tid = task.id.clone();
            let start = tid.clone();
            let count = self
                .workspace
                .agent_sessions
                .iter()
                .filter(|s| s.task.as_ref() == Some(&tid))
                .count();
            let outcome = self
                .workspace
                .task_outcomes
                .get(&tid)
                .unwrap_or(&Outcome::Open);
            let state = if *outcome == Outcome::Open {
                if count == 0 {
                    "未开始".into()
                } else {
                    format!("未完成 · {count} 个会话")
                }
            } else {
                outcome_label(outcome)
            };
            sidebar = sidebar.child(
                div()
                    .id(gpui::SharedString::from(format!("task-{tid}")))
                    .h(px(T::TASK_ROW))
                    .px(px(T::S8))
                    .flex()
                    .items_center()
                    .gap(px(T::S8))
                    .rounded(px(T::CONTROL_RADIUS))
                    .cursor_pointer()
                    .hover(|s| s.bg(T::color(T::RAISED)))
                    .child(
                        div()
                            .size(px(T::S8))
                            .flex_shrink_0()
                            .rounded_full()
                            .border(px(T::BORDER_WIDTH))
                            .border_color(T::color(T::LINK))
                            .when(count > 0, |d| d.bg(T::color(T::ACCENT))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(T::MONO_SIZE))
                                    .child(task.title.clone()),
                            )
                            .child(label(state).text_size(px(T::CHIP_SIZE))),
                    )
                    .child(
                        button(
                            gpui::SharedString::from(format!("start-task-{tid}")),
                            "开始",
                        )
                        .h(px(T::ICON_HEIGHT))
                        .px(px(T::S6))
                        .text_size(px(T::META))
                        .text_color(T::color(T::LINK))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.ask_agent(start.clone(), false, cx);
                        })),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.task_detail = Some(tid.clone());
                        cx.notify();
                    })),
            );
            if let Some(picker) = self.agent_picker(&task.id, cx) {
                sidebar = sidebar.child(picker);
            }
        }
        let needs = self.needs_you();
        sidebar = sidebar.child(
            label(format!("需要你 · {}", needs.len()))
                .mt(px(T::S14))
                .pt(px(T::S12))
                .px(px(T::S8))
                .mb(px(T::S4)),
        );
        for session in needs.into_iter().take(6) {
            let open = session.clone();
            let g = self.group(session);
            sidebar = sidebar.child(
                div()
                    .id(gpui::SharedString::from(format!("need-{}", session.id)))
                    .h(px(T::ROW))
                    .px(px(T::S8))
                    .flex()
                    .items_center()
                    .gap(px(T::S8))
                    .rounded(px(T::CONTROL_RADIUS))
                    .cursor_pointer()
                    .hover(|s| s.bg(T::color(T::RAISED)))
                    .child(dot(g))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(T::MONO_SIZE))
                            .child(self.session_title(session)),
                    )
                    .child(
                        label(
                            session_space(&self.workspace, session)
                                .and_then(|id| self.workspace.spaces.iter().find(|p| p.id == id))
                                .map(|p| p.name.clone())
                                .unwrap_or_default(),
                        )
                        .text_size(px(T::CHIP_SIZE))
                        .max_w(px(T::SIDEBAR / T::S4))
                        .truncate(),
                    )
                    // Straight to its pane; an exited one resumes in a new pane.
                    .on_click(
                        cx.listener(move |this, _, window, cx| {
                            this.open_session(&open, window, cx)
                        }),
                    ),
            );
        }
        sidebar = sidebar
            .child(div().flex_1())
            .child(
                button("sidebar-overview", "▦ 鸟瞰全部").on_click(cx.listener(|this, _, _, cx| {
                    this.overview = true;
                    this.sync_panes(cx);
                    cx.notify();
                })),
            )
            .child(
                label(format!(
                    "{} 个会话 · 终端宿主已连接",
                    self.workspace.agent_sessions.len()
                ))
                .px(px(T::S8))
                .pt(px(T::S20)),
            );
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
                    .font_family(T::mono(cx).family)
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
            let mut panel = div()
                .id("editor-form")
                .m(px(T::S20))
                .p(px(T::S20))
                .w(px(T::FORM_WIDTH))
                .max_w_full()
                .overflow_y_scroll()
                .border(px(T::BORDER_WIDTH))
                .border_color(T::color(T::CONTROL))
                .rounded(px(T::POPOVER_RADIUS))
                .bg(T::color(T::HEADER))
                .flex()
                .flex_col()
                .gap(px(T::S16))
                .child(div().text_size(px(T::FORM_TITLE)).child(match form.kind {
                    FormKind::Task if self.editing_task.is_some() => "编辑待办",
                    FormKind::Task => "新建待办",
                    FormKind::Settings => "设置",
                    FormKind::Space => "新建空间",
                    _ => "暂存",
                }));
            for (index, field) in form.fields.iter().enumerate() {
                panel = panel.child(field.clone());
                if matches!(form.kind, FormKind::Task) && index == 0 {
                    let mut projects = div().flex().flex_wrap().gap(px(T::S6));
                    for space in &self.workspace.spaces {
                        let sid = space.id.clone();
                        projects = projects.child(
                            button(
                                gpui::SharedString::from(format!("project-{sid}")),
                                space.name.clone(),
                            )
                            .when(form.space == sid, |d| {
                                d.bg(T::color(T::CHIP)).text_color(T::color(T::LINK))
                            })
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    if let Some(f) = &mut this.form {
                                        f.space = sid.clone();
                                    }
                                    cx.notify();
                                },
                            )),
                        );
                    }
                    panel = panel.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(T::S6))
                            .child(label("项目"))
                            .child(projects),
                    );
                }
            }
            if matches!(form.kind, FormKind::Task | FormKind::Settings) {
                let mut agents = div().flex().gap(px(T::S8));
                for agent in [Agent::Codex, Agent::Claude] {
                    agents = agents.child(
                        button(agent.executable(), agent.executable())
                            .when(form.agent == agent, |d| {
                                d.bg(T::color(T::CHIP)).text_color(T::color(T::LINK))
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(f) = &mut this.form {
                                    f.agent = agent;
                                }
                                cx.notify();
                            })),
                    );
                }
                panel = panel.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(T::S6))
                        .child(label("默认 Agent"))
                        .child(agents),
                );
            }
            if matches!(form.kind, FormKind::Settings) {
                panel = panel
                    .child(
                        button(
                            "notifications-toggle",
                            if form.notifications {
                                "✓ 系统通知已开启"
                            } else {
                                "系统通知关闭（点击开启）"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(form) = &mut this.form {
                                form.notifications = !form.notifications;
                            }
                            cx.notify();
                        })),
                    )
                    .child(label(
                        "保存开启后由 macOS 请求通知权限；默认关闭，退出 App 后不接收通知。",
                    ));
                if let Some(status) = cx.try_global::<crate::desktop::Status>() {
                    panel = panel.child(label(status.0.clone()));
                }
            }
            main = main.child(
                panel.child(
                    div()
                        .flex()
                        .gap(px(T::S8))
                        .child(
                            button(
                                "save",
                                if matches!(form.kind, FormKind::Task) {
                                    "保存为待办"
                                } else {
                                    "保存"
                                },
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.save_form(false, cx))),
                        )
                        .when(matches!(form.kind, FormKind::Task), |d| {
                            d.child(
                                button("save-start", "保存并开始")
                                    .bg(T::color(T::ACCENT))
                                    .text_color(T::color(T::WHITE))
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.save_form(true, cx)),
                                    ),
                            )
                        })
                        .child(
                            button("cancel", "取消").on_click(cx.listener(|this, _, _, cx| {
                                this.form = None;
                                cx.notify();
                            })),
                        ),
                ),
            );
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
            main =
                main.child(grid).child(
                    div()
                        .h(px(T::FOOTER))
                        .flex_shrink_0()
                        .px(px(T::S14))
                        .flex()
                        .items_center()
                        .gap(px(T::S16))
                        .bg(T::color(T::PANEL))
                        .border_t(px(T::BORDER_WIDTH))
                        .border_color(T::color(T::BORDER))
                        .child(label(format!(
                            "本标签 {count} 个画面 · 共 {} 个会话",
                            self.workspace.agent_sessions.len()
                        )))
                        .child(div().flex_1())
                        .when(!self.needs_you().is_empty(), |d| {
                            d.child(
                                button("next-waiting", "跳到下一个等你的 →")
                                    .h(px(T::FOOTER))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.next_waiting(window, cx)
                                    })),
                            )
                        })
                        .child(label(if self.busy {
                            "正在更新…"
                        } else {
                            "终端宿主已连接"
                        })),
                );
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Workspace {
        let mut w = Workspace::default();
        w.spaces.push(Space {
            id: "space".into(),
            name: "Space".into(),
            cwd: "/tmp".into(),
        });
        w.tabs.push(Tab {
            id: "tab".into(),
            space: "space".into(),
            title: "four panes".into(),
            layout: Some({
                let mut l = Layout::Leaf("t0".into());
                for i in 1..4 {
                    l.split(&format!("t{}", i - 1), format!("t{i}"), Axis::Right);
                }
                l
            }),
            default_task: None,
            closed: false,
        });
        for i in 0..4 {
            w.terminals.push(Terminal {
                id: format!("t{i}"),
                space: "space".into(),
                argv: vec!["/bin/sh".into()],
                cwd: "/tmp".into(),
                pid: None,
                state: if i == 1 {
                    ProcessState::Interrupted
                } else {
                    ProcessState::Running
                },
                exit_code: None,
                error: None,
            });
        }
        w.agent_sessions.push(AgentSession {
            id: "history".into(),
            runtime: Agent::Codex,
            cli_session_id: "cli".into(),
            terminal: Some("t1".into()),
            current: false,
            task: None,
            outcome: Outcome::Completed,
            updated: 1,
            cwd: "/tmp".into(),
            transcript: "".into(),
        });
        w
    }

    #[test]
    fn navigation_uses_live_identity_and_path_boundaries() {
        let mut w = fixture();
        let mut s = w.agent_sessions[0].clone();
        s.current = true;
        assert!(live_terminal(&w, &s).is_none());
        s.terminal = Some("t0".into());
        assert_eq!(live_terminal(&w, &s).unwrap().id, "t0");
        s.current = false;
        assert!(live_terminal(&w, &s).is_none());
        s.terminal = None;
        s.cwd = "/tmp/project".into();
        assert_eq!(session_space(&w, &s), Some("space"));
        s.cwd = "/tmp-other/project".into();
        assert_eq!(session_space(&w, &s), None);
        w.spaces.push(Space {
            id: "nested".into(),
            name: "Nested".into(),
            cwd: "/tmp/project".into(),
        });
        s.cwd = "/tmp/project/sub".into();
        assert_eq!(session_space(&w, &s), Some("nested"));
        s.updated = 100;
        assert!(!changed_since(&w, &s, 100));
        assert!(changed_since(&w, &s, 99));
        assert!(!changed_since(&w, &s, 0));
        w.content.insert(
            s.id.clone(),
            keepline_core::content::Content {
                updated: 90,
                ..Default::default()
            },
        );
        assert!(
            !changed_since(&w, &s, 99),
            "index mtime cannot mark an old conversation new"
        );
        assert!(!show_on_return(100, 1900, 30));
        assert!(show_on_return(100, 1901, 30));
        assert!(!show_on_return(0, 1901, 30));
    }

    #[test]
    fn interrupted_terminals_never_enter_the_connection_creation_list() {
        let mut w = fixture();
        assert_eq!(
            attachable_ids(&w, w.tab_terminals("tab")),
            ["t0", "t2", "t3"]
        );
        for terminal in &mut w.terminals {
            terminal.state = ProcessState::Interrupted;
        }
        assert!(attachable_ids(&w, w.tab_terminals("tab")).is_empty());
    }

    #[test]
    fn reopening_full_tab_replaces_exact_slot_and_preserves_history() {
        let mut w = fixture();
        let history = w.agent_sessions.clone();
        let mut replacement = w.terminals[1].clone();
        replacement.id = "new".into();
        replacement.state = ProcessState::Running;
        w.terminals.push(replacement);
        let mut tab = w.tabs[0].clone();
        assert!(tab.layout.as_mut().unwrap().replace("t1", "new"));
        w.edit(Edit::PutTab { tab }).unwrap();
        assert_eq!(w.tab_terminals("tab"), ["t0", "new", "t2", "t3"]);
        assert!(w.terminal_tab("t1").is_none());
        assert_eq!(w.terminals[1].state, ProcessState::Interrupted);
        assert_eq!(w.agent_sessions, history);
    }

    #[test]
    fn closing_interrupted_pane_only_removes_its_placement() {
        let mut w = fixture();
        let history = w.agent_sessions.clone();
        let mut tab = w.tabs[0].clone();
        tab.layout = tab.layout.take().unwrap().without("t1");
        w.edit(Edit::PutTab { tab }).unwrap();
        assert_eq!(w.tab_terminals("tab"), ["t0", "t2", "t3"]);
        assert_eq!(w.terminals.len(), 4);
        assert_eq!(w.agent_sessions, history);
    }
}
