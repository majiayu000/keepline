use crate::host::store::{Store, record_left};
use crate::model::{search::*, session_state::*, time::*};
use crate::ui::app_icon::{AppIcon, Chosen};
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
    path::{Path, PathBuf},
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
    /// The older host is being stopped; the app quits and reopens once it is gone.
    restarting: bool,
    nav: Nav,
    overview: OverviewState,
    tasks: TaskUi,
    layout: LayoutState,
    attention: Attention,
    resuming: Resuming,
    diagnostics: Option<Entity<diagnostics::DiagnosticView>>,
    settings: Option<Entity<settings::SettingsView>>,
    sidebar: sidebar_list::SidebarPrefs,
}
#[derive(Clone)]
struct PaneMenu {
    terminal: String,
    position: gpui::Point<gpui::Pixels>,
    page: MenuPage,
}
/// The first of `candidates` that still exists, else the home directory: a directory
/// removed since (a deleted worktree) must not stop a new terminal.
pub(super) fn usable_cwd(candidates: impl IntoIterator<Item = String>) -> String {
    candidates
        .into_iter()
        .find(|d| Path::new(d).is_dir())
        .or_else(|| std::env::var("HOME").ok())
        .unwrap_or_else(|| "/".into())
}
/// The new terminal's command line. A resume is the shell's first command, so it cannot be
/// lost when its pane is not on screen yet to type it; `-i` keeps the user's aliases, as
/// typing it would, and the shell stays afterwards.
pub(super) fn launch_argv(shell: String, resume: Option<&str>) -> Vec<String> {
    match resume {
        Some(command) => {
            let then = format!("{command}; exec {} -l", shell_quote(&shell));
            vec![shell, "-l".into(), "-i".into(), "-c".into(), then]
        }
        None => vec![shell, "-l".into()],
    }
}
/// What a close removes: a whole tab, one pane by its terminal, or the whole host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Closing {
    Tab(String),
    Pane(String),
    /// Restarting an older host: every terminal ends with it.
    Host,
}
#[derive(Clone, Copy)]
enum MenuPage {
    Main,
    Sessions,
    Assign,
    Move,
}
struct Form {
    kind: FormKind,
    fields: Vec<Entity<Field>>,
    agent: Agent,
    space: String,
}
#[derive(Clone, Copy)]
enum FormKind {
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
/// The first shown pane whose terminal the host lost and that has not been tried yet.
pub(super) fn reopen_candidate(
    w: &Workspace,
    shown: &[String],
    tried: &HashSet<String>,
) -> Option<String> {
    shown
        .iter()
        .find(|id| interrupted(w, id) && !tried.contains(*id))
        .cloned()
}
/// Whether closing these Agents would break into a turn or leave a question unanswered.
pub(super) fn interrupts(agents: &[(String, Option<Phase>)]) -> bool {
    agents
        .iter()
        .any(|(_, p)| matches!(p, Some(Phase::Working | Phase::Waiting)))
}
/// How many split panes closing a tab takes down with it; one pane, or a pane or host
/// close, counts none.
pub(super) fn splits_closed(workspace: &Workspace, closing: &Closing) -> usize {
    match closing {
        Closing::Tab(id) => Some(workspace.tab_terminals(id).len())
            .filter(|n| *n > 1)
            .unwrap_or(0),
        _ => 0,
    }
}
fn outcome_label(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Open => "未结束".into(),
        Outcome::Completed => "已完成".into(),
        Outcome::Abandoned => "已放弃".into(),
        Outcome::Deferred { reason, until } => format!(
            "稍后再看：{} · {} 小时后",
            reason,
            until.saturating_sub(now()).div_ceil(3600)
        ),
    }
}

