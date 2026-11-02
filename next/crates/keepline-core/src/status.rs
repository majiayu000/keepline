//! The one answer to "what is this session doing", for every surface: sidebar, panes,
//! tabs, status bar, board, overview, palette, menu bar, plugins, phones. Each state has
//! one key, one word and one glyph; the app gives it one color (specs/status-model).
//!
//! Where the reading came from (the screen, a hook, the log) is evidence, not a state:
//! a turn running in a Keepline terminal and one running in a log are both 在跑.
//! Whether you have looked yet is attention, not a state: an unseen finished turn is
//! still 等你验收.
use crate::content::{Content, Turn};
use crate::model::{AgentSession, Outcome, ProcessState, Terminal, Workspace};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Waiting,
    Error,
    Running,
    /// The turn ended but something it started still runs (a shell, a monitor, a
    /// scheduled wakeup) and may bring it back. Counts with 在跑.
    Background,
    /// The turn ended; whether the work is done is yours to say (2026-11-02).
    Review,
    /// A live agent that has not started a turn yet: just opened, or `/clear`ed.
    Fresh,
    /// The turn was interrupted, or the agent is gone without finishing one.
    Paused,
    /// Untouched past the stale days.
    Stale,
    /// You put it off until a time that has not come yet.
    Deferred,
    /// You marked it complete (or abandoned it): the only 做完了.
    Closed,
}

impl Status {
    pub const ALL: [Self; 10] = [
        Self::Waiting,
        Self::Error,
        Self::Running,
        Self::Background,
        Self::Review,
        Self::Fresh,
        Self::Paused,
        Self::Stale,
        Self::Deferred,
        Self::Closed,
    ];
    /// The name in `colors.json`, the plugin protocol and logs.
    pub fn key(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Error => "error",
            Self::Running => "running",
            Self::Background => "background",
            Self::Review => "review",
            Self::Fresh => "fresh",
            Self::Paused => "paused",
            Self::Stale => "stale",
            Self::Deferred => "deferred",
            Self::Closed => "closed",
        }
    }
    /// One word per state everywhere.
    pub fn label(self) -> &'static str {
        match self {
            Self::Waiting => "等你回答",
            Self::Error => "出错了",
            Self::Running => "在跑",
            Self::Background => "等后台",
            Self::Review => "等你验收",
            Self::Fresh => "新开的",
            Self::Paused => "停下了",
            Self::Stale => "很久没动",
            Self::Deferred => "稍后再看",
            Self::Closed => "做完了",
        }
    }
    /// The terminal style's symbol. ✓ is yours to give; a turn waiting for it is the open
    /// diamond beside 等你回答's ◆.
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Waiting => "◆",
            Self::Error => "✕",
            Self::Running => "◐",
            Self::Background => "◔",
            Self::Review => "◇",
            Self::Fresh => "○",
            Self::Paused => "‖",
            Self::Stale => "·",
            Self::Deferred => "◷",
            Self::Closed => "✓",
        }
    }
    /// Waiting on you or failed.
    pub fn needs_you(self) -> bool {
        matches!(self, Self::Waiting | Self::Error)
    }
    /// Work in flight: 在跑 and 等后台 count together.
    pub fn working(self) -> bool {
        matches!(self, Self::Running | Self::Background)
    }
}

/// A terminal with no conversation whose program reports its own state (OSC 7501):
/// working, blocked, error, done or idle, read as the same states sessions use.
pub fn of_program(state: &str) -> Status {
    match state {
        "working" => Status::Running,
        "blocked" => Status::Waiting,
        "error" => Status::Error,
        "done" => Status::Review,
        _ => Status::Paused,
    }
}

/// The running Keepline terminal whose current conversation this is.
pub fn live_terminal<'a>(w: &'a Workspace, s: &AgentSession) -> Option<&'a Terminal> {
    if !s.current {
        return None;
    }
    let id = s.terminal.as_ref()?;
    w.terminals
        .iter()
        .find(|t| &t.id == id && t.state == ProcessState::Running)
}

