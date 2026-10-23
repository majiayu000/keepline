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
    /// The session whose 「你在这里」 card shows its secondary actions.
    here_more: Option<String>,
    /// A press on the top bar's empty space that has not moved the window yet.
    title_drag: bool,
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
        .text_size(px(T::SMALL))
        .text_color(T::color(T::BODY))
        .cursor_pointer()
        .hover(|s| s.bg(T::color(T::BUTTON_HOVER)))
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

mod cleanup;
mod forms;
mod layout;
mod overview;
mod render;
mod search;
mod sessions;
mod sidebar;
mod tasks;
#[cfg(test)]
mod tests;
mod visual;

impl Workbench {
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
            here_more: None,
            title_drag: false,
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
    pub(super) fn sync_panes(&mut self, cx: &mut Context<Self>) {
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
    pub(super) fn request(&mut self, request: Request, cx: &mut Context<Self>) {
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
    pub(super) fn edit(&mut self, edit: Edit, cx: &mut Context<Self>) {
        self.request(
            Request::Edit {
                revision: self.workspace.revision,
                edit,
            },
            cx,
        );
    }
    pub(super) fn launch(&mut self, split: bool, task_id: Option<String>, cx: &mut Context<Self>) {
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
    pub(super) fn close_pane(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(mut tab) = self.workspace.terminal_tab(id).cloned() else {
            return;
        };
        tab.layout = tab.layout.take().and_then(|l| l.without(id));
        self.menu = None;
        self.zoomed = None;
        self.edit(Edit::PutTab { tab }, cx);
    }
    pub(super) fn launch_in_slot(
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
