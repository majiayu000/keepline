//! `keeplined plugins`: checks the plugins under the config directory with the same code
//! the app loads them with, so an Agent that wrote one knows before a restart whether it
//! will take effect, and why not. It reads only; it never changes a plugin.
use keepline_core::plugin::{self, Kind};
use std::io;
use std::path::{Path, PathBuf};

pub const USAGE: &str = "用法：keeplined plugins [--dir 插件目录] [--json]

检查插件能不能装上：逐个读 plugin.json 和它提供的文件，说哪个生效、哪个为什么没生效。
默认目录是 ${XDG_CONFIG_HOME:-$HOME/.config}/keepline/plugins。有问题时以 1 退出。
改动在 Keepline 下次启动时生效。写插件的说明：keeplined --skill plugins";

/// The first line of the `AGENTS.md` Keepline writes; a file without it is the user's.
const MARK: &str =
    "<!-- 由 Keepline 生成，每次启动会更新；要写自己的说明就删掉这一行，Keepline 就不再覆盖 -->";
const AGENTS_MD: &str = "# Keepline 配置目录

这里是 Keepline 的配置。用户让你改 Keepline 时（加右键菜单或 ⌘P 里的命令、换左栏排法、在窗格上显示点什么、会话等你或做完时发通知、换完成动画、改状态颜色）：

1. 先运行 `keeplined --skill plugins`，读完整说明：插件格式、规矩和例子。
2. 插件写在 `plugins/<id>/`，一个想法一个插件。写完运行 `keeplined plugins` 检查，全是 ✓ 才算写好。
3. 改动在 Keepline 下次启动时生效，告诉用户重启后去哪里看。

别改用户已有的插件和 `plugins/disabled.json`；快捷键、终端字体、完成动画在 App 的「设置」里改。
在 Keepline 打开的终端里 `keeplined` 直接能用；驱动别的终端的说明是 `keeplined --skill`。
";

/// Writes `<config>/AGENTS.md`, the note an Agent finds when it opens the config
/// directory, unless the user wrote their own there.
pub fn write_agents_md() {
    let Some(dir) = keepline_core::agent::config_dir() else {
        return;
    };
    if let Err(e) = write_note(&dir) {
        eprintln!("AGENTS.md: {e}");
    }
}
fn write_note(dir: &Path) -> io::Result<()> {
    let path = dir.join("AGENTS.md");
    let want = format!("{MARK}\n{AGENTS_MD}");
    match std::fs::read_to_string(&path) {
        Ok(text) if text == want || !text.starts_with(MARK) => return Ok(()),
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    std::fs::create_dir_all(dir)?;
    let temporary = path.with_extension("md.tmp");
    std::fs::write(&temporary, want)?;
    std::fs::rename(temporary, path)
}

const KINDS: [Kind; 7] = [
    Kind::Agent,
    Kind::Actions,
    Kind::Rules,
    Kind::Sidebar,
    Kind::Hooks,
    Kind::Decorations,
    Kind::Effects,
];

#[derive(serde::Serialize)]
struct Checked {
    dir: PathBuf,
    id: Option<String>,
    name: Option<String>,
    enabled: bool,
    /// Why the plugin as a whole cannot load.
    error: Option<String>,
    files: Vec<File>,
    /// Files it has but does not declare: they never load.
    unused: Vec<String>,
}

#[derive(serde::Serialize)]
struct File {
    kind: &'static str,
    file: PathBuf,
    ok: Option<String>,
    error: Option<String>,
}

pub fn run(mut args: impl Iterator<Item = String>) -> io::Result<()> {
    let mut dir = None;
    let mut json = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dir" => dir = args.next().map(PathBuf::from),
            "--json" => json = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            _ => return Err(io::Error::other(USAGE)),
        }
    }
    let dir = match dir {
        Some(dir) => dir,
        None => plugin::dir(
            &keepline_core::agent::config_dir()
                .ok_or_else(|| io::Error::other("找不到配置目录：HOME 没有设置"))?,
        ),
    };
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let checked = check(&dir, &home);
    let bad = checked
        .iter()
        .any(|c| c.error.is_some() || c.files.iter().any(|f| f.error.is_some()));
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&checked).map_err(io::Error::other)?
        );
    } else {
        print!("{}", render(&dir, &checked));
    }
    if bad {
        std::process::exit(1);
    }
    Ok(())
}

