# `gdf` CLI Tool

A command-line utility for detecting changes in a monorepo by comparing git diffs against glob patterns. Designed to integrate seamlessly with GitHub Actions workflows but works in any CI/CD environment or local development.

## Overview

`gdf` analyzes git diffs to determine which parts of the code changed, helping workflows decide whether to run specific jobs or steps. It performs glob pattern matching against git diffs to flag which components of a monorepo have been modified.

## Usage

```bash
gdf -p <glob> [-p <glob>...] [-b <base-ref>] [-g <name>]
```

### Arguments and Flags

#### Required Flags

- `-p, --pattern <glob>` - Glob pattern to match against changed files (can be specified multiple times)
  - **Note**: Wrap patterns in quotes to prevent shell expansion (e.g., `'libs/**'` not `libs/**`)

#### Optional Flags

- `-b, --base-ref <ref>` - The git reference to compare against (e.g., `refs/tags/production`, `main`, `HEAD~1`)
  - If not provided, it will try to use `BASE_REF` environment variable
  - Command-line flag takes precedence over environment variable
- `-g, --github-output <name>` - Enable GitHub Actions integration by specifying the output variable name
  - When provided, outputs in format `<name>=true|false` and writes to `$GITHUB_OUTPUT` file
  - When omitted, outputs plain `true` or `false` to stdout

#### Environment Variables

- `BASE_REF` - The git reference to compare against (fallback if `--base-ref` is not provided)
  - Either `--base-ref` flag or `BASE_REF` environment variable is required
  - Command-line flag takes precedence

### Behavior

1. Reads the base reference from `--base-ref` flag or falls back to `BASE_REF` environment variable
2. Executes `git diff --name-only $BASE_REF..HEAD` to get list of changed files
3. For each glob pattern specified with `-p`:
   - Matches the pattern against all changed files
   - If any file matches, considers the component as changed
4. Output:
   - **stderr**: Logs comparison info for debugging (e.g., `Comparing: main..HEAD`)
   - **stdout** (without `-g` flag): Outputs `true` or `false`
   - **stdout** (with `-g` flag): Outputs `<name>=true` or `<name>=false` AND writes to `$GITHUB_OUTPUT` file (if the environment variable exists)

### Exit Codes

- `0` - Success (always, even if no files match)
- `1` - Error (missing base ref, git command failed, invalid arguments, etc.)

## Examples

### Basic Usage (Plain Output)

```bash
gdf -p 'components/reporting/**' -b refs/tags/production
# stderr: Comparing: refs/tags/test..HEAD | Patterns: components/reporting/** | Match: true
# stdout: true
```

### GitHub Actions Integration

```bash
gdf -g reporting-api -p 'components/reporting/**' -b refs/tags/production
# stderr: Comparing: refs/tags/production..HEAD | Patterns: components/reporting/** | Match: true
# stdout: true
# Writes to $GITHUB_OUTPUT: reporting-api=true
```

### Using Environment Variable for Base Ref

```bash
export BASE_REF=refs/tags/test
gdf -p 'components/reporting/**'
# stderr: Comparing: refs/tags/test..HEAD | Patterns: components/reporting/** | Match: false
# stdout: false
```

### Multiple Glob Patterns

```bash
gdf -p 'libs/**' -p 'package.json' -p 'lerna.json' -b main
# stderr: Comparing: main..HEAD | Patterns: libs/**, package.json, lerna.json | Match: false
# stdout: false
```

### Excluding Files with Negation Patterns

```bash
# Match all source files except markdown
gdf -p 'graph-api/src/**' -p '!*.md' -b main
# stdout: true
```

### Plain Boolean Check in Scripts

```bash
# Simple boolean check
result=$(gdf -p 'src/**' -b main)
if [ "$result" = "true" ]; then
  echo "Source code changed"
fi
# stderr: Comparing: main..HEAD | Patterns: src/** | Match: true
# stdout: true
```

### Conditional Build in Shell Script

```bash
# Set base ref
export BASE_REF=main

# Check multiple components
reporting_api=$(gdf -p 'components/reporting/**' -p 'libs/**')
graph_api=$(gdf -p 'components/graph/**' -p 'libs/**')
scheduler=$(gdf -p 'components/scheduler/**')

# Build only changed components
[ "$reporting_api" = "true" ] && npm run build:reporting-api
[ "$graph_api" = "true" ] && npm run build:graph-api
[ "$scheduler" = "true" ] && npm run build:scheduler

echo "Build complete"
```

### Flag Overrides Environment Variable

```bash
export BASE_REF=refs/tags/production
gdf -g api -p 'api/**' -b main
# stderr: Comparing: main..HEAD (uses main, not refs/tags/production)
# stdout: api=true
```

### GitHub Actions Integration

Complete workflow example:

```yaml
jobs:
  setup:
    name: 'Detect changes'
    runs-on: ubuntu-latest
    outputs:
      analysis-api: ${{ steps.changes.outputs.analysis-api }}
      graph-api: ${{ steps.changes.outputs.graph-api }}
      ingest-api: ${{ steps.changes.outputs.ingest-api }}
      reporting-api: ${{ steps.changes.outputs.reporting-api }}
      scheduler: ${{ steps.changes.outputs.scheduler }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0  # Required for git history

      - name: Install gdf
        uses: your-org/git-diff-filter@v1

      - name: Detect component changes
        id: changes
        run: |
          export BASE_REF=main
          gdf -g analysis-api -p 'components/exceptions/analysis-api/**' -p 'libs/**'
          gdf -g graph-api -p 'components/graph/graph-api/**' -p 'libs/**'
          gdf -g ingest-api -p 'components/ingest/ingest-api/**' -p 'libs/**'
          gdf -g reporting-api -p 'components/reporting/reporting-api/**' -p 'libs/**'
          gdf -g scheduler -p 'components/scheduler/**' -p 'libs/**'

  build:
    name: 'Build changed components'
    needs: setup
    runs-on: ubuntu-latest
    strategy:
      matrix:
        component: [analysis-api, graph-api, ingest-api, reporting-api, scheduler]
    steps:
      - uses: actions/checkout@v4

      - name: Build ${{ matrix.component }}
        if: needs.setup.outputs[matrix.component] == 'true'
        run: npm run build:${{ matrix.component }}
```

