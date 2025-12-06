# GitHub Copilot Instructions

## Project Overview
This project is a GitHub Action that installs a command-line utility (`gdf`) in the workflow runtime.

The utility analyzes git diffs against glob patterns to determine which parts of a monorepo changed, helping workflows decide whether to run specific jobs or steps.

This is a clean reimplementation of gitignore-style glob pattern matching using only Rust's standard library.

## Code Architecture

### Repository Structure

Separate concerns, for example:

```
src/
├── main.rs           # Entry point, minimal orchestration, delegates to modules
├── cli.rs            # CLI argument parsing
├── config.rs         # Merge CLI args with environment variables
├── git.rs            # Execute git commands, parse output
├── matcher.rs        # Pattern matching orchestration
├── output.rs         # Write to stdout/stderr/file output
└── patterns/         # Glob pattern implementations, each file handles one pattern type
    ├── mod.rs        # Module exports
    ├── basic.rs      # *, ?, literals
    ├── directory.rs  # ** (globstar)
    ├── charset.rs    # [abc], [a-z], [!abc]
    ├── brace.rs      # {a,b,c}
    ├── negation.rs   # !pattern
    └── anchor.rs     # /pattern, pattern/, \
```


### Module Responsibilities
- **main.rs**: Minimal orchestration, delegates to modules
- **cli.rs**: Parse command-line arguments (only `std::env`)
- **config.rs**: Merge CLI args with environment variables
- **git.rs**: Execute git commands, parse output
- **matcher.rs**: Coordinate pattern matching
- **output.rs**: Write to stdout/stderr/files
- **patterns/**: Each file handles one pattern type

5. **Small functions**: Easy to test and understand
6. **No circular dependencies**: Clean import graph

## Code Style and Conventions

### Rust Guidelines
- Follow official Rust style guidelines
- Use `cargo fmt` for formatting (see `rustfmt.toml`)
- Use `cargo clippy` for linting (must pass with no warnings)

### Testing
- Tests must be in the same file as the code they test
- Use `#[cfg(test)]` module at the end of each file
- Test all public functions
- Test edge cases and error paths
- Mock external dependencies (git, file I/O)

### Imports
- **Only import from `std`** - no external crates
- Common imports:
  - `std::env` - Environment and arguments
  - `std::process` - Execute git commands
  - `std::fs` - File I/O for GITHUB_OUTPUT
  - `std::io` - stdout/stderr

### Error Handling
- Return `Result<T, String>` with descriptive error messages
- Error messages should be user-friendly
- All errors exit with code 1
- Success exits with code 0

### Implementation Process
1. **Write tests first** (TDD approach)
2. **Implement feature** until tests pass
3. **Verify 100% coverage** for new code
4. **Run checks**: `just ci` (fmt, lint, test)
5. **Commit** with clear message
6. **Move to next feature**

### Commands
- See justfile

## Specific Instructions

### Always
- Make sure you understand the user before making changes
- Confirm intent if not 100% clear
- Keep functions small and focused
- Add comments for unintuitive logic only

### Never
- Import external crates
- Make assumptions without confirming

## Reference Documents
- **User Documentation**: `README.md`
