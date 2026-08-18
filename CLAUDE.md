# CLAUDE.md

## Purpose

This file contains project-specific instructions for the Reinhardt project. These rules ensure code quality, maintainability, and consistent practices across the Rust codebase.

For detailed standards, see documentation in `instructions/` directory.

---

## Project Overview

See README.md for project details.

**Repository URL**: https://github.com/kent8192/reinhardt-web

@instructions/DESIGN_PHILOSOPHY.md

---

## Tech Stack

- **Language**: Rust 2024 Edition
- **Module System**: MUST use 2024 edition (NO `mod.rs`)
- **Database**: `reinhardt-query` for building SQL queries (in-house wrapper, replaces direct SeaQuery usage)
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
- **MUST manage all resources via RAII** (Drop-based guard types); NO manual `close()`/`release()`/`cleanup()` that can be skipped on an early `return`, `?`, or panic; NO `mem::forget` / `ManuallyDrop` / immediate-drop `let _ = lock()` without a justifying comment (see @instructions/ANTI_PATTERNS.md)

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

**Workaround Comments:**
- When introducing workaround code, MUST include the ideal implementation (the correct code without the workaround) as a comment
- This is in addition to the UR-4 format (issue reference + removal condition) in instructions/UPSTREAM_ISSUE_REPORTING.md
- The ideal implementation comment enables future developers to remove the workaround without re-investigating the intended design

**Comment template:**
```rust
// Workaround for upstream-repo#42 (tracked in reinhardt-web#15)
// Remove this workaround when the upstream issue is resolved.
//
// Ideal implementation (without workaround):
//   pool.health_check().await?;
```

See instructions/ANTI_PATTERNS.md for comprehensive anti-patterns guide.

### Testing

**Core Principles:**
- NO skeleton tests (all tests MUST have meaningful assertions)
- EVERY test MUST use at least one Reinhardt component
- Unit tests: Test single component behavior, place in functional crate
- Integration tests: Test integration points between components
  - Cross-crate integration: Place in `tests/` crate
  - Within-crate integration: Can place in functional crate
- Functional crates MUST NOT use `{ workspace = true }` for `reinhardt-test` in `dev-dependencies` (use optional dependency or path-only dev-dependency; see KI-2)
- ALL test artifacts MUST be cleaned up
- Global state tests MUST use `#[serial(group_name)]`
- Use strict assertions (`assert_eq!`) instead of loose matching (`contains`)
- Follow Arrange-Act-Assert (AAA) pattern for test structure

See instructions/TESTING_STANDARDS.md for comprehensive testing standards including:
- Testing philosophy (TP-1, TP-2)
- Test organization (TO-1)
- Test implementation (TI-1 ~ TI-8)
- Infrastructure testing (IT-1 ~ IT-2)

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
- Update all relevant: README.md, crate README, instructions/, lib.rs
- Planned features go in `lib.rs` header, NOT in README.md
- Test all code examples
- Verify all links are valid
- **NEVER** document user requests or AI assistant interactions in project documentation
  - Documentation must describe technical reasons, design decisions, and implementation details
  - Avoid phrases like "User requested...", "As requested by...", "User asked..."
  - Focus on the "why" (technical rationale) not the "who asked"

See instructions/DOCUMENTATION_STANDARDS.md for comprehensive documentation standards.

**CLAUDE.md ↔ AGENTS.md Sync Policy:**
- `CLAUDE.md` (Claude Code) and `AGENTS.md` (Codex) are deliberate mirror copies kept in sync
- The two files MUST differ only on a small set of mechanical substitutions:
  - `CLAUDE.md` ↔ `AGENTS.md` (title, references)
  - `CLAUDE.local.md` ↔ `AGENTS.local.md`
  - `Claude Code attribution` ↔ `Codex attribution`
- **MUST**: Any edit to one file MUST be mirrored into the other in the same commit
- **MUST**: After editing, run `diff CLAUDE.md AGENTS.md` and confirm only the documented substitutions remain
- **NEVER**: Commit a change that touches only one of the two files

### Git Workflow

