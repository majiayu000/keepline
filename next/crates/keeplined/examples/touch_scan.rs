//! Read-only: run the real content parser over CLI logs and print, per file, the session's
//! cwd and the files it changed with when, to check collision marks against real history.
//! `cargo run -p keeplined --example touch_scan -- <claude|codex|grok> FILE...`
use keepline_core::{content::Source, model::Agent};
use keeplined::content::Parser;
use std::io::BufRead;

fn main() {
    let mut args = std::env::args().skip(1);
    let agent = args
        .next()
        .as_deref()
        .and_then(Agent::from_executable)
        .expect("first argument: claude, codex or grok");
    for path in args {
        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        let mut p = Parser::default();
        let mut cwd = String::new();
        for line in std::io::BufReader::new(file)
            .split(b'\n')
            .map_while(Result::ok)
        {
            if cwd.is_empty() {
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&line) {
                    let found = v["cwd"].as_str().or_else(|| v["payload"]["cwd"].as_str());
                    cwd = found.unwrap_or_default().to_owned();
                }
            }
            let source = Source {
                path: path.clone(),
                ..Default::default()
            };
            p.add(agent, &line, source);
        }
        if p.card.files.is_empty() {
            continue;
        }
        let files: Vec<_> = p.card.files.iter().map(|t| (&t.path, t.at)).collect();
        println!(
            "{}",
            serde_json::json!({"path": path, "cwd": cwd, "files": files})
        );
    }
}
