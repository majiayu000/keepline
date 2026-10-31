//! How far a space's branch is ahead of and behind its upstream, as Herdr's sidebar shows
//! it. Counting needs git, so it runs only when one of the refs it depends on changed on
//! disk (a commit, a fetch, a push, a checkout), and never writes to `.git`.
use super::*;
use std::time::{Instant, SystemTime};

/// Commits on the branch the upstream lacks, and the other way round.
pub(super) type Divergence = (u32, u32);

/// The files whose change can move the counts: HEAD, the branch, its upstream, packed refs.
type Signature = Vec<Option<SystemTime>>;

#[derive(Default)]
struct Entry {
    looked: Option<Instant>,
    signature: Signature,
    counting: bool,
    counts: Option<Divergence>,
}
static CACHE: std::sync::LazyLock<std::sync::Mutex<HashMap<String, Entry>>> =
    std::sync::LazyLock::new(Default::default);
/// Redraws within this long reuse the last look without touching the disk.
const LOOK_EVERY: Duration = Duration::from_secs(1);

/// The repository's own git directory and the common one a linked worktree shares.
fn git_dirs(dir: &Path) -> Option<(PathBuf, PathBuf)> {
    for d in dir.ancestors() {
        let dot = d.join(".git");
        if dot.is_dir() {
            return Some((dot.clone(), dot));
        }
        if dot.is_file() {
            let text = std::fs::read_to_string(&dot).ok()?;
            let git_dir = d.join(text.trim().strip_prefix("gitdir:")?.trim());
            let common = std::fs::read_to_string(git_dir.join("commondir"))
                .ok()
                .map(|c| git_dir.join(c.trim()))
                .unwrap_or_else(|| git_dir.clone());
            return Some((git_dir, common));
        }
    }
    None
}
/// The branch's upstream ref as `refs/remotes/<remote>/<branch>`, from the repository's
/// config; None when it tracks nothing or tracks a local branch.
fn upstream_ref(config: &str, branch: &str) -> Option<String> {
    let header = format!("[branch \"{branch}\"]");
    let mut inside = false;
    let (mut remote, mut merge) = (None, None);
    for line in config.lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line == header;
            continue;
        }
        if !inside {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "remote" => remote = Some(value.trim().to_owned()),
            "merge" => merge = Some(value.trim().to_owned()),
            _ => (),
        }
    }
    let remote = remote.filter(|r| r != ".")?;
    let name = merge?.strip_prefix("refs/heads/")?.to_owned();
    Some(format!("refs/remotes/{remote}/{name}"))
}
/// What to stat for `dir`, or None when it is not on a branch with an upstream.
fn watched(dir: &Path) -> Option<Vec<PathBuf>> {
    let (git_dir, common) = git_dirs(dir)?;
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let branch = head.trim().strip_prefix("ref: refs/heads/")?.to_owned();
    let config = std::fs::read_to_string(common.join("config")).ok()?;
    let upstream = upstream_ref(&config, &branch)?;
    Some(vec![
        git_dir.join("HEAD"),
        common.join("refs/heads").join(&branch),
        common.join(upstream),
        common.join("packed-refs"),
    ])
}
fn signature(files: &[PathBuf]) -> Signature {
    files
        .iter()
        .map(|f| std::fs::metadata(f).and_then(|m| m.modified()).ok())
        .collect()
}
/// Counts with git, read-only: no index refresh, no fsmonitor, no optional locks.
pub(super) fn count(dir: &Path) -> Option<Divergence> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "--no-optional-locks",
            "-c",
            "core.fsmonitor=false",
            "rev-list",
            "--left-right",
            "--count",
            "HEAD...@{upstream}",
        ])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let text = String::from_utf8(out.stdout).ok()?;
    let mut parts = text.split_whitespace().map(|n| n.parse::<u32>().ok());
    Some((parts.next()??, parts.next()??))
}

