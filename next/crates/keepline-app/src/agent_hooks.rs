//! The Agent CLIs' hook entries, as `keeplined hooks status` reads them. The app never
//! edits those settings itself: installing runs `keeplined hooks install`, the command
//! line's own path, and only when the user clicks for it. Both block, so callers run them
//! off the UI thread.
use std::{io, process::Command};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Current,
    Outdated,
    Missing,
    /// The settings file could not be read; `differences` holds why.
    Unreadable,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Hook {
    /// The CLI's executable name, which `--agent` takes.
    pub agent: String,
    pub name: String,
    pub settings: String,
    pub state: State,
    pub differences: Vec<String>,
}

/// The daemon beside this app, as `lifecycle` starts it.
fn keeplined(args: &[&str]) -> io::Result<Vec<u8>> {
    let out = Command::new(std::env::current_exe()?.with_file_name("keeplined"))
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()?;
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr);
        return Err(io::Error::other(why.trim().to_owned()));
    }
    Ok(out.stdout)
}
pub fn check() -> io::Result<Vec<Hook>> {
    parse(&keeplined(&["hooks", "status", "--json"])?)
}
pub fn install(agent: &str) -> io::Result<()> {
    keeplined(&["hooks", "install", "--agent", agent]).map(drop)
}
fn parse(bytes: &[u8]) -> io::Result<Vec<Hook>> {
    let rows: Vec<serde_json::Value> = serde_json::from_slice(bytes).map_err(io::Error::other)?;
    Ok(rows
        .iter()
        .map(|row| {
            let text = |key: &str| row[key].as_str().unwrap_or_default().to_owned();
            Hook {
                agent: text("agent"),
                name: text("name"),
                settings: text("settings"),
                state: match row["state"].as_str() {
                    Some("current") => State::Current,
                    Some("outdated") => State::Outdated,
                    Some("missing") => State::Missing,
                    _ => State::Unreadable,
                },
                differences: row["differences"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|d| d.as_str().map(str::to_owned))
                    .collect(),
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_json_reads_back_every_state() {
        let json = r#"[
            {"agent":"claude","name":"Claude Code","settings":"/h/.claude/settings.json","state":"outdated","differences":["缺少事件：SessionEnd"]},
            {"agent":"claude","name":"Claude Code","settings":"/h","state":"current","differences":[]},
            {"agent":"claude","name":"Claude Code","settings":"/h","state":"missing","differences":[]},
            {"agent":"claude","name":"Claude Code","settings":"/h","state":"error","differences":["不是 JSON"]}
        ]"#;
        let hooks = parse(json.as_bytes()).unwrap();
        assert_eq!(
            hooks.iter().map(|h| h.state).collect::<Vec<_>>(),
            [
                State::Outdated,
                State::Current,
                State::Missing,
                State::Unreadable
            ]
        );
        assert_eq!(hooks[0].agent, "claude");
        assert_eq!(hooks[0].differences, ["缺少事件：SessionEnd"]);
        assert!(parse(b"not json").is_err());
    }
}
