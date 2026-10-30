//! How the left column lists sessions (`sidebar.json`). A preset only arranges what the
//! host already knows: it picks a grouping, which buckets fold into one summary row, when a
//! row shows its second line and how many rows come before 鸟瞰. Every preset reads the
//! same status judgement, so "needs you" means the same thing in all of them.
//!
//! The three built-in presets (经典 / 状态 / 目录) are written in this format too; plugins
//! add presets beside them and never replace one.
use crate::agent::Loaded;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The `sidebar.json` format's major version.
pub const API: u32 = 1;

/// What a preset groups rows by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Grouping {
    /// One list, what needs you first.
    Flat,
    /// A section per bucket: 需要你, 在跑, 做完了, 停下了, 很久没动.
    Status,
    /// 需要你 pinned on top, then one folder per working directory.
    Directory,
}

/// Coarse states a preset can name. The host maps its finer states onto these.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Bucket {
    /// Waiting on you or failed.
    Attention,
    Running,
    /// Finished, for you to check.
    Done,
    /// Stopped short or idle at the prompt.
    Stopped,
    /// Untouched past the stale days.
    Stale,
}
impl Bucket {
    pub const ALL: [Self; 5] = [
        Self::Attention,
        Self::Running,
        Self::Done,
        Self::Stopped,
        Self::Stale,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Attention => "需要你",
            Self::Running => "在跑",
            Self::Done => "做完了",
            Self::Stopped => "停下了",
            Self::Stale => "很久没动",
        }
    }
}

