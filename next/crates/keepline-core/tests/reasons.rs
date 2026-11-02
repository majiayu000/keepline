//! The reasons rules against real texts (`reasons-corpus.jsonl`, mined from Claude Code and
//! Codex logs by `scripts/mine-reasons.py`, sanitized). Weighted by how often each text
//! happened: at least 80% must get a rule, and every phrase must fit the status chip.
use keepline_core::reason::{BUNDLED, Kind, Rules, width};

#[test]
fn rules_cover_the_corpus() {
    let rules = Rules::parse(BUNDLED).unwrap();
    let corpus = include_str!("reasons-corpus.jsonl");
    let (mut hit, mut total) = (0u64, 0u64);
    let mut misses = Vec::new();
    for line in corpus.lines().filter(|l| !l.trim().is_empty()) {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        let kind = match row["kind"].as_str().unwrap() {
            "waiting" => Kind::Waiting,
            _ => Kind::Error,
        };
        let text = row["text"].as_str().unwrap();
        let count = row["count"].as_u64().unwrap_or(1);
        let p = rules.phrase(kind, text);
        assert!(
            width(&p.text) <= rules.max_width,
            "{text} → {} is wider than {}",
            p.text,
            rules.max_width
        );
        total += count;
        if p.rule.is_some() {
            hit += count;
        } else {
            misses.push(format!("{count:>4}  {text}"));
        }
    }
    let rate = hit as f64 / total as f64;
    eprintln!(
        "reasons: {hit}/{total} = {:.1}% covered; misses:\n{}",
        rate * 100.0,
        misses.join("\n")
    );
    assert!(
        rate >= 0.8,
        "only {:.1}% of the corpus gets a phrase",
        rate * 100.0
    );
}

/// `cargo test -p keepline-core --test reasons -- --ignored --nocapture` prints every
/// corpus text with its phrase, to read the words, not only count the hits.
#[test]
#[ignore]
fn print_the_corpus_phrases() {
    let rules = Rules::parse(BUNDLED).unwrap();
    for line in include_str!("reasons-corpus.jsonl")
        .lines()
        .filter(|l| !l.trim().is_empty())
    {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        let kind = if row["kind"] == "waiting" {
            Kind::Waiting
        } else {
            Kind::Error
        };
        let text = row["text"].as_str().unwrap();
        let p = rules.phrase(kind, text);
        let rule = p.rule.unwrap_or_else(|| "—".into());
        let short: String = text.chars().take(90).collect();
        println!("{:<18} {:<24} {short}", rule, p.text);
    }
}
