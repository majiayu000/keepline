//! One client connection: request dispatch, typed input and handing the socket to a
//! newer host.
use super::*;

pub(super) fn serve_client(
    mut stream: UnixStream,
    host: Shared,
    connection: u64,
) -> io::Result<()> {
    let mut uid = 0;
    let mut gid = 0;
    if unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } != 0
        || uid != unsafe { libc::geteuid() }
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "peer uid mismatch",
        ));
    }
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    match receive::<Request>(&mut stream)? {
        Request::Hello { major: MAJOR } => send(
            &mut stream,
            CONTROL,
            &Reply::Hello {
                major: MAJOR,
                instance: lock(&host)?.instance.clone(),
                build: BUILD.clone(),
            },
        )?,
        _ => {
            send(
                &mut stream,
                CONTROL,
                &Reply::Error(format!("protocol major {MAJOR} required")),
            )?;
            return Ok(());
        }
    }
    loop {
        let request = receive::<Request>(&mut stream)?;
        if matches!(request, Request::WatchRecognition) {
            return watch_recognition(stream, host, connection);
        }
        if matches!(request, Request::Diagnose) {
            let terminals = lock(&host)?
                .workspace
                .terminals
                .iter()
                .filter(|t| t.state == ProcessState::Running)
                .cloned()
                .collect::<Vec<_>>();
            // No workspace/terminal lock is held during process enumeration or waiting.
            if let Err(error) = crate::diagnostics::serve(&mut stream, &terminals) {
                send(&mut stream, CONTROL, &Reply::Error(error.to_string()))?;
            }
            return Ok(());
        }
        if matches!(request, Request::Handoff) {
            return hand_over(stream, &host);
        }
        if let Request::Attach { id, takeover } = request {
            let terminal = lock(&host)?.terminals.get(&id).cloned();
            match terminal {
                Some(terminal) => return attach(stream, terminal, connection, takeover, host, id),
                None => send(
                    &mut stream,
                    CONTROL,
                    &Reply::Error("terminal is not available in this daemon instance".into()),
                )?,
            }
            continue;
        }
        let result = dispatch(&host, request);
        let reply = result.unwrap_or_else(|e| Reply::Error(e.to_string()));
        send(&mut stream, CONTROL, &reply)?;
    }
}

/// A newer host takes over: every running terminal goes to it as a snapshot plus its PTY
/// descriptor, and this host exits without signalling the programs. Global and terminal
/// locks stay held to the end, so no output is fed, no state is committed and no
/// terminal is started after the snapshots are taken.
pub(super) fn hand_over(mut stream: UnixStream, shared: &Shared) -> io::Result<()> {
    let host = lock(shared)?;
    let mut held = vec![];
    for (id, terminal) in &host.terminals {
        let live = lock(terminal)?;
        if !live.exited {
            held.push((id.clone(), live));
        }
    }
    let terminals: Vec<_> = held
        .iter()
        .map(|(id, live)| Handed {
            id: id.clone(),
            pid: live.pid,
            cols: live.cols,
            rows: live.rows,
            title: live.title.clone(),
            cell: live.cell.get(),
        })
        .collect();
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    send(&mut stream, CONTROL, &Reply::Handoff { terminals })?;
    for (_, live) in &mut held {
        for chunk in live.vt.snapshot()?.chunks(MAX_FRAME - 1) {
            write_frame(&mut stream, SNAPSHOT, chunk)?;
        }
        write_frame(&mut stream, READY, &[])?;
    }
    let fds: Vec<_> = held
        .iter()
        .map(|(_, live)| live.master.as_raw_fd())
        .collect();
    crate::handoff::send_fds(&stream, &fds)?;
    // Only a confirmed takeover lets this host go: closing our copies before the new host
    // holds its own would hang up every terminal. Anything else keeps serving.
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    match read_frame(&mut stream) {
        Ok((ACK, taken)) if taken == HANDOFF_TAKEN => {
            eprintln!("handed {} terminals to a newer host; exiting", fds.len());
            std::process::exit(0);
        }
        other => Err(io::Error::other(format!(
            "handoff not confirmed, still serving: {other:?}"
        ))),
    }
}
/// What a new host answers once it holds every handed-over descriptor.
pub(super) const HANDOFF_TAKEN: &[u8] = b"taken";

