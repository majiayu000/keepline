//! What each agent is doing beyond its log: recognition status pushed to viewers,
//! process refresh, registry watch, quiet-screen questions, official status and the
//! probe after a submission.
use super::*;

pub(super) fn recognition_status(host: &Host) -> RecognitionStatus {
    let mut status = host.recognition.clone();
    let (ps, lsof) = crate::recognition::probe_counts();
    status.metrics.ps_spawns = ps;
    status.metrics.lsof_spawns = lsof;
    status
}
pub(super) fn publish_recognition(
    shared: &Shared,
    index: &crate::log_index::Index,
    phase: RecognitionPhase,
    detail: &str,
    scopes: &[std::path::PathBuf],
) -> io::Result<()> {
    let mut host = lock(shared)?;
    let scope_names = scopes
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let changed = host.recognition.phase != phase
        || host.recognition.detail != detail
        || host.recognition.scopes != scope_names;
    host.recognition.phase = phase;
    host.recognition.detail = detail.into();
    host.recognition.scopes = scope_names;
    host.recognition.metrics.directories = index.counts.directories;
    host.recognition.metrics.log_opens = index.counts.opens;
    host.recognition.metrics.log_bytes = index.counts.bytes;
    host.recognition.metrics.index_writes = index.counts.writes;
    if changed {
        let payload = Arc::new(
            serde_json::to_vec(&Reply::Recognition(recognition_status(&host)))
                .map_err(io::Error::other)?,
        );
        host.recognition_subscribers.retain(|_, subscriber| {
            if subscriber.tx.try_send((CONTROL, payload.clone())).is_err() {
                let _ = subscriber.socket.shutdown(std::net::Shutdown::Both);
                false
            } else {
                true
            }
        });
    }
    Ok(())
}
pub(super) fn watch_recognition(
    mut stream: UnixStream,
    shared: Shared,
    connection: u64,
) -> io::Result<()> {
    let (tx, rx) = sync_channel::<Frame>(16);
    {
        let mut host = lock(&shared)?;
        let payload = Arc::new(
            serde_json::to_vec(&Reply::Recognition(recognition_status(&host)))
                .map_err(io::Error::other)?,
        );
        tx.send((CONTROL, payload)).map_err(io::Error::other)?;
        host.recognition_subscribers.insert(
            connection,
            Subscriber {
                tx,
                socket: stream.try_clone()?,
                lagged: Default::default(),
            },
        );
    }
    let mut writer = stream.try_clone()?;
    thread::spawn(move || {
        for (kind, bytes) in rx {
            if write_frame(&mut writer, kind, &bytes).is_err() {
                break;
            }
        }
        let _ = writer.shutdown(std::net::Shutdown::Both);
    });
    // A blocked read observes client disconnect without a heartbeat/timer.
    let result = read_frame(&mut stream);
    lock(&shared)?.recognition_subscribers.remove(&connection);
    let _ = stream.shutdown(std::net::Shutdown::Both);
    result.map(|_| ())
}

