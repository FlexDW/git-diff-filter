use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process;

mod cli;
mod config;
mod git;
mod matcher;
mod output;

fn main() {
    let result = run();

    match result {
        Ok(()) => process::exit(0),
        Err(e) => {
            eprintln!("Error: {e}");
            process::exit(1);
        }
    }
}

fn run() -> Result<(), String> {
    let args = cli::parse_args()?;
    let config = config::from_args(args)?;

    let changed_files = git::get_changed_files(&config.base_ref)?;

    let mut has_match =
        !config.patterns.is_empty() && patterns_match_changes(&changed_files, &config.patterns)?;

    if !has_match {
        for dir in &config.container_dirs {
            if container_has_changes(&changed_files, dir)? {
                has_match = true;
                break;
            }
        }
    }

    output::write_output(
        has_match,
        config.github_output_name.as_deref(),
        config.github_output_filepath.as_deref(),
    )?;

    Ok(())
}

/// Returns true if any changed file matches a positive pattern and isn't excluded by a negative
/// one. Patterns prefixed with `!` are exclusions; all others are inclusions. Order-independent:
/// all inclusions are unioned, then all exclusions are subtracted from that set.
fn patterns_match_changes(changed_files: &[String], patterns: &[String]) -> Result<bool, String> {
    let mut included: HashSet<String> = HashSet::new();
    let mut excluded: HashSet<String> = HashSet::new();

    for pattern in patterns {
        if let Some(neg) = pattern.strip_prefix('!') {
            for file in changed_files {
                if matcher::matches_any(file, &[neg.to_string()])? {
                    excluded.insert(file.clone());
                }
            }
        } else {
            for file in changed_files {
                if matcher::matches_any(file, std::slice::from_ref(pattern))? {
                    included.insert(file.clone());
                }
            }
        }
    }

    Ok(!included.is_empty() && !included.is_subset(&excluded))
}

