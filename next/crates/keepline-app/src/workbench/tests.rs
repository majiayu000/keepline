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
        binding: None,
        name: String::new(),
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

#[test]
fn live_phase_reads_only_running_keepline_terminals() {
    use keepline_core::content::{Background, Content, Turn};
    let mut w = fixture();
    let mut s = w.agent_sessions[0].clone();
    s.id = "live".into();
    s.terminal = Some("t0".into());
    s.current = true;
    assert_eq!(live_phase(&w, &s), Some(Phase::Idle));
    for (turn, background, phase) in [
        (Turn::Running, None, Phase::Working),
        (Turn::Waiting, None, Phase::Waiting),
        (Turn::Completed, None, Phase::Idle),
        (Turn::Error, None, Phase::Idle),
        (
            Turn::Completed,
            Some(Background {
                agents: 1,
                ..Default::default()
            }),
            Phase::Working,
        ),
        (
            Turn::Completed,
            Some(Background {
                monitors: 1,
                ..Default::default()
            }),
            Phase::Idle,
        ),
    ] {
        w.content.insert(
            "live".into(),
            Content {
                turn,
                background,
                ..Default::default()
            },
        );
        assert_eq!(live_phase(&w, &s), Some(phase), "{turn:?}");
    }
    // An exited terminal, a past conversation or a scanned log keep the inbox grouping.
    s.terminal = Some("t1".into());
    assert_eq!(live_phase(&w, &s), None);
    s.terminal = Some("t0".into());
    s.current = false;
    assert_eq!(live_phase(&w, &s), None);
    s.terminal = None;
    assert_eq!(live_phase(&w, &s), None);
}

#[test]
fn finished_turn_stays_unseen_until_its_pane_is_shown() {
    let mut phases = HashMap::new();
    let mut unseen = HashSet::new();
    let step = |phases: &mut _, unseen: &mut _, phase, shown| {
        track_unseen(phases, unseen, [("s".to_string(), phase, shown)]);
    };
    // Already idle when first seen (e.g. after a restart): nothing new to look at.
    step(&mut phases, &mut unseen, Phase::Idle, false);
    assert!(unseen.is_empty());
    // Ends out of view: done, and stays done across later idle snapshots.
    step(&mut phases, &mut unseen, Phase::Working, false);
    step(&mut phases, &mut unseen, Phase::Idle, false);
    step(&mut phases, &mut unseen, Phase::Idle, false);
    assert!(unseen.contains("s"));
    // Showing the pane marks it seen.
    step(&mut phases, &mut unseen, Phase::Idle, true);
    assert!(unseen.is_empty());
    // Ends in view: seen at once.
    step(&mut phases, &mut unseen, Phase::Working, true);
    step(&mut phases, &mut unseen, Phase::Idle, true);
    assert!(unseen.is_empty());
    // A new turn clears done; a vanished session is forgotten.
    step(&mut phases, &mut unseen, Phase::Waiting, false);
    step(&mut phases, &mut unseen, Phase::Idle, false);
    assert!(unseen.contains("s"));
    step(&mut phases, &mut unseen, Phase::Working, false);
    assert!(unseen.is_empty());
    step(&mut phases, &mut unseen, Phase::Idle, false);
    track_unseen(&mut phases, &mut unseen, []);
    assert!(unseen.is_empty() && phases.is_empty());
}

#[test]
fn sidebar_rows_rank_by_the_shown_state() {
    let rank = sidebar::sidebar_rank;
    // A finished turn you have not seen is colored, so it sits with the others that need you.
    assert!(rank(Group::Done, true) < rank(Group::Working, false));
    assert!(rank(Group::Waiting, true) < rank(Group::Done, true));
    // Live work comes before sessions resting at their prompt.
    assert!(rank(Group::Working, false) < rank(Group::Ready, false));
    assert!(rank(Group::Ready, false) < rank(Group::Stale, false));
}

#[test]
fn reopening_a_failed_resume_returns_to_its_terminal() {
    let mut w = fixture();
    let s = w.agent_sessions[0].clone();
    let mut resumed = HashMap::new();
    assert!(live_terminal(&w, &s).is_none());
    assert!(resume_terminal(&w, &resumed, &s).is_none());
    resumed.insert(s.id.clone(), "t2".into());
    assert_eq!(resume_terminal(&w, &resumed, &s).unwrap().id, "t2");
    // Closed or exited: the next click resumes afresh.
    w.terminals[2].state = ProcessState::Interrupted;
    assert!(resume_terminal(&w, &resumed, &s).is_none());
    w.terminals[2].state = ProcessState::Running;
    w.tabs.clear();
    assert!(resume_terminal(&w, &resumed, &s).is_none());
}