mod actions;
mod cleanup;
mod diagnostics;
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
mod settings;
use sessions::{Attention, Resuming};
mod sidebar;
use sidebar::closed_recently;
mod sidebar_list;
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
                        this.mark_pane_agents(cx);
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
        // 跟随系统 switches with macOS; a fixed choice ignores it.
        cx.observe_window_appearance(window, |_, _, cx| crate::ui::appearance::apply(cx))
            .detach();
        cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() {
                if let Some(left) = this.away.take() {
                    if show_on_return(left, now(), this.workspace.settings.away_minutes) {
                        this.overview.since = left;
                        this.overview.cleanup.clear();
                        this.overview.welcome = true;
                        this.nav.overview = true;
                        this.nav.pending_focus = true;
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
            diagnostics: None,
            settings: None,
            sidebar: sidebar_list::SidebarPrefs::load(),
            search,
            store,
            workspace,
            title_drag: false,
            error: None,
            restarting: false,
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
                closed_only: false,
                since,
                show_stale: false,
                show_closed: false,
                show_running: false,
                show_review: false,
                show_hidden: false,
                card_menu: None,
                here: None,
                here_more: None,
                cleanup: vec![],
                cleanup_index: 0,
                cleanup_results: HashMap::new(),
                scroll: gpui::ScrollHandle::new(),
                welcome: overview,
                focus: cx.focus_handle(),
                #[cfg(feature = "visual-test")]
                toggle_at: None,
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
                pending_command: None,
                menu: None,
                tab_menu: None,
                space_menu: None,
                space_removing: false,
                renaming_space: None,
                icon_menu: None,
                confirm_close: None,
                renaming_tab: None,
                renaming_session: None,
                reopened: HashSet::new(),
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
        // A shell the host lost on restart comes back in place as soon as its tab shows.
        if !self.store.busy()
            && self.resuming.resume.is_none()
            && self.tasks.chosen_agent.is_none()
            && let Some(id) = reopen_candidate(&self.workspace, &ids, &self.layout.reopened)
        {
            self.layout.reopened.insert(id.clone());
            self.reopen(id, cx);
        }
        let ids = attachable_ids(&self.workspace, ids);
        if self.nav.focused.as_ref().is_none_or(|id| !ids.contains(id)) {
            self.nav.focused = ids.first().cloned();
            self.nav.pending_focus = true;
        }
        self.layout.panes.retain(|id, _| ids.contains(id));
        let split = ids.len() > 1;
        for id in ids {
            if !self.layout.panes.contains_key(&id) {
                let pane = cx.new(|cx| Pane::new(self.store.socket(), &id, false, cx));
                cx.observe(&pane, |_, _, cx| cx.notify()).detach();
                self.layout.panes.insert(id, pane);
            }
        }
        for pane in self.layout.panes.values() {
            pane.update(cx, |p, cx| {
                if p.split != split {
                    p.split = split;
                    cx.notify();
                }
            });
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
        self.mark_pane_agents(cx);
    }
    /// Which Agent each pane holds, for its tint. One the host sees but cannot read (a
    /// shared-daemon Codex, a Claude that saves no transcript) still counts: the host says
    /// which Agent is behind each terminal note.
    fn mark_pane_agents(&mut self, cx: &mut Context<Self>) {
        for (id, pane) in &self.layout.panes {
            let agent = self
                .workspace
                .current_agent(id)
                .map(|s| s.runtime)
                .or_else(|| self.store.recognition().terminal_agents.get(id).copied());
            pane.update(cx, |p, _| p.agent = agent);
        }
    }
    /// Marks a user action in flight; the label appears only if it outlasts 300ms.
    fn begin_busy(&mut self, cx: &mut Context<Self>) {
        let seq = self.store.begin_busy();
        if let Some(view) = &self.settings {
            view.update(cx, |v, cx| {
                v.busy = true;
                v.save_error = None;
                cx.notify();
            });
        }
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
                // Remembered per queued session so the steps show what each one became.
                if let Some(b) = new {
                    self.overview
                        .cleanup_results
                        .insert(id.clone(), b.outcome.clone());
                }
                self.overview.cleanup_index += 1;
            }
        }
        self.workspace = w;
        if let Some(view) = &self.settings {
            view.update(cx, |v, cx| {
                v.values = self.workspace.settings.clone();
                cx.notify();
            });
        }
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
                if let Some(view) = &this.settings {
                    view.update(cx, |v, cx| {
                        v.values = this.workspace.settings.clone();
                        v.busy = this.store.busy();
                        v.save_error = this.error.clone();
                        cx.notify();
                    });
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
    /// Each live Agent the close would stop: its title and what it is doing now.
    pub(super) fn running_agents(&self, closing: &Closing) -> Vec<(String, Option<Phase>)> {
        let ids = match closing {
            Closing::Tab(id) => self.workspace.tab_terminals(id),
            Closing::Pane(id) => vec![id.clone()],
            Closing::Host => self.running_terminals(),
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
                let title = self
                    .workspace
                    .content
                    .get(&s.id)
                    .map(|c| c.title.clone())
                    .filter(|t| !t.trim().is_empty())
                    .unwrap_or_else(|| s.runtime.executable().to_owned());
                (title, live_phase(&self.workspace, s))
            })
            .collect()
    }
    /// Terminals whose program is still running, Agent or not.
    pub(super) fn running_terminals(&self) -> Vec<String> {
        self.workspace
            .terminals
            .iter()
            .filter(|t| t.state == ProcessState::Running)
            .map(|t| t.id.clone())
            .collect()
    }
    pub(super) fn request_close(&mut self, closing: Closing, cx: &mut Context<Self>) {
        self.layout.menu = None;
        self.layout.tab_menu = None;
        self.layout.space_menu = None;
        let quiet = match closing {
            // A host that hands its terminals over ends none of them: nothing to confirm.
            Closing::Host => {
                self.running_terminals().is_empty()
                    || crate::lifecycle::stale_build().is_some_and(|b| b.handoff)
            }
            // An Agent whose turn has ended loses nothing it can't resume: only work in
            // progress or a question waiting for an answer is worth a confirmation, and
            // so is a tab of several splits, which a stray click would undo at once.
            _ => {
                splits_closed(&self.workspace, &closing) == 0
                    && !interrupts(&self.running_agents(&closing))
            }
        };
        if quiet {
            self.close_now(closing, cx);
        } else {
            self.layout.confirm_close = Some(closing);
            cx.notify();
        }
    }
    /// Stops the older host off the UI thread, then quits; this app reopens itself and
    /// starts its own host.
    fn restart_host(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.store.socket().parent().map(Path::to_path_buf) else {
            return;
        };
        // Recorded while the old host still answers; the release hook runs after it is gone.
        record_left(self.store.socket(), self.away.unwrap_or_else(now));
        self.restarting = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { crate::lifecycle::restart_host(&root) })
                .await;
            match result {
                Ok(()) => {
                    cx.update(|cx| cx.quit());
                }
                Err(e) => {
                    let _ = this.update(cx, |this, cx| {
                        this.restarting = false;
                        this.error = Some(format!("重启后台失败：{e}"));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }
    pub(super) fn close_now(&mut self, closing: Closing, cx: &mut Context<Self>) {
        self.layout.confirm_close = None;
        self.nav.zoomed = None;
        let (tab, edit) = match closing {
            Closing::Host => return self.restart_host(cx),
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
            self.sync_panes(cx);
        }
        self.edit(edit, cx);
    }
    /// Reopens a lost terminal in place, resuming the conversation it was running.
    pub(super) fn reopen(&mut self, id: String, cx: &mut Context<Self>) {
        if let Some(s) = interrupted_session(&self.workspace, &id) {
            self.resuming.resume = Some((
                s.runtime.resume_command(&s.cli_session_id),
                s.cwd.clone(),
                s.id.clone(),
            ));
        }
        self.launch_in_slot(true, None, Some(id), cx);
        // Taken unless the launch bailed early; a leftover would hijack the next launch.
        self.resuming.resume = None;
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
            self.sync_panes(cx);
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
        // A plugin action's command and the pane it ran for; it runs like a resume does.
        let pending = self.layout.pending_command.take();
        let resuming = resume.as_ref().map(|r| r.2.clone());
        // A resume is in the launch itself; other commands are typed into the new shell, and
        // task text only ever reaches the Agent's input.
        let start: Option<(String, Option<String>)> = if resume.is_some() {
            None
        } else {
            task_id
                .as_ref()
                .and_then(|id| self.workspace.tasks.iter().find(|t| &t.id == id))
                .map(|task| {
                    let agent = agent.unwrap_or(task.agent);
                    (agent.command(), Some(task.prompt()))
                })
                .or_else(|| pending.as_ref().map(|(c, _)| (c.clone(), None)))
        };
        let argv = launch_argv(
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into()),
            resume.as_ref().map(|r| r.0.as_str()),
        );
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
        // A plain new tab follows the focused pane of the current tab, like a split does.
        let inherit = source.as_ref().map(|(id, _)| id.clone()).or_else(|| {
            let tab = self.nav.tab.as_ref().filter(|_| previous.is_none())?;
            let panes = self.workspace.tab_terminals(tab);
            self.nav
                .focused
                .clone()
                .filter(|id| panes.contains(id))
                .or_else(|| panes.first().cloned())
        });
        let inherited = inherit
            .as_ref()
            .and_then(|id| self.workspace.terminals.iter().find(|t| &t.id == id))
            .filter(|t| t.space == space.id);
        let cwd = resume
            .map(|r| r.1)
            .or(task_cwd)
            .or(pending.map(|(_, cwd)| cwd))
            .unwrap_or_else(|| {
                previous
                    .map(|t| t.cwd)
                    .or_else(|| inherited.map(|t| t.cwd.clone()))
                    .unwrap_or_else(|| space.cwd.clone())
            });
        let cwd = usable_cwd([cwd, space.cwd.clone()]);
        let cwd_source = inherited
            .filter(|t| t.state == ProcessState::Running)
            .filter(|_| task_id.is_none() && resuming.is_none())
            .map(|t| t.id.clone());
        let request = Request::Launch {
            id: terminal.clone(),
            space: space.id,
            argv,
            cwd,
            cols: 100,
            rows: 30,
            scheme: crate::ui::theme::Theme::scheme(),
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
