//! Text rules over transcript records: completion claims, user text cleanup,
//! verification commands and git commits.
use super::*;

pub(crate) fn re(s: &str) -> Regex {
    Regex::new(s).unwrap()
}
pub(super) static EXIT: LazyLock<Regex> = LazyLock::new(|| {
    re(r"(?i)(?:Process exited with code|exit[_ ]code[^0-9-]{0,8}|Exit code:)\s*(-?\d+)")
});
pub(super) static SESSION: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)session ID\s+(\d+)"));
pub(super) static TEST: LazyLock<Regex> = LazyLock::new(|| {
    re(r"(?im)^.*(?:test result:|Tests?:|\b\d+\s+(?:passed|failed|pass|fail)\b).*$")
});
pub(super) static FAILED: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)\b([1-9]\d*)\s+(?:failed|fail)\b"));
pub(super) static COMMIT: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?m)^\[[^\]\n]+\s+([a-f0-9]{7,40})\][^\n]*"));
pub(super) static PATCH: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?m)^\*\*\* (?:Update|Add|Delete) File: (.+)$"));
pub(super) static CLAIM: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?i)(?:^\s*完成(?:[。.!！，,:：]|\s*$)|已(?:完成|修复|实现|提交)|修复完成|全部.{0,12}通过|完成了|implemented|fixed|completed|all tests pass|\bdone\b)",
    )
});
pub(crate) fn claims_done(s: &str) -> bool {
    let mut fence: Option<(char, usize)> = None;
    for line in s.lines() {
        let trimmed = line.trim_start();
        let marker = trimmed
            .chars()
            .next()
            .filter(|c| matches!(c, '`' | '~'))
            .map(|c| (c, trimmed.chars().take_while(|v| *v == c).count()));
        if let Some((ch, n)) = marker.filter(|(_, n)| *n >= 3) {
            match fence {
                None => fence = Some((ch, n)),
                Some((open, len)) if open == ch && n >= len => fence = None,
                _ => (),
            };
            continue;
        }
        if fence.is_some() || trimmed.starts_with('>') {
            continue;
        }
        // A negation only cancels the claim in its own sentence: "完成。…（可选项尚未实现）"
        // still claims the requested work is done. Commas do not end a sentence, so
        // "完成，但尚未测试" stays unclaimed.
        if CLAIM.find_iter(line).any(|m| {
            !NEGATIONS
                .iter()
                .any(|no| sentence(line, m.start()).contains(no))
        }) {
            return true;
        }
    }
    false
}
pub(super) const NEGATIONS: [&str; 6] = [
    "未完成",
    "尚未",
    "没有完成",
    "还没",
    "not completed",
    "not done",
];
/// The sentence of `line` containing byte `at`.
pub(super) fn sentence(line: &str, at: usize) -> &str {
    let ends = |i: usize, c: char| {
        matches!(c, '。' | '！' | '？' | '；' | '!' | '?' | ';')
            || (c == '.' && line[i + 1..].chars().next().is_none_or(char::is_whitespace))
    };
    let start = line[..at]
        .char_indices()
        .rev()
        .find(|&(i, c)| ends(i, c))
        .map_or(0, |(i, c)| i + c.len_utf8());
    let end = line[at..]
        .char_indices()
        .find(|&(i, c)| ends(at + i, c))
        .map_or(line.len(), |(i, c)| at + i + c.len_utf8());
    &line[start..end]
}
// Strip bounded, explicit wrappers; do not infer the author of arbitrary unmarked prose.
pub(crate) fn user_text(s: &str) -> String {
    let mut clean = s.to_owned();
    if clean
        .trim_start()
        .starts_with("# Files mentioned by the user:")
    {
        clean = clean
            .split_once("## My request:")
            .map(|(_, request)| request.to_owned())
            .unwrap_or_default();
    }

    let reply_open = "<send_user_message_question_reply>";
    let reply_close = "</send_user_message_question_reply>";
    if let Some(start) = clean.find(reply_open) {
        if let Some(end) = clean[start + reply_open.len()..].find(reply_close) {
            let body = &clean[start + reply_open.len()..start + reply_open.len() + end];
            let answers = serde_json::from_str::<Vec<Value>>(body)
                .ok()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v["answer"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            clean.replace_range(
                start..start + reply_open.len() + end + reply_close.len(),
                &answers,
            );
        }
    }

    // A copied web page can carry hundreds of lines before the user's own suffix.
    if let Some(start) = clean.find("要查看键盘快捷键，按下问号") {
        if let Some(end) = clean[start..].rfind("X Corp.") {
            clean.replace_range(start..start + end + "X Corp.".len(), "\n");
        } else {
            clean.truncate(start);
        }
    }

    // Codex wraps a pasted image in `<image name=[Image #1] path="…">…</image>`; the
    // ask itself keeps "[Image #1]". Only that exact opening, so a user asking about
    // an SVG `<image>` keeps their words.
    let image_open = "<image name=[Image #";
    while let Some(start) = clean.find(image_open) {
        let Some(end) = clean[start..]
            .find("</image>")
            .map(|end| start + end + "</image>".len())
        else {
            break;
        };
        clean.replace_range(start..end, "\n");
    }

    for tag in [
        "system-reminder",
        "environment_context",
        "instructions",
        "INSTRUCTIONS",
        "skills_instructions",
        "permissions instructions",
        "collaboration_mode",
        "command-name",
        "command-message",
        "command-args",
        "local-command-caveat",
        "local-command-stdout",
        "hook-output",
        "hook-context",
        "task-notification",
        "cross-session-message",
        "side-sessions-event",
        "teammate-message",
        "context",
        "quoted_text",
        "paste",
        "pasted_text",
        "pasted_content",
        "in-app-browser-context",
        "reference",
        "file_contents",
        "skill",
        "skill_content",
        "hook-additional-context",
    ] {
        let open = format!("<{tag}");
        let close = format!("</{tag}");
        while let Some(start) = clean.find(&open) {
            let boundary = clean.as_bytes().get(start + open.len()).copied();
            if !boundary.is_some_and(|c| c == b'>' || c.is_ascii_whitespace()) {
                break;
            }
            if let Some(end) = clean[start..]
                .find(&close)
                .and_then(|end| clean[start + end..].find('>').map(|last| end + last + 1))
            {
                clean.replace_range(start..start + end, "\n");
            } else {
                clean.truncate(start);
                break;
            }
        }
    }
    // Unquoted multi-bullet assistant explanations sometimes have the user's instruction
    // appended after the final sentence. Only retain that explicit imperative suffix.
    if clean
        .lines()
        .filter(|l| l.trim_start().starts_with("* "))
        .count()
        >= 2
        && (clean.contains("所以我按") || clean.contains("所以我先") || clean.contains("我先选了"))
    {
        if let Some((_, suffix)) = clean.rsplit_once('。') {
            let suffix = suffix.trim();
            if [
                "删掉",
                "删除",
                "请",
                "帮我",
                "你帮我",
                "我希望",
                "我想",
                "你看看",
            ]
            .iter()
            .any(|s| suffix.starts_with(s))
            {
                clean = suffix.to_owned();
            } else {
                return String::new();
            }
        } else {
            return String::new();
        }
    }
    // Standalone instruction/skill payloads are generated context, not a user request.
    if CONTEXT.is_match(clean.trim()) {
        return String::new();
    }
    let mut lines = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    let mut instruction = false;
    for line in clean.lines() {
        let t = line.trim();
        if matches!(t, "## My request:" | "# User request" | "# 用户要求") {
            instruction = false;
            continue;
        }
        if t.starts_with("# AGENTS.md") || t.starts_with("# CLAUDE.md") {
            instruction = !s.contains("<INSTRUCTIONS>") && !s.contains("<instructions>");
            continue;
        }
        if instruction {
            if t.starts_with("# User request") || t.starts_with("# 用户要求") {
                instruction = false;
            }
            continue;
        }
        let marker = t
            .chars()
            .next()
            .filter(|c| matches!(c, '`' | '~'))
            .map(|c| (c, t.chars().take_while(|v| *v == c).count()));
        if let Some((c, n)) = marker.filter(|(_, n)| *n >= 3) {
            match fence {
                None => fence = Some((c, n)),
                Some((ch, len)) if c == ch && n >= len => fence = None,
                _ => (),
            }
            continue;
        }
        if fence.is_some() || t.starts_with('>') {
            continue;
        }
        if !t.is_empty()
            || lines
                .last()
                .is_some_and(|last: &&str| !last.trim().is_empty())
        {
            lines.push(line);
        }
    }
    lines.join("\n").trim().to_owned()
}
pub(super) static CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?s)^\s*(?:# Global Codex Working Agreements|\[Request interrupted by user(?: for tool use)?\]|This session is being continued from a previous conversation|Background \(from the session that started this one\):|Base directory for this skill:|<turn_aborted>|<external_codex_apps_|<skill(?:\s|>)|<available_skills>|<skills>)",
    )
});

