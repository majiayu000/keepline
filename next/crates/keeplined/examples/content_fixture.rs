//! Isolated native acceptance daemon: only watches fixture logs, never user CLI directories.
fn main() -> std::io::Result<()> {
    let root = std::path::PathBuf::from(std::env::args().nth(1).expect("fixture runtime"));
    std::fs::create_dir_all(root.join("logs"))?;
    keeplined::serve_with_roots(
        &root,
        vec![(keepline_core::model::Agent::Codex, root.join("logs"))],
    )
}
