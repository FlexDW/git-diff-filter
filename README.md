# `gdf`

A command-line utility for detecting changes in a monorepo by comparing git diffs against glob patterns. Designed to integrate seamlessly with GitHub Actions workflows but works in any CI/CD environment or local development.

## Overview

`gdf` analyzes git diffs to determine which parts of the code changed, helping workflows decide whether to run specific jobs or steps. Common use cases:

- **Conditional Docker rebuilds** - rebuild and push a service image only when its source files (or shared dependencies) actually changed; uses the service's own `.dockerignore` to define what matters
- **Monorepo CI gating** - skip expensive build, test, or deploy jobs for services that didn't change
- **Glob pattern matching** - match changed files against patterns to flag which components were modified

## Pattern Support

### Fully Implemented
- `*` - Match zero or more characters (except `/`)
- `**` - Match zero or more directories (globstar)
- `?` - Match exactly one character (except `/`)
- `[abc]` - Character classes (match any character in set)
- `[a-z]` - Character ranges (match character in range)
- `[!abc]` or `[^abc]` - Negated character classes (match any character NOT in set)
- `\\` - Escape special characters (`\\*`, `\\?`, `\\[`, `\\]`, `\\\\`)
- `!pattern` - Exclusion patterns (exclude files matching pattern)
- `/pattern` - Root anchoring (match at repository root only)
- `pattern/` - Directory prefix matching (match directory and all contents)

### Not Implemented
- `{js,ts}` - Brace expansion (**out of scope** - use multiple `-p` flags instead)
  - Instead of: `gdf -p '*.{js,ts}'`
  - Use: `gdf -p '*.js' -p '*.ts'`

## Usage

**Pattern mode** - match changed files against glob patterns:
```bash
gdf -p <glob> [-p <glob>...] [-b <base-ref>] [-g <name>]
```

**Container mode** - detect changes inside a container directory, respecting `.dockerignore`:
```bash
gdf -c <dir> [-c <dir>...] [-b <base-ref>] [-g <name>]
```

**Combined** - true if patterns match *or* any container has changes:
```bash
gdf -p <glob> [-p <glob>...] -c <dir> [-c <dir>...] [-b <base-ref>] [-g <name>]
```

At least one `-p` or `-c` must be provided. Flags can be mixed freely.

## Conditional Docker Builds (GitHub Actions)

The primary use case for `-c` is avoiding unnecessary Docker image rebuilds. `gdf` uses the service's own `.dockerignore` to decide whether any relevant file changed, the same rules Docker uses when building the image.

### Example repo layout

```
services/
  api/
    .dockerignore
    Dockerfile
    src/
    tests/
  worker/
    .dockerignore
    Dockerfile
    src/
libs/           # shared code consumed by both services
```

### Example `services/api/.dockerignore`

```
# Never relevant to the image
tests/
*.md
.env*

# Always relevant even if otherwise ignored
!**/*.proto
```

### Workflow

```yaml
jobs:
  detect-changes:
    name: Detect changes
    runs-on: ubuntu-latest
    outputs:
      api: ${{ steps.changes.outputs.api }}
      worker: ${{ steps.changes.outputs.worker }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0  # required for git history

      - uses: FlexDW/git-diff-filter@v1

      - name: Detect changes
        id: changes
        env:
          BASE_REF: ${{ github.event.repository.default_branch }}
        run: |
          gdf -g api -c 'services/api' -p 'libs/**'
          gdf -g worker -c 'services/worker' -p 'libs/**'

  build-api:
    name: Build and push api
    needs: detect-changes
    if: needs.detect-changes.outputs.api == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build and push
        run: |
          docker build -t myorg/api:${{ github.sha }} services/api/
          docker push myorg/api:${{ github.sha }}

  build-worker:
    name: Build and push worker
    needs: detect-changes
    if: needs.detect-changes.outputs.worker == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build and push
        run: |
          docker build -t myorg/worker:${{ github.sha }} services/worker/
          docker push myorg/worker:${{ github.sha }}
```

**Key behaviour:** a commit that only changes `services/api/README.md` returns `false` for `api` - no rebuild triggered, because `*.md` is ignored by the `.dockerignore`. A commit that changes `libs/` returns `true` for both services.

