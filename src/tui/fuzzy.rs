//! Subsequence fuzzy matching and scoring engine (fzf-inspired).

/// Result of a fuzzy match operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzyMatch {
    /// Match score (higher is a better match).
    pub score: i64,
    /// 0-indexed byte/char positions in `target` that matched the pattern.
    pub matched_indices: Vec<usize>,
}

/// Matches `pattern` against `target` using subsequence fuzzy matching.
///
/// Returns `Some(FuzzyMatch)` if all characters of `pattern` appear in `target`
/// in sequence (case-insensitive), or `None` if they do not.
pub fn fuzzy_match(pattern: &str, target: &str) -> Option<FuzzyMatch> {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return Some(FuzzyMatch {
            score: 0,
            matched_indices: Vec::new(),
        });
    }

    let target_chars: Vec<char> = target.chars().collect();
    let target_lower: Vec<char> = target.to_lowercase().chars().collect();
    let pattern_lower: Vec<char> = pattern.to_lowercase().chars().collect();

    let mut matched_indices = Vec::with_capacity(pattern_lower.len());
    let mut target_idx = 0;

    for &p_char in &pattern_lower {
        let mut found = false;
        while target_idx < target_lower.len() {
            if target_lower[target_idx] == p_char {
                matched_indices.push(target_idx);
                target_idx += 1;
                found = true;
                break;
            }
            target_idx += 1;
        }
        if !found {
            return None;
        }
    }

    // Scoring calculation (fzf-inspired):
    let mut score: i64 = 100;

    // Exact match bonus
    if pattern.eq_ignore_ascii_case(target) {
        score += 250;
    }

    // Prefix match bonus
    if matched_indices.first() == Some(&0) {
        score += 60;
    }

    for (i, &idx) in matched_indices.iter().enumerate() {
        // Consecutive character match bonus
        if i > 0 && idx == matched_indices[i - 1] + 1 {
            score += 25;
        }

        // Word boundary bonuses
        if idx == 0 {
            score += 35;
        } else {
            let prev = target_chars[idx - 1];
            if prev == ' ' || prev == '-' || prev == '_' || prev == '/' || prev == ':' || prev == '.' {
                score += 35;
            } else if target_chars[idx].is_uppercase() && !prev.is_uppercase() {
                // CamelCase boundary (e.g. PromptEngineering)
                score += 30;
            }
        }
    }

    // Penalize target length distance to prioritize tighter matches
    let len_diff = (target_chars.len() as i64) - (pattern_lower.len() as i64);
    score -= len_diff.max(0) * 2;

    Some(FuzzyMatch {
        score,
        matched_indices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_match_empty_pattern() {
        let result = fuzzy_match("", "Mathematics");
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.score, 0);
        assert!(m.matched_indices.is_empty());
    }

    #[test]
    fn test_fuzzy_match_exact() {
        let result = fuzzy_match("Mathematics", "Mathematics");
        assert!(result.is_some());
        let m = result.unwrap();
        assert!(m.score > 300);
        assert_eq!(m.matched_indices.len(), 11);
    }

    #[test]
    fn test_fuzzy_match_subsequence_and_case_insensitive() {
        let result = fuzzy_match("math", "Advanced Mathematics 101");
        assert!(result.is_some());
        let m = result.unwrap();
        // 'm', 'a', 't', 'h' in "Mathematics" starting at index 9
        assert_eq!(m.matched_indices, vec![9, 10, 11, 12]);
    }

    #[test]
    fn test_fuzzy_match_word_boundaries_rank_higher() {
        // "pe" matching "Prompt Engineering" at word boundaries P and E
        let m_pe = fuzzy_match("pe", "Prompt Engineering").unwrap();
        // "pe" matching "Supercomputer" (p and e are internal)
        let m_sc = fuzzy_match("pe", "Supercomputer").unwrap();

        assert!(
            m_pe.score > m_sc.score,
            "Word boundary matches should score higher: {} vs {}",
            m_pe.score,
            m_sc.score
        );
    }

    #[test]
    fn test_fuzzy_match_non_matching() {
        assert!(fuzzy_match("xyz", "Mathematics").is_none());
        assert!(fuzzy_match("physics", "Prompt Engineering").is_none());
    }
}
