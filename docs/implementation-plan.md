# Implementation Plan

## Overview
Build `gdf` feature-by-feature with 100% test coverage at each step. Each feature should be implemented, tested, and working before moving to the next.

## Phase 1: Foundation (Core Infrastructure)

### 1.1 ~~CLI Argument Parsing~~ ✅
**Goal**: Parse command-line arguments using only `std::env`

**Features**:
- [x] ~~Parse `-p`/`--pattern` (multiple occurrences)~~
- [x] ~~Parse `-b`/`--base-ref` (single value)~~
- [x] ~~Parse `-g`/`--github-output` (single value)~~
- [x] ~~Detect unknown flags~~
- [x] ~~Validate required flags present~~

**Acceptance Criteria**:
- [x] ~~Can parse: `gdf -p 'pattern' -b main`~~
- [x] ~~Can parse multiple patterns: `gdf -p 'a' -p 'b'`~~
- [x] ~~Error on missing required flags~~
- [x] ~~Error on unknown flags~~
- [x] ~~Error on flags without values~~
- [x] ~~100% test coverage~~

---

### 1.2 ~~Environment Variable Handling~~ ✅
**Goal**: Read and merge environment variables with CLI args

**Features**:
- [x] ~~Read `BASE_REF` from environment~~
- [x] ~~CLI flag `-b` overrides `BASE_REF` env var~~
- [x] ~~Read `GITHUB_OUTPUT` from environment (for file writing)~~

**Acceptance Criteria**:
- [x] ~~Uses `BASE_REF` when `-b` not provided~~
- [x] ~~CLI `-b` overrides `BASE_REF`~~
- [x] ~~Errors if neither provided~~
- [x] ~~Detects `GITHUB_OUTPUT` file path~~
- [x] ~~100% test coverage~~

---

### 1.3 Error Handling (SKIPPED FOR NOW)
**Goal**: Clean error messages to stderr

**Features**:
- [ ] Error enum with variants:
  - [ ] `MissingBaseRef`
  - [ ] `MissingPattern`
  - [ ] `GitCommandFailed`
  - [ ] `InvalidArgument`
  - [ ] `UnknownFlag`
- [ ] Print to stderr with `Error: <message>`
- [ ] Exit with code 1 on error

**Acceptance Criteria**:
- [ ] Clear error messages
- [ ] All errors exit with code 1
- [ ] Success exits with code 0
- [ ] 100% test coverage

---

## Phase 2: Git Integration

### 2.1 ~~Execute Git Diff~~ ✅
**Goal**: Run `git diff --name-only` and capture output

**Features**:
- [x] ~~Execute: `git diff --name-only <base-ref>..HEAD`~~
- [x] ~~Capture stdout (list of changed files)~~
- [x] ~~Handle git command failures~~
- [x] ~~Parse output into `Vec<String>` of file paths~~

**Acceptance Criteria**:
- [x] ~~Executes git command correctly~~
- [x] ~~Parses newline-separated file paths~~
- [x] ~~Handles empty diff (no changes)~~
- [x] ~~Errors if git command fails~~
- [x] ~~Errors if git not in PATH~~
- [x] ~~100% test coverage (mock git execution)~~ (95% - parse logic 100% covered)

---

## Phase 3: Glob Pattern Matching

### 3.1 Basic Glob Patterns
**Goal**: Implement simple glob matching

**Features**:
- [ ] `*` - Match any characters except `/`
- [ ] `?` - Match single character
- [ ] Literal string matching

**Acceptance Criteria**:
- [ ] `*.txt` matches `file.txt`, not `dir/file.txt`
- [ ] `file?.txt` matches `file1.txt`, not `file.txt`
- [ ] `README.md` matches exactly `README.md`
- [ ] 100% test coverage

**Implementation Notes**:
- Create `fn matches_pattern(path: &str, pattern: &str) -> bool`
- Handle edge cases (empty strings, etc.)

---

### 3.2 Directory Wildcards (`**`)
**Goal**: Match any number of directories

**Features**:
- [ ] `**` - Match zero or more directories
- [ ] `src/**/*.rs` matches all `.rs` files under `src/`
- [ ] `**/test.txt` matches `test.txt` at any level

**Acceptance Criteria**:
- [ ] `**/*.rs` matches `a/b/c/file.rs`
- [ ] `src/**/*.rs` matches `src/x/y/file.rs`
- [ ] `**/test.txt` matches `a/b/test.txt` and `test.txt`
- [ ] 100% test coverage

---

### 3.3 Character Classes
**Goal**: Support `[...]` character matching

**Features**:
- [ ] `[abc]` - Match any character in set
- [ ] `[a-z]` - Match character range
- [ ] `[!abc]` or `[^abc]` - Match any character NOT in set
- [ ] Multiple ranges: `[a-zA-Z0-9]`

**Acceptance Criteria**:
- [ ] `file[0-9].txt` matches `file1.txt`, not `filea.txt`
- [ ] `[Tt]est.txt` matches `Test.txt` and `test.txt`
- [ ] `[!.]*.txt` matches files not starting with `.`
- [ ] `[a-z]` matches lowercase letters
- [ ] 100% test coverage

---

### 3.4 Brace Expansion
**Goal**: Support `{a,b}` alternatives

**Features**:
- [ ] `{js,ts}` - Match either `js` or `ts`
- [ ] Nested patterns: `*.{js,ts}` → `*.js` or `*.ts`

**Acceptance Criteria**:
- [ ] `*.{js,ts}` matches `file.js` and `file.ts`
- [ ] `{a,b,c}` matches `a`, `b`, or `c`
- [ ] 100% test coverage

