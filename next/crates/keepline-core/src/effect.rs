//! How the app plays 标记完成 (`effects.json`). A plugin picks one of the animations the
//! app draws and tunes it; it never draws anything itself, so every style follows the
//! theme and the status colors. The first plugin that sets the completion animation wins;
//! a later one is reported as not taking effect. What settings picks
//! (`~/.config/keepline/complete`, one style name) beats every plugin and applies at once.
//!
//! ```json
//! {"api": 1, "complete": {"style": "stamp", "duration_ms": 2400, "land": "sidebar"}}
//! ```
use crate::agent::Loaded;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// The `effects.json` format's major version.
pub const API: u32 = 1;
/// Short enough to be seen, long enough to reach 「留着这个画面」.
pub const MIN_MS: u64 = 600;
pub const MAX_MS: u64 = 4_000;

/// The animations the app can play when a session is marked complete.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    /// The button fills, a seal pops over the pane, the pane fades: the built-in one.
    #[default]
    Seal,
    /// The seal is pressed down with a shockwave and a burst of specks.
    Stamp,
    /// A light sweeps the terminal, then the pane closes onto a line like an eyelid.
    Sweep,
    /// The terminal's text turns into specks that stream into the sidebar.
    Dissolve,
    /// Only a short fade; also what reduced motion plays.
    Fade,
}
impl Style {
    pub const ALL: [Self; 5] = [
        Self::Seal,
        Self::Stamp,
        Self::Sweep,
        Self::Dissolve,
        Self::Fade,
    ];
    /// The name `effects.json` and the settings file write.
    pub fn name(self) -> &'static str {
        match self {
            Self::Seal => "seal",
            Self::Stamp => "stamp",
            Self::Sweep => "sweep",
            Self::Dissolve => "dissolve",
            Self::Fade => "fade",
        }
    }
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == text.trim())
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Seal => "印章",
            Self::Stamp => "落印",
            Self::Sweep => "扫光合上",
            Self::Dissolve => "化进侧栏",
            Self::Fade => "只淡出",
        }
    }
    pub fn default_ms(self) -> u64 {
        match self {
            Self::Seal => 1_600,
            Self::Stamp => 2_400,
            Self::Sweep => 2_200,
            Self::Dissolve => 2_800,
            Self::Fade => 600,
        }
    }
    /// Whether the style ends by flying into the session's sidebar row unless told not to.
    pub fn lands_by_default(self) -> bool {
        matches!(self, Self::Stamp | Self::Sweep | Self::Dissolve)
    }
}

