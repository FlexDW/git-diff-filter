//! This module handles command-line argument parsing.

use std::env;

/// Parsed command-line arguments
#[derive(Debug, PartialEq)]
pub struct Args {
    pub patterns: Vec<String>,
    pub base_ref: Option<String>,
    pub github_output: Option<String>,
}

/// Parse command-line arguments from environment
pub fn parse_args() -> Result<Args, String> {
    let args: Vec<String> = env::args().skip(1).collect(); // Skip program name
    parse_args_from_vec(&args) 
}

/// Parse arguments from a vector (for testing)
fn parse_args_from_vec(args: &[String]) -> Result<Args, String> {
    let mut patterns = Vec::new();
    let mut base_ref = None;
    let mut github_output = None;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        match arg.as_str() {
            "-p" | "--pattern" => {
                i += 1;
                if i >= args.len() {
                    return Err(format!("{} requires a value", arg));
                }
                patterns.push(args[i].clone());
            }
            "-b" | "--base-ref" => {
                i += 1;
                if base_ref.is_some() {
                    return Err(format!("{} can only be specified once", arg));
                }
                if i >= args.len() {
                    return Err(format!("{} requires a value", arg));
                }
                base_ref = Some(args[i].clone());
            }
            "-g" | "--github-output" => {
                i += 1;
                if github_output.is_some() {
                    return Err(format!("{} can only be specified once", arg));
                }
                if i >= args.len() {
                    return Err(format!("{} requires a value", arg));
                }
                github_output = Some(args[i].clone());
            }
            _ => {
                if arg.starts_with('-') {
                    return Err(format!("Unknown flag: {}", arg));
                } else {
                    return Err(format!("Unexpected argument: {}", arg));
                }
            }
        }
        i += 1;
    }

    // Validate required flags
    if patterns.is_empty() {
        return Err("at least one --pattern is required".to_string());
    }

    Ok(Args {
        patterns,
        base_ref,
        github_output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Args, String> {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        parse_args_from_vec(&args)
    }

    #[test]
    fn test_parse_single_pattern() {
        let result = parse(&["-p", "*.txt"]);
        assert_eq!(
            result,
            Ok(Args {
                patterns: vec!["*.txt".to_string()],
                base_ref: None,
                github_output: None,
            })
        );
    }

    #[test]
    fn test_parse_multiple_patterns() {
        let result = parse(&["-p", "*.txt", "-p", "*.rs"]);
        assert_eq!(
            result,
            Ok(Args {
                patterns: vec!["*.txt".to_string(), "*.rs".to_string()],
                base_ref: None,
                github_output: None,
            })
        );
    }

    #[test]
    fn test_parse_with_base_ref() {
        let result = parse(&["-p", "*.txt", "-b", "main"]);
        assert_eq!(
            result,
            Ok(Args {
                patterns: vec!["*.txt".to_string()],
                base_ref: Some("main".to_string()),
                github_output: None,
            })
        );
    }

    #[test]
    fn test_parse_with_github_output() {
        let result = parse(&["-p", "*.txt", "-g", "api"]);
        assert_eq!(
            result,
            Ok(Args {
                patterns: vec!["*.txt".to_string()],
                base_ref: None,
                github_output: Some("api".to_string()),
            })
        );
    }

    #[test]
    fn test_parse_all_flags() {
        let result = parse(&["-p", "*.txt", "-p", "*.rs", "-b", "main", "-g", "api"]);
        assert_eq!(
            result,
            Ok(Args {
                patterns: vec!["*.txt".to_string(), "*.rs".to_string()],
                base_ref: Some("main".to_string()),
                github_output: Some("api".to_string()),
            })
        );
    }

    #[test]
    fn test_parse_long_form_flags() {
        let result = parse(&[
            "--pattern",
            "*.txt",
            "--base-ref",
            "main",
            "--github-output",
            "api",
        ]);
        assert_eq!(
            result,
            Ok(Args {
                patterns: vec!["*.txt".to_string()],
                base_ref: Some("main".to_string()),
                github_output: Some("api".to_string()),
            })
        );
    }

    #[test]
    fn test_parse_mixed_short_long_flags() {
        let result = parse(&["-p", "*.txt", "--base-ref", "main", "-g", "api"]);
        assert_eq!(
            result,
            Ok(Args {
                patterns: vec!["*.txt".to_string()],
                base_ref: Some("main".to_string()),
                github_output: Some("api".to_string()),
            })
        );
    }

    #[test]
    fn test_error_missing_pattern() {
        let result = parse(&["-b", "main"]);
        assert_eq!(
            result,
            Err("at least one --pattern is required".to_string())
        );
    }

    #[test]
    fn test_error_pattern_without_value() {
        let result = parse(&["-p"]);
        assert_eq!(result, Err("-p requires a value".to_string()));
    }

    #[test]
    fn test_error_base_ref_without_value() {
        let result = parse(&["-p", "*.txt", "-b"]);
        assert_eq!(result, Err("-b requires a value".to_string()));
    }

    #[test]
    fn test_error_github_output_without_value() {
        let result = parse(&["-p", "*.txt", "-g"]);
        assert_eq!(result, Err("-g requires a value".to_string()));
    }

    #[test]
    fn test_error_unknown_flag() {
        let result = parse(&["-p", "*.txt", "-x"]);
        assert_eq!(result, Err("Unknown flag: -x".to_string()));
    }

    #[test]
    fn test_error_unknown_long_flag() {
        let result = parse(&["-p", "*.txt", "--unknown"]);
        assert_eq!(result, Err("Unknown flag: --unknown".to_string()));
    }

    #[test]
    fn test_error_unexpected_positional_argument() {
        let result = parse(&["-p", "*.txt", "extra"]);
        assert_eq!(result, Err("Unexpected argument: extra".to_string()));
    }

    #[test]
    fn test_error_duplicate_base_ref() {
        let result = parse(&["-p", "*.txt", "-b", "main", "-b", "develop"]);
        assert_eq!(
            result,
            Err("-b can only be specified once".to_string())
        );
    }

    #[test]
    fn test_error_duplicate_github_output() {
        let result = parse(&["-p", "*.txt", "-g", "api", "-g", "service"]);
        assert_eq!(
            result,
            Err("-g can only be specified once".to_string())
        );
    }

    #[test]
    fn test_empty_args() {
        let result = parse(&[]);
        assert_eq!(
            result,
            Err("at least one --pattern is required".to_string())
        );
    }

    #[test]
    fn test_pattern_with_spaces() {
        let result = parse(&["-p", "src/**/*.rs", "-b", "refs/tags/v1.0"]);
        assert_eq!(
            result,
            Ok(Args {
                patterns: vec!["src/**/*.rs".to_string()],
                base_ref: Some("refs/tags/v1.0".to_string()),
                github_output: None,
            })
        );
    }

    #[test]
    fn test_multiple_patterns_various_order() {
        let result = parse(&["-b", "main", "-p", "*.txt", "-g", "api", "-p", "*.rs"]);
        assert_eq!(
            result,
            Ok(Args {
                patterns: vec!["*.txt".to_string(), "*.rs".to_string()],
                base_ref: Some("main".to_string()),
                github_output: Some("api".to_string()),
            })
        );
    }
}
