//! Read-only: run the real content parser over CLI logs and print, per file, the turn,
//! whether the last reply counts as a completion claim, and the reply's first line.
//! `cargo run -p keeplined --example claim_scan -- <claude|codex> FILE...`
use keepline_core::{content::Source, model::Agent};
use keeplined::content::Parser;
use std::io::BufRead;

fn main() {
    let mut args = std::env::args().skip(1);
    let agent = match args.next().as_deref() {
        Some("claude") => Agent::Claude,
        Some("codex") => Agent::Codex,
        _ => panic!("first argument: claude or codex"),
    };
    for path in args {
        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        let mut p = Parser::default();
        for line in std::io::BufReader::new(file)
            .split(b'\n')
            .map_while(Result::ok)
        {
            p.add(agent, &line, Source::default());
        }
        let first: String = p
            .card
            .last_reply
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .chars()
            .take(60)
            .collect();
        println!(
            "{}",
            serde_json::json!({"path": path, "turn": format!("{:?}", p.card.turn),
                "claim": p.card.completed_claim, "evidence": p.card.evidence.len(), "first": first})
        );
    }
}
