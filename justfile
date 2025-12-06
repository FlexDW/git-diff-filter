# Build the project in debug mode
build:
    cargo build

# Build optimized release version
build-release:
    cargo build --release

# Run all tests
# Using --test-threads=1 to prevent race conditions when manipulating env vars
test:
    cargo test -- --test-threads=1

# Run tests with coverage report
test-coverage:
    cargo test --verbose

# Run the binary with arguments
run *args:
    cargo run -- {{args}}

# Check code without building (fast)
check:
    cargo check

# Run clippy linter
lint:
    cargo clippy -- -D warnings

# Format code
fmt:
    cargo fmt

# Check if code is formatted correctly
fmt-check:
    cargo fmt -- --check

# Clean build artifacts
clean:
    cargo clean

# Run all checks (fmt, clippy, test)
ci: fmt-check lint test

# Install the binary locally
install:
    cargo install --path .
