//! The plugin catalog: plugins offered for one-click install, from two places that share
//! one index format.
//!
//! - **Bundled**: the app ships `next/plugins/` (copied into `Contents/Resources/plugins`),
//!   so its example plugins are installable offline.
//! - **Remote**: an `index.json` at a URL (by default this repository's `next/plugins/`
//!   on GitHub; `<config>/market` or `KEEPLINE_PLUGIN_MARKET` names another). Files are
//!   fetched next to it as `<id>/<path>`, with the system curl.
//!
//! ```json
//! {"api": 1, "plugins": [{"id": "pane-facts", "name": "窗格信息", "version": "0.1.0",
//!   "description": "…", "provides": ["decorations"],
//!   "files": [{"path": "plugin.json", "sha256": "…", "size": 312}]}]}
//! ```
//!
//! Installing copies exactly the listed files, each checked against its sha256, into a
//! staging directory, checks it as a plugin, then swaps it in as `plugins/<id>`. Nothing
//! is run while installing; what a plugin may run later is stated up front (`permissions`).
use crate::plugin::{self, Kind, Plugin};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};

/// The index format's major version.
pub const API: u32 = 1;
/// This repository's plugins, as published on GitHub.
pub const DEFAULT_REMOTE: &str =
    "https://raw.githubusercontent.com/majiayu000/keepline/main/next/plugins/index.json";
/// No plugin file is this large; a bigger one is refused, not written.
const MAX_FILE: u64 = 16 << 20;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Index {
    pub api: u32,
    pub plugins: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub provides: Vec<Kind>,
    pub files: Vec<FileEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileEntry {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

/// Where an offer's files come from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// The catalog directory that holds `index.json` and `<id>/` folders.
    Bundled(PathBuf),
    /// The URL directory that holds `index.json`, without the trailing slash.
    Remote(String),
}

/// One installable plugin and where to get it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub entry: Entry,
    pub source: Source,
}

/// What a plugin's kinds may do once installed, in words for the install confirmation.
/// Installing itself never runs anything.
pub fn permissions(provides: &[Kind]) -> Vec<&'static str> {
    let mut out = Vec::new();
    for k in provides {
        out.push(match k {
            Kind::Actions => "右键菜单或 ⌘P 里点了它，就在终端里运行它写的命令",
            Kind::Hooks => "会话等你、出错、做完或命令结束时，在后台运行它写的命令或发 webhook",
            Kind::Decorations => "定时在窗格所在目录里运行它写的命令，显示第一行输出",
            Kind::Agent => "读这个 Agent 的对话记录，并能用它写的命令启动和恢复会话",
            Kind::Rules => "只决定哪些命令算跑过验证，不运行任何东西",
            Kind::Sidebar => "只决定左栏怎么排；WASM 排法在沙盒里跑，读不到文件和网络",
            Kind::Effects => "只挑选标记完成时播哪种动画，不运行任何东西",
        });
    }
    out
}
/// Whether installing it lets it run commands on this computer: worth a confirmation.
pub fn runs_commands(provides: &[Kind]) -> bool {
    provides.iter().any(|k| {
        matches!(
            k,
            Kind::Actions | Kind::Hooks | Kind::Decorations | Kind::Agent
        )
    })
}

/// `1.10.0` is newer than `1.9.2`; parts that are not numbers compare as text.
pub fn newer(a: &str, b: &str) -> bool {
    let parts = |v: &str| -> Vec<(u64, String)> {
        v.trim()
            .trim_start_matches('v')
            .split(['.', '-'])
            .map(|p| (p.parse().unwrap_or(0), p.to_owned()))
            .collect()
    };
    parts(a) > parts(b)
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A listed path that stays inside the plugin's directory: relative, no `..`, no hidden
/// segments, no backslashes.
fn safe_path(path: &str) -> bool {
    let p = Path::new(path);
    !path.is_empty()
        && !path.contains('\\')
        && p.components().all(|c| match c {
            Component::Normal(s) => !s.to_string_lossy().starts_with('.'),
            _ => false,
        })
}

impl Entry {
    /// Refuses an entry that could not be installed safely, before fetching anything.
    pub fn check(&self) -> Result<(), String> {
        if !plugin::valid_id(&self.id) {
            return Err(format!("id \"{}\" 不合法", self.id));
        }
        if !self.files.iter().any(|f| f.path == "plugin.json") {
            return Err(format!("{} 的文件里没有 plugin.json", self.id));
        }
        for f in &self.files {
            if !safe_path(&f.path) {
                return Err(format!("{} 的文件路径 \"{}\" 不安全", self.id, f.path));
            }
            if f.size > MAX_FILE {
                return Err(format!("{} 的 {} 太大（{} 字节）", self.id, f.path, f.size));
            }
            if f.sha256.len() != 64 || !f.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(format!("{} 的 {} 没有正确的 sha256", self.id, f.path));
            }
        }
        Ok(())
    }
}

