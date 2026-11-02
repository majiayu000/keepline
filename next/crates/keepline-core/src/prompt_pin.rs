//! Pinning the turn's ask while its answer runs past the top of the terminal.
//!
//! Three parts, and only the first knows which CLI runs: what was asked comes from the
//! transcripts (`Content::asks`); how a sent ask looks on screen comes from the CLI's
//! `PromptLine`; where each ask sits in the terminal's history and what to show is worked
//! out here from rows of text alone.
use crate::agent::PromptLine;

/// Characters of an ask matched against a row, whitespace left out.
pub const KEY_CHARS: usize = 10;
/// Columns read from the left of each row: a mark, an indent and the key fit in this.
pub const PREFIX_COLS: u16 = 48;
/// Rows looked through, newest first, before an ask counts as gone from history.
pub const MAX_ROWS: usize = 5000;
/// A row shorter than the key still matches the start of it, with at least this much.
const MIN_PARTIAL: usize = 4;

/// The part of an ask a row is matched on: its first line with text, image placeholders
/// and whitespace left out, cut to `KEY_CHARS`. `None` when too little is left to tell
/// one ask from another.
pub fn key(ask: &str) -> Option<String> {
    let line = ask.lines().map(plain).find(|l| !l.is_empty())?;
    let key: String = line.chars().take(KEY_CHARS).collect();
    (key.chars().count() >= 2).then_some(key)
}