fn check(dir: &Path, home: &Path) -> Vec<Checked> {
    plugin::discover(dir)
        .into_iter()
        .map(|found| {
            let undeclared = |provides: &[Kind]| {
                KINDS
                    .into_iter()
                    .filter(|k| !provides.contains(k))
                    .flat_map(|k| k.files(&found.dir))
                    .map(|f| name(&found.dir, &f))
                    .collect()
            };
            match found.result {
                Err(error) => Checked {
                    unused: vec![],
                    dir: found.dir,
                    id: None,
                    name: None,
                    enabled: false,
                    error: Some(error),
                    files: vec![],
                },
                Ok(p) => Checked {
                    unused: undeclared(&p.provides),
                    files: p
                        .provides
                        .iter()
                        .flat_map(|k| load(*k, k.files(&p.dir), home))
                        .collect(),
                    dir: p.dir,
                    id: Some(p.id),
                    name: Some(p.name),
                    enabled: p.enabled,
                    error: None,
                },
            }
        })
        .collect()
}

/// One kind's files through the loader the app uses.
fn load(kind: Kind, files: Vec<PathBuf>, home: &Path) -> Vec<File> {
    use keepline_core::*;
    let report = match kind {
        Kind::Agent => agent::load_files(files, home).1,
        Kind::Actions => action::load(files).1,
        Kind::Rules => rule::load(files).1,
        Kind::Sidebar => sidebar::load(files).1,
        Kind::Hooks => hook::load(files).1,
        Kind::Decorations => decoration::load(files).1,
        Kind::Effects => effect::load(files).1,
    };
    report
        .into_iter()
        .map(|l| {
            let (ok, error) = match l.result {
                Ok(ok) => (Some(ok), None),
                Err(e) => (None, Some(e)),
            };
            File {
                kind: kind.label(),
                file: l.file,
                ok,
                error,
            }
        })
        .collect()
}

fn name(dir: &Path, file: &Path) -> String {
    file.strip_prefix(dir).unwrap_or(file).display().to_string()
}

