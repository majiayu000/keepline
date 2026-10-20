#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub col: u16,
    pub cols: u16,
    pub text: String,
    pub fg: u32,
    pub bg: Option<u32>,
    pub bold: bool,
    pub italic: bool,
}

#[derive(Default)]
pub struct Screen {
    pub rows: Vec<Vec<Run>>,
    pub cursor: Option<(u16, u16)>,
    pub cursor_visible: bool,
    pub background: u32,
}

pub fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) << 16 | (g as u32) << 8 | b as u32
}

/// Builds runs cell by cell; a run breaks when the style changes.
pub struct RowBuilder<'a> {
    row: &'a mut Vec<Run>,
}

impl<'a> RowBuilder<'a> {
    pub fn new(row: &'a mut Vec<Run>) -> Self {
        row.clear();
        Self { row }
    }
    pub fn push(
        &mut self,
        col: u16,
        width: u16,
        text: &str,
        fg: u32,
        bg: Option<u32>,
        bold: bool,
        italic: bool,
    ) {
        if let Some(last) = self.row.last_mut()
            && last.fg == fg
            && last.bg == bg
            && last.bold == bold
            && last.italic == italic
            && last.col + last.cols == col
            && width == 1
            && text.is_ascii()
            && last.text.chars().all(|c| c.is_ascii())
        {
            last.text.push_str(text);
            last.cols += width;
            return;
        }
        // Wide / non-ASCII cells get their own run so each is placed on the cell grid.
        self.row.push(Run {
            col,
            cols: width,
            text: text.to_owned(),
            fg,
            bg,
            bold,
            italic,
        });
    }
}

pub const DEFAULT_FG: u32 = crate::theme::Theme::BODY;
pub const DEFAULT_BG: u32 = crate::theme::Theme::PANE;

use keepline_core::terminal::{
    Vt,
    libghostty_vt::{
        self,
        render::{CellIterator, RenderState, RowIterator},
        screen::CellWide,
    },
};
pub fn capture(vt: &Vt) -> Result<Screen, libghostty_vt::Error> {
    let mut state = RenderState::new()?;
    let mut row_it = RowIterator::new()?;
    let mut cell_it = CellIterator::new()?;
    let mut out = Screen::default();
    let snap = state.update(vt.terminal())?;
    // Only the unset defaults come from the UI theme; OSC and explicit ANSI colors survive.
    let def_fg = vt
        .terminal()
        .fg_color()?
        .map(|c| rgb(c.r, c.g, c.b))
        .unwrap_or(DEFAULT_FG);
    let mut rows = row_it.update(&snap)?;
    let mut y = 0usize;
    let mut text = String::new();
    while let Some(row) = rows.next() {
        if out.rows.len() <= y {
            out.rows.push(Vec::new());
        }
        let mut b = RowBuilder::new(&mut out.rows[y]);
        let mut cells = cell_it.update(row)?;
        let mut x = 0u16;
        while let Some(cell) = cells.next() {
            let raw = cell.raw_cell()?;
            let wide = raw.wide()?;
            if matches!(wide, CellWide::SpacerTail | CellWide::SpacerHead) {
                x += 1;
                continue;
            }
            text.clear();
            if cell.graphemes_len()? == 0 {
                text.push(' ');
            } else {
                cell.graphemes_utf8(&mut text)?;
            }
            let mut fg = cell
                .fg_color()?
                .map(|c| rgb(c.r, c.g, c.b))
                .unwrap_or(def_fg);
            let mut bg = cell.bg_color()?.map(|c| rgb(c.r, c.g, c.b));
            let (mut bold, mut italic) = (false, false);
            if cell.has_styling()? {
                let st = cell.style()?;
                if st.inverse {
                    let old = fg;
                    fg = bg.unwrap_or(DEFAULT_BG);
                    bg = Some(old);
                }
                bold = st.bold;
                italic = st.italic;
            }
            let width = if matches!(wide, CellWide::Wide) { 2 } else { 1 };
            b.push(x, width, &text, fg, bg, bold, italic);
            x += 1;
        }
        y += 1;
    }
    out.rows.truncate(y);
    let cur = snap.cursor()?;
    out.cursor = cur.viewport.map(|v| (v.x, v.y));
    out.cursor_visible = cur.visible;
    out.background = vt
        .terminal()
        .bg_color()?
        .map(|c| rgb(c.r, c.g, c.b))
        .unwrap_or(DEFAULT_BG);
    snap.clean()?;
    Ok(out)
}