**Commit Policy:**
- **NEVER** commit without explicit user instruction
- **NEVER** push without explicit user instruction
- **EXCEPTION**: Plan Mode approval is considered explicit commit authorization
  - When user approves a plan via Exit Plan Mode, implementation and commits are both authorized
  - Upon successful implementation, all planned commits are created automatically without additional confirmation
  - If implementation fails or tests fail, NO commits are created (report to user instead)
- **EXCEPTION (Reinhardt family)**: When operating inside `reinhardt-web` / `reinhardt-cloud` / `awesome-delions` / `reinhardt-cc`, the **Autonomous Operation Policy** below authorizes commit and push on any non-protected branch (plus Draft PR / Issue creation) without further confirmation — see the next subsection
- Split commits by specific intent (NOT feature-level goals)
- Each commit MUST be small enough to explain in one line
- Use `git apply <patchfile name>.patch` for partial file commits
- **NEVER** execute batch commits without user confirmation

**Autonomous Operation Policy (Reinhardt Family):**

This is an explicit, named exception to "NEVER commit/push without explicit user instruction" in the Commit Policy above. Comment-posting authorization (`instructions/GITHUB_INTERACTION.md` PP-1) is unchanged — see "Still Requires Explicit User Authorization" below.

Scope (applies only when the working directory is inside one of these four repositories):

- `kent8192/reinhardt-web`
- `kent8192/reinhardt-cloud`
- `kent8192/awesome-delions`
- `kent8192/reinhardt-cc`

Autonomously Allowed (no per-action confirmation required):

| Operation | Constraint |
|-----------|------------|
| `git commit` | On any non-protected branch |
| `git push` | On any non-protected branch (`feature/...`, `fix/...`, `refactor/...`, `docs/...`, `chore/...`, `test/...`, `perf/...`, `debug/...`, etc.); **never** on `main`, `master`, `develop/*`, or `release/*` |
| Create a **Draft** Pull Request | `gh pr create --draft` / MCP `create_pull_request` with `draft=true`; body MUST follow `.github/PULL_REQUEST_TEMPLATE.md` |
| Convert Draft PR to **Ready for Review** | **CI completion is not required** — this overrides any "CI green / tests pass" criterion in `instructions/`. All other PC-4a readiness criteria (implementation complete, PR description follows template, fmt/clippy clean, docs updated) still apply — see `instructions/PR_GUIDELINE.md` § PC-4a |
| Create an Issue | `gh issue create` / MCP `issue_write`; MUST follow the appropriate issue template and apply at least one type label |

**Protected Branches** (commit/push always require explicit user authorization):
- `main`, `master`
- `develop/*` (any branch starting with `develop/`)
- `release/*` (any branch starting with `release/`)

Still Requires Explicit User Authorization (no autonomy):

- Direct push to any protected branch listed above
- `git push --force`, `--force-with-lease`, or any other history-rewriting push
- `git rebase`, `git reset --hard`, `git branch -D`, deleting tags, or any other history-destructive operation
- Closing, merging, or deleting PRs
- Closing or deleting Issues, comments, or review threads
- Creating release tags or any PR carrying the `release` label
- Posting comments / replies / reviews on PRs/Issues — the comment-posting authorization model in `instructions/GITHUB_INTERACTION.md` PP-1 is unchanged; the autonomous policy covers only the **creation** of commits, pushes, Draft PRs, and Issues, not commenting

Unchanged Quality Guardrails (apply equally to autonomous operations):

- PR title and body MUST follow Conventional Commits and `.github/PULL_REQUEST_TEMPLATE.md`
- Breaking-change PR titles MUST use the `type!(scope): description` format, such as `fix!(auth): ...`, and MUST include the `breaking-change` label. The scope-less `type!: description` form is also valid. Existing `type(scope)!: description` titles remain accepted for compatibility.
- Issue body MUST follow `.github/ISSUE_TEMPLATE/*.yml`
- Branch naming, commit message format, Claude Code attribution footer, English-only policy, and all other rules in this document remain in force

