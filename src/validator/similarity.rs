use std::collections::HashSet;

use crate::SimilarityReport;

pub fn token_jaccard(a: &str, b: &str) -> f64 {
    let set_a: HashSet<&str> = a.split_whitespace().collect();
    let set_b: HashSet<&str> = b.split_whitespace().collect();
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 {
        return 0.0;
    }
    intersection as f64 / union as f64
}

pub fn function_name_overlap(original: &[String], generated: &[String]) -> f64 {
    let set_a: HashSet<&str> = original.iter().map(|s| s.as_str()).collect();
    let set_b: HashSet<&str> = generated.iter().map(|s| s.as_str()).collect();
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 {
        return 0.0;
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

/// Lightweight structural similarity based on code shape.
/// Counts structural patterns (braces, brackets, function definitions,
/// control flow keywords) and compares distributions.
pub fn structural_similarity(original: &str, generated: &str) -> f64 {
    let orig_shape = code_shape(original);
    let gen_shape = code_shape(generated);

    if orig_shape.is_empty() && gen_shape.is_empty() {
        return 0.0;
    }

    // Jaccard on structural tokens
    let set_a: HashSet<&str> = orig_shape.iter().map(|s| s.as_str()).collect();
    let set_b: HashSet<&str> = gen_shape.iter().map(|s| s.as_str()).collect();

    let intersection = set_a.intersection(&set_b).count() as f64;
    let union = set_a.union(&set_b).count() as f64;

    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn code_shape(code: &str) -> Vec<String> {
    let mut shapes = Vec::new();
    for line in code.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("//")
            || trimmed.starts_with("/*")
            || trimmed.starts_with('*')
        {
            continue;
        }
        // Normalize to structural pattern
        let pattern: String = trimmed
            .replace(|c: char| c.is_alphanumeric() || c == '_', "")
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        if !pattern.is_empty() {
            shapes.push(pattern);
        }
    }
    shapes
}

/// Fetch original package source code for similarity comparison.
/// This runs ONLY in the validator, AFTER Agent B has finished.
/// The source code is NEVER shown to any agent.
pub async fn fetch_original_source(
    name: &str,
    version: &str,
    ecosystem: &crate::Ecosystem,
) -> Option<String> {
    let client = reqwest::Client::new();
    match ecosystem {
        crate::Ecosystem::Npm => {
            // Fetch the tarball from npm, extract .js files
            let url = format!(
                "https://registry.npmjs.org/{}/-/{}-{}.tgz",
                name, name, version
            );
            let resp = client.get(&url).send().await.ok()?;
            if !resp.status().is_success() {
                return None;
            }
            let bytes = resp.bytes().await.ok()?;
            extract_js_from_tarball(&bytes)
        }
        _ => None, // Other ecosystems: similarity scoring deferred
    }
}

fn extract_js_from_tarball(bytes: &[u8]) -> Option<String> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let decoder = GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(decoder);
    let mut source = String::new();

    for entry in archive.entries().ok()? {
        let mut entry = entry.ok()?;
        let path = entry.path().ok()?.to_string_lossy().to_string();
        // Only extract .js source files (not .d.ts, not test files)
        if path.ends_with(".js") && !path.contains("test") && !path.contains("spec") {
            let mut content = String::new();
            entry.read_to_string(&mut content).ok()?;
            source.push_str(&content);
            source.push('\n');
        }
    }

    if source.is_empty() {
        None
    } else {
        Some(source)
    }
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
    let struct_sim = structural_similarity(original_code, generated_code);
    let overall_score =
        0.35 * token_similarity + 0.25 * string_overlap + 0.15 * name_overlap + 0.25 * struct_sim;
    SimilarityReport {
        token_similarity,
        name_overlap,
        string_overlap,
        structural_similarity: struct_sim,
        overall_score,
        threshold,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_jaccard_identical() {
        let score = token_jaccard(
            "function add(a, b) { return a + b; }",
            "function add(a, b) { return a + b; }",
        );
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
        let report = compute_similarity(
            "function add(a, b) { return a + b; }",
            "function add(x, y) { return x + y; }",
            &["add".into()],
            &["add".into()],
            0.70,
        );
        assert!(report.overall_score < 0.70);
    }

    #[test]
    fn test_structural_similarity_identical() {
        let code = "function add(a, b) {\n  return a + b;\n}";
        let score = structural_similarity(code, code);
        assert!((score - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_structural_similarity_different() {
        let a = "function add(a, b) { return a + b; }";
        let b = "if (x) { for (i = 0; i < 10; i++) { console.log(i); } }";
        let score = structural_similarity(a, b);
        assert!(score < 1.0);
    }

    #[test]
    fn test_structural_similarity_empty() {
        let score = structural_similarity("", "");
        assert!((score - 0.0).abs() < 0.001);
    }
}