## Output Format

### Default Mode (Plain Boolean)

Without the `-g` flag, outputs plain boolean:

```
true
```

or

```
false
```

Useful for scripts, shell conditionals, or any non-GitHub Actions environment.

### GitHub Actions Mode

With the `-g <name>` flag, outputs in GitHub Actions format:

```
<name>=true
```

or

```
<name>=false
```

This format is automatically written to `$GITHUB_OUTPUT` (if the environment variable exists) and can be used in workflow conditionals via `steps.<step-id>.outputs.<name>`.

## Implementation Notes

### Technology Stack

- Language: Rust
- Dependencies: Standard library only (`std`)
- Glob matching: Custom gitignore-style pattern implementation
- Git execution: `std::process::Command`
- Target platforms: Linux (x86_64)

### Git Operations

- Requires git to be available in PATH
- Requires repository to have fetched history (GitHub Actions: `fetch-depth: 0`)
- Compares current HEAD against the reference specified in `BASE_REF`
- Command: `git diff --name-only $BASE_REF..HEAD`

### Glob Matching

- Gitignore-style glob pattern matching
- Patterns are matched against relative file paths from repository root
- Supported patterns:
  - `**` - Match any number of directories (e.g., `src/**/*.rs`)
  - `*` - Match any characters except `/` (e.g., `*.json`)
  - `?` - Match single character (e.g., `file?.txt`)
  - `[abc]` - Match any character in brackets (e.g., `[Tt]est.txt`)
  - `[a-z]` - Match character range (e.g., `file[0-9].txt`)
  - `[!abc]` - Match any character NOT in brackets (e.g., `[!.]*.txt`)
  - `{a,b}` - Match either a or b (e.g., `*.{js,ts}`)
  - `!pattern` - Negate/exclude files matching pattern (e.g., `!*.md`)
  - `/pattern` - Anchor pattern to root directory (e.g., `/README.md`)
  - `pattern/` - Match directories only (e.g., `build/`)
  - `\` - Escape special characters (e.g., `\*.txt` matches literal `*.txt`)
- Pattern order matters: negations are applied after inclusions
- Matching is case-sensitive by default

### Error Handling

### Error Handling

The tool provides clear error messages:

- Missing base ref: `Error: BASE_REF must be provided via --base-ref flag or BASE_REF environment variable`
- Git command failure: `Error: Failed to execute git diff: <error message>`
- Missing required flags: `Error: at least one --pattern is required`
- Invalid arguments: `Error: Unknown argument: <argument>` or `Error: <flag> requires a value`

### Prerequisites

- Docker (for consistent builds)
- Rust 1.91+ (for local development)
- Just (task runner)

### Building

Build the binary using Just commands:

```bash
# Debug build
just build

# Release build
just build-release

# Build in Docker (consistent Ubuntu environment)
just docker-build
```

The compiled binary will be at:
- Debug: `target/debug/gdf`
- Release: `target/release/gdf`

### Testing

Run tests with:

```bash
# Run all tests
just test

# Run tests with verbose output
just test-coverage
```

**Test coverage requirement: 100%**

### Development Workflow

```bash
# Check code without building (fast)
just check

# Run clippy linter
just lint

# Format code
just fmt

# Run all CI checks (fmt, lint, test)
just ci
```

### Dev Container

This project includes a dev container configuration. Open the project in VS Code and use "Reopen in Container" for a consistent development environment with all tools pre-installed (Rust, clippy, rustfmt, just).

## Installation

### GitHub Actions (recommended)

Use the action for automatic setup:

```yaml
- uses: your-org/git-diff-filter@v1
```

This action:
- Downloads and installs the `gdf` binary for your platform
- Handles permissions automatically
- No Rust installation required in consuming workflows

### Manual Installation in GitHub Actions

If you prefer manual installation:

```yaml
- name: Install gdf
  run: |
    curl -L https://github.com/your-org/git-diff-filter/releases/latest/download/gdf-linux-x86_64 -o gdf
    sudo mv gdf /usr/local/bin/
    chmod +x /usr/local/bin/gdf
```

### Build from Source

Requires Rust toolchain:

```bash
cargo install --path .
```

## Release Process

1. Docker builds the binary in a consistent Ubuntu environment
2. Binary is uploaded to GitHub Releases as `gdf-linux-x86_64`
3. The action downloads this pre-built binary when invoked (fast, no compilation needed)

### Creating a Release

Releases are automated via GitHub Actions. When you push a tag:

```bash
git tag v1.0.0
git push origin v1.0.0
```

The CI will build and upload the binary to GitHub Releases.

## How It Works

- **Docker**: Ensures consistent, reproducible builds on Ubuntu
- **GitHub Releases**: Stores pre-built binaries
- **Composite Action**: Downloads and installs the binary in ~5-10 seconds
- **No Rust installation required** in consuming workflows

## Performance Considerations

- Statically compiled Rust binary with minimal overhead
- Single git diff execution per invocation
- Efficient glob matching via compiled glob sets
- No runtime dependencies or startup costs
- Expected execution time: <100ms for typical monorepos

## Dependencies

- Git (must be available in PATH)
- No runtime dependencies (statically compiled Rust binary)
