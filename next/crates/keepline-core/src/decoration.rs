//! Short facts a plugin puts on each pane (`decorations.json`): a command run in the pane's
//! directory whose first line of output is shown in the pane's title bar or in its title's
//! hover card, like a tmux status segment. `node -v`, a kube context, a ticket number.
//!
//! The app never waits for one: each runs off the UI thread under its timeout and the last
//! line it printed stays until the next run, every `refresh_s` while the pane is drawn. A
//! command that exits non-zero or prints nothing shows nothing, so a script can opt out of
//! directories it does not apply to.
use crate::agent::Loaded;
use serde::Deserialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// The `decorations.json` format's major version.
pub const API: u32 = 1;
/// The longest a decoration may take before it is stopped.
pub const MAX_TIMEOUT_MS: u64 = 5_000;
/// Title bar room is scarce; the hover card has more.
pub const TITLE_CHARS: usize = 40;
pub const TIP_CHARS: usize = 120;

/// Where the line shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Place {
    /// In the pane's title bar, before the Agent and its time; the default.
    #[default]
    Title,
    /// A line of its own in the title's hover card.
    Tip,
}

/// Which panes it is run for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum When {
    #[default]
    Always,
    /// Panes running an Agent session Keepline has bound.
    Agent,
    /// Panes with no Agent session: a plain shell.
    Shell,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decoration {
    pub id: String,
    /// Run with `sh -c` in the pane's directory; `KEEPLINE_CWD`, `KEEPLINE_AGENT` and
    /// `KEEPLINE_SESSION` say which pane it is for.
    pub command: String,
    #[serde(default)]
    pub place: Place,
    #[serde(default)]
    pub when: When,
    /// How often it is run again while shown.
    #[serde(default = "default_refresh")]
    pub refresh_s: u64,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(skip)]
    pub file: PathBuf,
}
fn default_refresh() -> u64 {
    10
}
fn default_timeout() -> u64 {
    1_000
}

/// The pane a decoration is run for.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Pane {
    pub cwd: String,
    /// The Agent's name, when the pane runs a bound session.
    pub agent: Option<String>,
    pub session: Option<String>,
}

impl Decoration {
    pub fn shown(&self, pane: &Pane) -> bool {
        match self.when {
            When::Always => true,
            When::Agent => pane.session.is_some(),
            When::Shell => pane.session.is_none(),
        }
    }
    pub fn refresh(&self) -> Duration {
        Duration::from_secs(self.refresh_s)
    }
    /// Runs it for `pane` (blocking, up to its timeout): the line to show, or why none.
    pub fn run(&self, pane: &Pane) -> Result<Option<String>, String> {
        let mut child = Command::new("/bin/sh")
            .arg("-c")
            .arg(&self.command)
            .current_dir(&pane.cwd)
            .env("KEEPLINE_CWD", &pane.cwd)
            .env("KEEPLINE_AGENT", pane.agent.as_deref().unwrap_or(""))
            .env("KEEPLINE_SESSION", pane.session.as_deref().unwrap_or(""))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("启动不了：{e}"))?;
        // Read on its own thread, so a command that fills the pipe still ends.
        let mut stdout = child.stdout.take().expect("piped");
        let reader = std::thread::spawn(move || {
            let mut out = Vec::new();
            let _ = stdout.by_ref().take(64 * 1024).read_to_end(&mut out);
            out
        });
        let deadline = Instant::now() + Duration::from_millis(self.timeout_ms);
        let status = loop {
            match child.try_wait().map_err(|e| e.to_string())? {
                Some(status) => break status,
                None if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("超过 {} ms，已停止", self.timeout_ms));
                }
                None => std::thread::sleep(Duration::from_millis(10)),
            }
        };
        let out = reader.join().unwrap_or_default();
        if !status.success() {
            return Ok(None);
        }
        let limit = match self.place {
            Place::Title => TITLE_CHARS,
            Place::Tip => TIP_CHARS,
        };
        Ok(String::from_utf8_lossy(&out)
            .lines()
            .map(|l| strip_controls(l.trim()))
            .find(|l| !l.is_empty())
            .map(|l| crate::content::short(&l, limit)))
    }
}