/// Returns true if any changed file within `container_dir` is relevant after applying
/// `.dockerignore` rules. Without a `.dockerignore`, any change inside the directory counts.
/// With one, rules are applied in order: plain patterns remove files; `!` patterns restore them.
fn container_has_changes(changed_files: &[String], container_dir: &str) -> Result<bool, String> {
    let container_glob = format!("{container_dir}/**");

    // Start with all changed files inside the container directory
    let mut relevant: HashSet<String> = HashSet::new();
    for file in changed_files {
        if matcher::matches_any(file, std::slice::from_ref(&container_glob))? {
            relevant.insert(file.clone());
        }
    }

    // Without a .dockerignore, any change inside the container dir is relevant
    let dockerignore_path = format!("{container_dir}/.dockerignore");
    if !Path::new(&dockerignore_path).exists() {
        return Ok(!relevant.is_empty());
    }

    // Apply .dockerignore rules in order — each rule either removes or restores files
    let content = fs::read_to_string(&dockerignore_path)
        .map_err(|e| format!("Failed to read {dockerignore_path}: {e}"))?;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(exception) = trimmed.strip_prefix('!') {
            // Exception pattern: restore any changed files that match
            let glob = format!("{container_dir}/{exception}");
            for file in changed_files {
                if matcher::matches_any(file, std::slice::from_ref(&glob))? {
                    relevant.insert(file.clone());
                }
            }
        } else {
            // Ignore pattern: remove matching files from the relevant set
            let glob = format!("{container_dir}/{trimmed}");
            let to_remove: Vec<String> = relevant
                .iter()
                .filter(|f| matcher::matches_any(f, std::slice::from_ref(&glob)).unwrap_or(false))
                .cloned()
                .collect();
            for file in to_remove {
                relevant.remove(&file);
            }
        }
    }

    Ok(!relevant.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // --- Fixture for container_has_changes tests ---

    struct ContainerFixture {
        pub dir: String,
    }

    impl ContainerFixture {
        fn new(test_name: &str) -> Self {
            // Use a relative path under target/ so paths match what git diff produces
            // (the matcher strips leading '/' from patterns, so absolute paths break matching)
            let dir = format!(
                "target/test_fixtures/gdf_container_test_{}_{}",
                test_name,
                std::process::id()
            );
            fs::create_dir_all(&dir).unwrap();
            Self { dir }
        }

        fn with_dockerignore(test_name: &str, content: &str) -> Self {
            let fixture = Self::new(test_name);
            fs::write(format!("{}/.dockerignore", fixture.dir), content).unwrap();
            fixture
        }

        /// Prepend the container dir to each relative path, simulating changed files
        fn files(&self, names: &[&str]) -> Vec<String> {
            names.iter().map(|n| format!("{}/{n}", self.dir)).collect()
        }
    }

    impl Drop for ContainerFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    // --- container_has_changes: no .dockerignore ---

    #[test]
    fn test_container_no_dockerignore_with_changes() {
        let f = ContainerFixture::new("no_di_changes");
        assert!(container_has_changes(&f.files(&["src/main.rs"]), &f.dir).unwrap());
    }

    #[test]
    fn test_container_no_dockerignore_no_changes_in_dir() {
        let f = ContainerFixture::new("no_di_outside");
        let outside = vec!["other/service/main.rs".to_string()];
        assert!(!container_has_changes(&outside, &f.dir).unwrap());
    }

    #[test]
    fn test_container_no_dockerignore_no_changed_files() {
        let f = ContainerFixture::new("no_di_empty");
        assert!(!container_has_changes(&[], &f.dir).unwrap());
    }

    // --- container_has_changes: with .dockerignore ---

    #[test]
    fn test_container_empty_dockerignore_counts_all_changes() {
        let f = ContainerFixture::with_dockerignore("di_empty", "");
        assert!(container_has_changes(&f.files(&["src/main.rs"]), &f.dir).unwrap());
    }

    #[test]
    fn test_container_dockerignore_removes_all_relevant_files() {
        let f = ContainerFixture::with_dockerignore("di_removes_all", "*.rs\n");
        assert!(!container_has_changes(&f.files(&["main.rs", "lib.rs"]), &f.dir).unwrap());
    }

    #[test]
    fn test_container_dockerignore_removes_some_files() {
        let f = ContainerFixture::with_dockerignore("di_removes_some", "*.rs\n");
        // *.rs removes main.rs but README.md remains — still a relevant change
        assert!(container_has_changes(&f.files(&["main.rs", "README.md"]), &f.dir).unwrap());
    }

    #[test]
    fn test_container_dockerignore_exception_restores_ignored_file() {
        let f = ContainerFixture::with_dockerignore("di_exception", "*.rs\n!important.rs\n");
        assert!(container_has_changes(&f.files(&["important.rs"]), &f.dir).unwrap());
    }

    #[test]
    fn test_container_dockerignore_outside_files_not_restored_by_exception() {
        let f =
            ContainerFixture::with_dockerignore("di_outside_exception", "*.rs\n!important.rs\n");
        let outside = vec!["other/service/important.rs".to_string()];
        assert!(!container_has_changes(&outside, &f.dir).unwrap());
    }

    #[test]
    fn test_container_dockerignore_comments_and_blanks_ignored() {
        let f = ContainerFixture::with_dockerignore("di_comments", "# ignore logs\n\n*.log\n");
        assert!(container_has_changes(&f.files(&["src/main.rs"]), &f.dir).unwrap());
    }

    #[test]
    fn test_container_dockerignore_order_ignore_then_exception() {
        // ignore all .rs, then restore important.rs → important.rs is relevant
        let f = ContainerFixture::with_dockerignore("di_order_restore", "*.rs\n!important.rs\n");
        assert!(container_has_changes(&f.files(&["important.rs", "other.rs"]), &f.dir).unwrap());
    }

    #[test]
    fn test_container_dockerignore_order_exception_then_ignore() {
        // restore important.rs first, then ignore all .rs → important.rs is removed again
        let f = ContainerFixture::with_dockerignore("di_order_remove", "!important.rs\n*.rs\n");
        assert!(!container_has_changes(&f.files(&["important.rs"]), &f.dir).unwrap());
    }

    // --- patterns_match_changes tests ---

    #[test]
    fn test_single_inclusion_pattern() {
        let files = vec![
            "file.txt".to_string(),
            "test.txt".to_string(),
            "main.rs".to_string(),
        ];
        let patterns = vec!["*.txt".to_string()];
        assert!(patterns_match_changes(&files, &patterns).unwrap());
    }

    #[test]
    fn test_multiple_inclusion_patterns() {
        let files = vec![
            "file.txt".to_string(),
            "test.rs".to_string(),
            "main.js".to_string(),
        ];
        let patterns = vec!["*.txt".to_string(), "*.rs".to_string()];
        assert!(patterns_match_changes(&files, &patterns).unwrap());
    }

    #[test]
    fn test_deduplication() {
        let files = vec!["file.txt".to_string()];
        let patterns = vec!["*.txt".to_string(), "file.*".to_string()];
        assert!(patterns_match_changes(&files, &patterns).unwrap());
    }

    #[test]
    fn test_simple_exclusion() {
        let files = vec![
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "src/README.md".to_string(),
        ];
        let patterns = vec!["src/**".to_string(), "!*.md".to_string()];
        assert!(patterns_match_changes(&files, &patterns).unwrap());
    }

    #[test]
    fn test_exclusion_removes_all() {
        let files = vec!["file.txt".to_string(), "test.txt".to_string()];
        let patterns = vec!["*.txt".to_string(), "!*.txt".to_string()];
        assert!(!patterns_match_changes(&files, &patterns).unwrap());
    }

    #[test]
    fn test_order_independent_exclusions() {
        let files = vec![
            "src/main.rs".to_string(),
            "src/test.rs".to_string(),
            "src/README.md".to_string(),
        ];

        let patterns1 = vec!["!*.md".to_string(), "src/**".to_string()];
        let result1 = patterns_match_changes(&files, &patterns1).unwrap();

        let patterns2 = vec!["src/**".to_string(), "!*.md".to_string()];
        let result2 = patterns_match_changes(&files, &patterns2).unwrap();
        assert_eq!(result1, result2);
        assert!(result1);
    }

    #[test]
    fn test_exclusion_only_affects_matched() {
        let files = vec!["file.txt".to_string(), "README.md".to_string()];
        let patterns = vec!["!*.md".to_string()];
        assert!(!patterns_match_changes(&files, &patterns).unwrap());
    }

    #[test]
    fn test_multiple_exclusions() {
        let files = vec![
            "src/main.rs".to_string(),
            "src/test.rs".to_string(),
            "src/README.md".to_string(),
            "src/notes.txt".to_string(),
        ];
        let patterns = vec![
            "src/**".to_string(),
            "!*.md".to_string(),
            "!*.txt".to_string(),
        ];
        assert!(patterns_match_changes(&files, &patterns).unwrap());
    }

    #[test]
    fn test_empty_pattern_list() {
        let files = vec!["file.txt".to_string()];
        let patterns = vec![];
        assert!(!patterns_match_changes(&files, &patterns).unwrap());
    }

    #[test]
    fn test_empty_file_list() {
        let files = vec![];
        let patterns = vec!["*.txt".to_string()];
        assert!(!patterns_match_changes(&files, &patterns).unwrap());
    }

    #[test]
    fn test_complex_inclusion_exclusion() {
        let files = vec![
            "libs/core/src/main.rs".to_string(),
            "libs/core/test/unit.rs".to_string(),
            "libs/utils/src/helper.rs".to_string(),
            "apps/web/src/app.js".to_string(),
            "apps/api/README.md".to_string(),
        ];
        let patterns = vec![
            "libs/**".to_string(),
            "apps/**".to_string(),
            "!**/test/**".to_string(),
            "!*.md".to_string(),
        ];
        assert!(patterns_match_changes(&files, &patterns).unwrap());
    }

    #[test]
    fn test_only_exclusions() {
        let files = vec!["file.txt".to_string(), "test.rs".to_string()];
        let patterns = vec!["!*.md".to_string(), "!*.js".to_string()];
        assert!(!patterns_match_changes(&files, &patterns).unwrap());
    }

    #[test]
    fn test_no_inclusions_match() {
        let files = vec!["file.js".to_string(), "test.py".to_string()];
        let patterns = vec!["*.txt".to_string(), "!*.js".to_string()];
        assert!(!patterns_match_changes(&files, &patterns).unwrap());
    }
}
