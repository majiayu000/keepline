#[path = "sprites.rs"]
mod sprites;
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
    pub links: Vec<Link>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub row: u16,
    pub col: u16,
    pub cols: u16,
    pub uri: String,
}
impl Screen {
    pub fn link_at(&self, row: u16, col: u16) -> Option<&Link> {
        self.links
            .iter()
            .find(|l| l.row == row && col >= l.col && col < l.col + l.cols)
    }
}
/// Only explicitly clicked OSC 8 links reach the OS. No OSC callback opens URLs.
pub fn allowed_link(uri: &str) -> bool {
    if uri.len() > 8192 || uri.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return false;
    }
    uri.strip_prefix("https://")
        .or_else(|| uri.strip_prefix("http://"))
        .is_some_and(|s| !s.is_empty() && !s.starts_with('/'))
        || uri.strip_prefix("file:///").is_some_and(|s| !s.is_empty())
}
pub fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) << 16 | (g as u32) << 8 | b as u32
}

/// Builds runs cell by cell; a run breaks when the style changes.
pub struct RowBuilder<'a> {
    row: &'a mut Vec<Run>,
    ascii: bool,
}

impl<'a> RowBuilder<'a> {
    pub fn new(row: &'a mut Vec<Run>) -> Self {
        row.clear();
        Self { row, ascii: false }
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
            && self.ascii
        {
            last.text.push_str(text);
            last.cols += width;
            return;
        }
        // Wide / non-ASCII cells get their own run so each is placed on the cell grid.
        self.ascii = text.is_ascii();
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
        render::CursorVisualStyle,
        render::{CellIterator, RenderState, RowIterator},
        screen::CellWide,
        style::{StyleColor, Underline as SgrUnderline},
    },
};
/// Retain Ghostty's dirty-row state and only rebuild changed viewport rows.
/// One instance belongs to one Pane/VT connection, never shared between terminals.
pub struct Capture {
    state: RenderState<'static>,
    rows: RowIterator<'static>,
    cells: CellIterator<'static>,
    dimensions: (u16, u16),
    colors: Option<libghostty_vt::render::Colors>,
    defaults: Option<(u32, u32)>,
}
impl Capture {
    pub fn new() -> Result<Self, libghostty_vt::Error> {
        Ok(Self {
            state: RenderState::new()?,
            rows: RowIterator::new()?,
            cells: CellIterator::new()?,
            dimensions: (0, 0),
            colors: None,
            defaults: None,
        })
    }
    pub fn update(&mut self, vt: &Vt, out: &mut Screen) -> Result<usize, libghostty_vt::Error> {
        let snap = self.state.update(vt.terminal())?;
        let dimensions = (snap.cols()?, snap.rows()?);
        let colors = snap.colors()?;
        let mut full = self.colors.as_ref() != Some(&colors)
            || self.dimensions != dimensions
            || snap.dirty()? == libghostty_vt::render::Dirty::Full;
        self.dimensions = dimensions;
        let mut changed = 0;
        // Only the unset defaults come from the UI theme; OSC and explicit ANSI colors survive.
        let def_fg = vt
            .terminal()
            .fg_color()?
            .map(|c| rgb(c.r, c.g, c.b))
            .unwrap_or(DEFAULT_FG);
        let def_bg = vt
            .terminal()
            .bg_color()?
            .map(|c| rgb(c.r, c.g, c.b))
            .unwrap_or(DEFAULT_BG);
        full |= self.defaults != Some((def_fg, def_bg));
        self.defaults = Some((def_fg, def_bg));
        self.colors = Some(colors.clone());
        let palette = colors.palette;
        let mut rows = self.rows.update(&snap)?;
        let mut y = 0usize;
        let mut text = String::new();
        while let Some(row) = rows.next() {
            if out.rows.len() <= y {
                out.rows.push(Vec::new());
            }
            if !full && !row.dirty()? {
                y += 1;
                continue;
            }
            changed += 1;
            out.links.retain(|link| link.row != y as u16);
            let mut b = RowBuilder::new(&mut out.rows[y]);
            let mut cells = self.cells.update(row)?;
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
                        fg = bg.unwrap_or(def_bg);
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
                if raw.has_hyperlink()? {
                    let grid = vt
                        .terminal()
                        .grid_ref(libghostty_vt::terminal::Point::Viewport(
                            libghostty_vt::terminal::PointCoordinate { x, y: y as u32 },
                        ))?;
                    let mut uri = vec![0; 8192];
                    if let Ok(n) = grid.hyperlink_uri(&mut uri)
                        && let Ok(uri) = std::str::from_utf8(&uri[..n])
                        && allowed_link(uri)
                    {
                        out.links.push(Link {
                            row: y as u16,
                            col: x,
                            cols: width,
                            uri: uri.into(),
                        });
                    }
                }
                b.push(x, width, &text, fg, bg, attrs);
                x += 1;
            }
            y += 1;
        }
        out.rows.truncate(y);
        out.links.retain(|link| (link.row as usize) < y);
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
        Ok(changed)
    }
}
/// Standalone captures for fixtures; production keeps a Capture per Pane.
#[cfg(any(test, feature = "visual-test"))]
pub fn capture(vt: &Vt) -> Result<Screen, libghostty_vt::Error> {
    let mut out = Screen::default();
    Capture::new()?.update(vt, &mut out)?;
    Ok(out)
}

