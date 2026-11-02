//! Status and tone colors, overridable from `~/.config/keepline/colors.json` (read once
//! at start):
//!
//! ```json
//! {
//!   "status": { "running": "#e8c547", "closed": "#4cc38a" },
//!   "tone": { "ok": "#4cc38a", "danger": "#e5534b" },
//!   "seen": { "title": "#b9b7b4", "status": "#7d7c7a", "dot": "ring" }
//! }
//! ```
//!
//! `status` keys are [`Status::key`]s (the older `working`, `done` and `ready` still read as
//! running and review); they color sessions only. `tone` keys are ok, warn and danger, for
//! everything else that is fine, a warning or a failure. `seen` styles session rows you
//! have already looked at, whose `dot` is "ring", "fill" or "fade"; running work always
//! keeps its filled status color. Anything missing or unreadable keeps the default.
use super::theme::{Status, Theme, mix};
use super::tone::Tone;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeenDot {
    /// An outline in the status hue: still says what it is doing, without shouting.
    Ring,
    Fill,
    /// The filled dot washed toward the panel.
    Fade,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusColors {
    /// `None` takes the theme's tone, so it follows a switch between light and dark.
    hues: [Option<u32>; Status::ALL.len()],
    tones: [u32; Tone::ALL.len()],
    seen_title: Option<u32>,
    seen_status: Option<u32>,
    pub seen_dot: SeenDot,
}
fn index(status: Status) -> usize {
    Status::ALL
        .iter()
        .position(|s| *s == status)
        .expect("ALL lists every status")
}
/// Keys a colors.json written before 2026-11-02 may still use.
fn legacy(key: &str) -> Option<Status> {
    match key {
        "working" => Some(Status::Running),
        "done" | "ready" => Some(Status::Review),
        _ => None,
    }
}
const YELLOW: u32 = 0xe8c547;
const GREEN: u32 = 0x4cc38a;
const SLATE: u32 = 0x8a9bb0;
const ORANGE: u32 = 0xe8823a;
const RED: u32 = 0xe5534b;
/// The default hue of each status (2026-11-02): yellow is only ever work in flight,
/// green only ever what you marked complete.
const fn default_hue(status: Status) -> Option<u32> {
    match status {
        Status::Waiting => Some(ORANGE),
        Status::Error => Some(RED),
        // 等后台 counts with 在跑; its ◔ tells them apart.
        Status::Running | Status::Background => Some(YELLOW),
        // Unseen fills the dot, seen draws it as a ring.
        Status::Review => Some(0x8f93ff),
        Status::Closed => Some(GREEN),
        // Quiet states share one slate and differ by glyph: ‖ ○ ◷.
        Status::Paused | Status::Fresh | Status::Deferred => Some(SLATE),
        // The muted text tone.
        Status::Stale => None,
    }
}
impl Default for StatusColors {
    fn default() -> Self {
        Self {
            hues: Status::ALL.map(default_hue),
            tones: [GREEN, ORANGE, RED],
            seen_title: None,
            seen_status: None,
            seen_dot: SeenDot::Ring,
        }
    }
}
impl StatusColors {
    pub fn get() -> &'static Self {
        static COLORS: OnceLock<StatusColors> = OnceLock::new();
        COLORS.get_or_init(|| {
            let Some(path) = path() else {
                return Self::default();
            };
            match std::fs::read_to_string(&path) {
                Ok(text) => Self::parse(&text).unwrap_or_else(|e| {
                    eprintln!("keepline: ignoring {}: {e}", path.display());
                    Self::default()
                }),
                Err(_) => Self::default(),
            }
        })
    }
    pub fn hue(&self, status: Status) -> u32 {
        self.hues[index(status)].unwrap_or_else(Theme::muted)
    }
    pub fn tone(&self, tone: Tone) -> u32 {
        self.tones[tone as usize]
    }
    pub fn seen_title(&self) -> u32 {
        self.seen_title
            .unwrap_or_else(|| mix(Theme::terminal_bg(), Theme::terminal_fg(), 760))
    }
    pub fn seen_status(&self) -> u32 {
        self.seen_status.unwrap_or_else(Theme::muted)
    }
    /// Unknown keys are reported so a typo does not silently keep the default.
    pub fn parse(text: &str) -> Result<Self, String> {
        let root: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let mut out = Self::default();
        if let Some(status) = root.get("status").and_then(|v| v.as_object()) {
            for (key, value) in status {
                let at = Status::ALL
                    .iter()
                    .copied()
                    .find(|s| s.key() == key)
                    .or_else(|| legacy(key))
                    .ok_or_else(|| format!("unknown status \"{key}\""))?;
                out.hues[index(at)] = Some(color(value)?);
            }
        }
        if let Some(tone) = root.get("tone").and_then(|v| v.as_object()) {
            for (key, value) in tone {
                let at = Tone::ALL
                    .iter()
                    .copied()
                    .find(|t| t.key() == key)
                    .ok_or_else(|| format!("unknown tone \"{key}\""))?;
                out.tones[at as usize] = color(value)?;
            }
        }
        if let Some(seen) = root.get("seen").and_then(|v| v.as_object()) {
            for (key, value) in seen {
                match key.as_str() {
                    "title" => out.seen_title = Some(color(value)?),
                    "status" => out.seen_status = Some(color(value)?),
                    "dot" => {
                        out.seen_dot = match value.as_str() {
                            Some("ring") => SeenDot::Ring,
                            Some("fill") => SeenDot::Fill,
                            Some("fade") => SeenDot::Fade,
                            _ => {
                                return Err(format!(
                                    "seen.dot must be ring, fill or fade: {value}"
                                ));
                            }
                        }
                    }
                    _ => return Err(format!("unknown seen key \"{key}\"")),
                }
            }
        }
        Ok(out)
    }
}
fn path() -> Option<std::path::PathBuf> {
    config_dir().map(|dir| dir.join("colors.json"))
}
pub fn config_dir() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".config")))?;
    Some(base.join("keepline"))
}
fn color(value: &serde_json::Value) -> Result<u32, String> {
    value
        .as_str()
        .and_then(|s| s.strip_prefix('#'))
        .filter(|hex| hex.len() == 6)
        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        .ok_or_else(|| format!("expected \"#rrggbb\", got {value}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overrides_only_what_is_given() {
        let c = StatusColors::parse(
            r##"{"status":{"running":"#112233"},"tone":{"ok":"#223344"},"seen":{"title":"#445566","dot":"fade"}}"##,
        )
        .unwrap();
        let d = StatusColors::default();
        assert_eq!(c.hue(Status::Running), 0x112233);
        assert_eq!(c.hue(Status::Background), d.hue(Status::Background));
        assert_eq!(c.tone(Tone::Ok), 0x223344);
        assert_eq!(c.tone(Tone::Danger), d.tone(Tone::Danger));
        assert_eq!(c.seen_title(), 0x445566);
        assert_eq!(c.seen_status(), d.seen_status());
        assert_eq!(c.seen_dot, SeenDot::Fade);
    }

    #[test]
    fn older_keys_still_read() {
        let c =
            StatusColors::parse(r##"{"status":{"working":"#112233","done":"#445566"}}"##).unwrap();
        assert_eq!(c.hue(Status::Running), 0x112233);
        assert_eq!(c.hue(Status::Review), 0x445566);
    }

    #[test]
    fn rejects_typos_and_bad_colors() {
        assert!(StatusColors::parse(r##"{"status":{"wroking":"#112233"}}"##).is_err());
        assert!(StatusColors::parse(r##"{"status":{"running":"yellow"}}"##).is_err());
        assert!(StatusColors::parse(r##"{"tone":{"good":"#112233"}}"##).is_err());
        assert!(StatusColors::parse(r##"{"seen":{"dot":"square"}}"##).is_err());
        assert_eq!(StatusColors::parse("{}").unwrap(), StatusColors::default());
    }

    /// Two states that share a hue must differ by glyph, and the loud hues belong to one
    /// state family each: yellow is in flight, green is closed.
    #[test]
    fn shared_hues_are_told_apart_by_glyph() {
        let d = StatusColors::default();
        for (i, a) in Status::ALL.iter().enumerate() {
            for b in &Status::ALL[i + 1..] {
                if d.hue(*a) == d.hue(*b) {
                    assert_ne!(a.glyph(), b.glyph(), "{a:?} {b:?}");
                }
            }
        }
        for s in Status::ALL {
            assert_eq!(d.hue(s) == YELLOW, s.working(), "{s:?}");
            assert_eq!(d.hue(s) == GREEN, s == Status::Closed, "{s:?}");
        }
    }
}
