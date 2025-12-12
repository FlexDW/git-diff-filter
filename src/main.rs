use std::collections::HashSet;
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

    // Get changed files
    let changed_files = git::get_changed_files(&config.base_ref)?;

    // Build positive and negative match sets
    let mut positive_matches = HashSet::new();
    let mut negative_matches = HashSet::new();

    for pattern in &config.patterns {
        if let Some(negated_pattern) = pattern.strip_prefix('!') {
            // Negative pattern - collect files that match
            for file in &changed_files {
                if matcher::matches_any(file, std::slice::from_ref(&negated_pattern.to_string()))? {
                    negative_matches.insert(file.clone());
                }
            }
        } else {
            // Positive pattern - collect files that match
            for file in &changed_files {
                if matcher::matches_any(file, std::slice::from_ref(pattern))? {
                    positive_matches.insert(file.clone());
                }
            }
        }
    }

    // Combine: true if any positive matches remain after removing negatives
    let has_match = !positive_matches.is_empty() && !positive_matches.is_subset(&negative_matches);

    // Debug output
    eprintln!(
        "Comparing: {}..HEAD | Patterns: {} | Match: {}",
        config.base_ref,
        config.patterns.join(", "),
        has_match
    );

    // Output result
    output::write_output(
        has_match,
        config.github_output_name.as_deref(),
        config.github_output_filepath.as_deref(),
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to test the orchestration logic without running full integration
    fn test_orchestration(files: &[String], patterns: &[String]) -> Result<bool, String> {
        let mut positive_matches = HashSet::new();
        let mut negative_matches = HashSet::new();

        for pattern in patterns {
            if let Some(negated_pattern) = pattern.strip_prefix('!') {
                for file in files {
                    if matcher::matches_any(
                        file,
                        std::slice::from_ref(&negated_pattern.to_string()),
                    )? {
                        negative_matches.insert(file.clone());
                    }
                }
            } else {
                for file in files {
                    if matcher::matches_any(file, std::slice::from_ref(pattern))? {
                        positive_matches.insert(file.clone());
                    }
                }
            }
        }

        Ok(!positive_matches.is_empty() && !positive_matches.is_subset(&negative_matches))
    }

    #[test]
    fn test_single_inclusion_pattern() {
        let files = vec![
            "file.txt".to_string(),
            "test.txt".to_string(),
            "main.rs".to_string(),
        ];
        let patterns = vec!["*.txt".to_string()];
        assert!(test_orchestration(&files, &patterns).unwrap());
    }

    #[test]
    fn test_multiple_inclusion_patterns() {
        let files = vec![
            "file.txt".to_string(),
            "test.rs".to_string(),
            "main.js".to_string(),
        ];
        let patterns = vec!["*.txt".to_string(), "*.rs".to_string()];
        assert!(test_orchestration(&files, &patterns).unwrap());
    }

    #[test]
    fn test_deduplication() {
        let files = vec!["file.txt".to_string()];
        let patterns = vec!["*.txt".to_string(), "file.*".to_string()];
        assert!(test_orchestration(&files, &patterns).unwrap());
    }

    #[test]
    fn test_simple_exclusion() {
        let files = vec![
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "src/README.md".to_string(),
        ];
        let patterns = vec!["src/**".to_string(), "!*.md".to_string()];
        assert!(test_orchestration(&files, &patterns).unwrap());
    }

    #[test]
    fn test_exclusion_removes_all() {
        let files = vec!["file.txt".to_string(), "test.txt".to_string()];
        let patterns = vec!["*.txt".to_string(), "!*.txt".to_string()];
        assert!(!test_orchestration(&files, &patterns).unwrap());
    }

    #[test]
    fn test_order_independent_exclusions() {
        let files = vec![
            "src/main.rs".to_string(),
            "src/test.rs".to_string(),
            "src/README.md".to_string(),
        ];

        let patterns1 = vec!["!*.md".to_string(), "src/**".to_string()];
        let result1 = test_orchestration(&files, &patterns1).unwrap();

        let patterns2 = vec!["src/**".to_string(), "!*.md".to_string()];
        let result2 = test_orchestration(&files, &patterns2).unwrap();
        assert_eq!(result1, result2);
        assert!(result1);
    }

    #[test]
    fn test_exclusion_only_affects_matched() {
        let files = vec!["file.txt".to_string(), "README.md".to_string()];
        let patterns = vec!["!*.md".to_string()];
        assert!(!test_orchestration(&files, &patterns).unwrap());
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
        assert!(test_orchestration(&files, &patterns).unwrap());
    }

    #[test]
    fn test_empty_pattern_list() {
        let files = vec!["file.txt".to_string()];
        let patterns = vec![];
        assert!(!test_orchestration(&files, &patterns).unwrap());
    }

    #[test]
    fn test_empty_file_list() {
        let files = vec![];
        let patterns = vec!["*.txt".to_string()];
        assert!(!test_orchestration(&files, &patterns).unwrap());
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
        assert!(test_orchestration(&files, &patterns).unwrap());
    }

    #[test]
    fn test_only_exclusions() {
        let files = vec!["file.txt".to_string(), "test.rs".to_string()];
        let patterns = vec!["!*.md".to_string(), "!*.js".to_string()];
        assert!(!test_orchestration(&files, &patterns).unwrap());
    }

    #[test]
    fn test_no_inclusions_match() {
        let files = vec!["file.js".to_string(), "test.py".to_string()];
        let patterns = vec!["*.txt".to_string(), "!*.js".to_string()];
        assert!(!test_orchestration(&files, &patterns).unwrap());
    }
}