use gpui::{
    App, Bounds, Font, FontStyle, FontWeight, Hsla, Pixels, Rgba, ShapedLine, SharedString,
    TextAlign, TextRun, UnderlineStyle, Window, fill, point, px, size,
};
pub const FONT_SIZE: f32 = crate::theme::Theme::MONO_SIZE;

/// One measurement shared by terminal paint, resize, preedit and IME geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellMetrics {
    pub width: Pixels,
    pub height: Pixels,
    /// Offsets from the cell top, matching where `ShapedLine::paint` puts the baseline.
    pub baseline: Pixels,
    pub x_height: Pixels,
    /// Decoration and bar-cursor thickness.
    pub stroke: Pixels,
    pub ic_width: Pixels,
}
impl CellMetrics {
    pub fn measure(font: &Font, window: &Window) -> Option<Self> {
        let ts = window.text_system();
        let id = ts.resolve_font(font);
        let width = ts.advance(id, px(FONT_SIZE), 'm').ok()?.width;
        let height = px(FONT_SIZE * crate::theme::Theme::TERMINAL_LINE_HEIGHT);
        let ic_width = ts
            .advance(id, px(FONT_SIZE), '水')
            .ok()
            .filter(|s| s.width > px(0.))
            .map(|s| s.width)
            .unwrap_or_else(|| {
                let mut top = px(0.);
                let mut bottom = px(0.);
                for ch in ' '..='~' {
                    if let Ok(b) = ts.typographic_bounds(id, px(FONT_SIZE), ch) {
                        top = top.min(b.top());
                        bottom = bottom.max(b.bottom());
                    }
                }
                (bottom - top).min(width * 2.)
            });
        (width > px(0.)).then_some(Self {
            width,
            height,
            baseline: ts.baseline_offset(id, px(FONT_SIZE), height),
            x_height: ts.x_height(id, px(FONT_SIZE)),
            stroke: px((FONT_SIZE / 14.).max(1.)),
            ic_width,
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
    // Cell backgrounds/decorations retain full width; trailing ASCII blanks have no ink.
    let text = run.text.trim_end_matches(' ');
    let tr = TextRun {
        len: text.len(),
        font,
        color: text_color(run),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let ts = window.text_system();
    let mut line = ts.shape_line(
        SharedString::from(text.to_owned()),
        px(FONT_SIZE),
        std::slice::from_ref(&tr),
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
        let target = metrics.width * run.cols as f32;
        let cjk = run.text.chars().any(|c| matches!(c as u32, 0x2e80..=0xa4cf | 0xac00..=0xd7ff | 0xf900..=0xfaff | 0xff01..=0xff60 | 0x20000..=0x323af));
        let scale = if cjk {
            let ic = metrics.ic_width;
            let fallback_ic = line
                .runs
                .first()
                .and_then(|r| ts.advance(r.font_id, px(FONT_SIZE), '水').ok())
                .map(|s| s.width)
                .unwrap_or(line.width());
            (ic / fallback_ic).min(metrics.height / px(FONT_SIZE))
        } else {
            (target / line.width()).min(metrics.height / px(FONT_SIZE))
        };
        line = ts.shape_line(
            SharedString::from(text.to_owned()),
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
// Printable ASCII has no shaping context when terminal ligatures are disabled.
// Shape this bounded alphabet once per face/style, then reuse the exact native
// glyph IDs. Non-ASCII and fonts whose advances aren't one cell retain shaping.
struct AsciiGlyphs {
    glyphs: [(gpui::FontId, gpui::GlyphId); 95],
    baseline: Pixels,
}
impl AsciiGlyphs {
    fn new(font: Font, metrics: CellMetrics, window: &Window) -> Option<Self> {
        if !["liga", "calt"].iter().all(|tag| {
            font.features
                .tag_value_list()
                .iter()
                .any(|(name, value)| name == tag && *value == 0)
        }) {
            return None;
        }
        let text: String = (' '..='~').collect();
        let line = window.text_system().shape_line(
            text.into(),
            px(FONT_SIZE),
            &[TextRun {
                len: 95,
                font,
                color: hsla(DEFAULT_FG),
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
        );
        let mut glyphs = [None; 95];
        for run in &line.runs {
            for glyph in &run.glyphs {
                if glyph.index >= 95
                    || glyphs[glyph.index].is_some()
                    || glyph.is_emoji
                    || f32::from(glyph.position.x - metrics.width * glyph.index as f32).abs() > 0.02
                    || glyph.position.y != px(0.)
                {
                    return None;
                }
                glyphs[glyph.index] = Some((run.font_id, glyph.id));
            }
        }
        if glyphs.iter().any(Option::is_none)
            || f32::from(line.width() - metrics.width * 95.).abs() > 0.02
        {
            return None;
        }
        Some(Self {
            glyphs: glyphs.map(Option::unwrap),
            baseline: (metrics.height - line.ascent - line.descent) / 2. + line.ascent,
        })
    }
    fn paint(
        &self,
        text: &str,
        origin: gpui::Point<Pixels>,
        metrics: CellMetrics,
        color: Hsla,
        window: &mut Window,
    ) {
        for (col, byte) in text.bytes().enumerate() {
            if byte == b' ' {
                continue;
            }
            let (font, glyph) = self.glyphs[(byte - b' ') as usize];
            let _ = window.paint_glyph(
                point(
                    origin.x + metrics.width * col as f32,
                    origin.y + self.baseline,
                ),
                font,
                glyph,
                px(FONT_SIZE),
                color,
            );
        }
    }
}
fn printable_cells(run: &Run) -> bool {
    run.text.len() == run.cols as usize && run.text.bytes().all(|b| (b' '..=b'~').contains(&b))
}
/// Current viewport only: no history of strings from a long-running terminal.
#[derive(Default)]
pub struct LayoutCache {
    key: Option<(Font, CellMetrics)>,
    rows: Vec<Vec<(Run, ShapedLine, Pixels)>>,
    ascii: [Option<Option<AsciiGlyphs>>; 4],
}
impl LayoutCache {
    fn prepare(&mut self, font: &Font, metrics: CellMetrics, rows: usize) {
        if self.key.as_ref() != Some(&(font.clone(), metrics)) {
            self.rows.clear();
            self.ascii = Default::default();
            self.key = Some((font.clone(), metrics));
        }
        self.rows.resize_with(rows, Vec::new);
    }
}
// Render inputs remain explicit alongside GPUI's required Window/App contexts.
// A wrapper type solely to reduce argument count would hide their call-site ownership.
#[allow(clippy::too_many_arguments)]
pub fn paint_cached(
    sc: &Screen,
    f: &Font,
    bounds: Bounds<Pixels>,
    metrics: CellMetrics,
    show_cursor: bool,
    cache: &mut LayoutCache,
    window: &mut Window,
    cx: &mut App,
) {
    // One layer for the grid: its thousands of glyphs and cell backgrounds share a
    // draw order instead of each being inserted into GPUI's per-primitive bounds tree.
    // Inside the layer quads draw under sprites, as Ghostty's background/text passes do.
    window.paint_layer(bounds, |window| {
        paint_grid(sc, f, bounds, metrics, cache, window, cx)
    });
    // Cursor and later overlays stay outside the layer so they still draw over glyphs.
    let cw = metrics.width;
    let lh = metrics.height;
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
            CursorShape::Bar => {
                window.paint_quad(fill(Bounds::new(cell.origin, size(t, lh)), color))
            }
            CursorShape::Underline => window.paint_quad(fill(
                Bounds::new(point(cell.left(), cell.bottom() - t), size(cw, t)),
                color,
            )),
            CursorShape::BlockHollow => window.paint_quad(
                gpui::outline(cell, color, gpui::BorderStyle::Solid).border_widths(metrics.stroke),
            ),
        }
    }
}

fn paint_grid(
    sc: &Screen,
    f: &Font,
    bounds: Bounds<Pixels>,
    metrics: CellMetrics,
    cache: &mut LayoutCache,
    window: &mut Window,
    cx: &mut App,
) {
    window.paint_quad(fill(bounds, hsla(sc.background)));
    let cw = metrics.width;
    let lh = metrics.height;
    cache.prepare(f, metrics, sc.rows.len());
    for (y, row) in sc.rows.iter().enumerate() {
        let oy = bounds.origin.y + lh * y as f32;
        if oy > bounds.origin.y + bounds.size.height {
            break;
        }
        let cached = &mut cache.rows[y];
        let mut index = 0;
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
            if printable_cells(run) {
                let style = usize::from(run.attrs.bold) | (usize::from(run.attrs.italic) << 1);
                let glyphs = cache.ascii[style].get_or_insert_with(|| {
                    let mut font = f.clone();
                    if run.attrs.bold {
                        font.weight = FontWeight::BOLD;
                    }
                    if run.attrs.italic {
                        font.style = FontStyle::Italic;
                    }
                    AsciiGlyphs::new(font, metrics, window)
                });
                if let Some(glyphs) = glyphs {
                    glyphs.paint(&run.text, point(ox, oy), metrics, text_color(run), window);
                    continue;
                }
            }
            if run.text.trim().is_empty() {
                continue;
            }
            if run.text.chars().count() == 1
                && sprites::paint(
                    run.text.chars().next().unwrap(),
                    point(ox, oy),
                    cw * run.cols as f32,
                    lh,
                    metrics.stroke,
                    text_color(run),
                    window,
                )
            {
                continue;
            }
            if cached.get(index).is_none_or(|(old, _, _)| old != run) {
                let mut font = f.clone();
                if run.attrs.bold {
                    font.weight = FontWeight::BOLD;
                }
                if run.attrs.italic {
                    font.style = FontStyle::Italic;
                }
                let (line, offset) = shape_cell(run, font, metrics, window);
                let entry = (run.clone(), line, offset);
                if index < cached.len() {
                    cached[index] = entry;
                } else {
                    cached.push(entry);
                }
            }
            let (_, line, offset) = &cached[index];
            index += 1;
            let _ = line.paint(
                point(ox + *offset, oy),
                lh,
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
        cached.truncate(index);
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
    fn retained_capture_rebuilds_only_dirty_rows_and_clears_old_links() {
        let mut vt = Vt::new(40, 10).unwrap();
        vt.feed(b"\x1b[3;1H");
        let mut capture = Capture::new().unwrap();
        let mut screen = Screen::default();
        assert_eq!(capture.update(&vt, &mut screen).unwrap(), 10);
        assert_eq!(capture.update(&vt, &mut screen).unwrap(), 0);
        vt.feed(b"\x1b[3;1H\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\");
        assert_eq!(capture.update(&vt, &mut screen).unwrap(), 1);
        assert!(screen.link_at(2, 0).is_some());
        let unchanged = screen.rows[0].as_ptr();
        vt.feed(b"\x1b[3;1H\x1b[2Kplain");
        assert_eq!(capture.update(&vt, &mut screen).unwrap(), 1);
        assert!(screen.links.is_empty());
        assert_eq!(screen.rows[0].as_ptr(), unchanged);
        assert_eq!(capture.update(&vt, &mut screen).unwrap(), 0);
    }

    #[test]
    fn retained_capture_matches_full_capture_after_resize_scroll_palette_and_alternate_screen() {
        let mut vt = Vt::new(40, 5).unwrap();
        let mut reader = Capture::new().unwrap();
        let mut screen = Screen::default();
        for bytes in [
            b"one\r\ntwo\r\nthree".as_slice(),
            b"\x1b[?1049hALT",
            b"\x1b[?1049l",
            b"\x1b]10;#123456\x07",
            b"\x1b[31mred\x1b]4;1;#abcdef\x07",
            b"\r\n4\r\n5\r\n6\r\n7",
        ] {
            vt.feed(bytes);
            reader.update(&vt, &mut screen).unwrap();
            let full = capture(&vt).unwrap();
            assert_eq!(screen.rows, full.rows);
            assert_eq!(screen.cursor, full.cursor);
            assert_eq!(screen.links, full.links);
        }
        vt.resize(20, 3).unwrap();
        reader.update(&vt, &mut screen).unwrap();
        assert_eq!(screen.rows, capture(&vt).unwrap().rows);
        vt.scroll(libghostty_vt::terminal::ScrollViewport::Top);
        reader.update(&vt, &mut screen).unwrap();
        assert_eq!(screen.rows, capture(&vt).unwrap().rows);
    }

    #[test]
    fn osc_links_keep_cell_bounds_and_reject_executable_schemes() {
        let mut vt = Vt::new(40, 3).unwrap();
        vt.feed("\x1b]8;;https://example.com\x1b\\中文\x1b]8;;\x1b\\ \x1b]8;;javascript:alert(1)\x1b\\bad".as_bytes());
        let s = capture(&vt).unwrap();
        assert_eq!(s.link_at(0, 3).unwrap().uri, "https://example.com");
        assert!(s.link_at(0, 4).is_none());
        assert!(s.link_at(0, 5).is_none());
        for uri in [
            "javascript:alert(1)",
            "data:text/plain,a",
            "https://",
            "https://a\n--arg",
            "file:///",
        ] {
            assert!(!allowed_link(uri));
        }
        assert!(allowed_link("file:///tmp/test"));
    }
    #[test]
    fn emoji_cluster_capture_is_one_two_column_run() {
        let mut vt = Vt::new(40, 3).unwrap();
        vt.feed("👨‍👩‍👧‍👦🇨🇳X".as_bytes());
        let s = capture(&vt).unwrap();
        assert_eq!(s.rows[0][0].text, "👨‍👩‍👧‍👦");
        assert_eq!(s.rows[0][0].cols, 2);
        assert_eq!(s.rows[0][1].col, 2);
        assert_eq!(s.cursor, Some((5, 0)));
    }
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
        let (hhea_asc, hhea_desc, hhea_gap) =
            (i16_at(hhea + 4), i16_at(hhea + 6), i16_at(hhea + 8));
        let (typo_asc, typo_desc, typo_gap) =
            (i16_at(os2 + 68), i16_at(os2 + 70), i16_at(os2 + 72));
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
    ///   by TERMINAL_LINE_HEIGHT instead, so the gap depends on the font. 1.6 stretched the Codex
    ///   logo ~37% while every older check (internal consistency only) still passed.
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
            assert_eq!(
                (ghostty_w, ghostty_h),
                (15., 29.),
                "Ghostty Menlo 12.5pt @2x cell"
            );
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
    fn ascii_cache_never_treats_wide_or_combining_text_as_independent_cells() {
        for (text, cols, expected) in [
            ("hello 123", 9, true),
            ("你", 2, false),
            ("e\u{301}", 1, false),
            ("\t", 1, false),
            ("ab", 1, false),
        ] {
            let run = Run {
                col: 0,
                cols,
                text: text.into(),
                fg: DEFAULT_FG,
                bg: None,
                attrs: Attrs::default(),
            };
            assert_eq!(printable_cells(&run), expected);
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
    for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut face = font.clone();
        if bold {
            face.weight = FontWeight::BOLD;
        }
        if italic {
            face.style = FontStyle::Italic;
        }
        let cached =
            AsciiGlyphs::new(face.clone(), metrics, window).expect("native mono ASCII cache");
        for text in [
            "0123456789 row 01",
            "!=>->==fiWWW_[]{}~",
            "ASCII with spaces",
        ] {
            let run = Run {
                col: 0,
                cols: text.len() as u16,
                text: text.into(),
                fg: DEFAULT_FG,
                bg: None,
                attrs: Attrs::default(),
            };
            let (line, _) = shape_cell(&run, face.clone(), metrics, window);
            assert_eq!(
                cached.baseline,
                (metrics.height - line.ascent - line.descent) / 2. + line.ascent
            );
            for shaped in &line.runs {
                for g in &shaped.glyphs {
                    assert_eq!(
                        cached.glyphs[(text.as_bytes()[g.index] - b' ') as usize],
                        (shaped.font_id, g.id)
                    );
                    assert!(f32::from(g.position.x - metrics.width * g.index as f32).abs() < 0.02);
                }
            }
        }
    }
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
    for text in ["nihao", "n你😀hao", "e\u{301}", "👨‍👩‍👧‍👦🇨🇳👍🏽"] {
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
