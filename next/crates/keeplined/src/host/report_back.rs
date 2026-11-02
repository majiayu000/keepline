//! `keepline run`'s report back: an Agent started for another one reports each turn it
//! ends (done, a question for the user, an error) into its starter's input, wrapped as
//! `<keepline from=…>` so the starter knows it is not the user speaking. A report waits
//! while the starter is working or waiting on the user, and goes in once it is idle.
//!
//! Live state only: links and waiting reports go with the host, and a link ends when
//! either terminal exits.
use keepline_core::content::{now, Turn};
use keepline_core::hook::Event;
use keepline_core::model::{ProcessState, Workspace};
use keepline_core::status;
use std::collections::HashMap;

/// A report longer than this is cut; the starter can `keepline read` the rest.
const LONGEST: usize = 6000;
/// Reports for a starter that never goes idle; past this the oldest goes.
const QUEUE: usize = 20;

#[derive(Default)]
pub(super) struct ReportBack {
    /// Child terminal -> its starter, and the turn end last reported.
    links: HashMap<String, Link>,
    /// Starter terminal -> reports not typed yet, oldest first.
    outbox: HashMap<String, Vec<String>>,
}

struct Link {
    parent: String,
    /// The turn end last seen, reported or not.
    last: Option<String>,
    /// The last report sent: a turn whose key moves as its log is read again (Codex
    /// settles its turn id after the reply) says the same thing and is not sent twice.
    said: Option<String>,
}

impl ReportBack {
    /// From now on `child` reports to `parent`; whatever it shows now is not news.
    pub(super) fn link(&mut self, w: &Workspace, child: &str, parent: &str) {
        let now = turn_end(w, child);
        self.links.insert(
            child.into(),
            Link {
                parent: parent.into(),
                last: now.as_ref().map(|(key, _)| key.clone()),
                said: now.map(|(_, text)| text),
            },
        );
    }
    pub(super) fn is_empty(&self) -> bool {
        self.links.is_empty() && self.outbox.is_empty()
    }
    /// What to type now: each idle starter with its waiting reports, joined.
    pub(super) fn plan(&mut self, w: &Workspace) -> Vec<(String, String)> {
        let running = |id: &str| {
            w.terminals
                .iter()
                .any(|t| t.id == id && t.state == ProcessState::Running)
        };
        self.links
            .retain(|child, l| running(child) && running(&l.parent));
        let outbox = &mut self.outbox;
        for (child, link) in self.links.iter_mut() {
            let Some((key, text)) = turn_end(w, child) else {
                continue;
            };
            if link.last.as_ref() == Some(&key) {
                continue;
            }
            link.last = Some(key);
            if link.said.as_ref() == Some(&text) {
                continue;
            }
            link.said = Some(text.clone());
            let queue = outbox.entry(link.parent.clone()).or_default();
            queue.push(text);
            if queue.len() > QUEUE {
                queue.remove(0);
            }
        }
        outbox.retain(|parent, queue| running(parent) && !queue.is_empty());
        let ready: Vec<String> = outbox
            .keys()
            .filter(|parent| idle(w, parent))
            .cloned()
            .collect();
        ready
            .into_iter()
            .filter_map(|p| outbox.remove(&p).map(|q| (p, q.join("\n\n"))))
            .collect()
    }
}

/// The turn end `child`'s Agent shows now, keyed so each is reported once, and its report.
fn turn_end(w: &Workspace, child: &str) -> Option<(String, String)> {
    let s = w.current_agent(child)?;
    let c = w.content.get(&s.id)?;
    let event = crate::plugin_hooks::event(status::of(w, s, now()), c.turn)?;
    let agent = s.runtime.name();
    let (key, body, hint) = match event {
        Event::Done => (
            format!("done:{}:{}", c.turn_key, c.turn_started),
            // The turn ended before its reply was read: report it once the reply is.
            match c.last_reply.trim() {
                "" => return None,
                reply => reply.to_owned(),
            },
            format!("要它接着做：keepline prompt {child} \"…\"；看画面：keepline read {child}"),
        ),
        Event::Waiting => (
            format!(
                "waiting:{}:{}:{}",
                c.turn_key, c.turn_started, c.waiting_episode
            ),
            format!("它在等回答：{}", c.waiting.trim()),
            format!(
                "看画面：keepline read {child}；替它回答前先问用户，用户同意后 keepline prompt {child} --force \"…\""
            ),
        ),
        Event::Error => (
            format!("error:{}:{}:{}", c.turn_key, c.turn_started, c.error),
            format!("出错了：{}", c.error.trim()),
            format!("看画面：keepline read {child}"),
        ),
        Event::Started | Event::CommandDone | Event::CommandFailed => return None,
    };
    let text = format!(
        "<keepline from=\"{child}\" agent=\"{agent}\" event=\"{}\">\n{}\n</keepline>\n这是你用 keepline run 派出去的 {agent} 的汇报，不是用户说的话。{hint}",
        event.name(),
        clean(&body),
    );
    Some((key, text))
}

