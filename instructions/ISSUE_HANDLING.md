# Issue Handling Principles

## Purpose

This file defines strategic principles for handling multiple issues efficiently. While docs/ISSUE_GUIDELINES.md covers individual issue creation and management, this document provides workflow-level guidance for planning, batching, and parallelizing issue resolution across the Reinhardt project's multi-crate workspace.

---

## Handling Approach

### HA-1 (SHOULD): Fix Pattern Batch Processing

Group issues by fix pattern (the technique or approach used to resolve them) and process them as a batch.

**Rationale:** When multiple issues require the same type of fix (e.g., input validation, error handling improvement, dependency update), addressing them together reduces context-switching overhead and ensures consistency.

**Example:**

| Fix Pattern | Issues |
|-------------|--------|
| Input validation | #101 (SQL injection), #103 (path traversal), #107 (XSS) |
| Error handling | #102 (panic on invalid input), #105 (missing error context) |
| Dependency update | #104 (outdated tokio), #106 (vulnerable serde version) |

**Application:**
1. Categorize open issues by their fix approach
2. Identify common patterns that span multiple issues
3. Address each pattern group as a cohesive work unit

### HA-2 (SHOULD): Phase Division by Severity

Divide batch work into phases ordered by severity and exploitability, addressing the most critical issues first.

**Rationale:** Prioritizing by severity ensures that the highest-risk issues are resolved earliest, minimizing exposure window.

**Phase Example:**

| Phase | Severity | Description | Issues |
|-------|----------|-------------|--------|
| Phase 1 | Critical | Actively exploitable vulnerabilities | #101, #103 |
| Phase 2 | High | Significant risk but harder to exploit | #107, #102 |
| Phase 3 | Medium | Important improvements | #104, #105, #106 |

**Application:**
1. Assess severity of each issue in the batch
2. Group into phases by severity tier
3. Complete each phase before moving to the next
4. Each phase produces one or more PRs

### HA-3 (SHOULD): Agent Team Parallel Work

Use Agent Teams to parallelize work across independent crates within the same phase.

**Rationale:** When fixes in different crates are independent (no shared code changes required), they can be implemented simultaneously by different agents, reducing total elapsed time.

**Prerequisites for parallelization:**
- Fixes are in separate crates with no shared dependencies being modified
- No cross-crate utility or shared code changes are needed
- Each agent can complete its work independently

**Example:**
```
Phase 1 (parallel work):
  Agent A → reinhardt-core (fix #101)
  Agent B → reinhardt-orm (fix #103)
  Agent C → reinhardt-http (fix #107)
```

**When NOT to parallelize:**
- When fixes require changes to shared utilities first (see WU-3)
- When one fix depends on another fix being completed
- When fixes modify the same files or modules

### HA-4 (MUST): Branch Organization

Organize work into branches with descriptive names. Each work unit MUST produce logically grouped PRs.

**Branch naming:**
```
<type>/<description>

Examples:
fix/sql-injection-prevention
fix/xss-sanitization
security/input-validation
```

**Rules:**
- One branch per logical work unit (see WU-1)
- Branch names MUST NOT include internal metadata such as phase numbers, agent states, or workflow identifiers
- Branch names MUST be descriptive and understandable to other developers without project-specific context
- Branches may contain multiple commits if they follow commit guidelines (@docs/COMMIT_GUIDELINE.md)

---

## Work Unit Principles

### WU-1 (MUST): Basic Work Unit

**1 PR = 1 crate × 1 fix pattern** is the basic work unit for batch issue handling.

**Rationale:** This granularity ensures PRs are focused, reviewable, and independently mergeable. Each PR addresses a specific concern in a specific location.

**Examples:**

| PR | Crate | Fix Pattern | Issues Addressed |
|----|-------|-------------|------------------|
| PR #1 | reinhardt-core | Input validation | #101 |
| PR #2 | reinhardt-orm | Input validation | #103 |
| PR #3 | reinhardt-http | Input validation | #107 |

**This means:**
- Each PR modifies files within a single crate
- Each PR applies a single, cohesive fix approach
- Each PR can be reviewed and merged independently

