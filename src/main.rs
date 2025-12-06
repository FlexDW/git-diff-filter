use std::process;

mod cli;

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
    let args: cli::Args = cli::parse_args()?;

    println!("Parsed args: {:?}", args);

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_run_basic() {
        // Integration tests will go here
    }
}
