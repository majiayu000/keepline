//! Installing the host's hook into a CLI's own settings. One command serves every CLI and
//! every Keepline: it reads the host from the terminal's environment and does nothing
//! outside a Keepline terminal, so it is installed once and never rewritten per build.
use super::{adapter, Hooks};
use keepline_core::{model::Agent, protocol::HOOK_ENV};
use serde_json::{json, Map, Value};
use std::{fs, io, path::Path};

/// The settings entry, recognised again by `HOOK_ENV` alone. It reads the payload, hands it
/// to the host in the background and returns at once: the CLI waits on some hooks (a prompt
/// is not sent until UserPromptSubmit returns), and the host's reply is never needed.
pub fn command(agent: Agent) -> String {
    format!(
        "[ -n \"${HOOK_ENV}\" ] || exit 0; p=$(cat); (printf '%s' \"$p\" | \"${HOOK_ENV}\" hook --agent {} >/dev/null 2>&1 &); exit 0",
        agent.executable()
    )
}
/// Seconds the CLI may wait on our entry; it returns at once, so this only bounds a stuck shell.
const TIMEOUT: u64 = 5;
fn ours(entry: &Value) -> bool {
    entry["command"]
        .as_str()
        .is_some_and(|c| c.contains(&format!("${HOOK_ENV}")))
}
pub fn target(agent: Agent, home: &Path) -> io::Result<Hooks> {
    adapter(agent).hooks(home).ok_or_else(|| {
        io::Error::other(format!(
            "{} 不需要 Hook：它的进程或文件已能说明对话",
            agent.name()
        ))
    })
}
fn read(path: &Path) -> io::Result<Map<String, Value>> {
    match fs::read(path) {
        Ok(bytes) if bytes.iter().all(u8::is_ascii_whitespace) => Ok(Map::new()),
        Ok(bytes) => match serde_json::from_slice(&bytes).map_err(io::Error::other)? {
            Value::Object(map) => Ok(map),
            _ => Err(io::Error::other("settings 不是 JSON 对象，未改动")),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Map::new()),
        Err(e) => Err(e),
    }
}
/// How the entries of ours compare with what `install` would write now.
#[derive(Debug, PartialEq, Eq)]
pub enum Status {
    Current,
    /// Ours, but not as `install` writes them now: one line per kind of difference.
    Outdated(Vec<String>),
    Missing,
}
/// Current when every event has exactly one entry of ours, written as `install` writes it
/// now. Reads the one settings file and never writes it.
pub fn status(agent: Agent, hooks: &Hooks) -> io::Result<Status> {
    let settings = Value::Object(read(&hooks.settings)?);
    // (event, its group's matcher, the entry) for every entry of ours.
    let mut found = vec![];
    for (event, groups) in settings["hooks"].as_object().into_iter().flatten() {
        for group in groups.as_array().into_iter().flatten() {
            for entry in group["hooks"].as_array().into_iter().flatten() {
                if ours(entry) {
                    found.push((event.as_str(), group["matcher"].as_str(), entry));
                }
            }
        }
    }
    if found.is_empty() {
        return Ok(Status::Missing);
    }
    let count = |event: &str| found.iter().filter(|(e, ..)| *e == event).count();
    let want = command(agent);
    let changed: Vec<_> = found
        .iter()
        .filter(|(.., entry)| entry["command"] != want.as_str())
        .collect();
    let mut differs = vec![];
    note(
        &mut differs,
        "缺少事件",
        hooks
            .events
            .iter()
            .filter(|e| count(e) == 0)
            .map(|e| e.to_string()),
    );
    note(
        &mut differs,
        "重复",
        hooks
            .events
            .iter()
            .filter(|e| count(e) > 1)
            .map(|e| format!("{e} 有 {} 条", count(e))),
    );
    note(
        &mut differs,
        "多余的事件",
        found
            .iter()
            .filter(|(e, ..)| !hooks.events.contains(e))
            .map(|(e, ..)| e.to_string()),
    );
    note(
        &mut differs,
        "命令和现在要写的不同",
        changed.iter().map(|(e, ..)| e.to_string()),
    );
    note(
        &mut differs,
        "指向的路径不存在",
        changed
            .iter()
            .flat_map(|(.., entry)| absent_paths(entry["command"].as_str().unwrap_or(""))),
    );
    // Claude reads an empty matcher or `*` as every case; anything else drops some.
    note(
        &mut differs,
        "限定了 matcher，会漏掉事件",
        found.iter().filter_map(|(e, matcher, _)| {
            let m = matcher.filter(|m| !m.is_empty() && *m != "*")?;
            Some(format!("{e}（{m}）"))
        }),
    );
    note(
        &mut differs,
        &format!("超时不是 {TIMEOUT} 秒"),
        found
            .iter()
            .filter(|(.., entry)| entry["timeout"] != TIMEOUT)
            .map(|(e, ..)| e.to_string()),
    );
    Ok(if differs.is_empty() {
        Status::Current
    } else {
        Status::Outdated(differs)
    })
}
/// Adds `kind：a、b` with each item once, in first-seen order; nothing when there are none.
fn note(differs: &mut Vec<String>, kind: &str, items: impl Iterator<Item = String>) {
    let mut seen: Vec<String> = vec![];
    for item in items {
        if !seen.contains(&item) {
            seen.push(item);
        }
    }
    if !seen.is_empty() {
        differs.push(format!("{kind}：{}", seen.join("、")));
    }
}
/// Absolute paths a command names that are not on disk, such as an old build's binary.
fn absent_paths(command: &str) -> Vec<String> {
    command
        .split(|c: char| c.is_whitespace() || ";|&()<>=".contains(c))
        .map(|word| word.trim_matches(|c| c == '\'' || c == '"'))
        .filter(|word| word.starts_with('/') && !Path::new(word).exists())
        .map(str::to_owned)
        .collect()
}
/// Drops our entries (and groups left empty by that) from every event, keeping the rest.
fn strip(settings: &mut Map<String, Value>) -> bool {
    let Some(Value::Object(events)) = settings.get_mut("hooks") else {
        return false;
    };
    let mut changed = false;
    for groups in events.values_mut() {
        let Some(groups) = groups.as_array_mut() else {
            continue;
        };
        for group in groups.iter_mut() {
            if let Some(entries) = group.get_mut("hooks").and_then(Value::as_array_mut) {
                let before = entries.len();
                entries.retain(|e| !ours(e));
                changed |= entries.len() != before;
            }
        }
        groups.retain(|g| g["hooks"].as_array().is_none_or(|e| !e.is_empty()));
    }
    events.retain(|_, groups| groups.as_array().is_none_or(|g| !g.is_empty()));
    if events.is_empty() {
        settings.remove("hooks");
    }
    changed
}
fn write(path: &Path, settings: &Map<String, Value>) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut text = serde_json::to_string_pretty(settings).map_err(io::Error::other)?;
    text.push('\n');
    // Beside the original, then renamed over it: a crash leaves one or the other whole.
    let temporary = path.with_extension("json.keepline-tmp");
    fs::write(&temporary, text)?;
    fs::rename(&temporary, path)
}
/// Returns whether the file changed. Outdated entries of ours are replaced, not added to.
pub fn install(agent: Agent, hooks: &Hooks) -> io::Result<bool> {
    if status(agent, hooks)? == Status::Current {
        return Ok(false);
    }
    let mut settings = read(&hooks.settings)?;
    strip(&mut settings);
    let events = settings
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| io::Error::other("settings 里的 hooks 不是对象，未改动"))?;
    for event in hooks.events {
        let groups = events
            .entry(*event)
            .or_insert_with(|| Value::Array(vec![]))
            .as_array_mut()
            .ok_or_else(|| io::Error::other(format!("hooks.{event} 不是数组，未改动")))?;
        groups.push(json!({
            "hooks": [{ "type": "command", "command": command(agent), "timeout": TIMEOUT }]
        }));
    }
    write(&hooks.settings, &settings)?;
    Ok(true)
}
pub fn uninstall(hooks: &Hooks) -> io::Result<bool> {
    let mut settings = read(&hooks.settings)?;
    if !strip(&mut settings) {
        return Ok(false);
    }
    write(&hooks.settings, &settings)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(dir: &Path) -> Hooks {
        Hooks {
            settings: dir.join("settings.json"),
            events: &["SessionStart", "Stop"],
        }
    }

    #[test]
    fn install_adds_one_entry_per_event_and_keeps_everything_else_in_order() {
        let dir = std::env::temp_dir().join(format!("kl-hooks-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let hooks = target(&dir);
        let mine = r#"{"model":"opus","hooks":{"Stop":[{"hooks":[{"type":"command","command":"remem summarize"}]}]},"alwaysThinkingEnabled":true}"#;
        fs::write(&hooks.settings, mine).unwrap();
        assert_eq!(status(Agent::Claude, &hooks).unwrap(), Status::Missing);
        assert!(install(Agent::Claude, &hooks).unwrap());
        assert_eq!(status(Agent::Claude, &hooks).unwrap(), Status::Current);
        // A second install is a no-op, not a second entry.
        assert!(!install(Agent::Claude, &hooks).unwrap());
        let text = fs::read_to_string(&hooks.settings).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        let keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, ["model", "hooks", "alwaysThinkingEnabled"]);
        assert_eq!(
            v["hooks"]["Stop"][0]["hooks"][0]["command"],
            "remem summarize"
        );
        assert_eq!(
            v["hooks"]["Stop"][1]["hooks"][0]["command"],
            command(Agent::Claude)
        );
        assert_eq!(v["hooks"]["SessionStart"].as_array().unwrap().len(), 1);
        assert!(uninstall(&hooks).unwrap());
        let v: Value = serde_json::from_str(&fs::read_to_string(&hooks.settings).unwrap()).unwrap();
        assert_eq!(
            v,
            serde_json::from_str::<Value>(mine).unwrap(),
            "uninstall restores the user's own settings"
        );
        assert!(!uninstall(&hooks).unwrap());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_missing_file_is_created_and_a_malformed_one_is_left_alone() {
        let dir = std::env::temp_dir().join(format!("kl-hooks-{}", uuid::Uuid::new_v4()));
        let hooks = target(&dir.join("nested"));
        assert!(install(Agent::Claude, &hooks).unwrap());
        assert_eq!(status(Agent::Claude, &hooks).unwrap(), Status::Current);
        fs::write(&hooks.settings, "[1,2]").unwrap();
        assert!(install(Agent::Claude, &hooks).is_err());
        assert_eq!(fs::read_to_string(&hooks.settings).unwrap(), "[1,2]");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_drifted_entry_is_named_by_what_differs_and_only_install_rewrites_it() {
        let dir = std::env::temp_dir().join(format!("kl-hooks-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let hooks = target(&dir);
        // An older build: a fixed binary path, a matcher, a longer timeout, an event since
        // dropped, and SessionStart never written.
        let old =
            format!("[ -n \"${HOOK_ENV}\" ] || exit 0; exec '/gone/keeplined' hook --agent claude");
        let drifted = json!({"hooks": {
            "Stop": [{"matcher": "startup", "hooks": [{"type": "command", "command": old, "timeout": 30}]}],
            "PreToolUse": [{"hooks": [{"type": "command", "command": command(Agent::Claude), "timeout": 5}]}],
            "SessionEnd": [{"hooks": [{"type": "command", "command": "remem summarize"}]}],
        }});
        let text = serde_json::to_string_pretty(&drifted).unwrap();
        fs::write(&hooks.settings, &text).unwrap();
        let Status::Outdated(differs) = status(Agent::Claude, &hooks).unwrap() else {
            panic!("drifted entries read as outdated");
        };
        assert_eq!(
            differs,
            [
                "缺少事件：SessionStart",
                "多余的事件：PreToolUse",
                "命令和现在要写的不同：Stop",
                "指向的路径不存在：/gone/keeplined",
                "限定了 matcher，会漏掉事件：Stop（startup）",
                "超时不是 5 秒：Stop",
            ]
        );
        assert_eq!(
            fs::read_to_string(&hooks.settings).unwrap(),
            text,
            "status only reads"
        );
        assert!(install(Agent::Claude, &hooks).unwrap());
        assert_eq!(status(Agent::Claude, &hooks).unwrap(), Status::Current);
        let v: Value = serde_json::from_str(&fs::read_to_string(&hooks.settings).unwrap()).unwrap();
        assert_eq!(v["hooks"]["SessionEnd"], drifted["hooks"]["SessionEnd"]);
        assert!(v["hooks"].get("PreToolUse").is_none());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_doubled_entry_is_outdated_and_install_leaves_one() {
        let dir = std::env::temp_dir().join(format!("kl-hooks-{}", uuid::Uuid::new_v4()));
        let hooks = target(&dir);
        assert!(install(Agent::Claude, &hooks).unwrap());
        let mut v: Value =
            serde_json::from_str(&fs::read_to_string(&hooks.settings).unwrap()).unwrap();
        let again = v["hooks"]["Stop"][0].clone();
        v["hooks"]["Stop"].as_array_mut().unwrap().push(again);
        fs::write(&hooks.settings, v.to_string()).unwrap();
        assert_eq!(
            status(Agent::Claude, &hooks).unwrap(),
            Status::Outdated(vec!["重复：Stop 有 2 条".into()])
        );
        assert!(install(Agent::Claude, &hooks).unwrap());
        assert_eq!(status(Agent::Claude, &hooks).unwrap(), Status::Current);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_command_is_a_no_op_outside_a_keepline_terminal() {
        let out = std::process::Command::new("/bin/sh")
            .args(["-c", &command(Agent::Claude)])
            .env_remove(HOOK_ENV)
            .output()
            .unwrap();
        assert!(out.status.success());
        assert!(out.stdout.is_empty() && out.stderr.is_empty());
    }
}
