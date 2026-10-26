pub use libghostty_vt;
use libghostty_vt::{
    Terminal,
    snapshot::Decoder,
    style::RgbColor,
    terminal::{
        ColorScheme, ConformanceLevel, DeviceAttributeFeature, DeviceAttributes, DeviceType,
        PrimaryDeviceAttributes, SecondaryDeviceAttributes, SizeReportSize,
        TertiaryDeviceAttributes,
    },
};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

/// Default colors of every terminal. The display paints unset cells with them, and
/// programs read the same values back through OSC 10/11/12 (Codex tints its input
/// box from the background, so an unanswered query leaves it flat).
pub const DEFAULT_FG: u32 = 0xe4e2df;
pub const DEFAULT_BG: u32 = 0x1c1c1e;
/// Reported for XTVERSION (CSI > q), like "ghostty 1.2.3".
pub const XTVERSION: &str = concat!("keepline ", env!("CARGO_PKG_VERSION"));
/// Matches the TERM the daemon gives programs; reported for XTGETTCAP TN.
pub const TERMINFO: &str = "xterm-256color";
// DA1 "CSI ? 62 ; 22 c" and DA2 "CSI > 1 ; 10 ; 0 c": what Ghostty and xterm-like
// terminals answer, so capability probes finish instead of waiting for a timeout.
const DEVICE_ATTRIBUTES: DeviceAttributes = DeviceAttributes {
    primary: PrimaryDeviceAttributes::new(
        ConformanceLevel::LEVEL_2,
        &[DeviceAttributeFeature::ANSI_COLOR],
    ),
    secondary: SecondaryDeviceAttributes {
        device_type: DeviceType::VT220,
        firmware_version: 10,
        rom_cartridge: 0,
    },
    tertiary: TertiaryDeviceAttributes { unit_id: 0 },
};

fn rgb(value: u32) -> RgbColor {
    RgbColor {
        r: (value >> 16) as u8,
        g: (value >> 8) as u8,
        b: value as u8,
    }
}

/// Bytes that clear scrollback and the rows above the cursor, keeping the cursor row
/// (usually the prompt) at the top: what Cmd+K does in Ghostty and Terminal.app. Empty
/// in the alternate screen, where a full-screen program owns every row.
/// The leading CAN ends any sequence the program left half-written, so the clear
/// never fuses with its output.
pub fn clear_history(t: &Terminal) -> io::Result<Vec<u8>> {
    use libghostty_vt::screen::Screen;
    if t.active_screen().map_err(io::Error::other)? == Screen::Alternate {
        return Ok(Vec::new());
    }
    let y = t.cursor_y().map_err(io::Error::other)?;
    let mut bytes = b"\x18".to_vec();
    if y > 0 {
        bytes.extend(format!("\x1b[{y}S\x1b[{y}A").bytes());
    }
    bytes.extend(b"\x1b[3J");
    Ok(bytes)
}

/// One cell's size in device pixels, as the display last measured it; 0 until known.
/// Shared with the size-report callback, which runs inside `feed`.
#[derive(Clone, Default)]
pub struct CellPixels(Arc<AtomicU64>);
impl CellPixels {
    pub fn set(&self, width: u32, height: u32) {
        self.0.store(
            u64::from(width) << 32 | u64::from(height),
            Ordering::Relaxed,
        );
    }
    pub fn get(&self) -> (u32, u32) {
        let v = self.0.load(Ordering::Relaxed);
        ((v >> 32) as u32, v as u32)
    }
}

/// A terminal may move between threads, but every access must be exclusive.
/// Ghostty callbacks capture only Send values; no platform/UI handles live here.
pub struct Vt(Terminal<'static, 'static>);
// SAFETY: owned native state, no concurrent calls; callbacks are installed only by
// the daemon with Send captures. Vt is not Sync and is held behind a Mutex.
unsafe impl Send for Vt {}

