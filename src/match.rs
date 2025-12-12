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

/// Match multiple strings against a single glob pattern
///
/// Matching is done on byte arrays as control characters are all single-byte ASCII
/// characters. Any other characters will need to match the pattern segments byte
/// for byte anyway, so we can avoid converting strings to chars.
/// 
/// Returns a `Vec<bool>` indicating which strings matched (`true`) or failed (`false`)
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

    let pattern_bytes: &[u8] = pattern.as_bytes();
    let mut pattern_idx: usize = 0;

    while pattern_idx < pattern_bytes.len() && !active.is_empty() {
        let c: u8 = pattern_bytes[pattern_idx];

        match c {
            b'\\' => {
                // Escape next character
                if pattern_idx + 1 >= pattern_bytes.len() {
                    return Err("Pattern ends with backslash".to_string());
                }
                pattern_idx += 1;
                let escaped: u8 = pattern_bytes[pattern_idx];
                
                // Match literal byte against all active strings
                let mut i: usize = 0;
                while i < active.len() {
                    let string: &mut ActiveString<'_> = &mut active[i];
                    
                    match string.current_byte() {
                        Some(b) if b == escaped => {
                            // Still matching - advance position
                            string.advance();
                            i += 1;
                        }
                        _ => {
                            // Failed - mark result and remove from active
                            results[string.original_idx] = false;
                            active.swap_remove(i);
                            // Don't increment i - check what was swapped in
                        }
                    }
                }
                
                pattern_idx += 1;
            }
            b'*' => {
                // Check for globstar
                if pattern_idx + 1 < pattern_bytes.len() && pattern_bytes[pattern_idx + 1] == b'*' {
                    // Globstar **
                    pattern_idx += 2;
                    
                    // Skip any additional * 
                    // Unlikely but also unclear what to do if we encountered so just treat as **
                    while pattern_idx < pattern_bytes.len() && pattern_bytes[pattern_idx] == b'*' {
                        pattern_idx += 1;
                    }

                    // Check if followed by / (true globstar - crosses directories)
                    if pattern_idx < pattern_bytes.len() && pattern_bytes[pattern_idx] == b'/' {
                        // Skip the / (don't include in anchor)
                        pattern_idx += 1;
                        
                        // Skip any additional / or * (like **/ or **/**)
                        while pattern_idx < pattern_bytes.len() && 
                              (pattern_bytes[pattern_idx] == b'/' || pattern_bytes[pattern_idx] == b'*') {
                            pattern_idx += 1;
                        }
                        
                        // Match globstar with next segment (up to next *)
                        let next_pattern_idx = match_wildcard_segment(
                            pattern_bytes,
                            pattern_idx,
                            &mut active,
                            &mut results,
                            true, // globstar mode
                        )?;
                        
                        pattern_idx = next_pattern_idx;
                    } else {
                        // ** NOT followed by / - use wildcard semantics (doesn't cross directories)
                        let next_pattern_idx = match_wildcard_segment(
                            pattern_bytes,
                            pattern_idx,
                            &mut active,
                            &mut results,
                            false, // wildcard mode
                        )?;
                        
                        pattern_idx = next_pattern_idx;
                    }
                } else {
                    // Single wildcard *
                    pattern_idx += 1;

                    // Match wildcard with next segment (up to next *)
                    let next_pattern_idx = match_wildcard_segment(
                        pattern_bytes,
                        pattern_idx,
                        &mut active,
                        &mut results,
                        false, // wildcard mode
                    )?;
                    
                    pattern_idx = next_pattern_idx;
                }
            }
            b'[' => {
                // Character class
                let (charset, class_end) = extract_charset(pattern_bytes, pattern_idx)?;
                pattern_idx = class_end;
                
                // Match charset against all active strings
                let mut i: usize = 0;
                while i < active.len() {
                    let string: &mut ActiveString<'_> = &mut active[i];
                    
                    match string.current_byte() {
                        Some(b) if charset.matches(b) => {
                            // Still matching - advance position
                            string.advance();
                            i += 1;
                        }
                        _ => {
                            // Failed - mark result and remove from active
                            results[string.original_idx] = false;
                            active.swap_remove(i);
                            // Don't increment i - check what was swapped in
                        }
                    }
                }
            }
            _ => {
                // Regular literal character
                let mut i: usize = 0;
                while i < active.len() {
                    let string: &mut ActiveString<'_> = &mut active[i];
                    
                    match string.current_byte() {
                        Some(b) if b == c => {
                            // Still matching - advance position
                            string.advance();
                            i += 1;
                        }
                        _ => {
                            // Failed - mark result and remove from active
                            results[string.original_idx] = false;
                            active.swap_remove(i);
                            // Don't increment i - check what was swapped in
                        }
                    }
                }
                
                pattern_idx += 1;
            }
        }
    }

    // Pattern exhausted - mark remaining active strings based on completion state
    for string in active {
        // String must be exhausted OR next character is b'/' (directory match)
        results[string.original_idx] = match string.current_byte() {
            None => true,       // String exhausted
            Some(b'/') => true, // Directory match
            Some(_) => false,   // String has more chars but not /
        };
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
/// Failed strings are swap-removed from active and marked false in results.
/// Returns the pattern index after consuming the segment.
fn match_wildcard_segment(
    pattern: &[u8],
    pattern_start: usize,
    active: &mut Vec<ActiveString>,
    results: &mut [bool],
    globstar: bool,
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
    let mut next_pattern_idx = None;  // Computed during first match, reused for all strings
    
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
/// Returns (charset, next_pattern_index)
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
                idx += 1;
                return Ok((CharSet { items, negated }, idx));
            }
            _ => {
                // Check for range
                if idx + 2 < pattern.len() && pattern[idx + 1] == b'-' {
                    if pattern[idx + 2] == b']' {
                        return Err(format!("Incomplete range [{}-]", c as char));
                    }

                    let start = c;
                    let end = pattern[idx + 2];
                    
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
        let result = match_batch("*.txt", &["file.txt", "doc.txt", "file.rs", "dir/file.txt"]).unwrap();
        assert_eq!(result, vec![true, true, false, false]);
    }

    #[test]
    fn test_wildcard_with_prefix() {
        let result = match_batch("test*.rs", &["test.rs", "test_util.rs", "mytest.rs", "test.txt"]).unwrap();
        assert_eq!(result, vec![true, true, false, false]);
    }

    #[test]
    fn test_wildcard_empty_anchor() {
        let result = match_batch("test*", &["test", "testing", "test123", "tes"]).unwrap();
        assert_eq!(result, vec![true, true, true, false]);
    }

    #[test]
    fn test_globstar_simple() {
        let result = match_batch("**/*.rs", &["main.rs", "src/lib.rs", "a/b/c.rs", "test.txt"]).unwrap();
        assert_eq!(result, vec![true, true, true, false]);
    }

    #[test]
    fn test_globstar_with_prefix() {
        let result = match_batch("src/**/*.rs", &["src/main.rs", "src/a/b.rs", "lib/c.rs", "src/test.txt"]).unwrap();
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
        let result = match_batch("test[123]", &["test1", "test2", "test3", "test4", "testx"]).unwrap();
        assert_eq!(result, vec![true, true, true, false, false]);
    }

    #[test]
    fn test_charset_range() {
        let result = match_batch("file[0-9].txt", &["file0.txt", "file5.txt", "file9.txt", "filea.txt"]).unwrap();
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
        let result = match_batch("*test*.rs", &["mytest.rs", "test_util.rs", "testing_lib.rs", "main.rs"]).unwrap();
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
}