fn render(dir: &Path, checked: &[Checked]) -> String {
    let mut out = String::new();
    if checked.is_empty() {
        out.push_str(&format!("{} 下没有插件\n", dir.display()));
        return out;
    }
    for c in checked {
        let folder = c.dir.file_name().unwrap_or_default().to_string_lossy();
        match (&c.error, &c.id) {
            (Some(error), _) => out.push_str(&format!("✗ {folder}：{error}\n")),
            (None, Some(id)) => {
                let bad = c.files.iter().any(|f| f.error.is_some());
                let off = if c.enabled {
                    ""
                } else {
                    "（设置里关掉了）"
                };
                out.push_str(&format!(
                    "{} {id}「{}」{off}\n",
                    if bad { "✗" } else { "✓" },
                    c.name.as_deref().unwrap_or_default()
                ));
                for f in &c.files {
                    let file = name(&c.dir, &f.file);
                    match (&f.ok, &f.error) {
                        (_, Some(e)) => out.push_str(&format!("    ✗ {} {file}：{e}\n", f.kind)),
                        (Some(ok), None) => {
                            out.push_str(&format!("    ✓ {} {file}：{ok}\n", f.kind))
                        }
                        (None, None) => {}
                    }
                }
                for file in &c.unused {
                    out.push_str(&format!(
                        "    ! {file} 不会加载：plugin.json 的 provides 里没写它的类型\n"
                    ));
                }
            }
            (None, None) => {}
        }
    }
    out.push_str("改动在 Keepline 下次启动时生效。\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn every_plugin_and_file_says_whether_it_takes_effect() {
        let root = std::env::temp_dir().join(format!("kl-plugins-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        write(
            &root.join("good/plugin.json"),
            r#"{"api":1,"id":"good","name":"好的","provides":["actions"]}"#,
        );
        write(
            &root.join("good/actions.json"),
            r#"{"api":1,"actions":[{"id":"st","title":"状态","command":"git -C {cwd} status"}]}"#,
        );
        write(
            &root.join("good/rules.json"),
            r#"{"api":1,"verify":[["make"]]}"#,
        );
        write(
            &root.join("badfile/plugin.json"),
            r#"{"api":1,"id":"badfile","name":"坏文件","provides":["effects"]}"#,
        );
        write(
            &root.join("badfile/effects.json"),
            r#"{"api":1,"complete":{"style":"stamp","duration_ms":99999}}"#,
        );
        write(&root.join("nomanifest/actions.json"), "{}");
        let checked = check(&root, &root);
        let text = render(&root, &checked);
        assert!(text.contains("✓ good「好的」"), "{text}");
        assert!(text.contains("✓ 动作 actions.json"), "{text}");
        assert!(text.contains("! rules.json 不会加载"), "{text}");
        assert!(text.contains("✗ badfile「坏文件」"), "{text}");
        assert!(text.contains("duration_ms 要在"), "{text}");
        assert!(text.contains("✗ nomanifest：缺少 plugin.json"), "{text}");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_note_is_written_kept_current_and_never_over_the_users_own() {
        let dir = std::env::temp_dir().join(format!("kl-agents-md-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write_note(&dir).unwrap();
        let path = dir.join("AGENTS.md");
        let ours = std::fs::read_to_string(&path).unwrap();
        assert!(ours.starts_with(MARK) && ours.contains("keeplined --skill plugins"));
        std::fs::write(&path, format!("{MARK}\n旧版本")).unwrap();
        write_note(&dir).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            ours,
            "an old note is updated"
        );
        std::fs::write(&path, "# 我自己的说明").unwrap();
        write_note(&dir).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "# 我自己的说明");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Every JSON example in the guide is a file the loaders accept as written.
    #[test]
    fn the_guides_examples_all_load() {
        let guide = include_str!("plugins.md");
        let root = std::env::temp_dir().join(format!("kl-guide-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut n = 0;
        for block in guide.split("```json\n").skip(1) {
            let text = block.split("```").next().unwrap();
            let value: serde_json::Value =
                serde_json::from_str(text).unwrap_or_else(|e| panic!("not JSON ({e}):\n{text}"));
            let file = [
                "actions",
                "presets",
                "decorations",
                "hooks",
                "verify",
                "complete",
            ]
            .into_iter()
            .find(|k| value.get(k).is_some())
            .map(|k| match k {
                "presets" => "sidebar",
                "verify" => "rules",
                "complete" => "effects",
                other => other,
            });
            let Some(kind) = file else {
                continue; // plugin.json or colors.json: checked below / by the app
            };
            n += 1;
            let dir = root.join(format!("example-{n}"));
            write(
                &dir.join("plugin.json"),
                &format!(r#"{{"api":1,"id":"example-{n}","name":"例子","provides":["{kind}"]}}"#),
            );
            write(&dir.join(format!("{kind}.json")), text);
        }
        assert!(n >= 6, "examples found: {n}");
        for c in check(&root, &root) {
            assert!(c.error.is_none(), "{}: {:?}", c.dir.display(), c.error);
            for f in &c.files {
                assert!(
                    f.error.is_none(),
                    "{}: {:?}\n{}",
                    f.file.display(),
                    f.error,
                    std::fs::read_to_string(&f.file).unwrap()
                );
            }
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// The plugins shipped in the repo are the guide's examples: they must all check out.
    #[test]
    fn the_bundled_example_plugins_all_take_effect() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
        let checked = check(&dir, Path::new("/nonexistent-home"));
        assert!(checked.len() >= 10);
        for c in &checked {
            assert!(c.error.is_none(), "{}: {:?}", c.dir.display(), c.error);
            for f in &c.files {
                assert!(f.error.is_none(), "{}: {:?}", f.file.display(), f.error);
            }
        }
    }
}
