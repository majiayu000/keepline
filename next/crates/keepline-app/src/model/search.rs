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
/// Smaller searches to offer when `query` finds nothing: its words, or for one run of
/// four or more CJK characters its two-character pieces; at most three, no repeats.
pub fn split_terms(query: &str) -> Vec<String> {
    let words: Vec<&str> = query.split_whitespace().collect();
    let mut terms: Vec<String> = if words.len() > 1 {
        words.into_iter().map(str::to_owned).collect()
    } else {
        let chars: Vec<char> = query.trim().chars().collect();
        let cjk = |c: &char| ('\u{4e00}'..='\u{9fff}').contains(c);
        if chars.len() >= 4 && chars.iter().all(cjk) {
            chars.chunks(2).map(|c| c.iter().collect()).collect()
        } else {
            vec![]
        }
    };
    let mut seen = std::collections::HashSet::new();
    terms.retain(|t| seen.insert(t.clone()));
    terms.truncate(3);
    terms
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
    fn search_offers_smaller_terms_when_nothing_matches() {
        assert_eq!(split_terms("发票报销"), ["发票", "报销"]);
        assert_eq!(split_terms("tab  rename"), ["tab", "rename"]);
        assert_eq!(split_terms("a a b c d"), ["a", "b", "c"]);
        assert!(split_terms("终端").is_empty());
        assert!(split_terms("rename").is_empty());
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