/// No turn has started in this conversation: no record yet, or one with no ask and no
/// turn state. An empty Claude reads as this, not as 等你验收.
pub fn fresh(c: Option<&Content>) -> bool {
    c.is_none_or(|c| c.asks.is_empty() && c.turn == Turn::Unknown)
}

/// The session's state now. The only place that decides it.
pub fn of(w: &Workspace, s: &AgentSession, now: u64) -> Status {
    let content = w.content.get(&s.id);
    if live_terminal(w, s).is_some() {
        live(s, content, now)
    } else {
        recorded(w, s, content, now)
    }
}

/// What you decided overrides what the agent last did, once its turn is over.
fn decided(s: &AgentSession, now: u64) -> Option<Status> {
    match s.outcome {
        Outcome::Completed | Outcome::Abandoned => Some(Status::Closed),
        Outcome::Deferred { until, .. } if until > now => Some(Status::Deferred),
        _ => None,
    }
}

/// An agent in a Keepline terminal: what it is doing right now wins.
fn live(s: &AgentSession, c: Option<&Content>, now: u64) -> Status {
    let background = c.and_then(|c| c.background).unwrap_or_default();
    let turn = c.map_or(Turn::Unknown, |c| c.turn);
    match turn {
        Turn::Waiting => return Status::Waiting,
        Turn::Running => return Status::Running,
        // Its subagents still work for it: the turn is not over.
        Turn::Completed if background.keeps_working() => return Status::Running,
        _ => (),
    }
    if let Some(d) = decided(s, now) {
        return d;
    }
    if fresh(c) {
        return Status::Fresh;
    }
    match turn {
        Turn::Error => Status::Error,
        _ if background.wakes() || background.shells > 0 => Status::Background,
        Turn::Interrupted => Status::Paused,
        _ => Status::Review,
    }
}