use gpui::{
    App, Bounds, Font, FontStyle, FontWeight, Hsla, Pixels, Rgba, ShapedLine, SharedString,
    TextAlign, TextRun, UnderlineStyle, Window, fill, point, px, size,
};
pub const FONT_SIZE: f32 = crate::theme::Theme::MONO_SIZE;

/// One measurement shared by terminal paint, resize, preedit and IME geometry.
#[derive(Clone, Copy, Debug)]
pub struct CellMetrics {
    pub width: Pixels,
    pub height: Pixels,
}
impl CellMetrics {
    pub fn measure(font: &Font, window: &Window) -> Option<Self> {
        let ts = window.text_system();
        let width = ts
            .advance(ts.resolve_font(font), px(FONT_SIZE), 'm')
            .ok()?
            .width;
        (width > px(0.)).then_some(Self {
            width,
            height: px(FONT_SIZE * crate::theme::Theme::TERMINAL_LINE_HEIGHT),
        })
    }
    pub fn cursor_bounds(self, bounds: Bounds<Pixels>, cursor: (u16, u16)) -> Bounds<Pixels> {
        Bounds::new(
            point(
                bounds.left() + self.width * cursor.0 as f32,
                bounds.top() + self.height * cursor.1 as f32,
            ),
            size(self.width, self.height),
        )
    }
}

#[derive(Default)]
pub struct CursorAnchor(Option<(u16, u16)>);
impl CursorAnchor {
    pub fn observe(&mut self, cursor: Option<(u16, u16)>) {
        if cursor.is_some() {
            self.0 = cursor;
        }
    }
    pub fn get(&self) -> Option<(u16, u16)> {
        self.0
    }
}

pub struct MarkedText {
    pub line: ShapedLine,
    pub bounds: Bounds<Pixels>,
}
impl MarkedText {
    pub fn new(text: &str, font: Font, anchor: Bounds<Pixels>, window: &Window) -> Self {
        let line = window.text_system().shape_line(
            SharedString::from(text.to_owned()),
            px(FONT_SIZE),
            &[TextRun {
                len: text.len(),
                font,
                color: hsla(DEFAULT_FG),
                background_color: None,
                underline: Some(UnderlineStyle {
                    color: Some(hsla(DEFAULT_FG)),
                    thickness: px(crate::theme::Theme::BORDER_WIDTH),
                    wavy: false,
                }),
                strikethrough: None,
            }],
            None,
        );
        let bounds = Bounds::new(
            anchor.origin,
            size(
                anchor.size.width * (line.width() / anchor.size.width).ceil().max(1.),
                anchor.size.height,
            ),
        );
        Self { line, bounds }
    }
    pub fn range_bounds(&self, range: std::ops::Range<usize>) -> Bounds<Pixels> {
        let start = utf16_byte_offset(&self.line.text, range.start);
        let end = utf16_byte_offset(&self.line.text, range.end).max(start);
        let x = self.line.x_for_index(start);
        Bounds::new(
            point(self.bounds.left() + x, self.bounds.top()),
            size(self.line.x_for_index(end) - x, self.bounds.size.height),
        )
    }
    pub fn paint(&self, background: u32, window: &mut Window, cx: &mut App) {
        window.paint_quad(fill(self.bounds, hsla(background)));
        let _ = self.line.paint(
            self.bounds.origin,
            self.bounds.size.height,
            TextAlign::Left,
            None,
            window,
            cx,
        );
    }
}
fn utf16_byte_offset(text: &str, offset: usize) -> usize {
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        if units + ch.len_utf16() > offset {
            return byte;
        }
        units += ch.len_utf16();
    }
    text.len()
}

