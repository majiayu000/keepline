use super::*;

fn fixture() -> Workspace {
    let mut w = Workspace::default();
    w.spaces.push(Space {
        id: "space".into(),
        name: "Space".into(),
        cwd: "/tmp".into(),
    });
    w.tabs.push(Tab {
        id: "tab".into(),
        space: "space".into(),
        title: "four panes".into(),
        layout: Some({
            let mut l = Layout::Leaf("t0".into());
            for i in 1..4 {
                l.split(&format!("t{}", i - 1), format!("t{i}"), Axis::Right);
            }
            l
        }),
        default_task: None,
        closed: false,
    });
    for i in 0..4 {
        w.terminals.push(Terminal {
            id: format!("t{i}"),
            space: "space".into(),
            argv: vec!["/bin/sh".into()],
            cwd: "/tmp".into(),
            pid: None,
            state: if i == 1 {
                ProcessState::Interrupted
            } else {
                ProcessState::Running
            },
            exit_code: None,
            error: None,
        });
    }
    w.agent_sessions.push(AgentSession {
        id: "history".into(),
        runtime: Agent::Codex,
        cli_session_id: "cli".into(),
        terminal: Some("t1".into()),
        current: false,
        task: None,
        outcome: Outcome::Completed,
        updated: 1,
        cwd: "/tmp".into(),
        transcript: "".into(),
    });
    w
}

#[test]
fn navigation_uses_live_identity_and_path_boundaries() {
    let mut w = fixture();
    let mut s = w.agent_sessions[0].clone();
    s.current = true;
    assert!(live_terminal(&w, &s).is_none());
    s.terminal = Some("t0".into());
    assert_eq!(live_terminal(&w, &s).unwrap().id, "t0");
    s.current = false;
    assert!(live_terminal(&w, &s).is_none());
    s.terminal = None;
    s.cwd = "/tmp/project".into();
    assert_eq!(session_space(&w, &s), Some("space"));
    s.cwd = "/tmp-other/project".into();
    assert_eq!(session_space(&w, &s), None);
    w.spaces.push(Space {
        id: "nested".into(),
        name: "Nested".into(),
        cwd: "/tmp/project".into(),
    });
    s.cwd = "/tmp/project/sub".into();
    assert_eq!(session_space(&w, &s), Some("nested"));
    s.updated = 100;
    assert!(!changed_since(&w, &s, 100));
    assert!(changed_since(&w, &s, 99));
    assert!(!changed_since(&w, &s, 0));
    w.content.insert(
        s.id.clone(),
        keepline_core::content::Content {
            updated: 90,
            ..Default::default()
        },
    );
    assert!(
        !changed_since(&w, &s, 99),
        "index mtime cannot mark an old conversation new"
    );
    assert!(!show_on_return(100, 1900, 30));
    assert!(show_on_return(100, 1901, 30));
    assert!(!show_on_return(0, 1901, 30));
}

#[test]
fn interrupted_terminals_never_enter_the_connection_creation_list() {
    let mut w = fixture();
    assert_eq!(
        attachable_ids(&w, w.tab_terminals("tab")),
        ["t0", "t2", "t3"]
    );
    for terminal in &mut w.terminals {
        terminal.state = ProcessState::Interrupted;
    }
    assert!(attachable_ids(&w, w.tab_terminals("tab")).is_empty());
}

#[test]
fn reopening_full_tab_replaces_exact_slot_and_preserves_history() {
    let mut w = fixture();
    let history = w.agent_sessions.clone();
    let mut replacement = w.terminals[1].clone();
    replacement.id = "new".into();
    replacement.state = ProcessState::Running;
    w.terminals.push(replacement);
    let mut tab = w.tabs[0].clone();
    assert!(tab.layout.as_mut().unwrap().replace("t1", "new"));
    w.edit(Edit::PutTab { tab }).unwrap();
    assert_eq!(w.tab_terminals("tab"), ["t0", "new", "t2", "t3"]);
    assert!(w.terminal_tab("t1").is_none());
    assert_eq!(w.terminals[1].state, ProcessState::Interrupted);
    assert_eq!(w.agent_sessions, history);
}

#[test]
fn closing_interrupted_pane_only_removes_its_placement() {
    let mut w = fixture();
    let history = w.agent_sessions.clone();
    let mut tab = w.tabs[0].clone();
    tab.layout = tab.layout.take().unwrap().without("t1");
    w.edit(Edit::PutTab { tab }).unwrap();
    assert_eq!(w.tab_terminals("tab"), ["t0", "t2", "t3"]);
    assert_eq!(w.terminals.len(), 4);
    assert_eq!(w.agent_sessions, history);
}
