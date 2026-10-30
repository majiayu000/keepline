pub use crate::agent::Agent;
pub use crate::layout::{Axis, Layout};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Workspace {
    pub revision: u64,
    #[serde(default)]
    pub last_left: u64,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub outcome_history: Vec<OutcomeChange>,
    #[serde(default)]
    pub task_outcomes: std::collections::HashMap<String, Outcome>,
    /// One proven binding consumes this override; None cancels the tab default.
    #[serde(default)]
    pub pending_tasks: std::collections::HashMap<String, Option<String>>,
    /// Terminals a restarted host lost, with the conversation each was running then
    /// (None: only its shell). Terminals lost before this was kept have no entry.
    #[serde(default)]
    pub lost_agents: std::collections::HashMap<String, Option<String>>,
    pub spaces: Vec<Space>,
    pub tasks: Vec<Task>,
    pub tabs: Vec<Tab>,
    pub terminals: Vec<Terminal>,
    pub agent_sessions: Vec<AgentSession>,
    pub content: std::collections::HashMap<String, crate::content::Content>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub notifications: bool,
    pub stale_days: u32,
    pub away_minutes: u32,
    pub default_agent: Agent,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            notifications: false,
            stale_days: 3,
            away_minutes: 30,
            default_agent: Agent::default(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Space {
    pub id: String,
    pub name: String,
    pub cwd: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub space: String,
    pub title: String,
    pub description: String,
    pub acceptance: Vec<AcceptanceCriterion>,
    pub agent: Agent,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceCriterion {
    pub id: String,
    pub description: String,
    /// A source reference, not an agent's claim or an automatic acceptance flag.
    pub evidence: Option<String>,
}

impl Task {
    /// What goes into the Agent's own input box. Never a shell argument; blank parts
    /// leave no empty headings behind.
    pub fn prompt(&self) -> String {
        let mut parts = vec![self.title.trim().to_owned()];
        if !self.description.trim().is_empty() {
            parts.push(self.description.trim().to_owned());
        }
        let criteria: Vec<_> = self
            .acceptance
            .iter()
            .map(|c| c.description.trim())
            .filter(|c| !c.is_empty())
            .map(|c| format!("- {c}"))
            .collect();
        if !criteria.is_empty() {
            parts.push(format!("验收标准：\n{}", criteria.join("\n")));
        }
        parts.join("\n\n")
    }
}
pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tab {
    pub id: String,
    pub space: String,
    /// A creation default, never the ownership of existing agent sessions.
    pub default_task: Option<String>,
    pub title: String,
    pub layout: Option<Layout>,
}

/// A managed PTY. Its shell may run many distinct agent conversations over time.
/// Placement belongs exclusively to Tab.layout; PTY identity survives closing its view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Terminal {
    pub id: String,
    pub space: String,
    pub argv: Vec<String>,
    pub cwd: String,
    pub pid: Option<u32>,
    pub state: ProcessState,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
}

/// CLI identity is (runtime, cli_session_id), independent of PTY and placement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: String,
    pub runtime: Agent,
    pub cli_session_id: String,
    /// Last associated terminal; retained for historical conversations.
    pub terminal: Option<String>,
    /// At most one current conversation per terminal. This is not its outcome.
    pub current: bool,
    pub task: Option<String>,
    pub outcome: Outcome,
    pub updated: u64,
    pub cwd: String,
    pub transcript: String,
    /// What the user called it; empty leaves the name to the CLI and the first ask.
    #[serde(default)]
    pub name: String,
    /// The evidence that tied it to its terminal; `None` while it has never been bound.
    #[serde(default)]
    pub binding: Option<Binding>,
}
/// How a conversation was tied to the terminal running it, strongest first. Each CLI
/// offers some of these; the host tries every one it offers and refuses ambiguity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Binding {
    /// The process names its conversation: on its command line (`--session-id`, `resume`)
    /// or in its CLI's own registry of live sessions.
    Process,
    /// The CLI's own hook reported its conversation from inside the terminal.
    Hook,
    /// The process holds the transcript open.
    OpenFile,
    /// The newest prompt shows on exactly one screen, or was submitted there.
    Screen,
}
impl Binding {
    pub fn label(self) -> &'static str {
        match self {
            Self::Process => "进程报出的会话 ID",
            Self::Hook => "Agent 的 Hook 上报",
            Self::OpenFile => "进程打开的对话文件",
            Self::Screen => "画面上的提示词",
        }
    }
}
impl AgentSession {
    /// Started in a Keepline terminal. Conversations found only by scanning CLI logs
    /// never ask for attention; they stay visible in the overview.
    pub fn managed(&self) -> bool {
        self.terminal.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessState {
    Starting,
    Running,
    Draining,
    Exited,
    Interrupted,
    Failed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    #[default]
    Open,
    Completed,
    Deferred {
        reason: String,
        until: u64,
    },
    Abandoned,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutcomeChange {
    pub session: String,
    pub before: Outcome,
    pub after: Outcome,
    pub at: u64,
    pub reason: String,
}
impl Outcome {
    pub fn needs_review(&self, now: u64) -> bool {
        matches!(self, Self::Open) || matches!(self, Self::Deferred { until, .. } if *until <= now)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Edit {
    Settings(Settings),
    AddSpace(Space),
    /// Rename a space or point it at another directory.
    UpdateSpace(Space),
    AddTask(Task),
    UpdateTask(Task),
    SetTaskOutcome {
        id: String,
        outcome: Outcome,
    },
    SetPendingTask {
        terminal: String,
        task: Option<String>,
    },
    PutTab {
        tab: Tab,
    },
    /// Removes the tab; the host stops every terminal it held.
    CloseTab {
        id: String,
    },
    /// Takes one terminal out of its tab, closing the tab with its last pane; the host
    /// stops that terminal.
    ClosePane {
        terminal: String,
    },
    SetOutcome {
        id: String,
        outcome: Outcome,
    },
    AssignTask {
        id: String,
        task: Option<String>,
    },
    SaveSessionTask {
        session: String,
        task: Task,
    },
    /// Names a conversation; an empty name hands it back to the CLI and the first ask.
    RenameSession {
        id: String,
        name: String,
    },
}

impl Edit {
    /// Whether the edit must be based on the host's current revision. Tabs and layouts
    /// are addressed by id and only ever changed by the user, so like herdr's tab
    /// commands they apply to whatever is there now and fail only when a referenced
    /// object is gone. Session output bumps the revision every second; requiring it
    /// here made 「收起标签」 fail almost every time.
    pub fn checks_revision(&self) -> bool {
        !matches!(
            self,
            Self::PutTab { .. }
                | Self::CloseTab { .. }
                | Self::ClosePane { .. }
                | Self::RenameSession { .. }
        )
    }
}

fn validate_outcome(outcome: &Outcome) -> Result<(), String> {
    if let Outcome::Deferred { reason, until } = outcome
        && (reason.trim().is_empty() || *until == 0)
    {
        return Err("稍后再看需要原因和时间".into());
    }
    Ok(())
}
impl Workspace {
    pub fn change_outcome(
        &mut self,
        id: &str,
        outcome: Outcome,
        at: u64,
        reason: &str,
    ) -> Result<(), String> {
        validate_outcome(&outcome)?;
        let session = self
            .agent_sessions
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or("Agent 会话不存在")?;
        if session.outcome != outcome {
            self.outcome_history.push(OutcomeChange {
                session: id.into(),
                before: session.outcome.clone(),
                after: outcome.clone(),
                at,
                reason: reason.into(),
            });
            session.outcome = outcome;
        }
        Ok(())
    }
    pub fn review_since(&self, id: &str) -> u64 {
        self.outcome_history
            .iter()
            .rev()
            .find(|h| h.session == id && h.reason == "暂存到期")
            .map(|h| h.at)
            .unwrap_or(0)
    }
    pub fn expire_outcomes(&mut self, now: u64) -> bool {
        let due: Vec<_> = self
            .agent_sessions
            .iter()
            .filter_map(|s| match s.outcome {
                Outcome::Deferred { until, .. } if until <= now => Some((s.id.clone(), until)),
                _ => None,
            })
            .collect();
        let mut changed = !due.is_empty();
        for (id, at) in due {
            self.change_outcome(&id, Outcome::Open, at, "暂存到期")
                .expect("collected session");
        }
        for outcome in self.task_outcomes.values_mut() {
            if matches!(outcome,Outcome::Deferred{until,..} if *until <= now) {
                *outcome = Outcome::Open;
                changed = true;
            }
        }
        changed
    }
    pub fn next_deadline(&self, now: u64) -> Option<u64> {
        let mut times = Vec::new();
        for session in &self.agent_sessions {
            if let Outcome::Deferred { until, .. } = session.outcome {
                times.push(until);
            }
            if let Some(c) = self.content.get(&session.id) {
                if c.hidden_for(session).is_some() || !session.outcome.needs_review(now) {
                    continue;
                }
                if c.turn == crate::content::Turn::Running && !session.current {
                    times.push(c.updated.saturating_add(901));
                }
                if c.turn != crate::content::Turn::Running || !session.current {
                    times.push(
                        (if c.turn_started > 0 {
                            c.turn_started
                        } else {
                            c.updated
                        })
                        .max(self.review_since(&session.id))
                        .saturating_add(u64::from(self.settings.stale_days) * 86400 + 1),
                    );
                }
            }
        }
        for outcome in self.task_outcomes.values() {
            if let Outcome::Deferred { until, .. } = outcome {
                times.push(*until);
            }
        }
        times.into_iter().filter(|t| *t > now).min()
    }
    pub fn task_terminal(&self, task: &str) -> Option<&Terminal> {
        let running = |t: &&Terminal| t.state == ProcessState::Running;
        self.terminals
            .iter()
            .filter(running)
            .find(|t| {
                self.current_agent(&t.id)
                    .is_some_and(|s| s.task.as_deref() == Some(task))
            })
            .or_else(|| {
                self.terminals.iter().filter(running).find(|t| {
                    self.current_agent(&t.id).is_none()
                        && self.pending_tasks.get(&t.id) != Some(&None)
                        && self
                            .terminal_tab(&t.id)
                            .is_some_and(|tab| tab.default_task.as_deref() == Some(task))
                })
            })
    }

    /// Like Warp: the name you gave it, then what runs in it (todo, conversation,
    /// the title the program set), then the program itself, numbered among same-named
    /// tabs. The space's directory is not a tab name; every tab in one space shares it.
    /// `titles` holds every terminal's program title, so a tab keeps its name while
    /// another tab is in front; `focused` breaks ties between a tab's own panes.
    pub fn tab_title(
        &self,
        tab: &Tab,
        titles: &std::collections::HashMap<String, String>,
        focused: Option<&str>,
    ) -> String {
        if let Some(title) = self.tab_given_title(tab, titles, focused) {
            return title;
        }
        let name = self.tab_program(tab);
        let same = self
            .tabs
            .iter()
            .filter(|t| t.space == tab.space)
            .take_while(|t| t.id != tab.id)
            .filter(|t| {
                self.tab_given_title(t, titles, None).is_none() && self.tab_program(t) == name
            })
            .count();
        if same == 0 {
            name
        } else {
            format!("{name} {}", same + 1)
        }
    }
    fn tab_given_title(
        &self,
        tab: &Tab,
        titles: &std::collections::HashMap<String, String>,
        focused: Option<&str>,
    ) -> Option<String> {
        if !tab.title.trim().is_empty() {
            return Some(tab.title.clone());
        }
        let ids = self.tab_terminals(&tab.id);
        let sessions: Vec<_> = self
            .agent_sessions
            .iter()
            .filter(|s| s.current && s.terminal.as_ref().is_some_and(|id| ids.contains(id)))
            .collect();
        if sessions.is_empty()
            && let Some(task) = tab
                .default_task
                .as_ref()
                .and_then(|id| self.tasks.iter().find(|t| &t.id == id))
        {
            return Some(task.title.clone());
        }
        if let Some(first) = sessions.first().and_then(|s| s.task.as_ref())
            && sessions.iter().all(|s| s.task.as_ref() == Some(first))
            && let Some(task) = self.tasks.iter().find(|t| &t.id == first)
        {
            return Some(task.title.clone());
        }
        if let Some(title) = sessions
            .iter()
            .filter_map(|s| Some((s.updated, self.session_name(s)?)))
            .max_by_key(|(updated, _)| *updated)
            .map(|(_, t)| t)
        {
            return Some(title);
        }
        let mut ids = ids;
        if let Some(i) = ids.iter().position(|id| Some(id.as_str()) == focused) {
            ids.swap(0, i);
        }
        ids.iter()
            .filter_map(|id| titles.get(id))
            .find(|t| !t.trim().is_empty())
            .cloned()
    }
    /// What runs in the tab's first pane: its agent, else its program ("zsh").
    fn tab_program(&self, tab: &Tab) -> String {
        let ids = self.tab_terminals(&tab.id);
        let Some(id) = ids.first() else {
            return "终端".into();
        };
        if let Some(session) = self.current_agent(id) {
            return session.runtime.name().into();
        }
        self.terminals
            .iter()
            .find(|t| &t.id == id)
            .and_then(|t| t.argv.first())
            .and_then(|program| program.rsplit('/').next())
            .map(|program| program.trim_start_matches('-'))
            .filter(|program| !program.is_empty())
            .unwrap_or("终端")
            .into()
    }
    pub fn tab_terminals(&self, id: &str) -> Vec<String> {
        self.tabs
            .iter()
            .find(|t| t.id == id)
            .and_then(|t| t.layout.as_ref())
            .map(Layout::ids)
            .unwrap_or_default()
    }
    pub fn terminal_tab(&self, id: &str) -> Option<&Tab> {
        self.tabs
            .iter()
            .find(|t| t.layout.as_ref().is_some_and(|l| l.contains(id)))
    }
    /// What a conversation is called, best first: the name the user gave it, the one its
    /// CLI keeps, its first ask. `None` when none of those is known.
    pub fn session_name(&self, s: &AgentSession) -> Option<String> {
        let named = s.name.trim();
        if !named.is_empty() {
            return Some(named.to_owned());
        }
        let c = self.content.get(&s.id)?;
        let named = crate::content::topic(&c.name);
        if !named.is_empty() {
            return Some(crate::content::short(&named, 70));
        }
        // The first ask keeps its lines, so pasted chrome above the words can go.
        let first = match c.asks.first() {
            Some(q) if c.first_ask_known => q.text.as_str(),
            _ => c.title.as_str(),
        };
        Some(crate::content::short(&crate::content::topic(first), 70)).filter(|t| !t.is_empty())
    }
    pub fn current_agent(&self, terminal: &str) -> Option<&AgentSession> {
        self.agent_sessions
            .iter()
            .find(|s| s.current && s.terminal.as_deref() == Some(terminal))
    }
    /// Called only after the host has proven the CLI identity and terminal link.
    /// Importing a log alone leaves terminal=None and never changes ownership.
    pub fn observe_agent(&mut self, mut observed: AgentSession) -> Result<(), String> {
        let prior = self.agent_sessions.iter().position(|s| {
            s.runtime == observed.runtime && s.cli_session_id == observed.cli_session_id
        });
        if let Some(index) = prior {
            let old = &self.agent_sessions[index];
            if observed.current
                && old.current
                && observed.terminal.is_some()
                && observed.terminal != old.terminal
            {
                return Err("同一 CLI 会话仍关联在另一个终端；先确认原关联结束".into());
            }
            observed.id = old.id.clone();
            observed.task = old.task.clone();
            observed.name = old.name.clone();
            observed.outcome = old.outcome.clone();
            observed.updated = old.updated.max(observed.updated);
            if observed.terminal.is_none() {
                observed.terminal = old.terminal.clone();
                observed.current = old.current;
                observed.binding = old.binding;
            }
        }
        if let Some(terminal_id) = &observed.terminal {
            let terminal = self
                .terminals
                .iter()
                .find(|t| &t.id == terminal_id)
                .ok_or("终端不存在")?;
            // Default only on the first proven association, not on moves/resumes.
            let first_binding = prior.is_none_or(|i| self.agent_sessions[i].terminal.is_none());
            if first_binding && observed.task.is_none() {
                observed.task = self
                    .pending_tasks
                    .get(terminal_id)
                    .cloned()
                    .unwrap_or_else(|| {
                        self.terminal_tab(&terminal.id)
                            .and_then(|t| t.default_task.clone())
                    });
            }
            if observed.current {
                self.pending_tasks.remove(terminal_id);
                for old in &mut self.agent_sessions {
                    if old.terminal.as_ref() == Some(terminal_id) {
                        old.current = false;
                    }
                }
            }
        }
        if let Some(index) = prior {
            self.agent_sessions[index] = observed;
        } else {
            self.agent_sessions.push(observed);
        }
        self.revision += 1;
        Ok(())
    }
    pub fn edit(&mut self, edit: Edit) -> Result<(), String> {
        match edit {
            Edit::Settings(settings) => {
                if !(1..=365).contains(&settings.stale_days)
                    || !(1..=1440).contains(&settings.away_minutes)
                {
                    return Err("天数须为 1–365，离开分钟数须为 1–1440".into());
                }
                self.settings = settings;
            }

            Edit::UpdateTask(task) => {
                let index = self
                    .tasks
                    .iter()
                    .position(|t| t.id == task.id)
                    .ok_or("待办不存在")?;
                // Reuse the creation boundary so validation stays in one place.
                let mut check = self.clone();
                check.tasks.remove(index);
                check.edit(Edit::AddTask(task.clone()))?;
                self.tasks[index] = task;
            }
            Edit::SetTaskOutcome { id, outcome } => {
                if !self.tasks.iter().any(|t| t.id == id) {
                    return Err("待办不存在".into());
                }
                validate_outcome(&outcome)?;
                self.task_outcomes.insert(id, outcome);
            }
            Edit::SetPendingTask { terminal, task } => {
                if !self.terminals.iter().any(|t| t.id == terminal) {
                    return Err("终端不存在".into());
                }
                if task
                    .as_ref()
                    .is_some_and(|id| !self.tasks.iter().any(|t| &t.id == id))
                {
                    return Err("待办不存在".into());
                }
                self.pending_tasks.insert(terminal, task);
            }
            Edit::SaveSessionTask { session, task } => {
                if !self.agent_sessions.iter().any(|s| s.id == session) {
                    return Err("会话不存在".into());
                }
                let task_id = task.id.clone();
                self.edit(Edit::AddTask(task))?;
                self.agent_sessions
                    .iter_mut()
                    .find(|s| s.id == session)
                    .unwrap()
                    .task = Some(task_id);
            }
            Edit::AddSpace(space) => {
                if self.spaces.iter().any(|s| s.id == space.id) {
                    return Err("空间 ID 已存在".into());
                }
                if space.name.trim().is_empty() {
                    return Err("空间名称不能为空".into());
                }
                self.spaces.push(space);
            }
            Edit::UpdateSpace(space) => {
                if space.name.trim().is_empty() {
                    return Err("空间名称不能为空".into());
                }
                let Some(existing) = self.spaces.iter_mut().find(|s| s.id == space.id) else {
                    return Err("空间不存在".into());
                };
                *existing = space;
            }
            Edit::AddTask(task) => {
                if !self.spaces.iter().any(|s| s.id == task.space) {
                    return Err("空间不存在".into());
                }
                if self.tasks.iter().any(|t| t.id == task.id) {
                    return Err("待办 ID 已存在".into());
                }
                if task.title.trim().is_empty() {
                    return Err("待办标题不能为空".into());
                }
                for (i, criterion) in task.acceptance.iter().enumerate() {
                    if criterion.description.trim().is_empty()
                        || criterion.id.is_empty()
                        || task.acceptance[..i].iter().any(|c| c.id == criterion.id)
                    {
                        return Err("验收条目需要独立 ID 和非空说明".into());
                    }
                }
                self.tasks.push(task);
            }
            Edit::PutTab { tab } => {
                let terminals = tab.layout.as_ref().map(Layout::ids).unwrap_or_default();
                if tab.layout.as_ref().is_some_and(|l| !l.valid_ratios()) {
                    return Err("分屏比例必须在 0 与 1 之间".into());
                }
                if !self.spaces.iter().any(|s| s.id == tab.space) {
                    return Err("空间不存在".into());
                }
                if tab
                    .default_task
                    .as_ref()
                    .is_some_and(|id| !self.tasks.iter().any(|t| &t.id == id))
                {
                    return Err("默认待办不存在".into());
                }
                for (i, id) in terminals.iter().enumerate() {
                    if terminals[..i].contains(id) {
                        return Err("终端不能重复摆放".into());
                    }
                    if !self.terminals.iter().any(|t| &t.id == id) {
                        return Err("终端不存在".into());
                    }
                }
                // Moving a leaf collapses its old parent, without changing work ownership.
                for other in &mut self.tabs {
                    if other.id != tab.id {
                        for id in &terminals {
                            other.layout = other.layout.take().and_then(|l| l.without(id));
                        }
                    }
                }
                if let Some(old) = self.tabs.iter_mut().find(|t| t.id == tab.id) {
                    *old = tab;
                } else {
                    self.tabs.push(tab);
                }
                // A tab without panes has nothing to show; it closes with its last pane.
                self.tabs.retain(|t| t.layout.is_some());
            }
            Edit::CloseTab { id } => {
                let before = self.tabs.len();
                self.tabs.retain(|t| t.id != id);
                if self.tabs.len() == before {
                    return Err("标签不存在".into());
                }
            }
            Edit::ClosePane { terminal } => {
                let tab = self
                    .tabs
                    .iter_mut()
                    .find(|t| t.layout.as_ref().is_some_and(|l| l.contains(&terminal)))
                    .ok_or("画面不存在")?;
                tab.layout = tab.layout.take().and_then(|l| l.without(&terminal));
                self.tabs.retain(|t| t.layout.is_some());
            }
            Edit::SetOutcome { id, outcome } => {
                self.change_outcome(&id, outcome, crate::content::now(), "用户操作")?;
            }
            Edit::AssignTask { id, task } => {
                if task
                    .as_ref()
                    .is_some_and(|id| !self.tasks.iter().any(|t| &t.id == id))
                {
                    return Err("待办不存在".into());
                }
                self.agent_sessions
                    .iter_mut()
                    .find(|s| s.id == id)
                    .ok_or("Agent 会话不存在")?
                    .task = task;
            }
            Edit::RenameSession { id, name } => {
                self.agent_sessions
                    .iter_mut()
                    .find(|s| s.id == id)
                    .ok_or("Agent 会话不存在")?
                    .name = name.split_whitespace().collect::<Vec<_>>().join(" ");
            }
        }
        self.revision += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn workspace() -> Workspace {
        let mut w = Workspace::default();
        for id in ["a", "b"] {
            w.spaces.push(Space {
                id: id.into(),
                name: id.into(),
                cwd: format!("/tmp/{id}"),
            });
            w.tasks.push(Task {
                id: id.into(),
                space: id.into(),
                title: id.into(),
                description: String::new(),
                acceptance: vec![],
                agent: Agent::Codex,
            });
        }
        w.terminals.push(Terminal {
            id: "pty".into(),
            space: "a".into(),
            argv: vec!["/bin/sh".into()],
            cwd: "/tmp/a".into(),
            pid: Some(42),
            state: ProcessState::Running,
            exit_code: None,
            error: None,
        });
        w
    }
    fn observation(id: &str) -> AgentSession {
        AgentSession {
            id: id.into(),
            runtime: Agent::Codex,
            cli_session_id: id.into(),
            terminal: Some("pty".into()),
            current: true,
            task: None,
            outcome: Outcome::Open,
            updated: 1,
            cwd: "/tmp/a".into(),
            transcript: format!("/logs/{id}.jsonl"),
            binding: None,
            name: String::new(),
        }
    }
    #[test]
    fn a_temporary_directory_hides_only_sessions_keepline_did_not_start() {
        let mut c = crate::content::Content {
            hidden_reason: crate::content::TEMPORARY_DIRECTORY.into(),
            ..Default::default()
        };
        let mut s = observation("tmp");
        assert_eq!(c.hidden_for(&s), None);
        s.terminal = None;
        assert!(c.hidden_for(&s).is_some());
        // Real automation stays hidden even in a Keepline terminal.
        s.terminal = Some("pty".into());
        c.hidden_reason = "Claude 子代理 / sidechain".into();
        assert!(c.hidden_for(&s).is_some());
    }
    fn tab(w: &mut Workspace, id: &str, space: &str, task: &str) {
        w.edit(Edit::PutTab {
            tab: Tab {
                id: id.into(),
                space: space.into(),
                default_task: Some(task.into()),
                title: String::new(),
                layout: Some(Layout::Leaf("pty".into())),
            },
        })
        .unwrap();
    }
    #[test]
    fn settings_reject_invalid_intervals_and_drive_stale_boundary() {
        let mut w = Workspace::default();
        let settings = Settings {
            stale_days: 0,
            ..Default::default()
        };
        assert!(w.edit(Edit::Settings(settings)).is_err());
        assert_eq!(w.settings, Settings::default());
        let settings = Settings {
            stale_days: 7,
            away_minutes: 45,
            ..Default::default()
        };
        w.edit(Edit::Settings(settings.clone())).unwrap();
        assert_eq!(w.settings, settings);
    }
    #[test]
    fn outcomes_expire_once_preserve_history_and_leave_processes_alone() {
        use crate::content::{Content, Inbox, Turn};
        let mut w = workspace();
        w.observe_agent(observation("one")).unwrap();
        let now = 4 * 86400;
        let c = Content {
            interactive: true,
            updated: 1,
            turn_started: 1,
            turn: Turn::Completed,
            ..Default::default()
        };
        w.content.insert("one".into(), c.clone());
        assert_eq!(c.group(&w.agent_sessions[0], &w, now), Inbox::Stale);
        w.change_outcome(
            "one",
            Outcome::Deferred {
                reason: "check later".into(),
                until: now + 2,
            },
            now,
            "用户操作",
        )
        .unwrap();
        assert_eq!(w.next_deadline(now), Some(now + 2));
        assert!(!w.expire_outcomes(now + 1));
        assert!(w.expire_outcomes(now + 2));
        assert!(!w.expire_outcomes(now + 3));
        assert_eq!(w.agent_sessions[0].outcome, Outcome::Open);
        assert_eq!(c.group(&w.agent_sessions[0], &w, now + 2), Inbox::Paused);
        assert_eq!(w.outcome_history.len(), 2);
        w.change_outcome("one", Outcome::Abandoned, now + 3, "用户操作")
            .unwrap();
        w.change_outcome("one", Outcome::Open, now + 4, "用户操作")
            .unwrap();
        assert_eq!(w.outcome_history.len(), 4);
        assert_eq!(w.terminals[0].state, ProcessState::Running);
        assert_eq!(
            w.agent_sessions[0].updated, 1,
            "outcome dates are not log activity"
        );
        assert_eq!(w.next_deadline(now + 4), Some(now + 2 + 3 * 86400 + 1));
    }
    #[test]
    fn task_start_reuses_running_shell_and_unlink_is_consumed_once() {
        let mut w = workspace();
        tab(&mut w, "tab", "a", "a");
        assert_eq!(w.task_terminal("a").unwrap().id, "pty");
        w.edit(Edit::SetPendingTask {
            terminal: "pty".into(),
            task: None,
        })
        .unwrap();
        w.observe_agent(observation("cancelled")).unwrap();
        assert!(w.agent_sessions[0].task.is_none());
        assert!(!w.pending_tasks.contains_key("pty"));
        w.observe_agent(observation("next")).unwrap();
        assert_eq!(w.agent_sessions[1].task.as_deref(), Some("a"));
        w.edit(Edit::SetTaskOutcome {
            id: "a".into(),
            outcome: Outcome::Completed,
        })
        .unwrap();
        assert_eq!(w.terminals[0].state, ProcessState::Running);
        assert_eq!(w.agent_sessions[1].outcome, Outcome::Open);
        w.terminals[0].state = ProcessState::Exited;
        assert!(w.task_terminal("a").is_none());
    }
    #[test]
    fn task_edit_preserves_individual_criteria_and_tab_title_tracks_ownership() {
        let mut w = workspace();
        tab(&mut w, "tab", "a", "a");
        w.observe_agent(observation("one")).unwrap();
        let mut task = w.tasks[0].clone();
        task.title = "新的待办名".into();
        task.acceptance.push(AcceptanceCriterion {
            id: "criterion".into(),
            description: "退出码为零".into(),
            evidence: Some("/log:42".into()),
        });
        w.edit(Edit::UpdateTask(task.clone())).unwrap();
        assert_eq!(
            w.tab_title(&w.tabs[0], &Default::default(), None),
            "新的待办名"
        );
        assert_eq!(w.tasks[0].acceptance, task.acceptance);
        w.edit(Edit::AssignTask {
            id: "one".into(),
            task: None,
        })
        .unwrap();
        assert_eq!(w.tab_title(&w.tabs[0], &Default::default(), None), "Codex");
        assert!(
            w.edit(Edit::SetTaskOutcome {
                id: "a".into(),
                outcome: Outcome::Deferred {
                    reason: "".into(),
                    until: 1
                }
            })
            .is_err()
        );
    }
    #[test]
    fn update_space_renames_and_moves_only_an_existing_space() {
        let mut w = workspace();
        let id = w.spaces[0].id.clone();
        w.edit(Edit::UpdateSpace(Space {
            id: id.clone(),
            name: "新名字".into(),
            cwd: "/tmp".into(),
        }))
        .unwrap();
        assert_eq!(
            (w.spaces[0].name.as_str(), w.spaces[0].cwd.as_str()),
            ("新名字", "/tmp")
        );
        assert!(
            w.edit(Edit::UpdateSpace(Space {
                id: "missing".into(),
                name: "x".into(),
                cwd: "/tmp".into(),
            }))
            .is_err()
        );
        assert!(
            w.edit(Edit::UpdateSpace(Space {
                id,
                name: " ".into(),
                cwd: "/tmp".into(),
            }))
            .is_err()
        );
    }
    #[test]
    fn tab_title_prefers_given_name_then_contents_then_program() {
        let mut w = workspace();
        for (tab, terminal) in [("first", "other"), ("second", "spare")] {
            w.terminals.push(Terminal {
                id: terminal.into(),
                ..w.terminals[0].clone()
            });
            w.edit(Edit::PutTab {
                tab: Tab {
                    id: tab.into(),
                    space: "a".into(),
                    default_task: None,
                    title: String::new(),
                    layout: Some(Layout::Leaf(terminal.into())),
                },
            })
            .unwrap();
        }
        tab(&mut w, "tab", "a", "a");
        w.observe_agent(observation("one")).unwrap();
        w.edit(Edit::AssignTask {
            id: "one".into(),
            task: None,
        })
        .unwrap();
        let mut titles = std::collections::HashMap::new();
        let title = |w: &Workspace,
                     titles: &std::collections::HashMap<String, String>,
                     i: usize| { w.tab_title(&w.tabs[i], titles, None) };
        // Never a bare position: the program, numbered only among same-named tabs.
        assert_eq!(title(&w, &titles, 0), "sh");
        assert_eq!(title(&w, &titles, 1), "sh 2");
        assert_eq!(title(&w, &titles, 2), "Codex");
        titles.insert("pty".to_owned(), "~/code — zsh".to_owned());
        assert_eq!(title(&w, &titles, 2), "~/code — zsh");
        let session = w.agent_sessions[0].id.clone();
        w.content.insert(
            session,
            crate::content::Content {
                title: "评估接入 Kling v4 ESR".into(),
                ..Default::default()
            },
        );
        assert_eq!(title(&w, &titles, 2), "评估接入 Kling v4 ESR");
        w.tabs[2].title = "kling".into();
        assert_eq!(title(&w, &titles, 2), "kling");
        titles.insert("spare".to_owned(), "vim".to_owned());
        assert_eq!(title(&w, &titles, 1), "vim");
        titles.insert("other".to_owned(), "htop".to_owned());
        assert_eq!(title(&w, &titles, 0), "htop");
    }
    #[test]
    fn agent_start_commands_need_no_daemon_flag_and_quote_resume_ids() {
        let first = Agent::Claude.command();
        let second = Agent::Claude.command();
        assert_ne!(first, second);
        assert!(uuid::Uuid::parse_str(first.strip_prefix("claude --session-id ").unwrap()).is_ok());
        assert_eq!(Agent::Codex.command(), "codex");
        assert_eq!(
            Agent::Codex.resume_command("a'b"),
            format!("codex resume {}", shell_quote("a'b"))
        );
        assert_eq!(
            Agent::Claude.resume_command("safe"),
            "claude --resume 'safe'"
        );
        assert_eq!(Agent::Grok.command(), "grok");
        assert_eq!(Agent::Grok.resume_command("id"), "grok --resume 'id'");
        for agent in Agent::all() {
            assert_eq!(Agent::from_executable(agent.executable()), Some(agent));
        }
        assert_eq!(Agent::from_executable("node"), None);
    }
    #[test]
    fn sequential_conversations_keep_independent_tasks_and_outcomes() {
        let mut w = workspace();
        tab(&mut w, "tab", "a", "a");
        w.observe_agent(observation("first")).unwrap();
        w.edit(Edit::SetOutcome {
            id: "first".into(),
            outcome: Outcome::Completed,
        })
        .unwrap();
        w.observe_agent(observation("second")).unwrap();
        w.edit(Edit::AssignTask {
            id: "second".into(),
            task: Some("b".into()),
        })
        .unwrap();
        assert_eq!(w.terminals.len(), 1);
        assert_eq!(w.agent_sessions.len(), 2);
        assert_eq!(w.agent_sessions[0].outcome, Outcome::Completed);
        assert_eq!(w.agent_sessions[0].task.as_deref(), Some("a"));
        assert_eq!(w.current_agent("pty").unwrap().cli_session_id, "second");
        assert_eq!(w.current_agent("pty").unwrap().task.as_deref(), Some("b"));
    }
    #[test]
    fn cross_space_placement_does_not_reassign_work_and_close_removes_the_tab() {
        let mut w = workspace();
        tab(&mut w, "old", "a", "a");
        w.observe_agent(observation("first")).unwrap();
        tab(&mut w, "new", "b", "b");
        assert!(
            w.tabs.iter().all(|t| t.id != "old"),
            "moving its only pane away closes the old tab"
        );
        assert_eq!(w.tab_terminals("new"), ["pty"]);
        assert_eq!(w.agent_sessions[0].task.as_deref(), Some("a"));
        assert_eq!(w.terminals[0].space, "a");
        w.edit(Edit::CloseTab { id: "new".into() }).unwrap();
        assert!(w.tabs.is_empty());
        assert!(w.terminal_tab("pty").is_none());
        assert!(w.edit(Edit::CloseTab { id: "new".into() }).is_err());
    }
    #[test]
    fn closing_the_last_pane_closes_its_tab() {
        let mut w = workspace();
        w.terminals.push(Terminal {
            id: "second".into(),
            ..w.terminals[0].clone()
        });
        w.edit(Edit::PutTab {
            tab: Tab {
                id: "t".into(),
                space: "a".into(),
                default_task: None,
                title: String::new(),
                layout: Some(Layout::Split {
                    axis: Axis::Right,
                    ratio: 5000,
                    first: Box::new(Layout::Leaf("pty".into())),
                    second: Box::new(Layout::Leaf("second".into())),
                }),
            },
        })
        .unwrap();
        w.edit(Edit::ClosePane {
            terminal: "second".into(),
        })
        .unwrap();
        assert_eq!(w.tab_terminals("t"), ["pty"]);
        w.edit(Edit::ClosePane {
            terminal: "pty".into(),
        })
        .unwrap();
        assert!(w.tabs.is_empty());
    }
    #[test]
    fn log_refresh_never_reopens_or_reassigns_an_existing_conversation() {
        let mut w = workspace();
        tab(&mut w, "t", "a", "a");
        w.observe_agent(observation("first")).unwrap();
        w.edit(Edit::AssignTask {
            id: "first".into(),
            task: None,
        })
        .unwrap();
        w.edit(Edit::SetOutcome {
            id: "first".into(),
            outcome: Outcome::Abandoned,
        })
        .unwrap();
        let mut unbound = observation("first");
        unbound.terminal = None;
        unbound.current = false;
        w.observe_agent(unbound).unwrap();
        assert_eq!(w.agent_sessions.len(), 1);
        assert_eq!(w.agent_sessions[0].task, None);
        assert_eq!(w.agent_sessions[0].outcome, Outcome::Abandoned);
    }
    #[test]
    fn task_prompt_and_per_criterion_evidence_are_independent() {
        let mut w = workspace();
        w.tasks[0].acceptance = vec![
            AcceptanceCriterion {
                id: "one".into(),
                description: "测试通过".into(),
                evidence: Some("test:42".into()),
            },
            AcceptanceCriterion {
                id: "two".into(),
                description: "原生输入正确".into(),
                evidence: None,
            },
        ];
        assert!(
            w.tasks[0]
                .prompt()
                .ends_with("验收标准：\n- 测试通过\n- 原生输入正确")
        );
        let mut bare = w.tasks[0].clone();
        bare.description = " ".into();
        bare.acceptance.clear();
        assert_eq!(
            bare.prompt(),
            bare.title.trim(),
            "blank parts add no headings"
        );
        assert_eq!(w.tasks[0].acceptance[1].evidence, None);
        assert_eq!(
            shell_quote("a'$(touch /tmp/no)\nb"),
            "'a'\\''$(touch /tmp/no)\nb'"
        );
    }
    #[test]
    fn session_names_prefer_the_user_then_the_cli_then_the_first_ask() {
        let mut w = Workspace::default();
        w.observe_agent(AgentSession {
            terminal: None,
            current: false,
            ..observation("one")
        })
        .unwrap();
        let id = w.agent_sessions[0].id.clone();
        let session = |w: &Workspace| w.agent_sessions[0].clone();
        assert_eq!(w.session_name(&session(&w)), None);
        w.content.insert(
            id.clone(),
            crate::content::Content {
                first_ask_known: true,
                title: "跳至主要内容 你帮我看看 这个页面讲了什么".into(),
                asks: vec![crate::content::Quote {
                    text: "跳至主要内容\n你帮我看看 这个页面讲了什么".into(),
                    ..Default::default()
                }],
                ..Default::default()
            },
        );
        assert_eq!(
            w.session_name(&session(&w)).as_deref(),
            Some("这个页面讲了什么")
        );
        w.content.get_mut(&id).unwrap().name = "总结页面".into();
        assert_eq!(w.session_name(&session(&w)).as_deref(), Some("总结页面"));
        w.edit(Edit::RenameSession {
            id: id.clone(),
            name: "我的名字".into(),
        })
        .unwrap();
        // Seeing the conversation again keeps what the user called it.
        w.observe_agent(AgentSession {
            terminal: None,
            current: false,
            ..observation("one")
        })
        .unwrap();
        assert_eq!(w.session_name(&session(&w)).as_deref(), Some("我的名字"));
        assert!(
            w.edit(Edit::RenameSession {
                id: "missing".into(),
                name: "x".into(),
            })
            .is_err()
        );
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    #[test]
    fn one_cli_identity_cannot_be_current_in_two_terminals() {
        let mut w = Workspace::default();
        for id in ["one", "two"] {
            w.terminals.push(Terminal {
                id: id.into(),
                space: "s".into(),
                argv: vec![],
                cwd: "/tmp".into(),
                pid: Some(1),
                state: ProcessState::Running,
                exit_code: None,
                error: None,
            });
        }
        let mut session = AgentSession {
            id: "a".into(),
            runtime: Agent::Codex,
            cli_session_id: "cli".into(),
            terminal: Some("one".into()),
            current: true,
            task: None,
            outcome: Outcome::Open,
            updated: 1,
            cwd: "/tmp".into(),
            transcript: "/log".into(),
            binding: None,
            name: String::new(),
        };
        w.observe_agent(session.clone()).unwrap();
        session.terminal = Some("two".into());
        assert!(w.observe_agent(session.clone()).is_err());
        assert_eq!(w.agent_sessions[0].terminal.as_deref(), Some("one"));
        w.agent_sessions[0].current = false;
        w.observe_agent(session).unwrap();
        assert_eq!(w.agent_sessions.len(), 1);
        assert_eq!(w.agent_sessions[0].terminal.as_deref(), Some("two"));
    }
}
