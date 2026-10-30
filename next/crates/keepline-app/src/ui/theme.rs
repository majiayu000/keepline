//! Design tokens from specs/terminal-workbench/design/README.md.
//! UI dimensions live here; terminal cell colors remain owned by the VT.
pub use crate::model::session_state::Group;
use gpui::{App, Font, Hsla, Rgba, font};
pub use keepline_core::terminal::Scheme;
use std::sync::atomic::{AtomicBool, Ordering};

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

/// JetBrains Mono 2.304 (OFL, assets/JETBRAINS-MONO-LICENSE): the face Ghostty embeds
/// as its default, registered in-process so `Theme::mono` never depends on an install.
pub const BUNDLED_MONO: [&[u8]; 4] = [
    include_bytes!("../../assets/JetBrainsMono-Regular.ttf"),
    include_bytes!("../../assets/JetBrainsMono-Bold.ttf"),
    include_bytes!("../../assets/JetBrainsMono-Italic.ttf"),
    include_bytes!("../../assets/JetBrainsMono-BoldItalic.ttf"),
];
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
    /// The group's base hue, from `colors.json` or the default; its ink and wash derive
    /// from it and the terminal.
    pub fn hue(self) -> u32 {
        super::status_colors::StatusColors::get().hue(self)
    }
    pub fn colors(self) -> (Hsla, Hsla, Hsla) {
        let (dot, ink, bg) = self.colors_in(Theme::palette());
        (hsla(dot), hsla(ink), hsla(bg))
    }
    /// Dot, ink and wash on one palette.
    fn colors_in(self, p: &Palette) -> (u32, u32, u32) {
        let (dot, ink, bg) = match self {
            Self::Stale => (legible(self.hue(), p), p.secondary, p.raised),
            // Ink moves the hue toward the text color, so it reads on either background.
            group => (
                legible(group.hue(), p),
                mix(group.hue(), p.vivid, p.ink_share),
                mix(p.terminal_bg, group.hue(), p.wash_share),
            ),
        };
        (dot, ink, bg)
    }
}
/// The hue pushed toward the text color until it reaches 3:1 on the terminal background,
/// so a yellow or green dot still reads on white; a hue that already passes is untouched.
fn legible(hue: u32, p: &Palette) -> u32 {
    (0..=1000)
        .step_by(50)
        .map(|share| mix(hue, p.vivid, share))
        .find(|&c| contrast(c, p.terminal_bg) >= 3.)
        .unwrap_or(p.vivid)
}
/// WCAG contrast ratio between two colors.
pub fn contrast(a: u32, b: u32) -> f32 {
    let luminance = |c: u32| {
        let channel = |shift: u32| {
            let v = ((c >> shift) & 255) as f32 / 255.;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
    };
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
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
/// Chrome colors for one terminal scheme. Every surface and text tone is a mix of the
/// terminal's default colors (plus black or white), so the window reads as one surface
/// with the terminal in either scheme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub terminal_bg: u32,
    pub terminal_fg: u32,
    /// White on dark, black on light: past the text, for ink and the one solid control.
    vivid: u32,
    ink_share: u32,
    wash_share: u32,
    /// One neutral surface: sidebar, top bar, panes and headers share the terminal's
    /// background and are told apart by hairlines, not by bands of different shades.
    pub app: u32,
    pub panel: u32,
    pub pane: u32,
    pub header: u32,
    pub header_focused: u32,
    pub slot: u32,
    pub todo: u32,
    pub raised: u32,
    pub border: u32,
    pub card_border: u32,
    pub control: u32,
    /// Text field outlines: 3:1 against the pane so an empty field is still findable.
    pub field_edge: u32,
    pub todo_border: u32,
    pub slot_border: u32,
    /// A quiet fill for ordinary buttons; they carry no outline.
    pub button: u32,
    pub button_hover: u32,
    pub primary: u32,
    pub body: u32,
    pub secondary: u32,
    pub muted: u32,
    /// The one solid control per view: near-white with dark text on dark, the reverse
    /// on light; color is left to status.
    pub accent: u32,
    pub on_accent: u32,
    pub focus: u32,
    pub link: u32,
    /// Selected text in a field.
    pub selection: u32,
    pub chip: u32,
    /// In-window popovers (「你在这里」, cleanup) sit one step above the pane.
    pub popover: u32,
    pub palette_row: u32,
    /// Matched text in search results: a quiet amber wash, the text keeps its own color.
    pub matched: u32,
    /// Dims the workbench behind the palette, as rgba.
    pub scrim: u32,
    pub inference: u32,
    /// Data charts only (the diagnostics panel): app identity, never status. Validated as
    /// adjacent pairs for color-vision deficiency on the popover in each scheme.
    pub series: [u32; 5],
    /// Everything ranked past the colored apps.
    pub series_other: u32,
    /// Busy time no app accounts for, drawn as a hatch.
    pub hatch: u32,
    pub chart_grid: u32,
    /// A meaningful threshold on an axis, such as one full core.
    pub chart_ref: u32,
}
impl Palette {
    /// Light shares are set against the contrast floors (text 4.5:1, focus and field edges
    /// 3:1) and a surface ladder whose steps stay visible; dark keeps its tuned values.
    const fn new(scheme: Scheme) -> Self {
        let bg = scheme.bg();
        let fg = scheme.fg();
        let light = matches!(scheme, Scheme::Light);
        let vivid = if light { Theme::BLACK } else { Theme::WHITE };
        let primary = mix(fg, vivid, 300);
        let border = mix(bg, fg, by(light, 170, 90));
        let control = mix(bg, fg, by(light, 300, 150));
        // Popovers sit one step above the pane: lighter on light, never pure white.
        let popover = if light {
            mix(bg, Theme::WHITE, 700)
        } else {
            mix(bg, fg, 55)
        };
        Self {
            terminal_bg: bg,
            terminal_fg: fg,
            vivid,
            ink_share: if light { 450 } else { 620 },
            wash_share: if light { 150 } else { 220 },
            app: mix(bg, Theme::BLACK, by(light, 70, 150)),
            panel: bg,
            pane: bg,
            header: bg,
            header_focused: mix(bg, fg, by(light, 120, 90)),
            slot: mix(bg, Theme::BLACK, by(light, 40, 80)),
            todo: mix(bg, fg, by(light, 45, 30)),
            raised: mix(bg, fg, by(light, 105, 70)),
            border,
            card_border: mix(bg, fg, by(light, 260, 100)),
            control,
            field_edge: mix(bg, fg, by(light, 490, 150)),
            todo_border: border,
            slot_border: control,
            button: mix(bg, fg, by(light, 90, 60)),
            button_hover: mix(bg, fg, by(light, 180, 120)),
            primary,
            body: fg,
            secondary: mix(bg, fg, by(light, 720, 580)),
            muted: mix(bg, fg, by(light, 640, 420)),
            accent: primary,
            on_accent: bg,
            focus: mix(bg, fg, by(light, 760, 380)),
            link: mix(bg, fg, 860),
            selection: mix(bg, fg, by(light, 180, 260)),
            chip: mix(bg, fg, by(light, 120, 80)),
            popover,
            palette_row: mix(popover, fg, by(light, 110, 75)),
            matched: mix(popover, 0xe8a33a, by(light, 350, 280)),
            scrim: if light { 0x0000_0040 } else { 0x0000_0099 },
            inference: mix(bg, fg, by(light, 55, 35)),
            series: if light {
                [0x2a78d6, 0xeb6834, 0x1baf7a, 0xeda100, 0xe87ba4]
            } else {
                [0x3987e5, 0xd95926, 0x199e70, 0xc98500, 0xd55181]
            },
            series_other: mix(bg, fg, by(light, 300, 380)),
            hatch: mix(bg, fg, by(light, 260, 240)),
            chart_grid: mix(popover, fg, by(light, 90, 70)),
            chart_ref: mix(bg, fg, by(light, 450, 420)),
        }
    }
}
const fn by(light: bool, on_light: u32, on_dark: u32) -> u32 {
    if light { on_light } else { on_dark }
}
const DARK: Palette = Palette::new(Scheme::Dark);
const LIGHT: Palette = Palette::new(Scheme::Light);
static LIGHT_ON: AtomicBool = AtomicBool::new(false);
macro_rules! tokens {
    ($($name:ident),* $(,)?) => {
        impl Theme {
            $(pub fn $name() -> u32 {
                Self::palette().$name
            })*
        }
    };
}
tokens!(
    terminal_bg,
    terminal_fg,
    app,
    panel,
    pane,
    header,
    header_focused,
    slot,
    todo,
    raised,
    border,
    card_border,
    control,
    field_edge,
    todo_border,
    slot_border,
    button,
    button_hover,
    primary,
    body,
    secondary,
    muted,
    accent,
    on_accent,
    focus,
    link,
    selection,
    chip,
    popover,
    palette_row,
    matched,
    scrim,
    inference,
    series_other,
    hatch,
    chart_grid,
    chart_ref,
);
#[allow(dead_code)] // The complete design palette also covers the deferred components.
impl Theme {
    pub const WHITE: u32 = 0xffffff;
    const BLACK: u32 = 0x000000;
    /// The scheme the window and its terminals paint with; read on every render.
    pub fn scheme() -> Scheme {
        if LIGHT_ON.load(Ordering::Relaxed) {
            Scheme::Light
        } else {
            Scheme::Dark
        }
    }
    pub fn set_scheme(scheme: Scheme) {
        LIGHT_ON.store(scheme == Scheme::Light, Ordering::Relaxed);
    }
    pub fn palette() -> &'static Palette {
        match Self::scheme() {
            Scheme::Dark => &DARK,
            Scheme::Light => &LIGHT,
        }
    }
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
    /// Ghostty's default font-size, so terminal text matches it cell for cell.
    pub const TERMINAL_FONT_SIZE: f32 = 13.;
    pub const CARD_TITLE: f32 = 13.5;
    pub const TITLE: f32 = 20.;
    pub const LINE_HEIGHT: f32 = 1.4;
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
    /// A name being edited gets more room than a tab shows, then shrinks back.
    pub const TAB_RENAME_MAX: f32 = 360.;
    /// A short name still leaves its × at the far edge, clear of a click on the name.
    pub const TAB_MIN_WIDTH: f32 = 96.;
    pub const TAB_MAX_WIDTH: f32 = 200.;
    pub const TAB_TIP_WIDTH: f32 = 420.;
    pub const TAB_TIP_REASON_CHARS: usize = 160;
    pub const TODO_HEIGHT: f32 = 36.;
    pub const PANE_HEADER: f32 = 32.;
    pub const FOOTER: f32 = 26.;
    pub const ROW: f32 = 30.;
    pub const BORDER_WIDTH: f32 = 1.;
    /// The focused split's ring, drawn over its 1px border so the grid keeps its size.
    pub const FOCUS_RING: f32 = 2.;
    /// How much of a split without focus still shows, as Ghostty's default.
    pub const UNFOCUSED_SPLIT_OPACITY: f32 = 0.7;
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
    /// The diagnostics panel and its charts.
    pub const DIAGNOSTIC_WIDTH: f32 = 980.;
    pub const DIAGNOSTIC_HEIGHT: f32 = 860.;
    pub const BIG_NUMBER: f32 = 26.;
    pub const STACK_HEIGHT: f32 = 22.;
    pub const BAR: f32 = 10.;
    pub const METER: f32 = 4.;
    pub const STEP_WIDTH: f32 = 46.;
    pub const STEP_HEIGHT: f32 = 3.;
    pub const SWATCH: f32 = 10.;
    pub const POINT: f32 = 9.;
    pub const TICK_ROW: f32 = 16.;
    pub const BAR_LABEL: f32 = 168.;
    pub const BAR_VALUE: f32 = 54.;
    pub const AXIS_GUTTER: f32 = 48.;
    pub const SCATTER_HEIGHT: f32 = 220.;
    pub const TIP_WIDTH: f32 = 240.;
    /// Room for a centered axis label such as `150%`.
    pub const TICK_LABEL: f32 = 48.;
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
    pub const SETTINGS_WIDTH: f32 = 940.;
    pub const SETTINGS_NAV: f32 = 164.;
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
    /// The chart color of the `i`th ranked app; past the palette, the shared "other".
    pub fn series(i: usize) -> u32 {
        Self::palette()
            .series
            .get(i)
            .copied()
            .unwrap_or(Self::palette().series_other)
    }
    pub fn color(value: u32) -> Hsla {
        hsla(value)
    }
    pub fn terminal_mono(cx: &App) -> Font {
        let mut face = Self::mono(cx);
        let family = super::terminal_text::current().family;
        if !family.is_empty() {
            face.family = family.into();
        }
        face
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
    use super::{Scheme, Theme, mix};

    #[test]
    fn mix_blends_each_channel() {
        assert_eq!(mix(0x000000, 0xffffff, 0), 0x000000);
        assert_eq!(mix(0x000000, 0xffffff, 1000), 0xffffff);
        assert_eq!(mix(0x102030, 0x304050, 500), 0x203040);
        assert_eq!(Theme::pane(), Theme::terminal_bg());
    }

    #[test]
    fn light_scheme_swaps_every_surface() {
        use super::{DARK, LIGHT};
        assert_eq!(LIGHT.terminal_bg, Scheme::Light.bg());
        assert!(LIGHT.app < LIGHT.pane && DARK.app < DARK.pane);
        // Text stays far from its background in both schemes.
        for p in [DARK, LIGHT] {
            let gap = |a: u32, b: u32| ((a & 255) as i32 - (b & 255) as i32).abs();
            assert!(gap(p.body, p.pane) > 180);
            assert!(gap(p.secondary, p.pane) > 100);
            assert!(gap(p.on_accent, p.accent) > 180);
        }
    }

    /// Light meets the contrast floors: text 4.5:1, focus and field edges 3:1, status
    /// dots 3:1 even for the pale yellow and green hues; its surfaces step visibly.
    #[test]
    fn light_scheme_meets_contrast_floors() {
        use super::{Group, LIGHT, Theme, contrast};
        let p = LIGHT;
        for (name, text) in [
            ("body", p.body),
            ("secondary", p.secondary),
            ("muted", p.muted),
        ] {
            assert!(contrast(text, p.pane) >= 4.5, "{name}");
        }
        assert!(contrast(p.focus, p.pane) >= 3.);
        assert!(contrast(p.field_edge, p.pane) >= 3.);
        assert!(
            contrast(p.focus, p.field_edge) >= 1.5,
            "focus must differ from the edge"
        );
        assert!(contrast(p.app, p.pane) >= 1.15);
        assert!(contrast(p.card_border, p.pane) >= 1.6);
        assert_ne!(p.popover, Theme::WHITE);
        for group in Group::ALL {
            let (dot, ink, _) = group.colors_in(&p);
            assert!(contrast(dot, p.pane) >= 3., "{} dot", group.label());
            assert!(contrast(ink, p.pane) >= 4.5, "{} ink", group.label());
        }
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
