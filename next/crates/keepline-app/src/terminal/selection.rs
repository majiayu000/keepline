//! Local text selection, kept by libghostty on the terminal itself so it stays on its
//! text as output scrolls it into history, and spans history like Ghostty's.
use keepline_core::terminal::libghostty_vt::{
    error::Result as GhosttyResult,
    screen::GridRef,
    selection::gesture::{
        Autoscroll, AutoscrollTickEvent, DragEvent, Geometry, Gesture, PressEvent, ReleaseEvent,
    },
    terminal::{Point, PointCoordinate, Terminal},
};
use std::time::{Duration, Instant};

/// The pane's cell grid in logical pixels, plus the backing scale for Ghostty's
/// pixel-based click and drag thresholds.
#[derive(Clone, Copy, Debug)]
pub struct Grid {
    pub cols: u16,
    pub rows: u16,
    pub cell_w: f32,
    pub cell_h: f32,
    pub scale: f32,
}
impl Grid {
    /// Cell under a pane-relative logical point, clamped to the grid.
    pub fn cell(&self, x: f32, y: f32) -> (u16, u16) {
        (
            ((x / self.cell_w).floor().max(0.) as u16).min(self.cols.saturating_sub(1)),
            ((y / self.cell_h).floor().max(0.) as u16).min(self.rows.saturating_sub(1)),
        )
    }
    /// Ghostty's whole-pixel cell size. A Retina cell is often fractional (7.8 logical
    /// points = 15.6 pixels); rounding it alone made Ghostty place a point 0.4 px per
    /// column further left than the grid draws it, so drag edges lost the last cell.
    fn cell_px(&self) -> (f64, f64) {
        (
            f64::from((self.cell_w * self.scale).round().max(1.)),
            f64::from((self.cell_h * self.scale).round().max(1.)),
        )
    }
    /// A pane-relative point in Ghostty's pixel space, scaled per cell so each cell
    /// spans exactly `cell_px` there, as it spans one drawn cell here.
    fn position(&self, at: (f32, f32)) -> (f64, f64) {
        let (w, h) = self.cell_px();
        (
            f64::from(at.0 / self.cell_w) * w,
            f64::from(at.1 / self.cell_h) * h,
        )
    }
    fn geometry(&self) -> Geometry {
        let (w, h) = self.cell_px();
        Geometry {
            columns: self.cols as u32,
            cell_width: w as u32,
            padding_left: 0,
            screen_height: (h as u32) * self.rows as u32,
        }
    }
}

