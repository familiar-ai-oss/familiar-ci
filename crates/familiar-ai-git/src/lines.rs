//! Splitting file bytes into lines without losing a byte.

/// Split `bytes` into lines, each with its line ending. A final line without a
/// terminator is a line too, and empty input has no lines.
pub fn split_lines(bytes: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut start = 0usize;
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'\n' {
            lines.push(&bytes[start..=i]);
            start = i + 1;
        }
    }
    if start < bytes.len() {
        lines.push(&bytes[start..]);
    }
    lines
}

/// The number of lines in `bytes` (as [`split_lines`] counts them).
pub fn line_count(bytes: &[u8]) -> u64 {
    split_lines(bytes).len() as u64
}

/// The exact bytes of lines `line_start..=line_end` (1-based, inclusive),
/// or `None` when the range is outside the file.
pub fn line_bytes(bytes: &[u8], line_start: u64, line_end: u64) -> Option<Vec<u8>> {
    if line_start == 0 || line_end < line_start {
        return None;
    }
    let lines = split_lines(bytes);
    let (s, e) = (line_start as usize, line_end as usize);
    if e > lines.len() {
        return None;
    }
    let mut out = Vec::new();
    for line in &lines[s - 1..e] {
        out.extend_from_slice(line);
    }
    Some(out)
}

/// The line git prints after a hunk's `@@` (its default `funcname`, as with no
/// diff driver set): the nearest line before line `before` (1-based) that
/// begins with a letter, `_` or `$`, with its number, trailing whitespace
/// trimmed and cut to 80 bytes; `None` when no line above qualifies.
pub fn funcname_above(bytes: &[u8], before: u64) -> Option<(u64, String)> {
    let lines = split_lines(bytes);
    let upto = usize::try_from(before.saturating_sub(1)).ok()?.min(lines.len());
    lines[..upto].iter().enumerate().rev().find_map(|(i, l)| {
        let first = *l.first()?;
        if !(first.is_ascii_alphabetic() || first == b'_' || first == b'$') {
            return None;
        }
        let mut end = l.len().min(80);
        while end > 0 && l[end - 1].is_ascii_whitespace() {
            end -= 1;
        }
        Some((i as u64 + 1, String::from_utf8_lossy(&l[..end]).into_owned()))
    })
}

#[cfg(test)]
mod funcname_tests {
    use super::funcname_above;

    #[test]
    fn the_nearest_line_above_that_starts_a_name_is_the_header() {
        let text = b"#!/bin/sh\ngreet() { echo hi; }\n  inside\ngreet\n";
        assert_eq!(funcname_above(text, 4), Some((2, "greet() { echo hi; }".to_string())));
        assert_eq!(funcname_above(text, 2), None, "the shebang starts with #");
        assert_eq!(funcname_above(text, 1), None);
        assert_eq!(funcname_above(b"def f():   \n    x\n", 2), Some((1, "def f():".to_string())));
    }
}
