//! Reads the marks a shell puts around each command it runs (OSC 133, as Ghostty, iTerm2
//! and kitty use them): `C` when the command starts, `D;<exit>` when the shell is back at
//! its prompt. A `D` with no `C` before it is a prompt with no command (an empty line, or
//! the first prompt), so only a `C`…`D` pair is a finished command. Marks may be split
//! across reads; the unread start of one is kept for the next read.
use std::time::{Duration, Instant};

const START: &[u8] = b"\x1b]133;";
/// A mark longer than this is not one the shells write; it is dropped, not buffered.
const LONGEST: usize = 256;

#[derive(Default)]
pub struct Marks {
    partial: Vec<u8>,
    started: Option<Instant>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Finished {
    /// None when the shell did not say.
    pub exit: Option<i32>,
    pub took: Duration,
}

impl Marks {
    /// The commands that finished in `bytes`, read at `at`.
    pub fn scan(&mut self, bytes: &[u8], at: Instant) -> Vec<Finished> {
        let mut found = vec![];
        let joined;
        let data = if self.partial.is_empty() {
            bytes
        } else {
            let mut j = std::mem::take(&mut self.partial);
            j.extend_from_slice(bytes);
            joined = j;
            &joined[..]
        };
        let mut i = 0;
        while let Some(off) = data[i..].iter().position(|&b| b == 0x1b) {
            let rest = &data[i + off..];
            if rest.len() < START.len() {
                if START.starts_with(rest) {
                    self.partial = rest.to_vec();
                }
                break;
            }
            if !rest.starts_with(START) {
                i += off + 1;
                continue;
            }
            let body = &rest[START.len()..];
            // BEL, or the ESC of ESC \, ends it.
            match body.iter().position(|&b| b == 0x07 || b == 0x1b) {
                Some(end) => {
                    self.mark(&body[..end], at, &mut found);
                    i += off + START.len() + end + 1;
                }
                None => {
                    if rest.len() <= LONGEST {
                        self.partial = rest.to_vec();
                    }
                    break;
                }
            }
        }
        found
    }
    fn mark(&mut self, body: &[u8], at: Instant, found: &mut Vec<Finished>) {
        let mut fields = body.split(|&b| b == b';');
        match fields.next() {
            Some(b"C") => self.started = Some(at),
            Some(b"D") => {
                if let Some(started) = self.started.take() {
                    found.push(Finished {
                        exit: fields
                            .next()
                            .and_then(|f| std::str::from_utf8(f).ok())
                            .and_then(|f| f.trim().parse().ok()),
                        took: at.saturating_duration_since(started),
                    });
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_is_a_start_mark_then_an_end_mark_with_its_exit_code() {
        let mut m = Marks::default();
        let t0 = Instant::now();
        // The first prompt and an empty line end nothing.
        assert!(m.scan(b"\x1b]133;D;0\x07$ ", t0).is_empty());
        assert!(m.scan(b"\x1b]133;C\x07", t0).is_empty());
        let t1 = t0 + Duration::from_millis(1500);
        assert_eq!(
            m.scan(b"out\r\n\x1b]133;D;2\x1b\\\x1b]133;A\x07$ ", t1),
            vec![Finished {
                exit: Some(2),
                took: Duration::from_millis(1500)
            }]
        );
        assert!(
            m.scan(b"\x1b]133;D;0\x07", t1).is_empty(),
            "one end per start"
        );
    }

    #[test]
    fn marks_split_across_reads_are_still_read_and_several_fit_in_one() {
        let mut m = Marks::default();
        let t = Instant::now();
        let all = b"\x1b]133;C\x07a\x1b]133;D;0\x07\x1b]133;C\x07b\x1b]133;D;1\x07";
        for split in 0..all.len() {
            let mut found = m.scan(&all[..split], t);
            found.extend(m.scan(&all[split..], t));
            let exits: Vec<_> = found.iter().map(|f| f.exit).collect();
            assert_eq!(exits, [Some(0), Some(1)], "split at {split}");
        }
    }

    #[test]
    fn other_escapes_and_endless_marks_are_skipped() {
        let mut m = Marks::default();
        let t = Instant::now();
        assert!(m
            .scan(b"\x1b[31m\x1b]7;file://h/x\x07\x1b]1337;y\x07", t)
            .is_empty());
        let mut long = b"\x1b]133;C".to_vec();
        long.resize(LONGEST + 10, b'x');
        assert!(m.scan(&long, t).is_empty());
        assert!(m.partial.is_empty(), "dropped, not buffered");
        // An end with no code is still an end.
        m.scan(b"\x1b]133;C\x07", t);
        assert_eq!(m.scan(b"\x1b]133;D\x07", t)[0].exit, None);
    }
}
