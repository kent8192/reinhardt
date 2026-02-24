# CLAUDE.md

## Purpose

This file contains project-specific instructions for the Reinhardt project. These rules ensure code quality, maintainability, and consistent practices across the Rust codebase.

For detailed standards, see documentation in `docs/` directory.

---

## Project Overview

See README.md for project details.

**Repository URL**: https://github.com/kent8192/reinhardt-web

---

## Tech Stack

- **Language**: Rust 2024 Edition
- **Module System**: MUST use 2024 edition (NO `mod.rs`)
- **Database**: SeaQuery v1.0.0-rc for building SQL queries
- **Testing**: Rust's built-in framework + TestContainers for infrastructure
- **Build**: Cargo workspace with multiple crates

---

## Critical Rules

### Module System
**MUST use `module.rs` + `module/` directory structure (Rust 2024 Edition)**
**NEVER use `mod.rs` files** (deprecated)

See instructions/MODULE_SYSTEM.md for comprehensive module system standards including:
- Basic module patterns (small, medium, large)
- Visibility control with `pub use`
- Anti-patterns to avoid
- Migration guide from old style

### Code Style

**Key Requirements:**
- **ALL code comments MUST be written in English** (no exceptions)
- MINIMIZE `.to_string()` calls - prefer borrowing
- DELETE obsolete code immediately
- NO deletion record comments in code
- NO relative paths beyond `../` (use absolute paths)
- Mark ALL placeholders with `todo!()` or `// TODO:` comment
- Document ALL `#[allow(...)]` attributes with explanatory comments (see @instructions/ANTI_PATTERNS.md)

**Unimplemented Features Notation:**
- `todo!()` - Features that WILL be implemented
- `unimplemented!()` - Features that WILL NOT be implemented (intentionally omitted)
- `// TODO:` - Planning notes
- **DELETE** `todo!()` and `// TODO:` when implemented
- **KEEP** `unimplemented!()` for permanently excluded features
- **NEVER** use alternative notations (`FIXME:`, `Implementation Note:`, etc.)

**CI Enforcement (TODO Check):**
- New `todo!()`, `// TODO`, and `// FIXME` added in PRs are detected and blocked by TODO Check CI
- `unimplemented!()` is exempt (reserved for permanently excluded features)
- Existing TODOs are not flagged due to diff-aware scanning
- `cargo make clippy-todo-check` enforces `clippy::todo`, `clippy::unimplemented`, and `clippy::dbg_macro` as deny lints
- Local pre-check: `semgrep scan --config .semgrep/ --error --metrics off`

See instructions/ANTI_PATTERNS.md for comprehensive anti-patterns guide.

### Testing

**Core Principles:**
- NO skeleton tests (all tests MUST have meaningful assertions)
- EVERY test MUST use at least one Reinhardt component
- Unit tests: Test single component behavior, place in functional crate
- Integration tests: Test integration points between components
  - Cross-crate integration: Place in `tests/` crate
  - Within-crate integration: Can place in functional crate
- Functional crates MUST NOT include other Reinhardt crates in `dev-dependencies`
- ALL test artifacts MUST be cleaned up
- Global state tests MUST use `#[serial(group_name)]`
- Use strict assertions (`assert_eq!`) instead of loose matching (`contains`)
- Follow Arrange-Act-Assert (AAA) pattern for test structure

See instructions/TESTING_STANDARDS.md for comprehensive testing standards including:
- Testing philosophy (TP-1, TP-2)
- Test organization (TO-1, TO-2)
- Test implementation (TI-1 ~ TI-6)
- Infrastructure testing (IT-1)

### File Management

**Critical Rules:**
- **NEVER** save temp files to project directory (use `/tmp`)
- **IMMEDIATELY** delete `/tmp` files when no longer needed
- **IMMEDIATELY** delete backup files (`.bak`, `.backup`, `.old`, `~` suffix)
- NO relative paths beyond one level up (`../..` is forbidden)
- Use absolute paths or single-level relative paths