/// One pane's selection gesture: click count, drag and edge autoscroll.
pub struct Selecting {
    gesture: Gesture<'static>,
    press: PressEvent<'static>,
    release: ReleaseEvent<'static>,
    drag: DragEvent<'static>,
    tick: AutoscrollTickEvent<'static>,
    started: Instant,
    pub active: bool,
    last: (f32, f32),
    pub autoscrolling: bool,
}
impl Selecting {
    pub fn new() -> Result<Self, String> {
        let e = |e: keepline_core::terminal::libghostty_vt::Error| e.to_string();
        let mut press = PressEvent::new().map_err(e)?;
        press
            .set_repeat_interval(Duration::from_millis(500))
            .map_err(e)?;
        Ok(Self {
            gesture: Gesture::new().map_err(e)?,
            press,
            release: ReleaseEvent::new().map_err(e)?,
            drag: DragEvent::new().map_err(e)?,
            tick: AutoscrollTickEvent::new().map_err(e)?,
            started: Instant::now(),
            active: false,
            last: (0., 0.),
            autoscrolling: false,
        })
    }
    fn cell<'t>(t: &'t Terminal, grid: Grid, at: (f32, f32)) -> GhosttyResult<GridRef<'t>> {
        let (col, row) = grid.cell(at.0, at.1);
        t.grid_ref(Point::Viewport(PointCoordinate {
            x: col,
            y: row as u32,
        }))
    }
    /// Click: cell, word or line by click count (Ghostty's default behavior table).
    pub fn press(&mut self, t: &Terminal, grid: Grid, at: (f32, f32)) -> GhosttyResult<()> {
        let selection = self
            .press
            .set_repeat_distance(grid.cell_px().0)?
            .set_time(self.started.elapsed())?
            .set_position(grid.position(at).0, grid.position(at).1)?
            .apply(&mut self.gesture, t, Self::cell(t, grid, at)?)?;
        t.set_selection(selection.as_ref())?;
        self.active = true;
        self.last = at;
        Ok(())
    }
    /// Extends the selection to `at`, which may lie outside the pane; reports
    /// whether the viewport should scroll to follow it.
    pub fn drag(&mut self, t: &Terminal, grid: Grid, at: (f32, f32)) -> GhosttyResult<Autoscroll> {
        self.last = at;
        let selection = self
            .drag
            .set_position(grid.position(at).0, grid.position(at).1)?
            .apply(
                &mut self.gesture,
                t,
                Self::cell(t, grid, at)?,
                grid.geometry(),
            )?;
        t.set_selection(selection.as_ref())?;
        self.gesture.autoscroll(t)
    }
    pub fn release(&mut self, t: &Terminal, grid: Grid, at: (f32, f32)) {
        let _ = self
            .release
            .apply(&mut self.gesture, t, Self::cell(t, grid, at).ok());
        self.active = false;
        self.autoscrolling = false;
    }
    /// Which way a held drag still wants the viewport to move.
    pub fn autoscroll(&self, t: &Terminal) -> GhosttyResult<Autoscroll> {
        self.gesture.autoscroll(t)
    }
    /// One autoscroll step after the viewport moved; extends to the held position.
    pub fn tick(&mut self, t: &Terminal, grid: Grid) -> GhosttyResult<()> {
        let (col, row) = grid.cell(self.last.0, self.last.1);
        let selection = self.tick.apply(
            &mut self.gesture,
            t,
            PointCoordinate {
                x: col,
                y: row as u32,
            },
            grid.geometry(),
        )?;
        t.set_selection(selection.as_ref())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keepline_core::terminal::{Vt, libghostty_vt::terminal::ScrollViewport};

    const GRID: Grid = Grid {
        cols: 20,
        rows: 4,
        cell_w: 7.5,
        cell_h: 15.,
        scale: 2.,
    };
    fn at(col: u16, row: u16) -> (f32, f32) {
        (col as f32 * GRID.cell_w + 2., row as f32 * GRID.cell_h + 2.)
    }
    /// Right half of a cell: Ghostty includes the end cell once the pointer passes its middle.
    fn past(col: u16, row: u16) -> (f32, f32) {
        (
            (col + 1) as f32 * GRID.cell_w - 1.,
            row as f32 * GRID.cell_h + 2.,
        )
    }
    fn selected(vt: &Vt) -> Option<String> {
        vt.selection_text().unwrap()
    }

    #[test]
    fn click_drag_double_and_triple_click_select_like_ghostty() {
        let mut vt = Vt::new(20, 4).unwrap();
        vt.feed(b"alpha beta gamma\r\nsecond line");
        let mut sel = Selecting::new().unwrap();
        let t = vt.terminal();
        sel.press(t, GRID, at(0, 0)).unwrap();
        sel.drag(t, GRID, at(3, 1)).unwrap();
        assert_eq!(selected(&vt).as_deref(), Some("alpha beta gamma\nsec"));
        sel.drag(t, GRID, past(3, 1)).unwrap();
        sel.release(t, GRID, past(3, 1));
        assert_eq!(selected(&vt).as_deref(), Some("alpha beta gamma\nseco"));
        // Second and third click at the same spot within the interval: word, then line.
        let t = vt.terminal();
        sel.press(t, GRID, at(7, 0)).unwrap();
        sel.release(t, GRID, at(7, 0));
        sel.press(t, GRID, at(7, 0)).unwrap();
        assert_eq!(selected(&vt).as_deref(), Some("beta"));
        sel.release(vt.terminal(), GRID, at(7, 0));
        sel.press(vt.terminal(), GRID, at(7, 0)).unwrap();
        assert_eq!(selected(&vt).as_deref(), Some("alpha beta gamma"));
    }

    #[test]
    fn fractional_retina_cells_keep_drag_edges_on_the_drawn_grid() {
        // JetBrains Mono at 13 pt is 7.8 points wide: 15.6 pixels, which Ghostty's
        // whole-pixel geometry rounds to 16. Far right, that put a point a cell left.
        const FRACTIONAL: Grid = Grid {
            cols: 80,
            rows: 4,
            cell_w: 7.8,
            cell_h: 17.,
            scale: 2.,
        };
        let line: String = (0..80).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
        let mut vt = Vt::new(80, 4).unwrap();
        vt.feed(line.as_bytes());
        let point = |col: f32| (col * FRACTIONAL.cell_w, FRACTIONAL.cell_h / 2.);
        // Ghostty takes a cell once the pointer passes 60% of it: from column 2's left
        // part to 30% into column `end` stops before it; to 80% takes it.
        for end in 3..80u16 {
            for (frac, last) in [(0.3, end - 1), (0.8, end)] {
                let mut sel = Selecting::new().unwrap();
                sel.press(vt.terminal(), FRACTIONAL, point(2.2)).unwrap();
                sel.drag(vt.terminal(), FRACTIONAL, point(end as f32 + frac))
                    .unwrap();
                assert_eq!(
                    selected(&vt).as_deref(),
                    Some(&line[2..=last as usize]),
                    "drag to {frac} into column {end}"
                );
            }
        }
    }

    #[test]
    fn double_click_word_follows_terminal_word_rules() {
        const WIDE: Grid = Grid { cols: 60, ..GRID };
        let mut vt = Vt::new(60, 4).unwrap();
        vt.feed("git log --oneline src/main.rs (实现反馈) \x1b[31mred\x1b[0mgreen".as_bytes());
        let mut sel = Selecting::new().unwrap();
        let mut word = |col| {
            // Each pair of clicks is far apart in time from the next, via release + press.
            sel.press(vt.terminal(), WIDE, at(col, 0)).unwrap();
            sel.release(vt.terminal(), WIDE, at(col, 0));
            sel.press(vt.terminal(), WIDE, at(col, 0)).unwrap();
            sel.release(vt.terminal(), WIDE, at(col, 0));
            let got = selected(&vt);
            // A third press would count as a triple click; start over far away.
            sel.press(vt.terminal(), WIDE, at(0, 3)).unwrap();
            sel.release(vt.terminal(), WIDE, at(0, 3));
            got
        };
        assert_eq!(word(1).as_deref(), Some("git"));
        assert_eq!(word(9).as_deref(), Some("--oneline"));
        assert_eq!(word(20).as_deref(), Some("src/main.rs"));
        assert_eq!(word(33).as_deref(), Some("实现反馈"));
        assert_eq!(word(42).as_deref(), Some("redgreen"));
    }

    #[test]
    fn selection_follows_text_into_scrollback_and_spans_it() {
        let mut vt = Vt::new(20, 4).unwrap();
        vt.feed(b"first\r\nsecond\r\nthird\r\nfourth");
        let mut sel = Selecting::new().unwrap();
        sel.press(vt.terminal(), GRID, at(0, 0)).unwrap();
        sel.drag(vt.terminal(), GRID, past(4, 0)).unwrap();
        sel.release(vt.terminal(), GRID, past(4, 0));
        // New output pushes the selected row into history; the selection stays on it.
        vt.feed(b"\r\nfifth\r\nsixth");
        assert_eq!(selected(&vt).as_deref(), Some("first"));
        // Scroll back, start in history, drag down to the live screen.
        vt.scroll(ScrollViewport::Top);
        sel.press(vt.terminal(), GRID, at(0, 1)).unwrap();
        vt.scroll(ScrollViewport::Bottom);
        sel.drag(vt.terminal(), GRID, past(4, 3)).unwrap();
        assert_eq!(
            selected(&vt).as_deref(),
            Some("second\nthird\nfourth\nfifth\nsixth")
        );
    }

    #[test]
    fn dragging_past_the_pane_edge_requests_autoscroll() {
        let mut vt = Vt::new(20, 4).unwrap();
        for i in 0..20 {
            vt.feed(format!("line {i}\r\n").as_bytes());
        }
        let mut sel = Selecting::new().unwrap();
        sel.press(vt.terminal(), GRID, at(0, 2)).unwrap();
        let up = sel.drag(vt.terminal(), GRID, (1., -10.)).unwrap();
        assert_eq!(up, Autoscroll::Up);
        let before = selected(&vt).unwrap();
        assert!(before.starts_with("line 17"), "{before:?}");
        vt.scroll(ScrollViewport::Delta(-1));
        sel.tick(vt.terminal(), GRID).unwrap();
        let after = selected(&vt).unwrap();
        // The selection grew into history and still ends at the anchor row.
        assert!(
            after.len() > before.len() && after.ends_with("line 18"),
            "{after:?}"
        );
        assert!(!vt.terminal().viewport_active().unwrap());
    }

    #[test]
    fn selection_ending_inside_a_wide_character_is_drawn_over_the_whole_character() {
        // a b 中中 文文 字字 c d: Ghostty takes a character from either half and copies
        // it whole; the highlight must cover both of its cells, not stop in the middle.
        let mut vt = Vt::new(20, 4).unwrap();
        vt.feed("ab中文字cd".as_bytes());
        let mut screen = crate::terminal::screen::Screen::default();
        let mut capture = crate::terminal::screen::Capture::new().unwrap();
        for (from, to, text, cols) in [
            (at(3, 0), at(5, 0), "中文", (2, 5)),
            (past(3, 0), past(4, 0), "文", (4, 5)),
            (at(0, 0), past(3, 0), "ab中", (0, 3)),
            (at(5, 0), at(3, 0), "中文", (2, 5)),
        ] {
            let mut sel = Selecting::new().unwrap();
            sel.press(vt.terminal(), GRID, from).unwrap();
            sel.drag(vt.terminal(), GRID, to).unwrap();
            sel.release(vt.terminal(), GRID, to);
            capture.update(&vt, &mut screen).unwrap();
            assert_eq!(selected(&vt).as_deref(), Some(text));
            assert_eq!(screen.selection[0], Some(cols), "{text}");
        }
    }
}
