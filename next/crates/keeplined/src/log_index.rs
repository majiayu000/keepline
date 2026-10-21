//! Durable identity/content index. First import is bounded; subsequent reads share one cursor.
use crate::{
    file_events::{normalized, Watch},
    recognition::Transcript,
};
use keepline_core::model::Agent;
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::{Duration, UNIX_EPOCH},
};
fn err(e: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::other(e)
}
#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct IoCounts {
    pub directories: u64,
    pub opens: u64,
    pub bytes: u64,
    pub writes: u64,
}
#[derive(Default)]
struct Cursor {
    dev: u64,
    inode: u64,
    offset: u64,
    pending: Vec<u8>,
    discard: bool,
    identity: Option<(String, String)>,
    updated: u64,
}
pub struct Index {
    db: Connection,
    pub roots: Vec<(Agent, PathBuf)>,
    pub counts: IoCounts,
    pub recent_days: u64,
    gate: std::sync::Arc<std::sync::Mutex<()>>,
}
impl Index {
    pub fn open(root: &Path, roots: Vec<(Agent, PathBuf)>) -> io::Result<Self> {
        let db = Connection::open(root.join("workbench.sqlite")).map_err(err)?;
        db.busy_timeout(Duration::from_secs(5)).map_err(err)?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS log_index (id INTEGER PRIMARY KEY CHECK(id=1), event_id INTEGER NOT NULL, error TEXT);
            CREATE TABLE IF NOT EXISTS log_scan_dirs(path TEXT PRIMARY KEY,runtime TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS log_files(path TEXT PRIMARY KEY,runtime TEXT NOT NULL,dev INTEGER NOT NULL,inode INTEGER NOT NULL,offset INTEGER NOT NULL,pending BLOB NOT NULL,discard INTEGER NOT NULL,cli_id TEXT,cwd TEXT,updated INTEGER NOT NULL);
            CREATE INDEX IF NOT EXISTS log_identity ON log_files(runtime,cli_id);").map_err(err)?;
        crate::content::schema(&db)?;
        let roots = roots
            .into_iter()
            .map(|(a, p)| Ok((a, normalized(&p)?)))
            .collect::<io::Result<Vec<_>>>()?;
        Ok(Self {
            db,
            roots,
            recent_days: 7,
            counts: IoCounts::default(),
            gate: std::sync::Arc::new(std::sync::Mutex::new(())),
        })
    }
    pub fn fork_current(&self) -> io::Result<Self> {
        let path = self
            .db
            .path()
            .ok_or_else(|| io::Error::other("index database path unavailable"))?;
        self.fork(
            Path::new(path)
                .parent()
                .ok_or_else(|| io::Error::other("index database directory unavailable"))?,
        )
    }
    pub fn fork(&self, root: &Path) -> io::Result<Self> {
        let mut index = Self::open(root, self.roots.clone())?;
        index.gate = self.gate.clone();
        index.recent_days = self.recent_days;
        Ok(index)
    }
    /// Durable directory worklist: a crash can repeat only its unfinished directory, not the tree.
    pub fn initialize(&mut self) -> io::Result<u64> {
        if let Some((event, error)) = self
            .db
            .query_row("SELECT event_id,error FROM log_index WHERE id=1", [], |r| {
                Ok((r.get::<_, i64>(0)? as u64, r.get::<_, Option<String>>(1)?))
            })
            .optional()
            .map_err(err)?
        {
            if error.is_some() {
                // Errors are advisory state, not a permanent latch. Resume the durable worklist/events.
                self.db
                    .execute("UPDATE log_index SET error=NULL WHERE id=1", [])
                    .map_err(err)?;
            }
            return Ok(event);
        }
        let event = Watch::current_id();
        let tx = self.db.transaction().map_err(err)?;
        tx.execute(
            "INSERT INTO log_index VALUES(1,?1,NULL)",
            [i64::try_from(event).map_err(err)?],
        )
        .map_err(err)?;
        for (agent, path) in &self.roots {
            tx.execute(
                "INSERT OR IGNORE INTO log_scan_dirs VALUES(?1,?2)",
                params![path.to_string_lossy(), agent.executable()],
            )
            .map_err(err)?;
        }
        tx.commit().map_err(err)?;
        self.counts.writes += 1;
        Ok(event)
    }
    pub fn bootstrap(&mut self) -> io::Result<()> {
        loop {
            let dir: Option<(String, String)> = self
                .db
                .query_row("SELECT path,runtime FROM log_scan_dirs LIMIT 1", [], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .optional()
                .map_err(err)?;
            let Some((path, runtime)) = dir else {
                // Content is a newly introduced object. Seed only cached recent identities that
                // have no projection, once, without another directory walk or replacing cursors.
                let missing = {
                    let mut q=self.db.prepare("SELECT runtime,path FROM log_files WHERE updated>=?1 AND NOT EXISTS(SELECT 1 FROM session_content WHERE session_content.path=log_files.path AND json_extract(parser,'$._rules')=4)").map_err(err)?;
                    let rows = q
                        .query_map(
                            [keepline_core::content::now()
                                .saturating_sub(self.recent_days.saturating_mul(86400))
                                as i64],
                            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                        )
                        .map_err(err)?;
                    rows.collect::<Result<Vec<_>, _>>().map_err(err)?
                };
                for (runtime, path) in missing {
                    self.update(agent(&runtime)?, Path::new(&path), true)?;
                }
                return Ok(());
            };
            let agent = agent(&runtime)?;
            self.counts.directories += 1;
            let mut children = vec![];
            match fs::read_dir(&path) {
                Ok(entries) => {
                    for entry in entries {
                        let entry = entry?;
                        let kind = entry.file_type()?;
                        if kind.is_dir() {
                            children.push(entry.path());
                        } else if kind.is_file() && is_log(&entry.path()) {
                            self.update(agent, &entry.path(), true)?;
                        }
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => (),
                Err(e) => return Err(e),
            }
            let tx = self.db.transaction().map_err(err)?;
            for child in children {
                tx.execute(
                    "INSERT OR IGNORE INTO log_scan_dirs VALUES(?1,?2)",
                    params![child.to_string_lossy(), runtime],
                )
                .map_err(err)?;
            }
            tx.execute("DELETE FROM log_scan_dirs WHERE path=?1", [path])
                .map_err(err)?;
            tx.commit().map_err(err)?;
            self.counts.writes += 1;
        }
    }
    pub fn reset_checkpoint(&self, event: u64) -> io::Result<()> {
        self.db
            .execute(
                "UPDATE log_index SET event_id=?1 WHERE id=1",
                [i64::try_from(event).map_err(err)?],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn checkpoint(&mut self, event: u64) -> io::Result<()> {
        self.db
            .prepare_cached(
                "UPDATE log_index SET event_id=MAX(event_id,?1) WHERE id=1 AND event_id<?1",
            )
            .map_err(err)?
            .execute([i64::try_from(event).map_err(err)?])
            .map_err(err)?;
        self.counts.writes += 1;
        Ok(())
    }
    pub fn fail(&self, error: &io::Error) -> io::Result<()> {
        self.db
            .execute(
                "UPDATE log_index SET error=?1 WHERE id=1",
                [error.to_string()],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn clear_error(&self) -> io::Result<()> {
        self.db
            .execute(
                "UPDATE log_index SET error=NULL WHERE id=1 AND error IS NOT NULL",
                [],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn queue_recovery(&self, scopes: &[PathBuf]) -> io::Result<()> {
        for path in scopes {
            if let Some(runtime) = self.runtime(path) {
                self.db
                    .execute(
                        "INSERT OR IGNORE INTO log_scan_dirs VALUES(?1,?2)",
                        params![path.to_string_lossy(), runtime.executable()],
                    )
                    .map_err(err)?;
            }
        }
        Ok(())
    }
    pub fn recovery_scopes(&self) -> io::Result<Vec<PathBuf>> {
        let mut statement = self
            .db
            .prepare_cached("SELECT path FROM log_scan_dirs")
            .map_err(err)?;
        let rows = statement
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(err)?;
        rows.map(|r| r.map(PathBuf::from).map_err(err)).collect()
    }
    pub fn finish_recovery(&mut self, scopes: &[PathBuf]) -> io::Result<()> {
        // Reconcile deletions missed during the gap, only inside the repaired subtree.
        // Removing an index entry never deletes an AgentSession or its user-owned outcome.
        for scope in scopes {
            let prefix = format!("{}/", scope.to_string_lossy());
            let paths = {
                let mut q = self
                    .db
                    .prepare("SELECT path FROM log_files WHERE substr(path,1,length(?1))=?1")
                    .map_err(err)?;
                let rows = q
                    .query_map(params![prefix], |r| r.get::<_, String>(0))
                    .map_err(err)?;
                rows.collect::<Result<Vec<_>, _>>().map_err(err)?
            };
            for path in paths {
                let remove = match fs::symlink_metadata(&path) {
                    Ok(m) => !m.is_file(),
                    Err(e) if e.kind() == io::ErrorKind::NotFound => true,
                    Err(e) => return Err(e),
                };
                if remove {
                    self.db
                        .execute("DELETE FROM log_files WHERE path=?1", [path])
                        .map_err(err)?;
                    self.counts.writes += 1;
                }
            }
        }
        for path in scopes {
            self.db
                .execute(
                    "DELETE FROM log_scan_dirs WHERE path=?1",
                    [path.to_string_lossy()],
                )
                .map_err(err)?;
        }
        self.clear_error()
    }
    pub fn runtime(&self, path: &Path) -> Option<Agent> {
        self.roots
            .iter()
            .find(|(_, root)| path.starts_with(root))
            .map(|(a, _)| *a)
    }
    pub fn cached(&self) -> io::Result<Vec<Transcript>> {
        cached(&self.db)
    }
    pub fn original_words(&self, path: &str) -> io::Result<String> {
        let mut q=self.db.prepare_cached("SELECT text FROM session_records WHERE path=?1 AND kind='user' ORDER BY at,offset,ordinal").map_err(err)?;
        let rows = q
            .query_map([path], |r| r.get::<_, String>(0))
            .map_err(err)?;
        Ok(rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?
            .join("\n\n追加要求："))
    }
    pub fn card(&self, path: &str) -> io::Result<Option<keepline_core::content::Content>> {
        crate::content::card(&self.db, path)
    }
    pub fn recent(&self, updated: u64) -> bool {
        updated
            >= keepline_core::content::now().saturating_sub(self.recent_days.saturating_mul(86400))
    }
    pub fn hook(&self, path: &str, event: &str) -> io::Result<()> {
        let _guard = self
            .gate
            .lock()
            .map_err(|_| io::Error::other("log cursor lock poisoned"))?;
        let mut parser = crate::content::load(&self.db, path)?;
        parser.hook(
            event,
            keepline_core::content::now(),
            keepline_core::content::Source {
                path: path.into(),
                offset: std::time::SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(err)?
                    .as_nanos() as u64,
                ..Default::default()
            },
        );
        let tx = self.db.unchecked_transaction().map_err(err)?;
        crate::content::save(&tx, path, &parser)?;
        tx.commit().map_err(err)
    }
    /// Read [offset, captured size) only; a bounded partial line survives restart.
    /// First import reads a 16 KiB identity header and at most 128 KiB of recent content.
    pub fn update(
        &mut self,
        runtime: Agent,
        path: &Path,
        historical: bool,
    ) -> io::Result<Option<Transcript>> {
        let gate = self.gate.clone();
        let _guard = gate
            .lock()
            .map_err(|_| io::Error::other("log cursor lock poisoned"))?;
        self.update_unlocked(runtime, path, historical)
    }
    /// Coalesced native notifications share one durable commit, with no timer or durability loss.
    pub fn update_many(
        &mut self,
        paths: impl IntoIterator<Item = PathBuf>,
        event: u64,
    ) -> io::Result<Vec<Transcript>> {
        let gate = self.gate.clone();
        let _guard = gate
            .lock()
            .map_err(|_| io::Error::other("log cursor lock poisoned"))?;
        self.db.execute_batch("BEGIN IMMEDIATE").map_err(err)?;
        let result = (|| {
            let mut found = vec![];
            for path in paths {
                if let Some(runtime) = self.runtime(&path) {
                    if let Some(t) = self.update_unlocked(runtime, &path, true)? {
                        found.push(t);
                    }
                }
            }
            self.checkpoint(event)?;
            Ok(found)
        })();
        match result {
            Ok(found) => {
                self.db.execute_batch("COMMIT").map_err(err)?;
                Ok(found)
            }
            Err(e) => {
                self.db.execute_batch("ROLLBACK").map_err(err)?;
                Err(e)
            }
        }
    }
    fn update_unlocked(
        &mut self,
        runtime: Agent,
        path: &Path,
        historical: bool,
    ) -> io::Result<Option<Transcript>> {
        if !is_log(path) {
            return Ok(None);
        }
        let key = path.to_string_lossy().into_owned();
        let mut cursor = self.db.prepare_cached("SELECT dev,inode,offset,pending,discard,cli_id,cwd,updated FROM log_files WHERE path=?1").map_err(err)?.query_row([&key],|r| {
            let id: Option<String> = r.get(5)?; let cwd: Option<String> = r.get(6)?;
            Ok(Cursor { dev:r.get::<_,i64>(0)? as u64,inode:r.get::<_,i64>(1)? as u64,offset:r.get::<_,i64>(2)? as u64,pending:r.get(3)?,discard:r.get(4)?,identity:id.zip(cwd),updated:r.get::<_,i64>(7)? as u64 })
        }).optional().map_err(err)?;
        // No symlink traversal in event handling, just as in the initial directory walk.
        let meta = match fs::symlink_metadata(path) {
            Ok(m) if m.is_file() => m,
            Ok(_) => return Ok(None),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                self.db
                    .execute("DELETE FROM log_files WHERE path=?1", [&key])
                    .map_err(err)?;
                self.counts.writes += 1;
                return Ok(None);
            }
            Err(e) => return Err(e),
        };
        let updated = meta
            .modified()?
            .duration_since(UNIX_EPOCH)
            .map_err(err)?
            .as_secs();
        let seeded: bool = self
            .db
            .prepare_cached("SELECT EXISTS(SELECT 1 FROM session_content WHERE path=?1 AND json_extract(parser,'$._rules')=4)")
            .map_err(err)?
            .query_row([&key], |r| r.get(0))
            .map_err(err)?;
        if let Some(c) = &cursor {
            if c.dev != meta.dev() || c.inode != meta.ino() || meta.len() < c.offset {
                cursor = None;
            } else if c.offset == meta.len() && (seeded || !self.recent(updated)) {
                return Ok(c.identity.as_ref().map(|(id, cwd)| Transcript {
                    runtime,
                    cli_id: id.clone(),
                    cwd: cwd.clone(),
                    path: key,
                    updated: c.updated,
                }));
            }
        }
        let fresh = cursor.is_none();
        let mut c = cursor.unwrap_or_default();
        self.counts.opens += 1;
        let mut file = File::open(path)?;
        let opened = file.metadata()?;
        if opened.dev() != meta.dev() || opened.ino() != meta.ino() {
            return Err(io::Error::other(
                "log rotated while opening; cursor not advanced",
            ));
        }
        let mut parser = if fresh || !seeded {
            crate::content::Parser::default()
        } else {
            crate::content::load(&self.db, &key)?
        };
        let seed = (fresh && historical) || (!seeded && !fresh);
        if seed {
            // Identity requires only the header, even when old bodies are deliberately skipped.
            file.seek(SeekFrom::Start(0))?;
            // Recent files get one cached prefix search: 64 KiB steps, at most 384 KiB.
            // Real Codex developer/environment records often push the first user past 128 KiB.
            // Old files retain the cheap identity-only 16 KiB read.
            let recent = self.recent(updated);
            let budget = meta.len().min(if recent { 384 * 1024 } else { 16 * 1024 });
            let mut head = Vec::new();
            let mut parsed = 0usize;
            while (head.len() as u64) < budget {
                let end = budget.min(head.len() as u64 + if recent { 64 * 1024 } else { 16 * 1024 })
                    as usize;
                let old = head.len();
                head.resize(end, 0);
                file.read_exact(&mut head[old..])?;
                self.counts.bytes += (end - old) as u64;
                for line in head[parsed..].split_inclusive(|b| *b == b'\n') {
                    parser.observe_header(runtime, line);
                    if c.identity.is_none() {
                        c.identity = identity(runtime, line);
                    }
                    if line.last() != Some(&b'\n') {
                        break;
                    }
                    if recent && !parser.card.first_ask_known {
                        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(line) {
                            let user = if runtime == Agent::Codex {
                                v["type"] == "event_msg" && v["payload"]["type"] == "user_message"
                                    || v["type"] == "response_item"
                                        && v["payload"]["role"] == "user"
                            } else {
                                v["type"] == "user"
                            };
                            if user {
                                parser.add(
                                    runtime,
                                    line,
                                    keepline_core::content::Source {
                                        path: key.clone(),
                                        inode: meta.ino(),
                                        offset: parsed as u64,
                                    },
                                );
                            }
                        }
                    }
                    parsed += line.len();
                }
                if parser.card.first_ask_known || !parser.card.hidden_reason.is_empty() || !recent {
                    break;
                }
            }
            c.offset = if recent {
                meta.len().saturating_sub(128 * 1024)
            } else {
                meta.len()
            };
            // The prefix is original intent, never proof of the current turn.
            parser.card.turn = keepline_core::content::Turn::Unknown;
            c.pending.clear();
            c.discard = c.offset > 0;
            parser.card.partial_history = c.offset > 0;
            parser.card.from_offset = c.offset;
            // One preceding byte lets a tail beginning exactly on a line boundary remain usable.
            if c.offset > 0 {
                file.seek(SeekFrom::Start(c.offset - 1))?;
                let mut b = [0];
                file.read_exact(&mut b)?;
                self.counts.bytes += 1;
                c.discard = b[0] != b'\n';
            }
        }
        file.seek(SeekFrom::Start(c.offset))?;
        let end = meta.len();
        let mut buffer = [0u8; 16 * 1024];
        let mut line_offset = c.offset.saturating_sub(c.pending.len() as u64);
        while c.offset < end {
            let want = ((end - c.offset) as usize).min(buffer.len());
            let n = file.read(&mut buffer[..want])?;
            if n == 0 {
                break;
            }
            self.counts.bytes += n as u64;
            for byte in &buffer[..n] {
                c.offset += 1;
                if *byte == b'\n' {
                    if !c.discard {
                        if c.identity.is_none() {
                            c.identity = identity(runtime, &c.pending);
                        }
                        parser.add(
                            runtime,
                            &c.pending,
                            keepline_core::content::Source {
                                path: key.clone(),
                                inode: meta.ino(),
                                offset: line_offset,
                            },
                        );
                    } else {
                        parser.card.partial_history = true;
                    }
                    c.pending.clear();
                    c.discard = false;
                    line_offset = c.offset;
                } else if !c.discard {
                    if c.pending.len() == 256 * 1024 {
                        c.pending.clear();
                        c.discard = true;
                        parser.card.unknown_records += 1;
                    } else {
                        c.pending.push(*byte);
                    }
                }
            }
        }
        c.dev = meta.dev();
        c.inode = meta.ino();
        c.updated = updated;
        let tx = self.db.savepoint().map_err(err)?;
        tx.prepare_cached("INSERT INTO log_files VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) ON CONFLICT(path) DO UPDATE SET runtime=excluded.runtime,dev=excluded.dev,inode=excluded.inode,offset=excluded.offset,pending=excluded.pending,discard=excluded.discard,cli_id=excluded.cli_id,cwd=excluded.cwd,updated=excluded.updated").map_err(err)?.execute(
            params![key,runtime.executable(),i64::try_from(c.dev).map_err(err)?,i64::try_from(c.inode).map_err(err)?,i64::try_from(c.offset).map_err(err)?,c.pending,c.discard,c.identity.as_ref().map(|v| &v.0),c.identity.as_ref().map(|v| &v.1),i64::try_from(c.updated).map_err(err)?]).map_err(err)?;
        if !seeded {
            tx.execute("DELETE FROM session_records WHERE path=?1", [&key])
                .map_err(err)?;
        }
        crate::content::save(&tx, &key, &parser)?;
        tx.commit().map_err(err)?;
        self.counts.writes += 1;
        Ok(c.identity.map(|(cli_id, cwd)| Transcript {
            runtime,
            cli_id,
            path: key,
            cwd,
            updated,
        }))
    }
}
pub fn cached(db: &Connection) -> io::Result<Vec<Transcript>> {
    let mut query = db
        .prepare("SELECT runtime,cli_id,path,cwd,updated FROM log_files WHERE cli_id IS NOT NULL")
        .map_err(err)?;
    let mut rows = query.query([]).map_err(err)?;
    let mut out = vec![];
    while let Some(r) = rows.next().map_err(err)? {
        out.push(Transcript {
            runtime: agent(&r.get::<_, String>(0).map_err(err)?)?,
            cli_id: r.get(1).map_err(err)?,
            path: r.get(2).map_err(err)?,
            cwd: r.get(3).map_err(err)?,
            updated: r.get::<_, i64>(4).map_err(err)? as u64,
        });
    }
    Ok(out)
}
fn agent(s: &str) -> io::Result<Agent> {
    match s {
        "codex" => Ok(Agent::Codex),
        "claude" => Ok(Agent::Claude),
        _ => Err(io::Error::other("unknown indexed runtime")),
    }
}
fn is_log(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "jsonl")
}

fn identity(runtime: Agent, bytes: &[u8]) -> Option<(String, String)> {
    let value = crate::content::header_value(runtime, bytes)?;
    let (id, cwd) = match runtime {
        Agent::Codex if value["type"] == "session_meta" => (
            value["payload"]["id"].as_str()?,
            value["payload"]["cwd"].as_str()?,
        ),
        Agent::Claude if value["isSidechain"] == true => {
            return Some((
                format!(
                    "{}:sidechain:{}",
                    value["sessionId"].as_str()?,
                    value["agentId"]
                        .as_str()
                        .or_else(|| value["uuid"].as_str())?
                ),
                value["cwd"].as_str()?.into(),
            ));
        }
        Agent::Claude => (value["sessionId"].as_str()?, value["cwd"].as_str()?),
        _ => return None,
    };
    (!id.is_empty()).then(|| (id.into(), cwd.into()))
}

/// Event-triggered subtree repair. Each step visits at most 128 entries or 20 ms;
/// file cursor logic still skips old bodies. No global scan and no retry timer.
pub struct Recovery {
    queue: std::collections::VecDeque<(Agent, PathBuf)>,
    current: Option<(Agent, fs::ReadDir)>,
}
impl Recovery {
    pub fn new(index: &Index, scopes: &[PathBuf]) -> Self {
        Self {
            queue: scopes
                .iter()
                .filter_map(|p| index.runtime(p).map(|a| (a, p.clone())))
                .collect(),
            current: None,
        }
    }
    pub fn step(&mut self, index: &mut Index) -> io::Result<(bool, Vec<Transcript>)> {
        let started = std::time::Instant::now();
        let mut transcripts = vec![];
        for _ in 0..128 {
            if started.elapsed() >= Duration::from_millis(20) {
                break;
            }
            if self.current.is_none() {
                let Some((runtime, path)) = self.queue.pop_front() else {
                    return Ok((true, transcripts));
                };
                if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
                    continue;
                }
                index.counts.directories += 1;
                match fs::read_dir(&path) {
                    Ok(entries) => self.current = Some((runtime, entries)),
                    Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                    Err(e) => return Err(e),
                }
            }
            let (runtime, entries) = self.current.as_mut().unwrap();
            match entries.next() {
                Some(entry) => {
                    let entry = entry?;
                    let ty = entry.file_type()?;
                    if ty.is_dir() {
                        self.queue.push_back((*runtime, entry.path()));
                    } else if ty.is_file() && is_log(&entry.path()) {
                        if let Some(t) = index.update(*runtime, &entry.path(), true)? {
                            transcripts.push(t);
                        }
                    }
                }
                None => self.current = None,
            }
        }
        Ok((self.current.is_none() && self.queue.is_empty(), transcripts))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("kl-index-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(root.join("logs")).unwrap();
            Self(root.canonicalize().unwrap())
        }
        fn index(&self) -> Index {
            Index::open(&self.0, vec![(Agent::Codex, self.0.join("logs"))]).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn line(id: &str) -> String {
        format!("{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{id}\",\"cwd\":\"/tmp\"}}}}\n")
    }
    #[test]
    fn original_request_beyond_16k_is_read_once_within_bounded_prefix() {
        let f = Fixture::new();
        let path = f.0.join("logs/head.jsonl");
        let mut log = format!(
            "{}\n",
            serde_json::json!({"type":"session_meta","payload":{"id":"head","cwd":"/repo","source":"vscode","thread_source":"user","originator":"Codex Desktop","base_instructions":"x".repeat(24*1024)}})
        );
        let offset = log.len() as u64;
        log.push_str("{\"type\":\"event_msg\",\"payload\":{\"type\":\"user_message\",\"message\":\"最初真实要求\"}}\n");
        log.push_str(&" ".repeat(200 * 1024));
        log.push_str("\n{\"type\":\"event_msg\",\"payload\":{\"type\":\"user_message\",\"message\":\"追加要求\"}}\n");
        fs::write(&path, log).unwrap();
        let mut i = f.index();
        i.initialize().unwrap();
        i.bootstrap().unwrap();
        let card = i.card(path.to_str().unwrap()).unwrap().unwrap();
        assert_eq!(card.title, "最初真实要求");
        assert_eq!(card.asks[0].source.offset, offset);
        assert!(card.first_ask_known && card.hidden_reason().is_none());
        assert!(i.counts.bytes <= 192 * 1024 + 1);
        i.counts = IoCounts::default();
        i.bootstrap().unwrap();
        assert_eq!(i.counts.bytes, 0);
        assert_eq!(i.counts.opens, 0);
    }
    #[test]
    fn changed_content_rules_reseed_once_without_a_directory_walk() {
        let f = Fixture::new();
        let path = f.0.join("logs/rules.jsonl");
        fs::write(&path,concat!("{\"type\":\"session_meta\",\"payload\":{\"id\":\"r\",\"cwd\":\"/repo\",\"originator\":\"codex-tui\",\"source\":\"cli\"}}\n",
            "{\"type\":\"event_msg\",\"payload\":{\"type\":\"user_message\",\"message\":\"真实要求\"}}\n")).unwrap();
        let mut i = f.index();
        i.initialize().unwrap();
        i.bootstrap().unwrap();
        i.db.execute(
            "UPDATE session_content SET parser=json_remove(parser,'$._rules')",
            [],
        )
        .unwrap();
        i.db.execute(
            "UPDATE session_records SET text='old injected context' WHERE kind='user'",
            [],
        )
        .unwrap();
        i.counts = IoCounts::default();
        i.bootstrap().unwrap();
        assert_eq!(i.counts.directories, 0);
        assert!(i.counts.bytes > 0);
        let c = i.card(path.to_str().unwrap()).unwrap().unwrap();
        assert!(c.interactive);
        assert_eq!(c.title, "真实要求");
        let old: i64 =
            i.db.query_row(
                "SELECT count(*) FROM session_records WHERE text='old injected context'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(old, 0);
        i.counts = IoCounts::default();
        i.bootstrap().unwrap();
        assert_eq!(i.counts.bytes, 0);
        assert_eq!(i.counts.opens, 0);
        assert_eq!(
            identity(
                Agent::Claude,
                br#"{"sessionId":"parent","cwd":"/repo","isSidechain":true,"agentId":"child"}"#
            )
            .unwrap()
            .0,
            "parent:sidechain:child"
        );
    }
    #[test]
    fn bounded_tail_and_atomic_cursor_content_survive_restart() {
        use std::io::Write;
        let f = Fixture::new();
        let path = f.0.join("logs/content.jsonl");
        let mut file = File::create(&path).unwrap();
        file.write_all(line("content").as_bytes()).unwrap();
        file.write_all(&vec![b'x'; 2 * 1024 * 1024]).unwrap();
        file.write_all(b"\n{\"type\":\"event_msg\",\"payload\":{\"type\":\"user_message\",\"message\":\"tail request\"}}\n").unwrap();
        let mut index = f.index();
        index.update(Agent::Codex, &path, true).unwrap();
        assert!(index.counts.bytes <= 512 * 1024 + 1);
        assert!(
            index
                .card(path.to_str().unwrap())
                .unwrap()
                .unwrap()
                .partial_history
        );
        drop(index);
        let mut index = f.index();
        let before = index.card(path.to_str().unwrap()).unwrap().unwrap();
        index.db.execute_batch("CREATE TRIGGER fail_record BEFORE INSERT ON session_records BEGIN SELECT RAISE(ABORT,'fixture atomic failure'); END;").unwrap();
        let extra=b"{\"type\":\"event_msg\",\"payload\":{\"type\":\"user_message\",\"message\":\"additional requirement\"}}\n";
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(extra)
            .unwrap();
        assert!(index.update(Agent::Codex, &path, false).is_err());
        assert_eq!(index.card(path.to_str().unwrap()).unwrap().unwrap(), before);
        index.db.execute_batch("DROP TRIGGER fail_record").unwrap();
        index.update(Agent::Codex, &path, false).unwrap();
        assert_eq!(
            index.counts.bytes,
            extra.len() as u64 * 2,
            "failed transaction retries only the uncommitted delta"
        );
        let c = index.card(path.to_str().unwrap()).unwrap().unwrap();
        assert_eq!(c.asks.len(), 2);
        assert_eq!(c.asks[1].text, "additional requirement");
        let counts = index.counts.clone();
        index.update(Agent::Codex, &path, true).unwrap();
        assert_eq!(index.counts, counts);
    }
    #[test]
    fn cursors_survive_restart_partial_lines_truncation_and_inode_replacement() {
        let f = Fixture::new();
        let path = f.0.join("logs/one.jsonl");
        let text = line("one");
        let cut = text.len() - 8;
        fs::write(&path, &text[..cut]).unwrap();
        let mut i = f.index();
        assert!(i.update(Agent::Codex, &path, false).unwrap().is_none());
        drop(i);
        let mut i = f.index();
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(&text.as_bytes()[cut..])
            .unwrap();
        assert_eq!(
            i.update(Agent::Codex, &path, false)
                .unwrap()
                .unwrap()
                .cli_id,
            "one"
        );
        assert_eq!(i.counts.bytes, (text.len() - cut) as u64);
        let before = i.counts.clone();
        i.update(Agent::Codex, &path, false).unwrap();
        assert_eq!(
            i.counts, before,
            "duplicate event performs no file read/write"
        );
        fs::write(&path, line("x")).unwrap();
        assert_eq!(
            i.update(Agent::Codex, &path, false)
                .unwrap()
                .unwrap()
                .cli_id,
            "x"
        );
        let replacement = f.0.join("replacement");
        fs::write(&replacement, line("replacement")).unwrap();
        fs::rename(replacement, &path).unwrap();
        assert_eq!(
            i.update(Agent::Codex, &path, false)
                .unwrap()
                .unwrap()
                .cli_id,
            "replacement"
        );
        fs::remove_file(&path).unwrap();
        assert!(i.update(Agent::Codex, &path, false).unwrap().is_none());
        assert!(i.cached().unwrap().is_empty());
    }
    #[test]
    fn initial_index_skips_large_bodies_and_never_rewalks_completed_directories() {
        let f = Fixture::new();
        let path = f.0.join("logs/huge.jsonl");
        let mut file = File::create(&path).unwrap();
        file.write_all(line("huge").as_bytes()).unwrap();
        file.set_len(17 * 1024 * 1024 * 1024).unwrap();
        let mut i = f.index();
        let checkpoint = i.initialize().unwrap();
        i.bootstrap().unwrap();
        assert!(i.counts.bytes <= 512 * 1024 + 1);
        assert_eq!(i.cached().unwrap()[0].cli_id, "huge");
        assert_eq!(
            i.db.query_row("SELECT offset FROM log_files", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            17 * 1024 * 1024 * 1024
        );
        drop(i);
        let mut i = f.index();
        assert_eq!(i.initialize().unwrap(), checkpoint);
        i.bootstrap().unwrap();
        assert_eq!(
            i.counts,
            IoCounts::default(),
            "warm start has no directory walk or file reads"
        );
        assert_eq!(i.cached().unwrap().len(), 1);
    }
    #[test]
    fn fsevents_delivers_new_files_and_replays_changes_after_restart() {
        let f = Fixture::new();
        let mut i = f.index();
        let since = i.initialize().unwrap();
        let paths = vec![f.0.join("logs")];
        let watch = Watch::start(&paths, since).unwrap();
        i.bootstrap().unwrap();
        let first = paths[0].join("one.jsonl");
        fs::write(&first, line("one")).unwrap();
        let until = std::time::Instant::now() + Duration::from_secs(10);
        let mut checkpoint = since;
        loop {
            let batch = watch
                .rx
                .recv_timeout(until.saturating_duration_since(std::time::Instant::now()))
                .unwrap();
            let found = batch.events.iter().any(|e| e.path == first);
            for e in batch.events {
                assert_eq!(e.flags & 0xf, 0, "event gap {e:?}");
                checkpoint = checkpoint.max(e.id);
            }
            if found {
                break;
            }
        }
        i.update(Agent::Codex, &first, false).unwrap();
        i.checkpoint(checkpoint).unwrap();
        drop(watch);
        drop(i);
        let second = paths[0].join("two.jsonl");
        fs::write(&second, line("two")).unwrap();
        // FSEvents persists and delivers these without a directory catch-up scan.
        let mut i = f.index();
        let since = i.initialize().unwrap();
        i.bootstrap().unwrap();
        assert_eq!(i.counts.directories, 0);
        let watch = Watch::start(&paths, since).unwrap();
        let until = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            let batch = watch
                .rx
                .recv_timeout(until.saturating_duration_since(std::time::Instant::now()))
                .unwrap();
            if batch.events.iter().any(|e| e.path == second) {
                break;
            }
        }
        assert_eq!(
            i.update(Agent::Codex, &second, false)
                .unwrap()
                .unwrap()
                .cli_id,
            "two"
        );
        assert_eq!(i.cached().unwrap().len(), 2);
    }
}