/// The files a plugin directory ships: `plugin.json`, every file of the kinds it provides,
/// and the `.wasm` files its sidebar presets name. Build files and sources stay behind.
pub fn shipped_files(plugin: &Plugin) -> Vec<PathBuf> {
    let mut files = vec![plugin.dir.join("plugin.json")];
    for kind in &plugin.provides {
        files.extend(kind.files(&plugin.dir));
    }
    if plugin.provides.contains(&Kind::Sidebar)
        && let Ok(bytes) = std::fs::read(plugin.dir.join("sidebar.json"))
        && let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes)
    {
        for preset in v["presets"].as_array().into_iter().flatten() {
            if let Some(w) = preset["wasm"].as_str().filter(|w| safe_path(w)) {
                files.push(plugin.dir.join(w));
            }
        }
    }
    files.sort();
    files.dedup();
    files
}

/// Builds the index of every plugin directory under `dir`; a directory that does not
/// check out as a plugin is an error, so a broken example cannot be published.
pub fn index_of(dir: &Path) -> Result<Index, String> {
    let mut plugins = Vec::new();
    for found in plugin::discover(dir) {
        let p = found
            .result
            .map_err(|e| format!("{}：{e}", found.dir.display()))?;
        if found.dir.file_name() != Some(std::ffi::OsStr::new(&p.id)) {
            return Err(format!(
                "{}：目录名要和 id「{}」一样，安装时按 id 找文件",
                found.dir.display(),
                p.id
            ));
        }
        let mut files = Vec::new();
        for file in shipped_files(&p) {
            let bytes =
                std::fs::read(&file).map_err(|e| format!("读不了 {}：{e}", file.display()))?;
            let path = file
                .strip_prefix(&p.dir)
                .map_err(|_| format!("{} 不在插件目录里", file.display()))?
                .to_string_lossy()
                .into_owned();
            files.push(FileEntry {
                path,
                sha256: sha256(&bytes),
                size: bytes.len() as u64,
            });
        }
        plugins.push(Entry {
            id: p.id,
            name: p.name,
            version: p.version.unwrap_or_else(|| "0.0.0".into()),
            description: p.description,
            provides: p.provides,
            files,
        });
    }
    Ok(Index { api: API, plugins })
}

/// The index as `index.json` holds it: pretty, one trailing newline.
pub fn render(index: &Index) -> String {
    serde_json::to_string_pretty(index).unwrap_or_default() + "\n"
}

fn parse(bytes: &[u8]) -> Result<Index, String> {
    let index: Index =
        serde_json::from_slice(bytes).map_err(|e| format!("index.json 格式不对：{e}"))?;
    if index.api != API {
        return Err(format!(
            "插件目录格式是 api {}，这个版本只认 api {API}",
            index.api
        ));
    }
    Ok(index)
}

/// The catalog shipped with this build: `KEEPLINE_PLUGIN_CATALOG`, else the bundle's
/// `Contents/Resources/plugins`, else `next/plugins` beside a development build.
pub fn bundled_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("KEEPLINE_PLUGIN_CATALOG") {
        return Some(PathBuf::from(dir));
    }
    let exe = std::env::current_exe().ok()?;
    let mac = exe.parent()?;
    let candidates = [
        mac.parent().map(|c| c.join("Resources/plugins")),
        // target/<profile>/keepline-app → next/plugins
        mac.parent()
            .and_then(Path::parent)
            .map(|n| n.join("plugins")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|d| d.join("index.json").is_file())
}

