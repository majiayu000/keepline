//! Plugin packages: one directory per plugin under `<config_dir>/plugins`, each with a
//! `plugin.json` naming what it provides. Every kind (agents now; actions, rules and
//! sidebar views next) reads its files from here, and every outcome lands in one report
//! the settings page shows: which plugin and file took effect, and why one did not.
//!
//! A bad plugin never stops the others or the app: it is reported and skipped.
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// The package format's major version; a plugin of another is refused, not guessed at.
pub const API: u32 = 1;

/// What a plugin can extend. The file each kind reads is fixed, so a manifest only says
/// which kinds it means to provide and a missing file is reported rather than ignored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// `agent.json`, and any `agents/*.json`: CLIs the registry learns (see `agent`).
    Agent,
    /// `actions.json`: commands for the pane menu and ⌘P (see `action`).
    Actions,
    /// `rules.json`: which commands count as verification (see `rule`).
    Rules,
    /// `sidebar.json`: ways to arrange the left column (see `sidebar`).
    Sidebar,
    /// `hooks.json`: commands or webhooks told when sessions change state (see `hook`).
    Hooks,
}
impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Agent => "Agent",
            Self::Actions => "动作",
            Self::Rules => "验证规则",
            Self::Sidebar => "左栏",
            Self::Hooks => "Hooks",
        }
    }
    /// The files of this kind inside a plugin directory, in load order.
    pub fn files(self, dir: &Path) -> Vec<PathBuf> {
        match self {
            Self::Agent => {
                let mut files: Vec<PathBuf> = std::fs::read_dir(dir.join("agents"))
                    .map(|entries| {
                        entries
                            .filter_map(|e| e.ok().map(|e| e.path()))
                            .filter(|p| p.extension().is_some_and(|e| e == "json"))
                            .collect()
                    })
                    .unwrap_or_default();
                files.sort();
                let single = dir.join("agent.json");
                if single.is_file() {
                    files.insert(0, single);
                }
                files
            }
            Self::Actions => [dir.join("actions.json")]
                .into_iter()
                .filter(|p| p.is_file())
                .collect(),
            Self::Rules => [dir.join("rules.json")]
                .into_iter()
                .filter(|p| p.is_file())
                .collect(),
            Self::Hooks => [dir.join("hooks.json")]
                .into_iter()
                .filter(|p| p.is_file())
                .collect(),
            Self::Sidebar => [dir.join("sidebar.json")]
                .into_iter()
                .filter(|p| p.is_file())
                .collect(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    api: u32,
    id: String,
    name: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    description: Option<String>,
    provides: Vec<Kind>,
}

/// A plugin whose manifest checked out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub dir: PathBuf,
    pub provides: Vec<Kind>,
    /// Turned off in settings: listed, but none of its files load.
    pub enabled: bool,
}
impl Plugin {
    /// Its files of `kind`; none when it does not provide that kind or is turned off.
    pub fn files(&self, kind: Kind) -> Vec<PathBuf> {
        if self.enabled && self.provides.contains(&kind) {
            kind.files(&self.dir)
        } else {
            vec![]
        }
    }
}

/// One plugin directory's outcome; its files' own outcomes are reported per kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub dir: PathBuf,
    pub result: Result<Plugin, String>,
}

static INSTALLED: std::sync::OnceLock<Vec<Found>> = std::sync::OnceLock::new();
static CONFIG: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Discovers the plugins under `<config>/plugins` once; later calls return the same list.
pub fn install(config: &Path) -> &'static [Found] {
    let _ = CONFIG.set(config.to_owned());
    INSTALLED.get_or_init(|| discover(&dir(config)))
}

/// `plugins/disabled.json`: the ids turned off in settings, as a JSON list.
fn switches(plugins: &Path) -> PathBuf {
    plugins.join("disabled.json")
}
fn disabled(plugins: &Path) -> Vec<String> {
    std::fs::read(switches(plugins))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
/// Whether the plugin will load at the next start: what settings show its switch as.
pub fn enabled_next_start(id: &str) -> bool {
    CONFIG
        .get()
        .is_none_or(|c| !disabled(&dir(c)).iter().any(|d| d == id))
}
/// Turns a plugin on or off from the next start; the running app keeps what it loaded.
pub fn set_enabled(id: &str, on: bool) -> std::io::Result<()> {
    let plugins = dir(CONFIG
        .get()
        .ok_or_else(|| std::io::Error::other("找不到配置目录"))?);
    let mut off = disabled(&plugins);
    off.retain(|d| d != id);
    if !on {
        off.push(id.to_owned());
    }
    std::fs::create_dir_all(&plugins)?;
    std::fs::write(switches(&plugins), serde_json::to_vec_pretty(&off)?)
}

/// Every plugin directory's outcome since `install`; empty before it.
pub fn report() -> &'static [Found] {
    INSTALLED.get().map_or(&[], Vec::as_slice)
}

/// The loaded plugins' files of one kind, plugin by plugin in directory order.
pub fn files(kind: Kind) -> Vec<PathBuf> {
    report()
        .iter()
        .filter_map(|f| f.result.as_ref().ok())
        .flat_map(|p| p.files(kind))
        .collect()
}

/// `<config_dir>/plugins`, the directory the settings page tells people to use.
pub fn dir(config: &Path) -> PathBuf {
    config.join("plugins")
}

