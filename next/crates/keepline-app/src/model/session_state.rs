//! What a session is doing and whether it needs you, derived from the workspace alone.
use super::time::now;
use keepline_core::model::*;
use std::collections::{HashMap, HashSet};

pub use keepline_core::status::{Status, live_terminal};
/// What a live agent in a Keepline terminal is doing right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Working,
    Waiting,
    Idle,
}
/// A terminal with no live conversation whose program reports its own status (OSC 7501):
/// the dot and the line its pane and sidebar row show.
pub fn report_view(
    r: &keepline_core::protocol::ProgramReport,
) -> (Status, keepline_core::protocol::TerminalNote) {
    let group = keepline_core::status::of_program(&r.state);
    let word = group.label();
    let who = [&r.app, &r.title]
        .into_iter()
        .find(|s| !s.is_empty())
        .map_or("程序", String::as_str);
    let progress = r.progress.map(|p| format!(" {p}%")).unwrap_or_default();
    let note = if r.msg.is_empty() {
        format!("{who} · {word}{progress}")
    } else {
        format!("{who} · {word}{progress}：{}", r.msg)
    };
    let mut why = "终端里的程序自己通过 OSC 7501 报告的状态".to_owned();
    if !r.title.is_empty() && r.title != who {
        why = format!("{}\n{why}", r.title);
    }
    (group, keepline_core::protocol::TerminalNote { note, why })
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
/// What a live agent in a Keepline terminal is doing, for tracking turns that end out
/// of view and warning before closing one that works; `None` outside a running Keepline
/// terminal. A projection of [`status_of`], never a second judgment.
pub fn live_phase(w: &Workspace, s: &AgentSession) -> Option<Phase> {
    live_terminal(w, s)?;
    Some(match status_of(w, s) {
        Status::Waiting => Phase::Waiting,
        Status::Running => Phase::Working,
        _ => Phase::Idle,
    })
}
/// "Done" versus "idle": a turn that ends out of view stays unseen until its
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
/// The conversation to pick up again in a terminal the host lost: the one it was running
/// then, as the host recorded. A terminal lost before that record was kept falls back to
/// the latest one it ran that has no outcome yet, which may have been quit already.
pub fn interrupted_session<'a>(w: &'a Workspace, terminal: &str) -> Option<&'a AgentSession> {
    match w.lost_agents.get(terminal) {
        Some(id) => id
            .as_ref()
            .and_then(|id| w.agent_sessions.iter().find(|s| &s.id == id)),
        None => w
            .agent_sessions
            .iter()
            .filter(|s| s.terminal.as_deref() == Some(terminal) && s.outcome == Outcome::Open)
            .max_by_key(|s| s.updated),
    }
}
pub fn attachable_ids(workspace: &Workspace, ids: Vec<String>) -> Vec<String> {
    ids.into_iter()
        .filter(|id| !interrupted(workspace, id))
        .collect()
}
/// Why a pane shows the state it does, line by line, for the pane menu's 「为什么是这个状态」:
/// which signal set it and when, the record, event or screen rule it carried, and what it
/// is waiting on, or why the screen showed no prompt. `hint` is the recognition note
/// for a terminal whose Agent was seen but not tied to a conversation.
pub fn state_reasons(w: &Workspace, terminal: &str, hint: Option<&str>, now: u64) -> Vec<String> {
    let Some(s) = w.current_agent(terminal) else {
        let mut lines = vec!["这个画面里没有认出 Agent 对话，只按普通终端显示。".to_owned()];
        if let Some(hint) = hint {
            lines.push(format!("识别提示：{hint}"));
        }
        return lines;
    };
    let mut lines = vec![format!(
        "{} 对话 {}",
        s.runtime.name(),
        &s.cli_session_id[..s.cli_session_id.len().min(8)]
    )];
    let Some(c) = w.content.get(&s.id) else {
        lines.push("还没读到这段对话的记录：还没开始第一轮，按「新开的」显示。".into());
        return lines;
    };
    let status = status_of(w, s);
    let phase = match status {
        Status::Fresh => "新开的 / 还没开始第一轮",
        Status::Review => "等你验收 / 这一轮停了，做没做完由你定",
        Status::Background => "等后台 / 这一轮停了，后台还有任务在跑",
        Status::Closed => "做完了 / 你标记过完成",
        Status::Paused => "停下了 / 这一轮被打断，或 Agent 没做完就退出了",
        Status::Deferred => "稍后再看 / 你推迟到了以后",
        other => other.label(),
    };
    lines.push(format!("现在：{phase}"));
    let why = match c.signal.as_str() {
        "日志" => "Agent 写进对话记录的最后一条",
        "Hook" => "Agent 通过 hook 报来的事件",
        "画面" => "画面底部认出了提问框；对话记录里没有这一条",
        "官方状态" => "Agent 自己报告的运行状态",
        "标题" => "终端标题里的转圈，Agent 正在跑这一轮",
        "画面在跑" => "画面上 Agent 每秒在走的计时行，正在跑这一轮",
        "程序上报" => "终端里的程序自己通过 OSC 7501 报告的状态",
        _ => "对话记录",
    };
    let at = if c.signal_at > 0 {
        c.signal_at
    } else {
        c.updated
    };
    lines.push(format!("依据：{why}（{}）", super::time::ago(at, now)));
    // The record, hook event or screen rule behind it, as the daemon wrote it down.
    lines.extend(c.signal_detail.lines().map(str::to_owned));
    // Nothing on screen is waiting on you: say the rules looked and found no prompt.
    if !matches!(status, Status::Running | Status::Waiting) && !c.screen_check.is_empty() {
        lines.push(format!("画面：{}，所以按「{phase}」显示。", c.screen_check));
    }
    if !c.waiting.is_empty() {
        lines.push(format!("在等：{}", c.waiting));
    }
    if !c.error.is_empty() {
        lines.push(format!("出错：{}", c.error));
    }
    if let Some(b) = c.background.and_then(|b| b.describe()) {
        lines.push(b);
    }
    lines.extend(shared_files(w, s, now));
    lines
}
/// 「和 X 都改了 a.rs」 for each file another live agent in this checkout also changed lately.
fn shared_files(w: &Workspace, s: &AgentSession, now: u64) -> Vec<String> {
    let Some(found) = super::collisions::collisions(w, now).remove(&s.id) else {
        return Vec::new();
    };
    let name = |id: &String| {
        w.agent_sessions
            .iter()
            .find(|o| &o.id == id)
            .and_then(|o| w.session_name(o))
            .map(|n| format!("「{n}」"))
            .unwrap_or_else(|| "另一个 Agent".into())
    };
    let mut lines: Vec<String> = found
        .iter()
        .take(3)
        .map(|c| {
            let path = std::path::Path::new(&c.path);
            let shown = path.strip_prefix(&s.cwd).unwrap_or(path).display();
            let others: Vec<_> = c.others.iter().map(name).collect();
            format!("和{}都改了 {shown}", others.join("、"))
        })
        .collect();
    if found.len() > 3 {
        lines.push(format!(
            "另有 {} 个文件也被别的 Agent 改过",
            found.len() - 3
        ));
    }
    lines
}
/// The session's state now: [`keepline_core::status::of`], the only judgment.
pub fn status_of(w: &Workspace, s: &AgentSession) -> Status {
    keepline_core::status::of(w, s, now())
}
/// The overview's and the palette's sections, in their order: one per state, except
/// that 等后台 sits in 在跑's section.
pub const SECTIONS: [Status; 9] = [
    Status::Waiting,
    Status::Error,
    Status::Review,
    Status::Paused,
    Status::Running,
    Status::Fresh,
    Status::Stale,
    Status::Deferred,
    Status::Closed,
];
pub fn section(status: Status) -> Status {
    if status.working() {
        Status::Running
    } else {
        status
    }
}
/// The state with whether it asks for you: waiting or failed in a Keepline terminal and
/// not yet handled, or a turn that ended out of view (`unseen`) and was not looked at.
pub fn session_status(w: &Workspace, unseen: &HashSet<String>, s: &AgentSession) -> (Status, bool) {
    let status = status_of(w, s);
    let attention = (status.needs_you() && s.managed() && s.outcome.needs_review(now()))
        || (unseen.contains(&s.id) && matches!(status, Status::Review | Status::Paused));
    (status, attention)
}
/// Waiting or failed in a Keepline terminal and not yet looked at.
pub fn needs_attention(w: &Workspace, s: &AgentSession) -> bool {
    s.managed() && s.outcome.needs_review(now()) && status_of(w, s).needs_you()
}

#[cfg(test)]
mod report_view_tests {
    use super::*;
    use keepline_core::protocol::ProgramReport;

    #[test]
    fn a_report_reads_as_its_dot_and_one_line() {
        let report = |state: &str, app: &str, msg: &str, progress| ProgramReport {
            state: state.into(),
            kind: None,
            progress,
            app: app.into(),
            title: String::new(),
            msg: msg.into(),
            since: 0,
        };
        let (g, n) = report_view(&report("working", "deploy", "", Some(30)));
        assert!(g == Status::Running);
        assert_eq!(n.note, "deploy · 在跑 30%");
        let (g, n) = report_view(&report("blocked", "", "要部署吗？", None));
        assert!(g == Status::Waiting);
        assert_eq!(n.note, "程序 · 等你回答：要部署吗？");
        assert!(report_view(&report("done", "x", "", None)).0 == Status::Review);
        assert!(report_view(&report("error", "x", "", None)).0 == Status::Error);
        assert!(report_view(&report("idle", "x", "", None)).0 == Status::Paused);
    }
}
