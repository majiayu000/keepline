//! Which transcript belongs to which terminal: log-event subscription, recovery of
//! missed log writes and following an associated session's log.
use super::*;

#[derive(Default)]
pub(super) struct Association {
    pub(super) attempted: std::collections::HashSet<(String, String)>,
    pub(super) inventory: Vec<u32>,
    /// (log, newest prompt) → screen comparisons so far. The TUI may paint the prompt a
    /// little after the log records it, so a few appends may retry; then it waits for a new prompt.
    pub(super) screened: HashMap<(String, String), u8>,
}
impl Association {
    pub(super) fn apply(
        &mut self,
        shared: &Shared,
        index: &crate::log_index::Index,
        transcripts: &[crate::recognition::Transcript],
    ) -> io::Result<()> {
        if transcripts.is_empty() {
            return Ok(());
        }
        let workspace = lock(shared)?.workspace.clone();
        let running: Vec<_> = workspace
            .terminals
            .iter()
            .filter(|t| t.state == ProcessState::Running)
            .filter_map(|t| t.pid)
            .collect();
        let unbound: Vec<_> = transcripts
            .iter()
            .filter(|t| {
                !workspace.agent_sessions.iter().any(|s| {
                    s.current
                        && s.runtime == t.runtime
                        && s.cli_session_id == t.cli_id
                        && s.transcript == t.path
                })
            })
            .collect();
        // A new identity/file (or a changed managed tree) gets one probe. An
        // unresolved manual Claude appending the same file is not a polling clock.
        let needs_probe = !running.is_empty()
            && unbound.iter().any(|t| {
                running != self.inventory
                    || !self.attempted.contains(&(t.path.clone(), t.cli_id.clone()))
            });
        // Known files only advance metadata. Never enumerate processes or query the whole index here.
        {
            let mut host = lock(shared)?;
            let mut next = host.workspace.clone();
            for t in transcripts {
                let known = next
                    .agent_sessions
                    .iter()
                    .any(|s| s.runtime == t.runtime && s.cli_session_id == t.cli_id);
                if !known && index.recent(t.updated) {
                    next.observe_agent(crate::recognition::observation(t, None))
                        .map_err(io::Error::other)?;
                }
            }
            crate::recognition::reconcile(&mut next, transcripts, &[], &HashMap::new())?;
            for t in transcripts {
                if let Some(s) = next
                    .agent_sessions
                    .iter_mut()
                    .find(|s| s.runtime == t.runtime && s.cli_session_id == t.cli_id)
                {
                    if let Some(card) = index.card(&t.path)? {
                        if next.content.get(&s.id).is_some_and(|old| {
                            !card.turn_key.is_empty() && card.turn_key != old.turn_key
                        }) && s.outcome != keepline_core::model::Outcome::Open
                        {
                            next.outcome_history
                                .push(keepline_core::model::OutcomeChange {
                                    session: s.id.clone(),
                                    before: s.outcome.clone(),
                                    after: keepline_core::model::Outcome::Open,
                                    at: card.turn_started,
                                    reason: "新回合".into(),
                                });
                            s.outcome = keepline_core::model::Outcome::Open;
                        }
                        s.updated = s.updated.max(card.updated);
                        next.content.insert(s.id.clone(), card);
                    }
                }
            }
            if next.agent_sessions != host.workspace.agent_sessions
                || next.content != host.workspace.content
            {
                host.commit(next)?;
            }
        }
        if needs_probe {
            self.inventory = running;
            for t in unbound {
                self.attempted.insert((t.path.clone(), t.cli_id.clone()));
            }
            refresh_agents(shared, &index.cached()?)?;
        }
        self.associate_screens(shared, index, transcripts)?;
        Ok(())
    }
    /// The last resort for any CLI: shared-daemon Codex runs its thread outside the PTY, and a
    /// hand-started Claude names no session and closes its transcript, so neither process nor
    /// open-file evidence exists. Bind only when a thread's newest prompt is on exactly one
    /// managed screen of an unbound Agent and that screen shows no other candidate thread.
    pub(super) fn associate_screens(
        &mut self,
        shared: &Shared,
        index: &crate::log_index::Index,
        batch: &[crate::recognition::Transcript],
    ) -> io::Result<()> {
        let (panes, workspace) = {
            let host = lock(shared)?;
            let mut panes = vec![];
            for (pid, (terminal, runtime)) in &host.unbound_pids {
                if host.workspace.current_agent(terminal).is_none() {
                    panes.push((*pid, terminal.clone(), *runtime, None));
                }
            }
            // A session only the screen tied to its pane can move to another thread in the
            // same TUI; one its command line, hook or open file names cannot.
            for (pid, id) in &host.agent_pids {
                if let Some(s) = host.workspace.agent_sessions.iter().find(|s| {
                    &s.id == id
                        && s.current
                        && s.terminal.is_some()
                        && s.binding.is_none_or(|b| b == Binding::Screen)
                }) {
                    panes.push((
                        *pid,
                        s.terminal.clone().unwrap(),
                        s.runtime,
                        Some(s.clone()),
                    ));
                }
            }
            panes.retain(|(_, t, ..)| host.terminals.contains_key(t));
            (
                panes
                    .into_iter()
                    .map(|(pid, t, runtime, s)| (pid, host.terminals[&t].clone(), t, runtime, s))
                    .collect::<Vec<_>>(),
                host.workspace.clone(),
            )
        };
        if panes.is_empty() {
            return Ok(());
        }
        let window = keepline_core::content::now().saturating_sub(600);
        let unbound = |t: &crate::recognition::Transcript| {
            t.updated >= window
                && !workspace
                    .agent_sessions
                    .iter()
                    .any(|s| s.current && s.runtime == t.runtime && s.cli_session_id == t.cli_id)
        };
        // The newest prompt: its text when long enough to be screen evidence, and its time.
        let prompt =
            |t: &crate::recognition::Transcript| -> io::Result<Option<(Option<String>, u64)>> {
                Ok(index
                    .latest_user(&t.path)?
                    .map(|(text, at)| (crate::recognition::screen_key(&text), at)))
            };
        // Only a prompt with comparisons left justifies reading screens.
        let mut fresh = false;
        for t in batch.iter().filter(|t| unbound(t)) {
            if let Some((key, at)) = prompt(t)? {
                let tries = self
                    .screened
                    .entry((t.path.clone(), key.unwrap_or_else(|| at.to_string())))
                    .or_default();
                if *tries < 5 {
                    *tries += 1;
                    fresh = true;
                }
            }
        }
        if !fresh {
            return Ok(());
        }
        let cached = index.cached()?;
        let mut threads = vec![];
        for t in cached.iter().filter(|t| unbound(t)) {
            if let Some((key, at)) = prompt(t)? {
                let cwd = fs::canonicalize(&t.cwd)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| t.cwd.clone());
                threads.push(crate::recognition::ScreenThread {
                    transcript: t,
                    key,
                    at: Some(at),
                    cwd,
                });
            }
        }
        let submits = lock(shared)?.submits.clone();
        let mut terminals = vec![];
        for (pid, live, terminal, runtime, current) in &panes {
            let screen = lock(live)?.vt.tail_text(200)?;
            let current = match current {
                Some(s) => Some((
                    s.cli_session_id.clone(),
                    s.updated,
                    index
                        .latest_user_text(&s.transcript)?
                        .and_then(|p| crate::recognition::screen_key(&p)),
                )),
                None => None,
            };
            terminals.push(crate::recognition::ScreenTerminal {
                terminal: terminal.clone(),
                runtime: *runtime,
                cwd: crate::recognition::process_cwd(*pid),
                screen: crate::recognition::screen_text(&screen),
                current,
                submits: submits
                    .get(terminal)
                    .map(|s| s.iter().copied().collect())
                    .unwrap_or_default(),
            });
        }
        let bindings = crate::recognition::screen_bindings(&threads, &terminals);
        if bindings.is_empty() {
            return Ok(());
        }
        let mut host = lock(shared)?;
        let mut next = host.workspace.clone();
        let mut bound = vec![];
        for (terminal, t) in bindings {
            if next
                .observe_agent(crate::recognition::bound(
                    t,
                    terminal.clone(),
                    Binding::Screen,
                ))
                .is_ok()
            {
                bound.push(terminal);
            }
        }
        if next.agent_sessions == host.workspace.agent_sessions {
            return Ok(());
        }
        host.commit(next)?;
        // The TUI pid now stands for the bound session: its exit clears `current`.
        for terminal in bound {
            let Some(id) = host
                .workspace
                .current_agent(&terminal)
                .map(|s| s.id.clone())
            else {
                continue;
            };
            let pids: Vec<_> = panes
                .iter()
                .filter(|(_, _, t, ..)| t == &terminal)
                .map(|(pid, ..)| *pid)
                .collect();
            for pid in pids {
                host.unbound_pids.remove(&pid);
                host.agent_pids.insert(pid, id.clone());
            }
            host.unhint(&terminal);
        }
        host.publish_status()
    }
}
pub(super) fn recover_logs(
    shared: &Shared,
    index: &mut crate::log_index::Index,
    association: &mut Association,
    scopes: &[std::path::PathBuf],
) -> io::Result<()> {
    if scopes.is_empty() {
        return Ok(());
    }
    // Persist scope before repairing: restart resumes only this unfinished subtree.
    index.queue_recovery(scopes)?;
    publish_recognition(
        shared,
        index,
        RecognitionPhase::Interrupted,
        "识别中断：文件事件不完整",
        scopes,
    )?;
    publish_recognition(
        shared,
        index,
        RecognitionPhase::Recovering,
        "识别中断，正在补扫",
        scopes,
    )?;
    let mut recovery = crate::log_index::Recovery::new(index, scopes);
    loop {
        let (done, transcripts) = recovery.step(index)?;
        if let Err(error) = association.apply(shared, index, &transcripts) {
            eprintln!("Agent association deferred after subtree repair: {error}");
        }
        if done {
            break;
        }
        thread::yield_now();
    }
    index.finish_recovery(scopes)?;
    if scopes.iter().any(|p| !p.exists()) {
        publish_recognition(
            shared,
            index,
            RecognitionPhase::Interrupted,
            "日志目录暂不可用，等待重建事件",
            scopes,
        )
    } else {
        publish_recognition(shared, index, RecognitionPhase::Healthy, "识别正常", &[])
    }
}
pub(super) fn event_scopes(
    batch: &crate::file_events::Batch,
    roots: &[std::path::PathBuf],
    scopes: &mut Vec<std::path::PathBuf>,
) -> bool {
    let mut restart = false;
    for event in &batch.events {
        if event.flags & 0x8 != 0 {
            // IDs wrapped: the journal cannot identify a narrower lost range.
            for root in roots {
                crate::file_events::add_scope(scopes, root, roots);
            }
            restart = true;
        } else if event.flags & 0x27 != 0 {
            let path = if event.path.extension().is_some_and(|e| e == "jsonl") {
                event.path.parent().unwrap_or(&event.path)
            } else {
                &event.path
            };
            crate::file_events::add_scope(scopes, path, roots);
            restart |= event.flags & 0x20 != 0;
        } else if event.flags & 0x20000 != 0 && event.flags & (0x100 | 0x800) != 0 {
            // A new/renamed populated directory may produce only a directory event.
            crate::file_events::add_scope(scopes, &event.path, roots);
        }
    }
    restart
}
// Bootstrap can establish ownership without any queued FSEvents batch. Install
// vnode listeners before the first blocking receive, too: open writers need not
// close their files to deliver FSEvents. Register then read closes the append race.
pub(super) fn follow_associated(
    shared: &Shared,
    index: &mut crate::log_index::Index,
    association: &mut Association,
    watch: &crate::file_events::Watch,
) -> io::Result<()> {
    let (mut associated, unbound) = {
        let host = lock(shared)?;
        let associated = host
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| s.current)
            .map(|s| std::path::PathBuf::from(&s.transcript))
            .collect::<Vec<_>>();
        (
            associated,
            host.unbound_pids.keys().copied().collect::<Vec<_>>(),
        )
    };
    // A shared daemon (Codex's) keeps an unbound thread's log open, so FSEvents would hold
    // its prompt back until exit and screen association would never see it. While a managed
    // Agent is unbound, also follow recent unassociated logs of such Agents from its cwd.
    if !unbound.is_empty() {
        let cwds: Vec<_> = unbound
            .iter()
            .filter_map(|pid| crate::recognition::process_cwd(*pid))
            .collect();
        let window = keepline_core::content::now().saturating_sub(600);
        let mut candidates: Vec<_> = index
            .cached()?
            .into_iter()
            .filter(|t| {
                crate::agents::adapter(t.runtime).outside_pty()
                    && t.updated >= window
                    && !associated.iter().any(|p| p.as_os_str() == t.path.as_str())
                    && fs::canonicalize(&t.cwd)
                        .is_ok_and(|c| cwds.iter().any(|cwd| c.as_os_str() == cwd.as_str()))
            })
            .collect();
        candidates.sort_by_key(|t| std::cmp::Reverse(t.updated));
        associated.extend(
            candidates
                .into_iter()
                .take(16)
                .map(|t| std::path::PathBuf::from(t.path)),
        );
    }
    // The same holds for a bound session's subagents: their logs stay open too.
    let parents: Vec<String> = {
        let host = lock(shared)?;
        host.workspace
            .agent_sessions
            .iter()
            .filter(|s| s.current && crate::agents::adapter(s.runtime).links_children())
            .map(|s| s.cli_session_id.clone())
            .collect()
    };
    if !parents.is_empty() {
        associated.extend(
            index
                .child_logs(&parents, 32)?
                .into_iter()
                .map(std::path::PathBuf::from),
        );
    }
    // Register first, then close the read/register race using the existing cursor.
    for path in watch.follow(&associated)? {
        if let Some(runtime) = index.runtime(&path) {
            if let Some(t) = index.update(runtime, &path, true)? {
                let mut found = vec![t];
                found.extend(index.take_touched());
                association.apply(shared, index, &found)?;
            }
        }
    }
    Ok(())
}
pub(super) fn start_subscription(shared: Shared, mut index: crate::log_index::Index) {
    watch_registries(&shared, &index.roots);
    thread::spawn(move || {
        // Any error restarts the subscription from the durable checkpoint, like a daemon
        // restart would; `initialize` clears the stored error once it is running again.
        let mut retry = SUBSCRIPTION_RETRY_MIN;
        loop {
            let started = Instant::now();
            let result = (|| -> io::Result<()> {
                let mut checkpoint = index.initialize()?;
                let roots = index
                    .roots
                    .iter()
                    .map(|(_, p)| p.clone())
                    .collect::<Vec<_>>();
                let mut watch = crate::file_events::Watch::start(&roots, checkpoint)?;
                lock(&shared)?.log_events = Some(watch.notifier());
                index.bootstrap()?;
                publish_recognition(&shared, &index, RecognitionPhase::Healthy, "识别正常", &[])?;
                let mut association = Association::default();
                association.apply(&shared, &index, &index.cached()?)?;
                follow_associated(&shared, &mut index, &mut association, &watch)?;
                loop {
                    let mut batch = watch.rx.recv().map_err(io::Error::other)?;
                    // A single bounded burst window starts ONLY after a native event arrives.
                    // This matches FSEvents' 200 ms latency for eager vnode writes too. It never
                    // wakes an idle subscriber, and continuous writers cannot extend the deadline.
                    let deadline = std::time::Instant::now() + Duration::from_millis(200);
                    for _ in 0..63 {
                        match watch.rx.recv_timeout(
                            deadline.saturating_duration_since(std::time::Instant::now()),
                        ) {
                            Ok(next) => batch.events.extend(next.events),
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                            Err(e) => return Err(io::Error::other(e)),
                        }
                    }
                    let mut scopes = watch.take_overflow();
                    let overflowed = !scopes.is_empty();
                    let restart = event_scopes(&batch, &roots, &mut scopes) || overflowed;
                    let wrapped = batch.events.iter().any(|e| e.flags & 0x8 != 0);
                    if restart {
                        // Watch before repairing. Replay from the last committed checkpoint, not 'now',
                        // so restarting one stream cannot discard queued events for the other CLI root.
                        if wrapped {
                            checkpoint = crate::file_events::Watch::current_id();
                            index.reset_checkpoint(checkpoint)?;
                        }
                        watch = crate::file_events::Watch::start(&roots, checkpoint)?;
                        lock(&shared)?.log_events = Some(watch.notifier());
                    }
                    if !scopes.is_empty() {
                        if let Err(error) =
                            recover_logs(&shared, &mut index, &mut association, &scopes)
                        {
                            index.fail(&error)?;
                            publish_recognition(
                                &shared,
                                &index,
                                RecognitionPhase::Interrupted,
                                &format!("补扫未完成：{error}；等待后续文件事件重试"),
                                &scopes,
                            )?;
                            // Leave the durable scope worklist. No busy retry or permanent error latch.
                            continue;
                        }
                    }
                    let mut paths = std::collections::BTreeSet::new();
                    let mut event_id = checkpoint;
                    for event in batch.events {
                        if !wrapped {
                            event_id = event_id.max(event.id);
                        }
                        if event.flags & 0x10 == 0
                            && event.path.extension().is_some_and(|e| e == "jsonl")
                        {
                            paths.insert(event.path);
                        }
                    }
                    let transcripts = index.update_many(paths, event_id)?;
                    if let Err(error) = association.apply(&shared, &index, &transcripts) {
                        eprintln!("Agent association deferred: {error}");
                    }
                    follow_associated(&shared, &mut index, &mut association, &watch)?;
                    // Only a real event drives retries of an earlier incomplete subtree.
                    let pending = index.recovery_scopes()?;
                    if !pending.is_empty() {
                        if let Err(error) =
                            recover_logs(&shared, &mut index, &mut association, &pending)
                        {
                            index.fail(&error)?;
                            publish_recognition(
                                &shared,
                                &index,
                                RecognitionPhase::Interrupted,
                                &format!("补扫未完成：{error}"),
                                &pending,
                            )?;
                            continue;
                        }
                    }
                    if event_id > checkpoint {
                        // Checkpoint is atomic with this batch of content/cursor writes.
                        checkpoint = event_id;
                    }
                    let status = lock(&shared)?.recognition.clone();
                    publish_recognition(
                        &shared,
                        &index,
                        status.phase,
                        &status.detail,
                        &status
                            .scopes
                            .iter()
                            .map(std::path::PathBuf::from)
                            .collect::<Vec<_>>(),
                    )?;
                }
            })();
            let Err(error) = result else { return };
            if started.elapsed() > SUBSCRIPTION_RETRY_MAX {
                retry = SUBSCRIPTION_RETRY_MIN;
            }
            if let Err(persist) = index.fail(&error) {
                eprintln!("persist subscription failure: {persist}");
            }
            let _ = publish_recognition(
                &shared,
                &index,
                RecognitionPhase::Interrupted,
                &format!("识别中断：{error}；{} 秒后自动重试", retry.as_secs()),
                &[],
            );
            eprintln!("log subscription failed, retrying in {retry:?}: {error}");
            thread::sleep(retry);
            retry = (retry * 2).min(SUBSCRIPTION_RETRY_MAX);
        }
    });
}

/// Backoff for restarting a failed log subscription or registry watch.
pub(super) const SUBSCRIPTION_RETRY_MIN: Duration = Duration::from_secs(1);
pub(super) const SUBSCRIPTION_RETRY_MAX: Duration = Duration::from_secs(60);
