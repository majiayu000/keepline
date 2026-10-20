pub use crate::layout::{Axis, Layout};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Workspace {
    pub revision: u64,
    pub spaces: Vec<Space>,
    pub tasks: Vec<Task>,
    pub tabs: Vec<Tab>,
    pub terminals: Vec<Terminal>,
    pub agent_sessions: Vec<AgentSession>,
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
impl Outcome {
    pub fn needs_review(&self, now: u64) -> bool {
        matches!(self, Self::Open) || matches!(self, Self::Deferred { until, .. } if *until <= now)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Edit {
    AddSpace(Space),
    AddTask(Task),
    PutTab { tab: Tab },
    CloseTab { id: String },
    SetOutcome { id: String, outcome: Outcome },
    AssignTask { id: String, task: Option<String> },
}

impl Workspace {
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
                    .terminal_tab(&terminal.id)
                    .and_then(|t| t.default_task.clone());
            }
            if observed.current {
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
                if let Outcome::Deferred { reason, until } = &outcome
                    && (reason.trim().is_empty() || *until == 0)
                {
                    return Err("暂存需要原因和再看时间".into());
                }
                self.agent_sessions
                    .iter_mut()
                    .find(|s| s.id == id)
                    .ok_or("Agent 会话不存在")?
                    .outcome = outcome;
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
