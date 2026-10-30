//! Replays every saved screen in `fixtures/screens/<cli>/` through that CLI's bundled rules
//! and the default rules `screen_wait` reads with; `expected.json` beside them names the
//! state, and for a prompt the rule and the question. Mismatches are listed by file name.
use keepline_core::model::Agent;
use keeplined::screen_rules;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/screens")
}

/// Every fixture as (cli directory, file name, screen, expectation).
fn fixtures() -> Vec<(String, String, String, serde_json::Value)> {
    let mut out = vec![];
    let mut dirs: Vec<_> = std::fs::read_dir(root())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    for dir in dirs {
        let cli = dir.file_name().unwrap().to_string_lossy().into_owned();
        let expected: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("expected.json")).unwrap())
                .unwrap();
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "txt"))
            .collect();
        files.sort();
        let names: Vec<String> = files
            .iter()
            .map(|f| f.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        for key in expected.as_object().unwrap().keys() {
            assert!(
                names.contains(key),
                "{cli}/expected.json 列了不存在的 {key}"
            );
        }
        for (file, name) in files.iter().zip(names) {
            let want = expected[&name].clone();
            assert!(!want.is_null(), "{cli}/{name} 没写进 expected.json");
            out.push((
                cli.clone(),
                name,
                std::fs::read_to_string(file).unwrap(),
                want,
            ));
        }
    }
    out
}

fn agent(cli: &str) -> Agent {
    Agent::all()
        .into_iter()
        .find(|a| a.key().to_lowercase() == cli)
        .unwrap_or_else(|| panic!("fixtures/screens/{cli} 不是已注册的 CLI"))
}

#[test]
fn every_saved_screen_reads_as_its_expected_state() {
    let fixtures = fixtures();
    let mut wrong = vec![];
    for (cli, name, screen, want) in &fixtures {
        for (label, rules) in [
            ("own", screen_rules::bundled(agent(cli))),
            ("default", screen_rules::default_rules()),
        ] {
            let e = screen_rules::explain(rules, screen);
            let got = (e.state(), e.rule().map(|r| r.id.as_str()), e.question());
            let expected = (
                want["state"].as_str().unwrap(),
                want["rule"].as_str(),
                want["question"].as_str(),
            );
            if got != expected {
                wrong.push(format!(
                    "{cli}/{name} ({label} rules {}): got {got:?}, want {expected:?}; {}",
                    rules.version,
                    e.fallback()
                ));
            }
        }
        // The function the old tests call answers the same.
        assert_eq!(
            keeplined::recognition::screen_wait(screen).as_deref(),
            want["question"].as_str(),
            "{cli}/{name}"
        );
    }
    let negatives = fixtures.iter().filter(|f| f.3["state"] == "idle").count();
    println!(
        "{} screens, {} negative, {} mismatches",
        fixtures.len(),
        negatives,
        wrong.len()
    );
    assert!(negatives >= 5);
    assert!(wrong.is_empty(), "mismatches:\n{}", wrong.join("\n"));
}
