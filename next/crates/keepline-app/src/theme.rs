//! Design tokens from specs/terminal-workbench/design/README.md.
//! UI dimensions live here; terminal cell colors remain owned by the VT.
use crate::screen::hsla;
use gpui::{App, Font, Hsla, font};

pub struct Theme;
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Waiting,
    Error,
    Review,
    Paused,
    Running,
    Stale,
}
impl Group {
    pub const ALL: [Self; 6] = [
        Self::Waiting,
        Self::Error,
        Self::Review,
        Self::Paused,
        Self::Running,
        Self::Stale,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Waiting => "等你回答",
            Self::Error => "出错了",
            Self::Review => "待核对",
            Self::Paused => "做一半停了",
            Self::Running => "进行中",
            Self::Stale => "很久没动",
        }
    }
    pub fn colors(self) -> (Hsla, Hsla, Hsla) {
        let (dot, ink, bg) = match self {
            Self::Waiting => (0xe8823a, 0xffd2b0, 0x4a2a12),
            Self::Error => (0xe5534b, 0xffc4bd, 0x4a1c18),
            Self::Review => (0x8f93ff, 0xd6d8ff, 0x2a2c5c),
            Self::Paused => (0xd9a441, 0xf3dca8, 0x3a2e12),
            Self::Running => (0x4cc38a, 0xb9ecd2, 0x173a2a),
            Self::Stale => (Theme::MUTED, Theme::SECONDARY, Theme::RAISED),
        };
        (hsla(dot), hsla(ink), hsla(bg))
    }
}
#[allow(dead_code)] // The complete design palette also covers the deferred components.
impl Theme {
    pub const APP: u32 = 0x0b0c10;
    pub const PANEL: u32 = 0x0f1116;
    pub const PANE: u32 = 0x101217;
    pub const HEADER: u32 = 0x12141a;
    pub const HEADER_FOCUSED: u32 = 0x181a24;
    pub const RAISED: u32 = 0x1b1e26;
    pub const TODO: u32 = 0x121527;
    pub const SLOT: u32 = 0x0e1015;
    pub const BORDER: u32 = 0x1d2028;
    pub const CONTROL: u32 = 0x2a2e38;
    pub const TODO_BORDER: u32 = 0x262a3a;
    pub const SLOT_BORDER: u32 = 0x2f3448;
    pub const CARD_BORDER: u32 = 0x22252f;
    pub const PRIMARY: u32 = 0xe9ebef;
    pub const BODY: u32 = 0xd7dae0;
    pub const SECONDARY: u32 = 0x8b919e;
    pub const MUTED: u32 = 0x6b7080;
    pub const ACCENT: u32 = 0x4b4fd6;
    pub const LINK: u32 = 0xc3c6ff;
    pub const CHIP: u32 = 0x1e2140;
    pub const WHITE: u32 = 0xffffff;
    pub const CLEANUP_BACKGROUND: u32 = 0xeef0f3;
    pub const POPOVER_TEXT: u32 = 0x17181c;
    pub const POPOVER_SECONDARY: u32 = 0x5d6270;
    pub const POPOVER_BORDER: u32 = 0xd5d8de;
    pub const INFERENCE: u32 = 0xf6f7fb;
    pub const UI_FONT: &'static str = ".AppleSystemUIFont";
    pub const BODY_SIZE: f32 = 13.;
    pub const SMALL: f32 = 12.;
    pub const META: f32 = 11.5;
    pub const CHIP_SIZE: f32 = 11.;
    pub const MONO_SIZE: f32 = 12.5;
    pub const CARD_TITLE: f32 = 13.5;
    pub const TITLE: f32 = 20.;
    pub const LINE_HEIGHT: f32 = 1.4;
    pub const TERMINAL_LINE_HEIGHT: f32 = 1.2;
    pub const S2: f32 = 2.;
    pub const S4: f32 = 4.;
    pub const S6: f32 = 6.;
    pub const S8: f32 = 8.;
    pub const S10: f32 = 10.;
    pub const S12: f32 = 12.;
    pub const S14: f32 = 14.;
    pub const S16: f32 = 16.;
    pub const S20: f32 = 20.;
    pub const SIDEBAR: f32 = 240.;
    pub const TOP: f32 = 46.;
    pub const TABS: f32 = 34.;
    pub const TODO_HEIGHT: f32 = 36.;
    pub const PANE_HEADER: f32 = 32.;
    pub const FOOTER: f32 = 26.;
    pub const ROW: f32 = 30.;
    pub const BORDER_WIDTH: f32 = 1.;
    pub const PANE_RADIUS: f32 = 8.;
    pub const CONTROL_RADIUS: f32 = 6.;
    pub const CHIP_RADIUS: f32 = 4.;
    pub const CARD_RADIUS: f32 = 10.;
    pub const POPOVER_RADIUS: f32 = 12.;
    pub const DOT: f32 = 7.;
    pub const LOGO: f32 = 18.;
    pub const ICON_WIDTH: f32 = 24.;
    pub const ICON_HEIGHT: f32 = 22.;
    pub const CARD_MIN: f32 = 230.;
    pub const OVERVIEW_X: f32 = 28.;
    pub const SEARCH_HEIGHT: f32 = 260.;
    pub const MENU_WIDTH: f32 = 230.;
    pub const HERE_WIDTH: f32 = 340.;
    pub const HERE_HEIGHT: f32 = 600.;
    pub const CLEANUP_WIDTH: f32 = 960.;
    pub const CLEANUP_ACTION_HEIGHT: f32 = 64.;
    pub const FORM_WIDTH: f32 = 640.;
    pub const FIELD_HEIGHT: f32 = 40.;
    pub const WINDOW_WIDTH: f32 = 1440.;
    pub const WINDOW_HEIGHT: f32 = 900.;
    pub fn color(value: u32) -> Hsla {
        hsla(value)
    }
    pub fn mono(cx: &App) -> Font {
        static MONO: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        let mut face = font(
            MONO.get_or_init(|| {
                let names = cx.text_system().all_font_names();
                ["JetBrains Mono", "SF Mono", "Menlo"]
                    .into_iter()
                    .find(|candidate| names.iter().any(|n| n == candidate))
                    .unwrap_or("Menlo")
                    .to_owned()
            })
            .clone(),
        );
        face.fallbacks = Some(gpui::FontFallbacks::from_fonts(vec![
            "Symbols Nerd Font Mono".into(),
        ]));
        face
    }
}