/// The bundled catalog's offers; none when this build ships no catalog.
pub fn bundled() -> Result<Vec<Offer>, String> {
    let Some(dir) = bundled_dir() else {
        return Ok(vec![]);
    };
    let bytes = std::fs::read(dir.join("index.json"))
        .map_err(|e| format!("读不了 {}：{e}", dir.join("index.json").display()))?;
    Ok(parse(&bytes)?
        .plugins
        .into_iter()
        .map(|entry| Offer {
            entry,
            source: Source::Bundled(dir.clone()),
        })
        .collect())
}

/// The remote index's URL: `KEEPLINE_PLUGIN_MARKET`, else the first line of
/// `<config>/market`, else this repository's.
pub fn remote_url() -> String {
    if let Some(url) = std::env::var("KEEPLINE_PLUGIN_MARKET")
        .ok()
        .filter(|u| !u.trim().is_empty())
    {
        return url.trim().to_owned();
    }
    crate::agent::config_dir()
        .and_then(|d| std::fs::read_to_string(d.join("market")).ok())
        .and_then(|t| t.lines().next().map(|l| l.trim().to_owned()))
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| DEFAULT_REMOTE.to_owned())
}

fn curl(url: &str) -> Result<Vec<u8>, String> {
    let out = std::process::Command::new("curl")
        .args(["-fsSL", "--max-time", "20", "--max-filesize"])
        .arg(MAX_FILE.to_string())
        .arg("--proto")
        .arg("=https,file")
        .arg(url)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("运行 curl 失败：{e}"))?;
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        return Err(if why.is_empty() {
            format!("下载 {url} 失败（curl 退出码 {:?}）", out.status.code())
        } else {
            format!("下载 {url} 失败：{why}")
        });
    }
    Ok(out.stdout)
}

/// Fetches the remote index. Only `https://` and `file://` URLs are followed.
pub fn fetch_remote(url: &str) -> Result<Vec<Offer>, String> {
    let base = url
        .rsplit_once('/')
        .map(|(b, _)| b.to_owned())
        .ok_or_else(|| format!("{url} 不是一个地址"))?;
    let index = parse(&curl(url)?)?;
    Ok(index
        .plugins
        .into_iter()
        .map(|entry| Offer {
            entry,
            source: Source::Remote(base.clone()),
        })
        .collect())
}

/// The bundled offers, each replaced by the remote one when that is newer, then the
/// plugins only the remote index has.
pub fn merge(bundled: Vec<Offer>, remote: Vec<Offer>) -> Vec<Offer> {
    let mut out = bundled;
    for r in remote {
        match out.iter_mut().find(|o| o.entry.id == r.entry.id) {
            Some(o) if newer(&r.entry.version, &o.entry.version) => *o = r,
            Some(_) => {}
            None => out.push(r),
        }
    }
    out
}

