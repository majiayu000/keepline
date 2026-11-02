//! Reads what a program says about itself through OSC 7501, the Program Status Protocol:
//! `ESC ] 7501 ; state=<word>[:kind=…][:progress=N][:app=…][:title=<b64>][:msg=<b64>]`
//! ended by `ESC \` or BEL. The words are the program's own; nothing is guessed.
//!
//! Only the terminal's own record counts here: a report with an `id` names a child
//! record (a step, a sub-task) and is skipped. A program's report lasts until it says
//! `clear` or ends, and the shell's command marks (OSC 133 `C` then `D`) say when a
//! command ended, so both are read in order: a script that reports `done` and exits in
//! the same read leaves nothing behind. Sequences may be split across reads; the unread
//! start of one is kept for the next read.
use base64::Engine;

const STATUS: &[u8] = b"\x1b]7501;";
const MARK: &[u8] = b"\x1b]133;";
/// A sequence longer than this is not one worth keeping; it is dropped, not buffered.
const LONGEST: usize = 4096;
/// Title and message are shown in one line; longer ones are cut.
const SHOWN: usize = 200;
const STATES: &[&str] = &["idle", "working", "done", "blocked", "error"];

/// What a program said, without the time the host stamps on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Said {
    pub state: String,
    pub kind: Option<String>,
    pub progress: Option<u8>,
    pub app: String,
    pub title: String,
    pub msg: String,
}

#[derive(Default)]
pub struct Scanner {
    partial: Vec<u8>,
    /// The shell marked a command started and not yet ended.
    in_command: bool,
}

impl Scanner {
    /// What `bytes` changed: `None` when nothing, `Some(None)` when the report is gone
    /// (cleared, or its command ended), `Some(Some(said))` for the latest report.
    pub fn scan(&mut self, bytes: &[u8]) -> Option<Option<Said>> {
        let joined;
        let data = if self.partial.is_empty() {
            bytes
        } else {
            let mut j = std::mem::take(&mut self.partial);
            j.extend_from_slice(bytes);
            joined = j;
            &joined[..]
        };
        let mut change = None;
        let mut i = 0;
        while let Some(off) = data[i..].iter().position(|&b| b == 0x1b) {
            let rest = &data[i + off..];
            let start = [STATUS, MARK]
                .into_iter()
                .find(|s| rest.starts_with(s) || (rest.len() < s.len() && s.starts_with(rest)));
            let Some(start) = start else {
                i += off + 1;
                continue;
            };
            if rest.len() < start.len() {
                self.partial = rest.to_vec();
                break;
            }
            let body = &rest[start.len()..];
            // BEL, or the ESC of ESC \, ends it.
            let Some(end) = body.iter().position(|&b| b == 0x07 || b == 0x1b) else {
                if rest.len() <= LONGEST {
                    self.partial = rest.to_vec();
                }
                break;
            };
            let body = &body[..end];
            if start == STATUS {
                if let Some(said) = parse(body) {
                    change = Some(said);
                }
            } else {
                match body.split(|&b| b == b';').next() {
                    Some(b"C") => self.in_command = true,
                    Some(b"D") if std::mem::take(&mut self.in_command) => change = Some(None),
                    _ => {}
                }
            }
            i += off + start.len() + end + 1;
        }
        change
    }
}

/// One report's body: `Some(None)` for `clear`, `None` for one to skip.
fn parse(body: &[u8]) -> Option<Option<Said>> {
    let body = std::str::from_utf8(body).ok()?;
    let mut said = Said {
        state: String::new(),
        kind: None,
        progress: None,
        app: String::new(),
        title: String::new(),
        msg: String::new(),
    };
    for field in body.split(':') {
        let (key, value) = field.split_once('=')?;
        match key {
            "state" => said.state = value.into(),
            // A child record: not the terminal's own word.
            "id" if !value.is_empty() => return None,
            "kind" if !value.is_empty() => said.kind = Some(word(value)),
            "progress" => said.progress = value.parse().ok().filter(|p| *p <= 100),
            "app" => said.app = word(value),
            "title" => said.title = text(value),
            "msg" => said.msg = text(value),
            _ => {}
        }
    }
    if said.state == "clear" {
        return Some(None);
    }
    STATES.contains(&said.state.as_str()).then_some(Some(said))
}

