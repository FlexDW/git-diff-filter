use std::process;

mod cli;
mod config;

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

    println!("Config: {:?}", config);

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_run_basic() {
        // Integration tests will go here
    }
}
