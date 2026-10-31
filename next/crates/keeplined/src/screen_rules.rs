//! Which quiet screens show an Agent blocked on a question, as data: one rules file per CLI,
//! compiled in from `agents/screen/` and replaced by `<config_dir>/screen-rules/<cli>.json`
//! when that file reads and validates. Every answer says which rule decided it, from which
//! file and version, and the screen lines it saw; a screen no rule takes says why.
//!
//! A rule hangs on one anchor line (the lowest that fits) and a numbered menu next to it.
//! Only two strict shapes ship; anything else is no evidence, never a guess:
//! - an approval: "Do you want to …?" / "Would you like to …?" followed by a numbered
//!   option list whose first choice is "Yes", ending at the bottom of the screen;
//! - a choice question: a cursor-marked numbered option above a footer that offers both
//!   "Enter to select" and "Esc to cancel".
use keepline_core::{content::short, model::Agent};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// One CLI's rules, tried in order; the first that matches decides.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub id: String,
    pub version: String,
    /// Non-empty lines read from the bottom; a menu further up was answered long ago.
    pub bottom_lines: usize,
    pub rules: Vec<Rule>,
    #[serde(skip)]
    pub source: Source,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Source {
    #[default]
    Bundled,
    Override(PathBuf),
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    /// What the user reads: 审批, 选择题.
    pub name: String,
    /// The state a match reports; only "waiting" is read from a screen.
    pub state: String,
    #[serde(default)]
    pub note: String,
    /// The line the rule hangs on; the lowest one that fits counts.
    pub anchor: Line,
    /// The anchor must be among this many bottom lines.
    #[serde(default)]
    pub anchor_within: Option<usize>,
    pub menu: Menu,
    pub ask: Ask,
    /// Lines that rule the screen out wherever they appear in the region.
    #[serde(default)]
    pub not: Vec<Line>,
}
/// Where the question shown to the user is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ask {
    /// The anchor line itself.
    Anchor,
    /// The nearest line above option 1 that is not an option.
    AboveMenu,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Below,
    Above,
}
/// The numbered menu ("❯ 1. Yes") next to the anchor.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Menu {
    pub side: Side,
    /// Option 1 is among this many lines next to the anchor.
    #[serde(default)]
    pub first_within: Option<usize>,
    #[serde(default)]
    pub first_starts_with: Option<String>,
    /// The lowest option on screen is below the anchor and among this many bottom lines:
    /// output after the menu means it was already answered and scrolled up.
    #[serde(default)]
    pub last_within: Option<usize>,
    /// An option from option 1 to the menu's end carries the cursor.
    #[serde(default)]
    pub cursor: bool,
}
/// A line test; every condition given must hold.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Line {
    /// Any one of these prefixes.
    #[serde(default)]
    pub starts_with: Vec<String>,
    #[serde(default)]
    pub ends_with: Option<String>,
    /// All of these, ignoring ASCII case.
    #[serde(default)]
    pub contains: Vec<String>,
}
impl Line {
    fn matches(&self, line: &str) -> bool {
        (self.starts_with.is_empty() || self.starts_with.iter().any(|p| line.starts_with(p)))
            && self.ends_with.as_ref().is_none_or(|s| line.ends_with(s))
            && self
                .contains
                .iter()
                .all(|n| contains_ignore_ascii_case(line, n))
    }
    fn is_empty(&self) -> bool {
        self.starts_with.is_empty() && self.ends_with.is_none() && self.contains.is_empty()
    }
}
/// `needle` is already lower case (`Rules::parse` folds it).
fn contains_ignore_ascii_case(line: &str, needle: &str) -> bool {
    let (hay, needle) = (line.as_bytes(), needle.as_bytes());
    needle.is_empty()
        || hay
            .windows(needle.len())
            .any(|w| w.eq_ignore_ascii_case(needle))
}

