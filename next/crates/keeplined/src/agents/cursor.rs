//! Cursor Agent CLI: `~/.cursor/projects/<encoded cwd>/agent-transcripts/<id>/<id>.jsonl`,
//! plain `{role, message}` records and a `turn_ended` marker. Records carry no time, id or
//! directory and tools leave no results. The chat's own folder,
//! `~/.cursor/chats/<hash>/<id>/`, keeps its directory (`meta.json`), its title (`store.db`)
//! and, once someone typed into it, `prompt_history.json`.
use super::{temporary, Adapter};
use crate::content::{text, Parser};
use keepline_core::content::{short, Source, Turn};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
    time::SystemTime,
};

pub struct Cursor;

fn cursor_home(home: &Path) -> PathBuf {
    home.join(".cursor")
}

/// The chat a transcript belongs to: its id is the transcript's folder.
fn chat_id(transcript: &Path) -> Option<&str> {
    let id = transcript.parent()?.file_name()?.to_str()?;
    uuid::Uuid::parse_str(id).ok().map(|_| id)
}

/// `~/.cursor/chats/<hash of cwd>/<id>`, found by id since the hash needs the cwd.
fn chat_dir(transcript: &Path) -> Option<PathBuf> {
    let id = chat_id(transcript)?;
    // <cursor>/projects/<project>/agent-transcripts/<id>/<id>.jsonl
    let cursor = transcript.ancestors().nth(5)?;
    std::fs::read_dir(cursor.join("chats"))
        .ok()?
        .flatten()
        .map(|e| e.path().join(id))
        .find(|d| d.is_dir())
}

/// The chat's working directory: `meta.json` when it says, else the project folder's name
/// matched against directories that exist.
fn session_cwd(transcript: &Path) -> Option<String> {
    if let Some(cwd) = chat_dir(transcript)
        .and_then(|d| std::fs::read(d.join("meta.json")).ok())
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|m| m["cwd"].as_str().map(str::to_owned))
    {
        return Some(cwd);
    }
    let project = transcript.ancestors().nth(3)?.file_name()?.to_str()?;
    decode(Path::new("/"), &project.split('-').collect::<Vec<_>>())
}

/// How Cursor names a path in a project folder: every run of other characters becomes `-`.
fn encoded(name: &str) -> Vec<&str> {
    name.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect()
}

/// The existing directory whose encoded path is `rest`, read one level at a time.
fn decode(dir: &Path, rest: &[&str]) -> Option<String> {
    if rest.is_empty() {
        return dir.to_str().map(str::to_owned);
    }
    std::fs::read_dir(dir).ok()?.flatten().find_map(|e| {
        let name = e.file_name();
        let parts = encoded(name.to_str()?);
        let path = e.path();
        (!parts.is_empty() && rest.starts_with(&parts) && path.is_dir())
            .then(|| decode(&path, &rest[parts.len()..]))?
    })
}

/// When a chat's store last changed, and the name read then.
type Cached = (Option<SystemTime>, Option<String>);