pub(super) fn dispatch(shared: &Shared, request: Request) -> io::Result<Reply> {
    match request {
        Request::RecognitionStatus => Ok(Reply::Recognition(recognition_status(&*lock(shared)?))),
        Request::RefreshAgents => {
            let transcripts = {
                let host = lock(shared)?;
                if let Some(error) = host.store.subscription_error()? {
                    return Err(io::Error::other(error));
                }
                host.store.transcripts()?
            };
            refresh_agents(shared, &transcripts)?;
            let mut host = lock(shared)?;
            let waiting: Vec<_> = host
                .workspace
                .agent_sessions
                .iter()
                .filter(|s| s.current)
                .filter(|s| {
                    host.workspace
                        .content
                        .get(&s.id)
                        .is_some_and(|c| c.turn == keepline_core::content::Turn::Waiting)
                })
                .cloned()
                .collect();
            if !waiting.is_empty() {
                host.reconcile(shared, waiting);
            }
            Ok(Reply::Workspace(host.snapshot()))
        }
        Request::Hook {
            runtime,
            pid,
            cli_session_id,
            transcript,
            ended,
            event,
            facts,
            terminal,
            cwd,
        } => {
            let (workspace, mut index, cached) = {
                let host = lock(shared)?;
                (
                    host.workspace.clone(),
                    host.index.fork_current()?,
                    host.store.transcripts()?,
                )
            };
            let processes = crate::recognition::processes(&workspace)?;
            // Reject unrelated callers before opening any supplied transcript. A hook whose
            // caller already exited still names its terminal through the environment.
            let named = terminal.as_deref().filter(|id| {
                workspace
                    .terminals
                    .iter()
                    .any(|t| &t.id == id && t.state == ProcessState::Running)
            });
            if crate::recognition::terminal_for_process(pid, &processes, &workspace).is_none()
                && named.is_none()
            {
                return Err(io::Error::other(
                    "Hook process is not a descendant of a managed terminal",
                ));
            }
            let path = if transcript.is_empty() {
                let matches: Vec<_> = cached
                    .iter()
                    .filter(|t| t.runtime == runtime && t.cli_id == cli_session_id)
                    .collect();
                if matches.len() != 1 {
                    return Err(io::Error::other(
                        "Hook transcript absent or ambiguous in index",
                    ));
                }
                std::path::PathBuf::from(&matches[0].path)
            } else {
                crate::file_events::normalized(Path::new(&transcript))?
            };
            // A new Claude announces itself before its first prompt creates the transcript:
            // the hook's own identity binds it now, and the index fills it in once written.
            let written = path.exists();
            let observed = if written {
                index
                    .update(runtime, &path, true)?
                    .ok_or_else(|| io::Error::other("transcript has no CLI identity"))?
            } else if !cwd.is_empty() && !transcript.is_empty() {
                crate::recognition::Transcript {
                    runtime,
                    cli_id: cli_session_id.clone(),
                    path: path.to_string_lossy().into_owned(),
                    cwd: cwd.clone(),
                    updated: keepline_core::content::now(),
                }
            } else {
                return Err(io::Error::other("Hook transcript not written yet"));
            };
            let mut host = lock(shared)?;
            let mut next = host.workspace.clone();
            crate::recognition::apply_hook(
                &mut next,
                runtime,
                pid,
                named,
                &cli_session_id,
                &observed,
                &processes,
                ended,
            )?;
            if let Some(event) = event.filter(|_| written) {
                index.hook(&observed.path, &event, &facts)?;
            }
            if let Some(s) = next
                .agent_sessions
                .iter()
                .find(|s| s.runtime == runtime && s.cli_session_id == cli_session_id)
            {
                if let Some(card) = index.card(&observed.path)? {
                    next.content.insert(s.id.clone(), card);
                }
            }
            host.commit(next)?;
            let mut ancestor = pid;
            let mut hook_files = HashMap::new();
            while let Some(process) = processes.iter().find(|p| p.pid == ancestor) {
                if process.runtime == Some(runtime) {
                    hook_files.insert(ancestor, vec![observed.path.clone()]);
                    break;
                }
                if ancestor == process.parent {
                    break;
                }
                ancestor = process.parent;
            }
            // A hook run in the background has left its CLI's process tree; the terminal
            // it names then has to hold exactly one such CLI for its exit to be watched.
            if hook_files.is_empty() && !ended {
                let mut runs = processes.iter().filter(|p| {
                    p.runtime == Some(runtime)
                        && !processes
                            .iter()
                            .any(|c| c.parent == p.pid && c.runtime == p.runtime)
                        && named.is_some_and(|named| {
                            crate::recognition::terminal_for_process(p.pid, &processes, &workspace)
                                .as_deref()
                                == Some(named)
                        })
                });
                if let (Some(only), None) = (runs.next(), runs.next()) {
                    hook_files.insert(only.pid, vec![observed.path.clone()]);
                }
            }
            host.observe_processes(&processes, &hook_files)?;
            if let Some(events) = &host.log_events {
                events.file(path);
            }
            Ok(Reply::Workspace(host.snapshot()))
        }
        Request::ClaimNotification { session, key } => {
            let host = lock(shared)?;
            if !host.workspace.settings.notifications
                || !host
                    .workspace
                    .agent_sessions
                    .iter()
                    .any(|s| s.id == session)
            {
                return Ok(Reply::NotificationClaimed(false));
            }
            Ok(Reply::NotificationClaimed(
                host.store.claim_notification(&session, &key)?,
            ))
        }
        Request::ClientLeft { at } => {
            let mut host = lock(shared)?;
            let mut next = host.workspace.clone();
            next.last_left = at;
            host.commit(next)?;
            Ok(Reply::Ok)
        }
        Request::Workspace => Ok(Reply::Workspace(lock(shared)?.snapshot())),
        Request::Edit { revision, mut edit } => {
            let mut host = lock(shared)?;
            if edit.checks_revision() && host.workspace.revision != revision {
                return Err(io::Error::other("工作台已变化，请刷新后重试"));
            }
            if let Edit::SaveSessionTask { session, task } = &mut edit {
                let s = host
                    .workspace
                    .agent_sessions
                    .iter()
                    .find(|s| &s.id == session)
                    .ok_or_else(|| io::Error::other("会话不存在"))?;
                task.description = host.index.original_words(&s.transcript)?;
                if task.description.is_empty() {
                    return Err(io::Error::other("已索引内容没有用户原话，不能建立待办"));
                }
                if host
                    .workspace
                    .content
                    .get(session)
                    .is_some_and(|c| c.partial_history)
                {
                    task.description
                        .push_str("\n\n（历史仅覆盖索引尾段及之后的增量；此前要求未知。）");
                }
            }
            // Closing a tab or a pane ends what ran in it; placement alone never does.
            let stopped = match &edit {
                Edit::CloseTab { id } => host.workspace.tab_terminals(id),
                Edit::ClosePane { terminal } => vec![terminal.clone()],
                Edit::RemoveSpace { id } => host
                    .workspace
                    .tabs
                    .iter()
                    .filter(|t| &t.space == id)
                    .flat_map(|t| host.workspace.tab_terminals(&t.id))
                    .collect(),
                _ => Vec::new(),
            };
            let mut next = host.workspace.clone();
            next.edit(edit).map_err(io::Error::other)?;
            host.commit(next)?;
            for id in &stopped {
                if let Some(Err(e)) = host.terminals.get(id).map(stop_terminal) {
                    eprintln!("stop closed terminal {id}: {e}");
                }
            }
            Ok(Reply::Workspace(host.snapshot()))
        }
        Request::Launch {
            id,
            space,
            argv,
            cwd,
            cols,
            rows,
            scheme,
        } => {
            geometry(cols, rows)?;
            let mut host = lock(shared)?;
            if let Some(existing) = host.workspace.terminals.iter().find(|s| s.id == id) {
                if existing.argv != argv || existing.cwd != cwd || existing.space != space {
                    return Err(io::Error::other(
                        "launch ID was already used with different arguments",
                    ));
                }
                return Ok(Reply::Workspace(host.snapshot()));
            }
            if id.is_empty()
                || argv.is_empty()
                || argv[0].is_empty()
                || !Path::new(&cwd).is_absolute()
                || !Path::new(&cwd).is_dir()
            {
                return Err(io::Error::other(
                    "launch needs an ID, argv and an absolute existing working directory",
                ));
            }
            if !host.workspace.spaces.iter().any(|s| s.id == space) {
                return Err(io::Error::other("space not found"));
            }
            let mut vt = Vt::new(cols, rows)?;
            vt.set_scheme(scheme)?;
            let replies = Arc::new(Mutex::new(Vec::new()));
            let out = Arc::clone(&replies);
            vt.on_reply(move |bytes| {
                if let Ok(mut out) = out.lock() {
                    out.extend_from_slice(bytes);
                }
            })?;
            let cell = CellPixels::default();
            vt.answer_queries(cell.clone())?;
            let mut next = host.workspace.clone();
            next.terminals.push(Terminal {
                id: id.clone(),
                space,
                argv: argv.clone(),
                cwd: cwd.clone(),
                pid: None,
                state: ProcessState::Starting,
                exit_code: None,
                error: None,
            });
            host.commit(next)?;
            let (mut child, master) =
                match pty::spawn(&argv, Path::new(&cwd), cols, rows, &id, scheme) {
                    Ok(result) => result,
                    Err(error) => {
                        let mut next = host.workspace.clone();
                        let session = next.terminals.last_mut().unwrap();
                        session.state = ProcessState::Failed;
                        session.error = Some(error.to_string());
                        host.commit(next)?;
                        return Err(error);
                    }
                };
            let reader = match master.try_clone() {
                Ok(reader) => reader,
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(e);
                }
            };
            let pid = child.id();
            let mut next = host.workspace.clone();
            let session = next.terminals.last_mut().unwrap();
            session.pid = Some(pid);
            session.state = ProcessState::Running;
            if let Err(e) = host.commit(next) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e);
            }
            run_live(
                &mut host,
                shared,
                id,
                Live {
                    vt,
                    master,
                    cols,
                    rows,
                    pid,
                    owner: None,
                    subscribers: HashMap::new(),
                    exited: false,
                    drained: false,
                    status: None,
                    exit_code: None,
                    title: String::new(),
                    cell,
                    input_at: None,
                    marks: Default::default(),
                },
                reader,
                replies,
                Reaper::Child(child),
            );
            Ok(Reply::Workspace(host.snapshot()))
        }
        Request::ReadScreen { id, rows } => {
            let host = lock(shared)?;
            let terminal = host
                .terminals
                .get(&id)
                .ok_or_else(|| io::Error::other("terminal not found"))?;
            let text = lock(terminal)?.vt.tail_text(rows.clamp(1, 10_000))?;
            Ok(Reply::Text(text))
        }
        Request::SendText { id, text, submit } => {
            send_text(shared, &id, &text, submit)?;
            Ok(Reply::Ok)
        }
        Request::TerminalCwd { id } => {
            let host = lock(shared)?;
            let terminal = host
                .terminals
                .get(&id)
                .ok_or_else(|| io::Error::other("terminal not found"))?;
            let live = lock(terminal)?;
            if live.exited {
                return Err(io::Error::other("terminal has exited"));
            }
            // What the shell reports (OSC 7) is the truth for it; the process directory
            // is the fallback for a shell without the integration.
            if let Some(path) = live.vt.pwd(&hostname()).filter(|p| Path::new(p).is_dir()) {
                return Ok(Reply::Cwd(path));
            }
            let pid = host
                .workspace
                .terminals
                .iter()
                .find(|t| t.id == id)
                .and_then(|t| t.pid)
                .ok_or_else(|| io::Error::other("terminal PID missing"))?;
            // Explicit split action only, querying this managed shell, never polling.
            let mut info: libc::proc_vnodepathinfo = unsafe { std::mem::zeroed() };
            let len = std::mem::size_of_val(&info) as i32;
            let n = unsafe {
                libc::proc_pidinfo(
                    pid as i32,
                    libc::PROC_PIDVNODEPATHINFO,
                    0,
                    (&mut info as *mut libc::proc_vnodepathinfo).cast(),
                    len,
                )
            };
            if n != len {
                return Err(io::Error::last_os_error());
            }
            let path = unsafe { std::ffi::CStr::from_ptr(info.pvi_cdir.vip_path.as_ptr().cast()) }
                .to_str()
                .map_err(io::Error::other)?
                .to_owned();
            Ok(Reply::Cwd(path))
        }
        Request::StopAgent { id } => {
            let host = lock(shared)?;
            let session = host
                .workspace
                .current_agent(&id)
                .ok_or_else(|| io::Error::other("没有已识别的活动 Agent"))?;
            let pid = host
                .agent_pids
                .iter()
                .find(|(_, sid)| *sid == &session.id)
                .map(|(pid, _)| *pid)
                .ok_or_else(|| io::Error::other("Agent 进程身份未确认，未发送停止信号"))?;
            let processes = crate::recognition::processes(&host.workspace)?;
            if !processes
                .iter()
                .any(|p| p.pid == pid && p.runtime == Some(session.runtime))
                || crate::recognition::terminal_for_process(pid, &processes, &host.workspace)
                    .as_deref()
                    != Some(id.as_str())
            {
                return Err(io::Error::other("Agent 进程身份已变化，未发送停止信号"));
            }
            // A freshly verified Agent PID only; never kill the PTY shell or group.
            if unsafe { libc::kill(pid as i32, libc::SIGTERM) } < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Reply::Ok)
        }
        Request::Stop { id } => {
            let terminal = lock(shared)?
                .terminals
                .get(&id)
                .cloned()
                .ok_or_else(|| io::Error::other("terminal not found"))?;
            stop_terminal(&terminal)?;
            Ok(Reply::Ok)
        }
        _ => Err(io::Error::other("operation requires an attached terminal")),
    }
}