### WU-2 (SHOULD): Same-Crate Combination

Related issues within the same crate MAY be combined into a single PR when they share context or the fixes are interrelated.

**When to combine:**
- Fixes touch the same files or modules
- One fix naturally addresses another issue
- Fixes are tightly related (e.g., path traversal + directory traversal in the same handler)

**When NOT to combine:**
- Fixes are in different modules with no shared context
- Combining would make the PR too large (>400 lines)
- Fixes are for different severity levels

**Example:**
```
PR: "fix(http): add input sanitization for path and query parameters"
  - Fixes #103 (path traversal in file handler)
  - Fixes #107 (XSS in query parameter reflection)
  - Both fixes are in reinhardt-http request handling
```

### WU-3 (MUST): Cross-Crate Preceding PRs

When batch fixes require shared utilities or cross-crate changes, these MUST be created as preceding PRs before per-crate fix PRs.

**Rationale:** Shared changes must be merged first to avoid merge conflicts and ensure each per-crate PR has a stable foundation.

**Workflow:**
```
Step 1: PR "feat(core): add shared input sanitization utilities"
  → Merged first

Step 2 (parallel, after Step 1 merge):
  PR "fix(orm): apply input sanitization to query builder"
  PR "fix(http): apply input sanitization to request handlers"
  PR "fix(api): apply input sanitization to API endpoints"
```

**Rules:**
- Preceding PRs MUST be merged before dependent PRs
- Preceding PRs SHOULD be minimal — only the shared code needed
- Per-crate PRs MUST reference the preceding PR in their description
- Never duplicate shared logic across crate-specific PRs

---

## Workflow Example

**Scenario:** 6 security issues identified across 3 crates.

**Step 1: Categorize by fix pattern (HA-1)**

| Fix Pattern | Issues | Crates Affected |
|-------------|--------|-----------------|
| Input validation | #101, #103, #107 | core, orm, http |
| Error handling | #102, #105 | core, orm |
| Dependency update | #104 | (workspace-level) |

**Step 2: Divide into phases by severity (HA-2)**

| Phase | Issues | Fix Pattern |
|-------|--------|-------------|
| Phase 1 | #101, #103, #107 | Input validation (critical) |
| Phase 2 | #102, #105 | Error handling (high) |
| Phase 3 | #104 | Dependency update (medium) |

**Step 3: Identify cross-crate dependencies (WU-3)**

Phase 1 requires shared sanitization utilities → preceding PR needed.

**Step 4: Execute Phase 1**

```
Commit 1 (preceding): "feat(core): add input sanitization module"
  → PR #A, merge first

Commits 2-4 (parallel via Agent Team, HA-3):
  "fix(core): apply input sanitization to query execution"  → PR #B
  "fix(orm): apply input sanitization to model fields"      → PR #C
  "fix(http): apply input sanitization to request parsing"   → PR #D
```

**Step 5: Execute Phase 2, Phase 3 similarly**

---

## Quick Reference

### ✅ MUST DO
- Use 1 PR = 1 crate × 1 fix pattern as the basic work unit (WU-1)
- Create preceding PRs for cross-crate shared changes before per-crate fix PRs (WU-3)
- Use descriptive branch names without internal metadata (HA-4)
- Merge preceding PRs before dependent per-crate PRs (WU-3)

### ❌ NEVER DO
- Mix changes to unrelated crates in a single issue-fix PR
- Mix unrelated fix patterns in a single PR
- Skip preceding PRs for cross-crate shared utilities
- Duplicate shared logic across crate-specific PRs instead of extracting to a preceding PR

---

## Related Documentation

- **Issue Guidelines**: docs/ISSUE_GUIDELINES.md
- **Pull Request Guidelines**: docs/PR_GUIDELINE.md
- **Commit Guidelines**: docs/COMMIT_GUIDELINE.md
- **GitHub Interaction**: docs/GITHUB_INTERACTION.md

---

**Note**: This document provides strategic guidance for batch issue handling. For individual issue creation and management, see docs/ISSUE_GUIDELINES.md. For PR formatting and review process, see docs/PR_GUIDELINE.md.
