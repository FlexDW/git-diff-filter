use std::process;

mod cli;
mod config;
mod git;

fn main() {
    let result= run();

    match result {
        Ok(_) => process::exit(0),
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }
}

fn run() -> Result<(), String> {
    let args = cli::parse_args()?;
    let config = config::from_args(args)?;

    // Get changed files
    let changed_files = git::get_changed_files(&config.base_ref)?;

    // Debug output
    eprintln!(
        "Comparing: {}..HEAD | Files changed: {}",
        config.base_ref,
        changed_files.len()
    );
    for file in &changed_files {
        eprintln!("  {}", file);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_run_basic() {
        // Integration tests will go here
    }
}
