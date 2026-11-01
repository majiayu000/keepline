//! Whether a sidebar click on a session out of sight floats its screen up for a look
//! (peek) or switches to it. Kept in `~/.config/keepline/peek` as "on"; off until chosen.
use gpui::{App, Global};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PeekChoice(pub bool);
impl Global for PeekChoice {}
impl PeekChoice {
    pub fn load() -> Self {
        Self(
            path()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .is_some_and(|text| text.trim() == "on"),
        )
    }
}
pub fn enabled(cx: &App) -> bool {
    cx.try_global::<PeekChoice>().is_some_and(|p| p.0)
}
pub fn choose(on: bool, cx: &mut App) -> Result<(), String> {
    let path = path().ok_or("找不到配置目录")?;
    path.parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&path, if on { "on" } else { "off" }))
        .map_err(|e| format!("无法保存 {}：{e}", path.display()))?;
    cx.set_global(PeekChoice(on));
    Ok(())
}
fn path() -> Option<std::path::PathBuf> {
    super::status_colors::config_dir().map(|dir| dir.join("peek"))
}
