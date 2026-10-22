pub use libghostty_vt;
use libghostty_vt::{Terminal, snapshot::Decoder};
use std::io;

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
    fn truncated_snapshot_is_not_a_ready_terminal() {
        let mut source = Vt::new(80, 24).unwrap();
        source.feed(b"hello");
        let mut bytes = source.snapshot().unwrap();
        bytes.truncate(bytes.len() - 10);
        assert!(Vt::restore(&bytes).is_err());
    }
}
