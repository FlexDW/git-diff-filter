//! Configuration merging from CLI arguments and environment variables.

use crate::cli::Args;
use std::env;

/// Final configuration after merging CLI args with environment variables
#[derive(Debug, PartialEq)]
pub struct Config {
    pub patterns: Vec<String>,
    pub base_ref: String,
    pub github_output_name: Option<String>,
    pub github_output_filepath: Option<String>,
}

/// Merge CLI arguments with environment variables
pub fn from_args(args: Args) -> Result<Config, String> {
    // Determine base_ref: CLI flag takes precedence over env var
    let base_ref = args
        .base_ref
        .filter(|s| !s.is_empty())
        .or_else(|| env::var("BASE_REF").ok().filter(|s| !s.is_empty()))
        .ok_or_else(|| {
            "BASE_REF must be provided via -b/--base-ref flag or BASE_REF environment variable"
                .to_string()
        })?;

    // Read GITHUB_OUTPUT file path from environment (if set)
    let github_output_filepath = env::var("GITHUB_OUTPUT").ok();

    Ok(Config {
        patterns: args.patterns,
        base_ref,
        github_output_name: args.github_output,
        github_output_filepath,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_ref_from_cli_flag() {
        let args = Args {
            patterns: vec!["*.txt".to_string()],
            base_ref: Some("main".to_string()),
            github_output: None,
        };

        let config = from_args(args).unwrap();
        assert_eq!(config.base_ref, "main");
        assert_eq!(config.patterns, vec!["*.txt".to_string()]);
        assert_eq!(config.github_output_name, None);
    }

    #[test]
    fn test_base_ref_from_env_var() {
        unsafe {
            env::set_var("BASE_REF", "develop");
        }

        let args = Args {
            patterns: vec!["*.rs".to_string()],
            base_ref: None,
            github_output: None,
        };

        let config = from_args(args).unwrap();
        assert_eq!(config.base_ref, "develop");

        unsafe {
            env::remove_var("BASE_REF");
        }
    }

    #[test]
    fn test_cli_flag_overrides_env_var() {
        unsafe {
            env::set_var("BASE_REF", "develop");
        }

        let args = Args {
            patterns: vec!["*.rs".to_string()],
            base_ref: Some("main".to_string()),
            github_output: None,
        };

        let config = from_args(args).unwrap();
        assert_eq!(config.base_ref, "main"); // CLI flag wins

        unsafe {
            env::remove_var("BASE_REF");
        }
    }

    #[test]
    fn test_error_when_base_ref_missing() {
        unsafe {
            env::remove_var("BASE_REF");
        }

        let args = Args {
            patterns: vec!["*.rs".to_string()],
            base_ref: None,
            github_output: None,
        };

        let result = from_args(args);
        assert_eq!(
            result,
            Err("BASE_REF must be provided via -b/--base-ref flag or BASE_REF environment variable"
                .to_string())
        );
    }

    #[test]
    fn test_error_when_base_ref_empty() {
        unsafe {
            env::set_var("BASE_REF", "");
        }

        let args = Args {
            patterns: vec!["*.rs".to_string()],
            base_ref: None,
            github_output: None,
        };

        let result = from_args(args);
        assert!(result.is_err());

        unsafe {
            env::remove_var("BASE_REF");
        }
    }

    #[test]
    fn test_github_output_name_passed_through() {
        let args = Args {
            patterns: vec!["*.rs".to_string()],
            base_ref: Some("main".to_string()),
            github_output: Some("api".to_string()),
        };

        let config = from_args(args).unwrap();
        assert_eq!(config.github_output_name, Some("api".to_string()));
    }

    #[test]
    fn test_github_output_file_from_env() {
        unsafe {
            env::set_var("GITHUB_OUTPUT", "/tmp/github_output.txt");
        }

        let args = Args {
            patterns: vec!["*.rs".to_string()],
            base_ref: Some("main".to_string()),
            github_output: None,
        };

        let config = from_args(args).unwrap();
        assert_eq!(
            config.github_output_filepath,
            Some("/tmp/github_output.txt".to_string())
        );

        unsafe {
            env::remove_var("GITHUB_OUTPUT");
        }
    }

    #[test]
    fn test_github_output_file_not_set() {
        unsafe {
            env::remove_var("GITHUB_OUTPUT");
        }

        let args = Args {
            patterns: vec!["*.rs".to_string()],
            base_ref: Some("main".to_string()),
            github_output: None,
        };

        let config = from_args(args).unwrap();
        assert_eq!(config.github_output_filepath, None);
    }

    #[test]
    fn test_all_config_fields() {
        unsafe {
            env::set_var("BASE_REF", "develop");
            env::set_var("GITHUB_OUTPUT", "/tmp/output");
        }

        let args = Args {
            patterns: vec!["*.rs".to_string(), "*.md".to_string()],
            base_ref: None,
            github_output: Some("my-api".to_string()),
        };

        let config = from_args(args).unwrap();
        assert_eq!(config.patterns, vec!["*.rs", "*.md"]);
        assert_eq!(config.base_ref, "develop");
        assert_eq!(config.github_output_name, Some("my-api".to_string()));
        assert_eq!(config.github_output_filepath, Some("/tmp/output".to_string()));

        unsafe {
            env::remove_var("BASE_REF");
            env::remove_var("GITHUB_OUTPUT");
        }
    }
}
