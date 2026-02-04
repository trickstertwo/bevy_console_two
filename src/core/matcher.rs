//! Fuzzy string matching for autocomplete.
//!
//! Zero-dependency subsequence matcher with scoring.

/// Result of a fuzzy match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchResult {
    /// The match score (higher is better).
    pub score: i32,
    /// Indices of matched characters in the text.
    pub indices: Vec<usize>,
}

impl MatchResult {
    /// Create a new match result.
    pub fn new(score: i32, indices: Vec<usize>) -> Self {
        Self { score, indices }
    }
}

/// Perform fuzzy subsequence matching.
///
/// Returns `Some(MatchResult)` if all characters in `pattern` appear
/// in `text` in order (not necessarily consecutive).
///
/// # Scoring
///
/// - Base score: +1 per matched character
/// - Consecutive match bonus: +10
/// - Word start bonus: +5 (after `_`, space, or at start)
/// - Exact prefix bonus: +20
///
/// # Examples
///
/// ```
/// use bevy_console::core::subsequence_match;
///
/// // Exact match
/// let result = subsequence_match("help", "help").unwrap();
/// assert!(result.score > 0);
///
/// // Subsequence match
/// let result = subsequence_match("sgr", "sv_gravity").unwrap();
/// assert_eq!(result.indices, vec![0, 3, 4]); // s, g, r
///
/// // No match
/// assert!(subsequence_match("xyz", "hello").is_none());
/// ```
pub fn subsequence_match(pattern: &str, text: &str) -> Option<MatchResult> {
    if pattern.is_empty() {
        return Some(MatchResult::new(0, Vec::new()));
    }

    let pattern_bytes = pattern.as_bytes();
    let text_bytes = text.as_bytes();

    let mut score = 0i32;
    let mut indices = Vec::with_capacity(pattern_bytes.len());
    let mut pattern_idx = 0;
    let mut prev_match_idx: Option<usize> = None;

    // Check for exact prefix match bonus
    if text.to_ascii_lowercase().starts_with(&pattern.to_ascii_lowercase()) {
        score += 20;
    }

    for (i, &text_char) in text_bytes.iter().enumerate() {
        if pattern_idx >= pattern_bytes.len() {
            break;
        }

        let pattern_char = pattern_bytes[pattern_idx];

        if text_char.eq_ignore_ascii_case(&pattern_char) {
            indices.push(i);

            // Base score for match
            score += 1;

            // Consecutive match bonus
            if let Some(prev) = prev_match_idx {
                if prev == i - 1 {
                    score += 10;
                }
            }

            // Word start bonus (after _, space, or at start)
            if i == 0 {
                score += 5;
            } else {
                let prev_char = text_bytes[i - 1];
                if prev_char == b'_' || prev_char == b' ' || prev_char == b'.' {
                    score += 5;
                }
            }

            prev_match_idx = Some(i);
            pattern_idx += 1;
        }
    }

    // All pattern characters must be matched
    if pattern_idx == pattern_bytes.len() {
        Some(MatchResult::new(score, indices))
    } else {
        None
    }
}

/// Match and sort multiple candidates by score.
///
/// Returns candidates sorted by score (highest first), with their match results.
pub fn match_and_sort<'a>(
    pattern: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Vec<(&'a str, MatchResult)> {
    let mut matches: Vec<_> = candidates
        .into_iter()
        .filter_map(|candidate| {
            subsequence_match(pattern, candidate).map(|result| (candidate, result))
        })
        .collect();

    // Sort by score descending, then alphabetically for ties
    matches.sort_by(|a, b| {
        b.1.score.cmp(&a.1.score).then_with(|| a.0.cmp(b.0))
    });

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let result = subsequence_match("help", "help").unwrap();
        assert_eq!(result.indices, vec![0, 1, 2, 3]);
        // Should have prefix bonus + consecutive bonuses
        assert!(result.score > 4);
    }

    #[test]
    fn test_subsequence_match() {
        let result = subsequence_match("sgr", "sv_gravity").unwrap();
        assert_eq!(result.indices, vec![0, 3, 4]); // s, g, r
    }

    #[test]
    fn test_no_match() {
        assert!(subsequence_match("xyz", "hello").is_none());
        assert!(subsequence_match("abc", "ab").is_none()); // Pattern longer than text
    }

    #[test]
    fn test_empty_pattern() {
        let result = subsequence_match("", "anything").unwrap();
        assert_eq!(result.score, 0);
        assert!(result.indices.is_empty());
    }

    #[test]
    fn test_case_insensitive() {
        assert!(subsequence_match("HELP", "help").is_some());
        assert!(subsequence_match("help", "HELP").is_some());
        assert!(subsequence_match("HeLp", "hElP").is_some());
    }

    #[test]
    fn test_word_boundary_bonus() {
        // sv_g should score higher than svg because g is at word boundary
        let result1 = subsequence_match("svg", "sv_gravity").unwrap();
        let result2 = subsequence_match("svg", "saving").unwrap();

        // sv_gravity has word boundary bonus for 'g' after '_'
        assert!(result1.score > result2.score);
    }

    #[test]
    fn test_consecutive_bonus() {
        // "hel" in "help" (consecutive) should score higher than in "h_e_l_p"
        let result1 = subsequence_match("hel", "help").unwrap();
        let result2 = subsequence_match("hel", "h_e_l_p").unwrap();

        assert!(result1.score > result2.score);
    }

    #[test]
    fn test_prefix_bonus() {
        // "sv" at start of "sv_cheats" should score higher than "sv" in "csv_data"
        let result1 = subsequence_match("sv", "sv_cheats").unwrap();
        let result2 = subsequence_match("sv", "csv_data").unwrap();

        assert!(result1.score > result2.score);
    }

    #[test]
    fn test_match_and_sort() {
        let candidates = [
            "sv_gravity",
            "sv_cheats",
            "cl_showfps",
            "saving",
        ];

        let results = match_and_sort("sv", candidates);

        assert_eq!(results.len(), 3); // cl_showfps doesn't match
        // sv_* should come first due to prefix match
        assert!(results[0].0.starts_with("sv_"));
        assert!(results[1].0.starts_with("sv_"));
    }
}