### Documentation

**Update Requirements:**
- **ALWAYS** update docs when code changes (same workflow)
- Update all relevant: README.md, crate README, docs/, lib.rs
- Planned features go in `lib.rs` header, NOT in README.md
- Test all code examples
- Verify all links are valid
- **NEVER** document user requests or AI assistant interactions in project documentation
  - Documentation must describe technical reasons, design decisions, and implementation details
  - Avoid phrases like "User requested...", "As requested by...", "User asked..."
  - Focus on the "why" (technical rationale) not the "who asked"

See instructions/DOCUMENTATION_STANDARDS.md for comprehensive documentation standards.

### Git Workflow

**Commit Policy:**
- **NEVER** commit without explicit user instruction
- **NEVER** push without explicit user instruction
- **EXCEPTION**: Plan Mode approval is considered explicit commit authorization
  - When user approves a plan via Exit Plan Mode, implementation and commits are both authorized
  - Upon successful implementation, all planned commits are created automatically without additional confirmation
  - If implementation fails or tests fail, NO commits are created (report to user instead)
- Split commits by specific intent (NOT feature-level goals)
- Each commit MUST be small enough to explain in one line
- Use `git apply <patchfile name>.patch` for partial file commits
- **NEVER** execute batch commits without user confirmation

**GitHub Integration:**
- **MUST** use GitHub CLI (`gh`) for all GitHub operations
- Use `gh pr create` for creating pull requests
- Use `gh pr view` for viewing PR details
- Use `gh issue create` for creating issues
- Use `gh issue view` for viewing issue details
- Use `gh api` for accessing GitHub API
- **NEVER** use raw `curl` or web browser for GitHub operations when `gh` is available

**GitHub Comments & Interactions:**
- **NEVER** post comments on PRs or Issues without authorization
- Authorization = explicit user instruction OR Plan Mode approval
- Self-initiated comments MUST be previewed and approved by user before posting
- ALL comments MUST be in English and include Claude Code attribution footer
- Comments MUST reference specific code locations with repository-relative paths
- Comments MUST NOT contain user requests, AI interactions, or absolute local paths

See instructions/GITHUB_INTERACTION.md for comprehensive GitHub interaction guidelines including:
- Posting authorization policy (PP-1 ~ PP-3)
- PR review response format (RR-1 ~ RR-3)
- Issue discussion guidelines (ID-1 ~ ID-2)
- Agent context provision (AC-1 ~ AC-2)

See instructions/COMMIT_GUIDELINE.md for detailed commit guidelines including:
- Commit execution policy (CE-1 ~ CE-5)
- Commit message format (CM-1 ~ CM-3)
- Commit message style guide
- CHANGELOG generation guidelines (CG-1 ~ CG-6)

### Release & Publishing Policy

**Automated Releases with release-plz:**

