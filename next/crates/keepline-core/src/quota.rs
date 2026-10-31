//! Account usage limits, as each CLI writes them into its own conversation logs.
//! Keepline never asks a provider for them: a reading is only as fresh as the last record
//! some conversation of that CLI wrote, so every use keeps its time.
use crate::model::{Agent, Workspace};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Readings older than this are left out: that CLI has not been used for a week.
pub const SHOWN_FOR: u64 = 7 * 86_400;
/// From here a window shows as nearly used up.
pub const NEAR: u16 = 80;
/// From here a window shows as used up.
pub const OVER: u16 = 95;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quota {
    /// When the CLI recorded it, Unix seconds.
    pub at: u64,
    /// The plan the CLI reported (plus, pro, max…); empty when it gave none.
    #[serde(default)]
    pub plan: String,
    /// Shortest window first.
    pub windows: Vec<Window>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Window {
    pub minutes: u32,
    /// Percent used as the provider reported it, rounded; above 100 once exceeded.
    pub used: u16,
    /// Unix seconds; 0 when the CLI gave no time.
    pub resets_at: u64,
}
impl Window {
    /// The windows CLIs report today get their usual names; any other its length.
    pub fn label(&self) -> String {
        match self.minutes {
            300 => "5h".into(),
            10_080 => "周".into(),
            43_200 => "月".into(),
            m if m % 1440 == 0 => format!("{}天", m / 1440),
            m if m % 60 == 0 => format!("{}h", m / 60),
            m => format!("{m}分"),
        }
    }
    /// The window rolled over after the reading, so its percentage no longer applies.
    pub fn reset(&self, now: u64) -> bool {
        self.resets_at != 0 && now >= self.resets_at
    }
}

/// The newest reading of each CLI used in the last week, in the app's agent order.
pub fn latest(w: &Workspace, now: u64) -> Vec<(Agent, &Quota)> {
    let mut newest: HashMap<Agent, &Quota> = HashMap::new();
    for s in &w.agent_sessions {
        let Some(q) = w.content.get(&s.id).and_then(|c| c.quota.as_ref()) else {
            continue;
        };
        if q.windows.is_empty() || now.saturating_sub(q.at) > SHOWN_FOR {
            continue;
        }
        let slot = newest.entry(s.runtime).or_insert(q);
        if q.at > slot.at {
            *slot = q;
        }
    }
    let order = Agent::all();
    let mut out: Vec<_> = newest.into_iter().collect();
    out.sort_by_key(|(a, _)| {
        (
            order.iter().position(|o| o == a).unwrap_or(order.len()),
            a.key(),
        )
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Content;
    use crate::model::{AgentSession, Outcome};

    fn session(id: &str, runtime: Agent) -> AgentSession {
        AgentSession {
            id: id.into(),
            runtime,
            cli_session_id: id.into(),
            terminal: None,
            current: false,
            task: None,
            outcome: Outcome::Open,
            updated: 0,
            cwd: "/tmp".into(),
            transcript: String::new(),
            binding: None,
            name: String::new(),
        }
    }
    fn reading(at: u64, used: u16) -> Content {
        Content {
            quota: Some(Quota {
                at,
                plan: String::new(),
                windows: vec![Window {
                    minutes: 300,
                    used,
                    resets_at: at + 3600,
                }],
            }),
            ..Default::default()
        }
    }

    #[test]
    fn newest_reading_per_agent_in_agent_order() {
        let now = 10 * 86_400;
        let mut w = Workspace {
            agent_sessions: vec![
                session("a", Agent::Codex),
                session("b", Agent::Codex),
                session("c", Agent::Claude),
                session("old", Agent::Grok),
                session("none", Agent::Cursor),
            ],
            ..Default::default()
        };
        w.content.insert("a".into(), reading(now - 600, 40));
        w.content.insert("b".into(), reading(now - 60, 42));
        w.content.insert("c".into(), reading(now - 30, 7));
        w.content
            .insert("old".into(), reading(now - SHOWN_FOR - 1, 99));
        w.content.insert("none".into(), Content::default());
        let got: Vec<_> = latest(&w, now)
            .into_iter()
            .map(|(a, q)| (a.key(), q.windows[0].used))
            .collect();
        assert_eq!(got, [("Codex", 42), ("Claude", 7)]);
    }

    #[test]
    fn labels_and_rollover() {
        let w = |minutes| Window {
            minutes,
            used: 0,
            resets_at: 100,
        };
        assert_eq!(w(300).label(), "5h");
        assert_eq!(w(10_080).label(), "周");
        assert_eq!(w(43_200).label(), "月");
        assert_eq!(w(2880).label(), "2天");
        assert_eq!(w(120).label(), "2h");
        assert!(w(300).reset(100) && !w(300).reset(99));
        assert!(!Window::default().reset(u64::MAX));
    }
}
