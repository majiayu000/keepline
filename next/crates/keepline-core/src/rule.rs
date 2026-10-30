//! Which commands count as verification (`rules.json`). A command an Agent ran that starts
//! with one of these words is a test or check run: its exit code and test counts become
//! evidence, and a turn that ran one is not judged "unverified".
//!
//! A rule is a list of words matched against the start of a command, each word compared
//! by its last path segment (`./node_modules/.bin/vitest` is `vitest`); `*` matches any one
//! word. Plugins add to the built-in list in `keeplined`; they never take one away.
use crate::agent::Loaded;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The `rules.json` format's major version.
pub const API: u32 = 1;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    api: u32,
    verify: Vec<Vec<String>>,
}

/// Whether `words` (already reduced to last path segments) start with `rule`, whose
/// words are compared by their last path segment too.
pub fn matches(rule: &[String], words: &[&str]) -> bool {
    rule.len() <= words.len()
        && rule
            .iter()
            .zip(words)
            .all(|(r, w)| r == "*" || r.rsplit('/').next() == Some(*w))
}

fn read(file: &Path) -> Result<Vec<Vec<String>>, String> {
    let bytes = std::fs::read(file).map_err(|e| format!("读不了：{e}"))?;
    let f: File = serde_json::from_slice(&bytes).map_err(|e| format!("格式不对：{e}"))?;
    if f.api != API {
        return Err(format!("规则格式是 api {}，这个版本只认 api {API}", f.api));
    }
    if f.verify.is_empty() {
        return Err("verify 至少写一条".into());
    }
    for rule in &f.verify {
        match rule.first().map(String::as_str) {
            None => return Err("verify 里有一条是空的".into()),
            // A leading wildcard would make every command a test.
            Some("*") => return Err(format!("{rule:?} 不能用 * 开头")),
            _ => {}
        }
        if rule
            .iter()
            .any(|w| w.trim().is_empty() || w.contains(char::is_whitespace))
        {
            return Err(format!("{rule:?} 每个词要单独写，不能带空格"));
        }
    }
    Ok(f.verify)
}

pub fn load(files: Vec<PathBuf>) -> (Vec<Vec<String>>, Vec<Loaded>) {
    let mut rules = Vec::new();
    let mut report = Vec::new();
    for file in files {
        let result = read(&file);
        report.push(Loaded {
            file: file.clone(),
            result: result
                .as_ref()
                .map(|r| r.iter().map(|w| w.join(" ")).collect::<Vec<_>>().join("、"))
                .map_err(Clone::clone),
        });
        if let Ok(found) = result {
            rules.extend(found);
        }
    }
    (rules, report)
}

static RULES: OnceLock<(Vec<Vec<String>>, Vec<Loaded>)> = OnceLock::new();

/// Loads the installed plugins' rules once (after `plugin::install`).
pub fn install() {
    RULES.get_or_init(|| load(crate::plugin::files(crate::plugin::Kind::Rules)));
}
/// The rules plugins added, beside the built-in ones.
pub fn verify() -> &'static [Vec<String>] {
    RULES.get().map_or(&[], |(r, _)| r.as_slice())
}
pub fn report() -> &'static [Loaded] {
    RULES.get().map_or(&[], |(_, r)| r.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_match_word_prefixes_with_wildcards() {
        let r = |w: &[&str]| w.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(matches(&r(&["just", "test"]), &["just", "test", "--all"]));
        assert!(matches(&r(&["make", "*"]), &["make", "lint"]));
        assert!(!matches(&r(&["make", "check"]), &["make"]));
        assert!(!matches(&r(&["just", "test"]), &["just", "build"]));
        assert!(matches(&r(&["./scripts/ci.sh"]), &["ci.sh"]));
    }

    #[test]
    fn bad_rule_files_are_reported() {
        let d = std::env::temp_dir().join(format!("kl-rules-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let write = |name: &str, body: &str| {
            let p = d.join(name);
            std::fs::write(&p, body).unwrap();
            p
        };
        let good = write(
            "a.json",
            r#"{"api":1,"verify":[["just","test"],["./scripts/ci.sh"]]}"#,
        );
        let star = write("b.json", r#"{"api":1,"verify":[["*","test"]]}"#);
        let space = write("c.json", r#"{"api":1,"verify":[["just test"]]}"#);
        let (rules, report) = load(vec![good, star, space]);
        assert_eq!(rules.len(), 2);
        assert_eq!(report[0].result, Ok("just test、./scripts/ci.sh".into()));
        assert!(report[1].result.as_ref().unwrap_err().contains("* 开头"));
        assert!(
            report[2]
                .result
                .as_ref()
                .unwrap_err()
                .contains("不能带空格")
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
