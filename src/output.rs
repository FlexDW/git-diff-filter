//! Output handling for stdout, stderr, and GitHub Actions output files.

use std::fs::OpenOptions;
use std::io::Write;

/// Write the match result to stdout and optionally to `GITHUB_OUTPUT` file
pub fn write_output(
    has_match: bool,
    output_name: Option<&str>,
    github_output_filepath: Option<&str>,
) -> Result<(), String> {
    let result = if has_match { "true" } else { "false" };

    if let Some(name) = output_name {
        // GitHub Actions output mode: <name>=<result>
        let output_line = format!("{name}={result}");
        println!("{output_line}");

        // Write to GITHUB_OUTPUT file if path is set
        if let Some(filepath) = github_output_filepath {
            write_to_file(filepath, &output_line)?;
        }
    } else {
        // Plain output mode: just true/false
        println!("{result}");
    }

    Ok(())
}

/// Append a line to a file (used for `GITHUB_OUTPUT`)
fn write_to_file(filepath: &str, content: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(filepath)
        .map_err(|e| format!("Failed to open {filepath}: {e}"))?;

    writeln!(file, "{content}").map_err(|e| format!("Failed to write to {filepath}: {e}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    // Helper to create a temporary file path for testing
    fn temp_file_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("gdf_test_{name}_{}", std::process::id()));
        path
    }

    // Helper to clean up test file
    fn cleanup(path: &PathBuf) {
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_write_to_file_creates_new_file() {
        let path = temp_file_path("create");
        cleanup(&path);

        let result = write_to_file(path.to_str().unwrap(), "test=true");
        assert!(result.is_ok());

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "test=true\n");

        cleanup(&path);
    }

    #[test]
    fn test_write_to_file_appends() {
        let path = temp_file_path("append");
        cleanup(&path);

        write_to_file(path.to_str().unwrap(), "first=true").unwrap();
        write_to_file(path.to_str().unwrap(), "second=false").unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "first=true\nsecond=false\n");

        cleanup(&path);
    }

    #[test]
    fn test_write_to_file_invalid_path() {
        let result = write_to_file("/invalid/path/that/does/not/exist", "test=true");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to open"));
    }

    #[test]
    fn test_write_output_plain_mode_true() {
        // Plain mode: no name, no file
        let result = write_output(true, None, None);
        assert!(result.is_ok());
        // Would print "true" to stdout (can't easily test in unit test)
    }

    #[test]
    fn test_write_output_plain_mode_false() {
        // Plain mode: no name, no file
        let result = write_output(false, None, None);
        assert!(result.is_ok());
        // Would print "false" to stdout (can't easily test in unit test)
    }

    #[test]
    fn test_write_output_github_mode_no_file() {
        // GitHub mode: name provided, but no file path
        let result = write_output(true, Some("changed"), None);
        assert!(result.is_ok());
        // Would print "changed=true" to stdout (can't easily test in unit test)
    }

    #[test]
    fn test_write_output_github_mode_with_file() {
        let path = temp_file_path("github_output");
        cleanup(&path);

        let result = write_output(true, Some("changed"), Some(path.to_str().unwrap()));
        assert!(result.is_ok());

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "changed=true\n");

        cleanup(&path);
    }

    #[test]
    fn test_write_output_github_mode_multiple_writes() {
        let path = temp_file_path("github_multi");
        cleanup(&path);

        write_output(true, Some("first"), Some(path.to_str().unwrap())).unwrap();
        write_output(false, Some("second"), Some(path.to_str().unwrap())).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "first=true\nsecond=false\n");

        cleanup(&path);
    }

    #[test]
    fn test_write_output_file_write_failure() {
        // Invalid file path should cause error
        let result = write_output(
            true,
            Some("changed"),
            Some("/invalid/path/that/does/not/exist"),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to open"));
    }
}
