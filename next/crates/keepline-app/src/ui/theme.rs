//! Design tokens from specs/terminal-workbench/design/README.md.
//! UI dimensions live here; terminal cell colors remain owned by the VT.
pub use crate::model::session_state::Group;
use gpui::{App, Font, Hsla, Rgba, font};

pub fn hsla(c: u32) -> Hsla {
    Rgba {
        r: ((c >> 16) & 255) as f32 / 255.,
        g: ((c >> 8) & 255) as f32 / 255.,
        b: (c & 255) as f32 / 255.,
        a: 1.,
    }
    .into()
}
pub struct Theme;
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
            // One word per state everywhere (2026-10-27): a turn that ended, whether it
            // claims success or just stopped talking, is 做完了; an interrupted one 停下了.
            Self::Review => "做完了",
            Self::Paused => "停下了",
            Self::Running => "在跑",
            Self::Stale => "很久没动",
            Self::Working => "在跑",
            Self::Done => "做完了",
            Self::Ready => "做完了",
        }
    }
    /// The group's base hue; its ink and wash derive from it and the terminal.
    pub const fn hue(self) -> u32 {
        match self {
            Self::Waiting => 0xe8823a,
            Self::Error => 0xe5534b,
            Self::Review => 0x8f93ff,
            Self::Paused => 0xd9a441,
            Self::Running | Self::Done => 0x4cc38a,
            Self::Stale => Theme::MUTED,
            Self::Working => 0xe8c547,
            Self::Ready => 0x8a9bb0,
        }
    }
    pub fn colors(self) -> (Hsla, Hsla, Hsla) {
        let (dot, ink, bg) = match self {
            Self::Stale => (Theme::MUTED, Theme::SECONDARY, Theme::RAISED),
            group => (
                group.hue(),
                mix(group.hue(), Theme::WHITE, 620),
                mix(Theme::TERMINAL_BG, group.hue(), 220),
            ),
        };
        (hsla(dot), hsla(ink), hsla(bg))
    }
}
/// `share` per mille of `to` blended into `from`, per channel; const so tokens stay constants.
pub const fn mix(from: u32, to: u32, share: u32) -> u32 {
    let mut out = 0;
    let mut shift = 0;
    while shift < 24 {
        let a = (from >> shift) & 255;
        let b = (to >> shift) & 255;
        out |= ((a * (1000 - share) + b * share + 500) / 1000) << shift;
        shift += 8;
    }
    out
}
#[allow(dead_code)] // The complete design palette also covers the deferred components.
impl Theme {
    /// The terminal's default colors; every chrome surface and text tone is a mix of
    /// these two (and the accent), so the window reads as one surface with the terminal.
    pub const TERMINAL_BG: u32 = 0x1c1c1e;
    pub const TERMINAL_FG: u32 = 0xe4e2df;
    pub const WHITE: u32 = 0xffffff;
    const BLACK: u32 = 0x000000;
    const BG: u32 = Self::TERMINAL_BG;
    const FG: u32 = Self::TERMINAL_FG;
    /// One neutral surface: sidebar, top bar, panes and headers share the terminal's
    /// gray and are told apart by hairlines, not by bands of different shades.
    pub const APP: u32 = mix(Self::BG, Self::BLACK, 150);
    pub const PANEL: u32 = Self::BG;
    pub const PANE: u32 = Self::BG;
    pub const HEADER: u32 = Self::BG;
    pub const HEADER_FOCUSED: u32 = mix(Self::BG, Self::FG, 35);
    pub const SLOT: u32 = mix(Self::BG, Self::BLACK, 80);
    pub const TODO: u32 = mix(Self::BG, Self::FG, 30);
    pub const RAISED: u32 = mix(Self::BG, Self::FG, 70);
    pub const BORDER: u32 = mix(Self::BG, Self::FG, 90);
    pub const CARD_BORDER: u32 = mix(Self::BG, Self::FG, 100);
    pub const CONTROL: u32 = mix(Self::BG, Self::FG, 150);
    pub const TODO_BORDER: u32 = Self::BORDER;
    pub const SLOT_BORDER: u32 = Self::CONTROL;
    /// A quiet fill for ordinary buttons; they carry no outline.
    pub const BUTTON: u32 = mix(Self::BG, Self::FG, 60);
    pub const BUTTON_HOVER: u32 = mix(Self::BG, Self::FG, 120);
    pub const PRIMARY: u32 = mix(Self::FG, Self::WHITE, 300);
    pub const BODY: u32 = Self::FG;
    pub const SECONDARY: u32 = mix(Self::BG, Self::FG, 580);
    pub const MUTED: u32 = mix(Self::BG, Self::FG, 420);
    /// The one solid control per view is near-white with dark text; color is left to status.
    pub const ACCENT: u32 = Self::PRIMARY;
    pub const ON_ACCENT: u32 = Self::BG;
    pub const FOCUS: u32 = mix(Self::BG, Self::FG, 380);
    pub const LINK: u32 = mix(Self::BG, Self::FG, 860);
    /// Selected text in a field.
    pub const SELECTION: u32 = mix(Self::BG, Self::FG, 260);
    pub const CHIP: u32 = mix(Self::BG, Self::FG, 80);
    /// In-window popovers (「你在这里」, cleanup) sit one step above the pane.
    pub const POPOVER: u32 = mix(Self::BG, Self::FG, 55);
    pub const PALETTE_ROW: u32 = mix(Self::POPOVER, Self::FG, 75);
    /// Matched text in search results: a quiet amber wash, the text keeps its own color.
    pub const MATCH: u32 = mix(Self::POPOVER, 0xe8a33a, 280);
    /// Dims the workbench behind the palette, as rgba.
    pub const SCRIM: u32 = 0x0000_0099;
    pub const INFERENCE: u32 = mix(Self::BG, Self::FG, 35);
    /// The menu bar panel is a separate light macOS popover, kept apart from the window.
    pub const MENU_ACCENT: u32 = 0x4b4fd6;
    pub const MENU_TEXT: u32 = 0x17181c;
    pub const MENU_SECONDARY: u32 = 0x5d6270;
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
    /// macOS draws the traffic lights inside the top bar; its content starts after them.
    pub const TRAFFIC_LIGHT_X: f32 = 16.;
    pub const TRAFFIC_LIGHT_Y: f32 = 17.;
    pub const TRAFFIC_LIGHT_INSET: f32 = 84.;
    pub const TABS: f32 = 34.;
    pub const TAB_RENAME_MIN: f32 = 64.;
    pub const TAB_MAX_WIDTH: f32 = 200.;
    pub const TAB_TIP_WIDTH: f32 = 420.;
    pub const TODO_HEIGHT: f32 = 36.;
    pub const PANE_HEADER: f32 = 32.;
    pub const FOOTER: f32 = 26.;
    pub const ROW: f32 = 30.;
    pub const BORDER_WIDTH: f32 = 1.;
    pub const CARET_WIDTH: f32 = 1.5;
    pub const PANE_RADIUS: f32 = 8.;
    pub const CONTROL_RADIUS: f32 = 6.;
    pub const CHIP_RADIUS: f32 = 4.;
    pub const CARD_RADIUS: f32 = 10.;
    pub const POPOVER_RADIUS: f32 = 12.;
    pub const DOT: f32 = 7.;
    pub const LOGO: f32 = 18.;
    pub const ICON_WIDTH: f32 = 24.;
    pub const ICON_HEIGHT: f32 = 22.;
    pub const CARD_MIN: f32 = 300.;
    pub const OVERVIEW_X: f32 = 28.;
    pub const SEARCH_HEIGHT: f32 = 260.;
    pub const MENU_WIDTH: f32 = 230.;
    pub const HERE_WIDTH: f32 = 340.;
    pub const HERE_HEIGHT: f32 = 600.;
    pub const CLEANUP_WIDTH: f32 = 960.;
    pub const CLEANUP_ACTION_HEIGHT: f32 = 64.;
    pub const MENUBAR_WIDTH: f32 = 360.;
    pub const MENUBAR_HEIGHT: f32 = 480.;
    pub const MENUBAR_ROW: f32 = 52.;
    pub const MENU_TITLE: f32 = 15.;
    pub const MENU_BUTTON: f32 = 36.;
    pub const MENU_WAIT_DOT: u32 = 0xc2570c;
    pub const MENU_WAIT_TEXT: u32 = 0x9a3f06;
    pub const MENU_WAIT_BG: u32 = 0xfff6ee;
    pub const MENU_ERROR_DOT: u32 = 0xc0392b;
    pub const MENU_ERROR_TEXT: u32 = 0x9b2318;
    pub const FORM_WIDTH: f32 = 520.;
    pub const DETAIL_WIDTH: f32 = 640.;
    pub const FORM_TITLE: f32 = 16.;
    pub const TASK_ROW: f32 = 48.;
    pub const SEARCH_WIDTH: f32 = 360.;
    /// The ⌘P palette: a floating panel over the workbench, list left, preview right.
    pub const PALETTE_WIDTH: f32 = 680.;
    pub const PALETTE_TOP: f32 = 72.;
    pub const PALETTE_LIST: f32 = 440.;
    pub const PALETTE_PREVIEW: f32 = 250.;
    pub const PALETTE_RADIUS: f32 = 14.;
    pub const SEARCH_FIELD: f32 = 48.;
    pub const SEARCH_TEXT: f32 = 15.;
    pub const SEARCH_MIN: f32 = 180.;
    pub const HERE_LABEL: f32 = 96.;
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
        // Terminal cells are independently addressable; match Ghostty's reference
        // configuration rather than letting contextual ligatures cross cell edits.
        face.features = gpui::FontFeatures(std::sync::Arc::new(vec![
            ("liga".into(), 0),
            ("calt".into(), 0),
        ]));
        face.fallbacks = Some(gpui::FontFallbacks::from_fonts(vec![
            "Symbols Nerd Font Mono".into(),
        ]));
        face
    }
}

