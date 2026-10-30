//! Times the screen rules against the hand-written `screen_wait` they replaced, over every
//! saved screen in `tests/fixtures/screens`. Both must answer alike on each screen.
//!
//! cargo run --release -p keeplined --example screen_bench -- [iterations]
use keeplined::screen_rules;
use std::hint::black_box;
use std::path::Path;
use std::time::Instant;

fn main() {
    let iterations: usize = std::env::args()
        .nth(1)
        .and_then(|n| n.parse().ok())
        .unwrap_or(20_000);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/screens");
    let mut screens = vec![];
    for dir in std::fs::read_dir(&root).unwrap().flatten() {
        for file in std::fs::read_dir(dir.path()).unwrap().flatten() {
            if file.path().extension().is_some_and(|e| e == "txt") {
                screens.push(std::fs::read_to_string(file.path()).unwrap());
            }
        }
    }
    let rules = screen_rules::default_rules();
    for s in &screens {
        assert_eq!(
            legacy(s),
            screen_rules::explain(rules, s).into_question(),
            "{s}"
        );
    }
    let calls = (iterations * screens.len()) as f64;
    // Alternate rounds so neither side always runs on a warmer cache.
    let (mut old, mut new) = (0u128, 0u128);
    for _ in 0..5 {
        let at = Instant::now();
        for _ in 0..iterations / 5 {
            for s in &screens {
                black_box(legacy(black_box(s)));
            }
        }
        old += at.elapsed().as_nanos();
        let at = Instant::now();
        for _ in 0..iterations / 5 {
            for s in &screens {
                black_box(screen_rules::explain(rules, black_box(s)));
            }
        }
        new += at.elapsed().as_nanos();
    }
    // As the host calls it: the per-CLI lookup under its lock, then the match.
    let agent = keepline_core::model::Agent::all()[0];
    let at = Instant::now();
    for _ in 0..iterations {
        for s in &screens {
            black_box(screen_rules::explain(
                screen_rules::bundled(black_box(agent)),
                black_box(s),
            ));
        }
    }
    let looked_up = at.elapsed().as_nanos() as f64 / calls;
    let (old, new) = (old as f64 / calls, new as f64 / calls);
    println!(
        "{} screens × {iterations}: old screen_wait {old:.0} ns/call, rules explain {new:.0} ns/call \
         (ratio {:.2}), with per-CLI lookup {looked_up:.0} ns/call (ratio {:.2})",
        screens.len(),
        new / old,
        looked_up / old
    );
}

/// `recognition::screen_wait` as it was before the rules files, verbatim.
fn legacy(tail: &str) -> Option<String> {
    const FRAME: &[char] = &[
        '│', '┃', '║', '╭', '╮', '╰', '╯', '┌', '┐', '└', '┘', '─', '━', '═',
    ];
    let lines: Vec<&str> = tail
        .lines()
        .map(|l| l.trim().trim_matches(FRAME).trim())
        .filter(|l| !l.is_empty() && !l.chars().all(|c| FRAME.contains(&c)))
        .collect();
    let bottom = lines.len().saturating_sub(16);
    let lines = &lines[bottom..];
    let option = |line: &str| -> Option<(bool, u32, String)> {
        let rest = line.trim_start_matches(['❯', '›', '>', '▶', ' ']);
        let cursor = rest.len() != line.len();
        let (n, text) = rest.split_once(". ")?;
        let n = n.parse::<u32>().ok().filter(|n| (1..=9).contains(n))?;
        Some((cursor, n, text.trim().to_owned()))
    };
    let approval = lines.iter().rposition(|l| {
        (l.starts_with("Do you want to ") || l.starts_with("Would you like to "))
            && l.ends_with('?')
    });
    if let Some(i) = approval {
        let first = lines[i + 1..]
            .iter()
            .take(4)
            .find_map(|l| option(l).filter(|(_, n, _)| *n == 1));
        // Output after the menu means it was already answered and scrolled up.
        let last = lines.iter().rposition(|l| option(l).is_some());
        if first.is_some_and(|(_, _, text)| text.starts_with("Yes"))
            && last.is_some_and(|last| last > i && last + 3 >= lines.len())
        {
            return Some(lines[i].to_owned());
        }
    }
    let footer = lines.iter().rposition(|l| {
        let l = l.to_lowercase();
        l.contains("enter to select") && l.contains("esc to cancel")
    })?;
    if footer + 3 < lines.len() {
        return None;
    }
    let first = lines[..footer]
        .iter()
        .rposition(|l| option(l).is_some_and(|(_, n, _)| n == 1))?;
    if !lines[first..footer]
        .iter()
        .any(|l| option(l).is_some_and(|(cursor, _, _)| cursor))
    {
        return None;
    }
    lines[..first]
        .iter()
        .rev()
        .find(|l| option(l).is_none())
        .map(|l| (*l).to_owned())
}
