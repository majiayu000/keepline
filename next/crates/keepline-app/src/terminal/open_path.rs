//! What ⌘-clicking a path in a terminal does, decided apart from the pane so the rules
//! can be tested: a folder opens in Finder, a file in its default app, `file:42` in the
//! chosen editor at that line, and anything that would run is only shown in Finder.
use crate::ui::terminal_text::Editor;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan {
    /// Selected in a Finder window; `executable` says why a plain ⌘-click did not open it.
    Reveal {
        executable: bool,
    },
    OpenFolder,
    /// The file's default app.
    OpenFile,
    /// An editor's command line tool, which opens the file at the line.
    Editor {
        name: &'static str,
        program: PathBuf,
        args: Vec<String>,
        line: u32,
    },
    /// `$EDITOR` in a new terminal tab; the command is for `sh`.
    Terminal {
        command: String,
        line: u32,
    },
}

/// Files the system would run, install or hand to a shell rather than show.
pub fn executable(path: &Path) -> bool {
    const RUNS: [&str; 18] = [
        "app",
        "command",
        "tool",
        "sh",
        "bash",
        "zsh",
        "csh",
        "ksh",
        "fish",
        "pkg",
        "mpkg",
        "terminal",
        "workflow",
        "action",
        "jar",
        "prefpane",
        "saver",
        "mobileconfig",
    ];
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    if ext.is_some_and(|e| RUNS.contains(&e.as_str())) {
        return true;
    }
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// An editor's bundled command line tool, found without the user installing it on PATH.
pub fn installed(editor: Editor) -> Option<PathBuf> {
    let (bundles, tool): (&[&str], &str) = match editor {
        Editor::Zed => (&["Zed.app", "Zed Preview.app"], "Contents/MacOS/cli"),
        Editor::VsCode => (
            &["Visual Studio Code.app"],
            "Contents/Resources/app/bin/code",
        ),
        Editor::Cursor => (&["Cursor.app"], "Contents/Resources/app/bin/cursor"),
        _ => return None,
    };
    let home = std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Applications"));
    [Some(PathBuf::from("/Applications")), home]
        .into_iter()
        .flatten()
        .flat_map(|dir| bundles.iter().map(move |b| dir.join(b).join(tool)))
        .find(|p| p.is_file())
}

pub fn editor_name(editor: Editor) -> &'static str {
    match editor {
        Editor::Auto => "自动",
        Editor::Zed => "Zed",
        Editor::VsCode => "VS Code",
        Editor::Cursor => "Cursor",
        Editor::Terminal => "$EDITOR",
        Editor::System => "默认程序",
    }
}

/// `reveal` is ⌘⌥; `at` is the line and column the word named; `find` looks an editor up.
pub fn plan(
    path: &Path,
    at: Option<(u32, Option<u32>)>,
    editor: Editor,
    reveal: bool,
    find: impl Fn(Editor) -> Option<PathBuf>,
) -> Plan {
    if reveal {
        return Plan::Reveal { executable: false };
    }
    let runs = executable(path);
    if path.is_dir() && !runs {
        return Plan::OpenFolder;
    }
    let fallback = if runs {
        Plan::Reveal { executable: true }
    } else {
        Plan::OpenFile
    };
    // An editor never runs what it opens, so a script's line still goes there.
    let Some((line, col)) = at.filter(|_| !path.is_dir()) else {
        return fallback;
    };
    let chosen = match editor {
        Editor::Auto => [Editor::Zed, Editor::VsCode, Editor::Cursor]
            .into_iter()
            .find_map(|e| Some((e, find(e)?))),
        Editor::Zed | Editor::VsCode | Editor::Cursor => find(editor).map(|p| (editor, p)),
        Editor::Terminal => {
            return Plan::Terminal {
                command: format!(
                    "sh -c '${{EDITOR:-vi}} +\"$2\" \"$1\"' sh {} {line}",
                    shell_quote(&path.to_string_lossy())
                ),
                line,
            };
        }
        Editor::System => None,
    };
    let Some((editor, program)) = chosen else {
        return fallback;
    };
    let target = match col {
        Some(col) => format!("{}:{line}:{col}", path.display()),
        None => format!("{}:{line}", path.display()),
    };
    let args = match editor {
        Editor::Zed => vec![target],
        _ => vec!["-g".into(), target],
    };
    Plan::Editor {
        name: editor_name(editor),
        program,
        args,
        line,
    }
}

