//! Search matching and palette list movement.
use std::ops::Range;

pub fn contains(text: &str, query: &str) -> bool {
    !match_ranges(text, query).is_empty()
}
/// Byte ranges of every case-insensitive, non-overlapping match of `query` in `text`.
pub fn match_ranges(text: &str, query: &str) -> Vec<Range<usize>> {
    let needle: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    if needle.is_empty() {
        return vec![];
    }
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + needle.len() <= chars.len() {
        let hit = chars[i..i + needle.len()]
            .iter()
            .zip(&needle)
            .all(|((_, c), n)| c.to_lowercase().eq(std::iter::once(*n)));
        if hit {
            let end = chars
                .get(i + needle.len())
                .map_or(text.len(), |(at, _)| *at);
            out.push(chars[i].0..end);
            i += needle.len();
        } else {
            i += 1;
        }
    }
    out
}
/// About `width` characters of `text`, starting a little before the first match so the
/// match is visible; whitespace runs collapse to one space.
pub fn excerpt(text: &str, query: &str, width: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let first = match_ranges(&text, query)
        .first()
        .map_or(0, |r| text[..r.start].chars().count());
    let start = first.saturating_sub(width / 4);
    let mut out: String = text.chars().skip(start).take(width).collect();
    if start > 0 {
        out.insert(0, '…');
    }
    if text.chars().count() > start + width {
        out.push('…');
    }
    out
}
pub fn step(index: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    (index.min(len - 1) as isize + delta).rem_euclid(len as isize) as usize
}
/// The first row of the next group (or of the previous one with `back`), wrapping.
pub fn section_jump(sizes: &[usize], index: usize, back: bool) -> usize {
    let starts: Vec<usize> = sizes
        .iter()
        .scan(0, |at, n| {
            let start = *at;
            *at += n;
            Some((start, *n))
        })
        .filter(|(_, n)| *n > 0)
        .map(|(start, _)| start)
        .collect();
    if starts.is_empty() {
        return 0;
    }
    let current = starts.iter().rposition(|s| *s <= index).unwrap_or(0);
    let next = if back {
        // ⇧Tab from inside a group goes to its own top first.
        if starts[current] < index {
            current
        } else {
            (current + starts.len() - 1) % starts.len()
        }
    } else {
        (current + 1) % starts.len()
    };
    starts[next]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_matches_ignore_case_and_keep_byte_ranges() {
        assert_eq!(
            match_ranges("修复标签栏和标签", "标签"),
            vec![6..12, 18..24]
        );
        assert_eq!(match_ranges("Fix Tab bar", "tab"), vec![4..7]);
        assert!(match_ranges("abc", "").is_empty());
        assert!(match_ranges("ab", "abc").is_empty());
    }

    #[test]
    fn search_excerpt_shows_the_match() {
        let long = format!("{}关键字{}", "前".repeat(100), "后".repeat(100));
        let e = excerpt(&long, "关键字", 40);
        assert!(e.starts_with('…') && e.ends_with('…') && e.contains("关键字"));
        assert_eq!(excerpt("a  b\n c", "", 10), "a b c");
    }

    #[test]
    fn search_keyboard_moves_wrap_and_jump_groups() {
        assert_eq!(step(0, 3, -1), 2);
        assert_eq!(step(2, 3, 1), 0);
        assert_eq!(step(9, 3, 1), 0);
        assert_eq!(step(0, 0, 1), 0);
        let sizes = [2, 0, 3, 1];
        assert_eq!(section_jump(&sizes, 0, false), 2);
        assert_eq!(section_jump(&sizes, 3, false), 5);
        assert_eq!(section_jump(&sizes, 5, false), 0);
        assert_eq!(section_jump(&sizes, 3, true), 2);
        assert_eq!(section_jump(&sizes, 2, true), 0);
        assert_eq!(section_jump(&sizes, 0, true), 5);
    }
}
