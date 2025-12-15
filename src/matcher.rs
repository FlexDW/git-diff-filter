//! Batch string matching against glob patterns.
//!
//! This implementation processes multiple strings against a single pattern,
//! maintaining only the active (still-matching) strings for optimal performance.

/// Check if a single path matches any of the provided patterns.
/// Returns true if ANY pattern matches the path.
///
/// # Errors
/// Returns an error if any pattern contains unsupported syntax.
pub fn matches_any(path: &str, patterns: &[String]) -> Result<bool, String> {
    for pattern in patterns {
        let results = match_batch(pattern, &[path])?;
        if results.first() == Some(&true) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Active string being matched against the pattern
#[derive(Debug)]
struct ActiveString<'a> {
    /// Index in the original input array (for result tracking)
    original_idx: usize,
    /// Byte representation of the string
    bytes: &'a [u8],
    /// Current position in the string
    position: usize,
}

impl ActiveString<'_> {
    /// Peek at current byte without advancing position
    /// Returns None if position is at or beyond end of string
    fn current_byte(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    /// Advance position by one byte
    fn advance(&mut self) {
        self.position += 1;
    }
}

/// Consume one byte from each active string based on a predicate.
/// Strings matching the predicate advance; others are marked false and removed.
fn consume_byte<F>(active: &mut Vec<ActiveString>, results: &mut [bool], predicate: F)
where
    F: Fn(Option<u8>) -> bool,
{
    let mut i: usize = 0;
    while i < active.len() {
        let string: &mut ActiveString<'_> = &mut active[i];
        if predicate(string.current_byte()) {
            string.advance();
            i += 1;
        } else {
            results[string.original_idx] = false;
            active.swap_remove(i);
        }
    }
}

/// Pattern matching state machine
#[derive(Debug, Clone, Copy, PartialEq)]
enum PatternState {
    Literal,              // Normal character-by-character matching
    InWildcard,           // Seen *, determining what kind
    InPossibleGlobstar,   // Seen **, determining if **/
    InGlobstar,           // Confirmed **/, ready to match
    InSuperWild,          // Confirmed **/*,  ready to match
}

/// Match multiple strings against a single glob pattern
///
/// Matching is done on byte arrays as control characters are all single-byte ASCII
/// characters. Any other characters will need to match the pattern segments byte
/// for byte anyway, so we can avoid converting strings to chars.
///
/// Returns a `Vec<bool>` indicating which strings matched (`true`) or failed (`false`)
#[allow(clippy::too_many_lines)]
pub fn match_batch(pattern: &str, strings: &[&str]) -> Result<Vec<bool>, String> {
    if strings.is_empty() {
        return Ok(Vec::new());
    }

    // Pre-allocate results array - all initially false
    let mut results: Vec<bool> = vec![false; strings.len()];

    // Build active strings list with original indices
    let mut active: Vec<ActiveString> = strings
        .iter()
        .enumerate()
        .map(|(idx, &s)| ActiveString {
            original_idx: idx,
            bytes: s.as_bytes(),
            position: 0,
        })
        .collect();

    // Strip leading / from pattern since git diff paths don't have leading slashes
    // Strip trailing / from pattern - as we already match directories
    let normalized_pattern = pattern.strip_prefix('/').unwrap_or(pattern);
    let normalized_pattern = normalized_pattern
        .strip_suffix('/')
        .unwrap_or(normalized_pattern);
    let pattern_bytes: &[u8] = normalized_pattern.as_bytes();

    let mut pattern_idx: usize = 0;
    let mut pattern_state = PatternState::Literal;
    let mut question_count: usize = 0;

    while pattern_idx < pattern_bytes.len() && !active.is_empty() {
        let c: u8 = pattern_bytes[pattern_idx];

        match c {
            b'*' => {
                match pattern_state {
                    PatternState::Literal => {
                        pattern_state = PatternState::InWildcard;
                        pattern_idx += 1;
                    }
                    PatternState::InWildcard => {
                        pattern_state = PatternState::InPossibleGlobstar;
                        pattern_idx += 1;
                    }
                    PatternState::InPossibleGlobstar => {
                        // Stay in InPossibleGlobstar (handles ***, ****, etc.)
                        pattern_idx += 1;
                    }
                    PatternState::InGlobstar => {
                        pattern_state = PatternState::InSuperWild;
                        pattern_idx += 1;
                    }
                    PatternState::InSuperWild => {
                        // Stay in InSuperWild
                        pattern_idx += 1;
                    }
                }
            }
            b'/' => {
                match pattern_state {
                    PatternState::InPossibleGlobstar => {
                        pattern_state = PatternState::InGlobstar;
                        pattern_idx += 1;
                    }
                    PatternState::InGlobstar | PatternState::InSuperWild => {
                        // Skip redundant slashes
                        pattern_idx += 1;
                    }
                    PatternState::InWildcard => {
                        // Trigger wildcard matching
                        let next_pattern_idx = match_wildcard_segment(
                            pattern_bytes,
                            pattern_idx,
                            &mut active,
                            &mut results,
                            false, // wildcard mode
                            question_count,
                        )?;
                        pattern_idx = next_pattern_idx;
                        pattern_state = PatternState::Literal;
                        question_count = 0;
                    }
                    PatternState::Literal => {
                        // Match / literally against active strings
                        consume_byte(&mut active, &mut results, |b| b == Some(b'/'));
                        pattern_idx += 1;
                    }
                }
            }
            b'?' => {
                match pattern_state {
                    PatternState::Literal => {
                        // Match ? as single char
                        pattern_idx += 1;
                        consume_byte(&mut active, &mut results, |b| matches!(b, Some(c) if c != b'/'));
                    }
                    PatternState::InWildcard
                    | PatternState::InPossibleGlobstar
                    | PatternState::InGlobstar
                    | PatternState::InSuperWild => {
                        // In any wildcard state: just count it
                        question_count += 1;
                        pattern_idx += 1;
                    }
                }
            }
            b'\\' => {
                match pattern_state {
                    PatternState::Literal => {
                        // Escape next character
                        if pattern_idx + 1 >= pattern_bytes.len() {
                            return Err("Pattern ends with backslash".to_string());
                        }
                        pattern_idx += 1;
                        let escaped: u8 = pattern_bytes[pattern_idx];

                        // Match literal byte against all active strings
                        consume_byte(&mut active, &mut results, |b| b == Some(escaped));
                        pattern_idx += 1;
                    }
                    PatternState::InWildcard | PatternState::InPossibleGlobstar => {
                        // Trigger wildcard matching
                        let next_pattern_idx = match_wildcard_segment(
                            pattern_bytes,
                            pattern_idx,
                            &mut active,
                            &mut results,
                            false, // wildcard mode
                            question_count,
                        )?;
                        pattern_idx = next_pattern_idx;
                        pattern_state = PatternState::Literal;
                        question_count = 0;
                    }
                    PatternState::InGlobstar => {
                        // Trigger globstar matching
                        let next_pattern_idx = match_wildcard_segment(
                            pattern_bytes,
                            pattern_idx,
                            &mut active,
                            &mut results,
                            true, // globstar mode
                            question_count,
                        )?;
                        pattern_idx = next_pattern_idx;
                        pattern_state = PatternState::Literal;
                        question_count = 0;
                    }
                    PatternState::InSuperWild => {
                        // Trigger super-wild matching (TODO: implement super-wild mode)
                        let next_pattern_idx = match_wildcard_segment(
                            pattern_bytes,
                            pattern_idx,
                            &mut active,
                            &mut results,
                            true, // use globstar mode for now
                            question_count,
                        )?;
                        pattern_idx = next_pattern_idx;
                        pattern_state = PatternState::Literal;
                        question_count = 0;
                    }
                }
            }
            b'[' => {
                match pattern_state {
                    PatternState::Literal => {
                        // Character class
                        let (charset, class_end) = extract_charset(pattern_bytes, pattern_idx)?;
                        pattern_idx = class_end;

                        // Match charset against all active strings
                        consume_byte(&mut active, &mut results, |b| matches!(b, Some(c) if charset.matches(c)));
                    }
                    PatternState::InWildcard | PatternState::InPossibleGlobstar => {
                        // Trigger wildcard matching
                        let next_pattern_idx = match_wildcard_segment(
                            pattern_bytes,
                            pattern_idx,
                            &mut active,
                            &mut results,
                            false, // wildcard mode
                            question_count,
                        )?;
                        pattern_idx = next_pattern_idx;
                        pattern_state = PatternState::Literal;
                        question_count = 0;
                    }
                    PatternState::InGlobstar => {
                        // Trigger globstar matching
                        let next_pattern_idx = match_wildcard_segment(
                            pattern_bytes,
                            pattern_idx,
                            &mut active,
                            &mut results,
                            true, // globstar mode
                            question_count,
                        )?;
                        pattern_idx = next_pattern_idx;
                        pattern_state = PatternState::Literal;
                        question_count = 0;
                    }
                    PatternState::InSuperWild => {
                        // Trigger super-wild matching
                        let next_pattern_idx = match_wildcard_segment(
                            pattern_bytes,
                            pattern_idx,
                            &mut active,
                            &mut results,
                            true, // use globstar mode for now
                            question_count,
                        )?;
                        pattern_idx = next_pattern_idx;
                        pattern_state = PatternState::Literal;
                        question_count = 0;
                    }
                }
            }
            _ => {
                match pattern_state {
                    PatternState::Literal => {
                        // Regular literal character
                        consume_byte(&mut active, &mut results, |b| b == Some(c));
                        pattern_idx += 1;
                    }
                    PatternState::InWildcard | PatternState::InPossibleGlobstar => {
                        // Trigger wildcard matching
                        let next_pattern_idx = match_wildcard_segment(
                            pattern_bytes,
                            pattern_idx,
                            &mut active,
                            &mut results,
                            false, // wildcard mode
                            question_count,
                        )?;
                        pattern_idx = next_pattern_idx;
                        pattern_state = PatternState::Literal;
                        question_count = 0;
                    }
                    PatternState::InGlobstar => {
                        // Trigger globstar matching
                        let next_pattern_idx = match_wildcard_segment(
                            pattern_bytes,
                            pattern_idx,
                            &mut active,
                            &mut results,
                            true, // globstar mode
                            question_count,
                        )?;
                        pattern_idx = next_pattern_idx;
                        pattern_state = PatternState::Literal;
                        question_count = 0;
                    }
                    PatternState::InSuperWild => {
                        // Trigger super-wild matching
                        let next_pattern_idx = match_wildcard_segment(
                            pattern_bytes,
                            pattern_idx,
                            &mut active,
                            &mut results,
                            true, // use globstar mode for now
                            question_count,
                        )?;
                        pattern_idx = next_pattern_idx;
                        pattern_state = PatternState::Literal;
                        question_count = 0;
                    }
                }
            }
        }
    }

    // Pattern exhausted - handle any remaining wildcard state
    match pattern_state {
        PatternState::Literal => {
            // Normal completion - mark remaining active strings based on completion state
            for string in active {
                // String must be exhausted OR next character is b'/' (directory match)
                results[string.original_idx] = match string.current_byte() {
                    Some(b'/') | None => true,
                    Some(_) => false,
                };
            }
        }
        PatternState::InWildcard | PatternState::InPossibleGlobstar => {
            // Pattern ends with wildcard - match remaining string (no /)
            for string in active.iter_mut() {
                loop {
                    match string.current_byte() {
                        Some(b'/') | None => break,
                        _ => string.advance(),
                    }
                }
                results[string.original_idx] = true;
            }
        }
        PatternState::InGlobstar | PatternState::InSuperWild => {
            // Pattern ends with globstar or super-wild - match everything
            for string in active.iter_mut() {
                string.position = string.bytes.len();
                results[string.original_idx] = true;
            }
        }
    }

    Ok(results)
}

/// Match wildcard followed by next pattern segment (up to next * or end)
///
/// For each string, try to match the pattern segment starting from different positions.
/// Pattern bytes are processed inline during matching - no separate lookahead.
/// - In wildcard mode: can consume any chars except /, enters terminating mode after /
/// - In globstar mode: can consume any chars including /
///
/// `required_chars` specifies the minimum number of non-slash characters that must be
/// consumed by the wildcard before the pattern segment starts matching.
///
/// Failed strings are swap-removed from active and marked false in results.
/// Returns the pattern index after consuming the segment.
fn match_wildcard_segment(
    pattern: &[u8],
    pattern_start: usize,
    active: &mut Vec<ActiveString>,
    results: &mut [bool],
    globstar: bool,
    required_chars: usize,
) -> Result<usize, String> {
    // Patterns ending in globstar or wild
    if pattern_start >= pattern.len() {
        for string in active.iter_mut() {
            if globstar {
                // Globstar: consume everything
                string.position = string.bytes.len();
            } else {
                // Wildcard: consume until / or end
                loop {
                    match string.current_byte() {
                        Some(b'/') | None => break,
                        _ => string.advance(),
                    }
                }
            }
        }
        return Ok(pattern_start);
    }

    // For each string, try to match pattern starting from different positions
    let mut i = 0;
    let mut next_pattern_idx = None; // Computed during first match, reused for all strings

    while i < active.len() {
        let string = &mut active[i];
        let start_pos = string.position;
        let mut matched = false;
        let mut terminating = false;

        // Try matching from different positions in the string
        for try_pos in start_pos..=string.bytes.len() {
            // In wildcard terminating mode, can't try new positions
            if !globstar && terminating {
                break;
            }

            // If question marks were specified after the wildcard, enforce exact count
            if required_chars > 0 {
                // Count non-slash chars immediately preceding try_pos
                let chars_before_try = if globstar {
                    // In globstar mode, count backwards from try_pos until we hit a / or start_pos
                    let mut count = 0;
                    let mut pos = try_pos;
                    while pos > start_pos && string.bytes.get(pos - 1).copied() != Some(b'/') {
                        count += 1;
                        pos -= 1;
                    }
                    count
                } else {
                    // In wildcard mode, all chars from start_pos to try_pos are non-slash
                    try_pos - start_pos
                };

                if chars_before_try < required_chars {
                    // Haven't consumed enough characters yet, keep trying
                    continue;
                } else if chars_before_try > required_chars {
                    // Consumed too many characters without a directory boundary
                    // In non-globstar mode, we can't skip ahead so stop
                    // In globstar mode, continue (might find a / that creates the right boundary)
                    if !globstar {
                        break;
                    }
                    continue;
                }
                // Exactly the right number of chars consumed, proceed with matching
            }

            // In wildcard mode, check if this position starts with /
            // If so, enter terminating mode (this is the last chance to match)
            if !globstar && string.bytes.get(try_pos).copied() == Some(b'/') {
                terminating = true;
            }

            // Process pattern bytes inline until we hit * or end
            let mut pattern_idx = pattern_start;
            let mut string_idx = try_pos;
            let mut segment_matched = true;

            while pattern_idx < pattern.len() && segment_matched {
                match pattern[pattern_idx] {
                    b'\\' => {
                        // Escaped character
                        if pattern_idx + 1 >= pattern.len() {
                            return Err("Pattern ends with backslash".to_string());
                        }
                        pattern_idx += 1;
                        let escaped = pattern[pattern_idx];

                        if string.bytes.get(string_idx).copied() == Some(escaped) {
                            string_idx += 1;
                            pattern_idx += 1;
                        } else {
                            segment_matched = false;
                        }
                    }
                    b'*' => {
                        // Hit next wildcard - segment complete
                        if next_pattern_idx.is_none() {
                            next_pattern_idx = Some(pattern_idx);
                        }
                        break;
                    }
                    b'[' => {
                        // Character class
                        let (charset, class_end) = extract_charset(pattern, pattern_idx)?;
                        pattern_idx = class_end;

                        match string.bytes.get(string_idx).copied() {
                            Some(b) if charset.matches(b) => string_idx += 1,
                            _ => segment_matched = false,
                        }
                    }
                    b'?' => {
                        // Single character wildcard (matches any character except /)
                        pattern_idx += 1;

                        match string.bytes.get(string_idx).copied() {
                            Some(b) if b != b'/' => string_idx += 1,
                            _ => segment_matched = false,
                        }
                    }
                    _ => {
                        // Literal character
                        if string.bytes.get(string_idx).copied() == Some(pattern[pattern_idx]) {
                            string_idx += 1;
                            pattern_idx += 1;
                        } else {
                            segment_matched = false;
                        }
                    }
                }
            }

            // If we exhausted pattern (no * found) and segment matched, that's success
            if segment_matched && pattern_idx >= pattern.len() && next_pattern_idx.is_none() {
                next_pattern_idx = Some(pattern_idx);
            }

            // Check if this trial succeeded
            if segment_matched {
                string.position = string_idx;
                matched = true;
                break;
            }
        }

        if matched {
            // Success - move onto next string
            i += 1;
        } else {
            // Failed - mark result and remove from active
            results[string.original_idx] = false;
            active.swap_remove(i);
            // Don't increment i - check what was swapped in
        }
    }

    // Return the pattern position where we stopped (same for all strings)
    Ok(next_pattern_idx.unwrap_or(pattern.len()))
}

/// Extract character set from pattern starting at '['
///
/// Returns the extracted character set and the next pattern index after the closing bracket
fn extract_charset(pattern: &[u8], start_idx: usize) -> Result<(CharSet, usize), String> {
    if pattern[start_idx] != b'[' {
        return Err("Expected '[' at start of character class".to_string());
    }

    let mut idx = start_idx + 1;
    let mut items = Vec::new();
    let mut negated = false;

    // Check for negation
    if idx < pattern.len() && (pattern[idx] == b'!' || pattern[idx] == b'^') {
        negated = true;
        idx += 1;
    }

    // Extract characters until ]
    while idx < pattern.len() {
        let c = pattern[idx];

        match c {
            b'\\' => {
                // Escape next character
                if idx + 1 >= pattern.len() {
                    return Err("Pattern ends with backslash in character class".to_string());
                }
                idx += 1;
                let escaped = pattern[idx];
                items.push(CharSetItem::Single(escaped));
                idx += 1;
            }
            b']' => {
                if items.is_empty() {
                    return Err("Empty character class".to_string());
                }
                idx += 1;
                return Ok((CharSet { items, negated }, idx));
            }
            _ => {
                // Check for range
                if idx + 2 < pattern.len() && pattern[idx + 1] == b'-' {
                    let start = c;
                    let end = pattern[idx + 2];

                    if end == b']' {
                        // Treat '-]' as literal dash followed by class end
                        items.push(CharSetItem::Single(c));
                        items.push(CharSetItem::Single(b'-'));
                        idx += 2;
                        continue;
                    }

                    if start > end {
                        return Err(format!("Invalid range [{}-{}]", start as char, end as char));
                    }

                    items.push(CharSetItem::Range(start, end));
                    idx += 3;
                } else {
                    items.push(CharSetItem::Single(c));
                    idx += 1;
                }
            }
        }
    }

    Err("Unclosed character class".to_string())
}

#[derive(Debug)]
enum CharSetItem {
    Single(u8),
    Range(u8, u8),
}

#[derive(Debug)]
struct CharSet {
    items: Vec<CharSetItem>,
    negated: bool,
}

impl CharSet {
    fn matches(&self, b: u8) -> bool {
        let contains = self.items.iter().any(|item| match item {
            CharSetItem::Single(c) => *c == b,
            CharSetItem::Range(start, end) => b >= *start && b <= *end,
        });

        if self.negated {
            !contains
        } else {
            contains
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_exact_match() {
        let result = match_batch("abc", &["abc", "axc", "ab"]).unwrap();
        assert_eq!(result, vec![true, false, false]);
    }

    #[test]
    fn test_literal_multiple_strings() {
        let result = match_batch("test", &["test", "TEST", "testing", "test2"]).unwrap();
        assert_eq!(result, vec![true, false, false, false]);
    }

    #[test]
    fn test_wildcard_simple() {
        let result =
            match_batch("*.txt", &["file.txt", "doc.txt", "file.rs", "dir/file.txt"]).unwrap();
        assert_eq!(result, vec![true, true, false, false]);
    }

    #[test]
    fn test_wildcard_with_prefix() {
        let result = match_batch(
            "test*.rs",
            &["test.rs", "test_util.rs", "mytest.rs", "test.txt"],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, false, false]);
    }

    #[test]
    fn test_wildcard_empty_anchor() {
        let result = match_batch("test*", &["test", "testing", "test123", "tes"]).unwrap();
        assert_eq!(result, vec![true, true, true, false]);
    }

    #[test]
    fn test_globstar_simple() {
        let result = match_batch(
            "**/*.rs",
            &["main.rs", "src/lib.rs", "a/b/c.rs", "test.txt"],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, true, false]);
    }

    #[test]
    fn test_globstar_with_prefix() {
        let result = match_batch(
            "src/**/*.rs",
            &["src/main.rs", "src/a/b.rs", "lib/c.rs", "src/test.txt"],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, false, false]);
    }

    #[test]
    fn test_globstar_empty_anchor() {
        let result = match_batch("src/**", &["src/a", "src/a/b/c", "lib/x", "src"]).unwrap();
        assert_eq!(result, vec![true, true, false, false]);
    }

    #[test]
    fn test_escaped_characters() {
        let result = match_batch(r"test\*.txt", &["test*.txt", "test.txt", "testing.txt"]).unwrap();
        assert_eq!(result, vec![true, false, false]);
    }

    #[test]
    fn test_charset_simple() {
        let result =
            match_batch("test[123]", &["test1", "test2", "test3", "test4", "testx"]).unwrap();
        assert_eq!(result, vec![true, true, true, false, false]);
    }

    #[test]
    fn test_charset_range() {
        let result = match_batch(
            "file[0-9].txt",
            &["file0.txt", "file5.txt", "file9.txt", "filea.txt"],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, true, false]);
    }

    #[test]
    fn test_charset_negated() {
        let result = match_batch("test[!abc]", &["testx", "testy", "testa", "testb"]).unwrap();
        assert_eq!(result, vec![true, true, false, false]);
    }

    #[test]
    fn test_directory_prefix_match() {
        let result = match_batch("src", &["src/main.rs", "src/lib", "srcx", "sr"]).unwrap();
        assert_eq!(result, vec![true, true, false, false]);
    }

    #[test]
    fn test_complex_pattern() {
        let pattern = "src/**/*[._]test.rs";
        let strings = vec![
            "src/my_test.rs",
            "src/a/b/util_test.rs",
            "src/lib.test.rs",
            "src/main.rs",
            "lib/test.rs",
        ];
        let result = match_batch(pattern, &strings).unwrap();
        assert_eq!(result, vec![true, true, true, false, false]);
    }

    #[test]
    fn test_empty_strings() {
        let result = match_batch("test", &[]).unwrap();
        assert_eq!(result, Vec::<bool>::new());
    }

    #[test]
    fn test_wildcard_across_slash_boundary() {
        let result = match_batch("*.txt", &["file.txt", "dir/file.txt"]).unwrap();
        assert_eq!(result, vec![true, false]);
    }

    #[test]
    fn test_multiple_wildcards() {
        let result = match_batch(
            "*test*.rs",
            &["mytest.rs", "test_util.rs", "testing_lib.rs", "main.rs"],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, true, false]);
    }

    #[test]
    fn test_globstar_not_crossing_without_slash() {
        // ** without / in anchor should behave like *
        let result = match_batch("**test", &["test", "mytest", "dir/test"]).unwrap();
        assert_eq!(result, vec![true, true, false]);
    }

    #[test]
    fn test_charset_escaped_closing_bracket() {
        let result = match_batch("test[\\]]", &["test]", "test[", "testx"]).unwrap();
        assert_eq!(result, vec![true, false, false]);
    }

    #[test]
    fn test_charset_escaped_dash() {
        let result = match_batch("test[a\\-z]", &["testa", "test-", "testz", "testb"]).unwrap();
        assert_eq!(result, vec![true, true, true, false]);
    }

    #[test]
    fn test_charset_escaped_backslash() {
        let result = match_batch("test[\\\\]", &["test\\", "testa", "testx"]).unwrap();
        assert_eq!(result, vec![true, false, false]);
    }

    // Section 3.5: Anchoring and Directory Matching

    #[test]
    fn test_leading_slash_anchor_root() {
        // Leading / is stripped - pattern matches at root level only
        let result = match_batch(
            "/README.md",
            &["README.md", "dir/README.md", "a/b/README.md"],
        )
        .unwrap();
        assert_eq!(result, vec![true, false, false]);
    }

    #[test]
    fn test_leading_slash_with_wildcard() {
        let result = match_batch("/*.txt", &["file.txt", "test.txt", "dir/file.txt"]).unwrap();
        assert_eq!(result, vec![true, true, false]);
    }

    #[test]
    fn test_leading_slash_with_directory() {
        let result = match_batch("/src/main.rs", &["src/main.rs", "lib/src/main.rs"]).unwrap();
        assert_eq!(result, vec![true, false]);
    }

    #[test]
    fn test_trailing_slash_directory_matching() {
        // Pattern ending in / matches directory and all contents
        let result = match_batch(
            "build/",
            &["build/output.txt", "build/dist/app.js", "buildx/file.txt"],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, false]);
    }

    #[test]
    fn test_trailing_slash_with_globstar() {
        let result = match_batch(
            "**/build/",
            &[
                "build/file.txt",
                "src/build/output.js",
                "a/b/c/build/dist/x.txt",
            ],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, true]);
    }

    #[test]
    fn test_leading_and_trailing_slash() {
        let result = match_batch(
            "/dist/",
            &["dist/bundle.js", "dist/css/main.css", "src/dist/file.txt"],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, false]);
    }

    #[test]
    fn test_escaped_literal_asterisk() {
        // Verify escaping works (already tested elsewhere, but part of 3.5 spec)
        let result = match_batch("\\*.txt", &["*.txt", "file.txt"]).unwrap();
        assert_eq!(result, vec![true, false]);
    }

    // ========== Literal & prefix/suffix behaviour ==========

    #[test]
    fn test_literal_case_sensitive() {
        let result = match_batch(
            "readme.md",
            &["readme.md", "README.md", "docs/readme.md", "readme.mdx"],
        )
        .unwrap();
        assert_eq!(result, vec![true, false, false, false]);
    }

    #[test]
    fn test_literal_path_with_prefix_suffix() {
        let result = match_batch(
            "src/main.rs",
            &[
                "src/main.rs",
                "src/main.rs.bak",
                "a/src/main.rs",
                "src/main.rs/foo",
            ],
        )
        .unwrap();
        assert_eq!(result, vec![true, false, false, true]);
    }

    #[test]
    fn test_empty_pattern() {
        let result = match_batch("", &["", "a", "foo/bar"]).unwrap();
        assert_eq!(result, vec![true, false, false]);
    }

    // ========== Wildcard * (non-globstar, no slash crossing) ==========

    #[test]
    fn test_wildcard_between_literals() {
        let result = match_batch(
            "foo*bar",
            &["foobar", "foo_bar", "fooXXXbar", "foo/bar", "foo"],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, true, false, false]);
    }

    #[test]
    fn test_single_wildcard_pattern() {
        let result = match_batch("*", &["", "a", "foo", "foo/bar"]).unwrap();
        assert_eq!(result, vec![true, true, true, true]);
    }

    #[test]
    fn test_wildcard_extension() {
        let result = match_batch(
            "*.rs",
            &["main.rs", "lib.rs", "src/main.rs", "main.r", ".rs"],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, false, false, true]);
    }

    #[test]
    fn test_wildcard_in_directory_path() {
        let result = match_batch(
            "src/*.rs",
            &["src/main.rs", "src/lib.rs", "src/a/main.rs", "src/.rs"],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, false, true]);
    }

    #[test]
    fn test_wildcard_any_extension() {
        let result = match_batch("*.*", &["a.b", "a.", ".gitignore", "no_dot"]).unwrap();
        assert_eq!(result, vec![true, true, true, false]);
    }

    #[test]
    fn test_wildcard_config_files() {
        let result = match_batch(
            "config.*",
            &["config.toml", "config.json", "config", "configs.toml"],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, false, false]);
    }

    // ========== Globstar ** vs * ==========

    #[test]
    fn test_globstar_rust_files() {
        let result = match_batch(
            "**/*.rs",
            &["main.rs", "src/lib.rs", "a/b/c.rs", "a/b.c", "src/dir/"],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, true, false, false]);
    }

    #[test]
    fn test_globstar_middle_of_path() {
        let result = match_batch(
            "src/**/mod.rs",
            &[
                "src/mod.rs",
                "src/a/mod.rs",
                "src/a/b/mod.rs",
                "src/a/b/mod.rs.bak",
                "lib/mod.rs",
            ],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, true, false, false]);
    }

    #[test]
    fn test_globstar_tests_directory() {
        let result = match_batch(
            "**/tests/*.rs",
            &[
                "tests/test.rs",
                "src/tests/test.rs",
                "src/a/tests/test.rs",
                "src/tests/nested/test.rs",
                "tests/test.txt",
            ],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, true, false, false]);
    }

    #[test]
    fn test_globstar_without_slash_wildcard_semantics() {
        let result =
            match_batch("**.rs", &["main.rs", "src/main.rs", "a/b.rs", "a/b/c.rs"]).unwrap();
        assert_eq!(result, vec![true, false, false, false]);
    }

    #[test]
    fn test_globstar_cargo_toml() {
        let result = match_batch(
            "**/Cargo.toml",
            &[
                "Cargo.toml",
                "src/Cargo.toml",
                "a/b/Cargo.toml",
                "Cargo.toml.bak",
            ],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, true, false]);
    }

    #[test]
    fn test_globstar_directory_prefix() {
        let result = match_batch(
            "src/**",
            &["src", "src/", "src/main.rs", "src/a/b/c", "srcx", "srcx/a"],
        )
        .unwrap();
        assert_eq!(result, vec![false, true, true, true, false, false]);
    }

    // ========== Leading / stripping ==========

    #[test]
    fn test_leading_slash_stripped() {
        let result = match_batch(
            "/src/lib.rs",
            &["src/lib.rs", "a/src/lib.rs", "/src/lib.rs"],
        )
        .unwrap();
        assert_eq!(result, vec![true, false, false]);
    }

    #[test]
    fn test_leading_slash_with_wildcard_root() {
        let result = match_batch("/*", &["foo", "bar", "dir/foo", "/foo"]).unwrap();
        assert_eq!(result, vec![true, true, true, true]);
    }

    #[test]
    fn test_leading_slash_with_globstar_pattern() {
        let result = match_batch(
            "/src/**/*.rs",
            &["src/main.rs", "src/a/b.rs", "lib/src/main.rs"],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, false]);
    }

    // ========== Trailing / stripping & directory prefix semantics ==========

    #[test]
    fn test_trailing_slash_directory_prefix() {
        let result = match_batch(
            "build/",
            &[
                "build",
                "build/",
                "build/output.txt",
                "build/dist/app.js",
                "buildx",
                "buildx/output.txt",
            ],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, true, true, false, false]);
    }

    #[test]
    fn test_trailing_slash_logs_directory() {
        let result = match_batch(
            "logs/",
            &["logs", "logs/", "logs/app.log", "var/logs/app.log"],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, true, false]);
    }

    #[test]
    fn test_directory_prefix_without_trailing_slash() {
        let result = match_batch(
            "src/bin",
            &["src/bin", "src/bin/main.rs", "src/binx", "src/bi"],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, false, false]);
    }

    #[test]
    fn test_leading_and_trailing_slash_dist() {
        let result = match_batch(
            "/dist/",
            &["dist", "dist/app.js", "dist/css/app.css", "src/dist/app.js"],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, true, false]);
    }

    // ========== Mixed * + ** + literals ==========

    #[test]
    fn test_mixed_globstar_wildcard_suffix() {
        let result = match_batch(
            "src/**/tests/*_test.rs",
            &[
                "src/tests/foo_test.rs",
                "src/a/tests/bar_test.rs",
                "src/a/b/tests/baz_test.rs",
                "src/tests/foo.rs",
                "tests/foo_test.rs",
            ],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, true, false, false]);
    }

    #[test]
    fn test_mixed_globstar_with_nested_wildcard() {
        let result = match_batch(
            "**/src/*/*.rs",
            &[
                "src/a/main.rs",
                "a/src/b/main.rs",
                "a/b/src/c/main.rs",
                "src/main.rs",
            ],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, true, false]);
    }

    #[test]
    fn test_mixed_globstar_target_directory() {
        let result = match_batch(
            "**/target/**",
            &[
                "target",
                "target/debug/app",
                "a/target/debug/app",
                "a/b/target",
                "targets/debug/app",
            ],
        )
        .unwrap();
        assert_eq!(result, vec![false, true, true, false, false]);
    }

    // ========== Character classes [ ], ranges, negation ==========

    #[test]
    fn test_charset_double_digit() {
        let result = match_batch(
            "file[0-9][0-9].txt",
            &[
                "file00.txt",
                "file01.txt",
                "file9.txt",
                "fileab.txt",
                "file99.txt",
            ],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, false, false, true]);
    }

    #[test]
    fn test_charset_lowercase_range() {
        let result = match_batch("[a-z].rs", &["a.rs", "z.rs", "A.rs", "aa.rs", "_.rs"]).unwrap();
        assert_eq!(result, vec![true, true, false, false, false]);
    }

    #[test]
    fn test_charset_uppercase_double() {
        let result = match_batch(
            "[A-Z][A-Z].log",
            &["AB.log", "ZZ.log", "A1.log", "A.log", "abc.log"],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, false, false, false]);
    }

    #[test]
    fn test_charset_negated_digit() {
        let result = match_batch(
            "test[!0-9].rs",
            &["testa.rs", "test_.rs", "test0.rs", "test9.rs", "test.rs"],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, false, false, false]);
    }

    #[test]
    fn test_charset_negated_lowercase() {
        let result = match_batch(
            "data[!a-z].bin",
            &["data1.bin", "data_.bin", "dataa.bin", "dataz.bin"],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, false, false]);
    }

    #[test]
    fn test_charset_slash_or_dash() {
        let result = match_batch("path[/-]sep", &["path/sep", "path-sep", "pathxsep"]).unwrap();
        assert_eq!(result, vec![true, true, false]);
    }

    #[test]
    fn test_charset_hex_digit() {
        let result = match_batch(
            "img[0-9a-f].png",
            &["img0.png", "img9.png", "imga.png", "imgf.png", "imgg.png"],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, true, true, false]);
    }

    #[test]
    fn test_charset_negated_exclamation() {
        let err = match_batch(
            "config[!].yml",
            &["config!.yml", "configa.yml", "config1.yml"],
        )
        .expect_err("expected empty character class to error");
        assert!(err.contains("Empty"));
    }

    // ========== Question mark wildcard (single character) ==========

    #[test]
    fn test_question_mark_basic() {
        let result = match_batch("file?.txt", &["file1.txt", "fileA.txt", "file.txt", "file12.txt"]).unwrap();
        assert_eq!(result, vec![true, true, false, false]);
    }

    #[test]
    fn test_question_mark_multiple() {
        let result = match_batch("test??.rs", &["test12.rs", "testab.rs", "test1.rs", "test.rs"]).unwrap();
        assert_eq!(result, vec![true, true, false, false]);
    }

    #[test]
    fn test_question_mark_with_wildcard() {
        let result = match_batch("*.?s", &["file.rs", "test.ts", "doc.js", "app.css"]).unwrap();
        assert_eq!(result, vec![true, true, true, false]);
    }

    #[test]
    fn test_question_mark_no_slash() {
        // ? should not match /
        let result = match_batch("dir?file.txt", &["dirXfile.txt", "dir/file.txt", "dirfile.txt"]).unwrap();
        assert_eq!(result, vec![true, false, false]);
    }

    #[test]
    fn test_question_mark_at_end() {
        let result = match_batch("test.rs?", &["test.rs1", "test.rsx", "test.rs", "test.rs/x"]).unwrap();
        assert_eq!(result, vec![true, true, false, false]);
    }

    #[test]
    fn test_question_mark_at_start() {
        let result = match_batch("?est.txt", &["test.txt", "rest.txt", "est.txt", "/est.txt"]).unwrap();
        assert_eq!(result, vec![true, true, false, false]);
    }

    #[test]
    fn test_question_mark_with_globstar() {
        let result = match_batch("src/**/??.rs", &["src/ab.rs", "src/mod/xy.rs", "src/a.rs", "src/abc.rs"]).unwrap();
        assert_eq!(result, vec![true, true, false, false]);
    }

    #[test]
    fn test_question_mark_with_charset() {
        let result = match_batch("file[0-9]?.txt", &["file00.txt", "file0a.txt", "file0.txt", "file01.txt"]).unwrap();
        assert_eq!(result, vec![true, true, false, true]);
    }

    #[test]
    fn test_question_mark_directory_boundary() {
        let result = match_batch("src?main.rs", &["srcXmain.rs", "src/main.rs", "srcmain.rs"]).unwrap();
        assert_eq!(result, vec![true, false, false]);
    }

    #[test]
    fn test_escaped_question_mark() {
        let result = match_batch("file\\?.txt", &["file?.txt", "fileX.txt", "file.txt"]).unwrap();
        assert_eq!(result, vec![true, false, false]);
    }

    #[test]
    fn test_question_mark_all_positions() {
        let result = match_batch("?a?b?", &["1a2b3", "xaybz", "ab", "1a2b"]).unwrap();
        assert_eq!(result, vec![true, true, false, false]);
    }

    // ========== Escaping within charsets and literals ==========

    #[test]
    fn test_charset_escaped_open_bracket() {
        let result = match_batch("foo[\\[]bar", &["foo[bar", "foo]bar", "foo\\bar"]).unwrap();
        assert_eq!(result, vec![true, false, false]);
    }

    #[test]
    fn test_charset_escaped_close_bracket() {
        let result = match_batch("foo[\\]]bar", &["foo]bar", "foo[bar", "foobar"]).unwrap();
        assert_eq!(result, vec![true, false, false]);
    }

    #[test]
    fn test_charset_escaped_dash_literal() {
        let result =
            match_batch("range[a\\-c]", &["rangea", "range-", "rangec", "rangeb"]).unwrap();
        assert_eq!(result, vec![true, true, true, false]);
    }

    #[test]
    fn test_charset_escaped_backslash_literal() {
        let result = match_batch(
            "backslash[\\\\]end",
            &["backslash\\end", "backslash/end", "backslashxend"],
        )
        .unwrap();
        assert_eq!(result, vec![true, false, false]);
    }

    #[test]
    fn test_literal_escaped_asterisk() {
        let result = match_batch(
            "literal\\*star",
            &["literal*star", "literal\\*star", "literalXstar"],
        )
        .unwrap();
        assert_eq!(result, vec![true, false, false]);
    }

    #[test]
    fn test_literal_escaped_brackets() {
        let result = match_batch("dir\\[test\\]", &["dir[test]", "dirXtest]", "dir[test"]).unwrap();
        assert_eq!(result, vec![true, false, false]);
    }

    // ========== Invalid / error-case patterns ==========

    #[test]
    fn test_error_trailing_backslash_only() {
        let err = match_batch("\\", &["x"]).expect_err("expected trailing backslash error");
        assert!(err.contains("backslash"));
    }

    #[test]
    fn test_error_trailing_backslash() {
        let err =
            match_batch("foo\\", &["foo\\", "foo"]).expect_err("expected trailing backslash error");
        assert!(err.contains("backslash"));
    }

    #[test]
    fn test_error_unclosed_range() {
        let err = match_batch("[a-", &["a"]).expect_err("expected unclosed range error");
        assert!(err.contains("Unclosed") || err.contains("range") || err.contains("ends with '-'"));
    }

    #[test]
    fn test_error_invalid_range_order() {
        let err = match_batch("[z-a]", &["m"]).expect_err("expected invalid range order error");
        assert!(err.contains("Invalid range"));
    }

    #[test]
    fn test_error_unclosed_charset() {
        let err = match_batch("foo[", &["foo["]).expect_err("expected unclosed charset error");
        assert!(err.contains("Unclosed"));
    }

    #[test]
    fn test_error_charset_trailing_backslash() {
        let err = match_batch("foo[\\]", &["foo\\"])
            .expect_err("expected charset trailing backslash error");
        assert!(err.contains("backslash") || err.contains("Unclosed"));
    }

    #[test]
    fn test_error_charset_only_negation() {
        let err = match_batch("[!]", &["!"]).expect_err("expected negation-only charset error");
        assert!(err.contains("Empty"));
    }

    // ========== Mixed directory & charsets ==========

    #[test]
    fn test_charset_in_directory_name() {
        let result = match_batch(
            "src/[a-z]*/mod.rs",
            &[
                "src/a/mod.rs",
                "src/abc/mod.rs",
                "src/A/mod.rs",
                "src//mod.rs",
                "src/mod.rs",
            ],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, false, false, false]);
    }

    #[test]
    fn test_charset_negated_in_filename() {
        let result = match_batch(
            "src/[!t]est.rs",
            &["src/aest.rs", "src/test.rs", "src/zest.rs"],
        )
        .unwrap();
        assert_eq!(result, vec![true, false, true]);
    }

    #[test]
    fn test_charset_with_globstar() {
        let result = match_batch(
            "[a-z]/**/main.rs",
            &["a/main.rs", "a/src/main.rs", "z/a/b/main.rs", "A/main.rs"],
        )
        .unwrap();
        assert_eq!(result, vec![true, true, true, false]);
    }
}