/// The title Cursor generated or was given, cached until the chat's store changes. A chat
/// nobody prompted is still called "New Agent", which names nothing.
fn chat_name(dir: &Path) -> Option<String> {
    static NAMES: Mutex<Option<HashMap<PathBuf, Cached>>> = Mutex::new(None);
    let changed = ["store.db", "store.db-wal"]
        .iter()
        .filter_map(|f| std::fs::metadata(dir.join(f)).ok()?.modified().ok())
        .max();
    let mut names = NAMES.lock().ok()?;
    let names = names.get_or_insert_with(HashMap::new);
    if let Some((at, name)) = names.get(dir) {
        if *at == changed {
            return name.clone();
        }
    }
    let name = read_chat_name(&dir.join("store.db"));
    names.insert(dir.to_owned(), (changed, name.clone()));
    name
}
fn read_chat_name(store: &Path) -> Option<String> {
    let db = Connection::open_with_flags(
        store,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    db.busy_timeout(std::time::Duration::from_millis(50)).ok()?;
    let hex: String = db
        .query_row("SELECT value FROM meta WHERE key='0'", [], |r| r.get(0))
        .optional()
        .ok()??;
    let bytes = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    let meta: Value = serde_json::from_slice(&bytes).ok()?;
    let name = meta["name"].as_str()?.trim();
    (!name.is_empty() && name != "New Agent").then(|| name.to_owned())
}

/// What the user typed: Cursor wraps it as `<timestamp>…</timestamp><user_query>…</user_query>`.
fn query(s: &str) -> &str {
    s.split_once("<user_query>")
        .and_then(|(_, q)| q.rsplit_once("</user_query>").map(|(q, _)| q))
        .unwrap_or(s)
        .trim()
}

/// `Tuesday, Aug 25, 2026, 11:21 AM (UTC+8)`, the only time a transcript records.
fn stamped(s: &str) -> Option<u64> {
    let (_, rest) = s.split_once("<timestamp>")?;
    let (stamp, _) = rest.split_once("</timestamp>")?;
    let (local, zone) = stamp.trim().rsplit_once(" (UTC")?;
    let zone = zone.strip_suffix(')')?;
    let (hours, minutes) = zone.split_once(':').unwrap_or((zone, "0"));
    let hours: i32 = if hours.is_empty() {
        0
    } else {
        hours.parse().ok()?
    };
    let minutes: i32 = minutes.parse().ok()?;
    let offset = chrono::FixedOffset::east_opt(hours * 3600 + hours.signum() * minutes * 60)?;
    let naive = chrono::NaiveDateTime::parse_from_str(local, "%A, %b %d, %Y, %I:%M %p").ok()?;
    let at = naive.and_local_timezone(offset).single()?.timestamp();
    u64::try_from(at).ok()
}

/// The card's names for the tools whose calls it reads; the rest keep Cursor's names.
fn canonical(name: &str) -> &str {
    match name {
        "Shell" => "Bash",
        "StrReplace" => "Edit",
        "AskQuestion" => "AskUserQuestion",
        _ => name,
    }
}

/// Cursor asks with `prompt`; the card's question reader looks for `question`.
fn arguments(name: &str, input: &Value) -> Value {
    if name != "AskQuestion" {
        return input.clone();
    }
    let questions: Vec<Value> = input["questions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|q| json!({"question": q["prompt"], "options": q["options"]}))
        .collect();
    json!({ "questions": questions })
}

/// A finished reply segment: before a tool call, or at the end of the turn.
fn flush(parser: &mut Parser, final_answer: bool, at: u64, source: &Source) {
    let reply = std::mem::take(&mut parser.reply);
    if !reply.trim().is_empty() {
        parser.message(false, reply, at, source, final_answer);
    }
}

impl Adapter for Cursor {
    fn root(&self, home: &Path) -> PathBuf {
        cursor_home(home).join("projects")
    }
    /// The launcher execs Cursor's bundled node as `cursor-agent`.
    fn runs(&self, executable: &str, script: Option<&str>) -> bool {
        executable == "cursor-agent"
            || script.is_some_and(|s| s.contains("/cursor-agent/versions/"))
    }
    fn process_session(&self, _pid: u32, args: &[String]) -> (Option<String>, bool) {
        let mut found = None;
        let mut args = args.iter().skip(1);
        while let Some(arg) = args.next() {
            let value = match arg.as_str() {
                "--resume" => args.next().map(String::as_str),
                _ => arg.strip_prefix("--resume="),
            };
            if let Some(id) = value.and_then(|v| uuid::Uuid::parse_str(v).ok()) {
                if found.is_some() {
                    return (None, false);
                }
                found = Some(id.to_string());
            }
        }
        (found, false)
    }
    fn is_transcript(&self, path: &Path) -> bool {
        chat_id(path).is_some_and(|id| {
            path.file_name() == Some(format!("{id}.jsonl").as_ref())
                && path
                    .ancestors()
                    .nth(2)
                    .and_then(Path::file_name)
                    .is_some_and(|n| n == "agent-transcripts")
        })
    }
    /// Project folders, their transcript folder and the chats in it; never terminals,
    /// canvases or tool output beside them.
    fn descend(&self, dir: &Path) -> bool {
        fn name(p: Option<&Path>) -> Option<&str> {
            p.and_then(Path::file_name).and_then(|n| n.to_str())
        }
        name(Some(dir)) == Some("agent-transcripts")
            || name(dir.parent()) == Some("agent-transcripts")
            || name(dir.parent()) == Some("projects")
    }
    fn identity(&self, path: &Path, _record: &Value) -> Option<(String, String)> {
        Some((chat_id(path)?.to_owned(), session_cwd(path)?))
    }
    /// Cursor's TUI keeps what was typed; `-p` runs, subagents and editor chats keep none.
    fn observe_file(&self, parser: &mut Parser, path: &Path) {
        if session_cwd(path).is_some_and(|cwd| temporary(&cwd)) {
            parser.hide(keepline_core::content::TEMPORARY_DIRECTORY);
        } else if chat_dir(path).is_some_and(|d| d.join("prompt_history.json").is_file()) {
            parser.card.interactive = true;
        }
    }
    fn name(&self, _root: &Path, transcript: &Path) -> Option<String> {
        chat_name(&chat_dir(transcript)?)
    }
    fn metadata(&self, parser: &mut Parser, record: &Value) {
        if self.is_user(record) {
            parser.machine_prompt(query(&text(&record["message"]["content"])));
        }
    }
    fn is_user(&self, record: &Value) -> bool {
        record["role"] == "user"
    }
    fn add(&self, parser: &mut Parser, record: &Value, at: u64, source: &Source) {
        let blocks = &record["message"]["content"];
        // Only prompts are dated; the rest happened by the time the file last changed.
        let at = match at {
            0 => stamped(&text(blocks))
                .filter(|_| self.is_user(record))
                .or_else(|| {
                    let modified = std::fs::metadata(&source.path).ok()?.modified().ok()?;
                    Some(
                        modified
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .ok()?
                            .as_secs(),
                    )
                })
                .unwrap_or(0)
                .max(parser.card.updated),
            at => at,
        };
        if self.is_user(record) {
            flush(parser, false, at, source);
            parser.phase(Turn::Running, "", at, source);
            parser.message(true, query(&text(blocks)).to_owned(), at, source, false);
            return;
        }
        if record["type"] == "turn_ended" {
            flush(parser, true, at, source);
            match record["status"].as_str() {
                Some("aborted") => parser.phase(Turn::Interrupted, "", at, source),
                Some("error") if record["error"] == "User aborted request" => {
                    parser.phase(Turn::Interrupted, "", at, source)
                }
                Some("error") => {
                    parser.card.error = short(&text(&record["error"]), 600);
                    parser.phase(Turn::Error, "", at, source);
                }
                _ => parser.phase(Turn::Completed, "", at, source),
            }
            return;
        }
        for (i, b) in blocks.as_array().into_iter().flatten().enumerate() {
            match b["type"].as_str() {
                Some("text") => {
                    parser.reply.push_str(b["text"].as_str().unwrap_or(""));
                    parser.card.updated = parser.card.updated.max(at);
                }
                Some("tool_use") => {
                    flush(parser, false, at, source);
                    let name = b["name"].as_str().unwrap_or("");
                    // Calls have no ids; where they were written is one.
                    parser.tool(
                        &format!("{}:{i}", source.offset),
                        canonical(name),
                        arguments(name, &b["input"]),
                        at,
                        source,
                    );
                }
                _ => (),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keepline_core::model::Agent;

    fn session() -> (PathBuf, PathBuf) {
        let home = std::env::temp_dir().join(format!("kl-cursor-{}", uuid::Uuid::new_v4()));
        let id = "c19a5748-897d-4836-a555-fce8a7797222";
        let dir = home.join(format!(".cursor/projects/repo/agent-transcripts/{id}"));
        std::fs::create_dir_all(&dir).unwrap();
        (home, dir.join(format!("{id}.jsonl")))
    }
    fn feed(path: &Path, lines: &[Value]) -> Parser {
        let mut p = Parser::default();
        for (i, l) in lines.iter().enumerate() {
            p.add(
                Agent::Cursor,
                &serde_json::to_vec(l).unwrap(),
                Source {
                    path: path.to_string_lossy().into(),
                    inode: 1,
                    offset: i as u64 * 100,
                },
            );
        }
        p
    }
    fn user(text: &str) -> Value {
        json!({"role":"user","message":{"content":[{"type":"text","text":text}]}})
    }
    fn assistant(content: Value) -> Value {
        json!({"role":"assistant","message":{"content":content}})
    }

    #[test]
    fn a_turn_reads_the_query_its_time_tools_and_final_reply() {
        let (home, path) = session();
        let p = feed(
            &path,
            &[
                user(
                    "<timestamp>Tuesday, Aug 25, 2026, 11:21 AM (UTC+8)</timestamp>\n<user_query>\n修好登录页\n</user_query>",
                ),
                assistant(json!([{"type":"text","text":"先看测试。"},
                    {"type":"tool_use","name":"Shell","input":{"command":"cargo test"}}])),
                assistant(json!([{"type":"text","text":"已修好。"}])),
                json!({"type":"turn_ended","status":"success"}),
            ],
        );
        assert_eq!(p.card.asks[0].text, "修好登录页");
        assert_eq!(p.card.turn_started, 1_787_628_060);
        assert_eq!(p.card.turn, Turn::Completed);
        assert_eq!(p.card.last_reply, "已修好。");
        assert!(p
            .records
            .iter()
            .any(|r| r.kind == "tool_call" && r.text.starts_with("Bash ")));
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn aborts_errors_and_questions_set_the_turn() {
        let (home, path) = session();
        for (end, turn) in [
            (
                json!({"type":"turn_ended","status":"aborted"}),
                Turn::Interrupted,
            ),
            (
                json!({"type":"turn_ended","status":"error","error":"User aborted request"}),
                Turn::Interrupted,
            ),
            (
                json!({"type":"turn_ended","status":"error","error":"rate limited"}),
                Turn::Error,
            ),
        ] {
            assert_eq!(
                feed(&path, &[user("<user_query>x</user_query>"), end])
                    .card
                    .turn,
                turn
            );
        }
        let asked = feed(
            &path,
            &[
                user("<user_query>做个 app</user_query>"),
                assistant(
                    json!([{"type":"tool_use","name":"AskQuestion","input":{"questions":[
                    {"id":"scope","prompt":"定位是什么？","options":[{"id":"a","label":"桌面端"}]}]}}]),
                ),
            ],
        );
        assert_eq!(asked.card.turn, Turn::Waiting);
        assert!(asked.card.waiting.contains("定位是什么？"));
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn the_chat_folder_gives_directory_title_and_interactivity() {
        let (home, path) = session();
        let id = chat_id(&path).unwrap().to_owned();
        assert!(Cursor.is_transcript(&path));
        assert!(!Cursor.is_transcript(&path.with_file_name("other.jsonl")));
        // No chat folder: the project name is matched against real directories.
        let repo = home.join("my-app.v2");
        std::fs::create_dir_all(&repo).unwrap();
        let encoded = encoded(repo.to_str().unwrap()).join("-");
        let project = home.join(format!(".cursor/projects/{encoded}/agent-transcripts/{id}"));
        std::fs::create_dir_all(&project).unwrap();
        let elsewhere = project.join(format!("{id}.jsonl"));
        assert_eq!(session_cwd(&elsewhere).as_deref(), repo.to_str());
        let mut p = Parser::default();
        Cursor.observe_file(&mut p, &path);
        assert!(!p.card.interactive);

        let chat = home.join(format!(".cursor/chats/abc/{id}"));
        std::fs::create_dir_all(&chat).unwrap();
        std::fs::write(chat.join("meta.json"), r#"{"cwd":"/Users/me/repo"}"#).unwrap();
        std::fs::write(chat.join("prompt_history.json"), "[]").unwrap();
        let hex = |name: &str| {
            json!({"agentId": id, "name": name})
                .to_string()
                .bytes()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        };
        let db = Connection::open(chat.join("store.db")).unwrap();
        db.execute_batch("CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT)")
            .unwrap();
        db.execute("INSERT INTO meta VALUES('0',?1)", [hex("New Agent")])
            .unwrap();
        assert_eq!(
            Cursor.identity(&path, &Value::Null),
            Some((id.clone(), "/Users/me/repo".into()))
        );
        assert_eq!(Cursor.name(&home, &path), None);
        db.execute(
            "UPDATE meta SET value=?1",
            [hex(" Libtv Face Recognition ")],
        )
        .unwrap();
        drop(db);
        // The name is cached until the store's modification time changes.
        touch_later(&chat.join("store.db"));
        assert_eq!(
            Cursor.name(&home, &path).as_deref(),
            Some("Libtv Face Recognition")
        );
        let mut p = Parser::default();
        Cursor.observe_file(&mut p, &path);
        assert!(p.card.interactive);
        let _ = std::fs::remove_dir_all(home);
    }

    fn touch_later(file: &Path) {
        std::fs::File::options()
            .write(true)
            .open(file)
            .unwrap()
            .set_modified(SystemTime::now() + std::time::Duration::from_secs(5))
            .unwrap();
    }

    #[test]
    fn resume_names_the_chat_a_process_runs() {
        let id = "c19a5748-897d-4836-a555-fce8a7797222";
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            Cursor
                .process_session(1, &args(&["cursor-agent", "--resume", id]))
                .0
                .as_deref(),
            Some(id)
        );
        assert_eq!(
            Cursor
                .process_session(1, &args(&["cursor-agent", &format!("--resume={id}")]))
                .0
                .as_deref(),
            Some(id)
        );
        assert_eq!(
            Cursor
                .process_session(1, &args(&["cursor-agent", "resume"]))
                .0,
            None
        );
        assert!(Cursor.runs("cursor-agent", None));
        assert!(!Cursor.runs("agent", None));
    }
}