/// One paste's worth of plain text: no control characters but line breaks and tabs.
fn clean(text: &str) -> String {
    let mut out: String = text
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .take(LONGEST)
        .collect();
    if text.chars().count() > LONGEST {
        out.push_str("\n…（太长，后面截掉了）");
    }
    out
}

fn idle(w: &Workspace, terminal: &str) -> bool {
    let Some(s) = w.current_agent(terminal) else {
        return false;
    };
    w.content.get(&s.id).is_none_or(|c| {
        !matches!(c.turn, Turn::Running | Turn::Waiting)
            && !c.background.is_some_and(|b| b.keeps_working())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use keepline_core::agent::Agent;
    use keepline_core::content::Content;
    use keepline_core::model::*;

    fn terminal(id: &str) -> Terminal {
        let mut t: Terminal = serde_json::from_value(serde_json::json!({
            "id": id, "space": "s", "cwd": "/tmp", "argv": ["zsh"], "state": "Running",
        }))
        .unwrap();
        t.state = ProcessState::Running;
        t
    }
    fn session(id: &str, terminal: &str, runtime: Agent) -> AgentSession {
        AgentSession {
            id: id.into(),
            runtime,
            cli_session_id: id.into(),
            terminal: Some(terminal.into()),
            current: true,
            task: None,
            outcome: Outcome::Open,
            updated: now(),
            cwd: "/tmp".into(),
            transcript: String::new(),
            name: String::new(),
            binding: None,
        }
    }
    fn content(turn: Turn, reply: &str, started: u64) -> Content {
        Content {
            interactive: true,
            turn,
            turn_key: "k".into(),
            turn_started: started,
            last_reply: reply.into(),
            updated: now(),
            ..Default::default()
        }
    }
    fn workspace() -> Workspace {
        let mut w = Workspace::default();
        w.terminals = vec![terminal("parent"), terminal("child")];
        w.agent_sessions = vec![
            session("p", "parent", Agent::Claude),
            session("c", "child", Agent::Codex),
        ];
        w.content.insert("p".into(), content(Turn::Running, "", 1));
        w.content.insert("c".into(), content(Turn::Running, "", 1));
        w
    }

    #[test]
    fn a_finished_turn_waits_for_an_idle_starter_and_goes_in_once() {
        let mut w = workspace();
        let mut r = ReportBack::default();
        r.link(&w, "child", "parent");
        assert!(r.plan(&w).is_empty(), "nothing ended yet");
        // The child finishes while its starter is still working: it waits.
        w.content.insert(
            "c".into(),
            content(Turn::Completed, "改好了，测试 12 个全过", 2),
        );
        assert!(r.plan(&w).is_empty(), "the starter is busy");
        // The starter goes idle: the report goes in, wrapped, once.
        w.content
            .insert("p".into(), content(Turn::Completed, "", 1));
        let sent = r.plan(&w);
        assert_eq!(sent.len(), 1);
        let (to, text) = &sent[0];
        assert_eq!(to, "parent");
        assert!(text.starts_with("<keepline from=\"child\" agent=\"Codex\" event=\"done\">\n改好了，测试 12 个全过\n</keepline>"), "{text}");
        assert!(text.contains("不是用户说的话") && text.contains("keepline prompt child"));
        assert!(r.plan(&w).is_empty(), "once");
        // The same turn read again under another key says the same: not sent twice.
        let mut reread = content(Turn::Completed, "改好了，测试 12 个全过", 2);
        reread.turn_key = "settled".into();
        w.content.insert("c".into(), reread);
        assert!(r.plan(&w).is_empty(), "the same report again");
        // A turn that ended before its reply was read waits for the reply.
        w.content
            .insert("c".into(), content(Turn::Completed, "", 5));
        assert!(r.plan(&w).is_empty(), "no reply read yet");
        // Its next turn reports again.
        w.content
            .insert("c".into(), content(Turn::Completed, "第二轮", 3));
        assert!(r.plan(&w)[0].1.contains("第二轮"));
    }

    #[test]
    fn questions_are_reported_and_links_end_with_their_terminals() {
        let mut w = workspace();
        w.content
            .insert("p".into(), content(Turn::Completed, "", 1));
        let mut r = ReportBack::default();
        r.link(&w, "child", "parent");
        let mut asking = content(Turn::Waiting, "", 2);
        asking.waiting = "要覆盖 config.toml 吗？".into();
        asking.waiting_episode = 1;
        w.content.insert("c".into(), asking);
        let sent = r.plan(&w);
        assert!(sent[0].1.contains("event=\"waiting\""));
        assert!(sent[0].1.contains("它在等回答：要覆盖 config.toml 吗？"));
        assert!(sent[0].1.contains("先问用户"));
        w.terminals.retain(|t| t.id != "child");
        r.plan(&w);
        assert!(r.is_empty(), "the child exited");
    }

    #[test]
    fn a_report_is_plain_text_and_bounded() {
        assert_eq!(clean("a\x1b[31mb\u{7}\nc\td"), "a[31mb\nc\td");
        let long = "字".repeat(LONGEST + 5);
        let cut = clean(&long);
        assert!(cut.ends_with("后面截掉了）"));
        assert_eq!(cut.chars().filter(|c| *c == '字').count(), LONGEST);
    }
}
