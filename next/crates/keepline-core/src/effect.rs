//! How the app plays 标记完成 (`effects.json`). A plugin names one of the animations the
//! app draws and tunes it; it never draws anything itself, so every style follows the
//! theme and the status colors. Each plugin's tuning becomes one more choice beside the
//! built-in ones in settings; none of them takes over by being installed. What settings
//! picks (`~/.config/keepline/complete`: a style name, or `plugin:<id>`) applies at once;
//! nothing picked plays the built-in seal.
//!
//! ```json
//! {"api": 1, "complete": {"style": "stamp", "duration_ms": 2400, "land": "sidebar"}}
//! ```
use crate::agent::Loaded;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
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

/// One plugin's tuning, offered in settings beside the built-in styles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preset {
    /// The plugin's id, which settings saves as `plugin:<id>`.
    pub id: String,
    /// The plugin's name, shown on the choice.
    pub name: String,
    pub complete: Complete,
}
impl Preset {
    /// Plays exactly what choosing the built-in style of the same name plays.
    pub fn same_as_builtin(&self) -> bool {
        self.complete == Complete::of(self.complete.style)
    }
}

/// The plugin a file belongs to: the loaded plugin whose directory holds it, else the
/// directory's name.
fn owner(file: &Path) -> (String, String) {
    let dir = file.parent().unwrap_or(file);
    if let Some(p) = crate::plugin::report()
        .iter()
        .filter_map(|f| f.result.as_ref().ok())
        .find(|p| p.dir == dir)
    {
        return (p.id.clone(), p.name.clone());
    }
    let name = dir
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    (name.clone(), name)
}

/// Every file that sets the completion animation becomes a preset; a plugin's second file
/// or a preset id already taken is reported, not offered twice.
pub fn load(files: Vec<PathBuf>) -> (Vec<Preset>, Vec<Loaded>) {
    let mut presets: Vec<Preset> = Vec::new();
    let mut report = Vec::new();
    for file in files {
        let result = match read(&file) {
            Ok(Some(c)) => {
                let (id, name) = owner(&file);
                if presets.iter().any(|p| p.id == id) {
                    Err(format!("{id} 已经有一个完成动画，这个没有加进选项"))
                } else {
                    let preset = Preset {
                        id,
                        name,
                        complete: c,
                    };
                    let did = if preset.same_as_builtin() {
                        format!(
                            "{}；和内置的「{}」一样，在外观 › 标记完成动画里选它就是",
                            describe(&c),
                            c.style.label()
                        )
                    } else {
                        format!(
                            "{}；在外观 › 标记完成动画里多了「{}」",
                            describe(&c),
                            preset.name
                        )
                    };
                    presets.push(preset);
                    Ok(did)
                }
            }
            Ok(None) => Ok("没有设置任何动效".into()),
            Err(e) => Err(e),
        };
        report.push(Loaded { file, result });
    }
    (presets, report)
}

/// What settings picked: a built-in style or a plugin's preset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    Builtin(Style),
    Plugin(String),
}
impl Pick {
    /// The text the settings file holds.
    pub fn name(&self) -> String {
        match self {
            Self::Builtin(s) => s.name().to_owned(),
            Self::Plugin(id) => format!("plugin:{id}"),
        }
    }
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        match text.strip_prefix("plugin:") {
            Some(id) if crate::plugin::valid_id(id) => Some(Self::Plugin(id.to_owned())),
            Some(_) => None,
            None => Style::parse(text).map(Self::Builtin),
        }
    }
}

static EFFECTS: crate::plugin::Slot<(Vec<Preset>, Vec<Loaded>)> = crate::plugin::Slot::new();
static REDUCE_MOTION: AtomicBool = AtomicBool::new(false);
/// What settings picked; None plays the built-in seal.
static PICKED: Mutex<Option<Pick>> = Mutex::new(None);

fn picked_path() -> Option<PathBuf> {
    crate::agent::config_dir().map(|d| d.join("complete"))
}