impl Rules {
    pub fn parse(text: &str, source: Source) -> Result<Rules, String> {
        let mut rules: Rules = serde_json::from_str(text).map_err(|e| e.to_string())?;
        rules.source = source;
        if rules.version.trim().is_empty() {
            return Err("version 不能为空".into());
        }
        if !(1..=200).contains(&rules.bottom_lines) {
            return Err("bottom_lines 要在 1..=200".into());
        }
        if rules.rules.is_empty() {
            return Err("rules 不能为空".into());
        }
        for (i, r) in rules.rules.iter_mut().enumerate() {
            let at = |what: &str| format!("rules[{i}] {}: {what}", r.id);
            if r.id.is_empty() || r.name.is_empty() {
                return Err(format!("rules[{i}]: id 和 name 不能为空"));
            }
            if r.state != "waiting" {
                return Err(at("state 只能是 waiting"));
            }
            if r.anchor.is_empty() || r.not.iter().any(Line::is_empty) {
                return Err(at("anchor 和 not 的每一项至少要有一个条件"));
            }
            for line in std::iter::once(&mut r.anchor).chain(r.not.iter_mut()) {
                for n in &mut line.contains {
                    n.make_ascii_lowercase();
                }
            }
        }
        let mut ids: Vec<&str> = rules.rules.iter().map(|r| r.id.as_str()).collect();
        ids.sort_unstable();
        if ids.windows(2).any(|w| w[0] == w[1]) {
            return Err("rule id 重复".into());
        }
        Ok(rules)
    }
    /// "claude 内置 2026.10.31.1", or the override file it came from.
    pub fn origin(&self) -> String {
        match &self.source {
            Source::Bundled => format!("{} 内置 {}", self.id, self.version),
            Source::Override(path) => format!(
                "{} 覆盖文件 {} {}",
                self.id,
                path.file_name().unwrap_or_default().to_string_lossy(),
                self.version
            ),
        }
    }
}

/// The rules compiled into this binary for `agent`.
pub fn bundled(agent: Agent) -> &'static Rules {
    loaded(agent, &BUNDLED, |agent| {
        compile(crate::agents::adapter(agent).screen_rules())
    })
}
/// The rules a CLI without its own file is read with, as every CLI was before rules files.
pub fn default_rules() -> &'static Rules {
    static DEFAULT: OnceLock<Rules> = OnceLock::new();
    DEFAULT.get_or_init(|| compile(crate::agents::DEFAULT_SCREEN_RULES))
}
fn compile(text: &str) -> Rules {
    Rules::parse(text, Source::Bundled).expect("bundled screen rules are valid")
}
/// The rules the daemon reads `agent`'s screens with: the user's override when it reads
/// and validates, otherwise the bundled ones. Read once per process; a bad file is
/// reported once and never fatal.
pub fn rules(agent: Agent) -> &'static Rules {
    loaded(agent, &ACTIVE, |agent| {
        active(agent, keepline_core::agent::config_dir().as_deref())
    })
}
fn active(agent: Agent, config: Option<&Path>) -> Rules {
    match config.and_then(|dir| load_override(agent, dir)) {
        Some(Ok(rules)) => rules,
        Some(Err(e)) => {
            eprintln!("keeplined: 画面规则覆盖文件不可用，改用内置规则：{e}");
            compile(crate::agents::adapter(agent).screen_rules())
        }
        None => compile(crate::agents::adapter(agent).screen_rules()),
    }
}
/// `<dir>/screen-rules/<cli>.json`: `None` when absent, an error naming the file otherwise.
pub fn load_override(agent: Agent, dir: &Path) -> Option<Result<Rules, String>> {
    let stem = agent.key().to_lowercase();
    let path = dir.join("screen-rules").join(format!("{stem}.json"));
    let text = std::fs::read_to_string(&path).ok()?;
    Some(
        Rules::parse(&text, Source::Override(path.clone()))
            .and_then(|r| {
                (r.id == stem)
                    .then_some(r)
                    .ok_or_else(|| format!("id 要写 {stem}"))
            })
            .map_err(|e| format!("{}: {e}", path.display())),
    )
}
type Cache = OnceLock<Mutex<HashMap<Agent, &'static Rules>>>;
static BUNDLED: Cache = OnceLock::new();
static ACTIVE: Cache = OnceLock::new();
/// Compiled once per CLI and kept for the process, like the agent registry.
fn loaded(
    agent: Agent,
    cache: &'static Cache,
    load: impl FnOnce(Agent) -> Rules,
) -> &'static Rules {
    let mut map = cache
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    map.entry(agent)
        .or_insert_with(|| Box::leak(Box::new(load(agent))))
}

