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

### 3.1 ~~Basic Glob Patterns~~ ✅
**Goal**: Implement simple glob matching

**Features**:
- [x] ~~`*` - Match any characters except `/`~~
- [x] ~~`?` - Match single character~~
- [x] ~~Literal string matching~~

**Acceptance Criteria**:
- [x] ~~**Literal matching**: `README.md` matches exactly `README.md`, not `README.mdx`~~
- [x] ~~**Star wildcard (`*`)**: 
  - [x] ~~`*.txt` matches `file.txt`, `test.txt`, but not `dir/file.txt` (no slash)~~
  - [x] ~~`file*` matches `file.txt`, `file123`, `file` (including empty)~~
  - [x] ~~`*` matches `README.md`, `.hidden`, but not `dir/file.txt`~~
  - [x] ~~`a*b` matches `ab`, `axxxb`, not `a/b`~~
- [x] ~~**Question mark (`?`)**: 
  - [x] ~~`file?.txt` matches `file1.txt`, `fileX.txt`, not `file.txt` or `file12.txt`~~
  - [x] ~~`???` matches exactly 3 characters~~
  - [x] ~~`test?.md` doesn't match `test/.md` (no slash)~~
- [x] ~~**Mixed patterns**: `file*.txt` matches `file1.txt`, `file_test.txt`~~
- [x] ~~**Path separators**: `*` and `?` don't match `/` character~~
- [x] ~~**Escaping**: `\*` matches literal `*`, `\?` matches literal `?`~~
- [x] ~~**Empty/edge cases**: empty pattern only matches empty string~~
- [x] ~~100% test coverage~~

**Implementation Notes**:
- ~~Create `fn matches_pattern(path: &str, pattern: &str) -> bool`~~
- ~~Handle edge cases (empty strings, etc.)~~

---

### 3.2 ~~Directory Wildcards (`**`)~~ ✅
**Goal**: Match any number of directories

**Features**:
- [x] ~~`**` - Match zero or more directories~~
- [x] ~~`src/**/*.rs` matches all `.rs` files under `src/`~~
- [x] ~~`**/test.txt` matches `test.txt` at any level~~

**Acceptance Criteria**:
- [x] ~~`**/*.rs` matches `a/b/c/file.rs`~~
- [x] ~~`src/**/*.rs` matches `src/x/y/file.rs`~~
- [x] ~~`**/test.txt` matches `a/b/test.txt` and `test.txt`~~
- [x] ~~100% test coverage~~

---

### 3.3 ~~Character Classes~~ ✅
**Goal**: Support `[...]` character matching

**Features**:
- [x] ~~`[abc]` - Match any character in set~~
- [x] ~~`[a-z]` - Match character range~~
- [x] ~~`[!abc]` or `[^abc]` - Match any character NOT in set~~
- [x] ~~Multiple ranges: `[a-zA-Z0-9]`~~

**Acceptance Criteria**:
- [x] ~~`file[0-9].txt` matches `file1.txt`, not `filea.txt`~~
- [x] ~~`[Tt]est.txt` matches `Test.txt` and `test.txt`~~
- [x] ~~`[!.]*.txt` matches files not starting with `.`~~
- [x] ~~`[a-z]` matches lowercase letters~~
- [x] ~~100% test coverage~~

---

### 3.4 Brace Expansion — ⚠️ **OUT OF SCOPE** ⚠️
**Goal**: Support `{a,b}` alternatives

**STATUS: NOT IMPLEMENTED - OUT OF SCOPE FOR CURRENT VERSION**

Brace expansion requires preprocessing patterns into multiple alternatives, which adds significant complexity. This feature is deferred to a future version.

**Features** (deferred):
- [ ] `{js,ts}` - Match either `js` or `ts`
- [ ] Nested patterns: `*.{js,ts}` → `*.js` or `*.ts`

**In Scope (Current)**:
- [x] ~~Panic with clear error message when `{` or `}` detected in pattern~~
- [x] ~~Error message directs users to use multiple `-p` flags instead~~

**Workaround**: Use multiple `-p` flags instead:
- Instead of: `gdf -p '*.{js,ts}'`
- Use: `gdf -p '*.js' -p '*.ts'`

---

### 3.5 Anchoring and Directory Matching
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

### 3.6 Matcher Orchestration
**Goal**: Implement multi-pattern matching with inclusion/exclusion logic

**Features**:
- [ ] Separate patterns into inclusion and exclusion lists
- [ ] Patterns starting with `!` are exclusions, others are inclusions
- [ ] Match all changed files against inclusion patterns first
- [ ] Build set of matched file paths (deduplicated)
- [ ] Remove file paths that match any exclusion pattern
- [ ] Return `true` if any files remain after exclusions

**Acceptance Criteria**:
- [ ] **Single inclusion pattern**: `-p '*.txt'` matches `file.txt`
- [ ] **Multiple inclusion patterns**: `-p '*.txt' -p '*.rs'` matches if ANY pattern matches
- [ ] **Deduplication**: Same file matched by multiple patterns only counted once
- [ ] **Simple exclusion**: `-p 'src/**' -p '!*.md'` includes all `src/` files except `.md`
- [ ] **Order-independent exclusions**: Exclusions apply to all inclusion results regardless of order
- [ ] **Exclusion only affects matched files**: `-p '!*.md'` by itself matches nothing (no inclusions)
- [ ] **Multiple exclusions**: `-p 'src/**' -p '!*.md' -p '!*.txt'` excludes both `.md` and `.txt`
- [ ] **Empty pattern list**: Returns `false` for any file
- [ ] **Empty file list**: Returns `false` for any pattern
- [ ] 100% test coverage

**Implementation Notes**:
- Create `fn matches_with_exclusions(files: &[String], patterns: &[String]) -> bool`
- Separate patterns: `patterns.iter().partition(|p| p.starts_with('!'))`
- Strip `!` prefix from exclusion patterns before matching
- Use `HashSet` for deduplication of matched files
- Exclusions processed after all inclusions collected
- Can use all implemented pattern types (basic, **, [], {}, /, anchoring)

**Current Implementation Status**:
- ❌ Current code only does simple OR matching across patterns
- ❌ No exclusion pattern handling
- ❌ No deduplication
- ✅ Basic single-pattern matching works

---

## Phase 4: Output and Integration

### 4.1 Plain Output Mode
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

### 4.2 GitHub Actions Output Mode
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
3. [ ] Error handling (basic errors work, comprehensive error enum deferred)
4. [x] ~~Git integration~~
5. [x] ~~Basic glob (`*`, `?`, literals)~~
6. [x] ~~Directory wildcards (`**`)~~
7. [x] ~~Character classes (`[...]`)~~
8. [ ] ~~Brace expansion (`{a,b}`)~~ **OUT OF SCOPE** - use multiple `-p` flags
9. [ ] Anchoring (`/`, trailing `/`) - DEFERRED
10. [ ] **Matcher orchestration (inclusion/exclusion/deduplication)** ← NEXT: Implement negation patterns
11. [ ] Plain output mode
12. [ ] GitHub Actions output mode
13. [ ] Testing and polish

Each feature must be fully implemented and tested before proceeding to the next.