/// A paste from outside the app, plus Enter as its own write: an Agent that sees the
/// Enter inside the paste would take it as a newline in the prompt, not a submit.
pub(super) fn send_text(shared: &Shared, id: &str, text: &str, submit: bool) -> io::Result<()> {
    if text.contains('\x1b') {
        return Err(io::Error::other("文本包含终端控制字符"));
    }
    if text.len() > 64 * 1024 {
        return Err(io::Error::other("单次输入超过 64 KiB"));
    }
    let terminal = lock(shared)?
        .terminals
        .get(id)
        .cloned()
        .ok_or_else(|| io::Error::other("terminal not found"))?;
    {
        let mut live = lock(&terminal)?;
        if live.exited {
            return Err(io::Error::other("terminal has exited"));
        }
        if !text.is_empty() {
            let bracketed = live
                .vt
                .terminal()
                .mode(keepline_core::terminal::libghostty_vt::terminal::Mode::BRACKETED_PASTE)
                .unwrap_or(false);
            let bytes = if bracketed {
                format!("\x1b[200~{text}\x1b[201~")
            } else if text.contains(['\n', '\r']) {
                return Err(io::Error::other("程序未启用安全粘贴，不能发送多行文本"));
            } else {
                text.to_owned()
            };
            pty::write(&mut live.master, bytes.as_bytes())?;
            live.input_at = Some(Instant::now());
        }
    }
    // Same as typing in the app: the prompt on screen is no longer waiting on the user.
    lock(shared)?.set_screen_wait(id, None)?;
    if !submit {
        return Ok(());
    }
    // Long enough for an Agent's paste handling to finish before Enter arrives.
    thread::sleep(Duration::from_millis(150));
    pty::write(&mut lock(&terminal)?.master, b"\r")?;
    record_submission(shared, id)
}
