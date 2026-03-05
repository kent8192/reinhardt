# Contributing to Reinhardt

Thank you for your interest in contributing to Reinhardt! This document provides guidelines and best practices for contributing to the project.

## Table of Contents

- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Code Style & Conventions](#code-style--conventions)
- [Testing Guidelines](#testing-guidelines)
- [Issue Guidelines](#issue-guidelines)
- [Commit Guidelines](#commit-guidelines)
- [Pull Request Process](#pull-request-process)
- [Documentation](#documentation)
- [Getting Help](#getting-help)

---

## Getting Started

### Prerequisites

- Rust 1.91.1+ (2024 Edition required)
- Docker (required for TestContainers-based integration tests)
- PostgreSQL (optional - can use TestContainers instead)

### Fork and Clone

1. Fork the repository on GitHub
2. Clone your fork locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/reinhardt.git
   cd reinhardt
   ```
3. Add the upstream repository:
   ```bash
   git remote add upstream https://github.com/kent8192/reinhardt-web.git
   ```

### Building the Project

This project uses `cargo-make` for task automation. Install it first:

```bash
cargo install cargo-make
```

Build commands:

```bash
# Build the entire workspace (recommended)
cargo make build

# Build with all features
cargo make build --all-features

# Or use plain cargo if needed
cargo build --workspace --all --all-features
```

### Running Tests

This project uses `cargo-nextest` as the test runner. Install it first:

```bash
cargo install cargo-nextest
```

Test commands:

```bash
# Run all tests (unit + integration + doc) - recommended
cargo make test

# Run unit tests only
cargo make unit-test

# Run integration tests only
cargo make integration-test

# Run doc tests
cargo make doc-test

# Or use plain cargo-nextest
cargo nextest run --workspace --all-features

# Or use standard cargo test
cargo test --workspace --all --all-features
```

---

## Development Setup

### Project Structure

Reinhardt uses a workspace structure:

- **Functional crates**: `crates/reinhardt-*/` - Individual feature implementations
- **Tests crate**: `tests/` - Integration tests only
- **Each crate**: Contains its own unit tests in `tests/` or inline

### Module System

**IMPORTANT**: Reinhardt uses Rust 2024 Edition module system:

- L **DO NOT USE** `mod.rs` files
-  **USE** `module_name.rs` files instead
-  **DECLARE** modules in `lib.rs` or parent module with `mod module_name;`

Example:

```rust
// In lib.rs
pub mod http;
pub mod routing;

// File structure:
// src/
//   lib.rs
//   http.rs          //  Correct
//   routing.rs       //  Correct
//   http/mod.rs      // L DO NOT USE
```

---

## Code Style & Conventions

### General Guidelines

1. **Follow Rust naming conventions**:
   - `snake_case` for functions, variables, modules
   - `PascalCase` for types, traits, enums
   - `SCREAMING_SNAKE_CASE` for constants

2. **Code organization**:
   - Keep related functionality together
   - Maintain clear separation of concerns
   - Group imports logically (std, external crates, internal crates)

3. **String conversions**:
   - **MINIMIZE** `.to_string()` calls
   - Prefer borrowing and string slices where possible
   - Use `String::from()`, `format!()`, or `Cow<str>` when appropriate

### Code Cleanup

- **DELETE obsolete code immediately** - don't leave commented-out code
- **NO comments documenting deleted code** - Git history is the record
- Extract important notes to `instructions/IMPLEMENTATION_NOTES.md` if needed

### TODO and Placeholder Policy

**TODO Comments**:

- Use `// TODO:` for planning and unimplemented features
- Explain what needs to be implemented and why it's pending
- **DELETE** when functionality is implemented

**Runtime Markers**:

- Use `todo!()` for features that **WILL** be implemented
- Use `unimplemented!()` for features that **WILL NOT** be implemented (intentionally omitted)

**Placeholder Implementations**:

- **ALL** placeholder implementations **MUST** be marked with `todo!()` or `// TODO:`
- This includes:
  - Empty function bodies returning default values
  - Stub implementations with minimal logic
  - Mock implementations intended to be replaced
- **Exception**: Tests and documentation may use simplified implementations

Examples:

```rust
//  Good: Marked placeholder
pub fn get_cache_config() -> CacheConfig {
    todo!("Implement cache configuration loading from settings")
}

//  Good: Using TODO comment
pub fn validate_input(data: &str) -> Result<()> {
    // TODO: Add input validation logic - planned for next sprint
    Ok(())
}

//  Good: Intentionally not implemented
fn legacy_api() -> String {
    unimplemented!("This legacy API is intentionally not supported")
}

// L Bad: Unmarked placeholder
pub fn get_config() -> Config {
    Config::default()  // Looks production-ready!
}
```

### Path References

- **DO NOT USE** relative paths going up more than one level (e.g., `../..`)
- **PREFER** absolute paths or single-level relative paths (e.g., `../`)
- Deep relative paths make code harder to understand

### API Deprecation Policy

When marking a public API as deprecated, follow these requirements:

**Required attributes:**

```rust
#[deprecated(
    since = "0.2.0",
    note = "Use `new_function()` instead. Will be removed in 1.0.0."
)]
pub fn old_function() { ... }
```

**Requirements:**
- `since`: Version when the item was deprecated (follow semantic versioning)
- `note`: Concise migration path describing what to use instead and when removal is planned
- Both `since` and `note` fields are **MANDATORY** — bare `#[deprecated]` is not allowed

**Deprecation lifecycle:**
1. Add `#[deprecated(since = "...", note = "...")]` to the item
2. Keep the implementation functional until the planned removal version
3. Add a `deprecated` commit type entry in CHANGELOG (triggers dedicated section)
4. Remove the item in the version specified in `note`

**Naming convention for commit messages:**
```
deprecated(auth): mark `old_session_token()` as deprecated in favor of `session_token()`
```

---

## Testing Guidelines

### Testing Philosophy

1. **NO skeleton implementations**: Tests MUST contain meaningful assertions
   - Tests must be capable of failing when code is incorrect
   - L Bad: `assert!(true)` or empty test bodies
   -  Good: Tests with real assertions

2. **Use Reinhardt components**: Every test MUST use at least one Reinhardt crate component

3. **Documentation tests**: Implement documentation tests for all features
   - Don't duplicate doc tests as unit/integration tests

### Test Organization

**Unit Tests**:

- Use exactly ONE Reinhardt crate
- Place within the functional crate being tested
- Test that specific crate's functionality in isolation

**Integration Tests**:

- Use TWO or MORE Reinhardt crates
- **MUST** be placed in the `tests` crate
- Test interactions between multiple crates

**Dependency Rules**:

- Functional crates **SHOULD NOT** include other Reinhardt functional crates as `dev-dependencies`
- **Exception**: `reinhardt-test` MAY be included in `dev-dependencies` for test utilities and fixtures
- This guideline ensures unit tests remain isolated and focused on single-crate functionality

### Test Implementation

**Global State Management**:

- Tests modifying global state MUST be serialized using `serial_test` crate
- Use named serial groups: `#[serial(group_name)]`
- Common groups: `i18n`, `url_overrides`, etc.
- Always call cleanup functions in teardown

Example:

```rust
use serial_test::serial;

#[test]
#[serial(i18n)]
fn test_translation() {
    activate("fr", catalog);
    // test code
    deactivate(); // cleanup
}
```

**Test Cleanup**:

- **ALL** files, directories, or environmental changes created during tests **MUST** be deleted upon completion
- Use test fixtures, `Drop` implementations, or explicit cleanup

**Infrastructure Testing**:

- Use **TestContainers** for tests requiring actual infrastructure (databases, message queues)
- Prefer real infrastructure over mocks when feasible

---

## Issue Guidelines

### Before Creating an Issue

**Search First:**

Always search existing issues before creating a new one:

```bash
# Search via GitHub CLI
gh issue list --search "your query"

# Search open and closed issues
gh issue list --state open --search "database"
gh issue list --state closed --search "leak"
```

**Check Documentation:**

- [Issue Guidelines](instructions/ISSUE_GUIDELINES.md) for detailed issue policies
- [Examples](examples/) for usage patterns
- [CLAUDE.md](CLAUDE.md) for project-specific guidelines

### Issue Types

Use the appropriate issue template when creating issues:

| Template | Use When | Label Applied |
|----------|----------|---------------|
| `1-bug.yml` | Unexpected behavior or error | `bug` |
| `2-feature.yml` | New functionality request | `enhancement` |
| `3-documentation.yml` | Documentation issues | `documentation` |
| `4-question.yml` | Usage questions | `question` |
| `5-performance.yml` | Performance issues | `performance` |
| `6-ci_cd.yml` | CI/CD workflow failures | `ci-cd` |
| `7-security.yml` | Security vulnerabilities | `security`, `critical` |

### Issue Title Format

Issue titles must be:
- **Specific**: Clearly describe the problem or request
- **Concise**: Maximum 72 characters
- **Professional**: Use technical language
- **Uppercase Start**: Begin with uppercase letter

**Format Options:**

**Option 1: Type Prefix (Recommended)**
```
[Type]: <brief description>
```

**Option 2: Plain Descriptive**
```
<Brief descriptive title>
```

**Examples:**
- `Bug: Connection pool leak when using async transactions`
- `Feature: Add MySQL database backend support`
- `Performance: Slow query execution with large datasets`
- `Docs: Missing migration guide for v0.2.0`
- `CI: TestContainers integration tests failing on macOS`

### Issue Labels

**Type Labels (required):**
- `bug` - Confirmed bug or unexpected behavior
- `enhancement` - New feature or improvement request
- `documentation` - Documentation issues or improvements
- `question` - Questions about usage or implementation
- `performance` - Performance-related issues
- `ci-cd` - CI/CD workflow issues
- `security` - Security vulnerabilities or concerns

**Priority Labels (recommended):**
- `critical` - Blocks release or major functionality
- `high` - Important fix or feature
- `medium` - Normal priority
- `low` - Minor fix or enhancement

**Scope Labels (recommended):**
- `database`, `auth`, `orm`, `http`, `routing`, `api`, `admin`, `graphql`, `websockets`, `i18n`

### Security Issues

**Security vulnerabilities MUST be reported privately:**

- **DO NOT** create public issues for security vulnerabilities
- **DO** use GitHub Security Advisories for private reporting
- **DO** include reproduction steps and impact assessment

**Report via GitHub Security Advisories:**
```
https://github.com/kent8192/reinhardt-web/security/advisories
```

See [SECURITY.md](SECURITY.md) for detailed security policy.

### Creating Issues

**Via GitHub Web Interface:**

1. Go to [Issues](https://github.com/kent8192/reinhardt-web/issues)
2. Click "New Issue"
3. Select the appropriate template
4. Fill out all required fields
5. Submit

**Via GitHub CLI:**

```bash
# Create a bug report
gh issue create --title "Bug: Connection pool leak" \
  --body "Description..." \
  --label bug

# Create a feature request
gh issue create --title "Feature: Add MySQL support" \
  --body "Description..." \
  --label enhancement
```

For detailed issue guidelines, see [instructions/ISSUE_GUIDELINES.md](instructions/ISSUE_GUIDELINES.md).

---

## Commit Guidelines

### Commit Policy

**CRITICAL RULES**:

- **NEVER** create commits without explicit user instruction
- **NEVER** push commits without explicit user instruction
- Always wait for user confirmation before committing

### Commit Granularity

- Each commit should represent a **single logical change**
- **Each commit MUST be small enough to be explained in a single line**
  - If you can't describe the commit clearly in one line, it's too large
  - Split large changes into multiple commits
- Group files by change purpose at a fine-grained level

### Partial File Commits

When a file contains changes with different purposes, use:

**Method: Patch File Approach**
Use --patch option to apply patch file to the commit.

### Commit Message Format

**Structure**:

```
type(scope): Brief description in English

Body explaining the changes in detail.

Module Section:
- file/path.rs: +XXX lines - Description
  - Sub-detail 1
  - Sub-detail 2
- Removed: old_file.rs (reason)

Features:
- Feature 1
- Feature 2

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
```

**Type Values**:

- `feat`: New feature
- `fix`: Bug fix
- `refactor`: Code refactoring
- `test`: Testing changes
- `docs`: Documentation changes
- `chore`: Build/tooling changes
- `perf`: Performance improvements
- `style`: Code style changes

**Scope**: Module or component name (e.g., `orm`, `http`, `shortcuts`)

**Requirements**:

- Subject line: Be specific, not vague
- Body: Organize by module/component, list file changes with line counts
- Footer: Include Claude Code attribution and Co-Authored-By line
- Exactly one blank line between body and footer

**Style Reference**:
Always examine recent commits before writing new ones:

```bash
git log --pretty=format:"%s%n%b" -10
```

For detailed commit guidelines, see [COMMIT_GUIDELINE.md](instructions/COMMIT_GUIDELINE.md).

---

## Pull Request Process

### Before Submitting

1. **Ensure all tests pass**:

   ```bash
   cargo make test
   ```

2. **Run code formatting**:

   ```bash
   # Check formatting
   cargo make fmt-check

   # Auto-fix formatting
   cargo make fmt-fix
   ```

3. **Check for linting issues**:

   ```bash
   # Check linting
   cargo make clippy-check

   # Auto-fix linting issues
   cargo make clippy-fix
   ```

4. **Update documentation** (see [Documentation](#documentation) section)

### Submitting a Pull Request

1. **Create a feature branch**:

   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes** following the guidelines in this document

3. **Commit your changes** following commit guidelines

4. **Push to your fork**:

   ```bash
   git push origin feature/your-feature-name
   ```

5. **Open a Pull Request** on GitHub with:
   - Clear title describing the change
   - Description explaining what and why
   - References to related issues

### PR Review Process

- Maintainers will review your PR
- Address feedback and requested changes
- Once approved, maintainers will merge your PR

---

## Documentation

### Update Requirements

**ALWAYS** update documentation when implementing or modifying features:

- **README.md**: Project-level overview
- **Crate README.md**: Individual crate documentation
- **instructions/** directory: Detailed guides and standards

### Documentation Consistency

- Ensure consistency across all documentation levels

### Documentation Scope

Update documentation for:

- **New features**: Add descriptions, usage examples, API references
- **Modified features**: Update affected sections
- **Deprecated features**: Mark as deprecated, provide migration guides
- **Removed features**: Remove documentation, add migration notes

### Documentation Quality

- Ensure examples are tested and working
- Update code snippets to reflect current API
- Verify all links and references are valid
- Maintain consistency in terminology and formatting

---

## API Stabilization Process

### Final Comment Period (FCP)

The **Final Comment Period** (FCP) is a structured process for reaching consensus on significant API changes before they are stabilized. FCP ensures that all stakeholders have an opportunity to review and comment on proposed changes before they become part of the stable API.

### When FCP Applies

FCP is required for:

- **New public API additions** that affect the stable interface
- **Modifications to existing public APIs** that are backward-incompatible
- **Deprecation of stable APIs** that users depend on
- **Removal of previously deprecated APIs**
- **Significant behavioral changes** that affect the documented contract

FCP is **not required** for:
- Internal implementation changes
- Documentation-only changes
- Bug fixes that restore intended behavior
- Additions to unstable/experimental API categories

### FCP Process

1. **Proposal Phase**: Open a GitHub Issue using the [API Change Proposal template](.github/ISSUE_TEMPLATE/8-api_change.yml)
   - Describe the current API, proposed change, and rationale
   - Classify the change (breaking/non-breaking)
   - Provide a migration path for breaking changes

2. **Discussion Phase** (minimum 7 days for non-breaking, 14 days for breaking):
   - Community and maintainers review the proposal
   - Alternative approaches are discussed
   - Concerns and objections are raised and addressed

3. **FCP Announcement**: A maintainer posts an FCP announcement comment on the issue
   - States the proposed disposition (merge/postpone/close)
   - Begins the final comment period countdown
   - Labels the issue with `fcp-merge`, `fcp-postpone`, or `fcp-close`

4. **Final Comment Period** (minimum 7 days):
   - Community has a final opportunity to raise concerns
   - Any new objections restart the discussion phase
   - No new concerns → proceed to disposition

5. **Resolution**: After FCP completes without new objections
   - Issue is closed with final decision documented
   - Implementation PR is opened referencing the stabilization issue
   - API is marked as stable in the next minor/major release

### API Stability Categories

| Category | Description | FCP Required |
|----------|-------------|--------------|
| `Stable` | Fully supported, covered by SemVer | Yes (for changes) |
| `Experimental` | Subject to change without major version bump | No |
| `Internal` | Not part of the public API contract | No |

### Marking API Stability

Use `#[doc(cfg(...))]` and doc comments to communicate API status:

```rust
/// Stable API - covered by SemVer guarantees.
pub fn stable_function() {}

/// **Experimental**: This API is subject to change.
///
/// May be modified or removed in future minor versions.
#[cfg_attr(docsrs, doc(cfg(feature = "experimental")))]
pub fn experimental_function() {}
```

### Deprecation Policy

When an API is deprecated:

1. Add `#[deprecated(since = "x.y.z", note = "Use `new_function()` instead")]`
2. Open a GitHub Issue documenting the deprecation timeline
3. Provide a migration guide in the documentation
4. Maintain the deprecated API for at least one minor version cycle
5. Remove the API only after FCP and appropriate deprecation period

### Resources

- [API Stability Policy](docs/API_STABILITY.md) - Detailed stability guarantees
- [API Change Proposal Template](.github/ISSUE_TEMPLATE/8-api_change.yml) - Template for API proposals
- [SemVer specification](https://semver.org/) - Versioning guidelines

---

## Getting Help

### Resources

- [Getting Started Guide](/quickstart/getting-started/)
- [Feature Flags Guide](/docs/feature-flags/)
- [Issue Guidelines](instructions/ISSUE_GUIDELINES.md) - Issue creation and management
- [Pull Request Guidelines](instructions/PR_GUIDELINE.md) - PR policies and procedures
- [Security Policy](SECURITY.md) - Security vulnerability reporting
- [Code of Conduct](CODE_OF_CONDUCT.md) - Community standards
- [Project Instructions](CLAUDE.md)
- GitHub Discussions (for questions and ideas)
- GitHub Issues (for bug reports)

### Before Asking

Please check:

-  [Getting Started Guide](/quickstart/getting-started/)
-  [Issue Guidelines](instructions/ISSUE_GUIDELINES.md)
-  [Pull Request Guidelines](instructions/PR_GUIDELINE.md)
-  [Examples](examples/)
-  Existing GitHub Issues and Discussions
-  [CLAUDE.md](CLAUDE.md) for project-specific guidelines

---

## Quick Reference

### Critical Rules

**Code & Module System**:

- L NO `mod.rs` files
- L NO TODO/NOTE comments in user-facing placeholders
- L NO unmarked placeholder implementations
-  USE 2024 edition module system
-  MARK ALL placeholders with `todo!()` or `// TODO:`

**Testing**:

- L NO skeleton tests
- L NO cross-crate functional dev-dependencies (except reinhardt-test)
-  CLEAN UP all test artifacts
-  SERIALIZE tests with global state using `#[serial]`

**File Management**:

- L NO saving files to project directory (use `/tmp`)
- L NO relative paths with more than one level up
-  DELETE `/tmp` files when done

**Documentation**:

- L NO outdated documentation after code changes
-  UPDATE documentation with code changes

**Commits**:

- L NO commits without user instruction
- L NO batch commits without confirmation
-  SPLIT commits by logical purpose
-  KEEP commits small and focused


### Common Commands

**Development Tools**:

```bash
# Install required tools
cargo install cargo-make
cargo install cargo-nextest
```

**Build & Test**:

```bash
# Build
cargo make build

# Test
cargo make test

# Unit tests only
cargo make unit-test

# Integration tests only
cargo make integration-test
```

**Code Quality**:

```bash
# Format (check)
cargo make fmt-check

# Format (fix)
cargo make fmt-fix

# Lint (check)
cargo make clippy-check

# Lint (fix)
cargo make clippy-fix
```
---

## License

By contributing to Reinhardt, you agree that your contributions will be licensed under the BSD 3-Clause License.

---

Thank you for contributing to Reinhardt! >