#[cfg(test)]
mod tests {
    use super::{Theme, mix};

    #[test]
    fn mix_blends_each_channel() {
        assert_eq!(mix(0x000000, 0xffffff, 0), 0x000000);
        assert_eq!(mix(0x000000, 0xffffff, 1000), 0xffffff);
        assert_eq!(mix(0x102030, 0x304050, 500), 0x203040);
        assert_eq!(Theme::PANE, Theme::TERMINAL_BG);
    }

    /// Colors and sizes come from `Theme`; a literal in a component drifts from the tokens.
    /// The terminal renderer and the visual-test harness measure cells, not chrome.
    #[test]
    fn components_take_colors_and_sizes_from_tokens() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files: Vec<_> = [
            "terminal/pane.rs",
            "ui/field.rs",
            "ui/widgets.rs",
            "desktop.rs",
        ]
        .iter()
        .map(|name| src.join(name))
        .collect();
        for entry in std::fs::read_dir(src.join("workbench")).unwrap() {
            files.push(entry.unwrap().path());
        }
        let mut offenders = Vec::new();
        for path in files {
            let text = std::fs::read_to_string(&path).unwrap();
            for (n, line) in text.lines().enumerate() {
                if color_literal(line) || size_literal(line) {
                    offenders.push(format!("{}:{}: {}", path.display(), n + 1, line.trim()));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "use Theme tokens:\n{}",
            offenders.join("\n")
        );
    }

    fn color_literal(line: &str) -> bool {
        line.match_indices("0x").any(|(at, _)| {
            let hex = line[at + 2..]
                .chars()
                .take_while(|c| c.is_ascii_hexdigit())
                .count();
            hex == 6
        })
    }

    fn size_literal(line: &str) -> bool {
        line.match_indices("px(").any(|(at, _)| {
            let rest = &line[at + 3..];
            rest.starts_with(|c: char| c.is_ascii_digit()) && !rest.starts_with("0.)")
        })
    }
}
