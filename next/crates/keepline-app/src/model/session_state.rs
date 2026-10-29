//! What a session is doing and whether it needs you, derived from the workspace alone.
use super::time::now;
use keepline_core::model::*;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Waiting,
    Error,
    Review,
    Paused,
    Running,
    Stale,
    /// Live states of an agent in a Keepline terminal, read like herdr's pane states.
    /// Not inbox groups: they never appear in `ALL`.
    Working,
    Done,
    Ready,
}
/// What a live agent in a Keepline terminal is doing right now, as herdr shows panes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Working,
    Waiting,
    Idle,
}
pub fn session_space<'a>(w: &'a Workspace, s: &AgentSession) -> Option<&'a str> {
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
pub fn live_terminal<'a>(w: &'a Workspace, s: &AgentSession) -> Option<&'a Terminal> {
    if !s.current {
        return None;
    }
    s.terminal.as_ref().and_then(|id| {
        w.terminals
            .iter()
            .find(|t| &t.id == id && t.state == ProcessState::Running)
    })
}
/// A resume that has not taken yet, or failed and left its shell, still owns a running
/// terminal in a tab; clicking the session returns there instead of stacking tabs.
pub fn resume_terminal<'a>(
    w: &'a Workspace,
    resumed: &HashMap<String, String>,
    s: &AgentSession,
) -> Option<&'a Terminal> {
    let id = resumed.get(&s.id)?;
    w.terminals
        .iter()
        .find(|t| &t.id == id && t.state == ProcessState::Running)
        .filter(|t| w.terminal_tab(&t.id).is_some())
}
/// `None` unless the session is the current conversation of a running Keepline terminal;
/// those keep the inbox grouping. A turn whose subagents still run is still working.
pub fn live_phase(w: &Workspace, s: &AgentSession) -> Option<Phase> {
    use keepline_core::content::Turn;
    live_terminal(w, s)?;
    let Some(c) = w.content.get(&s.id) else {
        return Some(Phase::Idle);
    };
    Some(match c.turn {
        Turn::Waiting => Phase::Waiting,
        Turn::Running => Phase::Working,
        Turn::Completed if c.background.is_some_and(|b| b.keeps_working()) => Phase::Working,
        _ => Phase::Idle,
    })
}
/// herdr's "done" versus "idle": a turn that ends out of view stays unseen until its
/// pane is shown; one that ends in view, or any other phase, is seen.
pub fn track_unseen(
    phases: &mut HashMap<String, Phase>,
    unseen: &mut HashSet<String>,
    live: impl IntoIterator<Item = (String, Phase, bool)>,
) {
    let before = std::mem::take(phases);
    for (id, phase, shown) in live {
        if phase != Phase::Idle || shown {
            unseen.remove(&id);
        } else if before.get(&id).is_some_and(|p| *p != Phase::Idle) {
            unseen.insert(id.clone());
        }
        phases.insert(id, phase);
    }
    unseen.retain(|id| phases.contains_key(id));
}
pub fn changed_since(w: &Workspace, s: &AgentSession, since: u64) -> bool {
    since > 0 && w.content.get(&s.id).map_or(s.updated, |c| c.updated) > since
}
pub fn show_on_return(left: u64, at: u64, minutes: u32) -> bool {
    left > 0 && at.saturating_sub(left) > u64::from(minutes) * 60
}
pub fn interrupted(workspace: &Workspace, id: &str) -> bool {
    workspace
        .terminals
        .iter()
        .any(|t| t.id == id && t.state == ProcessState::Interrupted)
}
/// The conversation to pick up again in a terminal the host lost: the latest one it ran
/// that has no outcome yet. The host clears `current` for every lost terminal, so this
/// cannot tell an Agent killed mid-run from one that had already been quit there.
pub fn interrupted_session<'a>(w: &'a Workspace, terminal: &str) -> Option<&'a AgentSession> {
    w.agent_sessions
        .iter()
        .filter(|s| s.terminal.as_deref() == Some(terminal) && s.outcome == Outcome::Open)
        .max_by_key(|s| s.updated)
}
pub fn attachable_ids(workspace: &Workspace, ids: Vec<String>) -> Vec<String> {
    ids.into_iter()
        .filter(|id| !interrupted(workspace, id))
        .collect()
}
pub fn session_group(w: &Workspace, s: &AgentSession) -> Group {
    use keepline_core::content::{Content, Inbox};
    match w
        .content
        .get(&s.id)
        .cloned()
        .unwrap_or_else(|| Content {
            updated: s.updated,
            ..Default::default()
        })
        .group(s, w, now())
    {
        Inbox::Waiting => Group::Waiting,
        Inbox::Error => Group::Error,
        Inbox::Review => Group::Review,
        Inbox::Running => Group::Running,
        Inbox::Stale => Group::Stale,
        _ => Group::Paused,
    }
}
/// The sidebar's dot and label: a live agent in a Keepline terminal shows what it is
/// doing now; every other session keeps its inbox group. The flag marks attention.
pub fn sidebar_status(w: &Workspace, unseen: &HashSet<String>, s: &AgentSession) -> (Group, bool) {
    match live_phase(w, s) {
        Some(Phase::Working) => (Group::Working, false),
        Some(Phase::Waiting) => (Group::Waiting, true),
        Some(Phase::Idle) if unseen.contains(&s.id) => (Group::Done, true),
        Some(Phase::Idle) => (Group::Ready, false),
        None => (session_group(w, s), needs_attention(w, s)),
    }
}
/// Waiting or failed in a Keepline terminal and not yet looked at.
pub fn needs_attention(w: &Workspace, s: &AgentSession) -> bool {
    s.managed()
        && s.outcome.needs_review(now())
        && matches!(session_group(w, s), Group::Waiting | Group::Error)
}
