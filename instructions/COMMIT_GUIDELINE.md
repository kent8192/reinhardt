# Git Commit Guidelines

## Purpose

This file defines the git commit policy for the Reinhardt project. These rules ensure clear commit history, proper granularity, and consistent commit message formatting across the development lifecycle.

---

## Specification Reference

This document follows [Conventional Commits v1.0.0](https://www.conventionalcommits.org/en/v1.0.0/).

Key principles from the specification:
- Commit messages MUST be structured as: `<type>[optional scope]: <description>`
- The types `feat` and `fix` correlate with SemVer MINOR and PATCH respectively
- Breaking changes MUST be indicated with `!` after type/scope or via `BREAKING CHANGE:` footer

---

## Commit Execution Policy

### CE-1 (MUST): Explicit User Authorization

- **NEVER** create commits without explicit user instruction
- **NEVER** push commits without explicit user instruction
- Always wait for user confirmation before committing changes
- Prepare changes and inform the user, but let them decide when to commit

**EXCEPTION: Plan Mode Approval**

When a user approves a plan by accepting Exit Plan Mode, this constitutes explicit authorization for both:
1. Implementation of the planned changes
2. Creation of all commits associated with the implementation

**Automatic Commit Workflow after Plan Mode Approval:**

1. **Success Case**: If implementation completes successfully and all tests pass:
   - Automatically create all commits as planned in the approved plan
   - NO additional user confirmation required for each commit
   - Follow commit granularity rules (CE-2) and commit message format (CM-1, CM-2, CM-3)
   - Commits are created sequentially in the logical order defined in the plan

2. **Failure Case**: If implementation fails or tests fail:
   - **DO NOT** create any commits
   - Report the failure to the user with detailed information
   - Wait for user instruction on how to proceed

**Important Notes:**

- Plan Mode approval does NOT authorize pushing commits to remote
- Pushing still requires explicit user instruction
- The approved plan should clearly outline the planned commits (number, scope, messages)
- If the implementation deviates significantly from the plan, seek user confirmation before committing
- Batch commits are still prohibited - commits are created one at a time, but automatically without confirmation

### CE-2 (MUST): Commit Granularity

- Commits **MUST** be split into developer-friendly, understandable units
- **Each commit should represent a specific intent to achieve a goal, NOT the goal itself**
  - ❌ Bad: Committing an entire "authentication feature" in one commit (goal-level)
  - ✅ Good: Separate commits for each building block:
    - "implement password hashing with bcrypt" (specific intent)
    - "add JWT token generation logic" (specific intent)
    - "create session middleware" (specific intent)
- **Each commit MUST be small enough to be explained in a single line**
  - If you can't describe the commit clearly in one line, it's too large and should be split
  - The description should explain the specific intent, not just the feature name
  - **Note**: This refers to the _scope of changes_, not the commit message length
    - The commit message itself can and should be detailed with body and footer
    - What matters is that the commit's _intent_ can be summarized in one line
- **Avoid monolithic commits at feature-level**
  - ❌ Bad Examples:
    - "feat(auth): Implement authentication feature" (too broad, feature-level, uppercase start)
    - "feat(auth): Add JWT, session, and OAuth support" (multiple intents, uppercase start)
    - "feat(api): Add user CRUD endpoints" (multiple endpoints = multiple intents)
  - ✅ Good Examples:
    - "feat(auth): implement bcrypt password hashing for user authentication"
    - "feat(auth): add JWT token generation with RS256 algorithm"
    - "feat(auth): create session storage middleware"
    - "feat(api): add user creation endpoint with validation"
    - "feat(api): add user retrieval endpoint"
- Group files by specific intent at a fine-grained level
- Ensure commits tell a clear story when reviewing history, with each step being a concrete implementation detail

### CE-3 (MUST): Partial File Commits

When a single file contains changes with different purposes, use one of the following methods for line-level commits:

#### Method 1: Editor-based Patch Editing (Recommended)

```bash
git add --patch <file>
```

- Opens patch directly in editor without interactive prompts
- Edit patch file: delete `+` lines to unstage additions, change `-` to ` ` (space) to unstage deletions
- Provides fine-grained control with single editor operation

#### Method 2: Patch File Approach (For Automation)

```bash
# Generate patch
git diff <file> > /tmp/changes.patch

# Edit patch file to keep only desired changes

# Apply to staging area
git apply --cached /tmp/changes.patch
```

- Fully non-interactive and scriptable
- Allows review and modification before staging
- Ideal for automated workflows

**General Guidelines:**

- Stage only related changes together
- Keep each commit focused on one objective

### CE-4 (MUST): Respect .gitignore

- **NEVER** commit files specified in .gitignore
- Verify staged files before committing
- Use `git status` to confirm no ignored files are included

### CE-5: Automated Releases with release-plz

**Overview:**

This project uses [release-plz](https://release-plz.ieni.dev/) for automated release management. Version bumps, CHANGELOG updates, and publishing are handled automatically based on conventional commits.

**How It Works:**

1. Write commits following [Conventional Commits](https://www.conventionalcommits.org/) format
2. Push to main branch
3. release-plz automatically creates a Release PR with:
   - Version bumps in `Cargo.toml` files
   - Updated CHANGELOG.md files
   - Summary of changes
4. Review and merge the Release PR
5. release-plz publishes to crates.io and creates Git tags

**Commit-to-Version Mapping:**

| Commit Type | Version Bump | Example |
|-------------|--------------|---------|
| `feat:` | MINOR | `feat(auth): add OAuth support` |
| `fix:` | PATCH | `fix(orm): resolve connection leak` |
| `feat!:` or `BREAKING CHANGE:` | MAJOR | `feat!: change API response format` |
| Other types | PATCH | `docs:`, `chore:`, `refactor:`, etc. |

**Manual Intervention:**

- Edit the Release PR to adjust CHANGELOG entries or version numbers if needed
- Release PRs can be modified before merging

**Critical Rules:**
- ✅ Use conventional commit format for proper version detection
- ✅ Review Release PRs before merging
- ❌ NEVER manually bump versions in feature branches (let release-plz handle it)
- ❌ NEVER create release tags manually (release-plz creates them)

**For Detailed Information:**

See [docs/RELEASE_PROCESS.md](RELEASE_PROCESS.md) for complete release workflow documentation

---

## Commit Message Structure

### Format

Commit messages consist of three parts:

1. **Subject line**
2. **Body**
3. **Footer**

### CM-1 (MUST): Subject Line Format

```
<type>[optional scope][optional !]: <description>

Examples:
feat(auth): add password validation with bcrypt
fix(orm): resolve race condition in connection pool
feat(api)!: change response format to JSON:API specification
```

**Requirements:**

- **Type**: One of the defined commit types (see Commit Types section below)
- **Scope**: Module or component name (e.g., `auth`, `orm`, `http`)
  - Multiple scopes separated by commas: `(shortcuts,dispatch)`
  - Scope is OPTIONAL but RECOMMENDED for clarity
- **Breaking Change Indicator**: Append `!` after type/scope to indicate breaking changes
  - Example: `feat!:` or `feat(api)!:`
  - This is the PREFERRED method for indicating breaking changes
- **Description**: Concise summary in English
  - **MUST** start with lowercase letter
  - **MUST** be specific, not vague
  - **MUST NOT** end with a period
  - ❌ Bad: "Improve authentication overall" (Too vague, starts with uppercase)
  - ❌ Bad: "add feature." (Ends with period)
  - ✅ Good: "add RS256 algorithm support to JWT token validation"

### Commit Types

**Required Types (correlate with SemVer):**

| Type | Description | SemVer | CHANGELOG Section |
|------|-------------|--------|-------------------|
| `feat` | A new feature | MINOR | Added |
| `fix` | A bug fix | PATCH | Fixed |

**Recommended Types:**

| Type | Description | CHANGELOG Section |
|------|-------------|-------------------|
| `perf` | Performance improvements | Performance |
| `refactor` | Code change that neither fixes a bug nor adds a feature | Changed |
| `docs` | Documentation only changes | Documentation |
| `revert` | Reverts a previous commit | Reverted |
| `deprecated` | Marks features/APIs as deprecated | Deprecated |
| `security` | Security vulnerability fixes | Security |
| `chore` | Maintenance tasks (no production code change) | Maintenance |
| `ci` | CI configuration changes | Maintenance |
| `build` | Changes affecting build system or external dependencies | Maintenance |
| `test` | Adding or modifying tests | Testing |
| `style` | Code style changes (formatting, whitespace) | Styling |

**Choosing Between Similar Types:**

- **`security` vs `fix`**: Use `security` for fixes addressing security vulnerabilities (CVEs, injection flaws, auth bypasses). This ensures they appear in a dedicated "Security" CHANGELOG section for visibility. Use `fix` for general bug fixes without security implications.
- **`docs` vs `chore(docs)`**: Use `docs` for user-facing documentation changes (README, API docs, guides) — mapped to "Documentation" section. Use `chore(docs)` for internal documentation (CI comments, code comments) — mapped to "Maintenance" section.
- **`refactor` vs `feat`/`fix`**: Use `refactor` when behavior does not change (mapped to "Changed"). If the refactoring introduces a new API or changes existing behavior, use `feat!:` or `fix!:` instead.
- **`deprecated` vs `feat`**: Use `deprecated` when the primary purpose is marking features/APIs as deprecated (mapped to "Deprecated"). If deprecation is part of a larger feature replacement, use `feat` with deprecation noted in the body.

### BREAKING CHANGE

Breaking changes introduce API incompatibility and correlate with SemVer MAJOR version bump.

**Indicating Breaking Changes:**

1. **Preferred: Using `!` notation** (concise and visible)
   ```
   feat!: remove deprecated authentication endpoints
   feat(api)!: change response format to JSON:API specification
   ```

2. **Alternative: Using footer** (allows detailed explanation)
   ```
   feat(auth): migrate to OAuth 2.0

   BREAKING CHANGE: legacy session-based authentication is no longer supported.
   Users must migrate to OAuth 2.0 tokens before upgrading.
   ```

3. **Combined: Both `!` and footer** (for maximum clarity)
   ```
   refactor(db)!: change connection pool implementation

   BREAKING CHANGE: `ConnectionPool::new()` now requires a `Config` parameter.
   Previous: `ConnectionPool::new(url)`
   New: `ConnectionPool::new(url, Config::default())`
   ```

**Requirements:**

- Breaking changes MUST be indicated using `!` or `BREAKING CHANGE:` footer
- When using `!`, additional `BREAKING CHANGE:` footer is OPTIONAL
- `BREAKING CHANGE` MUST be uppercase in footer
- `BREAKING-CHANGE` is synonymous with `BREAKING CHANGE`

### Revert Commits

When reverting a previous commit, use the `revert` type with references to the original commit(s).

**Format:**

```
revert: <original commit subject>

Refs: <commit SHA(s)>
```

**Example:**

```
revert: add experimental caching layer

This reverts the experimental caching implementation that caused
memory leaks under high load conditions.

Refs: a1b2c3d, e4f5g6h
```

**Requirements:**

- Subject SHOULD match the original commit's subject
- Body SHOULD explain why the revert is necessary
- Footer MUST include `Refs:` with the original commit SHA(s)

### CM-2 (MUST): Body Format

```
Brief summary paragraph explaining the changes.

Module/Component Section 1:
- file/path.rs: +XXX lines - Description
  - Sub-detail 1
  - Sub-detail 2
- file/path2.rs: Description

Module/Component Section 2:
- file/path.rs: Changes
- Removed: old_file.rs (reason)

Features:
- Feature 1
- Feature 2
- Feature 3

Additional context or explanation.
```

**Requirements:**

- Write in English
- Organize changes by module or component
- List modified files with line count changes where significant
- Include "Removed:" entries for deleted files with reasons
- Summarize new features in a dedicated "Features:" section
- Provide context for complex changes

### CM-3 (MUST): Footer Format

Footers follow the [git trailer convention](https://git-scm.com/docs/git-interpret-trailers).

**Footer Syntax:**

```
<token>: <value>
```

or

```
<token> #<value>
```

**Standard Footers:**

| Token | Description | Example |
|-------|-------------|---------|
| `BREAKING CHANGE` | Indicates breaking API change | `BREAKING CHANGE: remove deprecated method` |
| `Co-Authored-By` | Credit to co-authors | `Co-Authored-By: Name <email>` |
| `Refs` | Reference to issues/commits | `Refs: #123, #456` |
| `Closes` | Closes related issues | `Closes #123` |
| `Fixes` | Fixes related issues | `Fixes #789` |
| `Reviewed-by` | Reviewer credit | `Reviewed-by: Name <email>` |

**Required Footer for Claude Code:**

```

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
```

**Requirements:**

- **EXACTLY one blank line** between body and footer section
- Footer tokens MUST use `-` in place of whitespace (except `BREAKING CHANGE`)
- Footer **MUST** include the Claude Code attribution when AI-assisted
- Footer **MUST** include Co-Authored-By line when AI-assisted

---

## Commit Message Style Guide

### Style Reference

- **ALWAYS** examine recent commit messages before writing new ones:
  ```bash
  git log --pretty=format:"%s%n%b" -10
  ```
- Match the style, tone, and structure of existing commits
- Maintain consistency across the project history

### Specificity Requirements

#### SR-1 (MUST): Concrete Descriptions

- Be specific about what changed and why
- ❌ Bad: "improve authentication" (too vague)
- ✅ Good: "add RS256 algorithm support to JWT token validation"

#### SR-2 (SHOULD): Context and Impact

- Explain the purpose of changes when not obvious
- Include impact on existing functionality if significant
- Mention related issues or PRs when applicable

---

## CG: CHANGELOG Generation Guidelines

This section describes how commit messages are transformed into CHANGELOG entries by release-plz. The configuration is defined in `release-plz.toml`.

### CG-1: Commit Type to CHANGELOG Section Mapping

Every commit type maps to a specific CHANGELOG section:

| Commit Type | CHANGELOG Section | Keep a Changelog Category |
|-------------|-------------------|---------------------------|
| `feat` | Added | Added |
| `fix` | Fixed | Fixed |
| `perf` | Performance | — (custom) |
| `refactor` | Changed | Changed |
| `docs` | Documentation | — (custom) |
| `revert` | Reverted | — (custom) |
| `deprecated` | Deprecated | Deprecated |
| `security` | Security | Security |
| `chore` | Maintenance | — (custom) |
| `ci` | Maintenance | — (custom) |
| `build` | Maintenance | — (custom) |
| `test` | Testing | — (custom) |
| `style` | Styling | — (custom) |

All commit types are included in the CHANGELOG. No commit type is silently dropped.

### CG-2: Writing CHANGELOG-Friendly Descriptions

Commit descriptions appear directly in the CHANGELOG. Write them so they make sense as standalone release note entries:

- ❌ Bad: `fix: resolve issue` (unclear without context)
- ❌ Bad: `refactor: clean up code` (too vague for release notes)
- ✅ Good: `fix(orm): resolve connection pool exhaustion under high concurrency`
- ✅ Good: `refactor(core): extract query builder into dedicated module`

**Guidelines:**

- Write descriptions that are meaningful to users reading release notes
- Include the affected component in the scope when applicable
- Be specific about what changed, not just that something changed

### CG-3: Scope and Breaking Change Rendering

**Scope**: The `(scope)` portion of the commit type is preserved in the CHANGELOG entry. Use scopes consistently to help users filter relevant changes.

**Breaking changes**: When `protect_breaking_commits = true` is set in `release-plz.toml`, commits with `!` or `BREAKING CHANGE:` footer are always included in the CHANGELOG, even if the commit type would otherwise be skipped. Breaking changes are rendered with a `[**breaking**]` prefix in the CHANGELOG.

### CG-4: GitHub Issue/PR Reference Auto-Linking

References to GitHub issues and PRs in commit messages are automatically converted to clickable links:

- `#123` → `[#123](https://github.com/kent8192/reinhardt-web/issues/123)`

This is handled by `commit_preprocessors` in `release-plz.toml`. Use `#NNN` format in commit descriptions or bodies to reference issues.

### CG-5: Automatically Skipped Commits

The following commit patterns are excluded from CHANGELOG generation:

| Pattern | Reason |
|---------|--------|
| `chore: release` | release-plz automation commits |
| `Merge ...` | Git merge commits |
| `Revert "..."` | GitHub-generated revert commits (manual `revert:` type commits are included) |
| `Initial plan ...` | Plan mode initialization commits |

**Note**: Even skipped commits with breaking changes are included due to `protect_breaking_commits = true`.

### CG-6: CHANGELOG Verification

After pushing commits, verify CHANGELOG generation in the Release PR:

1. Check that each commit appears in the expected section
2. Verify breaking changes are highlighted
3. Confirm issue references are properly linked
4. Review the Release PR diff for `CHANGELOG.md` files

```bash
# Preview what release-plz will generate (requires release-plz CLI)
release-plz generate-changelog
```

---

## Related Documentation

- **Main Quick Reference**: @CLAUDE.md (see Quick Reference section)
- **Main Standards**: @CLAUDE.md
- **Release Process**: @RELEASE_PROCESS.md
