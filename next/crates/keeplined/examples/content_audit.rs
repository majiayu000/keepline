//! Offline, read-only source import into a caller-owned, empty evidence database.
use keeplined::{log_index::Index, recognition};
fn main() -> std::io::Result<()> {
    let root = std::env::args()
        .nth(1)
        .expect("destination runtime required");
    let root = std::path::Path::new(&root);
    std::fs::create_dir_all(root)?;
    let started = std::time::Instant::now();
    let mut index = Index::open(root, recognition::roots()?)?;
    index.initialize()?;
    index.bootstrap()?;
    let mut cards = Vec::new();
    for t in index.cached()? {
        if index.recent(t.updated) {
            if let Some(card) = index.card(&t.path)? {
                cards.push(serde_json::json!({"runtime":t.runtime,"cli_id":t.cli_id,"path":t.path,"cwd":t.cwd,"mtime":t.updated,"here":card.here(),"content":card}));
            }
        }
    }
    cards.sort_by_key(|v| std::cmp::Reverse(v["mtime"].as_u64().unwrap_or(0)));
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"seconds":started.elapsed().as_secs_f64(),"directories":index.counts.directories,"opens":index.counts.opens,"bytes":index.counts.bytes,"cards":cards})
        )?
    );
    Ok(())
}
