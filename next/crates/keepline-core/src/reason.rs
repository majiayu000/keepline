//! Why a session needs you, as a short phrase: 「批准 WebSearch」「要重新登录」「合并授权」.
//!
//! The words come from a rules file, not from code: `assets/reasons.json` is compiled in,
//! and `<config_dir>/reasons.json` (same format) is tried first, so a rule is added or
//! corrected without a rebuild. The first rule of the text's kind that matches decides; a
//! text no rule takes gets no phrase (the state word alone is shown, the full text stays in
//! the hover card) and is appended once to `<config_dir>/reasons-misses.jsonl`, the list
//! the next rules are written from. `say: ""` marks a text as known with nothing to add.
//!
//! A `say` template fills `{name}` from the rule's named groups; `{name:word}` keeps the
//! first word's last path segment (`/usr/bin/ls -la` → `ls`), `{name:base}` a path's last
//! segment, and `{name:phrase}` runs the rules again on the group. Every phrase is cut to
//! `max_width` columns (a CJK character is two) so it always fits the status chip.
use regex::Regex;
use serde::Deserialize;
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// The reasons file format's major version.
pub const API: u32 = 1;
pub const BUNDLED: &str = include_str!("../assets/reasons.json");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// The Agent waits on you: a question, an approval.
    Waiting,
    /// The turn ended on an error.
    Error,
}
impl Kind {
    fn key(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Error => "error",
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    api: u32,
    version: String,
    #[serde(default)]
    max_width: Option<usize>,
    rules: Vec<RawRule>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRule {
    id: String,
    kind: Kind,
    #[serde(rename = "match")]
    pattern: String,
    say: String,
    #[serde(default)]
    #[allow(dead_code)]
    note: String,
}
pub struct Rule {
    pub id: String,
    pub kind: Kind,
    re: Regex,
    say: String,
}
pub struct Rules {
    pub version: String,
    pub max_width: usize,
    pub rules: Vec<Rule>,
}
/// What the rules made of one text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Phrase {
    /// The rule that took it; `None` is a miss.
    pub rule: Option<String>,
    /// The short words; empty when the rule has nothing to add or none matched.
    pub text: String,
}

const DEFAULT_WIDTH: usize = 24;

impl Rules {
    pub fn parse(text: &str) -> Result<Rules, String> {
        let f: File = serde_json::from_str(text).map_err(|e| format!("格式不对：{e}"))?;
        if f.api != API {
            return Err(format!("原因规则是 api {}，这个版本只认 api {API}", f.api));
        }
        let mut ids = HashSet::new();
        let rules = f
            .rules
            .into_iter()
            .map(|r| {
                if !ids.insert(r.id.clone()) {
                    return Err(format!("规则 id 重复：{}", r.id));
                }
                let re =
                    Regex::new(&r.pattern).map_err(|e| format!("规则 {} 的 match：{e}", r.id))?;
                check_template(&r.say, &re).map_err(|e| format!("规则 {} 的 say：{e}", r.id))?;
                Ok(Rule {
                    id: r.id,
                    kind: r.kind,
                    re,
                    say: r.say,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(Rules {
            version: f.version,
            max_width: f.max_width.unwrap_or(DEFAULT_WIDTH).max(8),
            rules,
        })
    }
    /// The phrase for `text`: the first non-empty line set is matched whole, so a rule may
    /// read past the first line (a Bash command split over lines).
    pub fn phrase(&self, kind: Kind, text: &str) -> Phrase {
        self.phrase_at(kind, text.trim(), 0)
    }
    fn phrase_at(&self, kind: Kind, text: &str, depth: usize) -> Phrase {
        for rule in self.rules.iter().filter(|r| r.kind == kind) {
            let Some(caps) = rule.re.captures(text) else {
                continue;
            };
            let filled = fill(&rule.say, &caps, |inner| {
                if depth < 2 {
                    self.phrase_at(kind, inner, depth + 1).text
                } else {
                    String::new()
                }
            });
            return Phrase {
                rule: Some(rule.id.clone()),
                text: fit(
                    filled.trim().trim_end_matches(['·', ' ']).trim(),
                    self.max_width,
                ),
            };
        }
        Phrase {
            rule: None,
            text: String::new(),
        }
    }
}

fn check_template(say: &str, re: &Regex) -> Result<(), String> {
    let names: HashSet<&str> = re.capture_names().flatten().collect();
    for (name, filter) in fields(say) {
        if !names.contains(name) {
            return Err(format!("{{{name}}} 不是 match 里的命名分组"));
        }
        if !matches!(filter, "" | "word" | "base" | "phrase") {
            return Err(format!("不认识的 :{filter}，只有 word、base、phrase"));
        }
    }
    Ok(())
}
/// `{name}` / `{name:filter}` fields of a template.
fn fields(say: &str) -> Vec<(&str, &str)> {
    let mut out = Vec::new();
    let mut rest = say;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        let field = &rest[open + 1..open + close];
        out.push(field.split_once(':').unwrap_or((field, "")));
        rest = &rest[open + close + 1..];
    }
    out
}
fn fill(say: &str, caps: &regex::Captures, again: impl Fn(&str) -> String) -> String {
    let mut out = String::new();
    let mut rest = say;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        out.push_str(&rest[..open]);
        let field = &rest[open + 1..open + close];
        let (name, filter) = field.split_once(':').unwrap_or((field, ""));
        let value = caps.name(name).map(|m| m.as_str().trim()).unwrap_or("");
        out.push_str(&match filter {
            "word" => base(value.split_whitespace().next().unwrap_or("")).to_owned(),
            "base" => base(value).to_owned(),
            "phrase" => again(value),
            _ => one_line(value),
        });
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}
fn base(s: &str) -> &str {
    let s = s.trim_matches(|c: char| {
        matches!(
            c,
            '"' | '\'' | '`' | ',' | '.' | ';' | '&' | '|' | '(' | ')'
        ) || c.is_whitespace()
    });
    s.trim_end_matches('/').rsplit('/').next().unwrap_or(s)
}
fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
/// Columns a string takes: CJK, fullwidth forms and emoji count two.
pub fn width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}
fn char_width(c: char) -> usize {
    let u = c as u32;
    let wide = (0x1100..=0x115F).contains(&u)
        || (0x2E80..=0xA4CF).contains(&u)
        || (0xAC00..=0xD7A3).contains(&u)
        || (0xF900..=0xFAFF).contains(&u)
        || (0xFE30..=0xFE4F).contains(&u)
        || (0xFF00..=0xFF60).contains(&u)
        || (0xFFE0..=0xFFE6).contains(&u)
        || (0x1F300..=0x1FAFF).contains(&u)
        || (0x20000..=0x3FFFD).contains(&u);
    if wide { 2 } else { 1 }
}
/// `s` cut to `max` columns, ending in `…` when cut.
pub fn fit(s: &str, max: usize) -> String {
    if width(s) <= max {
        return s.to_owned();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let w = char_width(c);
        if used + w > max - 1 {
            break;
        }
        used += w;
        out.push(c);
    }
    out.trim_end().to_owned() + "…"
}

/// The rules this process reads with: `<config_dir>/reasons.json` when it reads and
/// validates, then the bundled file. A bad user file is reported once and skipped.
pub fn rules() -> &'static Rules {
    static RULES: OnceLock<Rules> = OnceLock::new();
    RULES.get_or_init(|| load(crate::agent::config_dir().as_deref()))
}
fn load(config: Option<&Path>) -> Rules {
    let bundled = Rules::parse(BUNDLED).expect("bundled reasons.json is valid");
    let Some(path) = config.map(|d| d.join("reasons.json")) else {
        return bundled;
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return bundled;
    };
    match Rules::parse(&text) {
        Ok(mut user) => {
            // The user's rules go first; bundled ones with the same id are replaced.
            let ids: HashSet<String> = user.rules.iter().map(|r| r.id.clone()).collect();
            user.rules
                .extend(bundled.rules.into_iter().filter(|r| !ids.contains(&r.id)));
            user
        }
        Err(e) => {
            eprintln!(
                "keepline: 原因规则 {} 不可用，改用内置规则：{e}",
                path.display()
            );
            bundled
        }
    }
}

/// The phrase for `text` with the process's rules; a miss is noted for the next rules.
pub fn phrase(kind: Kind, text: &str) -> Phrase {
    let p = rules().phrase(kind, text);
    if p.rule.is_none() && !text.trim().is_empty() {
        note_miss(kind, text);
    }
    p
}

const MISSES_CAP: u64 = 1 << 20;
/// Appends `text` to `<config_dir>/reasons-misses.jsonl` once per process; the file stops
/// growing at 1 MB. Best effort: a failed write is dropped.
fn note_miss(kind: Kind, text: &str) {
    static SEEN: OnceLock<Mutex<HashSet<(Kind, String)>>> = OnceLock::new();
    let text: String = text.trim().chars().take(600).collect();
    let fresh = SEEN
        .get_or_init(Default::default)
        .lock()
        .map(|mut s| s.insert((kind, text.clone())))
        .unwrap_or(false);
    if !fresh {
        return;
    }
    let Some(path) = misses_path() else { return };
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MISSES_CAP) {
        return;
    }
    let line = serde_json::json!({
        "kind": kind.key(),
        "text": text,
        "rules": rules().version,
        "at": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    });
    let _ = std::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")));
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "{line}");
    }
}
pub fn misses_path() -> Option<PathBuf> {
    crate::agent::config_dir().map(|d| d.join("reasons-misses.jsonl"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn say(kind: Kind, text: &str) -> String {
        Rules::parse(BUNDLED).unwrap().phrase(kind, text).text
    }

    #[test]
    fn bundled_rules_read() {
        let r = Rules::parse(BUNDLED).unwrap();
        assert!(r.rules.len() > 20);
    }
    #[test]
    fn permissions_name_what_to_approve() {
        assert_eq!(
            say(
                Kind::Waiting,
                "Claude requested permissions to use WebSearch, but you haven't granted it yet."
            ),
            "批准 WebSearch"
        );
        assert_eq!(
            say(
                Kind::Waiting,
                "This Bash command contains multiple operations. The following part requires approval: /usr/bin/grep -rn x ."
            ),
            "批准 grep"
        );
        assert_eq!(
            say(
                Kind::Waiting,
                "Claude requested permissions to write to /tmp/chk.py, but you haven't granted it yet."
            ),
            "批准 改 chk.py"
        );
    }
    #[test]
    fn a_question_shows_its_short_title() {
        assert_eq!(
            say(Kind::Waiting, "【合并授权】要不要合并到 main？（是 / 否）"),
            "合并授权"
        );
        assert_eq!(
            say(Kind::Waiting, "子 Agent reviewer：【范围】改哪些？"),
            "子 Agent 范围"
        );
    }
    #[test]
    fn known_texts_with_nothing_to_add_are_covered_but_empty() {
        let p = Rules::parse(BUNDLED)
            .unwrap()
            .phrase(Kind::Waiting, "Agent 等待你的输入（Hook）");
        assert_eq!(p.rule.as_deref(), Some("generic-input"));
        assert_eq!(p.text, "");
    }
    #[test]
    fn errors_say_what_to_do() {
        assert_eq!(
            say(
                Kind::Error,
                "Failed to authenticate: OAuth session expired and could not be refreshed"
            ),
            "要重新登录"
        );
        assert_eq!(
            say(
                Kind::Error,
                "You've hit your session limit · resets 12:20am (Asia/Shanghai)"
            ),
            "额度用完 · 12:20am 恢复"
        );
        assert_eq!(
            say(
                Kind::Error,
                "Please run /login · API Error: 403 You've reached your usage limit for this billing cycle."
            ),
            "额度用完"
        );
        assert_eq!(say(Kind::Error, "终端退出码 2"), "退出码 2");
    }
    #[test]
    fn unknown_text_gets_no_phrase() {
        let p = Rules::parse(BUNDLED)
            .unwrap()
            .phrase(Kind::Error, "exec: Script completed\nWall time 0.2 seconds");
        assert_eq!(
            p,
            Phrase {
                rule: None,
                text: String::new()
            }
        );
    }
    #[test]
    fn prose_that_only_mentions_a_keyword_gets_no_phrase() {
        // A failing test's words are not a connection error.
        let r = Rules::parse(BUNDLED).unwrap();
        for text in [
            "managed_session 测试超时：timed out waiting for Settled after 30s",
            "curl returned 403 for /admin, the route is restricted",
        ] {
            assert_eq!(r.phrase(Kind::Error, text).rule, None, "{text}");
        }
    }
    #[test]
    fn phrases_are_cut_to_the_width() {
        assert_eq!(fit("一二三四五六七八九十", 10), "一二三四…");
        assert!(
            width(&say(
                Kind::Waiting,
                "【一个特别特别特别特别特别长的标题】？"
            )) <= 24
        );
    }
    #[test]
    fn a_user_file_goes_first_and_replaces_by_id() {
        let dir = std::env::temp_dir().join(format!("kl-reasons-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("reasons.json"),
            r#"{"api":1,"version":"mine","rules":[
                {"id":"login-expired","kind":"error","match":"(?i)oauth","say":"去登录"},
                {"id":"mine","kind":"error","match":"^boom","say":"炸了"}]}"#,
        )
        .unwrap();
        let r = load(Some(&dir));
        assert_eq!(r.phrase(Kind::Error, "boom!").text, "炸了");
        assert_eq!(
            r.phrase(Kind::Error, "OAuth session expired").text,
            "去登录"
        );
        assert_eq!(
            r.phrase(Kind::Error, "API Error: 529 overloaded").text,
            "服务繁忙"
        );
        std::fs::write(dir.join("reasons.json"), "{bad").unwrap();
        assert_eq!(
            load(Some(&dir)).version,
            Rules::parse(BUNDLED).unwrap().version
        );
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn a_bad_template_is_refused() {
        let e = Rules::parse(
            r#"{"api":1,"version":"x","rules":[{"id":"a","kind":"error","match":"x","say":"{nope}"}]}"#,
        );
        assert!(e.err().unwrap().contains("nope"));
    }
}