/// The note shown beside a hovered path while ⌘ is held.
pub fn hint(plan: &Plan) -> String {
    const REVEAL: &str = "⌘⌥ 在访达中显示";
    match plan {
        Plan::Reveal { executable: true } => "可执行文件 · ⌘ 点击在访达中显示，不运行".into(),
        Plan::Reveal { executable: false } => "⌘ 点击在访达中显示".into(),
        Plan::OpenFolder => format!("⌘ 点击在访达中打开 · {REVEAL}"),
        Plan::OpenFile => format!("⌘ 点击打开 · {REVEAL}"),
        Plan::Editor { name, line, .. } => format!("⌘ 点击在 {name} 打开第 {line} 行 · {REVEAL}"),
        Plan::Terminal { line, .. } => {
            format!("⌘ 点击在新标签用 $EDITOR 打开第 {line} 行 · {REVEAL}")
        }
    }
}

/// `path` as the user would type it from `cwd`, when it lies under it.
pub fn relative(path: &Path, cwd: Option<&Path>) -> Option<String> {
    let rest = path.strip_prefix(cwd?).ok()?;
    (!rest.as_os_str().is_empty()).then(|| rest.to_string_lossy().into_owned())
}

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("keepline-open-path-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("Tool.app")).unwrap();
        std::fs::write(dir.join("index.html"), "").unwrap();
        std::fs::write(dir.join("run.sh"), "").unwrap();
        std::fs::write(dir.join("a.rs"), "").unwrap();
        let bin = dir.join("build");
        std::fs::write(&bin, "").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        dir
    }
    fn none(_: Editor) -> Option<PathBuf> {
        None
    }
    #[test]
    fn files_open_folders_open_in_finder_and_programs_are_only_shown() {
        let d = dir();
        let open = |name: &str| plan(&d.join(name), None, Editor::Auto, false, none);
        assert_eq!(open("index.html"), Plan::OpenFile);
        assert_eq!(open(""), Plan::OpenFolder);
        for runs in ["Tool.app", "run.sh", "build"] {
            assert_eq!(open(runs), Plan::Reveal { executable: true }, "{runs}");
        }
        assert_eq!(
            plan(&d.join("index.html"), None, Editor::Auto, true, none),
            Plan::Reveal { executable: false }
        );
        assert!(hint(&open("index.html")).contains("⌘⌥ 在访达中显示"));
        assert!(hint(&open("run.sh")).contains("不运行"));
    }
    #[test]
    fn a_line_goes_to_the_chosen_editor_at_that_line() {
        let d = dir();
        let file = d.join("a.rs");
        let only_cursor = |e: Editor| (e == Editor::Cursor).then(|| PathBuf::from("/c/cursor"));
        let p = plan(&file, Some((42, Some(7))), Editor::Auto, false, only_cursor);
        assert_eq!(
            p,
            Plan::Editor {
                name: "Cursor",
                program: "/c/cursor".into(),
                args: vec!["-g".into(), format!("{}:42:7", file.display())],
                line: 42,
            }
        );
        assert!(hint(&p).contains("Cursor 打开第 42 行"));
        let zed = |_: Editor| Some(PathBuf::from("/z/cli"));
        let Plan::Editor { args, .. } = plan(&file, Some((3, None)), Editor::Zed, false, zed)
        else {
            panic!()
        };
        assert_eq!(args, [format!("{}:3", file.display())]);
        // No editor installed: the default app, which cannot take a line.
        assert_eq!(
            plan(&file, Some((3, None)), Editor::Auto, false, none),
            Plan::OpenFile
        );
        assert_eq!(
            plan(&file, Some((3, None)), Editor::System, false, zed),
            Plan::OpenFile
        );
        // A script's line is safe in an editor, but never run without one.
        let script = d.join("run.sh");
        assert!(matches!(
            plan(&script, Some((1, None)), Editor::Zed, false, zed),
            Plan::Editor { .. }
        ));
        assert_eq!(
            plan(&script, Some((1, None)), Editor::System, false, zed),
            Plan::Reveal { executable: true }
        );
        let Plan::Terminal { command, .. } =
            plan(&file, Some((9, None)), Editor::Terminal, false, none)
        else {
            panic!()
        };
        assert!(command.ends_with(&format!("'{}' 9", file.display())));
    }
    #[test]
    fn relative_paths_only_under_the_working_directory() {
        let cwd = Path::new("/work/app");
        assert_eq!(
            relative(Path::new("/work/app/src/a.rs"), Some(cwd)).as_deref(),
            Some("src/a.rs")
        );
        assert_eq!(relative(Path::new("/etc/hosts"), Some(cwd)), None);
        assert_eq!(relative(cwd, Some(cwd)), None);
        assert_eq!(relative(Path::new("/work/app/a"), None), None);
    }
}
