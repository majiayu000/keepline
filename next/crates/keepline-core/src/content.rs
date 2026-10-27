//! Small, sourced projections. The transcript and mechanical receipts remain distinct.
use crate::model::{AgentSession, Outcome, ProcessState, Workspace};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub path: String,
    pub inode: u64,
    pub offset: u64,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Work still in flight when the turn last stopped, from the Stop hook payload.
    /// `None` means no payload reported it, which is unknown rather than "nothing runs".
    #[serde(default)]
    pub background: Option<Background>,
    /// Counts entries into Waiting, so one dialog reported by several signals is one event.
    #[serde(default)]
    pub waiting_episode: u32,
    /// Which signal set the current turn: 日志, Hook, 画面 or 官方状态.
    #[serde(default)]
    pub signal: String,
    #[serde(default)]
    pub signal_at: u64,
    /// A Codex subagent's parent thread id and its name, from its own session header.
    #[serde(default)]
    pub parent: String,
    #[serde(default)]
    pub agent_name: String,
}
/// In-flight work by kind. Agents (subagents, workflows, teammates, cloud sessions, MCP
/// tasks) keep a stopped turn working. A monitor or a scheduled wakeup will wake the session
/// again, so it is not done either: it is waiting to wake, as Claude Code's agent view shows
/// it. Shells alone are a mark on a finished turn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Background {
    pub agents: u32,
    pub shells: u32,
    pub monitors: u32,
    pub crons: u32,
}
impl Background {
    pub fn keeps_working(&self) -> bool {
        self.agents > 0
    }
    pub fn wakes(&self) -> bool {
        self.monitors > 0 || self.crons > 0
    }
    /// "后台仍有子 Agent 1 个、shell 2 个", or `None` when nothing is in flight.
    pub fn describe(&self) -> Option<String> {
        let parts: Vec<String> = [
            (self.agents, "子 Agent"),
            (self.shells, "shell"),
            (self.monitors, "监视任务"),
            (self.crons, "已排定唤醒"),
        ]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, kind)| format!("{kind} {n} 个"))
        .collect();
        (!parts.is_empty()).then(|| format!("后台仍有{}", parts.join("、")))
    }
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
            Self::Review => "做完了",
            Self::Paused => "停下了",
            Self::Running => "在跑",
            Self::Stale => "很久没动",
            Self::Deferred => "稍后再看",
            Self::Closed => "已收尾",
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
/// Agent prose as one readable line: Markdown emphasis, headings, quotes, list
/// bullets and table rules go; table cells read as `a · b`.
pub fn plain_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.chars().all(|c| "|-: ".contains(c)))
        .map(|l| {
            let l = l
                .trim_start_matches('#')
                .trim_start_matches('>')
                .trim_start();
            let l = ["- ", "* ", "+ "]
                .iter()
                .find_map(|b| l.strip_prefix(b))
                .unwrap_or(l);
            let l = l.replace("**", "").replace("__", "").replace('`', "");
            l.trim_matches('|')
                .split('|')
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .collect::<Vec<_>>()
                .join(" · ")
        })
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
/// A first ask as a name: the polite lead-in most asks share ("你帮我看看") goes,
/// so tabs and rows start with what differs.
pub fn topic(title: &str) -> String {
    const LEAD: [&str; 16] = [
        "你",
        "请",
        "帮我",
        "帮忙",
        "麻烦",
        "能不能",
        "可以",
        "看看",
        "看下",
        "看一下",
        "查查",
        "查一下",
        "一下",
        "，",
        ",",
        " ",
    ];
    let mut rest = title.trim();
    while let Some(next) = LEAD.iter().find_map(|p| rest.strip_prefix(p)) {
        if next.trim().chars().count() < 4 {
            break;
        }
        rest = next;
    }
    rest.trim().to_owned()
}
/// `Here::done` when nothing confirmed has happened yet.
pub const NO_DONE: &str = "尚无已确认的修改、提交或测试成功证据";
/// Hidden reason for a session whose working directory is temporary.
pub const TEMPORARY_DIRECTORY: &str = "临时目录会话";
impl Content {
    /// One readable line on where the session stands, without the labels its state chip
    /// already says ("回合结束；Agent 自述：", "等待输入："): the question it waits on,
    /// the error, what it was asked while it runs, or its last reply.
    pub fn headline(&self) -> String {
        let text = match self.turn {
            Turn::Waiting if !self.waiting.is_empty() => self.waiting.as_str(),
            Turn::Error if !self.error.is_empty() => self.error.as_str(),
            Turn::Running => self.asks.last().map_or("", |q| q.text.as_str()),
            _ => self.last_reply.as_str(),
        };
        let line = plain_line(text);
        if line.is_empty() {
            plain_line(&self.here().stopped)
        } else {
            line
        }
    }
    /// `hidden_reason` for this session: one Keepline started in its own terminal was
    /// started by the user, so a temporary working directory alone does not hide it.
    /// Sub-agents, SDK and print-mode runs stay hidden wherever they run.
    /// The turn ended and only a monitor or a scheduled wakeup will resume it.
    pub fn waking(&self) -> bool {
        self.turn == Turn::Completed
            && self
                .background
                .is_some_and(|b| !b.keeps_working() && b.wakes())
    }
    pub fn hidden_for(&self, session: &AgentSession) -> Option<&str> {
        if session.managed() && self.hidden_reason == TEMPORARY_DIRECTORY {
            return None;
        }
        self.hidden_reason()
    }
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
        if !running && now.saturating_sub(since) > u64::from(workspace.settings.stale_days) * 86400
        {
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
        // A stopped turn whose subagents still run, or that a monitor or wakeup will resume,
        // is not done yet.
        let background = self.turn == Turn::Completed
            && session.current
            && self
                .background
                .is_some_and(|b| b.keeps_working() || b.wakes());
        if self.turn == Turn::Error || failed {
            Inbox::Error
        } else if background {
            Inbox::Running
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
            NO_DONE.into()
        } else {
            facts.join("\n")
        };
        let stopped = match self.turn {
            Turn::Waiting => format!("等待输入：{}", short(&self.waiting, 240)),
            Turn::Error => format!("最后回合出错：{}", short(&self.error, 240)),
            Turn::Running => "日志最后记录：回合进行中；不代表此刻进程仍在线".into(),
            Turn::Interrupted => "回合已中断，尚未验收".into(),
            Turn::Completed => match self.background.and_then(|b| b.describe()) {
                Some(work) if self.waking() => format!(
                    "等待唤醒，{work}；Agent 自述：{}",
                    short(&self.last_reply, 220)
                ),
                Some(work) => format!(
                    "回合结束，{work}；Agent 自述：{}",
                    short(&self.last_reply, 220)
                ),
                None => format!("回合结束；Agent 自述：{}", short(&self.last_reply, 260)),
            },
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
            Turn::Completed if self.background.is_some_and(|b| b.keeps_working()) => {
                "等后台子 Agent 结束后再核对结果"
            }
            Turn::Completed if self.waking() => "等它被监视任务或定时唤醒，回合结束后再核对",
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
    #[test]
    fn topic_drops_shared_lead_in() {
        assert_eq!(topic("你帮我看看 mirror 为什么挂了"), "mirror 为什么挂了");
        assert_eq!(topic("你帮我看看我最近有什么…"), "我最近有什么…");
        assert_eq!(topic("帮我看看"), "帮我看看");
        assert_eq!(topic("修复登录"), "修复登录");
    }
    #[test]
    fn headline_drops_the_labels_the_state_already_says() {
        let c = Content {
            last_reply: "**建议都接收**".into(),
            turn: Turn::Completed,
            ..Default::default()
        };
        assert_eq!(c.headline(), "建议都接收");
        let c = Content {
            waiting: "是否允许推送分支？".into(),
            turn: Turn::Waiting,
            ..Default::default()
        };
        assert_eq!(c.headline(), "是否允许推送分支？");
        let c = Content {
            asks: vec![Quote {
                text: "修复登录跳转".into(),
                ..Default::default()
            }],
            turn: Turn::Running,
            ..Default::default()
        };
        assert_eq!(c.headline(), "修复登录跳转");
    }
    #[test]
    fn plain_line_drops_markdown() {
        assert_eq!(
            plain_line(
                "## 结论\n**建议都接收**，见 `a.rs`\n\n| 投稿 | 建议处理 |\n|---|:-:|\n- 第一条"
            ),
            "结论 建议都接收，见 a.rs 投稿 · 建议处理 第一条"
        );
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
    #[test]
    fn running_subagents_keep_a_stopped_turn_working_but_shells_do_not() {
        let now = 10 * 86400;
        let s = AgentSession {
            id: "s".into(),
            runtime: Agent::Claude,
            cli_session_id: "c".into(),
            terminal: Some("t".into()),
            current: true,
            task: None,
            outcome: Outcome::Open,
            updated: now,
            cwd: "/tmp".into(),
            transcript: "f".into(),
        };
        let w = Workspace::default();
        let mut c = Content {
            updated: now,
            turn: Turn::Completed,
            completed_claim: true,
            background: Some(Background {
                agents: 1,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(c.group(&s, &w, now), Inbox::Running);
        c.background = Some(Background {
            monitors: 1,
            ..Default::default()
        });
        assert_eq!(
            c.group(&s, &w, now),
            Inbox::Running,
            "a monitor will wake it"
        );
        assert!(c.waking());
        assert!(
            c.here()
                .stopped
                .starts_with("等待唤醒，后台仍有监视任务 1 个")
        );
        c.background = Some(Background {
            crons: 1,
            ..Default::default()
        });
        assert_eq!(
            c.group(&s, &w, now),
            Inbox::Running,
            "so will a scheduled wakeup"
        );
        c.background = Some(Background {
            shells: 2,
            ..Default::default()
        });
        assert!(!c.waking());
        assert_eq!(
            c.group(&s, &w, now),
            Inbox::Review,
            "shells alone are a mark"
        );
        c.background = None;
        assert_eq!(
            c.group(&s, &w, now),
            Inbox::Review,
            "unknown is not working"
        );
        let mut gone = s.clone();
        gone.current = false;
        c.background = Some(Background {
            agents: 1,
            ..Default::default()
        });
        assert_eq!(
            c.group(&gone, &w, now),
            Inbox::Review,
            "no live process, nothing runs"
        );
    }
}
