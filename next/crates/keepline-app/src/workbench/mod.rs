use crate::host::store::{Store, record_left};
use crate::model::{search::*, session_state::*, time::*};
use crate::{
    terminal::pane::Pane,
    ui::{
        field::Field,
        theme::{Group, Theme as T},
        widgets::*,
    },
};
use gpui::{Context, Entity, Focusable, Window, div, prelude::*, px};
use keepline_core::{model::*, protocol::*};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub struct Workbench {
    away: Option<u64>,
    search: Search,
    store: Store,
    workspace: Workspace,
    /// A press on the top bar's empty space that has not moved the window yet.
    title_drag: bool,
    error: Option<String>,
    nav: Nav,
    overview: OverviewState,
    tasks: TaskUi,
    layout: LayoutState,
    attention: Attention,
    resuming: Resuming,
}
#[derive(Clone)]
struct PaneMenu {
    terminal: String,
    position: gpui::Point<gpui::Pixels>,
    page: MenuPage,
}
/// What a close removes: a whole tab, or one pane by its terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Closing {
    Tab(String),
    Pane(String),
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
    space: String,
}
#[derive(Clone, Copy)]
enum FormKind {
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
fn home_relative(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && path.starts_with(&home) => {
            format!("~{}", &path[home.len()..])
        }
        _ => path.to_owned(),
    }
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

mod cleanup;
mod filters;
mod forms;
mod layout;
use layout::LayoutState;
mod nav;
use nav::Nav;
mod overview;
use overview::OverviewState;
mod palette;
use palette::Search;
mod render;
mod sessions;
use sessions::{Attention, Resuming};
mod sidebar;
mod tabs;
mod tasks;
use tasks::TaskUi;
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
            .find(|t| t.space == space)
            .map(|t| t.id.clone());
        let (status_tx, mut status_rx) = futures::channel::mpsc::unbounded();
        let store = Store::new(socket, status_tx);
        cx.spawn(async move |this, cx| {
            use futures::StreamExt;
            while let Some(status) = status_rx.next().await {
                if this
                    .update(cx, |this, cx| {
                        let revision = status.workspace_revision;
                        this.store.set_recognition(status);
                        if revision > this.workspace.revision {
                            this.sync(cx);
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
        let search = Search::new(window, cx);
        cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() {
                if let Some(left) = this.away.take() {
                    if show_on_return(left, now(), this.workspace.settings.away_minutes) {
                        this.overview.since = left;
                        this.nav.overview = true;
                        this.sync_panes(cx);
                    }
                    cx.notify();
                }
            } else if this.away.is_none() {
                let at = now();
                this.away = Some(at);
                let socket = this.store.socket().to_owned();
                cx.background_spawn(async move {
                    record_left(&socket, at);
                })
                .detach();
            }
        })
        .detach();
        cx.on_release(|this, _| record_left(this.store.socket(), this.away.unwrap_or_else(now)))
            .detach();
        let mut this = Self {
            away: None,
            search,
            store,
            workspace,
            title_drag: false,
            error: None,
            nav: Nav {
                space,
                tab,
                overview,
                task_detail: None,
                focused: None,
                pending_focus: true,
                zoomed: None,
            },
            overview: OverviewState {
                filter_space: None,
                changed_only: false,
                since,
                show_stale: false,
                show_closed: false,
                show_running: false,
                show_hidden: false,
                card_menu: None,
                here: None,
                here_more: None,
                cleanup: vec![],
                cleanup_index: 0,
            },
            tasks: TaskUi {
                form: None,
                editing_task: None,
                start_after_save: None,
                quick_task: None,
                todos_open: false,
                picking: None,
                last_agent: None,
                chosen_agent: None,
            },
            layout: LayoutState {
                panes: HashMap::new(),
                drag: None,
                split_bounds: HashMap::new(),
                split_target: None,
                menu: None,
                tab_menu: None,
                confirm_close: None,
                renaming_tab: None,
            },
            attention: Attention {
                phases: HashMap::new(),
                unseen: HashSet::new(),
            },
            resuming: Resuming {
                resume: None,
                resumed: HashMap::new(),
            },
        };
        this.sync_panes(cx);
        if this.workspace.terminals.is_empty() && !this.nav.overview {
            this.launch(false, None, cx);
        }
        this
    }
    pub(super) fn sync_panes(&mut self, cx: &mut Context<Self>) {
        let ids = if self.nav.overview {
            Vec::new()
        } else {
            self.workspace
                .tabs
                .iter()
                .find(|t| Some(&t.id) == self.nav.tab.as_ref())
                .map(|t| self.workspace.tab_terminals(&t.id))
                .unwrap_or_default()
        };
        if self.nav.zoomed.as_ref().is_some_and(|id| !ids.contains(id)) {
            self.nav.zoomed = None;
        }
        let ids = if let Some(id) = &self.nav.zoomed {
            vec![id.clone()]
        } else {
            ids
        };
        let ids = attachable_ids(&self.workspace, ids);
        if self.nav.focused.as_ref().is_none_or(|id| !ids.contains(id)) {
            self.nav.focused = ids.first().cloned();
            self.nav.pending_focus = true;
        }
        self.layout.panes.retain(|id, _| ids.contains(id));
        for id in ids {
            if !self.layout.panes.contains_key(&id) {
                let pane = cx.new(|cx| Pane::new(self.store.socket(), &id, false, cx));
                cx.observe(&pane, |_, _, cx| cx.notify()).detach();
                self.layout.panes.insert(id, pane);
            }
        }
        let live: Vec<_> = self
            .workspace
            .agent_sessions
            .iter()
            .filter_map(|s| {
                let shown = s
                    .terminal
                    .as_ref()
                    .is_some_and(|t| self.layout.panes.contains_key(t));
                live_phase(&self.workspace, s).map(|p| (s.id.clone(), p, shown))
            })
            .collect();
        track_unseen(&mut self.attention.phases, &mut self.attention.unseen, live);
    }
    /// Marks a user action in flight; the label appears only if it outlasts 300ms.
    fn begin_busy(&mut self, cx: &mut Context<Self>) {
        let seq = self.store.begin_busy();
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.store.mark_slow(seq) {
                    cx.notify();
                }
            });
        })
        .detach();
    }
    fn end_busy(&mut self) {
        self.store.end_busy();
    }
    /// Takes a snapshot unless a newer one already arrived by another reply.
    fn apply_workspace(&mut self, w: Workspace, cx: &mut Context<Self>) {
        if w.revision < self.workspace.revision {
            return;
        }
        if let Some(id) = self.overview.cleanup.get(self.overview.cleanup_index) {
            let old = self.workspace.agent_sessions.iter().find(|s| &s.id == id);
            let new = w.agent_sessions.iter().find(|s| &s.id == id);
            if old
                .zip(new)
                .is_some_and(|(a, b)| a.outcome != b.outcome && b.outcome != Outcome::Open)
            {
                self.overview.cleanup_index += 1;
            }
        }
        self.workspace = w;
        self.sync_panes(cx);
    }
    /// Background refresh after the host publishes a newer revision. Runs beside user
    /// actions and coalesces bursts into one follow-up fetch.
    pub(super) fn sync(&mut self, cx: &mut Context<Self>) {
        if !self.store.begin_sync() {
            return;
        }
        let task = cx.background_spawn(self.store.send(Request::Workspace));
        cx.spawn(async move |this, cx| {
            let reply = task.await;
            let _ = this.update(cx, |this, cx| {
                let again = this.store.end_sync();
                match reply {
                    Ok(Reply::Workspace(w)) => this.apply_workspace(w, cx),
                    Ok(_) => (),
                    Err(e) => this.error = Some(e.to_string()),
                }
                if again || this.store.behind(this.workspace.revision) {
                    this.sync(cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn request(&mut self, request: Request, cx: &mut Context<Self>) {
        if self.store.busy() {
            self.store.hold(request);
            return;
        }
        self.begin_busy(cx);
        let task = cx.background_spawn(self.store.send(request));
        cx.spawn(async move |this, cx| {
            let reply = task.await;
            let _ = this.update(cx, |this, cx| {
                this.end_busy();
                let received_workspace = matches!(&reply, Ok(Reply::Workspace(_)));
                match reply {
                    Ok(Reply::Workspace(w)) => {
                        this.apply_workspace(w, cx);
                        if let Some(task) = this.tasks.start_after_save.take() {
                            this.launch(false, Some(task), cx);
                        }
                    }
                    Ok(_) => (),
                    Err(e) => {
                        this.tasks.start_after_save = None;
                        this.error = Some(e.to_string());
                    }
                }
                if received_workspace && this.store.behind(this.workspace.revision) {
                    this.sync(cx);
                }
                if let Some(next) = this.store.next_queued() {
                    let error = this.error.take();
                    this.request(next, cx);
                    this.error = this.error.take().or(error);
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
            && self.resuming.resume.is_none()
            && let Some(task) = task_id.as_deref()
            && let Some(terminal) = self.workspace.task_terminal(task)
        {
            let id = terminal.id.clone();
            if let Some(tab) = self.workspace.terminal_tab(&id) {
                self.nav.space = tab.space.clone();
                self.nav.tab = Some(tab.id.clone());
                self.nav.focused = Some(id);
                self.nav.pending_focus = true;
                self.nav.overview = false;
                self.nav.task_detail = None;
                self.sync_panes(cx);
                cx.notify();
                return;
            }
        }
        if let Some(task) = task_id
            .as_ref()
            .and_then(|id| self.workspace.tasks.iter().find(|t| &t.id == id))
        {
            self.nav.space = task.space.clone();
        }
        self.nav.task_detail = None;
        self.launch_in_slot(split, task_id, None, cx);
    }
    /// Closing a pane stops its terminal; its tab closes with the last pane.
    pub(super) fn close_pane(&mut self, id: &str, cx: &mut Context<Self>) {
        self.request_close(Closing::Pane(id.to_owned()), cx);
    }
    /// Closing a tab removes it and stops every terminal in it.
    pub(super) fn close_tab(&mut self, id: &str, cx: &mut Context<Self>) {
        self.request_close(Closing::Tab(id.to_owned()), cx);
    }
    /// Cmd+W: the focused pane of the current tab, else the current tab.
    pub(super) fn close_focused(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = self.nav.tab.clone() else {
            return;
        };
        match self
            .nav
            .focused
            .clone()
            .filter(|id| self.workspace.tab_terminals(&tab).contains(id))
        {
            Some(pane) => self.close_pane(&pane, cx),
            None => self.close_tab(&tab, cx),
        }
    }
    /// Terminals of `closing` with an Agent still running in them.
    pub(super) fn running_agents(&self, closing: &Closing) -> Vec<String> {
        let ids = match closing {
            Closing::Tab(id) => self.workspace.tab_terminals(id),
            Closing::Pane(id) => vec![id.clone()],
        };
        ids.into_iter()
            .filter(|id| {
                self.workspace
                    .terminals
                    .iter()
                    .any(|t| &t.id == id && t.state == ProcessState::Running)
            })
            .filter_map(|id| self.workspace.current_agent(&id))
            .map(|s| {
                self.workspace
                    .content
                    .get(&s.id)
                    .map(|c| c.title.clone())
                    .filter(|t| !t.trim().is_empty())
                    .unwrap_or_else(|| s.runtime.executable().to_owned())
            })
            .collect()
    }
    fn request_close(&mut self, closing: Closing, cx: &mut Context<Self>) {
        self.layout.menu = None;
        self.layout.tab_menu = None;
        if self.running_agents(&closing).is_empty() {
            self.close_now(closing, cx);
        } else {
            self.layout.confirm_close = Some(closing);
            cx.notify();
        }
    }
    pub(super) fn close_now(&mut self, closing: Closing, cx: &mut Context<Self>) {
        self.layout.confirm_close = None;
        self.nav.zoomed = None;
        let (tab, edit) = match closing {
            Closing::Tab(id) => (Some(id.clone()), Edit::CloseTab { id }),
            Closing::Pane(terminal) => (
                self.workspace
                    .terminal_tab(&terminal)
                    .filter(|t| t.layout.as_ref().is_some_and(|l| l.ids().len() == 1))
                    .map(|t| t.id.clone()),
                Edit::ClosePane { terminal },
            ),
        };
        // The neighbour to the right takes over, like a browser; else the one to the left.
        if let Some(tab) = tab.filter(|t| self.nav.tab.as_ref() == Some(t)) {
            let row: Vec<_> = self
                .workspace
                .tabs
                .iter()
                .filter(|t| t.space == self.nav.space)
                .map(|t| t.id.clone())
                .collect();
            let at = row.iter().position(|t| *t == tab).unwrap_or(0);
            self.nav.tab = row
                .get(at + 1)
                .or_else(|| at.checked_sub(1).and_then(|i| row.get(i)))
                .cloned();
            self.nav.focused = None;
            self.nav.pending_focus = true;
        }
        self.edit(edit, cx);
    }
    pub(super) fn launch_in_slot(
        &mut self,
        mut split: bool,
        task_id: Option<String>,
        replace: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if self.store.busy() {
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
            self.nav.tab = Some(tab.id.clone());
            split = true;
        }
        let source = self.layout.split_target.take().or_else(|| {
            if !split {
                return None;
            }
            self.nav
                .focused
                .clone()
                .filter(|id| {
                    self.workspace
                        .terminal_tab(id)
                        .is_some_and(|t| Some(&t.id) == self.nav.tab.as_ref())
                })
                .or_else(|| {
                    self.nav
                        .tab
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
                    .unwrap_or(&self.nav.space)
                    .as_str()
            })
            .cloned()
        else {
            return;
        };
        let agent = self.tasks.chosen_agent.take();
        let resume = self.resuming.resume.take();
        let resuming = resume.as_ref().map(|r| r.2.clone());
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
                .find(|t| Some(&t.id) == self.nav.tab.as_ref())
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
                layout: None,
            }
        };
        let terminal = id();
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
            .filter(|_| task_id.is_none() && resuming.is_none())
            .map(|t| t.id.clone());
        let request = Request::Launch {
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
        let tab_id = tab.id.clone();
        self.begin_busy(cx);
        let job = cx.background_spawn(self.store.launch(cwd_source, request, tab));
        cx.spawn(async move |this, cx| {
            let reply = job.await;
            let _ = this.update(cx, |this, cx| {
                this.end_busy();
                match reply {
                    Ok(Reply::Workspace(w)) => {
                        this.apply_workspace(w, cx);
                        if let Some(session) = resuming {
                            this.resuming.resumed.insert(session, terminal.clone());
                        }
                        this.nav.tab = Some(tab_id);
                        this.nav.zoomed = None;
                        this.layout.menu = None;
                        this.nav.focused = Some(terminal.clone());
                        this.nav.pending_focus = true;
                        this.nav.overview = false;
                        this.sync_panes(cx);
                        if let (Some((command, text)), Some(pane)) =
                            (&start, this.layout.panes.get(&terminal))
                        {
                            pane.update(cx, |pane, cx| pane.start_agent(command, text.clone(), cx));
                        }
                        if this.store.behind(this.workspace.revision) {
                            this.sync(cx);
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
}
