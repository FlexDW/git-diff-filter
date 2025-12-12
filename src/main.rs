use std::process;

mod cli;
mod config;
mod git;
mod matcher;

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

    // Check if any files match the patterns
    let mut has_match = false;
    for file in &changed_files {
        if matcher::matches_any(file, &config.patterns)? {
            has_match = true;
            break;
        }
    }

    // Debug output
    eprintln!(
        "Comparing: {}..HEAD | Patterns: {} | Match: {}",
        config.base_ref,
        config.patterns.join(", "),
        has_match
    );

    // Output result
    println!("{has_match}");

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_run_basic() {
        // Integration tests will go here
    }
}
