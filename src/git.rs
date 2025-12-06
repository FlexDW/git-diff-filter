//! Git command execution and output parsing.

use std::process::Command;

/// Get the list of files changed between base_ref and HEAD
pub fn get_changed_files(base_ref: &str) -> Result<Vec<String>, String> {
    let output = execute_git_diff(base_ref)?;
    parse_git_output(&output)
}

/// Execute git diff command and return stdout
fn execute_git_diff(base_ref: &str) -> Result<String, String> {
    let output = Command::new("git")
        .args(["diff", "--name-only", &format!("{}..HEAD", base_ref)])
        .output()
        .map_err(|e| format!("Failed to execute git command: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Git command failed: {}", stderr.trim()));
    }

    String::from_utf8(output.stdout)
        .map_err(|e| format!("Failed to parse git output as UTF-8: {}", e))
}

/// Parse git diff output into a list of file paths
fn parse_git_output(output: &str) -> Result<Vec<String>, String> {
    Ok(output
        .lines()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_git_output_single_file() {
        let output = "file.txt\n";
        let result = parse_git_output(output).unwrap();
        assert_eq!(result, vec!["file.txt"]);
    }

    #[test]
    fn test_parse_git_output_multiple_files() {
        let output = "file1.txt\nfile2.rs\nfile3.md\n";
        let result = parse_git_output(output).unwrap();
        assert_eq!(result, vec!["file1.txt", "file2.rs", "file3.md"]);
    }

    #[test]
    fn test_parse_git_output_with_paths() {
        let output = "src/main.rs\nREADME.md\ndocs/guide.md\n";
        let result = parse_git_output(output).unwrap();
        assert_eq!(result, vec!["src/main.rs", "README.md", "docs/guide.md"]);
    }

    #[test]
    fn test_parse_git_output_empty() {
        let output = "";
        let result = parse_git_output(output).unwrap();
        assert_eq!(result, Vec::<String>::new());
    }

    #[test]
    fn test_parse_git_output_only_newlines() {
        let output = "\n\n\n";
        let result = parse_git_output(output).unwrap();
        assert_eq!(result, Vec::<String>::new());
    }

    #[test]
    fn test_parse_git_output_with_whitespace() {
        let output = "  file1.txt  \n  file2.rs\n";
        let result = parse_git_output(output).unwrap();
        assert_eq!(result, vec!["file1.txt", "file2.rs"]);
    }

    #[test]
    fn test_parse_git_output_mixed_whitespace() {
        let output = "file1.txt\n\nfile2.rs\n  \nfile3.md\n";
        let result = parse_git_output(output).unwrap();
        assert_eq!(result, vec!["file1.txt", "file2.rs", "file3.md"]);
    }

    #[test]
    fn test_parse_git_output_no_trailing_newline() {
        let output = "file1.txt\nfile2.rs";
        let result = parse_git_output(output).unwrap();
        assert_eq!(result, vec!["file1.txt", "file2.rs"]);
    }

    #[test]
    fn test_parse_git_output_windows_newlines() {
        let output = "file1.txt\r\nfile2.rs\r\n";
        let result = parse_git_output(output).unwrap();
        assert_eq!(result, vec!["file1.txt", "file2.rs"]);
    }

    #[test]
    fn test_parse_git_output_deep_paths() {
        let output = "a/b/c/d/file.txt\nx/y/z/file.rs\n";
        let result = parse_git_output(output).unwrap();
        assert_eq!(result, vec!["a/b/c/d/file.txt", "x/y/z/file.rs"]);
    }

    #[test]
    fn test_parse_git_output_special_characters_in_path() {
        let output = "file-name.txt\nfile_name.rs\nfile.test.md\n";
        let result = parse_git_output(output).unwrap();
        assert_eq!(
            result,
            vec!["file-name.txt", "file_name.rs", "file.test.md"]
        );
    }
}
