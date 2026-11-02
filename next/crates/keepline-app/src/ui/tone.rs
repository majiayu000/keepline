//! Meaning colors for everything that is not a session: a form error, a used-up quota,
//! the host link, a phone's connection, a plugin's line. They never borrow a session
//! status color, so recoloring 在跑 or 等你回答 leaves them alone (specs/status-model).
use super::status_colors::StatusColors;
use super::theme::{Theme, hsla, tone_of};
use gpui::Hsla;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// Fine: connected, within limits, succeeded.
    Ok = 0,
    /// Worth a look: near a limit, missing something optional.
    Warn = 1,
    /// Failed or blocked.
    Danger = 2,
}
impl Tone {
    pub const ALL: [Self; 3] = [Self::Ok, Self::Warn, Self::Danger];
    pub fn key(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Danger => "danger",
        }
    }
    /// The raw hue, for drawings that are not text or chrome (a preview's window buttons).
    pub fn hue(self) -> u32 {
        StatusColors::get().tone(self)
    }
    /// Dot, ink and wash, by the same formula as the status colors.
    pub fn colors(self) -> (Hsla, Hsla, Hsla) {
        let (dot, ink, wash) = tone_of(StatusColors::get().tone(self), Theme::palette());
        (hsla(dot), hsla(ink), hsla(wash))
    }
    pub fn dot(self) -> Hsla {
        self.colors().0
    }
    /// Text in this tone, readable on the pane.
    pub fn ink(self) -> Hsla {
        self.colors().1
    }
    pub fn wash(self) -> Hsla {
        self.colors().2
    }
}

#[cfg(test)]
mod tests {
    use crate::ui::theme::Status;
    use std::path::{Path, PathBuf};

    fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    /// Each file's code before its `#[cfg(test)]`, skipping files that are only tests or
    /// screenshot scenes.
    fn product_code() -> Vec<(String, String)> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = vec![];
        sources(&root, &mut files);
        files
            .into_iter()
            .filter_map(|path| {
                let name = path.strip_prefix(&root).unwrap().display().to_string();
                let test_only = name.ends_with("tests.rs")
                    || name.ends_with("_evidence.rs")
                    || ["harness.rs", "workbench/visual.rs"].contains(&name.as_str());
                if test_only {
                    return None;
                }
                let text = std::fs::read_to_string(&path).unwrap();
                let code = text.split("#[cfg(test)]").next().unwrap().to_owned();
                Some((name, code))
            })
            .collect()
    }

    /// One judgment (specs/status-model): what a session is doing is decided in
    /// `keepline_core::status`, so no view reads the turn and decides for itself.
    #[test]
    fn views_never_judge_a_turn_themselves() {
        for (name, code) in product_code() {
            assert!(
                !code.contains("Turn::"),
                "{name} reads a Turn; ask status_of / session_status instead"
            );
        }
    }

    /// Status colors are for sessions. A file outside this list that wants "fine",
    /// "careful" or "failed" takes a Tone; one that draws a session's state takes it
    /// from that session, not from a named status.
    #[test]
    fn named_status_colors_stay_where_sessions_are_drawn() {
        const DRAWS_SESSIONS: [&str; 8] = [
            // Column icons, the 在跑 sweep and 做完了 marks on the board.
            "workbench/board.rs",
            // The 做完了 stamp when you mark one complete.
            "workbench/complete.rs",
            // The 很久没动 age chart.
            "workbench/overview.rs",
            // A deferred step in the cleanup walk.
            "workbench/cleanup.rs",
            // 「N 工作中」「N 等你」 counts.
            "workbench/statusbar.rs",
            // The sidebar's 很久没动 summary row.
            "workbench/sidebar.rs",
            "ui/theme.rs",
            "ui/status_colors.rs",
        ];
        let named: Vec<String> = Status::ALL
            .iter()
            .flat_map(|s| {
                let v = format!("Status::{s:?}");
                [
                    format!("{v}.colors()"),
                    format!("{v}.hue()"),
                    format!("dot({v})"),
                    format!("glyph({v})"),
                ]
            })
            .collect();
        for (name, code) in product_code() {
            if DRAWS_SESSIONS.contains(&name.as_str()) {
                continue;
            }
            for n in &named {
                assert!(!code.contains(n.as_str()), "{name} borrows {n}; use a Tone");
            }
        }
    }
}