> **Note:** `.dockerignore` patterns are matched against paths relative to the service directory. `*.md` matches `services/api/README.md`; use `**/*.md` to match files in subdirectories too.

## Arguments and Flags

### Required (one of)

- `-p, --pattern <glob>` - Glob pattern to match against changed files (can be specified multiple times)
  - **Note**: Wrap patterns in quotes to prevent shell expansion (e.g., `'libs/**'` not `libs/**`)
- `-c, --container <dir>` - Path to a container directory (e.g., `services/api`); can be specified multiple times
  - Returns `true` if any file inside `<dir>` changed, after applying `.dockerignore` rules if present
  - Without a `.dockerignore`, any change inside the directory is relevant
  - With a `.dockerignore`, rules are applied in-order: plain patterns remove files from the relevant set; `!pattern` lines restore them
  - Can be combined with `-p`; the overall result is true if patterns match **or** any container has changes

### Optional Flags

- `-b, --base-ref <ref>` - The git reference to compare against (e.g., `refs/tags/production`, `main`, `HEAD~1`)
  - If not provided, it will try to use `BASE_REF` environment variable
  - Command-line flag takes precedence over environment variable
- `-g, --github-output <name>` - Enable GitHub Actions integration by specifying the output variable name
  - When provided, outputs in format `<name>=true|false` and writes to `$GITHUB_OUTPUT` file
  - When omitted, outputs plain `true` or `false` to stdout

### Environment Variables

- `BASE_REF` - The git reference to compare against (fallback if `--base-ref` is not provided)
  - Either `--base-ref` flag or `BASE_REF` environment variable is required
  - Command-line flag takes precedence

## Behavior

1. Reads the base reference from `--base-ref` flag or falls back to `BASE_REF` environment variable
2. Executes `git diff --name-only $BASE_REF..HEAD` to get list of changed files
3. Matching logic:

   **Pattern mode** (`-p`):
   - Separate patterns into inclusion patterns (no `!` prefix) and exclusion patterns (`!` prefix)
   - Match all changed files against all inclusion patterns - build a deduplicated set
   - Remove from that set any file matching an exclusion pattern
   - Exclusions are order-independent and applied globally
   - Returns `true` if any files remain after exclusions

   **Container mode** (`-c`, repeated per directory):
   - For each specified `<dir>`, collect all changed files inside it (matched against `<dir>/**`)
   - If no `.dockerignore` exists in `<dir>`, that directory contributes `true` immediately if any such files exist
   - If a `.dockerignore` exists, apply its rules **in order** to the collected set:
     - Plain pattern (e.g., `*.log`) - remove matching files from the relevant set
     - Exception pattern (e.g., `!important.log`) - restore matching files to the relevant set
     - Comments (`#`) and blank lines are ignored
   - That directory contributes `true` if any files remain in the relevant set
   - Note: `.dockerignore` `*` only matches within one directory level; use `**` for recursive matching

   **Combined:** the overall result is `true` if the pattern check passes **or** any container check passes (short-circuit OR, patterns evaluated first).

4. Output:
   - **stdout** (without `-g` flag): Outputs `true` or `false`
   - **stdout** (with `-g` flag): Outputs `<name>=true` or `<name>=false` AND writes to `$GITHUB_OUTPUT` file (if the environment variable exists)

## Exit Codes

- `0` - Success (always, even if no files match)
- `1` - Error (missing base ref, git command failed, invalid arguments, etc.)

## Examples

### Basic Usage (Plain Output)

```bash
gdf -p 'services/admin/**' -b refs/tags/production
# stdout: true
```

### GitHub Actions Integration

```bash
gdf -g admin-api -p 'services/admin/**' -b refs/tags/production
# stdout: admin-api=true
# Writes to $GITHUB_OUTPUT: admin-api=true
```

### Using Environment Variable for Base Ref

```bash
export BASE_REF=refs/tags/test
gdf -p 'services/admin/**'
# stdout: false
```

### Multiple Glob Patterns

```bash
# Match if any of these patterns match
gdf -p 'libs/**' -p 'package.json' -p 'lerna.json' -b main
# stdout: true
```

### Container Mode (without `.dockerignore`)

```bash
# Any change inside services/api/ triggers true
gdf -c 'services/api' -b main
# stdout: true
```