impl Vt {
    pub fn terminal(&self) -> &Terminal<'static, 'static> {
        &self.0
    }
    pub fn scroll(&mut self, scroll: libghostty_vt::terminal::ScrollViewport) {
        self.0.scroll_viewport(scroll);
    }
    /// Explicit user search only: reads the bounded VT history, never the transcript directory.
    pub fn search(&self, query: &str) -> io::Result<Vec<(usize, String)>> {
        if query.is_empty() {
            return Ok(vec![]);
        }
        let mut matches = Vec::new();
        for y in 0..self.0.total_rows().map_err(io::Error::other)? {
            let line = self.row(y)?;
            if line.contains(query) {
                matches.push((y, line.trim_end().to_owned()));
            }
        }
        Ok(matches)
    }
    /// The last `rows` rows of history plus screen, one line per row.
    pub fn tail_text(&self, rows: usize) -> io::Result<String> {
        let total = self.0.total_rows().map_err(io::Error::other)?;
        let mut text = String::new();
        for y in total.saturating_sub(rows)..total {
            text.push_str(self.row(y)?.trim_end());
            text.push('\n');
        }
        Ok(text)
    }
    fn row(&self, y: usize) -> io::Result<String> {
        use libghostty_vt::{
            screen::CellWide,
            terminal::{Point, PointCoordinate},
        };
        let cols = self.0.cols().map_err(io::Error::other)?;
        let mut line = String::new();
        for x in 0..cols {
            let cell = self
                .0
                .grid_ref(Point::Screen(PointCoordinate { x, y: y as u32 }))
                .map_err(io::Error::other)?;
            if matches!(
                cell.cell()
                    .and_then(|c| c.wide())
                    .map_err(io::Error::other)?,
                CellWide::SpacerHead | CellWide::SpacerTail
            ) {
                continue;
            }
            let mut chars = vec!['\0'; 32];
            let len = match cell.graphemes(&mut chars) {
                Ok(n) => n,
                Err(libghostty_vt::Error::OutOfSpace { required }) => {
                    chars.resize(required, '\0');
                    cell.graphemes(&mut chars).map_err(io::Error::other)?
                }
                Err(e) => return Err(io::Error::other(e)),
            };
            if len == 0 {
                line.push(' ');
            } else {
                line.extend(chars[..len].iter());
            }
        }
        Ok(line)
    }
    pub fn on_reply(&mut self, mut callback: impl FnMut(&[u8]) + Send + 'static) -> io::Result<()> {
        self.0
            .on_pty_write(move |_, bytes| callback(bytes))
            .map_err(io::Error::other)?;
        Ok(())
    }
    /// BEL (0x07) from the program; a BEL ending an OSC string is not a bell.
    pub fn on_bell(&mut self, mut callback: impl FnMut() + Send + 'static) -> io::Result<()> {
        self.0
            .on_bell(move |_| callback())
            .map_err(io::Error::other)?;
        Ok(())
    }
    /// The title the program set with OSC 0/2, if any.
    pub fn title(&self) -> Option<String> {
        self.0
            .title()
            .ok()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_owned)
    }
    /// Answer the queries programs send every terminal: device attributes, XTVERSION,
    /// window/cell size and the dark color scheme. Colors are answered from the
    /// defaults set in `new`. Only the daemon installs this; display copies stay silent.
    pub fn answer_queries(&mut self, cell: CellPixels) -> io::Result<()> {
        self.0
            .on_device_attributes(|_| Some(DEVICE_ATTRIBUTES))
            .and_then(|t| t.on_xtversion(|_| Some(XTVERSION)))
            .and_then(|t| t.on_color_scheme(|_| Some(ColorScheme::Dark)))
            .and_then(|t| {
                t.on_size(move |t| {
                    let (cell_width, cell_height) = cell.get();
                    Some(SizeReportSize {
                        rows: t.rows().ok()?,
                        columns: t.cols().ok()?,
                        cell_width,
                        cell_height,
                    })
                })
            })
            .and_then(|t| t.set_terminfo_name(TERMINFO))
            .map_err(io::Error::other)?;
        Ok(())
    }
    pub fn new(cols: u16, rows: u16) -> io::Result<Self> {
        let mut terminal = Terminal::new(cols, rows).map_err(io::Error::other)?;
        terminal
            .set_scrollback_max_lines(Some(5_000))
            .map_err(io::Error::other)?;
        terminal
            .set_scrollback_max_bytes(Some(16 * 1024 * 1024))
            .map_err(io::Error::other)?;
        terminal
            .set_continuation_max_bytes(1024 * 1024)
            .map_err(io::Error::other)?;
        terminal
            .set_mode(libghostty_vt::terminal::Mode::GRAPHEME_CLUSTER, true)
            .map_err(io::Error::other)?;
        terminal
            .set_default_fg_color(Some(rgb(DEFAULT_FG)))
            .and_then(|t| t.set_default_bg_color(Some(rgb(DEFAULT_BG))))
            .and_then(|t| t.set_default_cursor_color(Some(rgb(DEFAULT_FG))))
            .map_err(io::Error::other)?;
        Ok(Self(terminal))
    }
    pub fn snapshot(&mut self) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        self.0
            .encode_snapshot(&mut bytes)
            .map_err(io::Error::other)?;
        Ok(bytes)
    }
    pub fn restore(bytes: &[u8]) -> io::Result<Self> {
        Decoder::new_buf(bytes)
            .and_then(|d| d.decode())
            .map(Self)
            .map_err(io::Error::other)
    }
    pub fn feed(&mut self, bytes: &[u8]) {
        self.0.vt_write(bytes);
    }
    pub fn resize(&mut self, cols: u16, rows: u16) -> io::Result<()> {
        self.0.resize(cols, rows, 0, 0).map_err(io::Error::other)
    }
}