**Implementation Notes**:
- Expand `{a,b}` into multiple patterns internally
- Test each alternative

---

### 3.5 Negation Patterns
**Goal**: Support `!pattern` to exclude files

**Features**:
- [ ] `!pattern` - Exclude files matching pattern
- [ ] Applied after inclusion patterns
- [ ] Order matters: inclusions first, then exclusions

**Acceptance Criteria**:
- [ ] `-p 'src/**' -p '!*.md'` excludes markdown files from `src/`
- [ ] `-p '*.txt' -p '!test.txt'` includes all `.txt` except `test.txt`
- [ ] Negation only applies to previously matched files
- [ ] 100% test coverage

**Implementation Notes**:
- Track included and excluded patterns separately
- First apply inclusions to get candidates
- Then apply exclusions to filter

---

### 3.6 Anchoring and Directory Matching
**Goal**: Support `/pattern` and `pattern/`

**Features**:
- [ ] `/pattern` - Anchor to root (match only at repo root)
- [ ] `pattern/` - Match directories only
- [ ] `\` - Escape special characters

**Acceptance Criteria**:
- [ ] `/README.md` matches `README.md`, not `dir/README.md`
- [ ] `build/` matches directories named `build`
- [ ] `\*.txt` matches literal `*.txt` filename
- [ ] 100% test coverage

---

## Phase 4: Output and Integration

### 4.1 Match Detection
**Goal**: Determine if any files match patterns

**Features**:
- [ ] Check all changed files against all patterns
- [ ] Return `true` if ANY file matches ANY pattern (after exclusions)
- [ ] Return `false` if no matches

**Acceptance Criteria**:
- [ ] Returns `true` when at least one file matches
- [ ] Returns `false` when no files match
- [ ] Handles empty file list (no changes)
- [ ] Handles empty pattern list
- [ ] 100% test coverage

---

### 4.2 Plain Output Mode
**Goal**: Output `true` or `false` to stdout

**Features**:
- [ ] Write `true\n` or `false\n` to stdout
- [ ] Log comparison info to stderr: `Comparing: main..HEAD | Patterns: ... | Match: true`

**Acceptance Criteria**:
- [ ] stdout contains only `true` or `false`
- [ ] stderr contains debug info
- [ ] Exits with code 0 on success
- [ ] 100% test coverage

---

### 4.3 GitHub Actions Output Mode
**Goal**: Write to `$GITHUB_OUTPUT` when `-g` flag provided

**Features**:
- [ ] Output format: `<name>=true` or `<name>=false`
- [ ] Write to stdout: `<name>=true\n`
- [ ] Write to `$GITHUB_OUTPUT` file: `<name>=true\n` (append mode)
- [ ] Only write to file if `GITHUB_OUTPUT` env var exists

**Acceptance Criteria**:
- [ ] stdout: `reporting-api=true`
- [ ] File written if `GITHUB_OUTPUT` set
- [ ] File NOT written if `GITHUB_OUTPUT` not set
- [ ] File appended (multiple invocations don't overwrite)
- [ ] stderr contains debug info
- [ ] 100% test coverage

**Implementation Notes**:
- Use `std::fs::OpenOptions::new().append(true).open()`
- Gracefully handle file write failures

---

## Phase 5: Polish and Documentation

### 5.1 Comprehensive Testing
**Goal**: Ensure 100% test coverage

**Features**:
- [ ] Unit tests for all functions
- [ ] Integration tests for full CLI workflows
- [ ] Edge case coverage
- [ ] Error path coverage

**Acceptance Criteria**:
- [ ] 100% line coverage
- [ ] 100% branch coverage
- [ ] All edge cases tested
- [ ] All error paths tested

---

### 5.2 Performance Optimization
**Goal**: Ensure <100ms execution time

**Features**:
- [ ] Optimize pattern compilation
- [ ] Minimize allocations
- [ ] Efficient file path matching

**Acceptance Criteria**:
- [ ] Typical monorepo check completes in <100ms
- [ ] No unnecessary allocations
- [ ] Efficient matching algorithm

---

### 5.3 Documentation
**Goal**: Update README with any implementation details

**Features**:
- [ ] Document any limitations discovered
- [ ] Add troubleshooting section if needed
- [ ] Verify all examples work

**Acceptance Criteria**:
- [ ] README accurate
- [ ] All examples tested
- [ ] Code comments added where needed

---

## Testing Strategy

### Unit Tests
- Each glob pattern feature tested independently
- CLI parsing tested in isolation
- Git command execution mocked

### Integration Tests
- Full end-to-end scenarios
- Real git repository scenarios
- GitHub Actions output scenarios

### Test Coverage Requirements
- **100% coverage required before moving to next phase**
- Tests included in same file as implementation
- Use `#[cfg(test)]` modules

---

## Implementation Order Summary

1. [x] ~~CLI parsing~~
2. [x] ~~Environment variables~~
3. [ ] Error handling
4. [ ] Git integration
5. [ ] Basic glob (`*`, `?`, literals)
6. [ ] Directory wildcards (`**`)
7. [ ] Character classes (`[...]`)
8. [ ] Brace expansion (`{a,b}`)
9. [ ] Negation (`!pattern`)
10. [ ] Anchoring (`/`, trailing `/`, escaping)
11. [ ] Match detection
12. [ ] Plain output mode
13. [ ] GitHub Actions output mode
14. [ ] Testing and polish

Each feature must be fully implemented and tested before proceeding to the next.
