//! The workbench's look, chosen in settings and kept in `~/.config/keepline/style` as
//! "standard" or "terminal". Both show the same things; they differ in how much shell
//! stays around the terminals (specs/terminal-workbench/design/main-a-plus.html).
use gpui::{App, Global};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Style {
    /// C: the familiar layout with the repeats taken out.
    #[default]
    Standard,
    /// A: the shell recedes to hairlines, monospace throughout, tabs in a bottom status
    /// line and states as symbols, like a terminal multiplexer.
    Terminal,
}
impl Global for Style {}
impl Style {
    pub const ALL: [Self; 2] = [Self::Standard, Self::Terminal];
    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "标准",
            Self::Terminal => "终端原生",
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Terminal => "terminal",
        }
    }
    fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == text.trim())
    }
    pub fn load() -> Self {
        path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|text| Self::parse(&text))
            .unwrap_or_default()
    }
    fn save(self) -> Result<(), String> {
        let path = path().ok_or("找不到配置目录")?;
        path.parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&path, self.name()))
            .map_err(|e| format!("无法保存 {}：{e}", path.display()))
    }
}
pub fn current(cx: &App) -> Style {
    cx.try_global::<Style>().copied().unwrap_or_default()
}
pub fn choose(style: Style, cx: &mut App) -> Result<(), String> {
    style.save()?;
    cx.set_global(style);
    cx.refresh_windows();
    Ok(())
}
fn path() -> Option<std::path::PathBuf> {
    super::status_colors::config_dir().map(|dir| dir.join("style"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_or_missing_names_fall_back_to_standard() {
        assert_eq!(Style::parse("terminal\n"), Some(Style::Terminal));
        assert_eq!(Style::parse("fancy"), None);
        assert_eq!(Style::default(), Style::Standard);
    }
}
