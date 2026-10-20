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
    prefill_agent: Option<Agent>,
    resume: Option<(String, String)>,
    here: Option<String>,
    show_stale: bool,
    show_running: bool,
    show_hidden: bool,
    card_menu: Option<String>,
    drag: Option<Vec<bool>>,
    split_bounds: HashMap<Vec<bool>, gpui::Bounds<gpui::Pixels>>,
    overview: bool,
    busy: bool,
    error: Option<String>,
    form: Option<Form>,
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
    kind: FormKind,
    fields: Vec<Entity<Field>>,
    agent: Agent,
}
#[derive(Clone, Copy)]
enum FormKind {
    Space,
    Task,
    Defer,
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
        .hover(|s| s.bg(T::color(T::RAISED)))
        .child(text)
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
    pub fn new(
        socket: PathBuf,
        workspace: Workspace,
        _: &mut Window,
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
        let mut this = Self {
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
            prefill_agent: None,
            resume: None,
            here: None,
            show_stale: false,
            show_running: false,
            show_hidden: false,
            card_menu: None,
            drag: None,
            split_bounds: HashMap::new(),
            overview: false,
            busy: false,
            error: None,
            form: None,
            recognition: RecognitionStatus::default(),
            _recognition_connection: recognition_connection,
        };
        this.sync_panes(cx);
        if this.workspace.terminals.is_empty() {
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
                        this.workspace = w;
                        this.sync_panes(cx);
                    }
                    Ok(_) => (),
                    Err(e) => this.error = Some(e.to_string()),
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
        if !split && let Some(task_id) = &task_id {
            if let Some(tab) = self
                .workspace
                .tabs
                .iter()
                .find(|t| t.default_task.as_ref() == Some(task_id))
            {
                self.tab = Some(tab.id.clone());
                split = true;
            }
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
        let agent = self.prefill_agent.take();
        let resume = self.resume.take();
        let prefill = resume.as_ref().map(|r| r.0.clone()).or_else(|| {
            task_id
                .as_ref()
                .and_then(|id| self.workspace.tasks.iter().find(|t| &t.id == id))
                .map(|task| {
                    let mut task = task.clone();
                    if let Some(agent) = agent {
                        task.agent = agent;
                    }
                    task.command()
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
            let title = task_id
                .as_ref()
                .and_then(|id| self.workspace.tasks.iter().find(|t| &t.id == id))
                .map(|t| t.title.clone())
                .unwrap_or_else(|| "终端".into());
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
        let cwd = resume.map(|r| r.1).unwrap_or_else(|| {
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
                        if let (Some(command), Some(pane)) = (&prefill, this.panes.get(&terminal)) {
                            pane.update(cx, |pane, cx| pane.queue_prefill(command, cx));
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
    fn show_form(&mut self, kind: FormKind, window: &mut Window, cx: &mut Context<Self>) {
        self.overview = false;
        self.sync_panes(cx);
        let fields = match kind {
            FormKind::Space => vec![
                cx.new(|cx| Field::new("空间名称", "", cx)),
                cx.new(|cx| Field::new("工作目录（绝对路径）", "", cx)),
            ],
            FormKind::Defer => vec![cx.new(|cx| Field::new("暂存原因（明天此时再看）", "", cx))],
            FormKind::Task => vec![
                cx.new(|cx| Field::new("待办标题", "", cx)),
                cx.new(|cx| Field::new("说明", "", cx)),
                cx.new(|cx| Field::new("验收标准", "", cx)),
            ],
        };
        window.focus(&fields[0].focus_handle(cx), cx);
        self.form = Some(Form {
            kind,
            fields,
            agent: Agent::Codex,
        });
        cx.notify();
    }
    fn save_form(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self.form.take() else {
            return;
        };
        let values: Vec<String> = form
            .fields
            .iter()
            .map(|f| f.read(cx).text.clone())
            .collect();
        let edit = match form.kind {
            FormKind::Defer => {
                let Some(id) = self.here.clone() else {
                    return;
                };
                Edit::SetOutcome {
                    id,
                    outcome: Outcome::Deferred {
                        reason: values[0].clone(),
                        until: now() + 86400,
                    },
                }
            }
            FormKind::Space => Edit::AddSpace(Space {
                id: id(),
                name: values[0].clone(),
                cwd: values[1].clone(),
            }),
            FormKind::Task => Edit::AddTask(Task {
                id: id(),
                space: self.space.clone(),
                title: values[0].clone(),
                description: values[1].clone(),
                acceptance: values[2]
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| AcceptanceCriterion {
                        id: id(),
                        description: line.trim().into(),
                        evidence: None,
                    })
                    .collect(),
                agent: form.agent,
            }),
        };
        self.edit(edit, cx);
    }
    fn open_session(
        &mut self,
        session: &AgentSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(terminal) = session
            .terminal
            .as_ref()
            .and_then(|id| self.workspace.terminals.iter().find(|t| &t.id == id))
        else {
            self.resume = Some((
                session.runtime.resume_command(&session.cli_session_id),
                session.cwd.clone(),
            ));
            // Keep the overview until Launch/PutTab returns and sync_panes creates entities.
            self.launch(false, None, cx);
            return;
        };
        let terminal_id = terminal.id.clone();
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
        panel = panel.child(
            button("save-here-task", "存为待办")
                .text_color(T::color(T::POPOVER_TEXT))
                .on_click(cx.listener(move |this, _, _, cx| this.save_session_task(&save, cx))),
        );
        let mut actions = div().flex().flex_wrap().gap(px(T::S6));
        let sid = session.id.clone();
        actions = actions.child(
            button("here-defer", "暂存一天")
                .text_color(T::color(T::POPOVER_TEXT))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.here = Some(sid.clone());
                    this.overview = false;
                    this.show_form(FormKind::Defer, window, cx);
                })),
        );
        for (name, outcome) in [("已验收", Outcome::Completed), ("放弃", Outcome::Abandoned)] {
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
        panel.child(actions)
    }
    fn session_hidden(&self, s: &AgentSession) -> Option<&str> {
        self.workspace
            .content
            .get(&s.id)
            .map_or(Some("缺少交互入口证据"), |c| c.hidden_reason())
    }
    fn enter_card(&mut self, s: &AgentSession, window: &mut Window, cx: &mut Context<Self>) {
        self.card_menu = None;
        if s.terminal.is_some() {
            self.open_session(s, window, cx);
        } else {
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
        for (label, outcome) in [("已验收", Outcome::Completed), ("放弃", Outcome::Abandoned)]
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
    fn overview_view(&self, window: &Window, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        let open: Vec<_> = self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| s.outcome.needs_review(now()) && self.session_hidden(s).is_none())
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
            .when_some(self.error.clone(), |d, error| {
                d.child(div().text_color(Group::Error.colors().1).child(error))
            });
        let available = f32::from(window.viewport_size().width) - T::OVERVIEW_X * 2.;
        let columns = ((available + T::S10) / (T::CARD_MIN + T::S10))
            .floor()
            .max(1.);
        let card_width = (available - T::S10 * (columns - 1.)) / columns;
        let unstarted: Vec<_> = self
            .workspace
            .tasks
            .iter()
            .filter(|t| {
                !self
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
                                this.launch(false, Some(id.clone()), cx)
                            })),
                    ),
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
                        .child(
                            button("overview-new-task", "＋ 新待办").on_click(cx.listener(
                                |this, _, window, cx| this.show_form(FormKind::Task, window, cx),
                            )),
                        ),
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
                    "{} · {} · {}",
                    if session.terminal.is_none() {
                        "外部·只读"
                    } else {
                        "托管"
                    },
                    session.runtime.executable(),
                    session.cwd
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
                .child(label(format!(
                    "暂存 {deferred} · 已结束：完成 {completed} / 放弃 {abandoned}"
                ))),
        );
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
                            session.map(|s| s.runtime.executable()).unwrap_or("Shell"),
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
                        button(gpui::SharedString::from(format!("menu-{id}")), "⋯")
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
                    .child(label(pane.read(cx).connection_label()))
                    .child(
                        button(gpui::SharedString::from(format!("close-pane-{id}")), "×")
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
                    .child(pane.clone()),
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
                            format!("{}（仅预填）", agent.executable()),
                        )
                        .w_full()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.menu = None;
                            this.prefill_agent = Some(agent);
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
                    .child(div().flex_1())
                    .child(button("add-task", "＋ 新待办").on_click(cx.listener(
                        |this, _, window, cx| this.show_form(FormKind::Task, window, cx),
                    )))
                    .child(button("refresh", "刷新").on_click(
                        cx.listener(|this, _, _, cx| this.request(Request::Workspace, cx)),
                    )),
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
            let count = self
                .workspace
                .terminals
                .iter()
                .filter(|t| t.space == id)
                .count();
            sidebar = sidebar.child(
                button(
                    gpui::SharedString::from(format!("space-{id}")),
                    space.name.clone(),
                )
                .justify_start()
                .px(px(T::S8))
                .border_0()
                .when(active, |s| s.bg(T::color(T::RAISED)))
                .child(div().flex_1())
                .child(label(count.to_string()))
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
                label(format!("{space_name} 的待办"))
                    .mt(px(T::S14))
                    .pt(px(T::S12))
                    .px(px(T::S8))
                    .mb(px(T::S4)),
            );
        for task in self
            .workspace
            .tasks
            .iter()
            .filter(|t| t.space == self.space)
        {
            let id = task.id.clone();
            sidebar = sidebar.child(
                button(
                    gpui::SharedString::from(format!("task-{id}")),
                    task.title.clone(),
                )
                .border_0()
                .justify_start()
                .px(px(T::S8))
                .overflow_hidden()
                .on_click(
                    cx.listener(move |this, _, _, cx| this.launch(false, Some(id.clone()), cx)),
                ),
            );
        }
        let needs: Vec<_> = self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| {
                s.outcome.needs_review(now())
                    && self.session_hidden(s).is_none()
                    && matches!(self.group(s), Group::Waiting | Group::Error)
            })
            .collect();
        sidebar = sidebar.child(
            label(format!("需要你 · {}", needs.len()))
                .mt(px(T::S14))
                .pt(px(T::S12))
                .px(px(T::S8))
                .mb(px(T::S4)),
        );
        for session in needs {
            let open = session.clone();
            let g = self.group(session);
            sidebar =
                sidebar.child(
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
                        .child(div().truncate().child(self.session_title(session)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_session(&open, window, cx)
                        })),
                );
        }
        sidebar = sidebar.child(div().flex_1()).child(
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
            tabs = tabs.child(
                button(
                    gpui::SharedString::from(format!("tab-{id}")),
                    format!(
                        "{}{}",
                        if tab.default_task.is_some() {
                            "☑ "
                        } else {
                            ""
                        },
                        tab.title
                    ),
                )
                .h(px(T::TABS))
                .border_0()
                .rounded_none()
                .font_family(T::mono(cx).family)
                .text_size(px(T::MONO_SIZE))
                .text_color(T::color(if active { T::PRIMARY } else { T::SECONDARY }))
                .when(active, |s| s.bg(T::color(T::RAISED)))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.tab = Some(id.clone());
                    this.sync_panes(cx);
                    if let Some(pane) = this.panes.values().next() {
                        window.focus(&pane.focus_handle(cx), cx);
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
                .m(px(T::S20))
                .p(px(T::S20))
                .max_w(px(T::FORM_WIDTH))
                .rounded(px(T::POPOVER_RADIUS))
                .bg(T::color(T::HEADER))
                .flex()
                .flex_col()
                .gap(px(T::S16));
            for field in &form.fields {
                panel = panel.child(field.clone());
            }
            if matches!(form.kind, FormKind::Task) {
                panel = panel.child(
                    button(
                        "agent",
                        format!("Agent：{}（点击切换）", form.agent.executable()),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(form) = &mut this.form {
                            form.agent = match form.agent {
                                Agent::Codex => Agent::Claude,
                                Agent::Claude => Agent::Codex,
                            };
                        }
                        cx.notify();
                    })),
                );
            }
            main = main.child(
                panel.child(
                    div()
                        .flex()
                        .gap(px(T::S8))
                        .child(
                            button("save", "保存")
                                .bg(T::color(T::ACCENT))
                                .on_click(cx.listener(|this, _, _, cx| this.save_form(cx))),
                        )
                        .child(
                            button("cancel", "取消").on_click(cx.listener(|this, _, _, cx| {
                                this.form = None;
                                cx.notify();
                            })),
                        ),
                ),
            );
        } else {
            if let Some(task) = current
                .and_then(|t| t.default_task.as_ref())
                .and_then(|id| self.workspace.tasks.iter().find(|t| &t.id == id))
            {
                let command = task.command();
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
                            button("prefill", "填入命令（回车执行）")
                                .h(px(T::FOOTER))
                                .text_color(T::color(T::LINK))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    if let Some(id) = this.tab.as_ref().and_then(|id| {
                                        this.workspace.tab_terminals(id).first().cloned()
                                    }) {
                                        if let Some(pane) = this.panes.get(&id) {
                                            pane.update(cx, |pane, cx| pane.prefill(&command, cx));
                                            window.focus(&pane.focus_handle(cx), cx);
                                        }
                                    }
                                })),
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
            main = main.child(grid).child(
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