fn word(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .take(64)
        .collect()
}

/// A base64 field as one line of text, control characters dropped.
fn text(value: &str) -> String {
    let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(value) else {
        return String::new();
    };
    String::from_utf8_lossy(&bytes)
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(SHOWN)
        .collect::<String>()
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(state: &str) -> Option<Option<Said>> {
        Some(Some(Said {
            state: state.into(),
            kind: None,
            progress: None,
            app: String::new(),
            title: String::new(),
            msg: String::new(),
        }))
    }

    #[test]
    fn a_report_carries_its_fields_and_the_latest_one_wins() {
        let mut s = Scanner::default();
        // "Apply these changes?" and "Run tests" in base64.
        let got = s.scan(
            b"out\x1b]7501;state=working\x07more\x1b]7501;state=blocked:kind=permission:progress=40:app=claude-code:title=UnVuIHRlc3Rz:msg=QXBwbHkgdGhlc2UgY2hhbmdlcz8=\x1b\\",
        );
        assert_eq!(
            got,
            Some(Some(Said {
                state: "blocked".into(),
                kind: Some("permission".into()),
                progress: Some(40),
                app: "claude-code".into(),
                title: "Run tests".into(),
                msg: "Apply these changes?".into(),
            }))
        );
        assert_eq!(s.scan(b"plain output"), None, "nothing said");
    }

    #[test]
    fn clear_child_records_and_unknown_words() {
        let mut s = Scanner::default();
        assert_eq!(s.scan(b"\x1b]7501;state=clear\x07"), Some(None));
        assert_eq!(
            s.scan(b"\x1b]7501;state=working:id=build\x07"),
            None,
            "a child record"
        );
        assert_eq!(
            s.scan(b"\x1b]7501;state=sleeping\x07"),
            None,
            "not a protocol word"
        );
        assert_eq!(s.scan(b"\x1b]7501;nonsense\x07"), None);
        assert_eq!(
            s.scan(b"\x1b]7501;state=done:progress=300\x07"),
            said("done")
        );
    }

    #[test]
    fn a_command_ending_takes_its_report_with_it_in_order() {
        let mut s = Scanner::default();
        // The first prompt ends no command.
        assert_eq!(s.scan(b"\x1b]133;D;0\x07$ "), None);
        // A script reports done and exits within one read: nothing is left.
        assert_eq!(
            s.scan(b"\x1b]133;C\x07\x1b]7501;state=done\x07\x1b]133;D;0\x07"),
            Some(None)
        );
        // A report after its command ended (a background job) stays.
        assert_eq!(
            s.scan(b"\x1b]133;C\x07\x1b]133;D;0\x07\x1b]7501;state=working\x07"),
            said("working")
        );
    }

    #[test]
    fn reports_split_across_reads_are_still_read() {
        let all: &[u8] = b"a\x1b]7501;state=working\x1b\\b\x1b]133;C\x07\x1b]7501;state=error\x07";
        for split in 0..all.len() {
            let mut s = Scanner::default();
            let first = s.scan(&all[..split]);
            let second = s.scan(&all[split..]);
            assert_eq!(second.or(first), said("error"), "split at {split}");
        }
    }

    #[test]
    fn other_escapes_and_endless_reports_are_skipped() {
        let mut s = Scanner::default();
        assert_eq!(s.scan(b"\x1b[31m\x1b]7;file://h/x\x07\x1b]75;y\x07"), None);
        let mut long = b"\x1b]7501;state=working:msg=".to_vec();
        long.resize(LONGEST + 10, b'A');
        assert_eq!(s.scan(&long), None);
        assert!(s.partial.is_empty(), "dropped, not buffered");
        // Control characters in the text never reach the pane.
        assert_eq!(text("YQpi"), "a b");
    }
}