/// Why one rule did not take the screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Miss {
    NoAnchor,
    AnchorHigh,
    Excluded(String),
    NoFirst,
    FirstMismatch,
    Answered,
    NoCursor,
    NoAsk,
}
impl Miss {
    pub fn id(&self) -> &'static str {
        match self {
            Self::NoAnchor => "no_anchor",
            Self::AnchorHigh => "anchor_not_at_bottom",
            Self::Excluded(_) => "excluded",
            Self::NoFirst => "no_option_1",
            Self::FirstMismatch => "option_1_mismatch",
            Self::Answered => "output_after_menu",
            Self::NoCursor => "no_cursor",
            Self::NoAsk => "no_question",
        }
    }
    pub fn describe(&self, rule: &Rule) -> String {
        match self {
            Self::NoAnchor => "底部没有这类提示".into(),
            Self::AnchorHigh => "提示不在最底部，已经滚上去了".into(),
            Self::Excluded(line) => format!("画面上有排除行「{}」", short(line, 40)),
            Self::NoFirst => "提示旁边没有 1 号选项".into(),
            Self::FirstMismatch => format!(
                "1 号选项不是「{}」开头",
                rule.menu.first_starts_with.as_deref().unwrap_or("")
            ),
            Self::Answered => "选项下面还有输出，像是已经答过了".into(),
            Self::NoCursor => "选项上没有光标，不是待选的菜单".into(),
            Self::NoAsk => "选项上面没有问题".into(),
        }
    }
}
/// A rule that took the screen: the question it shows and the lines it read, verbatim
/// after the frame is trimmed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub rule: usize,
    pub question: String,
    pub evidence: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct Explained {
    pub rules: &'static Rules,
    /// The rule that matched, or why each rule did not, in rule order.
    pub found: Result<Hit, Vec<Miss>>,
}
impl PartialEq for Explained {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.rules, other.rules) && self.found == other.found
    }
}
impl Eq for Explained {}
impl Explained {
    pub fn waiting(&self) -> bool {
        self.found.is_ok()
    }
    pub fn question(&self) -> Option<&str> {
        self.found.as_ref().ok().map(|h| h.question.as_str())
    }
    pub fn into_question(self) -> Option<String> {
        self.found.ok().map(|h| h.question)
    }
    /// The question, when the rule that took the screen is the approval one.
    pub fn approval(&self) -> Option<&str> {
        self.rule()
            .filter(|r| r.id == "approval")
            .and(self.question())
    }
    /// Whether the menu's cursor is on its first option, the one Enter would choose.
    pub fn cursor_on_first(&self) -> bool {
        self.found.as_ref().is_ok_and(|h| {
            h.evidence
                .iter()
                .filter_map(|l| option(l))
                .any(|(cursor, n, _)| cursor && n == 1)
        })
    }
    pub fn rule(&self) -> Option<&'static Rule> {
        let rules = self.rules;
        self.found.as_ref().ok().map(|h| &rules.rules[h.rule])
    }
    pub fn state(&self) -> &'static str {
        if self.waiting() {
            "waiting"
        } else {
            "idle"
        }
    }
    /// The 「为什么是这个状态」 lines for a match: the rule and what it saw.
    pub fn detail(&self) -> String {
        let (Ok(hit), Some(rule)) = (&self.found, self.rule()) else {
            return self.fallback();
        };
        let mut seen: Vec<String> = hit.evidence.iter().take(6).map(|l| short(l, 60)).collect();
        if hit.evidence.len() > seen.len() {
            seen.push("…".into());
        }
        format!(
            "规则：{} {}（{}）\n看到：{}",
            rule.id,
            rule.name,
            self.rules.origin(),
            seen.join(" / ")
        )
    }
    /// Why the screen counts as no prompt: one line, naming the rules that came closest.
    pub fn fallback(&self) -> String {
        let Err(misses) = &self.found else {
            return String::new();
        };
        let near: Vec<String> = misses
            .iter()
            .zip(&self.rules.rules)
            .filter(|(m, _)| **m != Miss::NoAnchor)
            .map(|(m, r)| format!("{}——{}", r.name, m.describe(r)))
            .collect();
        let why = if near.is_empty() {
            let names: Vec<&str> = self.rules.rules.iter().map(|r| r.name.as_str()).collect();
            format!("底部没有{}的提示", names.join("或"))
        } else {
            near.join("；")
        };
        format!("没认出提问框（{why}；规则 {}）", self.rules.origin())
    }
    pub fn to_json(&self) -> serde_json::Value {
        let rule = self.rule();
        let source = match &self.rules.source {
            Source::Bundled => serde_json::json!("bundled"),
            Source::Override(path) => serde_json::json!(path.display().to_string()),
        };
        let misses: Vec<_> = match &self.found {
            Ok(_) => vec![],
            Err(misses) => misses
                .iter()
                .zip(&self.rules.rules)
                .map(|(m, r)| serde_json::json!({"rule": r.id, "reason": m.id(), "why": m.describe(r)}))
                .collect(),
        };
        serde_json::json!({
            "state": self.state(),
            "rules": self.rules.id,
            "version": self.rules.version,
            "source": source,
            "rule": rule.map(|r| r.id.as_str()),
            "question": self.question(),
            "evidence": self.found.as_ref().map(|h| h.evidence.clone()).unwrap_or_default(),
            "fallback": (!self.waiting()).then(|| self.fallback()),
            "misses": misses,
        })
    }
}
/// Reads the bottom of a quiet screen with `rules`.
pub fn explain(rules: &'static Rules, tail: &str) -> Explained {
    const FRAME: &[char] = &[
        '│', '┃', '║', '╭', '╮', '╰', '╯', '┌', '┐', '└', '┘', '─', '━', '═',
    ];
    let lines: Vec<&str> = tail
        .lines()
        .map(|l| l.trim().trim_matches(FRAME).trim())
        .filter(|l| !l.is_empty() && !l.chars().all(|c| FRAME.contains(&c)))
        .collect();
    let lines = &lines[lines.len().saturating_sub(rules.bottom_lines)..];
    let mut misses = Vec::new();
    for (i, rule) in rules.rules.iter().enumerate() {
        match check(rule, lines) {
            Ok((ask, span)) => {
                let found = Ok(Hit {
                    rule: i,
                    question: lines[ask].to_owned(),
                    evidence: lines[span].iter().map(|l| (*l).to_owned()).collect(),
                });
                return Explained { rules, found };
            }
            Err(miss) => misses.push(miss),
        }
    }
    Explained {
        rules,
        found: Err(misses),
    }
}
/// A numbered menu entry: whether a cursor marks it, its number, its text.
fn option(line: &str) -> Option<(bool, u32, &str)> {
    let rest = line.trim_start_matches(['❯', '›', '>', '▶', ' ']);
    let cursor = rest.len() != line.len();
    let (n, text) = rest.split_once(". ")?;
    let n = n.parse::<u32>().ok().filter(|n| (1..=9).contains(n))?;
    Some((cursor, n, text.trim()))
}
fn is_first(line: &str) -> bool {
    option(line).is_some_and(|(_, n, _)| n == 1)
}
/// The question's line and the lines read, or why the rule does not take the screen.
fn check(rule: &Rule, lines: &[&str]) -> Result<(usize, std::ops::RangeInclusive<usize>), Miss> {
    let anchor = lines
        .iter()
        .rposition(|l| rule.anchor.matches(l))
        .ok_or(Miss::NoAnchor)?;
    if rule.anchor_within.is_some_and(|n| anchor + n < lines.len()) {
        return Err(Miss::AnchorHigh);
    }
    if let Some(l) = lines.iter().find(|l| rule.not.iter().any(|n| n.matches(l))) {
        return Err(Miss::Excluded((*l).to_owned()));
    }
    let menu = &rule.menu;
    let within = menu.first_within.unwrap_or(usize::MAX);
    let (first, end) = match menu.side {
        Side::Below => {
            let first = lines[anchor + 1..]
                .iter()
                .take(within)
                .position(|l| is_first(l))
                .map(|p| anchor + 1 + p)
                .ok_or(Miss::NoFirst)?;
            (first, first)
        }
        Side::Above => {
            let first = lines[..anchor]
                .iter()
                .rposition(|l| is_first(l))
                .filter(|f| anchor - f <= within)
                .ok_or(Miss::NoFirst)?;
            (first, anchor)
        }
    };
    if let Some(prefix) = &menu.first_starts_with {
        if !option(lines[first]).is_some_and(|(_, _, text)| text.starts_with(prefix.as_str())) {
            return Err(Miss::FirstMismatch);
        }
    }
    let mut end = end;
    if let Some(n) = menu.last_within {
        let last = lines.iter().rposition(|l| option(l).is_some());
        match last {
            Some(last) if last > anchor && last + n >= lines.len() => end = end.max(last),
            _ => return Err(Miss::Answered),
        }
    }
    // Above the anchor the menu stops before it; below, at its last option.
    let options = match menu.side {
        Side::Above => &lines[first..end],
        Side::Below => &lines[first..=end],
    };
    if menu.cursor && !options.iter().any(|l| option(l).is_some_and(|(c, _, _)| c)) {
        return Err(Miss::NoCursor);
    }
    let ask = match rule.ask {
        Ask::Anchor => anchor,
        Ask::AboveMenu => lines[..first]
            .iter()
            .rposition(|l| option(l).is_none())
            .ok_or(Miss::NoAsk)?,
    };
    Ok((ask, ask.min(first)..=end.max(anchor)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_registered_cli_has_valid_bundled_rules() {
        for agent in Agent::all() {
            let rules = bundled(agent);
            assert!(!rules.rules.is_empty(), "{}", agent.key());
        }
        assert_eq!(default_rules().id, "claude");
    }
    #[test]
    fn an_override_wins_only_when_it_reads_and_validates() {
        let dir = std::env::temp_dir().join(format!("keepline-rules-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("screen-rules")).unwrap();
        let agent = Agent::all()[0];
        let stem = agent.key().to_lowercase();
        let file = dir.join("screen-rules").join(format!("{stem}.json"));
        assert!(load_override(agent, &dir).is_none(), "no file, no override");

        assert_eq!(active(agent, Some(&dir)).source, Source::Bundled);

        std::fs::write(&file, "{ not json").unwrap();
        assert!(load_override(agent, &dir).unwrap().is_err());
        // A broken file is reported and the bundled rules read the screen.
        let fallback = active(agent, Some(&dir));
        assert_eq!(fallback.source, Source::Bundled);
        assert_eq!(fallback.version, bundled(agent).version);
        let wrong_state = format!(
            r#"{{"id":"{stem}","version":"1","bottom_lines":16,"rules":[{{"id":"x","name":"x","state":"working","anchor":{{"contains":["a"]}},"menu":{{"side":"below"}},"ask":"anchor"}}]}}"#
        );
        std::fs::write(&file, wrong_state).unwrap();
        let err = load_override(agent, &dir).unwrap().unwrap_err();
        assert!(err.contains("state"), "{err}");

        // A valid override that also refuses screens showing a transcript viewer.
        let text = crate::agents::DEFAULT_SCREEN_RULES
            .replace("\"id\": \"claude\"", &format!("\"id\": \"{stem}\""))
            .replace("\"version\": \"2026.10.31.1\"", "\"version\": \"test.1\"")
            .replace(
                "\"ask\": \"anchor\"",
                "\"ask\": \"anchor\", \"not\": [{ \"contains\": [\"Showing detailed transcript\"] }]",
            );
        std::fs::write(&file, text).unwrap();
        let rules: &'static Rules = Box::leak(Box::new(active(agent, Some(&dir))));
        assert_eq!(rules.version, "test.1");
        assert_eq!(rules.source, Source::Override(file.clone()));
        let approval = "Do you want to proceed?\n❯ 1. Yes\n  2. No\n";
        assert!(explain(rules, approval).waiting());
        let viewer = format!("{approval}showing detailed transcript · ctrl+o to toggle\n");
        let e = explain(rules, &viewer);
        assert!(!e.waiting());
        assert!(matches!(&e.found, Err(m) if matches!(m[0], Miss::Excluded(_))));
        assert!(e.fallback().contains("排除行"), "{}", e.fallback());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn a_match_names_its_rule_and_lines_and_a_miss_says_why() {
        let rules = default_rules();
        let e = explain(
            rules,
            "⏺ Bash(ls)\n│ Do you want to proceed? │\n│ ❯ 1. Yes │\n│   2. No │\n",
        );
        assert_eq!(e.rule().map(|r| r.id.as_str()), Some("approval"));
        assert_eq!(
            e.found.as_ref().unwrap().evidence,
            ["Do you want to proceed?", "❯ 1. Yes", "2. No"]
        );
        assert_eq!(
            e.detail(),
            "规则：approval 审批（claude 内置 2026.10.31.1）\n看到：Do you want to proceed? / ❯ 1. Yes / 2. No"
        );
        let idle = explain(rules, "$ ls\nsrc\n$ ");
        assert_eq!(
            idle.fallback(),
            "没认出提问框（底部没有审批或选择题的提示；规则 claude 内置 2026.10.31.1）"
        );
        let answered = explain(
            rules,
            "Do you want to proceed?\n❯ 1. Yes\n  2. No\n⏺ Bash(ls)\n  ⎿ a\n  ⎿ b\n  ⎿ c\n  ⎿ d\n",
        );
        assert!(
            answered.fallback().contains("审批——选项下面还有输出"),
            "{}",
            answered.fallback()
        );
        assert_eq!(
            answered.to_json()["misses"][0]["reason"],
            "output_after_menu"
        );
    }
}