/// Text only: escape sequences and other control characters a command printed are dropped.
fn strip_controls(line: &str) -> String {
    let mut out = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // CSI: ESC [ … a final byte in @..~. OSC: ESC ] … BEL or ESC \. Anything
            // else: the one character after ESC.
            match chars.next() {
                Some('[') => while chars.next().is_some_and(|c| !('@'..='~').contains(&c)) {},
                Some(']') => {
                    while let Some(c) = chars.next() {
                        if c == '\x07' || (c == '\x1b' && chars.next_if_eq(&'\\').is_some()) {
                            break;
                        }
                    }
                }
                _ => {}
            }
        } else if !c.is_control() {
            out.push(c);
        }
    }
    out.trim().to_owned()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    api: u32,
    decorations: Vec<Decoration>,
}

fn read(file: &Path, taken: &[Decoration]) -> Result<Vec<Decoration>, String> {
    let bytes = std::fs::read(file).map_err(|e| format!("读不了：{e}"))?;
    let f: File = serde_json::from_slice(&bytes).map_err(|e| format!("格式不对：{e}"))?;
    if f.api != API {
        return Err(format!("装饰格式是 api {}，这个版本只认 api {API}", f.api));
    }
    if f.decorations.is_empty() {
        return Err("decorations 至少写一条".into());
    }
    let mut found = f.decorations;
    for i in 0..found.len() {
        let (before, rest) = found.split_at_mut(i);
        let d = &mut rest[0];
        if !crate::plugin::valid_id(&d.id) {
            return Err(format!(
                "装饰 id \"{}\" 只能用小写字母、数字和 -，不超过 40 个字",
                d.id
            ));
        }
        if taken.iter().chain(before.iter()).any(|t| t.id == d.id) {
            return Err(format!("装饰 id \"{}\" 和前面的插件重名", d.id));
        }
        if d.command.trim().is_empty() {
            return Err(format!("装饰 {} 要写 command", d.id));
        }
        if !(2..=3600).contains(&d.refresh_s) {
            return Err(format!("装饰 {} 的 refresh_s 要在 2 到 3600 之间", d.id));
        }
        if !(100..=MAX_TIMEOUT_MS).contains(&d.timeout_ms) {
            return Err(format!(
                "装饰 {} 的 timeout_ms 要在 100 到 {MAX_TIMEOUT_MS} 之间",
                d.id
            ));
        }
        d.file = file.to_owned();
    }
    Ok(found)
}

pub fn load(files: Vec<PathBuf>) -> (Vec<Decoration>, Vec<Loaded>) {
    let mut all: Vec<Decoration> = Vec::new();
    let mut report = Vec::new();
    for file in files {
        let result = read(&file, &all);
        report.push(Loaded {
            file: file.clone(),
            result: result
                .as_ref()
                .map(|found| {
                    found
                        .iter()
                        .map(|d| d.id.as_str())
                        .collect::<Vec<_>>()
                        .join("、")
                })
                .map_err(Clone::clone),
        });
        if let Ok(found) = result {
            all.extend(found);
        }
    }
    (all, report)
}

static DECORATIONS: OnceLock<(Vec<Decoration>, Vec<Loaded>)> = OnceLock::new();