/// Loads the installed plugins' effects once (after `plugin::install`) and what settings
/// picked, and reads the system's reduce-motion setting off the main thread.
pub fn install() {
    EFFECTS.get_or_init(|| load(crate::plugin::files(crate::plugin::Kind::Effects)));
    let saved = picked_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| Pick::parse(&t));
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
/// Loads them again after plugins changed (`plugin::reload`).
pub(crate) fn reload() {
    EFFECTS.set(load(crate::plugin::files(crate::plugin::Kind::Effects)));
}
/// Every installed plugin's preset, in plugin order.
pub fn presets() -> &'static [Preset] {
    EFFECTS.get().map_or(&[], |(p, _)| p.as_slice())
}
/// What settings picked, if anything.
pub fn picked() -> Option<Pick> {
    PICKED.lock().unwrap_or_else(|e| e.into_inner()).clone()
}
/// Picks the animation from now on and keeps it for the next start; None is the seal.
pub fn pick(choice: Option<Pick>) -> Result<(), String> {
    let path = picked_path().ok_or("找不到配置目录")?;
    match &choice {
        Some(c) => path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&path, c.name())),
        None => match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        },
    }
    .map_err(|e| format!("无法保存 {}：{e}", path.display()))?;
    *PICKED.lock().unwrap_or_else(|e| e.into_inner()) = choice;
    Ok(())
}
/// Whether macOS asks for reduced motion, which turns every style into the fade.
pub fn reduce_motion() -> bool {
    REDUCE_MOTION.load(Ordering::Relaxed)
}
/// The animation chosen, before reduced motion: a built-in style as the app tunes it, a
/// plugin's preset while that plugin is installed, else the built-in seal.
pub fn chosen(picked: Option<&Pick>, presets: &[Preset]) -> Complete {
    match picked {
        Some(Pick::Builtin(s)) => Complete::of(*s),
        Some(Pick::Plugin(id)) => presets
            .iter()
            .find(|p| &p.id == id)
            .map_or_else(Complete::default, |p| p.complete),
        None => Complete::default(),
    }
}
/// The completion animation to play now.
pub fn complete() -> Complete {
    chosen(picked().as_ref(), presets()).played(reduce_motion())
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

        // All in one directory: one plugin, so only its first preset is offered.
        let (presets, report) = load(vec![stamp.clone(), sweep.clone(), long, odd, empty]);
        assert_eq!(presets.len(), 1);
        let c = presets[0].complete;
        assert_eq!(c.style, Style::Stamp);
        assert_eq!(c.duration, Duration::from_millis(2_400));
        assert!(c.land, "stamp lands in the sidebar unless told not to");
        assert!(presets[0].same_as_builtin());
        assert!(
            report[0]
                .result
                .as_ref()
                .unwrap()
                .contains("内置的「落印」一样")
        );
        assert!(
            report[1]
                .result
                .as_ref()
                .unwrap_err()
                .contains("已经有一个")
        );
        assert!(report[2].result.as_ref().unwrap_err().contains("600–4000"));
        assert!(report[3].result.as_ref().unwrap_err().contains("格式不对"));
        assert!(report[4].result.is_ok());

        let (presets, report) = load(vec![sweep]);
        let c = presets[0].complete;
        assert_eq!(
            (c.style, c.duration, c.land),
            (Style::Sweep, Duration::from_millis(1_800), false)
        );
        assert!(!presets[0].same_as_builtin());
        assert!(report[0].result.as_ref().unwrap().contains("多了"));
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
    fn only_what_settings_picked_plays_and_a_removed_preset_falls_back() {
        let tuned = Preset {
            id: "slow-sweep".into(),
            name: "慢扫".into(),
            complete: Complete {
                style: Style::Sweep,
                duration: Duration::from_millis(3_000),
                land: false,
            },
        };
        let presets = [tuned.clone()];
        // Installing a plugin never changes what plays.
        assert_eq!(chosen(None, &presets), Complete::default());
        assert_eq!(
            chosen(Some(&Pick::Plugin("slow-sweep".into())), &presets),
            tuned.complete
        );
        assert_eq!(
            chosen(Some(&Pick::Builtin(Style::Sweep)), &presets),
            Complete::of(Style::Sweep)
        );
        assert_eq!(
            chosen(Some(&Pick::Plugin("gone".into())), &presets),
            Complete::default()
        );
        for s in Style::ALL {
            assert_eq!(
                Pick::parse(&format!("{}\n", s.name())),
                Some(Pick::Builtin(s))
            );
            let json: Style = serde_json::from_value(serde_json::json!(s.name())).unwrap();
            assert_eq!(json, s);
        }
        let p = Pick::Plugin("slow-sweep".into());
        assert_eq!(Pick::parse(&p.name()), Some(p));
        assert_eq!(Pick::parse("fancy"), None);
        assert_eq!(Pick::parse("plugin:../x"), None);
    }
}
