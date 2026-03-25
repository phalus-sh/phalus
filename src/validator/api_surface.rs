pub fn check_api_surface(expected_exports: &[String], generated_code: &str) -> f64 {
    if expected_exports.is_empty() {
        return 1.0;
    }
    let found = expected_exports
        .iter()
        .filter(|name| generated_code.contains(name.as_str()))
        .count();
    found as f64 / expected_exports.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_exports_present() {
        let exports = vec!["add".into(), "subtract".into()];
        let code = "function add(a, b) {}\nfunction subtract(a, b) {}";
        assert!((check_api_surface(&exports, code) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_partial_exports() {
        let exports = vec!["add".into(), "subtract".into()];
        let code = "function add(a, b) {}";
        assert!((check_api_surface(&exports, code) - 0.5).abs() < 0.001);
    }
}