fn shape_cell(
    run: &Run,
    font: Font,
    metrics: CellMetrics,
    window: &Window,
) -> (ShapedLine, Pixels) {
    let tr = TextRun {
        len: run.text.len(),
        font,
        color: hsla(run.fg),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let ts = window.text_system();
    let mut line = ts.shape_line(
        SharedString::from(run.text.clone()),
        px(FONT_SIZE),
        &[tr.clone()],
        None,
    );
    let mut offset = px(0.);
    if !run.text.is_ascii()
        && line.width() > px(0.)
        && (run.cols == 2
            || run
                .text
                .chars()
                .any(|c| matches!(c as u32, 0xe000..=0xf8ff | 0xf0000..=0xffffd)))
    {
        // Ghostty scales fallback faces by ideograph metrics (Collection.scaleFactor,
        // default_fallback_adjustment=ic_width) then centers them on the cell grid.
        // GPUI picks the fallback face during shaping. Match this wide cell's shaped
        // advance to two columns, keeping the aspect ratio and fitting the row height.
        let target = metrics.width * run.cols as f32;
        let scale = (target / line.width()).min(metrics.height / px(FONT_SIZE));
        line = ts.shape_line(
            SharedString::from(run.text.clone()),
            px(FONT_SIZE) * scale,
            &[tr],
            None,
        );
        offset = ((target - line.width()) / 2.).max(px(0.));
    }
    (line, offset)
}
pub fn hsla(c: u32) -> Hsla {
    Rgba {
        r: ((c >> 16) & 255) as f32 / 255.,
        g: ((c >> 8) & 255) as f32 / 255.,
        b: (c & 255) as f32 / 255.,
        a: 1.,
    }
    .into()
}
pub fn paint_pane(
    sc: &Screen,
    f: &Font,
    bounds: Bounds<Pixels>,
    metrics: CellMetrics,
    show_cursor: bool,
    window: &mut Window,
    cx: &mut App,
) {
    window.paint_quad(fill(bounds, hsla(sc.background)));
    let cw = metrics.width;
    let lh = metrics.height;
    for (y, row) in sc.rows.iter().enumerate() {
        let oy = bounds.origin.y + lh * y as f32;
        if oy > bounds.origin.y + bounds.size.height {
            break;
        }
        for run in row {
            let ox = bounds.origin.x + cw * run.col as f32;
            if let Some(bg) = run.bg {
                window.paint_quad(fill(
                    Bounds::new(point(ox, oy), size(cw * run.cols as f32, lh)),
                    hsla(bg),
                ));
            }
            if run.text.trim().is_empty() {
                continue;
            }
            let mut font = f.clone();
            if run.bold {
                font.weight = FontWeight::BOLD;
            }
            if run.italic {
                font.style = FontStyle::Italic;
            }
            let (line, offset) = shape_cell(run, font, metrics, window);
            let _ = line.paint(
                point(ox + offset, oy),
                lh,
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
    }
    if show_cursor
        && sc.cursor_visible
        && let Some((x, y)) = sc.cursor
    {
        window.paint_quad(fill(
            Bounds::new(
                point(
                    bounds.origin.x + cw * x as f32,
                    bounds.origin.y + lh * y as f32,
                ),
                size(cw, lh),
            ),
            hsla(DEFAULT_FG).opacity(0.6),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_cursor_keeps_ime_anchor_and_missing_position_keeps_last_known() {
        let mut vt = Vt::new(80, 24).unwrap();
        vt.feed(b"\x1b[6;12H\x1b[?25l");
        let screen = capture(&vt).unwrap();
        assert_eq!(screen.cursor, Some((11, 5)));
        assert!(!screen.cursor_visible);
        let mut anchor = CursorAnchor::default();
        assert_eq!(anchor.get(), None);
        anchor.observe(screen.cursor);
        anchor.observe(None);
        assert_eq!(anchor.get(), Some((11, 5)));
        anchor.observe(Some((7, 3)));
        assert_eq!(anchor.get(), Some((7, 3)));
    }

    #[test]
    fn captured_cjk_occupies_two_columns_and_combining_text_stays_separate() {
        let mut vt = Vt::new(80, 24).unwrap();
        vt.feed("A实现反馈日本語한글B e\u{301}".as_bytes());
        let screen = capture(&vt).unwrap();
        let runs: Vec<_> = screen.rows[0]
            .iter()
            .filter(|r| !r.text.trim().is_empty())
            .collect();
        assert_eq!(runs[0].text, "A");
        for (i, expected) in "实现反馈日本語한글".chars().enumerate() {
            assert_eq!(runs[i + 1].text, expected.to_string());
            assert_eq!(runs[i + 1].col, 1 + i as u16 * 2);
            assert_eq!(runs[i + 1].cols, 2);
        }
        assert!(runs.iter().any(|r| r.text == "e\u{301}" && r.cols == 1));
    }

    #[test]
    fn ime_offsets_are_utf16_not_utf8_and_never_split_surrogates() {
        let text = "n你😀hao";
        assert_eq!(utf16_byte_offset(text, 1), 1);
        assert_eq!(utf16_byte_offset(text, 2), 4);
        assert_eq!(utf16_byte_offset(text, 3), 4);
        assert_eq!(utf16_byte_offset(text, 4), 8);
        assert_eq!(utf16_byte_offset(text, usize::MAX), text.len());
    }
}

/// Run by the isolated native-render harness, using macOS fonts rather than GPUI's
/// mock text system. Both IME range geometry and the painted overlay use this layout.
#[cfg(feature = "visual-test")]
pub fn verify_native_geometry(window: &Window, cx: &App) {
    let font = crate::theme::Theme::mono(cx);
    let metrics = CellMetrics::measure(&font, window).expect("native font measurement");
    for ch in [
        '\u{e0b0}', '\u{e0b2}', '\u{e0a0}', '\u{f120}', '\u{f07b}', '\u{f013}',
    ] {
        let run = Run {
            col: 0,
            cols: 1,
            text: ch.to_string(),
            fg: DEFAULT_FG,
            bg: None,
            bold: false,
            italic: false,
        };
        let (line, offset) = shape_cell(&run, font.clone(), metrics, window);
        assert!(
            line.runs.iter().any(|r| !r.glyphs.is_empty())
                && line
                    .runs
                    .iter()
                    .flat_map(|r| &r.glyphs)
                    .all(|g| g.id.0 != 0),
            "missing symbol {ch}"
        );
        assert!(
            (f32::from(line.width() + offset * 2. - metrics.width)).abs() < 0.1,
            "symbol not aligned: {ch}"
        );
    }
    eprintln!("native bundled symbol font: six Nerd/Powerline glyphs present and cell-aligned");
    let pane = Bounds::new(point(px(37.), px(83.)), size(px(700.), px(500.)));
    let anchor = metrics.cursor_bounds(pane, (11, 5));
    assert_eq!(
        anchor.origin,
        point(
            pane.left() + metrics.width * 11.,
            pane.top() + metrics.height * 5.
        )
    );
    for text in ["nihao", "n你😀hao", "e\u{301}"] {
        let marked = MarkedText::new(text, font.clone(), anchor, window);
        assert_eq!(marked.bounds.origin, anchor.origin);
        let whole = marked.range_bounds(0..text.encode_utf16().count());
        assert_eq!(whole.origin, marked.bounds.origin);
        assert_eq!(whole.size.width, marked.line.width());
        assert_eq!(whole.size.height, metrics.height);
        for (byte, _) in text.char_indices() {
            let index = text[..byte].encode_utf16().count();
            let caret = marked.range_bounds(index..index);
            assert_eq!(
                caret.left(),
                marked.bounds.left() + marked.line.x_for_index(byte)
            );
            assert_eq!(caret.bottom(), marked.bounds.bottom());
            assert_eq!(caret.size.width, px(0.));
            let suffix = marked.range_bounds(index..text.encode_utf16().count());
            assert_eq!(suffix.left(), caret.left());
            assert_eq!(suffix.right(), marked.bounds.left() + marked.line.width());
        }
    }
    for text in ["实", "现", "反", "馈", "日", "語", "한", "글", "😀"] {
        let run = Run {
            col: 0,
            cols: 2,
            text: text.into(),
            fg: DEFAULT_FG,
            bg: None,
            bold: false,
            italic: false,
        };
        let (line, offset) = shape_cell(&run, font.clone(), metrics, window);
        let target = metrics.width * run.cols as f32;
        assert!(
            (f32::from(line.width() + offset * 2. - target)).abs() < 0.1,
            "wide cell {text}"
        );
        assert!(
            line.width() <= target + px(0.1),
            "wide glyph overflow {text}"
        );
        eprintln!(
            "wide-cell {text}: advance={:?}, two-columns={target:?}, inset={offset:?}",
            line.width()
        );
    }
    eprintln!("native IME geometry: overlay/ranges/UTF16 agree; measured cell={metrics:?}");
}