**Draft PR Policy:**
- The agent MAY convert a Draft PR to Ready for Review autonomously once the PC-4a readiness criteria are met. CI completion is **not** required (the Autonomous Operation Policy overrides the previous "CI green / tests pass" prerequisite); fmt/clippy cleanliness and the other PC-4a criteria are still required
- Explicit user instruction also authorizes conversion at any time (overrides readiness check)
- The agent MUST NOT convert when any PC-4a readiness criterion (other than CI completion) is unmet, unless the user explicitly overrides
- Use `gh pr ready <number>` (or GitHub MCP equivalent) for conversion
- See instructions/PR_GUIDELINE.md § PC-4a for full details

**Branch Operations:**
- When merging branches and resolving conflicts, execute immediately without entering Plan Mode
- Before creating branches, verify names don't conflict with existing ones using `git worktree list` and `git branch -a`
- Issue-linked branches: `<type>/issue-XXXX-to-YYYY-<desc>` (range), `<type>/issue-XXXX-to-YYYY-and-WWWW-to-ZZZZ-<desc>` (multiple ranges)

**PR Conflict Resolution:**
- **MUST** use worktree-based merge strategy for resolving PR conflicts (NOT rebase or force-push)
- Procedure:
  1. Create a local worktree for the PR source branch
  2. Merge the target branch (e.g., `main`) into the source branch within the worktree
  3. Resolve conflicts in the worktree
  4. Commit the merge resolution
  5. Push the source branch to remote
  6. Clean up the worktree
- **NEVER** use `git rebase` or `git push --force` to resolve PR conflicts
- This preserves commit history and avoids force-push risks

**GitHub Integration:**
- **DEFAULT**: Use GitHub MCP tools for all GitHub operations (PR, issues, discussions, releases)
- **FALLBACK**: Use GitHub CLI (`gh`) **only** when GitHub MCP is unavailable, errors out (e.g., 404), or lacks the required capability
- When GitHub MCP returns an error, immediately fall back to `gh` CLI without retrying the MCP call
- **NEVER** use raw `curl` or web browser for GitHub operations
- For usage questions, prefer GitHub Discussions over Issues
- Common operations and their MCP/CLI equivalents:
  - PR create: MCP `create_pull_request` / CLI `gh pr create`
  - PR view: MCP `pull_request_read` / CLI `gh pr view`
  - PR update: MCP `update_pull_request` / CLI `gh pr edit`
  - Issue create: MCP `issue_write` / CLI `gh issue create`
  - Issue view: MCP `issue_read` / CLI `gh issue view`
  - Raw API: CLI `gh api` (use only when no MCP equivalent)

**GitHub Comments & Interactions:**
- **NEVER** post comments on PRs or Issues without authorization
- Authorization = explicit user instruction OR Plan Mode approval
- Self-initiated comments MUST be previewed and approved by user before posting
- ALL comments MUST be in English and include Claude Code attribution footer
- Comments MUST reference specific code locations with repository-relative paths
- Comments MUST NOT contain user requests, AI interactions, or absolute local paths
- **Reinhardt family scope note**: The Autonomous Operation Policy authorizes *creation* of Draft PRs and Issues without further confirmation in the four Reinhardt-family repos, but *commenting* on PRs/Issues remains fully subject to the rules above

See instructions/GITHUB_INTERACTION.md for comprehensive GitHub interaction guidelines including:
- Posting authorization policy (PP-1 ~ PP-3)
- PR review response format (RR-1 ~ RR-3)
- Copilot review handling (CR-1 ~ CR-5)
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
  - Examples: `reinhardt-core@v0.2.0`, `reinhardt-db@v0.1.1`
- Tags are created automatically by release-plz upon Release PR merge
- **NEVER** create release tags manually

**Release Workflow:**
1. Write commits following Conventional Commits format
2. Push to main branch
3. release-plz creates Release PR with version bumps and CHANGELOG updates
4. Review and merge Release PR
5. release-plz publishes to crates.io and creates Git tags

**Release PR Branch Policy:**
- **NEVER** push code fixes directly to a release-plz branch (`release-plz-*` or `develop-release-plz-*`) — direct pushes bypass review and may be overwritten when release-plz regenerates the PR
- If a code fix is needed before merging a Release PR, create a `fix/` or `hotfix/` branch from the Release PR's **base branch** (e.g., `main` or `develop/*`), open a PR targeting that base branch, and merge it — release-plz will regenerate the Release PR automatically
- CHANGELOG or version edits on the Release PR branch are acceptable via GitHub UI when done immediately before merging (these are release metadata adjustments, not code changes)