pub(super) fn refresh_agents(
    shared: &Shared,
    transcripts: &[crate::recognition::Transcript],
) -> io::Result<()> {
    let workspace = lock(shared)?.workspace.clone();
    let mut processes = crate::recognition::processes(&workspace)?;
    crate::recognition::name_by_pid(&workspace, &mut processes);
    let files = crate::recognition::open_transcripts(&workspace, &processes)?;
    let mut host = lock(shared)?;
    let mut next = host.workspace.clone();
    crate::recognition::reconcile(&mut next, transcripts, &processes, &files)?;
    crate::recognition::record_launch(&mut next, &processes);
    if next.agent_sessions != host.workspace.agent_sessions
        || next.agent_launch != host.workspace.agent_launch
    {
        host.commit(next)?;
    }
    host.observe_processes(&processes, &files)?;
    Ok(())
}
/// A process found before its CLI registered it stays unidentified until something else
/// changes; the registry changing is that moment. Event-driven, like the log watch.
pub(super) fn watch_registries(shared: &Shared, roots: &[(Agent, std::path::PathBuf)]) {
    let paths: Vec<_> = roots
        .iter()
        .filter_map(|(agent, root)| crate::agents::adapter(*agent).registry(root))
        .collect();
    if paths.is_empty() {
        return;
    }
    let shared = shared.clone();
    thread::spawn(move || {
        let mut retry = SUBSCRIPTION_RETRY_MIN;
        loop {
            let started = Instant::now();
            let result = (|| -> io::Result<()> {
                let watch = crate::file_events::Watch::start(
                    &paths,
                    crate::file_events::Watch::current_id(),
                )?;
                loop {
                    watch.rx.recv().map_err(io::Error::other)?;
                    // One identification per burst; a continuous writer cannot extend it.
                    let deadline = std::time::Instant::now() + Duration::from_millis(200);
                    for _ in 0..63 {
                        let left = deadline.saturating_duration_since(std::time::Instant::now());
                        if watch.rx.recv_timeout(left).is_err() {
                            break;
                        }
                    }
                    // A failed identification waits for the next registry change; only a
                    // broken watch restarts this thread's loop.
                    if let Err(error) = refresh_registered(&shared) {
                        eprintln!("Agent registry refresh failed: {error}");
                    }
                }
            })();
            let Err(error) = result else { return };
            if started.elapsed() > SUBSCRIPTION_RETRY_MAX {
                retry = SUBSCRIPTION_RETRY_MIN;
            }
            eprintln!("Agent registry watch stopped, retrying in {retry:?}: {error}");
            thread::sleep(retry);
            retry = (retry * 2).min(SUBSCRIPTION_RETRY_MAX);
        }
    });
}
pub(super) fn refresh_registered(shared: &Shared) -> io::Result<()> {
    let transcripts = {
        let host = lock(shared)?;
        if host
            .workspace
            .terminals
            .iter()
            .all(|t| t.state != ProcessState::Running)
        {
            return Ok(());
        }
        host.store.transcripts()?
    };
    refresh_agents(shared, &transcripts)
}
pub(super) const SCREEN_QUIET: Duration = Duration::from_secs(1);
/// A fresh hook-reported dialog is not vetoed by the screen yet: it may still be drawing.
pub(super) const RECONCILE_HOLD: Duration = Duration::from_secs(5);
/// At most one `claude agents --json` per session in this window.
pub(super) const RECONCILE_EVERY: Duration = Duration::from_secs(5);
/// Menus the user opens appear right after a keystroke; an agent's question does not.
pub(super) const INPUT_SETTLE: Duration = Duration::from_secs(3);

/// Returns when to look again, if the user typed too recently to attribute the prompt.
pub(super) fn check_screen_wait(
    terminal: &LiveTerminal,
    shared: &Shared,
    id: &str,
) -> io::Result<Option<Instant>> {
    // Each CLI's own screen rules; one with no Agent yet is read with the default ones.
    let agent = {
        let host = lock(shared)?;
        let agent = host.workspace.current_agent(id).map(|s| s.runtime);
        if agent.is_none() && !host.screen_waits.get(id).is_some_and(|e| e.waiting()) {
            return Ok(None);
        }
        agent
    };
    let rules = agent.map_or_else(
        crate::screen_rules::default_rules,
        crate::screen_rules::rules,
    );
    let (tail, input_at) = {
        let live = lock(terminal)?;
        if live.exited {
            return Ok(None);
        }
        (live.vt.tail_text(usize::from(live.rows))?, live.input_at)
    };
    let seen = crate::screen_rules::explain(rules, &tail);
    let settle = input_at.map(|at| at + INPUT_SETTLE);
    if let Some(settle) = settle.filter(|at| seen.waiting() && *at > Instant::now()) {
        return Ok(Some(settle));
    }
    let mut host = lock(shared)?;
    let seen = host.workspace.current_agent(id).is_some().then_some(seen);
    let veto = !seen.as_ref().is_some_and(|e| e.waiting());
    host.set_screen_wait(id, seen)?;
    if !veto {
        return Ok(None);
    }
    // A dialog the hooks or the log reported but the screen no longer shows: ask the CLI.
    let Some(session) = host.workspace.current_agent(id).cloned() else {
        return Ok(None);
    };
    let waiting = host
        .workspace
        .content
        .get(&session.id)
        .filter(|c| c.turn == keepline_core::content::Turn::Waiting);
    let Some(card) = waiting else {
        return Ok(None);
    };
    let held = card.signal_at + RECONCILE_HOLD.as_secs();
    let now = keepline_core::content::now();
    if held > now {
        return Ok(Some(Instant::now() + Duration::from_secs(held - now)));
    }
    Ok(host.reconcile(shared, vec![session]))
}

