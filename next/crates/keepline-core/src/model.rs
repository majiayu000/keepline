pub use crate::layout::{Axis, Layout};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Workspace {
    pub revision: u64,
    #[serde(default)]
    pub outcome_history: Vec<OutcomeChange>,
    #[serde(default)]
    pub task_outcomes: std::collections::HashMap<String, Outcome>,
    /// One proven binding consumes this override; None cancels the tab default.
    #[serde(default)]
    pub pending_tasks: std::collections::HashMap<String, Option<String>>,
    pub spaces: Vec<Space>,
    pub tasks: Vec<Task>,
    pub tabs: Vec<Tab>,
    pub terminals: Vec<Terminal>,
    pub agent_sessions: Vec<AgentSession>,
    pub content: std::collections::HashMap<String, crate::content::Content>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Agent {
    Codex,
    Claude,
}
impl Agent {
    /// Temporary until daemon-backed Codex can prove a TUI-to-session association.
    pub fn command(self) -> &'static str {
        match self {
            Self::Codex => "codex --no-daemon",
            Self::Claude => "claude",
        }
    }
    pub fn resume_command(self, cli_id: &str) -> String {
        match self {
            Self::Codex => format!("codex --no-daemon resume {}", shell_quote(cli_id)),
            Self::Claude => format!("claude --resume {}", shell_quote(cli_id)),
        }
    }
    pub fn executable(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }
}
impl Task {
    pub fn command(&self) -> String {
        let criteria = self
            .acceptance
            .iter()
            .map(|c| format!("- {}", c.description))
            .collect::<Vec<_>>()
            .join("\n");
        let prompt = format!(
            "{}\n{}\n验收标准：\n{}",
            self.title, self.description, criteria
        );
        format!("{} {}", self.agent.command(), shell_quote(&prompt))
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
    pub closed: bool,
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
    AddSpace(Space),
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
    CloseTab {
        id: String,
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
}

fn validate_outcome(outcome: &Outcome) -> Result<(), String> {
    if let Outcome::Deferred { reason, until } = outcome {
        if reason.trim().is_empty() || *until == 0 {
            return Err("暂存需要原因和再看时间".into());
        }
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
                if c.hidden_reason().is_some() || !session.outcome.needs_review(now) {
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
                        .saturating_add(3 * 86400 + 1),
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

    pub fn tab_title(&self, tab: &Tab) -> String {
        let ids = self.tab_terminals(&tab.id);
        let sessions: Vec<_> = self
            .agent_sessions
            .iter()
            .filter(|s| s.current && s.terminal.as_ref().is_some_and(|id| ids.contains(id)))
            .collect();
        if sessions.is_empty() {
            if let Some(task) = tab
                .default_task
                .as_ref()
                .and_then(|id| self.tasks.iter().find(|t| &t.id == id))
            {
                return task.title.clone();
            }
        }
        if let Some(first) = sessions.first().and_then(|s| s.task.as_ref()) {
            if sessions.iter().all(|s| s.task.as_ref() == Some(first)) {
                if let Some(task) = self.tasks.iter().find(|t| &t.id == first) {
                    return task.title.clone();
                }
            }
        }
        if !tab.title.is_empty() {
            return tab.title.clone();
        }
        self.spaces
            .iter()
            .find(|s| s.id == tab.space)
            .and_then(|s| std::path::Path::new(&s.cwd).file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "终端".into())
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
            observed.outcome = old.outcome.clone();
            observed.updated = old.updated.max(observed.updated);
            if observed.terminal.is_none() {
                observed.terminal = old.terminal.clone();
                observed.current = old.current;
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
            }
            Edit::CloseTab { id } => {
                self.tabs
                    .iter_mut()
                    .find(|t| t.id == id)
                    .ok_or("标签不存在")?
                    .closed = true;
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
        }
    }
    fn tab(w: &mut Workspace, id: &str, space: &str, task: &str) {
        w.edit(Edit::PutTab {
            tab: Tab {
                id: id.into(),
                space: space.into(),
                default_task: Some(task.into()),
                title: id.into(),
                closed: false,
                layout: Some(Layout::Leaf("pty".into())),
            },
        })
        .unwrap();
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
        w.edit(Edit::CloseTab { id: "tab".into() }).unwrap();
        assert_eq!(
            w.task_terminal("a").unwrap().id,
            "pty",
            "closed view retains the launch target"
        );
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
        assert_eq!(w.tab_title(&w.tabs[0]), "新的待办名");
        assert_eq!(w.tasks[0].acceptance, task.acceptance);
        w.edit(Edit::AssignTask {
            id: "one".into(),
            task: None,
        })
        .unwrap();
        assert_eq!(w.tab_title(&w.tabs[0]), "tab");
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
    fn codex_prefill_and_resume_use_no_daemon_and_preserve_shell_quoting() {
        assert_eq!(Agent::Codex.command(), "codex --no-daemon");
        assert_eq!(
            Agent::Codex.resume_command("a'b"),
            format!("codex --no-daemon resume {}", shell_quote("a'b"))
        );
        assert_eq!(
            Agent::Claude.resume_command("safe"),
            "claude --resume 'safe'"
        );
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
    fn cross_space_placement_does_not_reassign_work_and_close_preserves_layout() {
        let mut w = workspace();
        tab(&mut w, "old", "a", "a");
        w.observe_agent(observation("first")).unwrap();
        tab(&mut w, "new", "b", "b");
        assert!(w.tab_terminals("old").is_empty());
        assert_eq!(w.tab_terminals("new"), ["pty"]);
        assert_eq!(w.agent_sessions[0].task.as_deref(), Some("a"));
        assert_eq!(w.terminals[0].space, "a");
        w.edit(Edit::CloseTab { id: "new".into() }).unwrap();
        assert_eq!(w.terminals[0].state, ProcessState::Running);
        assert_eq!(w.tab_terminals("new"), ["pty"]);
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
        assert!(w.tasks[0].command().contains("- 测试通过\n- 原生输入正确"));
        assert_eq!(w.tasks[0].acceptance[1].evidence, None);
        assert_eq!(
            shell_quote("a'$(touch /tmp/no)\nb"),
            "'a'\\''$(touch /tmp/no)\nb'"
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