**Critical Rules:**
- **MUST** use conventional commit format for proper version detection
- **MUST** review Release PRs before merging
- **NEVER** manually bump versions in feature branches
- **NEVER** create release tags manually

**Key Warnings (Lessons Learned):**
- **NEVER** create circular publish dependency chains (functional crates must not dev-depend on other Reinhardt crates)
- **MUST** declare `reinhardt-test` as an optional dependency (not dev-dependency) in functional crates for correct release-plz publish ordering (see KI-2 in instructions/RELEASE_PROCESS.md)
- **MUST** include `version` field in `reinhardt-test` workspace dependency (same as other published crates)
- **MUST** follow RP-1 recovery procedure for partial release failures (see instructions/RELEASE_PROCESS.md)
- **NEVER** change `pr_branch_prefix` from `"release-plz-"` (breaks two-step release workflow)
- `publish_no_verify = true` is required because dev-dependencies reference unpublished workspace crates

See instructions/RELEASE_PROCESS.md for detailed release procedures.

### Obsidian Wiki Maintenance

**Vault:** Discovered via Obsidian MCP server (`obsidian-vault`). Call `obsidian_list_files_in_vault` to verify connectivity and resolve the vault root.

Update the Obsidian wiki **frequently and proactively** — at the end of every meaningful work unit (architectural decision, new pattern, troubleshooting solution, lesson learned, module/component studied, dependency assessed, flow traced, concept clarified, options compared, question answered) and whenever you learn something worth preserving. When in doubt, write a short page (OW-1):

1. Check Obsidian MCP availability — if unavailable, **skip entirely** (do NOT block primary work)
2. Use `/wiki-query` to retrieve relevant existing knowledge before answering or deciding (OW-5)
3. Read `wiki/hot.md` to check for duplicates
4. Create/update pages under the **most specific** category, drawing on the vault's full category set (`modules/`, `components/`, `knowledge/{patterns,troubleshooting,learnings}/`, `decisions/`, `dependencies/`, `flows/`, `concepts/`, `entities/`, `domains/`, `comparisons/`, `questions/`) — never default everything to `knowledge/troubleshooting/` (OW-2, OW-7)
5. Update meta pages: `wiki/index.md`, `wiki/hot.md`, `wiki/log.md`

**Distribute (OW-7):** Spread knowledge across categories — a single work unit often yields several cross-linked pages. Proactively populate under-used categories instead of concentrating in one.

**Dual-write rule (OW-6):** When saving to any memory system (claude-mem, auto-memory, Serena), simultaneously invoke `/wiki-ingest` to persist the same knowledge in the Obsidian wiki.

**Skip when:** MCP unavailable, trivial changes, work in progress, or emergency/hotfix work.

See instructions/OBSIDIAN_WIKI.md for detailed standards (OW-1 ~ OW-7).

### Workflow Best Practices

- Run dry-run for ALL batch operations before actual execution
- Use parallel agents for independent file edits
- NO batch commits (create one at a time with user confirmation)
- Execute straightforward operations (branch deletion, worktree cleanup) immediately without planning

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

**Upstream Issue Reporting:**
- When an upstream dependency issue is discovered during reinhardt-web development, **immediately** create an issue in the upstream repository (UR-1)
- Use `gh issue create -R [owner]/[repo]` for upstream issue creation (UR-2)
- Create a tracking issue in reinhardt-web with `upstream-tracking` label for every upstream issue (UR-4)
- **NEVER** implement workarounds without creating an upstream issue first (WP-2)

See instructions/UPSTREAM_ISSUE_REPORTING.md for upstream dependency issue reporting policy including:
- Reporting policy (UR-1 ~ UR-5)
- Issue categories (IC-1, IC-2)
- Workaround policy (WP-1 ~ WP-3)

---

## Common Commands

**Check & Build:**
```bash
cargo check --workspace --all --all-features
cargo build --workspace --all --all-features
cargo make feature-check  # Check representative feature combinations (27 patterns)
```

