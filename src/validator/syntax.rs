pub fn syntax_check_command(language: &str) -> Option<(&'static str, Vec<&'static str>)> {
    match language {
        "javascript" | "js" => Some(("node", vec!["--check"])),
        "typescript" | "ts" => Some(("npx", vec!["tsc", "--noEmit"])),
        "rust" => Some(("cargo", vec!["check", "-j2"])),
        "python" | "py" => Some(("python", vec!["-m", "py_compile"])),
        "go" => Some(("go", vec!["build", "."])),
        _ => None,
    }
}

pub async fn run_syntax_check(language: &str, dir: &std::path::Path) -> Result<bool, std::io::Error> {
    let Some((cmd, args)) = syntax_check_command(language) else {
        return Ok(true);
    };
    let output = tokio::process::Command::new(cmd)
        .args(&args)
        .current_dir(dir)
        .output()
        .await?;
    Ok(output.status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syntax_command_for_js() {
        let cmd = syntax_check_command("javascript");
        assert!(cmd.is_some());
        let (bin, args) = cmd.unwrap();
        assert_eq!(bin, "node");
        assert_eq!(args, vec!["--check"]);
    }

    #[test]
    fn test_syntax_command_for_rust() {
        let cmd = syntax_check_command("rust");
        assert!(cmd.is_some());
        let (bin, _) = cmd.unwrap();
        assert_eq!(bin, "cargo");
    }

    #[test]
    fn test_syntax_command_for_unknown() {
        assert!(syntax_check_command("brainfuck").is_none());
    }
}