/// Every plugin directory under `plugins`, by name; a later id already taken is refused.
pub fn discover(plugins: &Path) -> Vec<Found> {
    let mut dirs: Vec<PathBuf> = match std::fs::read_dir(plugins) {
        Ok(entries) => entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect(),
        Err(_) => return vec![],
    };
    dirs.sort();
    let off = disabled(plugins);
    let mut found: Vec<Found> = Vec::new();
    for dir in dirs {
        let mut result = read(&dir).map(|mut p| {
            p.enabled = !off.contains(&p.id);
            p
        });
        if let Ok(plugin) = &result
            && found
                .iter()
                .any(|f| f.result.as_ref().is_ok_and(|p| p.id == plugin.id))
        {
            result = Err(format!("id \"{}\" 和前面的插件重名", plugin.id));
        }
        found.push(Found { dir, result });
    }
    found
}

fn read(dir: &Path) -> Result<Plugin, String> {
    let file = dir.join("plugin.json");
    let bytes = std::fs::read(&file).map_err(|_| "缺少 plugin.json".to_owned())?;
    let m: Manifest =
        serde_json::from_slice(&bytes).map_err(|e| format!("plugin.json 格式不对：{e}"))?;
    if m.api != API {
        return Err(format!("插件格式是 api {}，这个版本只认 api {API}", m.api));
    }
    if !valid_id(&m.id) {
        return Err(format!(
            "id \"{}\" 只能用小写字母、数字和 -，不超过 40 个字",
            m.id
        ));
    }
    if m.name.trim().is_empty() {
        return Err("name 不能为空".into());
    }
    if m.provides.is_empty() {
        return Err("provides 至少写一种：agent、actions、rules、sidebar、hooks".into());
    }
    let plugin = Plugin {
        id: m.id,
        name: m.name,
        version: m.version,
        description: m.description,
        dir: dir.to_owned(),
        provides: m.provides,
        enabled: true,
    };
    for kind in &plugin.provides {
        if kind.files(dir).is_empty() {
            return Err(format!("写了提供 {}，但目录里没有它的文件", kind.label()));
        }
    }
    Ok(plugin)
}

/// The id rule every plugin and declaration shares.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 40
        && id.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kl-plugin-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    fn plugin(root: &Path, dir: &str, manifest: &str, files: &[(&str, &str)]) {
        let d = root.join(dir);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("plugin.json"), manifest).unwrap();
        for (f, body) in files {
            let p = d.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        }
    }

    #[test]
    fn a_bad_plugin_is_reported_and_the_rest_load() {
        let root = tmp("discover");
        plugin(
            &root,
            "a-aider",
            r#"{"api":1,"id":"aider","name":"Aider","provides":["agent"]}"#,
            &[("agent.json", "{}"), ("agents/extra.json", "{}")],
        );
        plugin(&root, "b-broken", "{not json", &[]);
        plugin(
            &root,
            "c-future",
            r#"{"api":2,"id":"future","name":"F","provides":["agent"]}"#,
            &[("agent.json", "{}")],
        );
        plugin(
            &root,
            "d-empty",
            r#"{"api":1,"id":"empty","name":"E","provides":["agent"]}"#,
            &[],
        );
        plugin(
            &root,
            "e-dup",
            r#"{"api":1,"id":"aider","name":"Again","provides":["agent"]}"#,
            &[("agent.json", "{}")],
        );
        std::fs::create_dir_all(root.join("f-nomanifest")).unwrap();
        let found = discover(&root);
        let results: Vec<_> = found
            .iter()
            .map(|f| f.result.as_ref().map(|p| p.id.clone()))
            .collect();
        assert_eq!(results[0], Ok("aider".into()));
        assert!(results[1].as_ref().unwrap_err().contains("格式不对"));
        assert!(results[2].as_ref().unwrap_err().contains("api 2"));
        assert!(results[3].as_ref().unwrap_err().contains("没有它的文件"));
        assert!(results[4].as_ref().unwrap_err().contains("重名"));
        assert!(
            results[5]
                .as_ref()
                .unwrap_err()
                .contains("缺少 plugin.json")
        );
        let aider = found[0].result.as_ref().unwrap();
        let files = aider.files(Kind::Agent);
        assert_eq!(files.len(), 2);
        assert!(files[0].ends_with("agent.json"), "agent.json loads first");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn shipped_example_plugins_load() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
        let found = discover(&root);
        assert!(found.len() >= 3);
        for f in &found {
            let p = f.result.as_ref().unwrap_or_else(|e| panic!("{e}: {f:?}"));
            let (_, actions) = crate::action::load(p.files(Kind::Actions));
            let (_, rules) = crate::rule::load(p.files(Kind::Rules));
            let (_, sidebar) = crate::sidebar::load(p.files(Kind::Sidebar));
            let (_, hooks) = crate::hook::load(p.files(Kind::Hooks));
            for loaded in actions.iter().chain(&rules).chain(&sidebar).chain(&hooks) {
                assert!(loaded.result.is_ok(), "{loaded:?}");
            }
        }
    }

    #[test]
    fn a_turned_off_plugin_is_listed_but_loads_nothing() {
        let root = tmp("switch");
        plugin(
            &root,
            "a-aider",
            r#"{"api":1,"id":"aider","name":"Aider","provides":["agent"]}"#,
            &[("agent.json", "{}")],
        );
        std::fs::write(root.join("disabled.json"), r#"["aider"]"#).unwrap();
        let found = discover(&root);
        assert_eq!(found.len(), 1, "the switch file is not a plugin");
        let p = found[0].result.as_ref().unwrap();
        assert!(!p.enabled);
        assert!(p.files(Kind::Agent).is_empty());
        std::fs::write(root.join("disabled.json"), "not json").unwrap();
        assert!(discover(&root)[0].result.as_ref().unwrap().enabled);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn no_plugins_directory_is_no_plugins() {
        assert!(discover(Path::new("/nonexistent/keepline/plugins")).is_empty());
    }
}