impl Offer {
    fn fetch(&self, path: &str) -> Result<Vec<u8>, String> {
        match &self.source {
            Source::Bundled(dir) => {
                let file = dir.join(&self.entry.id).join(path);
                std::fs::read(&file).map_err(|e| format!("读不了 {}：{e}", file.display()))
            }
            Source::Remote(base) => curl(&format!("{base}/{}/{path}", self.entry.id)),
        }
    }
    /// Where it comes from, in words.
    pub fn origin(&self) -> &'static str {
        match self.source {
            Source::Bundled(_) => "随 App 附带",
            Source::Remote(_) => "远程目录",
        }
    }
    /// Installs it as `<plugins>/<id>`, replacing an installed copy of the same id
    /// (wherever that one's directory is). Nothing is left half-written: a bad file or
    /// a plugin that does not check out leaves the old copy as it was.
    pub fn install_into(&self, plugins: &Path) -> Result<PathBuf, String> {
        let e = &self.entry;
        e.check()?;
        std::fs::create_dir_all(plugins).map_err(|err| format!("建不了插件目录：{err}"))?;
        let stage = plugins.join(format!(".installing-{}-{}", e.id, std::process::id()));
        let _ = std::fs::remove_dir_all(&stage);
        let staged = (|| {
            for f in &e.files {
                let bytes = self.fetch(&f.path)?;
                if bytes.len() as u64 != f.size || sha256(&bytes) != f.sha256 {
                    return Err(format!(
                        "{} 的 {} 和目录里登记的不一致（大小或 sha256 不对），没有安装",
                        e.id, f.path
                    ));
                }
                let to = stage.join(&f.path);
                if let Some(parent) = to.parent() {
                    std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
                }
                std::fs::write(&to, bytes).map_err(|err| err.to_string())?;
            }
            let p = plugin::check(&stage)?;
            if p.id != e.id {
                return Err(format!(
                    "plugin.json 里的 id 是 {}，目录里登记的是 {}",
                    p.id, e.id
                ));
            }
            Ok(())
        })();
        if let Err(err) = staged {
            let _ = std::fs::remove_dir_all(&stage);
            return Err(err);
        }
        // Replace whichever directory holds this id now, else take `<id>`.
        let target = plugin::discover(plugins)
            .into_iter()
            .find(|f| f.result.as_ref().is_ok_and(|p| p.id == e.id))
            .map(|f| f.dir)
            .filter(|d| d.parent() == Some(plugins))
            .unwrap_or_else(|| plugins.join(&e.id));
        let old = plugins.join(format!(".replaced-{}-{}", e.id, std::process::id()));
        let _ = std::fs::remove_dir_all(&old);
        let had = target.exists();
        if had {
            std::fs::rename(&target, &old).map_err(|err| {
                let _ = std::fs::remove_dir_all(&stage);
                format!("换不掉旧的 {}：{err}", target.display())
            })?;
        }
        if let Err(err) = std::fs::rename(&stage, &target) {
            if had {
                let _ = std::fs::rename(&old, &target);
            }
            let _ = std::fs::remove_dir_all(&stage);
            return Err(format!("装不进 {}：{err}", target.display()));
        }
        let _ = std::fs::remove_dir_all(&old);
        Ok(target)
    }
    /// Installs it into this process's plugins directory (after `plugin::install`).
    pub fn install(&self) -> Result<PathBuf, String> {
        let config = plugin::config().ok_or("找不到配置目录")?;
        self.install_into(&plugin::dir(config))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kl-market-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    fn plugin(root: &Path, id: &str, version: &str) {
        let d = root.join(id);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("plugin.json"),
            format!(
                r#"{{"api":1,"id":"{id}","name":"{id}","version":"{version}","provides":["effects"]}}"#
            ),
        )
        .unwrap();
        std::fs::write(
            d.join("effects.json"),
            r#"{"api":1,"complete":{"style":"sweep","duration_ms":1200}}"#,
        )
        .unwrap();
        // A build leftover that must not travel.
        std::fs::write(d.join("build.sh"), "echo hi").unwrap();
    }

    #[test]
    fn the_shipped_index_is_current() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
        let want = render(&index_of(&dir).unwrap());
        let have = std::fs::read_to_string(dir.join("index.json")).unwrap_or_default();
        assert!(
            want == have,
            "next/plugins/index.json 过期了：运行 cargo run -p keeplined -- plugins index next/plugins"
        );
    }

    #[test]
    fn installs_only_listed_files_checked_and_replaces_in_place() {
        let catalog = tmp("catalog");
        plugin(&catalog, "quick-sweep", "1.0.0");
        let index = index_of(&catalog).unwrap();
        assert_eq!(
            index.plugins[0]
                .files
                .iter()
                .map(|f| f.path.as_str())
                .collect::<Vec<_>>(),
            ["effects.json", "plugin.json"]
        );
        std::fs::write(catalog.join("index.json"), render(&index)).unwrap();
        let offer = Offer {
            entry: index.plugins[0].clone(),
            source: Source::Bundled(catalog.clone()),
        };
        let plugins = tmp("plugins");
        // A hand-placed copy under another directory name is replaced where it is.
        plugin(&plugins, "quick-sweep", "0.9.0");
        std::fs::rename(plugins.join("quick-sweep"), plugins.join("mine")).unwrap();
        let at = offer.install_into(&plugins).unwrap();
        assert_eq!(at, plugins.join("mine"));
        assert!(!at.join("build.sh").exists());
        assert!(plugin::check(&at).unwrap().version.as_deref() == Some("1.0.0"));
        assert_eq!(
            std::fs::read_dir(&plugins).unwrap().count(),
            1,
            "no staging left behind"
        );

        // A tampered file is refused and the installed copy stays.
        std::fs::write(catalog.join("quick-sweep/effects.json"), "{}").unwrap();
        let err = offer.install_into(&plugins).unwrap_err();
        assert!(err.contains("不一致"), "{err}");
        assert!(plugin::check(&at).is_ok());
        assert_eq!(std::fs::read_dir(&plugins).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(&catalog);
        let _ = std::fs::remove_dir_all(&plugins);
    }

    #[test]
    fn unsafe_entries_are_refused_before_fetching() {
        let file = |path: &str| FileEntry {
            path: path.into(),
            sha256: "0".repeat(64),
            size: 1,
        };
        let entry = |id: &str, files: Vec<FileEntry>| Entry {
            id: id.into(),
            name: "x".into(),
            version: "1".into(),
            description: None,
            provides: vec![Kind::Rules],
            files,
        };
        assert!(
            entry("ok", vec![file("plugin.json"), file("a/b.json")])
                .check()
                .is_ok()
        );
        for bad in ["../x", "/etc/passwd", "a/../../x", ".hidden", "a\\b", ""] {
            assert!(
                entry("ok", vec![file("plugin.json"), file(bad)])
                    .check()
                    .is_err(),
                "{bad}"
            );
        }
        assert!(entry("../ok", vec![file("plugin.json")]).check().is_err());
        assert!(entry("ok", vec![file("rules.json")]).check().is_err());
    }

    #[test]
    fn remote_offers_replace_bundled_only_when_newer() {
        let offer = |id: &str, v: &str, remote: bool| Offer {
            entry: Entry {
                id: id.into(),
                name: id.into(),
                version: v.into(),
                description: None,
                provides: vec![Kind::Rules],
                files: vec![],
            },
            source: if remote {
                Source::Remote("https://x".into())
            } else {
                Source::Bundled(PathBuf::new())
            },
        };
        let merged = merge(
            vec![offer("a", "0.2.0", false), offer("b", "0.1.0", false)],
            vec![
                offer("a", "0.10.0", true),
                offer("b", "0.1.0", true),
                offer("c", "1", true),
            ],
        );
        let got: Vec<_> = merged
            .iter()
            .map(|o| (o.entry.id.as_str(), o.origin()))
            .collect();
        assert_eq!(
            got,
            [("a", "远程目录"), ("b", "随 App 附带"), ("c", "远程目录")]
        );
        assert!(newer("1.10.0", "1.9.2") && !newer("1.0.0", "1.0.0"));
        assert!(runs_commands(&[Kind::Effects, Kind::Hooks]));
        assert!(!runs_commands(&[Kind::Effects, Kind::Sidebar]));
    }

    #[test]
    fn a_file_url_index_installs_like_a_remote_one() {
        let catalog = tmp("remote");
        plugin(&catalog, "quick-sweep", "2.0.0");
        std::fs::write(
            catalog.join("index.json"),
            render(&index_of(&catalog).unwrap()),
        )
        .unwrap();
        let url = format!("file://{}/index.json", catalog.display());
        let offers = fetch_remote(&url).unwrap();
        assert_eq!(offers.len(), 1);
        let plugins = tmp("remote-plugins");
        let at = offers[0].install_into(&plugins).unwrap();
        assert_eq!(at, plugins.join("quick-sweep"));
        assert!(fetch_remote(&format!("file://{}/missing.json", catalog.display())).is_err());
        let _ = std::fs::remove_dir_all(&catalog);
        let _ = std::fs::remove_dir_all(&plugins);
    }
}
