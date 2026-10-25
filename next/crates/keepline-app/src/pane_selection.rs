use crate::screen::Run;

type Cell = (u16, u16);

/// A click is only a drag anchor; explicit inclusive ranges may contain one cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextSelection {
    Anchor(Cell),
    Range {
        anchor: Cell,
        head: Cell,
    },
}

impl TextSelection {
    pub fn range(anchor: Cell, head: Cell) -> Self {
        Self::Range { anchor, head }
    }

    pub fn bounds(self) -> Option<(Cell, Cell)> {
        match self {
            Self::Anchor(_) => None,
            Self::Range { anchor, head } => Some((anchor.min(head), anchor.max(head))),
        }
    }

    pub fn drag_to(self, head: Cell) -> Self {
        let anchor = match self {
            Self::Anchor(anchor) | Self::Range { anchor, .. } => anchor,
        };
        if anchor == head {
            Self::Anchor(anchor)
        } else {
            Self::range(anchor, head)
        }
    }
}

pub fn copied_text(rows: &[Vec<Run>], selection: Option<TextSelection>) -> String {
    let selected = selection.and_then(TextSelection::bounds);
    let mut lines = Vec::new();
    for (y, row) in rows.iter().enumerate() {
        let mut text = String::new();
        for run in row {
            if run.text.is_ascii() {
                for (i, c) in run.text.chars().enumerate() {
                    let pos = (y as u16, run.col + i as u16);
                    if selected.is_none_or(|(a, b)| a <= pos && pos <= b) {
                        text.push(c);
                    }
                }
            } else {
                let pos = (y as u16, run.col);
                if selected.is_none_or(|(a, b)| a <= pos && pos <= b) {
                    text.push_str(&run.text);
                }
            }
        }
        if selected.is_none_or(|(a, b)| a.0 <= y as u16 && y as u16 <= b.0) {
            lines.push(text.trim_end().to_owned());
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::{Attrs, word_around};

    fn rows(text: &[&str]) -> Vec<Vec<Run>> {
        text.iter()
            .map(|text| {
                vec![Run {
                    col: 0,
                    cols: text.len() as u16,
                    text: (*text).into(),
                    fg: 0,
                    bg: None,
                    attrs: Attrs::default(),
                }]
            })
            .collect()
    }

    #[test]
    fn single_cell_words_and_separators_keep_explicit_bounds_and_copy() {
        let rows = rows(&[" x ( ", "other row"]);
        for (col, expected) in [(1, "x"), (3, "("), (0, "")] {
            let (start, end) = word_around(&rows[0], col);
            let selection = TextSelection::range((0, start), (0, end));
            assert_eq!(selection.bounds(), Some(((0, col), (0, col))));
            assert_eq!(copied_text(&rows, Some(selection)), expected);
        }
    }

    #[test]
    fn click_and_empty_drag_preserve_full_viewport_copy() {
        let rows = rows(&[" x ", "next"]);
        let click = TextSelection::Anchor((0, 1));
        for selection in [
            click,
            click.drag_to((0, 1)),
            click.drag_to((1, 2)).drag_to((0, 1)),
        ] {
            assert_eq!(selection.bounds(), None);
            assert_eq!(copied_text(&rows, Some(selection)), " x\nnext");
        }
        assert_eq!(copied_text(&rows, None), " x\nnext");
    }

    #[test]
    fn forward_reverse_and_cross_row_drags_keep_inclusive_bounds() {
        let rows = rows(&["abcd", "efgh"]);
        for (anchor, head, expected) in [
            ((0, 1), (0, 2), "bc"),
            ((0, 2), (0, 1), "bc"),
            ((0, 2), (1, 1), "cd\nef"),
            ((1, 1), (0, 2), "cd\nef"),
        ] {
            let selection = TextSelection::Anchor(anchor).drag_to(head);
            assert_eq!(
                selection.bounds(),
                Some((anchor.min(head), anchor.max(head)))
            );
            assert_eq!(copied_text(&rows, Some(selection)), expected);
        }
    }

    #[test]
    fn one_cell_row_and_select_all_are_not_click_anchors() {
        let rows = rows(&["x"]);
        let selection = TextSelection::range((0, 0), (0, 0));
        assert_eq!(selection.bounds(), Some(((0, 0), (0, 0))));
        assert_eq!(copied_text(&rows, Some(selection)), "x");
        assert_eq!(TextSelection::Anchor((0, 0)).bounds(), None);
    }
}