/// Whitespace and `[Image #n]` placeholders out; the CLI and its transcript disagree on
/// both.
fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("[Image #") {
        out.push_str(&rest[..at]);
        let tail = &rest[at + "[Image #".len()..];
        match tail.find(']') {
            Some(end) if tail[..end].chars().all(|c| c.is_ascii_digit()) => {
                rest = &tail[end + 1..];
            }
            _ => {
                out.push_str("[Image #");
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out.retain(|c| !c.is_whitespace());
    out
}

/// Whether `row` (the left of one terminal row) is the sent ask whose key is `key`.
pub fn is_prompt_row(row: &str, key: &str, prompt: &PromptLine) -> bool {
    let row = row.trim_start();
    let rest = if prompt.markers.is_empty() {
        row
    } else {
        match prompt
            .markers
            .iter()
            .find_map(|m| row.strip_prefix(m.as_str()))
        {
            Some(rest) => rest,
            None => return false,
        }
    };
    let rest = plain(rest);
    if rest.is_empty() {
        return false;
    }
    rest.starts_with(key) || (rest.chars().count() >= MIN_PARTIAL && key.starts_with(&rest))
}

/// Each ask's history row, newest ask first found from the bottom up: an ask is looked
/// for only above the newer one found, so two asks with the same words land on their own
/// rows. `rows` yields (row, text) from the bottom of history upward.
pub fn locate(
    keys: &[Option<String>],
    prompt: &PromptLine,
    rows: impl IntoIterator<Item = (usize, String)>,
) -> Vec<Option<usize>> {
    let mut found = vec![None; keys.len()];
    // Asks at or above this index are still being looked for.
    let mut want = keys.len();
    for (y, text) in rows {
        if want == 0 {
            break;
        }
        let hit = (0..want).rev().find(|&i| {
            keys[i]
                .as_deref()
                .is_some_and(|k| is_prompt_row(&text, k, prompt))
        });
        if let Some(i) = hit {
            found[i] = Some(y);
            want = i;
        }
    }
    found
}

/// What the pane draws for the asks it knows, given where they are.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pinned {
    /// The newest ask, when its row is above the viewport: the turn on screen is its
    /// answer. `row` is where to scroll back to; `None` when the ask was not found in
    /// history (gone, or drawn differently), shown only at the live bottom.
    pub latest: Option<(usize, Option<usize>)>,
    /// While reading history: the ask whose answer the viewport's top row belongs to,
    /// when that ask's own row is above the viewport and it is not the newest one.
    pub viewed: Option<(usize, usize)>,
}

/// `rows` from `locate`, `viewport` as `Vt::viewport` gives it: (first shown row, total
/// rows, shown rows).
pub fn pinned(rows: &[Option<usize>], viewport: (usize, usize, usize)) -> Pinned {
    let (top, total, shown) = viewport;
    let at_bottom = top + shown >= total;
    let Some(last) = rows.len().checked_sub(1) else {
        return Pinned::default();
    };
    let latest = match rows[last] {
        Some(row) if row < top => Some((last, Some(row))),
        Some(_) => None,
        None if at_bottom => Some((last, None)),
        None => None,
    };
    // The newest found ask at or above the top row owns what is shown there.
    let viewed = rows
        .iter()
        .enumerate()
        .filter_map(|(i, r)| r.map(|r| (i, r)))
        .filter(|&(_, r)| r <= top)
        .max_by_key(|&(_, r)| r)
        .filter(|&(i, r)| r < top && i != last);
    Pinned { latest, viewed }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claude() -> PromptLine {
        PromptLine {
            markers: vec!["❯".into(), ">".into()],
            pins_own: false,
        }
    }

    #[test]
    fn the_key_skips_blank_lines_whitespace_and_image_placeholders() {
        assert_eq!(
            key("\n  [Image #1] codex 的 这个设计 是 codex 本来的设计吗").as_deref(),
            Some("codex的这个设计")
        );
        assert_eq!(key("ok").as_deref(), Some("ok"));
        assert_eq!(key(" a \n"), None);
        assert_eq!(key("[Image #2]"), None);
    }

    #[test]
    fn a_row_matches_only_behind_the_clis_mark() {
        let k = key("你帮我看看代码再说啊").unwrap();
        assert!(is_prompt_row("❯ 你帮我看看代码再说啊", &k, &claude()));
        assert!(is_prompt_row("  > 你帮我 看看代码再说啊   ", &k, &claude()));
        // The answer quoting the ask is not the ask.
        assert!(!is_prompt_row("你帮我看看代码再说啊", &k, &claude()));
        assert!(!is_prompt_row("● 你帮我看看代码再说啊", &k, &claude()));
        // The empty input box and a numbered menu are not asks.
        assert!(!is_prompt_row("❯ ", &k, &claude()));
        assert!(!is_prompt_row("❯ 1. Yes", &k, &claude()));
        // A narrow pane wraps the ask; its first row still names it.
        assert!(is_prompt_row("❯ 你帮我看看", &k, &claude()));
        assert!(!is_prompt_row("❯ 你帮", &k, &claude()));
        // A CLI without a mark: the row starts with the ask.
        let bare = PromptLine::default();
        assert!(is_prompt_row("你帮我看看代码再说啊", &k, &bare));
    }

    #[test]
    fn asks_are_found_newest_first_each_above_the_newer_one() {
        let keys = vec![key("继续"), key("修一下登录"), key("继续")];
        let screen = [
            (0, "❯ 继续"),
            (5, "❯ 修一下登录"),
            (9, "● 好的"),
            (12, "❯ 继续"),
            (20, "❯ "),
        ];
        let rows = screen.iter().rev().map(|(y, t)| (*y, t.to_string()));
        assert_eq!(
            locate(&keys, &claude(), rows),
            vec![Some(0), Some(5), Some(12)]
        );
        // One not on screen leaves the older ones findable.
        let keys = vec![
            key("修一下登录"),
            key("[Pasted text #1 +20 lines] 太长了的东西"),
        ];
        let rows = screen.iter().rev().map(|(y, t)| (*y, t.to_string()));
        assert_eq!(locate(&keys, &claude(), rows), vec![Some(5), None]);
    }

    #[test]
    fn the_newest_ask_pins_once_its_row_scrolls_above_the_view() {
        let rows = [Some(10), Some(40)];
        // Its row on screen: no pin; the top rows still answer the older ask.
        let both = pinned(&rows, (30, 100, 70));
        assert_eq!(both.latest, None);
        assert_eq!(both.viewed, Some((0, 10)));
        assert_eq!(pinned(&rows, (10, 100, 90)), Pinned::default());
        // Answer runs past: pinned, with the row to go back to.
        assert_eq!(pinned(&rows, (60, 100, 40)).latest, Some((1, Some(40))));
        // Reading the older turn: the newest is below, the older one is the context.
        let older = pinned(&rows, (20, 100, 10));
        assert_eq!(older.latest, None);
        assert_eq!(older.viewed, Some((0, 10)));
        // Above every ask: nothing to say.
        assert_eq!(pinned(&rows, (0, 100, 5)), Pinned::default());
    }

    #[test]
    fn an_ask_not_in_history_pins_only_at_the_live_bottom() {
        let rows = [None];
        assert_eq!(pinned(&rows, (60, 100, 40)).latest, Some((0, None)));
        assert_eq!(pinned(&rows, (10, 100, 40)).latest, None);
        assert_eq!(pinned(&[], (0, 0, 0)), Pinned::default());
    }
}
