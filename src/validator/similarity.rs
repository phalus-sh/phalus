use std::collections::HashSet;

use crate::SimilarityReport;

pub fn token_jaccard(a: &str, b: &str) -> f64 {
    let set_a: HashSet<&str> = a.split_whitespace().collect();
    let set_b: HashSet<&str> = b.split_whitespace().collect();
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 {
        return 1.0;
    }
    intersection as f64 / union as f64
}

pub fn function_name_overlap(original: &[String], generated: &[String]) -> f64 {
    let set_a: HashSet<&str> = original.iter().map(|s| s.as_str()).collect();
    let set_b: HashSet<&str> = generated.iter().map(|s| s.as_str()).collect();
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 {
        return 1.0;
    }
    intersection as f64 / union as f64
}

pub fn extract_string_literals(code: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut chars = code.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' {
            let mut s = String::new();
            loop {
                match chars.next() {
                    Some('\\') => {
                        // skip escaped character
                        chars.next();
                    }
                    Some('"') => break,
                    Some(ch) => s.push(ch),
                    None => break,
                }
            }
            results.push(s);
        }
    }
    results
}

pub fn string_literal_overlap(a: &str, b: &str) -> f64 {
    let lits_a: HashSet<String> = extract_string_literals(a).into_iter().collect();
    let lits_b: HashSet<String> = extract_string_literals(b).into_iter().collect();
    let intersection = lits_a.intersection(&lits_b).count();
    let union = lits_a.union(&lits_b).count();
    if union == 0 {
        // Neither side has string literals; treat as no signal (neutral zero)
        return 0.0;
    }
    intersection as f64 / union as f64
}

pub fn compute_similarity(
    original_code: &str,
    generated_code: &str,
    original_names: &[String],
    generated_names: &[String],
    threshold: f64,
) -> SimilarityReport {
    let token_similarity = token_jaccard(original_code, generated_code);
    let name_overlap = function_name_overlap(original_names, generated_names);
    let string_overlap = string_literal_overlap(original_code, generated_code);
    let overall_score = 0.45 * token_similarity + 0.35 * string_overlap + 0.20 * name_overlap;
    SimilarityReport {
        token_similarity,
        name_overlap,
        string_overlap,
        overall_score,
        threshold,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_jaccard_identical() {
        let score = token_jaccard("function add(a, b) { return a + b; }", "function add(a, b) { return a + b; }");
        assert!((score - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_token_jaccard_completely_different() {
        let score = token_jaccard("alpha beta gamma", "delta epsilon zeta");
        assert!(score < 0.01);
    }

    #[test]
    fn test_token_jaccard_partial() {
        let score = token_jaccard("hello world foo", "hello world bar");
        assert!(score > 0.3);
        assert!(score < 0.8);
    }

    #[test]
    fn test_name_overlap() {
        let original = &["add".into(), "subtract".into(), "multiply".into()];
        let generated = &["add".into(), "subtract".into(), "divide".into()];
        let overlap = function_name_overlap(original, generated);
        assert!((overlap - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_string_overlap() {
        let a = "\"hello\" and \"world\" are strings";
        let b = "\"hello\" and \"foo\" are different";
        let overlap = string_literal_overlap(a, b);
        assert!(overlap > 0.0);
        assert!(overlap < 1.0);
    }

    #[test]
    fn test_overall_score() {
        let report = compute_similarity("function add(a, b) { return a + b; }", "function add(x, y) { return x + y; }", &["add".into()], &["add".into()], 0.70);
        assert!(report.overall_score < 0.70);
    }
}
