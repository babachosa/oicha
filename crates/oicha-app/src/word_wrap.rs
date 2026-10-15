//! Prose wraps at spaces, with grapheme-safe fallback for oversized tokens.
use std::ops::Range;

pub fn breakable_space(text: &str) -> bool {
    text.chars()
        .all(|c| c.is_whitespace() && !matches!(c, '\u{00a0}' | '\u{202f}'))
}

/// Stops are shaped grapheme/cluster starts plus the final text endpoint.
pub fn breaks(text: &str, stops: &[(usize, f32)], width: f32) -> Vec<usize> {
    let mut result = Vec::new();
    let mut row = 0;
    let mut cursor = 0;
    let mut candidate = None;
    while cursor + 1 < stops.len() {
        let span: Range<usize> = stops[cursor].0..stops[cursor + 1].0;
        let space = breakable_space(&text[span]);
        if cursor > row && breakable_space(&text[stops[cursor - 1].0..stops[cursor].0]) && !space {
            candidate = Some(cursor);
        }
        // Trailing spaces belong to their row, so they don't create a blank row
        // or push a preceding whole word onto another row by themselves.
        if !space && stops[cursor + 1].1 - stops[row].1 > width && cursor > row {
            let next = candidate.filter(|next| *next > row).unwrap_or(cursor);
            result.push(stops[next].0);
            row = next;
            cursor = next;
            candidate = None;
        } else {
            cursor += 1;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_segmentation::UnicodeSegmentation;

    fn wrap(text: &str, width: f32) -> Vec<String> {
        let stops: Vec<_> = text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain([text.len()])
            .enumerate()
            .map(|(x, i)| (i, x as f32))
            .collect();
        let mut start = 0;
        breaks(text, &stops, width)
            .into_iter()
            .chain([text.len()])
            .map(|end| {
                let row = text[start..end].to_owned();
                start = end;
                row
            })
            .collect()
    }

    #[test]
    fn vietnamese_words_and_punctuation_move_together() {
        assert_eq!(wrap("này niềm tin", 7.), ["này ", "niềm ", "tin"]);
        assert_eq!(
            wrap("này nie\u{0302}\u{0300}m tin", 7.),
            ["này ", "nie\u{0302}\u{0300}m ", "tin"]
        );
        assert_eq!(wrap("đây “niềm,” tin", 10.), ["đây ", "“niềm,” ", "tin"]);
    }

    #[test]
    fn oversized_words_make_progress_without_splitting_graphemes() {
        assert_eq!(
            wrap("a\u{0306}\u{0301}bcdefghi", 3.),
            ["a\u{0306}\u{0301}bc", "def", "ghi"]
        );
        assert_eq!(wrap("👩‍👩‍👧‍👦🌿abc", 1.), ["👩‍👩‍👧‍👦", "🌿", "a", "b", "c"]);
        assert_eq!(wrap("áá", 0.), ["á", "á"]);
    }

    #[test]
    fn spaces_empty_text_and_nonbreaking_spaces_are_preserved() {
        assert_eq!(wrap("", 4.), [""]);
        assert_eq!(wrap("abc   def", 3.), ["abc   ", "def"]);
        assert_eq!(wrap("x niềm\u{00a0}tin", 8.), ["x ", "niềm\u{00a0}tin"]);
        assert_eq!(wrap("x niềm\u{202f}tin", 8.), ["x ", "niềm\u{202f}tin"]);
    }
}
