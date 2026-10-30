//! Qoder CLI: `~/.qoder/projects/<encoded cwd>/<id>.jsonl`, written in Claude Code's
//! transcript format (`user`/`assistant`/`system` records naming `sessionId` and `cwd`), so
//! records are read as Claude's. A session opens with `workspace-directories` before any
//! record names its cwd. Subagents and the older `transcript/` layout sit in folders beside
//! the sessions and are not read.
use super::{claude::Claude, Adapter};
use crate::content::Parser;
use keepline_core::content::Source;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub struct Qoder;
impl Adapter for Qoder {
    fn root(&self, home: &Path) -> PathBuf {
        home.join(".qoder").join("projects")
    }
    fn runs(&self, executable: &str, _script: Option<&str>) -> bool {
        // The installed binary is versioned: `qodercli-1.1.12`.
        executable == "qodercli" || executable.starts_with("qodercli-")
    }
    fn process_session(&self, _pid: u32, args: &[String]) -> (Option<String>, bool) {
        (super::claude::claude_session_id(args), false)
    }
    fn is_transcript(&self, path: &Path) -> bool {
        path.extension().is_some_and(|e| e == "jsonl")
            && path
                .file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(|s| uuid::Uuid::parse_str(s).is_ok())
    }
    /// Only the project folders; what sits inside them belongs to a session.
    fn descend(&self, dir: &Path) -> bool {
        dir.parent()
            .and_then(Path::file_name)
            .is_some_and(|n| n == "projects")
    }
    fn identity(&self, path: &Path, value: &Value) -> Option<(String, String)> {
        if value["type"] == "workspace-directories" {
            return Some((
                value["sessionId"].as_str()?.into(),
                value["directories"][0].as_str()?.into(),
            ));
        }
        Claude.identity(path, value)
    }
    fn metadata(&self, parser: &mut Parser, record: &Value) {
        Claude.metadata(parser, record);
    }
    fn is_user(&self, record: &Value) -> bool {
        Claude.is_user(record)
    }
    fn add(&self, parser: &mut Parser, record: &Value, at: u64, source: &Source) {
        Claude.add(parser, record, at, source);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keepline_core::{content::Turn, model::Agent};
    use serde_json::json;

    #[test]
    fn a_qoder_session_reads_as_claude_and_skips_background_notices() {
        let id = "7fb630d5-8e8e-4665-816b-0fea038356cc";
        let lines = [
            json!({"type":"workspace-directories","sessionId":id,"directories":["/Users/me/repo"]}),
            json!({"type":"user","uuid":"u1","timestamp":"2026-10-27T11:42:24.524Z","cwd":"/Users/me/repo",
                "sessionId":id,"entrypoint":"cli","userType":"external","isSidechain":false,
                "origin":{"kind":"human"},"message":{"role":"user","content":[{"type":"text","text":"分析数据处理"}]}}),
            json!({"type":"user","uuid":"u2","timestamp":"2026-10-27T11:43:00Z","cwd":"/Users/me/repo",
                "sessionId":id,"entrypoint":"cli","userType":"external","isSidechain":false,"isMeta":true,
                "origin":{"kind":"task-notification"},"message":{"role":"user","content":[{"type":"text",
                "text":"[SYSTEM NOTIFICATION - NOT USER INPUT] background task finished"}]}}),
            json!({"type":"assistant","uuid":"a1","timestamp":"2026-10-27T11:44:18.479Z","cwd":"/Users/me/repo",
                "sessionId":id,"entrypoint":"cli","userType":"external","isSidechain":false,
                "message":{"role":"assistant","model":"dfmodel","stop_reason":"end_turn",
                "content":[{"type":"text","text":"处理完了。"}]}}),
        ];
        let path = Path::new("/h/.qoder/projects/-Users-me-repo").join(format!("{id}.jsonl"));
        assert!(Qoder.is_transcript(&path));
        assert!(!Qoder.is_transcript(&path.with_file_name("task-1.session.execution.jsonl")));
        assert!(Qoder.descend(Path::new("/h/.qoder/projects/-Users-me-repo")));
        assert!(!Qoder.descend(Path::new("/h/.qoder/projects/-Users-me-repo/transcript")));
        assert_eq!(
            Qoder.identity(&path, &lines[0]),
            Some((id.into(), "/Users/me/repo".into()))
        );
        let mut p = Parser::default();
        for l in &lines {
            p.add(
                Agent::Qoder,
                &serde_json::to_vec(l).unwrap(),
                Source::default(),
            );
        }
        assert!(p.card.interactive);
        assert_eq!(p.card.asks.len(), 1);
        assert_eq!(p.card.asks[0].text, "分析数据处理");
        assert_eq!(p.card.turn, Turn::Completed);
        assert_eq!(p.card.last_reply, "处理完了。");
    }

    #[test]
    fn the_versioned_binary_and_resume_name_the_process() {
        let id = "7fb630d5-8e8e-4665-816b-0fea038356cc";
        assert!(Qoder.runs("qodercli-1.1.12", None));
        assert!(!Qoder.runs("qoder", None));
        let args: Vec<String> = ["qodercli", "-r", id].map(String::from).into();
        assert_eq!(Qoder.process_session(1, &args).0.as_deref(), Some(id));
    }
}