/// Where the finished session is shown going when the pane closes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Land {
    /// Into its row in the sidebar, which lights up.
    Sidebar,
    /// Nowhere: the pane just goes.
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Complete {
    pub style: Style,
    pub duration: Duration,
    /// Whether it ends in the sidebar; the dissolve's specks always head there.
    pub land: bool,
}
impl Default for Complete {
    fn default() -> Self {
        Self::of(Style::Seal)
    }
}
impl Complete {
    pub fn of(style: Style) -> Self {
        Self {
            style,
            duration: Duration::from_millis(style.default_ms()),
            land: style.lands_by_default(),
        }
    }
    /// What actually plays: reduced motion turns every style into the short fade.
    pub fn played(self, reduce_motion: bool) -> Self {
        if reduce_motion && self.style != Style::Fade {
            Self::of(Style::Fade)
        } else {
            self
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    api: u32,
    #[serde(default)]
    complete: Option<CompleteFile>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CompleteFile {
    style: Style,
    #[serde(default)]
    duration_ms: Option<u64>,
    #[serde(default)]
    land: Option<Land>,
}

fn read(file: &Path) -> Result<Option<Complete>, String> {
    let bytes = std::fs::read(file).map_err(|e| format!("读不了：{e}"))?;
    let f: File = serde_json::from_slice(&bytes).map_err(|e| format!("格式不对：{e}"))?;
    if f.api != API {
        return Err(format!("动效格式是 api {}，这个版本只认 api {API}", f.api));
    }
    let Some(c) = f.complete else {
        return Ok(None);
    };
    let ms = c.duration_ms.unwrap_or(c.style.default_ms());
    if !(MIN_MS..=MAX_MS).contains(&ms) {
        return Err(format!(
            "duration_ms 要在 {MIN_MS}–{MAX_MS} 之间，写的是 {ms}"
        ));
    }
    Ok(Some(Complete {
        style: c.style,
        duration: Duration::from_millis(ms),
        land: match c.land {
            Some(l) => l == Land::Sidebar,
            None => c.style.lands_by_default(),
        },
    }))
}

fn describe(c: &Complete) -> String {
    format!(
        "标记完成时播放「{}」，{:.1} 秒{}",
        c.style.label(),
        c.duration.as_secs_f32(),
        if c.land {
            "，最后落到侧栏那一行"
        } else {
            ""
        }
    )
}

/// The first file that sets the completion animation wins; later ones are reported.
pub fn load(files: Vec<PathBuf>) -> (Option<Complete>, Vec<Loaded>) {
    let mut chosen: Option<(Complete, PathBuf)> = None;
    let mut report = Vec::new();
    for file in files {
        let result = match read(&file) {
            Ok(Some(c)) => match &chosen {
                Some((_, by)) => Err(format!(
                    "{} 已经定了完成动画，这个没有生效；在设置里关掉那个插件就会用这个",
                    by.parent().unwrap_or(by).display()
                )),
                None => {
                    chosen = Some((c, file.clone()));
                    Ok(describe(&c))
                }
            },
            Ok(None) => Ok("没有设置任何动效".into()),
            Err(e) => Err(e),
        };
        report.push(Loaded { file, result });
    }
    (chosen.map(|(c, _)| c), report)
}

static EFFECTS: OnceLock<(Option<Complete>, Vec<Loaded>)> = OnceLock::new();
static REDUCE_MOTION: AtomicBool = AtomicBool::new(false);
/// The style picked in settings; None follows the plugins.
static PICKED: Mutex<Option<Style>> = Mutex::new(None);

fn picked_path() -> Option<PathBuf> {
    crate::agent::config_dir().map(|d| d.join("complete"))
}

/// Loads the installed plugins' effects once (after `plugin::install`) and the style
/// picked in settings, and reads the system's reduce-motion setting off the main thread.
pub fn install() {
    EFFECTS.get_or_init(|| load(crate::plugin::files(crate::plugin::Kind::Effects)));
    let saved = picked_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| Style::parse(&t));
    *PICKED.lock().unwrap_or_else(|e| e.into_inner()) = saved;
    #[cfg(target_os = "macos")]
    std::thread::spawn(|| {
        let on = std::process::Command::new("defaults")
            .args(["read", "com.apple.universalaccess", "reduceMotion"])
            .stderr(std::process::Stdio::null())
            .output()
            .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "1");
        REDUCE_MOTION.store(on, Ordering::Relaxed);
    });
}
/// What a plugin set, if any.
pub fn from_plugin() -> Option<Complete> {
    EFFECTS.get().and_then(|(c, _)| *c)
}
/// The style picked in settings, if one is.
pub fn picked() -> Option<Style> {
    *PICKED.lock().unwrap_or_else(|e| e.into_inner())
}
/// Picks the style from now on and keeps it for the next start; None follows the plugins.
pub fn pick(style: Option<Style>) -> Result<(), String> {
    let path = picked_path().ok_or("找不到配置目录")?;
    match style {
        Some(s) => path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&path, s.name())),
        None => match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        },
    }
    .map_err(|e| format!("无法保存 {}：{e}", path.display()))?;
    *PICKED.lock().unwrap_or_else(|e| e.into_inner()) = style;
    Ok(())
}
/// Whether macOS asks for reduced motion, which turns every style into the fade.
pub fn reduce_motion() -> bool {
    REDUCE_MOTION.load(Ordering::Relaxed)
}
/// The animation chosen, before reduced motion: settings' pick (with a plugin's tuning
/// when it set the same style), else a plugin's, else the built-in seal.
pub fn chosen(picked: Option<Style>, plugin: Option<Complete>) -> Complete {
    match (picked, plugin) {
        (Some(s), Some(p)) if p.style == s => p,
        (Some(s), _) => Complete::of(s),
        (None, Some(p)) => p,
        (None, None) => Complete::default(),
    }
}
/// The completion animation to play now.
pub fn complete() -> Complete {
    chosen(picked(), from_plugin()).played(reduce_motion())
}
pub fn report() -> &'static [Loaded] {
    EFFECTS.get().map_or(&[], |(_, r)| r.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_files_pick_one_animation_and_report_the_rest() {
        let d = std::env::temp_dir().join(format!("kl-effects-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let write = |name: &str, body: &str| {
            let p = d.join(name);
            std::fs::write(&p, body).unwrap();
            p
        };
        let stamp = write("a.json", r#"{"api":1,"complete":{"style":"stamp"}}"#);
        let sweep = write(
            "b.json",
            r#"{"api":1,"complete":{"style":"sweep","duration_ms":1800,"land":"none"}}"#,
        );
        let long = write(
            "c.json",
            r#"{"api":1,"complete":{"style":"fade","duration_ms":9000}}"#,
        );
        let odd = write("d.json", r#"{"api":1,"complete":{"style":"confetti"}}"#);
        let empty = write("e.json", r#"{"api":1}"#);

        let (chosen, report) = load(vec![stamp.clone(), sweep.clone(), long, odd, empty]);
        let c = chosen.unwrap();
        assert_eq!(c.style, Style::Stamp);
        assert_eq!(c.duration, Duration::from_millis(2_400));
        assert!(c.land, "stamp lands in the sidebar unless told not to");
        assert!(report[0].result.as_ref().unwrap().contains("落印"));
        assert!(report[1].result.as_ref().unwrap_err().contains("已经定了"));
        assert!(report[2].result.as_ref().unwrap_err().contains("600–4000"));
        assert!(report[3].result.as_ref().unwrap_err().contains("格式不对"));
        assert!(report[4].result.is_ok());

        let (chosen, _) = load(vec![sweep]);
        let c = chosen.unwrap();
        assert_eq!(
            (c.style, c.duration, c.land),
            (Style::Sweep, Duration::from_millis(1_800), false)
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn reduced_motion_only_fades() {
        let c = Complete::of(Style::Dissolve);
        assert_eq!(c.played(false), c);
        assert_eq!(c.played(true).style, Style::Fade);
        assert!(!c.played(true).land);
    }

    #[test]
    fn the_settings_pick_beats_plugins_and_keeps_their_tuning_for_the_same_style() {
        let tuned = Complete {
            style: Style::Sweep,
            duration: Duration::from_millis(3_000),
            land: false,
        };
        assert_eq!(chosen(None, None).style, Style::Seal);
        assert_eq!(chosen(None, Some(tuned)), tuned);
        assert_eq!(chosen(Some(Style::Sweep), Some(tuned)), tuned);
        assert_eq!(
            chosen(Some(Style::Stamp), Some(tuned)),
            Complete::of(Style::Stamp)
        );
        for s in Style::ALL {
            assert_eq!(Style::parse(&format!("{}\n", s.name())), Some(s));
            let json: Style = serde_json::from_value(serde_json::json!(s.name())).unwrap();
            assert_eq!(json, s);
        }
        assert_eq!(Style::parse("fancy"), None);
    }
}