/// Loads the installed plugins' decorations once (after `plugin::install`).
pub fn install() {
    DECORATIONS.get_or_init(|| load(crate::plugin::files(crate::plugin::Kind::Decorations)));
}
pub fn all() -> &'static [Decoration] {
    DECORATIONS.get().map_or(&[], |(d, _)| d.as_slice())
}
pub fn report() -> &'static [Loaded] {
    DECORATIONS.get().map_or(&[], |(_, r)| r.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decoration(command: &str, place: Place, timeout_ms: u64) -> Decoration {
        Decoration {
            id: "t".into(),
            command: command.into(),
            place,
            when: When::Always,
            refresh_s: 10,
            timeout_ms,
            file: PathBuf::new(),
        }
    }
    fn pane(cwd: &Path) -> Pane {
        Pane {
            cwd: cwd.display().to_string(),
            agent: Some("Codex".into()),
            session: Some("s1".into()),
        }
    }

    #[test]
    fn the_first_line_printed_in_the_panes_directory_is_shown_as_text() {
        let dir = std::env::temp_dir().join(format!("kl-deco-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("marker"), "").unwrap();
        let shown =
            |command: &str, place| decoration(command, place, 2_000).run(&pane(&dir)).unwrap();
        assert_eq!(
            shown("\n  ls; echo second", Place::Title).as_deref(),
            Some("marker")
        );
        assert_eq!(
            shown(
                "printf '%s|%s' \"$KEEPLINE_AGENT\" \"$KEEPLINE_SESSION\"",
                Place::Title
            )
            .as_deref(),
            Some("Codex|s1")
        );
        assert_eq!(
            shown(r"printf '\033[31m红\033[0m\a\033]0;x\a ok'", Place::Title).as_deref(),
            Some("红 ok"),
            "colors, titles and bells are dropped, text kept"
        );
        assert_eq!(shown("echo nope; exit 1", Place::Title), None);
        assert_eq!(shown("true", Place::Title), None);
        let long = "字".repeat(200);
        let title = shown(&format!("echo {long}"), Place::Title).unwrap();
        let tip = shown(&format!("echo {long}"), Place::Tip).unwrap();
        assert!(title.chars().count() <= TITLE_CHARS + 1, "{title}");
        assert!(tip.chars().count() > TITLE_CHARS + 1 && tip.chars().count() <= TIP_CHARS + 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_slow_or_noisy_command_is_stopped_at_its_timeout() {
        let dir = std::env::temp_dir();
        let at = Instant::now();
        let e = decoration("sleep 5", Place::Title, 200)
            .run(&pane(&dir))
            .unwrap_err();
        assert!(e.contains("200 ms"), "{e}");
        assert!(at.elapsed() < Duration::from_secs(2));
        // Endless output is read only so far, and the command still ends.
        let at = Instant::now();
        let _ = decoration("yes", Place::Title, 300).run(&pane(&dir));
        assert!(at.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn only_sane_declarations_load() {
        let d = std::env::temp_dir().join(format!("kl-deco-load-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let write = |name: &str, body: &str| {
            let p = d.join(name);
            std::fs::write(&p, body).unwrap();
            p
        };
        let good = write(
            "a.json",
            r#"{"api":1,"decorations":[{"id":"node","command":"node -v","when":"shell"},{"id":"ctx","command":"kubectl config current-context","place":"tip","refresh_s":60,"timeout_ms":3000}]}"#,
        );
        let fast = write(
            "b.json",
            r#"{"api":1,"decorations":[{"id":"x","command":"true","refresh_s":1}]}"#,
        );
        let place = write(
            "c.json",
            r#"{"api":1,"decorations":[{"id":"y","command":"true","place":"footer"}]}"#,
        );
        let dup = write(
            "d.json",
            r#"{"api":1,"decorations":[{"id":"node","command":"true"}]}"#,
        );
        let (all, report) = load(vec![good, fast, place, dup]);
        assert_eq!(all.len(), 2);
        assert_eq!(
            (all[0].place, all[0].when, all[0].refresh_s),
            (Place::Title, When::Shell, 10)
        );
        assert_eq!((all[1].place, all[1].timeout_ms), (Place::Tip, 3000));
        assert_eq!(report[0].result, Ok("node、ctx".into()));
        assert!(report[1].result.as_ref().unwrap_err().contains("refresh_s"));
        assert!(report[2].result.as_ref().unwrap_err().contains("格式不对"));
        assert!(report[3].result.as_ref().unwrap_err().contains("重名"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