/// A program's window title as a name: Claude Code and Codex lead theirs with a spinner
/// or activity glyph that changes many times a second ("✳ fix login" -> "fix login").
pub fn shown_title(raw: &str) -> Option<String> {
    let title = raw.trim();
    let mut chars = title.chars();
    let first = chars.next()?;
    let rest = chars.as_str();
    let glyph = matches!(first, '\u{2800}'..='\u{28ff}') || "·✢✳✶✻✽◐◓◑◒".contains(first);
    let title = if glyph && rest.chars().next().is_none_or(char::is_whitespace) {
        rest.trim()
    } else {
        title
    };
    (!title.is_empty()).then(|| title.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_preserves_shell_bracketed_paste_readiness() {
        let mut vt = Vt::new(80, 24).unwrap();
        vt.feed(b"\x1b[?2004hprompt> ");
        let restored = Vt::restore(&vt.snapshot().unwrap()).unwrap();
        assert!(
            restored
                .terminal()
                .mode(libghostty_vt::terminal::Mode::BRACKETED_PASTE)
                .unwrap()
        );
    }
    #[test]
    fn emoji_clusters_use_two_cells_and_survive_snapshot_boundaries() {
        for text in ["👨‍👩‍👧‍👦", "🇨🇳", "👍🏽", "1️⃣", "🏳️‍🌈", "🧑‍💻"]
        {
            let mut vt = Vt::new(40, 3).unwrap();
            for ch in text.chars() {
                vt.feed(ch.to_string().as_bytes());
                vt = Vt::restore(&vt.snapshot().unwrap()).unwrap();
            }
            assert_eq!(vt.terminal().cursor_x().unwrap(), 2, "{text}");
            assert!(
                vt.terminal()
                    .mode(libghostty_vt::terminal::Mode::GRAPHEME_CLUSTER)
                    .unwrap()
            );
            vt.feed(b"X");
            assert!(vt.search(&format!("{text}X")).unwrap().len() == 1);
        }
    }
    #[test]
    fn snapshot_preserves_incomplete_utf8_and_escape_sequence() {
        for (prefix, tail) in [
            (&b"before \xe4\xb8"[..], &b"\xad after"[..]),
            (&b"\x1b[?1049h\x1b[31"[..], &b"mRED\x1b[0m"[..]),
        ] {
            let mut source = Vt::new(80, 24).unwrap();
            source.feed(prefix);
            let mut restored = Vt::restore(&source.snapshot().unwrap()).unwrap();
            source.feed(tail);
            restored.feed(tail);
            assert_eq!(source.snapshot().unwrap(), restored.snapshot().unwrap());
        }
    }
    #[test]
    fn search_finds_scrollback_and_wide_graphemes_without_spacer_gaps() {
        let mut vt = Vt::new(40, 3).unwrap();
        vt.feed("old 实现反馈 e\u{301}\r\nsecond\r\nthird\r\nfourth\r\nfifth".as_bytes());
        let found = vt.search("实现反馈").unwrap();
        assert_eq!(found.len(), 1);
        assert!(found[0].1.contains("实现反馈 e\u{301}"));
        assert!(vt.search("absent").unwrap().is_empty());
        assert!(vt.search("").unwrap().is_empty());
    }
    fn replies_to(vt: &mut Vt, query: &[u8]) -> String {
        let got = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = got.clone();
        vt.on_reply(move |b| sink.lock().unwrap().extend_from_slice(b))
            .unwrap();
        vt.feed(query);
        String::from_utf8(got.lock().unwrap().clone()).unwrap()
    }
    #[test]
    fn answers_the_queries_every_terminal_answers() {
        let cell = CellPixels::default();
        let mut vt = Vt::new(80, 24).unwrap();
        vt.answer_queries(cell.clone()).unwrap();
        cell.set(16, 34);
        for (query, reply) in [
            (&b"\x1b]10;?\x1b\\"[..], "\x1b]10;rgb:e4e4/e2e2/dfdf\x1b\\"),
            (b"\x1b]11;?\x07", "\x1b]11;rgb:1c1c/1c1c/1e1e\x07"),
            (b"\x1b]12;?\x07", "\x1b]12;rgb:e4e4/e2e2/dfdf\x07"),
            (b"\x1b]4;1;?\x07", "\x1b]4;1;rgb:"),
            (b"\x1b[c", "\x1b[?62;22c"),
            (b"\x1b[>c", "\x1b[>1;10;0c"),
            (b"\x1b[>q", "\x1bP>|keepline "),
            (b"\x1b[18t", "\x1b[8;24;80t"),
            (b"\x1b[16t", "\x1b[6;34;16t"),
            (b"\x1b[14t", "\x1b[4;816;1280t"),
            (b"\x1b[?996n", "\x1b[?997;1n"),
            (
                b"\x1bP+q544e\x1b\\",
                "\x1bP1+r544E=787465726D2D323536636F6C6F72\x1b\\",
            ),
        ] {
            let got = replies_to(&mut vt, query);
            assert!(got.starts_with(reply), "{query:?}: got {got:?}");
        }
        // A program's own OSC 11 wins over the default, as in any terminal.
        vt.feed(b"\x1b]11;#ffffff\x07");
        assert_eq!(
            replies_to(&mut vt, b"\x1b]11;?\x07"),
            "\x1b]11;rgb:ffff/ffff/ffff\x07"
        );
    }
    #[test]
    fn clear_history_keeps_the_prompt_row_at_the_top() {
        let mut vt = Vt::new(20, 4).unwrap();
        for i in 0..10 {
            vt.feed(format!("line{i}\r\n").as_bytes());
        }
        vt.feed(b"\x1b[31mhalf");
        vt.feed(b"\x1b[3"); // the program's output stops mid-sequence
        vt.feed(b"$ ");
        let clear = clear_history(vt.terminal()).unwrap();
        vt.feed(&clear);
        assert_eq!(vt.terminal().scrollback_rows().unwrap(), 0);
        assert_eq!(vt.terminal().cursor_y().unwrap(), 0);
        let text = vt.tail_text(4).unwrap();
        assert!(text.starts_with("half"), "{text:?}");
        assert!(!text.contains("line"), "{text:?}");
        vt.feed(b"echo");
        assert!(vt.tail_text(4).unwrap().starts_with("halfecho"));
        // Full-screen programs keep their screen.
        vt.feed(b"\x1b[?1049hTUI");
        assert!(clear_history(vt.terminal()).unwrap().is_empty());
    }
    #[test]
    fn bells_count_real_bel_only_and_titles_survive_snapshots() {
        let rung = Arc::new(AtomicU64::new(0));
        let count = rung.clone();
        let mut vt = Vt::new(80, 24).unwrap();
        vt.on_bell(move || {
            count.fetch_add(1, Ordering::Relaxed);
        })
        .unwrap();
        assert_eq!(vt.title(), None);
        vt.feed(b"\x1b]0;~/code \xe2\x80\x94 zsh\x07"); // BEL ends the OSC: not a bell
        vt.feed(b"done\x07\x07");
        assert_eq!(rung.load(Ordering::Relaxed), 2);
        assert_eq!(vt.title().as_deref(), Some("~/code — zsh"));
        let restored = Vt::restore(&vt.snapshot().unwrap()).unwrap();
        assert_eq!(restored.title().as_deref(), Some("~/code — zsh"));
        vt.feed(b"\x1b]2;\x1b\\");
        assert_eq!(vt.title(), None);
    }
    #[test]
    fn display_copies_keep_default_colors() {
        let mut vt = Vt::new(80, 24).unwrap();
        let restored = Vt::restore(&vt.snapshot().unwrap()).unwrap();
        let bg = restored.terminal().bg_color().unwrap().unwrap();
        let [_, r, g, b] = DEFAULT_BG.to_be_bytes();
        assert_eq!((bg.r, bg.g, bg.b), (r, g, b));
    }
    #[test]
    fn untrusted_osc_and_malformed_bytes_remain_terminal_data() {
        let mut vt = Vt::new(80, 24).unwrap();
        let replies = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = replies.clone();
        vt.on_reply(move |bytes| captured.lock().unwrap().extend_from_slice(bytes))
            .unwrap();
        vt.feed(b"\x1b]52;c;c2VjcmV0\x07\x1b]52;c;?\x07");
        vt.feed(b"\x1b]8;;javascript:alert(1)\x07LINK\x1b]8;;\x07\r\n");
        vt.feed(b"\xff\xfe\x1b[999999999999m\x1b[0mRECOVERED");
        assert!(
            replies.lock().unwrap().is_empty(),
            "no clipboard response callback is installed"
        );
        assert_eq!(vt.search("LINK").unwrap().len(), 1);
        assert_eq!(vt.search("RECOVERED").unwrap().len(), 1);
        assert!(vt.search("javascript").unwrap().is_empty());
        let restored = Vt::restore(&vt.snapshot().unwrap()).unwrap();
        assert_eq!(restored.search("RECOVERED").unwrap().len(), 1);
    }
    #[test]
    fn drops_leading_activity_glyphs_only() {
        assert_eq!(shown_title("✳ 你好啊").as_deref(), Some("你好啊"));
        assert_eq!(shown_title("⠋ fix login").as_deref(), Some("fix login"));
        assert_eq!(shown_title("·x").as_deref(), Some("·x"));
        assert_eq!(shown_title("~/code — zsh").as_deref(), Some("~/code — zsh"));
        assert_eq!(shown_title(" ⠙ "), None);
        assert_eq!(shown_title(""), None);
    }
    #[test]
    fn truncated_snapshot_is_not_a_ready_terminal() {
        let mut source = Vt::new(80, 24).unwrap();
        source.feed(b"hello");
        let mut bytes = source.snapshot().unwrap();
        bytes.truncate(bytes.len() - 10);
        assert!(Vt::restore(&bytes).is_err());
    }
}
