//! Files two live agents both changed lately: the same path in the same checkout, so one
//! may overwrite the other or their changes end up mixed in one commit.
use super::session_state::live_terminal;
use keepline_core::model::*;
use std::collections::{BTreeMap, HashMap};
use std::path::{Component, Path, PathBuf};

/// How far back a change still counts; older ones were most likely committed or settled.
pub const WINDOW: u64 = 3 * 3600;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collision {
    /// Absolute path of the shared file.
    pub path: String,
    /// The other live sessions that changed it, by session id.
    pub others: Vec<String>,
}

/// For each live session in a Keepline terminal, the files it shares with another one,
/// most recently changed first. Sessions with no shared file are left out.
pub fn collisions(w: &Workspace, now: u64) -> HashMap<String, Vec<Collision>> {
    let since = now.saturating_sub(WINDOW);
    // path -> (session id -> when it last changed the path)
    let mut by_path: BTreeMap<PathBuf, BTreeMap<&str, u64>> = BTreeMap::new();
    for s in &w.agent_sessions {
        if live_terminal(w, s).is_none() {
            continue;
        }
        let Some(c) = w.content.get(&s.id) else {
            continue;
        };
        for t in c.files.iter().filter(|t| t.at >= since) {
            let at = by_path
                .entry(resolve(&s.cwd, &t.path))
                .or_default()
                .entry(s.id.as_str())
                .or_default();
            *at = (*at).max(t.at);
        }
    }
    let mut out: HashMap<String, Vec<(u64, Collision)>> = HashMap::new();
    for (path, sessions) in by_path.iter().filter(|(_, s)| s.len() > 1) {
        for (id, at) in sessions {
            out.entry((*id).to_owned()).or_default().push((
                *at,
                Collision {
                    path: path.to_string_lossy().into_owned(),
                    others: sessions
                        .keys()
                        .filter(|o| *o != id)
                        .map(|o| (*o).to_owned())
                        .collect(),
                },
            ));
        }
    }
    out.into_iter()
        .map(|(id, mut list)| {
            list.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.path.cmp(&b.1.path)));
            (id, list.into_iter().map(|(_, c)| c).collect())
        })
        .collect()
}

/// `path` against `cwd` when relative, with `.` and `..` folded away without touching the
/// disk: a deleted file still names the place both agents wrote to.
fn resolve(cwd: &str, path: &str) -> PathBuf {
    let joined = Path::new(cwd).join(path);
    let mut out = PathBuf::new();
    for part in joined.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use keepline_core::content::Content;

    const NOW: u64 = 100_000;

    fn session(w: &mut Workspace, id: &str, cwd: &str, running: bool, files: &[(&str, u64)]) {
        let terminal = format!("t-{id}");
        w.terminals.push(Terminal {
            id: terminal.clone(),
            space: String::new(),
            argv: vec![],
            cwd: cwd.into(),
            pid: None,
            state: if running {
                ProcessState::Running
            } else {
                ProcessState::Exited
            },
            exit_code: None,
            error: None,
        });
        w.agent_sessions.push(AgentSession {
            id: id.into(),
            runtime: Agent::Claude,
            cli_session_id: id.into(),
            terminal: Some(terminal),
            current: true,
            task: None,
            outcome: Outcome::Open,
            updated: NOW,
            cwd: cwd.into(),
            transcript: String::new(),
            name: String::new(),
            binding: None,
        });
        let mut c = Content::default();
        for (path, at) in files {
            c.touch(path, *at);
        }
        w.content.insert(id.into(), c);
    }

    #[test]
    fn same_file_from_two_live_sessions_marks_both() {
        let mut w = Workspace::default();
        session(
            &mut w,
            "a",
            "/r",
            true,
            &[("/r/src/x.rs", NOW - 60), ("/r/y.rs", NOW)],
        );
        // Relative to its own cwd, through `..`: still the same file.
        session(&mut w, "b", "/r/sub", true, &[("../src/./x.rs", NOW - 30)]);
        let found = collisions(&w, NOW);
        assert_eq!(
            found["a"],
            [Collision {
                path: "/r/src/x.rs".into(),
                others: vec!["b".into()],
            }]
        );
        assert_eq!(found["b"][0].others, ["a"]);
        assert_eq!(found.len(), 2);

        w.agent_sessions[1].name = "修侧栏".into();
        let lines = super::super::session_state::state_reasons(&w, "t-a", None, NOW);
        assert_eq!(lines.last().unwrap(), "和「修侧栏」都改了 src/x.rs");
    }

    #[test]
    fn no_mark_without_a_second_live_recent_session_in_the_same_checkout() {
        let mut w = Workspace::default();
        // The same session changing a file again is not a collision.
        session(
            &mut w,
            "a",
            "/r",
            true,
            &[("/r/x.rs", NOW - 10), ("x.rs", NOW)],
        );
        // Same name in another repo or a linked worktree is another file.
        session(&mut w, "b", "/other", true, &[("x.rs", NOW)]);
        session(&mut w, "c", "/r-wt", true, &[("/r-wt/x.rs", NOW)]);
        // An exited session and an old change do not count.
        session(&mut w, "d", "/r", false, &[("/r/x.rs", NOW)]);
        session(&mut w, "e", "/r", true, &[("/r/x.rs", NOW - WINDOW - 1)]);
        assert!(collisions(&w, NOW).is_empty());
    }

    #[test]
    fn three_sessions_list_each_other_and_recent_paths_come_first() {
        let mut w = Workspace::default();
        session(
            &mut w,
            "a",
            "/r",
            true,
            &[("old.rs", NOW - 600), ("new.rs", NOW)],
        );
        session(
            &mut w,
            "b",
            "/r",
            true,
            &[("old.rs", NOW - 500), ("new.rs", NOW - 5)],
        );
        session(&mut w, "c", "/r", true, &[("new.rs", NOW - 9)]);
        let found = collisions(&w, NOW);
        let a: Vec<_> = found["a"].iter().map(|c| c.path.as_str()).collect();
        assert_eq!(a, ["/r/new.rs", "/r/old.rs"]);
        assert_eq!(found["a"][0].others, ["b", "c"]);
        assert_eq!(found["c"][0].others, ["a", "b"]);
    }
}
