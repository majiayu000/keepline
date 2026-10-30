//! A declared CLI that keeps its conversations in SQLite (`AgentSpec::source`) gets them
//! written out as JSON lines, one file per session, under a directory Keepline owns. From
//! there the ordinary transcript path takes over: watching, cursors, parsing, binding and
//! recovery are the same as for a CLI that writes its own files.
//!
//! The database is only ever opened read-only, kept open between polls, waits briefly on a
//! writer's lock and is dropped on any error to be found again on the next poll, so a busy
//! or migrating CLI never stalls the daemon.
use keepline_core::agent::RecordSource;
use keepline_core::model::Agent;
use rusqlite::{types::ValueRef, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Where a sourced CLI's sessions are written: the root its adapter reports.
pub fn dir(home: &Path, agent: Agent) -> PathBuf {
    home.join(".local/share/keepline/sources").join(agent.key())
}

/// One thread per registered CLI with a SQLite source; none when there is none.
pub fn start(home: &Path) {
    for agent in Agent::all() {
        let Some(source) = agent.spec().and_then(|s| s.source.clone()) else {
            continue;
        };
        let dir = dir(home, agent);
        std::thread::Builder::new()
            .name(format!("mirror-{}", agent.key()))
            .spawn(move || run(&source, &dir))
            .ok();
    }
}

fn run(source: &RecordSource, dir: &Path) {
    let mut mirror = Mirror::open(dir);
    let mut db: Option<Connection> = None;
    let mut last_error = String::new();
    loop {
        match mirror.poll(source, &mut db) {
            Ok(()) => last_error.clear(),
            Err(e) => {
                db = None;
                let e = e.to_string();
                if e != last_error {
                    eprintln!("keeplined: {} 读不了：{e}", source.sqlite.display());
                    last_error = e;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(source.poll_ms));
    }
}

/// How far each session has been written, so a restart neither repeats nor skips rows.
#[derive(Default, Serialize, Deserialize)]
struct Cursors {
    sessions: i64,
    rows: HashMap<String, (i64, String)>,
}

pub struct Mirror {
    dir: PathBuf,
    cursors: Cursors,
}

impl Mirror {
    pub fn open(dir: &Path) -> Self {
        let cursors = std::fs::read(dir.join(".cursors.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self {
            dir: dir.to_owned(),
            cursors,
        }
    }

    /// One pass: sessions changed since the last, and each one's rows after its cursor.
    pub fn poll(&mut self, source: &RecordSource, db: &mut Option<Connection>) -> io::Result<()> {
        if db.is_none() {
            let conn = Connection::open_with_flags(
                &source.sqlite,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .map_err(io::Error::other)?;
            conn.busy_timeout(Duration::from_millis(50))
                .map_err(io::Error::other)?;
            *db = Some(conn);
        }
        let conn = db.as_ref().expect("opened above");
        let changed = self.changed(conn, source).map_err(io::Error::other)?;
        if changed.is_empty() {
            return Ok(());
        }
        std::fs::create_dir_all(&self.dir)?;
        for (session, cwd, t) in changed {
            self.write_session(conn, source, &session, &cwd)?;
            self.cursors.sessions = self.cursors.sessions.max(t);
        }
        self.save()
    }

    fn changed(
        &self,
        conn: &Connection,
        source: &RecordSource,
    ) -> rusqlite::Result<Vec<(String, String, i64)>> {
        let mut stmt = conn.prepare_cached(&source.sessions)?;
        let rows = stmt.query_map([self.cursors.sessions], |r| {
            Ok((
                r.get::<_, String>("_session")?,
                r.get::<_, String>("_cwd")?,
                r.get::<_, i64>("_t")?,
            ))
        })?;
        rows.collect()
    }

    fn write_session(
        &mut self,
        conn: &Connection,
        source: &RecordSource,
        session: &str,
        cwd: &str,
    ) -> io::Result<()> {
        let (mut t, mut id) = self
            .cursors
            .rows
            .get(session)
            .cloned()
            .unwrap_or((i64::MIN, String::new()));
        let mut lines = Vec::new();
        {
            let mut stmt = conn
                .prepare_cached(&source.records)
                .map_err(io::Error::other)?;
            let names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
            let mut rows = stmt
                .query(rusqlite::params![session, t, id])
                .map_err(io::Error::other)?;
            while let Some(row) = rows.next().map_err(io::Error::other)? {
                let mut record = Map::new();
                for (i, name) in names.iter().enumerate() {
                    let value = row.get_ref(i).map_err(io::Error::other)?;
                    record.insert(name.clone(), column(value));
                }
                t = record.get("_t").and_then(Value::as_i64).unwrap_or(t);
                id = record
                    .get("_id")
                    .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_owned))
                    .unwrap_or_default();
                record.insert("_session".into(), session.into());
                record.insert("_cwd".into(), cwd.into());
                lines.extend(serde_json::to_vec(&record).map_err(io::Error::other)?);
                lines.push(b'\n');
            }
        }
        if lines.is_empty() {
            return Ok(());
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join(format!("{}.jsonl", file_name(session))))?;
        // One write per batch: the watcher never sees half a row.
        file.write_all(&lines)?;
        self.cursors.rows.insert(session.to_owned(), (t, id));
        Ok(())
    }

    fn save(&self) -> io::Result<()> {
        let tmp = self.dir.join(".cursors.json.tmp");
        std::fs::write(
            &tmp,
            serde_json::to_vec(&self.cursors).map_err(io::Error::other)?,
        )?;
        std::fs::rename(tmp, self.dir.join(".cursors.json"))
    }
}

/// A column as JSON; text that is a JSON object or array is read as one.
fn column(value: ValueRef) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(n) => n.into(),
        ValueRef::Real(f) => serde_json::Number::from_f64(f).map_or(Value::Null, Value::Number),
        ValueRef::Text(t) => {
            let s = String::from_utf8_lossy(t);
            let trimmed = s.trim_start();
            if trimmed.starts_with('{') || trimmed.starts_with('[') {
                serde_json::from_str(&s).unwrap_or_else(|_| Value::String(s.into_owned()))
            } else {
                Value::String(s.into_owned())
            }
        }
        ValueRef::Blob(_) => Value::Null,
    }
}

/// A session id as a file name: anything outside [A-Za-z0-9._-] becomes `_`.
fn file_name(session: &str) -> String {
    session
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(root: &Path) -> (RecordSource, Connection) {
        let db = root.join("cli.db");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE session(id TEXT, dir TEXT, updated INTEGER);
             CREATE TABLE part(id TEXT, session_id TEXT, t INTEGER, role TEXT, data TEXT);",
        )
        .unwrap();
        let source = RecordSource {
            sqlite: db,
            sessions: "SELECT id AS _session, dir AS _cwd, updated AS _t FROM session WHERE updated > ?1".into(),
            records: "SELECT t AS _t, id AS _id, role, data FROM part WHERE session_id = ?1 AND (t, id) > (?2, ?3) ORDER BY t, id".into(),
            poll_ms: 200,
        };
        (source, conn)
    }

    fn lines(path: &Path) -> Vec<Value> {
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn rows_are_written_once_in_order_and_survive_a_restart() {
        let root = std::env::temp_dir().join(format!("kl-mirror-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let (source, writer) = fixture(&root);
        let out = root.join("out");
        writer
            .execute_batch(
                "INSERT INTO session VALUES('s/1','/work',10);
                 INSERT INTO part VALUES('a','s/1',1,'user','{\"type\":\"text\",\"text\":\"hi\"}');
                 INSERT INTO part VALUES('b','s/1',2,'assistant','{\"type\":\"step-finish\",\"reason\":\"stop\"}');",
            )
            .unwrap();
        let mut mirror = Mirror::open(&out);
        let mut db = None;
        mirror.poll(&source, &mut db).unwrap();
        let file = out.join("s_1.jsonl");
        let first = lines(&file);
        assert_eq!(first.len(), 2);
        assert_eq!(first[0]["data"]["text"], "hi", "JSON text is read as JSON");
        assert_eq!(first[0]["_session"], "s/1");
        assert_eq!(first[0]["_cwd"], "/work");
        assert_eq!(first[1]["data"]["reason"], "stop");

        // Nothing new: nothing written.
        mirror.poll(&source, &mut db).unwrap();
        assert_eq!(lines(&file).len(), 2);

        // A restart resumes from the saved cursors; only the new row is appended.
        writer
            .execute_batch(
                "UPDATE session SET updated = 20 WHERE id = 's/1';
                 INSERT INTO part VALUES('c','s/1',3,'user','{\"type\":\"text\",\"text\":\"again\"}');",
            )
            .unwrap();
        let mut restarted = Mirror::open(&out);
        restarted.poll(&source, &mut None).unwrap();
        let all = lines(&file);
        assert_eq!(all.len(), 3);
        assert_eq!(all[2]["data"]["text"], "again");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_database_is_an_error_not_a_panic() {
        let source = RecordSource {
            sqlite: PathBuf::from("/nonexistent/keepline/cli.db"),
            sessions: "SELECT 1".into(),
            records: "SELECT 1".into(),
            poll_ms: 200,
        };
        let mut mirror = Mirror::open(Path::new("/nonexistent/keepline/out"));
        let mut db = None;
        assert!(mirror.poll(&source, &mut db).is_err());
        assert!(db.is_none());
    }
}