pub(super) fn apply_official(
    shared: &Shared,
    index: &crate::log_index::Index,
    due: &[(AgentSession, Option<u32>)],
) -> io::Result<()> {
    // Only CLIs whose adapter has an official status ask; the others rely on logs and screens.
    let mut found: Vec<(&AgentSession, crate::content::Official)> = Vec::new();
    let mut errors = Vec::new();
    // Each CLI's last outcome, shown in the app's recognition notice until one succeeds.
    let mut outcomes: Vec<(&str, Option<String>)> = Vec::new();
    for agent in Agent::all() {
        let mine: Vec<_> = due.iter().filter(|(s, _)| s.runtime == agent).collect();
        if mine.is_empty() {
            continue;
        }
        let Some((asked, result)) = crate::agents::adapter(agent).official(&mine) else {
            continue;
        };
        match result {
            Ok(statuses) => {
                found.extend(
                    statuses
                        .into_iter()
                        .filter_map(|(i, status)| Some((&mine.get(i).copied()?.0, status))),
                );
                outcomes.push((agent.key(), None));
            }
            Err(e) => {
                errors.push(format!("{asked}: {e}"));
                outcomes.push((agent.key(), Some(e.to_string())));
            }
        }
    }
    {
        let mut host = lock(shared)?;
        let before = host.recognition.official.clone();
        for (cli, error) in outcomes {
            match error {
                Some(e) => host.recognition.official.insert(
                    cli.into(),
                    format!("{cli} 官方状态查询不可用，已改为只看日志和画面：{e}"),
                ),
                None => host.recognition.official.remove(cli),
            };
        }
        if host.recognition.official != before {
            host.publish_status()?;
        }
    }
    let mut cards = Vec::new();
    for (session, status) in found {
        index.official(&session.transcript, &status)?;
        if let Some(card) = index.card(&session.transcript)? {
            cards.push((session.id.clone(), card));
        }
    }
    if !cards.is_empty() {
        let mut host = lock(shared)?;
        let mut next = host.workspace.clone();
        for (id, card) in cards {
            next.content.insert(id, card);
        }
        host.commit(next)?;
    }
    match errors.is_empty() {
        true => Ok(()),
        false => Err(io::Error::other(errors.join("; "))),
    }
}

/// An Enter reached this terminal: remember when (screen recognition pairs a new
/// conversation with the latest ones) and look for the Agent it may have started.
pub(super) fn record_submission(shared: &Shared, terminal: &str) -> io::Result<()> {
    {
        let mut host = lock(shared)?;
        let submits = host.submits.entry(terminal.to_owned()).or_default();
        submits.push_back(keepline_core::content::now());
        if submits.len() > 8 {
            submits.pop_front();
        }
    }
    probe_after_submission(shared, terminal)
}
pub(super) fn probe_after_submission(shared: &Shared, terminal: &str) -> io::Result<()> {
    let mut host = lock(shared)?;
    if host.workspace.current_agent(terminal).is_some()
        || !host.input_probes.insert(terminal.into())
    {
        return Ok(());
    }
    let shared = shared.clone();
    let terminal = terminal.to_owned();
    thread::spawn(move || {
        // One startup grace after user Enter, not periodic process or filesystem polling.
        thread::sleep(Duration::from_millis(500));
        let result = (|| -> io::Result<()> {
            let transcripts = {
                let mut host = lock(&shared)?;
                host.input_probes.remove(&terminal);
                if host.workspace.current_agent(&terminal).is_some() {
                    return Ok(());
                }
                host.store.transcripts()?
            };
            refresh_agents(&shared, &transcripts)?;
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("Agent startup identification: {e}");
        }
    });
    Ok(())
}