impl Workbench {
    /// The last counts for `dir`; looks at its refs at most once a second while drawn, and
    /// counts again in the background only after one of them changed.
    pub(super) fn git_divergence(&self, dir: &str, cx: &Context<Self>) -> Option<Divergence> {
        let mut cache = CACHE.lock().ok()?;
        let entry = cache.entry(dir.to_owned()).or_default();
        if entry.counting || entry.looked.is_some_and(|at| at.elapsed() < LOOK_EVERY) {
            return entry.counts;
        }
        entry.looked = Some(Instant::now());
        let Some(files) = watched(Path::new(dir)) else {
            entry.signature.clear();
            entry.counts = None;
            return None;
        };
        let now = signature(&files);
        if now == entry.signature {
            return entry.counts;
        }
        entry.counting = true;
        let old = entry.counts;
        drop(cache);
        let dir = dir.to_owned();
        cx.spawn(async move |this, cx| {
            let at = PathBuf::from(&dir);
            let counts = cx.background_spawn(async move { count(&at) }).await;
            if let Ok(mut cache) = CACHE.lock()
                && let Some(entry) = cache.get_mut(&dir)
            {
                entry.counting = false;
                entry.signature = now;
                entry.counts = counts;
            }
            if counts != old {
                let _ = this.update(cx, |_, cx| cx.notify());
            }
        })
        .detach();
        old
    }
}
/// `↑2 ↓1`, leaving out a side that is zero; nothing when even.
pub(super) fn divergence_label((ahead, behind): Divergence) -> Option<String> {
    let parts: Vec<String> = [(ahead, '↑'), (behind, '↓')]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, arrow)| format!("{arrow}{n}"))
        .collect();
    (!parts.is_empty()).then(|| parts.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn git(dir: &Path, args: &[&str]) {
        let ok = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
            .args(args)
            .output()
            .unwrap();
        assert!(ok.status.success(), "{args:?}: {}", String::from_utf8_lossy(&ok.stderr));
    }

    #[test]
    fn upstream_comes_from_the_branch_section_only() {
        let config = "[core]\n\tbare = false\n[branch \"main\"]\n\tremote = origin\n\tmerge = refs/heads/main\n[branch \"x\"]\n\tremote = .\n\tmerge = refs/heads/main\n";
        assert_eq!(upstream_ref(config, "main").as_deref(), Some("refs/remotes/origin/main"));
        assert_eq!(upstream_ref(config, "x"), None, "a local upstream is not a remote");
        assert_eq!(upstream_ref(config, "other"), None);
    }

    #[test]
    fn label_leaves_out_zero_sides() {
        assert_eq!(divergence_label((0, 0)), None);
        assert_eq!(divergence_label((2, 0)).as_deref(), Some("↑2"));
        assert_eq!(divergence_label((2, 1)).as_deref(), Some("↑2 ↓1"));
    }

    /// A clone two commits ahead and one behind its origin, counted without writing to
    /// `.git`; the watched refs change on commit and fetch, and a linked worktree resolves.
    #[test]
    fn counts_ahead_and_behind_and_watches_the_refs_that_move_them() {
        let root = std::env::temp_dir().join(format!("kl-div-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (origin, clone) = (root.join("origin"), root.join("clone"));
        std::fs::create_dir_all(&origin).unwrap();
        git(&origin, &["init", "-q", "-b", "main"]);
        git(&origin, &["commit", "-q", "--allow-empty", "-m", "a"]);
        git(&root, &["clone", "-q", origin.to_str().unwrap(), "clone"]);
        assert_eq!(count(&clone), Some((0, 0)));
        let files = watched(&clone).expect("main tracks origin/main");
        let before = signature(&files);
        git(&clone, &["commit", "-q", "--allow-empty", "-m", "b"]);
        git(&clone, &["commit", "-q", "--allow-empty", "-m", "c"]);
        assert_ne!(signature(&files), before, "a commit moves the branch ref");
        git(&origin, &["commit", "-q", "--allow-empty", "-m", "d"]);
        let fetched = signature(&files);
        git(&clone, &["fetch", "-q"]);
        assert_ne!(signature(&files), fetched, "a fetch moves the upstream ref");
        let index = std::fs::metadata(clone.join(".git/index")).ok().and_then(|m| m.modified().ok());
        assert_eq!(count(&clone), Some((2, 1)));
        assert_eq!(
            std::fs::metadata(clone.join(".git/index")).ok().and_then(|m| m.modified().ok()),
            index,
            "counting wrote to .git"
        );
        // A linked worktree on a branch without upstream: nothing to watch, nothing run.
        git(&clone, &["worktree", "add", "-q", "-b", "side", root.join("wt").to_str().unwrap()]);
        assert!(git_dirs(&root.join("wt")).is_some_and(|(g, c)| g != c));
        assert!(watched(&root.join("wt")).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}