/// Every other session: what its log and its terminal's exit say.
fn recorded(w: &Workspace, s: &AgentSession, c: Option<&Content>, now: u64) -> Status {
    if let Some(d) = decided(s, now) {
        return d;
    }
    let default = Content {
        updated: s.updated,
        ..Default::default()
    };
    let c = c.unwrap_or(&default);
    // External logs prove activity, not the existence of a live OS process indefinitely.
    let running = c.turn == Turn::Running && (s.current || now.saturating_sub(c.updated) < 900);
    let last_turn = if c.turn_started > 0 {
        c.turn_started
    } else {
        c.updated
    };
    let since = match s.outcome {
        Outcome::Deferred { until, .. } => last_turn.max(until),
        _ => last_turn.max(w.review_since(&s.id)),
    };
    if !running && now.saturating_sub(since) > u64::from(w.settings.stale_days) * 86400 {
        return Status::Stale;
    }
    if c.turn == Turn::Waiting {
        return Status::Waiting;
    }
    let failed = w
        .terminals
        .iter()
        .find(|t| s.terminal.as_ref() == Some(&t.id))
        .is_some_and(|t| {
            matches!(t.state, ProcessState::Failed | ProcessState::Interrupted)
                || t.exit_code.is_some_and(|c| c != 0)
        });
    let background = c.background.unwrap_or_default();
    if c.turn == Turn::Error || failed {
        Status::Error
    } else if c.turn == Turn::Completed && s.current && background.keeps_working() {
        Status::Running
    } else if c.turn == Turn::Completed
        && s.current
        && (background.wakes() || background.shells > 0)
    {
        Status::Background
    } else if c.turn == Turn::Completed {
        Status::Review
    } else if running {
        Status::Running
    } else {
        Status::Paused
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Background;
    use crate::model::Agent;

    const NOW: u64 = 10 * 86400;

    fn session(terminal: Option<&str>, current: bool) -> AgentSession {
        AgentSession {
            id: "s".into(),
            runtime: Agent::Codex,
            cli_session_id: "c".into(),
            terminal: terminal.map(Into::into),
            current,
            task: None,
            outcome: Outcome::Open,
            updated: NOW,
            cwd: "/tmp".into(),
            transcript: "f".into(),
            binding: None,
            name: String::new(),
        }
    }
    fn content(turn: Turn) -> Content {
        Content {
            updated: NOW,
            turn_started: NOW,
            turn,
            asks: vec![crate::content::Quote::default()],
            ..Default::default()
        }
    }
    /// A workspace whose terminal `t` runs, or has exited with `exit`.
    fn workspace(c: Option<Content>, exit: Option<i32>) -> Workspace {
        let mut w = Workspace::default();
        w.terminals.push(Terminal {
            id: "t".into(),
            space: String::new(),
            argv: vec![],
            cwd: "/tmp".into(),
            pid: None,
            state: if exit.is_some() {
                ProcessState::Exited
            } else {
                ProcessState::Running
            },
            exit_code: exit,
            error: None,
        });
        if let Some(c) = c {
            w.content.insert("s".into(), c);
        }
        w
    }
    fn live_status(c: Option<Content>) -> Status {
        of(&workspace(c, None), &session(Some("t"), true), NOW)
    }
    fn recorded_status(c: Option<Content>) -> Status {
        of(&workspace(c, Some(0)), &session(Some("t"), false), NOW)
    }

    #[test]
    fn live_agent_reads_what_it_does_now() {
        assert_eq!(live_status(Some(content(Turn::Waiting))), Status::Waiting);
        assert_eq!(live_status(Some(content(Turn::Running))), Status::Running);
        assert_eq!(live_status(None), Status::Fresh);
        assert_eq!(live_status(Some(content(Turn::Error))), Status::Error);
        assert_eq!(
            live_status(Some(content(Turn::Interrupted))),
            Status::Paused
        );
        assert_eq!(live_status(Some(content(Turn::Completed))), Status::Review);
        let with = |b: Background| {
            let mut c = content(Turn::Completed);
            c.background = Some(b);
            live_status(Some(c))
        };
        assert_eq!(
            with(Background {
                agents: 1,
                ..Default::default()
            }),
            Status::Running
        );
        assert_eq!(
            with(Background {
                shells: 1,
                ..Default::default()
            }),
            Status::Background
        );
        assert_eq!(
            with(Background {
                crons: 1,
                ..Default::default()
            }),
            Status::Background
        );
    }

    #[test]
    fn your_decision_overrides_an_ended_turn_but_not_a_running_one() {
        let mut w = workspace(Some(content(Turn::Completed)), None);
        let mut s = session(Some("t"), true);
        s.outcome = Outcome::Completed;
        assert_eq!(of(&w, &s, NOW), Status::Closed);
        s.outcome = Outcome::Deferred {
            reason: String::new(),
            until: NOW + 1,
        };
        assert_eq!(of(&w, &s, NOW), Status::Deferred);
        assert_eq!(of(&w, &s, NOW + 1), Status::Review);
        w.content.get_mut("s").unwrap().turn = Turn::Running;
        s.outcome = Outcome::Completed;
        assert_eq!(of(&w, &s, NOW), Status::Running);
    }

    #[test]
    fn recorded_session_reads_its_log() {
        assert_eq!(
            recorded_status(Some(content(Turn::Waiting))),
            Status::Waiting
        );
        assert_eq!(
            recorded_status(Some(content(Turn::Completed))),
            Status::Review
        );
        assert_eq!(
            recorded_status(Some(content(Turn::Interrupted))),
            Status::Paused
        );
        assert_eq!(recorded_status(Some(content(Turn::Error))), Status::Error);
        // Exited non-zero: failed whatever the log said last.
        let w = workspace(Some(content(Turn::Completed)), Some(1));
        assert_eq!(of(&w, &session(Some("t"), false), NOW), Status::Error);
        // A log still running within 15 minutes is running; past the stale days, stale.
        let s = session(None, false);
        let w = workspace(Some(content(Turn::Running)), None);
        assert_eq!(of(&w, &s, NOW + 60), Status::Running);
        assert_eq!(of(&w, &s, NOW + 4 * 86400), Status::Stale);
    }

    #[test]
    fn every_status_has_its_own_key_word_and_glyph() {
        for (i, a) in Status::ALL.iter().enumerate() {
            for b in &Status::ALL[i + 1..] {
                assert_ne!(a.key(), b.key());
                assert_ne!(a.label(), b.label());
                assert_ne!(a.glyph(), b.glyph());
            }
        }
    }
}
