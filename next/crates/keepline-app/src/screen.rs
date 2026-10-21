#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub col: u16,
    pub cols: u16,
    pub text: String,
    pub fg: u32,
    pub bg: Option<u32>,
    pub attrs: Attrs,
}

/// SGR attributes other than colors; a run breaks when any of them changes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Attrs {
    pub bold: bool,
    pub italic: bool,
    pub faint: bool,
    pub invisible: bool,
    pub strikethrough: bool,
    pub overline: bool,
    pub underline: Underline,
    /// SGR 58; `None` draws the underline in the text color.
    pub underline_color: Option<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Underline {
    #[default]
    None,
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

/// DECSCUSR shape as resolved by Ghostty (hollow when the terminal is unfocused).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CursorShape {
    #[default]
    Block,
    BlockHollow,
    Bar,
    Underline,
}

#[derive(Default)]
pub struct Screen {
    pub rows: Vec<Vec<Run>>,
    pub cursor: Option<(u16, u16)>,
    pub cursor_visible: bool,
    pub cursor_shape: CursorShape,
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
        attrs: Attrs,
    ) {
        if let Some(last) = self.row.last_mut()
            && last.fg == fg
            && last.bg == bg
            && last.attrs == attrs
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
            attrs,
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
        render::CursorVisualStyle,
        screen::CellWide,
        style::{StyleColor, Underline as SgrUnderline},
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
    let palette = snap.colors()?.palette;
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
            let mut attrs = Attrs::default();
            if cell.has_styling()? {
                let st = cell.style()?;
                if st.inverse {
                    let old = fg;
                    fg = bg.unwrap_or(DEFAULT_BG);
                    bg = Some(old);
                }
                attrs = Attrs {
                    bold: st.bold,
                    italic: st.italic,
                    faint: st.faint,
                    invisible: st.invisible,
                    strikethrough: st.strikethrough,
                    overline: st.overline,
                    underline: match st.underline {
                        SgrUnderline::Single => Underline::Single,
                        SgrUnderline::Double => Underline::Double,
                        SgrUnderline::Curly => Underline::Curly,
                        SgrUnderline::Dotted => Underline::Dotted,
                        SgrUnderline::Dashed => Underline::Dashed,
                        _ => Underline::None,
                    },
                    underline_color: match st.underline_color {
                        StyleColor::None => None,
                        StyleColor::Palette(i) => {
                            let c = palette[i.0 as usize];
                            Some(rgb(c.r, c.g, c.b))
                        }
                        StyleColor::Rgb(c) => Some(rgb(c.r, c.g, c.b)),
                    },
                };
            }
            let width = if matches!(wide, CellWide::Wide) { 2 } else { 1 };
            b.push(x, width, &text, fg, bg, attrs);
            x += 1;
        }
        y += 1;
    }
    out.rows.truncate(y);
    let cur = snap.cursor()?;
    out.cursor = cur.viewport.map(|v| (v.x, v.y));
    out.cursor_visible = cur.visible;
    out.cursor_shape = match cur.visual_style {
        CursorVisualStyle::Bar => CursorShape::Bar,
        CursorVisualStyle::Underline => CursorShape::Underline,
        CursorVisualStyle::BlockHollow => CursorShape::BlockHollow,
        _ => CursorShape::Block,
    };
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
    /// Offsets from the cell top, matching where `ShapedLine::paint` puts the baseline.
    pub baseline: Pixels,
    pub x_height: Pixels,
    /// Decoration and bar-cursor thickness.
    pub stroke: Pixels,
}
impl CellMetrics {
    pub fn measure(font: &Font, window: &Window) -> Option<Self> {
        let ts = window.text_system();
        let id = ts.resolve_font(font);
        let width = ts.advance(id, px(FONT_SIZE), 'm').ok()?.width;
        let height = px(FONT_SIZE * crate::theme::Theme::TERMINAL_LINE_HEIGHT);
        (width > px(0.)).then_some(Self {
            width,
            height,
            baseline: ts.baseline_offset(id, px(FONT_SIZE), height),
            x_height: ts.x_height(id, px(FONT_SIZE)),
            stroke: px((FONT_SIZE / 14.).max(1.)),
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
        color: text_color(run),
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
            if run.attrs.invisible {
                continue;
            }
            paint_decorations(run, point(ox, oy), metrics, window);
            if run.text.trim().is_empty() {
                continue;
            }
            let mut font = f.clone();
            if run.attrs.bold {
                font.weight = FontWeight::BOLD;
            }
            if run.attrs.italic {
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
        let cell = Bounds::new(
            point(
                bounds.origin.x + cw * x as f32,
                bounds.origin.y + lh * y as f32,
            ),
            size(cw, lh),
        );
        let color = hsla(DEFAULT_FG).opacity(0.6);
        let t = metrics.stroke * 2.;
        match sc.cursor_shape {
            CursorShape::Block => window.paint_quad(fill(cell, color)),
            CursorShape::Bar => window.paint_quad(fill(
                Bounds::new(cell.origin, size(t, lh)),
                color,
            )),
            CursorShape::Underline => window.paint_quad(fill(
                Bounds::new(point(cell.left(), cell.bottom() - t), size(cw, t)),
                color,
            )),
            CursorShape::BlockHollow => window.paint_quad(
                gpui::outline(cell, color, gpui::BorderStyle::Solid)
                    .border_widths(metrics.stroke),
            ),
        }
    }
}

fn text_color(run: &Run) -> Hsla {
    // Ghostty's faint-opacity default.
    hsla(run.fg).opacity(if run.attrs.faint { 0.5 } else { 1. })
}

/// Underline, strikethrough and overline drawn on the cell grid, like Ghostty's sprites,
/// so adjacent runs join and every SGR 4 variant has its own shape.
fn paint_decorations(run: &Run, origin: gpui::Point<Pixels>, m: CellMetrics, window: &mut Window) {
    let a = run.attrs;
    let w = m.width * run.cols as f32;
    let t = m.stroke;
    let color = text_color(run);
    let mut line = |y: Pixels, x: Pixels, len: Pixels, color: Hsla| {
        window.paint_quad(fill(
            Bounds::new(point(origin.x + x, origin.y + y), size(len, t)),
            color,
        ));
    };
    if a.underline != Underline::None {
        let color = a
            .underline_color
            .map(|c| hsla(c).opacity(color.a))
            .unwrap_or(color);
        // Halfway between the baseline and the cell bottom, clear of descenders.
        let y = (m.baseline + (m.height - m.baseline) / 2. - t / 2.).min(m.height - t);
        match a.underline {
            Underline::Single => line(y, px(0.), w, color),
            Underline::Double => {
                let y = (y - t).max(m.baseline);
                line(y, px(0.), w, color);
                line(y + t * 2., px(0.), w, color);
            }
            Underline::Dotted | Underline::Dashed => {
                let (on, period) = if a.underline == Underline::Dotted {
                    (t, t * 2.)
                } else {
                    (m.width / 2., m.width)
                };
                let mut x = px(0.);
                while x < w {
                    line(y, x, on.min(w - x), color);
                    x += period;
                }
            }
            Underline::Curly => {
                let amp = (m.height - y - t).max(t) / 2.;
                let steps = (f32::from(w) / f32::from(t)).ceil() as usize;
                for i in 0..steps {
                    let x = t * i as f32;
                    let phase = f32::from(x) / f32::from(m.width) * std::f32::consts::TAU;
                    line(y + amp * phase.sin(), x, t.min(w - x), color);
                }
            }
            Underline::None => {}
        }
    }
    if a.strikethrough {
        line(m.baseline - m.x_height / 2. - t / 2., px(0.), w, color);
    }
    if a.overline {
        line(px(0.), px(0.), w, color);
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

    /// Per-face metrics read straight from the font file (no GPUI window in unit tests).
    struct FaceMetrics {
        units_per_em: f32,
        line_height: f32,
        advance_m: f32,
    }
    fn be16(d: &[u8], o: usize) -> u16 {
        u16::from_be_bytes([d[o], d[o + 1]])
    }
    fn face_metrics(d: &[u8]) -> FaceMetrics {
        // First face of a collection (Menlo.ttc face 0 is Regular).
        let face = if &d[..4] == b"ttcf" {
            u32::from_be_bytes(d[12..16].try_into().unwrap()) as usize
        } else {
            0
        };
        let table = |tag: &[u8; 4]| {
            (0..be16(d, face + 4) as usize)
                .map(|i| face + 12 + 16 * i)
                .find(|&r| &d[r..r + 4] == tag)
                .map(|r| u32::from_be_bytes(d[r + 8..r + 12].try_into().unwrap()) as usize)
                .expect("font table")
        };
        let (head, hhea, os2, cmap, hmtx) = (
            table(b"head"),
            table(b"hhea"),
            table(b"OS/2"),
            table(b"cmap"),
            table(b"hmtx"),
        );
        let i16_at = |o| be16(d, o) as i16 as f32;
        // Ghostty face/coretext.zig: typo metrics if USE_TYPO_METRICS, else hhea, else typo, else win.
        let (hhea_asc, hhea_desc, hhea_gap) = (i16_at(hhea + 4), i16_at(hhea + 6), i16_at(hhea + 8));
        let (typo_asc, typo_desc, typo_gap) = (i16_at(os2 + 68), i16_at(os2 + 70), i16_at(os2 + 72));
        let line_height = if be16(d, os2 + 62) & 0x80 != 0 {
            typo_asc - typo_desc + typo_gap
        } else if hhea_asc != 0. || hhea_desc != 0. {
            hhea_asc - hhea_desc + hhea_gap
        } else if typo_asc != 0. || typo_desc != 0. {
            typo_asc - typo_desc + typo_gap
        } else {
            be16(d, os2 + 74) as f32 + be16(d, os2 + 76) as f32
        };
        // cmap format 4 lookup of 'm', then its hmtx advance (what CellMetrics::measure reads).
        let sub = (0..be16(d, cmap + 2) as usize)
            .map(|i| cmap + 4 + 8 * i)
            // Unicode platform (Menlo ships only 0/3) or Windows Unicode BMP.
            .filter(|&r| matches!((be16(d, r), be16(d, r + 2)), (0, _) | (3, 1)))
            .map(|r| cmap + u32::from_be_bytes(d[r + 4..r + 8].try_into().unwrap()) as usize)
            .find(|&s| be16(d, s) == 4)
            .expect("cmap format 4");
        let segs = be16(d, sub + 6) as usize;
        let ch = 'm' as u16;
        let seg = (0..segs / 2)
            .find(|&i| be16(d, sub + 14 + 2 * i) >= ch)
            .unwrap();
        let start = be16(d, sub + 16 + segs + 2 * seg);
        assert!(start <= ch, "'m' mapped");
        let delta = be16(d, sub + 16 + 2 * segs + 2 * seg);
        let range_at = sub + 16 + 3 * segs + 2 * seg;
        let range = be16(d, range_at) as usize;
        let glyph = if range == 0 {
            ch.wrapping_add(delta)
        } else {
            be16(d, range_at + range + 2 * (ch - start) as usize).wrapping_add(delta)
        } as usize;
        let metrics = be16(d, hhea + 34) as usize;
        FaceMetrics {
            units_per_em: be16(d, head + 18) as f32,
            line_height,
            advance_m: be16(d, hmtx + 4 * glyph.min(metrics - 1)) as f32,
        }
    }

    /// Ghostty sizes the cell from the face (font/Metrics.zig): height = round(ascent - descent
    /// + line_gap), width = round(advance), in backing pixels. Keepline multiplies the font size
    /// by TERMINAL_LINE_HEIGHT instead, so the gap depends on the font. 1.6 stretched the Codex
    /// logo ~37% while every older check (internal consistency only) still passed.
    #[test]
    fn cell_aspect_ratio_stays_within_five_percent_of_ghostty_for_the_resolved_font() {
        // Mirror Theme::mono: JetBrains Mono when installed, else Menlo. SF Mono is a hidden
        // system face that GPUI does not list, so stock macOS resolves Menlo (measured by
        // examples/terminal_reference).
        let home = std::env::var("HOME").unwrap_or_default();
        let candidates = [
            format!("{home}/Library/Fonts/JetBrainsMono-Regular.ttf"),
            "/Library/Fonts/JetBrainsMono-Regular.ttf".into(),
            "/System/Library/Fonts/Menlo.ttc".into(),
        ];
        let Some(path) = candidates.iter().find(|p| std::path::Path::new(p).exists()) else {
            eprintln!("no candidate monospace font on this system; skipped");
            return;
        };
        let face = face_metrics(&std::fs::read(path).unwrap());
        let backing = FONT_SIZE * 2.; // Retina: Ghostty rasterizes at points * scale.
        let ghostty_w = (face.advance_m / face.units_per_em * backing).round();
        let ghostty_h = (face.line_height / face.units_per_em * backing).round();
        if path.ends_with("Menlo.ttc") {
            // Anchors the parser to the Ghostty 1.3.1 capture (eval/terminal-reference).
            assert_eq!((ghostty_w, ghostty_h), (15., 29.), "Ghostty Menlo 12.5pt @2x cell");
        }
        let ghostty = ghostty_h / ghostty_w;
        let keepline = |line_height: f32| {
            (FONT_SIZE * line_height) / (face.advance_m / face.units_per_em * FONT_SIZE)
        };
        let delta = keepline(crate::theme::Theme::TERMINAL_LINE_HEIGHT) / ghostty - 1.;
        assert!(
            delta.abs() <= 0.05,
            "{path}: Keepline cell h/w {:.3} vs Ghostty {ghostty:.3} ({:+.1}%)",
            keepline(crate::theme::Theme::TERMINAL_LINE_HEIGHT),
            delta * 100.
        );
        assert!(
            keepline(1.6) / ghostty - 1. > 0.05,
            "the check must catch the 1.6 regression"
        );
    }

    #[test]
    fn sgr_decorations_and_faint_survive_capture() {
        let mut vt = Vt::new(80, 24).unwrap();
        vt.feed(b"\x1b[2ma\x1b[0m\x1b[4:3;58;5;1mb\x1b[0m\x1b[9;53mc\x1b[0m\x1b[8md\x1b[21me");
        let screen = capture(&vt).unwrap();
        let attrs: Vec<Attrs> = screen.rows[0].iter().take(5).map(|r| r.attrs).collect();
        assert!(attrs[0].faint);
        assert_eq!(attrs[1].underline, Underline::Curly);
        assert!(attrs[1].underline_color.is_some());
        assert!(attrs[2].strikethrough && attrs[2].overline);
        assert!(attrs[3].invisible);
        assert_eq!(attrs[4].underline, Underline::Double);
    }

    #[test]
    fn decscusr_shape_reaches_the_painter() {
        let mut vt = Vt::new(80, 24).unwrap();
        for (seq, shape) in [
            (&b"\x1b[6 q"[..], CursorShape::Bar),
            (b"\x1b[4 q", CursorShape::Underline),
            (b"\x1b[2 q", CursorShape::Block),
        ] {
            vt.feed(seq);
            assert_eq!(capture(&vt).unwrap().cursor_shape, shape);
        }
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
            attrs: Attrs::default(),
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
            attrs: Attrs::default(),
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