/// When a session row shows its second line (what it stopped at, what it is doing).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Detail {
    /// Never; the hover card still has it.
    #[default]
    Never,
    /// For rows that need you and the selected one.
    Attention,
    Always,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub group: Grouping,
    /// Buckets folded into one summary row until clicked; `attention` cannot fold.
    #[serde(default)]
    pub fold: Vec<Bucket>,
    #[serde(default)]
    pub detail: Detail,
    /// Session rows before the rest hand off to 鸟瞰.
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(skip)]
    pub file: PathBuf,
}
fn default_limit() -> usize {
    12
}
impl Preset {
    pub fn folds(&self, b: Bucket) -> bool {
        self.fold.contains(&b)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    api: u32,
    presets: Vec<Preset>,
}

/// 经典, 状态 and 目录, in the order the switch shows them.
pub fn builtin() -> Vec<Preset> {
    let p = |id: &str, name: &str, group, fold: &[Bucket], detail| Preset {
        id: id.into(),
        name: name.into(),
        group,
        fold: fold.to_vec(),
        detail,
        limit: default_limit(),
        file: PathBuf::new(),
    };
    vec![
        p("classic", "经典", Grouping::Flat, &[], Detail::Never),
        p(
            "status",
            "状态",
            Grouping::Status,
            &[Bucket::Done, Bucket::Stopped, Bucket::Stale],
            Detail::Attention,
        ),
        p(
            "directory",
            "目录",
            Grouping::Directory,
            &[Bucket::Done, Bucket::Stopped, Bucket::Stale],
            Detail::Attention,
        ),
    ]
}

fn read(file: &Path, taken: &[Preset]) -> Result<Vec<Preset>, String> {
    let bytes = std::fs::read(file).map_err(|e| format!("读不了：{e}"))?;
    let f: File = serde_json::from_slice(&bytes).map_err(|e| format!("格式不对：{e}"))?;
    if f.api != API {
        return Err(format!("左栏格式是 api {}，这个版本只认 api {API}", f.api));
    }
    if f.presets.is_empty() {
        return Err("presets 至少写一个".into());
    }
    let mut presets = f.presets;
    for i in 0..presets.len() {
        let (before, rest) = presets.split_at_mut(i);
        let p = &mut rest[0];
        if !crate::plugin::valid_id(&p.id) {
            return Err(format!(
                "左栏 id \"{}\" 只能用小写字母、数字和 -，不超过 40 个字",
                p.id
            ));
        }
        if taken.iter().chain(before.iter()).any(|t| t.id == p.id) {
            return Err(format!("左栏 id \"{}\" 和内置或前面的插件重名", p.id));
        }
        if p.name.trim().is_empty() || p.name.chars().count() > 6 {
            return Err(format!("左栏 {} 的 name 要写，最多 6 个字", p.id));
        }
        if p.fold.contains(&Bucket::Attention) {
            return Err(format!("左栏 {} 不能折叠需要你的会话", p.id));
        }
        if !(1..=100).contains(&p.limit) {
            return Err(format!("左栏 {} 的 limit 要在 1 到 100 之间", p.id));
        }
        p.file = file.to_owned();
    }
    Ok(presets)
}

/// The built-in presets, then each file's in order; a file with any bad preset loads none.
pub fn load(files: Vec<PathBuf>) -> (Vec<Preset>, Vec<Loaded>) {
    let mut presets = builtin();
    let mut report = Vec::new();
    for file in files {
        let result = read(&file, &presets);
        report.push(Loaded {
            file: file.clone(),
            result: result
                .as_ref()
                .map(|found| {
                    found
                        .iter()
                        .map(|p| p.name.as_str())
                        .collect::<Vec<_>>()
                        .join("、")
                })
                .map_err(Clone::clone),
        });
        if let Ok(found) = result {
            presets.extend(found);
        }
    }
    (presets, report)
}

static PRESETS: OnceLock<(Vec<Preset>, Vec<Loaded>)> = OnceLock::new();

/// Loads the installed plugins' presets once (after `plugin::install`).
pub fn install() {
    PRESETS.get_or_init(|| load(crate::plugin::files(crate::plugin::Kind::Sidebar)));
}
/// Every preset: the built-in three, then the plugins'.
pub fn all() -> &'static [Preset] {
    static BUILTIN: OnceLock<Vec<Preset>> = OnceLock::new();
    match PRESETS.get() {
        Some((p, _)) => p,
        None => BUILTIN.get_or_init(builtin),
    }
}
/// The preset with this id, or 经典 when it is gone (a removed plugin's choice).
pub fn find(id: &str) -> &'static Preset {
    let all = all();
    all.iter().find(|p| p.id == id).unwrap_or(&all[0])
}
pub fn report() -> &'static [Loaded] {
    PRESETS.get().map_or(&[], |(_, r)| r.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_presets_add_to_the_builtin_ones() {
        let d = std::env::temp_dir().join(format!("kl-sidebar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let write = |name: &str, body: &str| {
            let p = d.join(name);
            std::fs::write(&p, body).unwrap();
            p
        };
        let good = write(
            "a.json",
            r#"{"api":1,"presets":[{"id":"quiet","name":"安静","group":"status","fold":["running","done","stopped","stale"],"detail":"always","limit":8}]}"#,
        );
        let dup = write(
            "b.json",
            r#"{"api":1,"presets":[{"id":"status","name":"又一个","group":"flat"}]}"#,
        );
        let attention = write(
            "c.json",
            r#"{"api":1,"presets":[{"id":"hide","name":"藏","group":"flat","fold":["attention"]}]}"#,
        );
        let unknown = write(
            "d.json",
            r#"{"api":1,"presets":[{"id":"x","name":"x","group":"by-tag"}]}"#,
        );
        let (presets, report) = load(vec![good, dup, attention, unknown]);
        assert_eq!(
            presets.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            ["classic", "status", "directory", "quiet"]
        );
        assert_eq!(presets[3].limit, 8);
        assert!(presets[3].folds(Bucket::Running));
        assert_eq!(report[0].result, Ok("安静".into()));
        assert!(report[1].result.as_ref().unwrap_err().contains("重名"));
        assert!(report[2].result.as_ref().unwrap_err().contains("需要你"));
        assert!(report[3].result.as_ref().unwrap_err().contains("格式不对"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_missing_choice_falls_back_to_classic() {
        assert_eq!(find("gone").id, "classic");
        assert_eq!(find("directory").group, Grouping::Directory);
    }
}
