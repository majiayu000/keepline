//! Only the agent registry and the daemon's adapters may name a particular CLI. Everything
//! else asks them, so another CLI is a declaration or an adapter, never an edit to the
//! host, the index or the app. See docs/architecture/PLUGIN_SYSTEM.md.
use std::path::{Path, PathBuf};

/// Where naming a CLI is the point.
const OWNERS: &[&str] = &["keeplined/src/agents/", "keepline-core/src/agent.rs"];

/// Code that is not a test module but builds fixtures, each with why it may name one.
const FIXTURES: &[(&str, &str)] = &[
    (
        "keepline-app/src/workbench/visual.rs",
        "visual evidence draws a fixed Codex session",
    ),
    (
        "keepline-app/src/settings_evidence.rs",
        "settings evidence checks a saved default Agent",
    ),
];

const NAMED: &[&str] = &["Agent::Codex", "Agent::Claude", "Agent::Grok"];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Lines before the file's inline test module; a `tests.rs` file is all test.
fn production(path: &Path, source: &str) -> Vec<(usize, String)> {
    if path.file_name().is_some_and(|n| n == "tests.rs") {
        return vec![];
    }
    let lines: Vec<&str> = source.lines().collect();
    let mut out = vec![];
    for (i, line) in lines.iter().enumerate() {
        if line.trim() == "#[cfg(test)]"
            && lines[i + 1..]
                .iter()
                .find(|l| !l.trim().is_empty())
                .is_some_and(|l| l.trim_start().starts_with("mod ") && l.trim_end().ends_with('{'))
        {
            break;
        }
        out.push((i + 1, line.to_string()));
    }
    out
}

#[test]
fn only_the_registry_and_adapters_name_a_cli() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut files = vec![];
    for krate in ["keepline-core", "keeplined", "keepline-app"] {
        rust_files(&crates.join(krate).join("src"), &mut files);
    }
    assert!(files.len() > 20, "scanned too little: {}", files.len());
    let mut found = vec![];
    for file in &files {
        let relative = file
            .strip_prefix(crates)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if OWNERS.iter().any(|o| relative.starts_with(o))
            || FIXTURES.iter().any(|(f, _)| relative == *f)
        {
            continue;
        }
        let source = std::fs::read_to_string(file).unwrap();
        for (n, line) in production(file, &source) {
            let code = line.split("//").next().unwrap_or("");
            if NAMED.iter().any(|name| code.contains(name)) {
                found.push(format!("{relative}:{n}: {}", line.trim()));
            }
        }
    }
    assert!(
        found.is_empty(),
        "these name a particular CLI outside its adapter; move the behaviour into an \
         `Adapter` hook (keeplined/src/agents) or read it from `AgentSpec`:\n{}",
        found.join("\n")
    );
}

#[test]
fn the_scan_skips_test_modules_only() {
    let source = "fn a() {}\n#[cfg(test)]\nmod tests;\nfn b() { Agent::Codex }\n#[cfg(test)]\nmod tests {\n    Agent::Claude\n}\n";
    let lines: Vec<_> = production(Path::new("x.rs"), source)
        .into_iter()
        .map(|(_, l)| l)
        .collect();
    assert!(lines.iter().any(|l| l.contains("Agent::Codex")));
    assert!(!lines.iter().any(|l| l.contains("Agent::Claude")));
}