### Container Mode (with `.dockerignore`)

Given `services/api/.dockerignore`:
```
# Ignore generated and vendor directories
vendor/
*.generated.go

# But always track proto files
!**/*.proto
```

```bash
gdf -c 'services/api' -b main
# Returns true only if non-ignored files changed inside services/api/
```

### Multiple Containers

```bash
# True if either service has relevant changes
gdf -c 'services/api' -c 'services/web' -b main
# stdout: true
```

### Combined Pattern + Container Mode

```bash
# True if shared libs changed OR either service has relevant changes
gdf -p 'libs/**' -c 'services/api' -c 'services/web' -b main
# stdout: true
```

### Container Mode in GitHub Actions

```bash
gdf -g api-service -c 'services/api' -b main
# stdout: api-service=true
# Writes to $GITHUB_OUTPUT: api-service=true
```

### Root-Anchored Patterns

```bash
# Match only at repository root (not in subdirectories)
gdf -p '/README.md' -b main
# Matches: README.md
# Does NOT match: docs/README.md
```

### Directory Prefix Matching

```bash
# Match directory and all contents
gdf -p 'build/' -b main
# Matches: build, build/output.js, build/dist/app.css
# Does NOT match: buildx/file.txt

# Combine with globstar
gdf -p '**/dist/' -b main
# Matches: dist/app.js, src/dist/bundle.js, a/b/c/dist/main.css
```

### Excluding Files with Exclusion Patterns

```bash
# Match all source files except markdown
gdf -p 'src/**' -p '!*.md' -b main
# stdout: true

# Match files in src/ but exclude test directories and markdown
gdf -p 'src/**' -p '!**/test/**' -p '!*.md' -b main
# Matches .rs files but not .md or files in test/ subdirectories

# Multiple exclusions are order-independent
gdf -p '!*.md' -p 'src/**' -p '!*.txt' -b main
# Same result regardless of order

# Exclusions only affect matched files
gdf -p '!*.md' -b main
# Always returns false (no inclusions to match)
```

### Question Mark Wildcard

```bash
# Match files with exactly one character before extension
gdf -p 'file?.txt' -b main
# Matches: file1.txt, fileA.txt, file_.txt
# Does NOT match: file.txt, file12.txt

# Multiple question marks
gdf -p 'test??.rs' -b main
# Matches: test01.rs, testab.rs
# Does NOT match: test1.rs, test.rs, test123.rs

# Question mark does not match /
gdf -p 'src?main.rs' -b main
# Matches: srcXmain.rs
# Does NOT match: src/main.rs

# Combine with other patterns
gdf -p '*.?s' -b main
# Matches: file.rs, test.ts, app.js
# Does NOT match: style.css
```

### Character Classes

```bash
# Match files with digits in name
gdf -p 'file[0-9].txt' -b main
# Matches: file1.txt, file5.txt
# Does NOT match: filea.txt, file.txt

# Match hexadecimal characters
gdf -p 'img[0-9a-f].png' -b main
# Matches: img0.png, img9.png, imga.png, imgf.png

# Negated character class
gdf -p '[!.]*.txt' -b main
# Matches files not starting with dot
```

### Plain Boolean Check in Scripts

```bash
# Simple boolean check
result=$(gdf -p 'src/**' -b main)
if [ "$result" = "true" ]; then
  echo "Source code changed"
fi
# stdout: true
```

### Conditional Build in Shell Script

```bash
# Set base ref
export BASE_REF=main

# Check multiple components
web_api=$(gdf -p 'services/web/**' -p 'libs/**')
mobile_api=$(gdf -p 'services/mobile/**' -p 'libs/**')
worker=$(gdf -p 'services/worker/**')

# Build only changed components
[ "$web_api" = "true" ] && npm run build:web-api
[ "$mobile_api" = "true" ] && npm run build:mobile-api
[ "$worker" = "true" ] && npm run build:worker

echo "Build complete"
```

### Flag Overrides Environment Variable

```bash
export BASE_REF=refs/tags/production
gdf -g web-api -p 'services/web/**' -b main
# Uses main, not refs/tags/production (CLI flag takes precedence)
# stdout: web-api=true
```

### GitHub Actions Integration

Complete workflow example:

```yaml
jobs:
  setup:
    name: 'Detect changes'
    runs-on: ubuntu-latest
    outputs:
      web-api: ${{ steps.changes.outputs.web-api }}
      mobile-api: ${{ steps.changes.outputs.mobile-api }}
      worker-service: ${{ steps.changes.outputs.worker-service }}
      admin-api: ${{ steps.changes.outputs.admin-api }}
      frontend: ${{ steps.changes.outputs.frontend }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0  # Required for git history

      - name: Install gdf
        uses: FlexDW/git-diff-filter@v1

      - name: Detect component changes
        id: changes
        run: |
          export BASE_REF=main
          gdf -g web-api -p 'services/web/**' -p 'libs/**'
          gdf -g mobile-api -p 'services/mobile/**' -p 'libs/**'
          gdf -g worker-service -p 'services/worker/**' -p 'libs/**'
          gdf -g admin-api -p 'services/admin/**' -p 'libs/**'
          gdf -g frontend -p 'apps/frontend/**' -p 'libs/**'
          # Combined: shared libs OR container-specific changes (respecting .dockerignore)
          gdf -g api-docker -p 'libs/**' -c 'services/api'

  build:
    name: 'Build changed components'
    needs: setup
    runs-on: ubuntu-latest
    strategy:
      matrix:
        component: [web-api, mobile-api, worker-service, admin-api, frontend]
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

- Custom glob pattern matching using only Rust's standard library
- Patterns are matched against relative file paths from repository root
- **Supported patterns**:
  - `**` - Match any number of directories (e.g., `src/**/*.rs`)
    - Must be followed by `/` to cross directories: `**/test/*.rs`
    - `**` without `/` behaves like `*` (doesn't cross directories)
  - `*` - Match any characters except `/` (e.g., `*.json`)
  - `?` - Match exactly one character except `/` (e.g., `file?.txt`)
  - `[abc]` - Match any character in brackets (e.g., `[Tt]est.txt`)
  - `[a-z]` - Match character range (e.g., `file[0-9].txt`)
  - `[!abc]` or `[^abc]` - Match any character NOT in brackets (e.g., `[!.]*.txt`)
  - `\\` - Escape special characters (e.g., `\\*.txt` matches literal `*.txt`)
  - `!pattern` - Exclude files matching pattern (must have inclusion patterns too)
  - `/pattern` - Anchor pattern to root directory (e.g., `/README.md`)
  - `pattern/` - Match directory and all contents (e.g., `build/`)
- **Pattern behavior**:
  - Leading `/` is stripped (anchors to root)
  - Trailing `/` is stripped (matches directory prefix)
  - Patterns can match directory prefixes: `src/bin` matches `src/bin/main.rs`
  - Exclusions are order-independent and apply to all inclusion results (pattern mode only)
  - In container mode, `.dockerignore` rules are order-dependent: later rules override earlier ones
  - `.dockerignore` `*` only matches within a single directory level; `**` is required for recursive matching
- **Not supported**:
  - `{a,b}` - Brace expansion (OUT OF SCOPE - use multiple `-p` flags instead)
- Matching is case-sensitive

### Error Handling

The tool provides clear error messages:

- Missing base ref: `Error: BASE_REF must be provided via -b/--base-ref flag or BASE_REF environment variable`
- Git command failure: `Error: Failed to execute git diff: <error message>`
- Missing required flags: `Error: at least one --pattern or --container is required`
- Failed to read `.dockerignore`: `Error: Failed to read <path>/.dockerignore: <error message>`
- Invalid arguments: `Error: Unknown flag: <flag>` or `Error: <flag> requires a value`

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
- uses: FlexDW/git-diff-filter@v1
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
    curl -L https://github.com/FlexDW/git-diff-filter/releases/latest/download/gdf-linux-x86_64 -o gdf
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
- Efficient batch matching algorithm:
  - Single-pass state machine for pattern matching
  - Processes all paths in parallel against each pattern
  - Uses `swap_remove` optimization to minimize allocations
  - Byte-level processing for control characters (no UTF-8 overhead)
- No runtime dependencies or startup costs
- Expected execution time: <100ms for typical monorepos
- 167 comprehensive tests ensure correctness

## Dependencies

- Git (must be available in PATH)
- No runtime dependencies (statically compiled Rust binary)
