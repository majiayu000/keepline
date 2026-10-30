//! Terminal typography shared by settings, zoom shortcuts and the cell renderer.
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{OnceLock, RwLock},
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerminalText {
    /// Empty selects the bundled default face.
    pub family: String,
    pub size: f32,
    /// Multiplies the face's natural cell height, keeping the glyph centered.
    pub line_height: f32,
}
impl Default for TerminalText {
    fn default() -> Self {
        Self {
            family: String::new(),
            size: super::theme::Theme::TERMINAL_FONT_SIZE,
            line_height: 1.,
        }
    }
}
impl TerminalText {
    fn parse(text: &str) -> Result<Self, String> {
        let value: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if !(8. ..=36.).contains(&value.size) || !(1. ..=2.).contains(&value.line_height) {
            return Err("字号须为 8–36，行距须为 1–2 倍".into());
        }
        Ok(value)
    }
    fn load() -> Self {
        let Some(path) = path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text).unwrap_or_else(|e| {
                eprintln!("keepline: reading {}: {e}", path.display());
                Self::default()
            }),
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    eprintln!("keepline: reading {}: {e}", path.display());
                }
                Self::default()
            }
        }
    }
}
fn state() -> &'static RwLock<TerminalText> {
    static TEXT: OnceLock<RwLock<TerminalText>> = OnceLock::new();
    TEXT.get_or_init(|| RwLock::new(TerminalText::load()))
}
pub fn current() -> TerminalText {
    state().read().unwrap().clone()
}
pub fn size() -> f32 {
    state().read().unwrap().size
}
pub fn line_height() -> f32 {
    state().read().unwrap().line_height
}
pub fn path() -> Option<PathBuf> {
    super::status_colors::config_dir().map(|dir| dir.join("terminal-text.json"))
}
/// Save before applying, so a failed write never looks like a saved preference.
pub fn update(value: TerminalText) -> Result<(), String> {
    let path = path().ok_or("找不到配置目录")?;
    let text = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
    let value = TerminalText::parse(&text)?;
    let write = || -> std::io::Result<()> {
        std::fs::create_dir_all(path.parent().unwrap())?;
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, text)?;
        std::fs::rename(temporary, &path)
    };
    write().map_err(|e| format!("无法保存 {}：{e}", path.display()))?;
    *state().write().unwrap() = value;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typography_round_trip_and_bounds() {
        let value = TerminalText {
            family: "Menlo".into(),
            size: 18.,
            line_height: 1.4,
        };
        assert_eq!(
            TerminalText::parse(&serde_json::to_string(&value).unwrap()).unwrap(),
            value
        );
        for size in [0., 37.] {
            let text = format!(r#"{{"family":"","size":{size},"line_height":1}}"#);
            assert!(TerminalText::parse(&text).is_err());
        }
        assert!(TerminalText::parse(r#"{"family":"","size":13,"line_height":0.8}"#).is_err());
    }
}
