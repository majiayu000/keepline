//! Light or dark: chosen from the Keepline menu and kept in
//! `~/.config/keepline/appearance` as "system", "light" or "dark".
use super::style::Style;
use super::theme::{Scheme, Theme};
use gpui::{App, Global, WindowAppearance};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Appearance {
    /// Follows macOS, switching when the system does.
    #[default]
    System,
    Light,
    Dark,
}
impl Global for Appearance {}
impl Appearance {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "跟随系统",
            Self::Light => "亮色",
            Self::Dark => "暗色",
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
    fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.name() == text.trim())
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
    /// The terminal-native style is dark unless light was picked outright
    /// (specs/terminal-workbench/design/main-directions.html).
    fn scheme(self, system: WindowAppearance, style: Style) -> Scheme {
        match (self, system) {
            (Self::Light, _) => Scheme::Light,
            (Self::Dark, _) => Scheme::Dark,
            (Self::System, _) if style == Style::Terminal => Scheme::Dark,
            (Self::System, WindowAppearance::Light | WindowAppearance::VibrantLight) => {
                Scheme::Light
            }
            (Self::System, WindowAppearance::Dark | WindowAppearance::VibrantDark) => Scheme::Dark,
        }
    }
}
/// Re-read the choice against the system appearance and repaint every window.
pub fn apply(cx: &mut App) {
    let chosen = cx.try_global::<Appearance>().copied().unwrap_or_default();
    let scheme = chosen.scheme(cx.window_appearance(), super::style::current(cx));
    if scheme != Theme::scheme() {
        Theme::set_scheme(scheme);
        cx.refresh_windows();
    }
}
pub fn choose(appearance: Appearance, cx: &mut App) -> Result<(), String> {
    appearance.save()?;
    cx.set_global(appearance);
    apply(cx);
    cx.refresh_windows();
    Ok(())
}
fn path() -> Option<std::path::PathBuf> {
    super::status_colors::config_dir().map(|dir| dir.join("appearance"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choice_round_trips_and_system_follows_macos() {
        for a in Appearance::ALL {
            assert_eq!(Appearance::parse(a.name()), Some(a));
        }
        assert_eq!(Appearance::parse("sepia"), None);
        let system = Appearance::System;
        let standard = Style::Standard;
        assert_eq!(
            system.scheme(WindowAppearance::VibrantLight, standard),
            Scheme::Light
        );
        assert_eq!(
            system.scheme(WindowAppearance::Dark, standard),
            Scheme::Dark
        );
        assert_eq!(
            Appearance::Dark.scheme(WindowAppearance::Light, standard),
            Scheme::Dark
        );
    }

    #[test]
    fn terminal_style_is_dark_unless_light_was_picked() {
        let terminal = Style::Terminal;
        assert_eq!(
            Appearance::System.scheme(WindowAppearance::Light, terminal),
            Scheme::Dark
        );
        assert_eq!(
            Appearance::Light.scheme(WindowAppearance::Dark, terminal),
            Scheme::Light
        );
    }
}