**Testing:**
```bash
cargo nextest run --workspace --all-features
cargo test --doc  # Documentation tests
```

**Code Quality:**
```bash
cargo make fmt-check   # Check format rules of the code
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

**Security Audit:**
```bash
cargo make audit  # Check for known vulnerabilities in dependencies
```

**SemVer Check (Local):**
```bash
# Run the same cargo-semver-checks command as the shared semver-check workflow.
# Capture output ONCE — semver-check is slow (typically 1.5–2 h) and running it twice
# can also yield inconsistent results if `main` advances between invocations.
OUT=$(cargo make semver-check 2>&1) || true

# Post the result to the PR (required by PR_GUIDELINE.md RP-1a).
# Use the <!-- local-semver-check --> marker. On re-runs, UPDATE the existing marked
# comment via `gh api ... -X PATCH` instead of creating a new one.
PR=<PR number>
OWNER=<owner>
REPO=<repo>
BODY=$(printf '<!-- local-semver-check -->\n## Local SemVer Check Result\n\n````text\n%s\n````\n\n*Generated locally via `cargo make semver-check` (mirrors the shared semver-check workflow).*' "$OUT")

# Look up an existing marked comment, then PATCH it; otherwise create a new one.
EXISTING=$(gh api "repos/$OWNER/$REPO/issues/$PR/comments" --paginate \
  --jq '.[] | select(.body | startswith("<!-- local-semver-check -->")) | .id' \
  | head -n1)
if [ -n "$EXISTING" ]; then
  gh api -X PATCH "repos/$OWNER/$REPO/issues/comments/$EXISTING" -f body="$BODY" >/dev/null
else
  gh api -X POST "repos/$OWNER/$REPO/issues/$PR/comments" -f body="$BODY" >/dev/null
fi
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

**Orphan Detector (CI Infrastructure, Issue #3903):**
```bash
# Unit tests (requires Node.js 20+)
cd infra/github-runners/lambda-src/orphan-detector
npm ci
npm test                     # 55 unit tests (vitest)
npm run test:coverage        # with coverage thresholds (90% line, 85% branch)

# Build Lambda bundle locally
npm run build                # bundles src/index.ts -> dist/index.mjs

# Dry-run invoke against CI sub-account (after deploy)
aws lambda invoke \
  --function-name reinhardt-ci-orphan-detector \
  --payload '{"dryRun":true}' \
  /tmp/resp.json

# Tail CloudWatch logs
aws logs tail /aws/lambda/reinhardt-ci-orphan-detector --follow
```

See `infra/github-runners/README.md` for architecture, configuration
reference, and runbook.

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
- **Basic CRUD**: Use `reinhardt-db` for table-level operations
- **Low-Level**: Use `reinhardt-db` for schema management, raw queries, DB-specific operations

---

## Review Process

**CI Failure Diagnosis (Known Patterns):**
- Check these recurring patterns first:
  1. rustdoc intra-doc link errors with `-D warnings` (broken links, redundant explicit targets, private item links — see RD-8/RD-9/RD-10 in instructions/DOCUMENTATION_STANDARDS.md)
  2. docs.rs build issues from empty code blocks
  3. SemVer compatibility with `cargo-semver-checks`
  4. Windows CI-specific failures
- Always run `cargo doc --no-deps` locally before pushing doc-related fixes

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
   - [ ] Upstream issues reported (@instructions/UPSTREAM_ISSUE_REPORTING.md)
   - [ ] No unresolved TODO/FIXME comments in new code (TODO Check CI)

---

## Additional Instructions

@CLAUDE.local.md - Project-specific local preferences

**Note**: When editing files in `examples/` directory, also refer to examples/CLAUDE.md for examples-specific coding standards and dependency rules.

---

@instructions/QUICK_REFERENCE.md

@instructions/PR_BASE_BRANCH_POLICY.md

---

**Note**: This CLAUDE.md focuses on core rules and quick reference. All detailed standards, examples, and comprehensive guides are in the `instructions/` directory. Always review CLAUDE.md before starting work, and consult detailed documentation as needed.
