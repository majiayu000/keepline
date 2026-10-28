//! Status colors, overridable from `~/.config/keepline/colors.json` (read once at start):
//!
//! ```json
//! {
//!   "status": { "working": "#e8c547", "done": "#4cc38a" },
//!   "seen": { "title": "#b9b7b4", "status": "#7d7c7a", "dot": "ring" }
//! }
//! ```
//!
//! `status` keys are the group names (waiting, error, review, paused, running, stale,
//! working, done, ready); `seen` styles session rows you have already looked at, whose
//! `dot` is "ring", "fill" or "fade". Anything missing or unreadable keeps the default.
use super::theme::{Group, Theme, mix};
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
    hues: [u32; 9],
    pub seen_title: u32,
    pub seen_status: u32,
    pub seen_dot: SeenDot,
}
const NAMES: [&str; 9] = [
    "waiting", "error", "review", "paused", "running", "stale", "working", "done", "ready",
];
fn index(group: Group) -> usize {
    match group {
        Group::Waiting => 0,
        Group::Error => 1,
        Group::Review => 2,
        Group::Paused => 3,
        Group::Running => 4,
        Group::Stale => 5,
        Group::Working => 6,
        Group::Done => 7,
        Group::Ready => 8,
    }
}
impl Default for StatusColors {
    fn default() -> Self {
        Self {
            hues: [
                0xe8823a, // waiting
                0xe5534b, // error
                0x8f93ff, // review
                0xd9a441, // paused
                0x4cc38a, // running
                Theme::MUTED,
                0xe8c547, // working
                0x4cc38a, // done
                0x8a9bb0, // ready
            ],
            seen_title: mix(Theme::TERMINAL_BG, Theme::TERMINAL_FG, 760),
            seen_status: Theme::MUTED,
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
    pub fn hue(&self, group: Group) -> u32 {
        self.hues[index(group)]
    }
    /// Unknown keys are reported so a typo does not silently keep the default.
    pub fn parse(text: &str) -> Result<Self, String> {
        let root: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let mut out = Self::default();
        if let Some(status) = root.get("status").and_then(|v| v.as_object()) {
            for (key, value) in status {
                let at = NAMES
                    .iter()
                    .position(|n| n == key)
                    .ok_or_else(|| format!("unknown status \"{key}\""))?;
                out.hues[at] = color(value)?;
            }
        }
        if let Some(seen) = root.get("seen").and_then(|v| v.as_object()) {
            for (key, value) in seen {
                match key.as_str() {
                    "title" => out.seen_title = color(value)?,
                    "status" => out.seen_status = color(value)?,
                    "dot" => {
                        out.seen_dot = match value.as_str() {
                            Some("ring") => SeenDot::Ring,
                            Some("fill") => SeenDot::Fill,
                            Some("fade") => SeenDot::Fade,
                            _ => {
                                return Err(format!("seen.dot must be ring, fill or fade: {value}"));
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
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".config")))?;
    Some(base.join("keepline/colors.json"))
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
            r##"{"status":{"working":"#112233"},"seen":{"title":"#445566","dot":"fade"}}"##,
        )
        .unwrap();
        let d = StatusColors::default();
        assert_eq!(c.hue(Group::Working), 0x112233);
        assert_eq!(c.hue(Group::Running), d.hue(Group::Running));
        assert_eq!(c.seen_title, 0x445566);
        assert_eq!(c.seen_status, d.seen_status);
        assert_eq!(c.seen_dot, SeenDot::Fade);
    }

    #[test]
    fn rejects_typos_and_bad_colors() {
        assert!(StatusColors::parse(r##"{"status":{"wroking":"#112233"}}"##).is_err());
        assert!(StatusColors::parse(r##"{"status":{"working":"yellow"}}"##).is_err());
        assert!(StatusColors::parse(r##"{"seen":{"dot":"square"}}"##).is_err());
        assert_eq!(StatusColors::parse("{}").unwrap(), StatusColors::default());
    }
}