// Parse argv without executing it. Compound commands remain unknown: their combined exit
// status cannot establish which step succeeded. Quoted arguments are safe to inspect.
pub(super) fn words(command: &str) -> Vec<String> {
    let Some(words) = shlex::split(command) else {
        return vec![];
    };
    if words
        .iter()
        .any(|w| w.contains([';', '&', '|']) || w.contains('`') || w.contains("$("))
        || command.contains('\n')
    {
        return vec![];
    }
    words
        .into_iter()
        .skip_while(|s| s.contains('=') && !s.starts_with('-'))
        .collect()
}
/// Whether a command an Agent ran checks the work: a test, type check or lint run. A
/// chain counts when one of its parts does and that part's failure reaches the exit code:
/// every link after it is `&&` (after a `;` only the last part speaks). Pipes, background
/// jobs and substitutions hide the exit code, so they never count.
pub(super) fn verification(command: &str) -> bool {
    if command.contains('\n') || command.contains('`') || command.contains("$(") {
        return false;
    }
    let Some(words) = shlex::split(command) else {
        return false;
    };
    let mut parts: Vec<Vec<String>> = vec![vec![]];
    let mut links: Vec<&str> = vec![];
    for word in words {
        match word.as_str() {
            "&&" => {
                parts.push(vec![]);
                links.push("&&");
            }
            ";" => {
                parts.push(vec![]);
                links.push(";");
            }
            // `2>&1` only moves output; it keeps the exit code.
            w if redirect(w) => {}
            w if w.contains(['|', '&']) => return false,
            w if w.ends_with(';') => {
                parts
                    .last_mut()
                    .unwrap()
                    .push(w.trim_end_matches(';').to_owned());
                parts.push(vec![]);
                links.push(";");
            }
            _ => parts.last_mut().unwrap().push(word),
        }
    }
    parts
        .iter()
        .enumerate()
        .any(|(i, part)| links[i..].iter().all(|l| *l == "&&") && verifies(part))
}
pub(super) fn redirect(w: &str) -> bool {
    let w = w.trim_start_matches(|c: char| c.is_ascii_digit());
    w.strip_prefix(">&")
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}
/// One simple command: built-in test and check runners, then the plugins' rules.
pub(super) fn verifies(part: &[String]) -> bool {
    if part
        .iter()
        .any(|w| matches!(w.as_str(), "--help" | "-h" | "--version"))
    {
        return false;
    }
    // `FOO=1 env time cargo test` is `cargo test`.
    let w: Vec<&str> = part
        .iter()
        .skip_while(|w| w.contains('=') && !w.starts_with('-'))
        .map(|s| s.rsplit('/').next().unwrap_or(s))
        .skip_while(|w| matches!(*w, "env" | "time" | "nice" | "caffeinate"))
        .collect();
    let script = |s: &str| {
        s.starts_with("test") || matches!(s, "typecheck" | "check" | "lint" | "verify" | "ci")
    };
    let built_in = match w.as_slice() {
        ["cargo", "test" | "check" | "clippy" | "nextest", ..]
        | ["go", "test" | "vet", ..]
        | ["pytest" | "vitest" | "jest" | "tsc" | "mypy" | "ruff", ..]
        | ["python" | "python3", "-m", "pytest" | "mypy", ..]
        | ["uv" | "poetry" | "pdm", "run", "pytest" | "mypy" | "ruff", ..]
        | ["npx" | "bunx" | "pnpx", "vitest" | "jest" | "tsc" | "playwright", ..]
        | ["deno" | "swift", "test", ..]
        | ["xcodebuild", .., "test"]
        | ["mvn" | "gradle" | "gradlew", "test" | "verify" | "check", ..]
        | ["make" | "just", "test" | "check" | "lint" | "ci", ..] => true,
        ["bun" | "npm" | "pnpm" | "yarn", s, ..] if script(s) => true,
        ["bun" | "npm" | "pnpm" | "yarn", "run" | "exec", s, ..]
            if script(s) || matches!(*s, "vitest" | "jest" | "tsc") =>
        {
            true
        }
        _ => false,
    };
    built_in
        || keepline_core::rule::verify()
            .iter()
            .any(|r| keepline_core::rule::matches(r, &w))
}
pub(super) fn git_commit(command: &str) -> bool {
    let w = words(command);
    w.first()
        .is_some_and(|s| s.rsplit('/').next() == Some("git"))
        && w.get(1).is_some_and(|s| s == "commit")
}