This project uses [release-plz](https://release-plz.ieni.dev/) for automated release management:

- **Automated Versioning**: Versions determined from conventional commits
- **Automated CHANGELOGs**: Generated from commit messages
- **Release PRs**: Automatically created when changes are pushed to main
- **Automated Publishing**: Crates published to crates.io upon Release PR merge

**Commit-to-Version Mapping:**

| Commit Type | Version Bump |
|-------------|--------------|
| `feat:` | MINOR |
| `fix:` | PATCH |
| `feat!:` or `BREAKING CHANGE:` | MAJOR |
| Other types | PATCH |

**Commit-to-CHANGELOG Mapping:**

| Commit Type | CHANGELOG Section |
|-------------|-------------------|
| `feat` | Added |
| `fix` | Fixed |
| `perf` | Performance |
| `refactor` | Changed |
| `docs` | Documentation |
| `revert` | Reverted |
| `deprecated` | Deprecated |
| `security` | Security |
| `chore`, `ci`, `build` | Maintenance |
| `test` | Testing |
| `style` | Styling |

**Tagging Strategy (Per-Crate Tagging):**
- Format: `[crate-name]@v[version]`
  - Examples: `reinhardt-core@v0.2.0`, `reinhardt-orm@v0.1.1`
- Tags are created automatically by release-plz upon Release PR merge
- **NEVER** create release tags manually

**Release Workflow:**
1. Write commits following Conventional Commits format
2. Push to main branch
3. release-plz creates Release PR with version bumps and CHANGELOG updates
4. Review and merge Release PR
5. release-plz publishes to crates.io and creates Git tags

**Manual Intervention:**
- Edit Release PR to adjust CHANGELOG entries or versions if needed
- Release PRs can be modified before merging

**Critical Rules:**
- **MUST** use conventional commit format for proper version detection
- **MUST** review Release PRs before merging
- **NEVER** manually bump versions in feature branches
- **NEVER** create release tags manually

**Key Warnings (Lessons Learned):**
- **NEVER** create circular publish dependency chains (functional crates must not dev-depend on other Reinhardt crates)
- **NEVER** add `version` field to `reinhardt-test` workspace dependency (causes publish failure; see cargo#15151)
- **MUST** follow RP-1 recovery procedure for partial release failures (see instructions/RELEASE_PROCESS.md)
- **NEVER** change `pr_branch_prefix` from `"release-plz-"` (breaks two-step release workflow)
- `publish_no_verify = true` is required because dev-dependencies reference unpublished workspace crates

See instructions/RELEASE_PROCESS.md for detailed release procedures.

### Workflow Best Practices

- Run dry-run for ALL batch operations before actual execution
- Use parallel agents for independent file edits
- NO batch commits (create one at a time with user confirmation)

### Issue Handling

**Batch Issue Strategy:**
- Group issues by fix pattern and process as a batch (HA-1)
- Divide work into phases ordered by severity (HA-2)
- Parallelize independent crate work using Agent Teams (HA-3)
- Organize phases into logically grouped branches and PRs (HA-4)

**Work Unit Principles:**
- 1 PR = 1 crate × 1 fix pattern as the basic work unit (WU-1)
- Same-crate related issues MAY be combined into a single PR (WU-2)
- Cross-crate shared changes MUST be preceding PRs, merged before per-crate fix PRs (WU-3)

See instructions/ISSUE_HANDLING.md for comprehensive issue handling principles including:
- Handling approach (HA-1 ~ HA-4)
- Work unit principles (WU-1 ~ WU-3)
- Workflow examples

---

## Common Commands

**Check & Build:**
```bash
cargo check --workspace --all --all-features
cargo build --workspace --all --all-features
```

**Testing:**
```bash
cargo test --workspace --all --all-features
cargo test --doc  # Documentation tests
```

**Code Quality:**
```bash
cargo make fmt-check   # Cheeck format rules of the code
cargo make clippy-check  # Check lint rules of the code
cargo make fmt-fix   # Automatically fix code based on formatting rules
cargo make clippy-fix  # Automatically fix code based on lint rules
```

**TODO Comment Check:**
```bash
# Clippy: detect todo!(), unimplemented!(), dbg!() macros
cargo make clippy-todo-check

# Semgrep: full scan for TODO/FIXME comments (all files, not diff-aware)
docker run --rm -v "$(pwd):/src" semgrep/semgrep semgrep scan --config .semgrep/ --error --metrics off

# Semgrep: diff-aware scan (compare against main branch)
docker run --rm -v "$(pwd):/src" semgrep/semgrep semgrep scan --config .semgrep/ --baseline-commit origin/main --error --metrics off
```

**Placeholder Check (formatter artifact detection):**
```bash
# Check for __reinhardt_placeholder__! left in source files after page! macro formatting
cargo make placeholder-check
```

**Database Tests:**
```bash
# Database tests use TestContainers automatically (no external database needed)
cargo nextest run --package reinhardt-integration-tests
```

**Container Runtime:**
```bash
# Verify Docker status
docker version
docker ps

# Docker daemon should be running automatically on most systems
```

**GitHub Operations (using GitHub CLI):**
```bash
# Pull Requests
gh pr create --title "feat: Add feature" --body "Description" --label enhancement
gh pr view [number]
gh pr list --state open
gh pr checks

# Issues
gh issue create --title "Bug report" --body "Description"
gh issue view [number]
gh issue list

# Releases
gh release list
gh release view [tag]
gh release create [tag] --title "Release v1.0.0" --notes "Release notes"

# Repository
gh repo view
gh api repos/{owner}/{repo}/pulls
```

**PR/Issue Template Compliance:**

- **PR Template:** `.github/PULL_REQUEST_TEMPLATE.md` (see @instructions/PR_GUIDELINE.md for details)
- **Issue Templates:** `.github/ISSUE_TEMPLATE/*.yml` (see @instructions/ISSUE_GUIDELINES.md for details)
- **Note:** GitHub CLI does not auto-apply templates; include template structure in `--body`

**Linking PRs to Issues:**

Use keywords to auto-close issues on merge: `Fixes #N`, `Closes #N`, `Resolves #N`
- Use `Refs #N` for related issues (no auto-close)
- See [GitHub Docs](https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/linking-a-pull-request-to-an-issue) for details

**CRITICAL: This project uses Docker for TestContainers integration, NOT Podman.**

- **MUST** ensure Docker Desktop is installed and running
- **MUST** ensure `DOCKER_HOST` environment variable points to Docker socket:
  - ✅ Correct: `unix:///var/run/docker.sock` or not set
  - ❌ Incorrect: `unix:///.../podman/...` (will cause container startup failures)
- If both Docker and Podman are installed:
  - Use `.testcontainers.properties` to force Docker usage (already configured in project)
  - Ensure `DOCKER_HOST` is not set to Podman socket
- **NEVER** use Podman for integration tests in this project

**Troubleshooting Container Errors:**

If you encounter "Cannot connect to Docker daemon" or "IncompleteMessage" errors:

```bash
# 1. Check Docker is running
docker ps

# 2. Check DOCKER_HOST environment variable
echo $DOCKER_HOST

# 3. If DOCKER_HOST points to Podman, unset it
unset DOCKER_HOST

# 4. Verify .testcontainers.properties exists in project root
cat .testcontainers.properties
```

---

## Database Operations

**Layer Selection:**
- **Basic CRUD**: Use `reinhardt-orm` for table-level operations
- **Low-Level**: Use `reinhardt-database` for schema management, raw queries, DB-specific operations

---

## Review Process

Before submitting code:

1. **Run all commands:**
   - `cargo check --workspace --all --all-features`
   - `cargo build --workspace --all --all-features`
   - `cargo test --workspace --all --all-features`
   - `cargo make fmt-check`
   - `cargo make clippy-check`

2. **Iterate until all issues resolved**

3. **Review compliance with standards:**
   - [ ] Module system (@instructions/MODULE_SYSTEM.md)
   - [ ] Testing standards (@instructions/TESTING_STANDARDS.md)
   - [ ] No anti-patterns (@instructions/ANTI_PATTERNS.md)
   - [ ] Documentation updated (@instructions/DOCUMENTATION_STANDARDS.md)
   - [ ] Git commit policy (@instructions/COMMIT_GUIDELINE.md)
   - [ ] GitHub interaction policy (@instructions/GITHUB_INTERACTION.md)
   - [ ] Issue handling principles (@instructions/ISSUE_HANDLING.md)
   - [ ] No unresolved TODO/FIXME comments in new code (TODO Check CI)

---

## Additional Instructions

@CLAUDE.local.md - Project-specific local preferences

**Note**: When editing files in `examples/` directory, also refer to examples/CLAUDE.md for examples-specific coding standards and dependency rules.

---

## Quick Reference

### ✅ MUST DO
- Write ALL code comments in English (no exceptions)
- Use `module.rs` + `module/` directory (NO `mod.rs`)
- Update docs with code changes (same workflow)
- Clean up ALL test artifacts
- Delete temp files from `/tmp` immediately
- Wait for explicit user instruction before commits
- Understand that Plan Mode approval authorizes both implementation and commits
- Mark placeholders with `todo!()` or `// TODO:`
- Use `#[serial(group_name)]` for global state tests
- Split commits by specific intent, not features
- Follow Conventional Commits v1.0.0 format: `<type>[scope]: <description>`
- Start commit description with lowercase letter (e.g., `feat: add feature`)
- Use `!` notation for breaking changes (e.g., `feat!:` or `feat(scope)!:`)
- Use conventional commit format for proper version detection by release-plz
- Write commit descriptions as standalone CHANGELOG entries (meaningful without additional context)
- Use `security` type for security vulnerability fixes (dedicated CHANGELOG section)
- Use `deprecated` type for marking features/APIs as deprecated (dedicated CHANGELOG section)
- Review Release PRs created by release-plz before merging
- Verify no circular dev-dependency chains exist before publishing (functional crates must not dev-depend on other Reinhardt crates)
- Keep `reinhardt-test` workspace dependency without `version` field (unpublished crate; cargo#15151)
- Follow RP-1 procedure in instructions/RELEASE_PROCESS.md for partial release failures
- Use GitHub CLI (`gh`) for all GitHub operations (PR, issues, releases)
- Search existing issues before creating new ones
- Use appropriate issue templates for all issues
- Apply at least one type label to every issue
- Report security vulnerabilities privately via GitHub Security Advisories
- Use `.github/labels.yml` as source of truth for label definitions
- Follow PR/Issue template structure when creating via `gh` CLI
- Use 1 PR = 1 crate x 1 fix pattern as the basic work unit for batch issue handling
- Create preceding PRs for cross-crate shared changes before per-crate fix PRs
- Organize batch work into phases by severity and parallelize across independent crates
- Use `rstest` for ALL test cases (no plain `#[test]`)
- Follow Arrange-Act-Assert (AAA) pattern with `// Arrange`, `// Act`, `// Assert` comments for test structure
- Use `reinhardt-test` fixtures for test setup/teardown
- Create specialized fixtures wrapping generic `reinhardt-test` fixtures for test data injection
- Use SeaQuery (not raw SQL) for SQL construction in tests
- Wrap generic types in backticks in doc comments: `` `Result<T>` ``, NOT `Result<T>`
- Wrap macro attributes in backticks: `` `#[inject]` ``, NOT `#[inject]`
- Wrap URLs in angle brackets or backticks: `<https://...>` or `` `https://...` ``
- Specify language for code blocks: ` ```rust `, NOT ` ``` `
- Wrap bracket patterns in backticks: `` `array[0]` ``, NOT `array[0]`
- Use backticks (not intra-doc links) for feature-gated types: `` `FeatureType` ``, NOT `` [`FeatureType`] ``
- Use Mermaid diagrams (via `aquamarine`) for architecture documentation instead of ASCII art
- Ensure `.stderr` files in trybuild tests contain only single error type (no warning/error mixing)
- Resolve all `todo!()` and `// TODO:` before merging PR (enforced by TODO Check CI)
- Preview and get user confirmation before posting self-initiated GitHub comments
- Include Claude Code attribution footer on all GitHub comments
- Use repository-relative paths (not absolute) in GitHub comments
- Provide structured agent context using AC-2 template format

### ❌ NEVER DO
- Use `mod.rs` files (deprecated pattern)
- Commit without user instruction (except Plan Mode approval)
- Leave docs outdated after code changes
- Document user requests or AI interactions in project documentation
- Save files to project directory (use `/tmp`)
- Leave backup files (`.bak`, `.backup`, `.old`, `~`)
- Create skeleton tests (tests without assertions)
- Use loose assertions (`contains`) without justification
- Use glob imports (`use module::*`)
- Create circular dependencies
- Leave unmarked placeholder implementations
- Use `#[allow(...)]` without explanatory comments
- Use alternative TODO notations (`FIXME:`, `NOTE:` for unimplemented features)
- Create batch commits without user confirmation
- Use relative paths beyond `../`
- Manually bump versions in feature branches (let release-plz handle it)
- Create release tags manually (release-plz creates them automatically)
- Skip reviewing Release PRs before merging
- Add `reinhardt-test` to functional crate `[dev-dependencies]` (creates circular publish dependency)
- Add `version` field to `reinhardt-test` workspace dependency (breaks cargo publish; cargo#15151)
- Change `pr_branch_prefix` from `"release-plz-"` (breaks two-step release workflow)
- Merge Release PR without rolling back unpublished crate versions after partial release failure
- Write vague commit descriptions that are unclear as CHANGELOG entries (e.g., "fix issue", "update code")
- Start commit description with uppercase letter
- End commit description with a period
- Omit `!` or `BREAKING CHANGE:` for API-breaking changes
- Create issues without appropriate labels
- Create public issues for security vulnerabilities
- Create duplicate issues without searching first
- Skip issue templates when creating issues
- Use non-English in issue titles or descriptions
- Apply `release` label to issues (only for PRs)
- Mix changes to unrelated crates in a single issue-fix PR
- Mix unrelated fix patterns in a single PR
- Skip preceding PRs for cross-crate shared utilities
- Use plain `#[test]` instead of `#[rstest]`
- Use non-standard phase labels in tests (`// Setup`, `// Execute`, `// Verify` -- use `// Arrange`, `// Act`, `// Assert`)
- Write raw SQL strings in tests (use SeaQuery instead)
- Duplicate infrastructure setup code (use `reinhardt-test` fixtures)
- Write generic types without backticks in doc comments (causes HTML tag warnings)
- Write macro attributes without backticks in doc comments (causes unresolved link warnings)
- Write bare URLs in doc comments (causes bare URL warnings)
- Use intra-doc links for feature-gated items (causes unresolved link warnings)
- Create new ASCII art diagrams in doc comments (use Mermaid instead)
- Mix warnings and errors in trybuild `.stderr` files
- Merge PR with unresolved `todo!()` or `// TODO:` comments (blocked by TODO Check CI)
- Post GitHub comments without authorization (explicit instruction or Plan Mode approval)
- Include absolute local paths in GitHub comments (`/Users/...`, `/home/...`)
- Post vague or non-actionable GitHub comments
- Skip Claude Code attribution footer on GitHub comments
- Create PRs/Issues without following template structure

### 📚 Detailed Standards

For comprehensive guidelines, see:
- **Module System**: instructions/MODULE_SYSTEM.md
- **Testing**: instructions/TESTING_STANDARDS.md
- **Anti-Patterns**: instructions/ANTI_PATTERNS.md
- **Documentation**: instructions/DOCUMENTATION_STANDARDS.md
- **Git Commits**: instructions/COMMIT_GUIDELINE.md (includes CHANGELOG generation guidelines)
- **Release Process**: instructions/RELEASE_PROCESS.md
- **Issues**: instructions/ISSUE_GUIDELINES.md
- **Issue Handling**: instructions/ISSUE_HANDLING.md
- **GitHub Interactions**: instructions/GITHUB_INTERACTION.md
- **Security Policy**: SECURITY.md
- **Code of Conduct**: CODE_OF_CONDUCT.md
- **Label Definitions**: .github/labels.yml
- **Project Overview**: README.md

---

**Note**: This CLAUDE.md focuses on core rules and quick reference. All detailed standards, examples, and comprehensive guides are in the `instructions/` directory. Always review CLAUDE.md before starting work, and consult detailed documentation as needed.