#[test]
fn only_revision_free_edits_wait_behind_a_busy_action() {
    use crate::host::store::queueable;
    let w = fixture();
    let edit = |edit| Request::Edit { revision: 0, edit };
    assert!(queueable(&edit(Edit::CloseTab { id: "tab".into() })));
    assert!(queueable(&edit(Edit::PutTab {
        tab: w.tabs[0].clone()
    })));
    assert!(!queueable(&edit(Edit::SetTaskOutcome {
        id: "task".into(),
        outcome: Outcome::Abandoned,
    })));
    assert!(!queueable(&Request::Workspace));
}
#[test]
fn a_removed_directory_falls_back_to_one_that_exists() {
    let gone = "/nonexistent/keepline-idle-test/next".to_owned();
    let tmp = std::env::temp_dir().to_string_lossy().into_owned();
    assert_eq!(usable_cwd([gone.clone(), tmp.clone()]), tmp);
    assert_eq!(
        usable_cwd([gone.clone(), gone]),
        std::env::var("HOME").unwrap()
    );
}
#[test]
fn close_confirms_only_work_in_progress_or_a_pending_question() {
    let agent = |p| vec![("a".to_owned(), p)];
    assert!(!interrupts(&[]));
    assert!(!interrupts(&agent(Some(Phase::Idle))));
    assert!(!interrupts(&agent(None)));
    assert!(interrupts(&agent(Some(Phase::Working))));
    assert!(interrupts(&agent(Some(Phase::Waiting))));
    let mut mixed = agent(Some(Phase::Idle));
    mixed.push(("b".to_owned(), Some(Phase::Working)));
    assert!(interrupts(&mixed));
}
#[test]
fn closing_a_tab_of_several_splits_counts_them() {
    let mut w = fixture();
    assert_eq!(splits_closed(&w, &Closing::Tab("tab".into())), 4);
    assert_eq!(splits_closed(&w, &Closing::Pane("t0".into())), 0);
    assert_eq!(splits_closed(&w, &Closing::Host), 0);
    w.tabs[0].layout = Some(Layout::Leaf("t0".into()));
    assert_eq!(splits_closed(&w, &Closing::Tab("tab".into())), 0);
}
#[test]
fn closed_sessions_past_the_stale_days_leave_the_sidebar_count() {
    let mut w = fixture();
    let s = &mut w.agent_sessions[0];
    s.outcome = Outcome::Open;
    s.updated = now();
    assert!(closed_recently(&w, &w.agent_sessions[0]));
    // Quiet longer than the stale days: still in the overview, no longer counted.
    w.agent_sessions[0].updated = 1;
    assert!(!closed_recently(&w, &w.agent_sessions[0]));
    // Settled ones never count.
    w.agent_sessions[0].updated = now();
    w.agent_sessions[0].outcome = Outcome::Completed;
    assert!(!closed_recently(&w, &w.agent_sessions[0]));
}
#[test]
fn a_shown_interrupted_pane_reopens_once() {
    let w = fixture();
    let shown = w.tab_terminals("tab");
    let mut tried = HashSet::new();
    assert_eq!(reopen_candidate(&w, &shown, &tried).as_deref(), Some("t1"));
    // Tried once: a failed launch leaves its reopen button instead of retrying forever.
    tried.insert("t1".to_owned());
    assert_eq!(reopen_candidate(&w, &shown, &tried), None);
    // Only panes in view; another tab's lost shells wait until it shows.
    assert_eq!(reopen_candidate(&w, &shown[2..], &HashSet::new()), None);
}
#[test]
fn a_reopened_pane_resumes_its_latest_open_conversation() {
    let mut w = fixture();
    // Only a finished conversation ran there: reopen a plain shell.
    assert!(interrupted_session(&w, "t1").is_none());
    let mut open = w.agent_sessions[0].clone();
    open.outcome = Outcome::Open;
    let older = AgentSession {
        id: "older".into(),
        updated: 2,
        ..open.clone()
    };
    let latest = AgentSession {
        id: "latest".into(),
        updated: 3,
        ..open.clone()
    };
    let elsewhere = AgentSession {
        id: "elsewhere".into(),
        terminal: Some("t0".into()),
        updated: 9,
        ..open
    };
    w.agent_sessions.extend([older, latest, elsewhere]);
    // Lost before the host kept a record: the latest open conversation there.
    assert_eq!(interrupted_session(&w, "t1").unwrap().id, "latest");
    // The host's record wins, including that nothing but the shell was running.
    w.lost_agents.insert("t1".into(), Some("older".into()));
    assert_eq!(interrupted_session(&w, "t1").unwrap().id, "older");
    w.lost_agents.insert("t1".into(), None);
    assert!(interrupted_session(&w, "t1").is_none());
}
#[test]
fn a_resume_is_the_new_shells_first_command_not_typed_later() {
    assert_eq!(launch_argv("/bin/zsh".into(), None), ["/bin/zsh", "-l"]);
    let command = Agent::Claude.resume_command("it's-1", &[]);
    let argv = launch_argv("/bin/sh".into(), Some(&command));
    assert_eq!(&argv[..4], ["/bin/sh", "-l", "-i", "-c"]);
    // Run it for real with `claude` stubbed: the quoted id arrives intact, then the shell
    // that takes over reads the empty stdin and ends.
    let out = std::process::Command::new(&argv[0])
        .args(&argv[1..3])
        .arg("-c")
        .arg(format!("claude() {{ printf '%s|' \"$@\"; }}; {}", argv[4]))
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "--resume|it's-1|");
}
#[test]
fn the_pane_menu_says_which_signal_set_the_state() {
    let mut w = fixture();
    let lines = state_reasons(&w, "t0", Some("Claude 没保存对话记录"), 1000);
    assert_eq!(lines.len(), 2);
    assert!(lines[1].contains("Claude 没保存对话记录"));
    w.agent_sessions[0].terminal = Some("t0".into());
    w.agent_sessions[0].current = true;
    let c = keepline_core::content::Content {
        turn: keepline_core::content::Turn::Waiting,
        signal: "画面".into(),
        signal_at: 1000 - 120,
        waiting: "Do you want to proceed?（画面上的提示）".into(),
        ..Default::default()
    };
    w.content.insert("history".into(), c);
    let lines = state_reasons(&w, "t0", None, 1000);
    assert_eq!(lines[0], "Codex 对话 cli");
    assert_eq!(lines[1], "现在：等你回答");
    assert_eq!(lines[2], "依据：画面底部认出了提问框；对话记录里没有这一条（2 分钟前）");
    assert!(lines[3].starts_with("在等：Do you want to proceed?"));

    // The rule and the lines it saw, as the daemon wrote them.
    let detail = "规则：approval 审批（codex 内置 2026.10.31.1）\n看到：Do you want to proceed? / ❯ 1. Yes / 2. No";
    w.content.get_mut("history").unwrap().signal_detail = detail.into();
    let lines = state_reasons(&w, "t0", None, 1000);
    assert_eq!(lines[3], "规则：approval 审批（codex 内置 2026.10.31.1）");
    assert_eq!(lines[4], "看到：Do you want to proceed? / ❯ 1. Yes / 2. No");
    assert!(lines[5].starts_with("在等："));

    // A finished turn from the log, and a quiet screen no rule took.
    let c = keepline_core::content::Content {
        turn: keepline_core::content::Turn::Completed,
        signal: "日志".into(),
        signal_at: 1000 - 30,
        signal_detail: "记录：assistant「测试都过了，已经提交。」".into(),
        screen_check: "没认出提问框（底部没有审批或选择题的提示；规则 codex 内置 2026.10.31.1）"
            .into(),
        ..Default::default()
    };
    w.content.insert("history".into(), c);
    let lines = state_reasons(&w, "t0", None, 1000);
    assert_eq!(lines[1], "现在：做完了 / 等你下一步");
    assert_eq!(lines[3], "记录：assistant「测试都过了，已经提交。」");
    assert_eq!(
        lines[4],
        "画面：没认出提问框（底部没有审批或选择题的提示；规则 codex 内置 2026.10.31.1），所以按「做完了 / 等你下一步」显示。"
    );
    for line in &lines {
        println!("{line}");
    }

    // While it runs, a screen with no prompt is not news.
    w.content.get_mut("history").unwrap().turn = keepline_core::content::Turn::Running;
    let lines = state_reasons(&w, "t0", None, 1000);
    assert!(!lines.iter().any(|l| l.starts_with("画面：")), "{lines:?}");
}
