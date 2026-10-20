//! Small, sourced projections. The transcript and mechanical receipts remain distinct.
use crate::model::{AgentSession, Outcome, ProcessState, Workspace};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub path: String,
    pub inode: u64,
    pub offset: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quote {
    pub text: String,
    pub at: u64,
    pub source: Source,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub kind: String,
    pub summary: String,
    pub exit_code: Option<i32>,
    pub at: u64,
    pub source: Source,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Turn {
    #[default]
    Unknown,
    Running,
    Waiting,
    Completed,
    Interrupted,
    Error,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Content {
    #[serde(default)]
    pub interactive: bool,
    #[serde(default)]
    pub hidden_reason: String,
    #[serde(default)]
    pub first_ask_known: bool,
    pub title: String,
    pub asks: Vec<Quote>,
    pub evidence: Vec<Evidence>,
    pub last_reply: String,
    pub turn: Turn,
    pub turn_started: u64,
    pub turn_key: String,
    pub updated: u64,
    pub completed_claim: bool,
    pub waiting: String,
    pub error: String,
    pub partial_history: bool,
    pub from_offset: u64,
    pub unknown_records: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Inbox {
    Waiting,
    Error,
    Review,
    Paused,
    Running,
    Stale,
    Deferred,
    Closed,
}
impl Inbox {
    pub fn label(self) -> &'static str {
        match self {
            Self::Waiting => "等你回答",
            Self::Error => "出错了",
            Self::Review => "待核对",
            Self::Paused => "做一半停了",
            Self::Running => "进行中",
            Self::Stale => "很久没动",
            Self::Deferred => "暂存",
            Self::Closed => "已结束",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Here {
    pub ask: String,
    pub done: String,
    pub stopped: String,
    pub next: String,
}
pub fn short(text: &str, limit: usize) -> String {
    let mut s: String = text.chars().take(limit).collect();
    if text.chars().count() > limit {
        s.push('…');
    }
    s
}
impl Content {
    pub fn hidden_reason(&self) -> Option<&str> {
        if !self.hidden_reason.is_empty() {
            Some(&self.hidden_reason)
        } else if !self.interactive {
            Some("缺少交互入口证据")
        } else {
            None
        }
    }
    pub fn group(&self, session: &AgentSession, workspace: &Workspace, now: u64) -> Inbox {
        match session.outcome {
            Outcome::Completed | Outcome::Abandoned => return Inbox::Closed,
            Outcome::Deferred { until, .. } if until > now => return Inbox::Deferred,
            _ => (),
        }
        // External logs prove activity, not the existence of a live OS process indefinitely.
        let running = self.turn == Turn::Running
            && (session.current || now.saturating_sub(self.updated) < 900);
        let last_turn = if self.turn_started > 0 {
            self.turn_started
        } else {
            self.updated
        };
        let since = match session.outcome {
            Outcome::Deferred { until, .. } => last_turn.max(until),
            _ => last_turn.max(workspace.review_since(&session.id)),
        };
        if !running && now.saturating_sub(since) > 3 * 86400 {
            return Inbox::Stale;
        }
        if self.turn == Turn::Waiting {
            return Inbox::Waiting;
        }
        let failed = workspace
            .terminals
            .iter()
            .find(|t| session.terminal.as_ref() == Some(&t.id))
            .is_some_and(|t| {
                matches!(t.state, ProcessState::Failed | ProcessState::Interrupted)
                    || t.exit_code.is_some_and(|c| c != 0)
            });
        if self.turn == Turn::Error || failed {
            Inbox::Error
        } else if self.turn == Turn::Completed && self.completed_claim {
            Inbox::Review
        } else if running {
            Inbox::Running
        } else {
            Inbox::Paused
        }
    }
    pub fn here(&self) -> Here {
        let ask = if self.asks.is_empty() {
            "历史尾段未覆盖用户原话，要求未知".into()
        } else {
            let quotes = if self.asks.len() <= 3 {
                self.asks.iter().collect::<Vec<_>>()
            } else {
                std::iter::once(&self.asks[0])
                    .chain(self.asks.iter().skip(self.asks.len() - 2))
                    .collect()
            };
            quotes
                .into_iter()
                .map(|q| short(&q.text, 240))
                .collect::<Vec<_>>()
                .join("\n追加：")
        };
        let ask = if !self.first_ask_known && !self.asks.is_empty() {
            format!("最初要求未覆盖；已读取追加：\n{ask}")
        } else {
            ask
        };
        let facts: Vec<_> = self
            .evidence
            .iter()
            .filter(|f| {
                f.exit_code == Some(0) && matches!(f.kind.as_str(), "file" | "test" | "commit")
            })
            .rev()
            .take(4)
            .map(|f| short(&f.summary, 180))
            .collect();
        let done = if facts.is_empty() {
            "尚无已确认的修改、提交或测试成功证据".into()
        } else {
            facts.join("\n")
        };
        let stopped = match self.turn {
            Turn::Waiting => format!("等待输入：{}", short(&self.waiting, 240)),
            Turn::Error => format!("最后回合出错：{}", short(&self.error, 240)),
            Turn::Running => "日志最后记录：回合进行中；不代表此刻进程仍在线".into(),
            Turn::Interrupted => "回合已中断，尚未验收".into(),
            Turn::Completed => format!("回合结束；Agent 自述：{}", short(&self.last_reply, 260)),
            Turn::Unknown => {
                if self.last_reply.is_empty() {
                    "尾段没有明确回合结束事件，当前状态未知".into()
                } else {
                    format!(
                        "最近 Agent 自述（状态未知）：{}",
                        short(&self.last_reply, 240)
                    )
                }
            }
        };
        let next = match self.turn {
            Turn::Waiting => "回应上面的审批或问题",
            Turn::Error => "检查失败证据后决定是否继续",
            Turn::Running => "等回合结束后核对结果",
            Turn::Completed if self.completed_claim => "按用户原话逐项核对机械证据，再决定是否验收",
            _ => "先核对原话与日志，再决定是否继续",
        }
        .into();
        Here {
            ask,
            done,
            stopped,
            next,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn self_report_never_becomes_done() {
        let c = Content {
            last_reply: "全部测试通过，已提交 abc1234".into(),
            turn: Turn::Completed,
            completed_claim: true,
            ..Default::default()
        };
        assert!(c.here().done.contains("尚无"));
        assert!(c.here().stopped.contains("自述"));
    }
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod grouping_tests {
    use super::*;
    use crate::model::{Agent, Outcome};
    #[test]
    fn grouping_priority_and_deferred_expiry() {
        let now = 10 * 86400;
        let mut s = AgentSession {
            id: "s".into(),
            runtime: Agent::Codex,
            cli_session_id: "c".into(),
            terminal: None,
            current: false,
            task: None,
            outcome: Outcome::Open,
            updated: now,
            cwd: "/tmp".into(),
            transcript: "f".into(),
        };
        let w = Workspace::default();
        let mut c = Content {
            updated: now,
            turn: Turn::Waiting,
            ..Default::default()
        };
        assert_eq!(c.group(&s, &w, now), Inbox::Waiting);
        c.turn = Turn::Error;
        assert_eq!(c.group(&s, &w, now), Inbox::Error);
        c.turn = Turn::Completed;
        c.completed_claim = true;
        assert_eq!(c.group(&s, &w, now), Inbox::Review);
        c.completed_claim = false;
        assert_eq!(c.group(&s, &w, now), Inbox::Paused);
        c.turn = Turn::Running;
        assert_eq!(c.group(&s, &w, now), Inbox::Running);
        c.updated = 1;
        assert_eq!(c.group(&s, &w, now), Inbox::Stale);
        s.outcome = Outcome::Deferred {
            until: now + 1,
            reason: "later".into(),
        };
        assert_eq!(c.group(&s, &w, now), Inbox::Deferred);
        s.outcome = Outcome::Deferred {
            until: now - 1,
            reason: "later".into(),
        };
        assert_eq!(c.group(&s, &w, now), Inbox::Paused);
        s.outcome = Outcome::Completed;
        assert_eq!(c.group(&s, &w, now), Inbox::Closed);
    }
}
