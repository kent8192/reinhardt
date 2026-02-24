# Release Process

## Purpose

This document provides step-by-step procedures for releasing Reinhardt crates to
crates.io using release-plz for automated versioning, CHANGELOG generation, and
publishing.

---

## Table of Contents

- [Overview](#overview)
- [How release-plz Works](#how-release-plz-works)
- [Automated Workflow](#automated-workflow)
- [Manual Intervention](#manual-intervention)
- [Configuration](#configuration)
	- [Configuration Rationale](#configuration-rationale)
- [Known Issues & Pitfalls](#known-issues--pitfalls)
- [Recovery Procedures](#recovery-procedures)
- [Troubleshooting](#troubleshooting)

---

## Overview

### Release Strategy

Reinhardt uses **release-plz** for fully automated release management:

- **Automated Versioning**: Versions are determined from conventional commits
- **Automated CHANGELOGs**: Generated from commit messages
- **Release PRs**: Automatically created when changes are detected
- **Git Tags**: Format `[crate-name]@v[version]`
- **GitHub Releases**: Created automatically upon merge

### Key Principles

1. **Conventional Commits**: Use proper commit message format for version bumps
2. **Automated Process**: release-plz handles version bumps and CHANGELOGs
3. **Review Before Release**: Release PRs allow review before publishing
4. **Per-Crate Releases**: Only changed crates are released

---

## How release-plz Works

### Commit-to-Version Mapping

release-plz uses [Conventional Commits](https://www.conventionalcommits.org/) to determine version bumps:

| Commit Type | Version Bump | Example |
|-------------|--------------|---------|
| `feat:` | MINOR | `feat(auth): add OAuth support` |
| `fix:` | PATCH | `fix(orm): resolve connection leak` |
| `feat!:` or `BREAKING CHANGE:` | MAJOR | `feat!: change API response format` |
| Other types | PATCH | `docs:`, `chore:`, `refactor:`, etc. |

### Automated CHANGELOG Generation

CHANGELOGs are generated from commit messages and categorized into sections based on custom `commit_parsers` in `release-plz.toml`. Each commit type maps to a specific CHANGELOG section:

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

**Example CHANGELOG output:**

```markdown
## [0.2.0] - 2026-01-30

### Added
- feat(auth): add OAuth support ([#123](https://github.com/kent8192/reinhardt-web/issues/123))

### Fixed
- fix(orm): resolve connection pool exhaustion under high concurrency ([#124](https://github.com/kent8192/reinhardt-web/issues/124))

### Performance
- perf(query): optimize batch insert with prepared statements

### Changed
- refactor(core): extract query builder into dedicated module

### Security
- security(auth): patch session fixation vulnerability
```

**Key features:**
- All commit types are categorized (no entries fall into "Other" unless unrecognized)
- GitHub issue/PR references (`#123`) are automatically converted to clickable links
- Breaking changes are always included, even from otherwise-skipped commits
- Commits are sorted in chronological order (oldest first)

For detailed guidelines on writing CHANGELOG-friendly commit messages, see [docs/COMMIT_GUIDELINE.md](COMMIT_GUIDELINE.md) § CG: CHANGELOG Generation Guidelines.

---

## Automated Workflow

### Step 1: Develop with Conventional Commits

Write commits following [Conventional Commits](https://www.conventionalcommits.org/):

```bash
git commit -m "feat(auth): add JWT token validation"
git commit -m "fix(orm): resolve race condition in connection pool"
git commit -m "feat!: change Model trait signature"
```

### Step 2: Push to Main Branch

```bash
git push origin main
```

### Step 3: release-plz Creates Release PR

When changes are pushed to main, release-plz automatically:

1. Analyzes commits since last release
2. Determines version bumps for affected crates
3. Updates `Cargo.toml` versions
4. Generates/updates CHANGELOG.md files
5. Creates a Release PR

**Release PR includes:**
- Version bumps in `Cargo.toml`
- Updated CHANGELOG.md files
- List of changes for each crate

### Step 4: Review and Merge Release PR

1. Review the Release PR
2. Verify version bumps are correct
3. Check CHANGELOG entries
4. Merge when ready

### Step 5: Automatic Publishing

Upon merge, release-plz:

1. Publishes crates to crates.io (in dependency order)
   - Automatically skips already-published versions
   - Handles workspace dependency ordering correctly
2. Creates Git tags (`[crate-name]@v[version]`)
3. Creates GitHub Releases

**Note**: The `release-plz release` command handles publishing gracefully:
- Already-published crate versions are skipped automatically (no errors on retry)
- Only publishable crates are processed (respects `publish = false` in Cargo.toml)
- Workspace dependencies are published in the correct order

---

## Manual Intervention

### Editing Release PR

You can modify the Release PR before merging:

- **Adjust CHANGELOG entries**: Edit for clarity or add details
- **Modify version bumps**: Change version in `Cargo.toml` if needed
- **Add migration notes**: Include breaking change documentation

### Force Version Bump

To force a specific version, manually edit `Cargo.toml` in the Release PR.

### Skip Release

To skip releasing a crate, add to `release-plz.toml`:

```toml
[[package]]
name = "crate-name"
release = false
```

---

## Configuration

### release-plz.toml

Configuration file at repository root. Key CHANGELOG-related settings:

```toml
[workspace]
changelog_update = true
pr_branch_prefix = "release-plz-"
pr_labels = ["release", "automated"]
pr_name = "chore: release"
git_release_enable = true
git_tag_enable = true
git_tag_name = "{{ package }}@v{{ version }}"
git_release_type = "auto"
semver_check = false
publish_timeout = "10m"
dependencies_update = true
release_always = true
publish_no_verify = true

# Exclude packages from release
[[package]]
name = "reinhardt-test-support"
release = false
publish = false

[changelog]
protect_breaking_commits = true
sort_commits = "oldest"
commit_parsers = [
  # Skip automated commits, then map types to sections
  { message = "^chore: release", skip = true },
  { message = "^Merge", skip = true },
  { message = "^feat", group = "Added" },
  { message = "^fix", group = "Fixed" },
  # ... (see release-plz.toml for full list)
]
commit_preprocessors = [
  # Convert #123 to clickable GitHub links
  { pattern = '#(\d+)', replace = "[#${1}](https://github.com/kent8192/reinhardt-web/issues/${1})" },
]
```

**CHANGELOG Customization Settings:**

| Setting | Purpose |
|---------|---------|
| `protect_breaking_commits` | Always include breaking change commits, even if their type would be skipped |
| `sort_commits` | Sort commits chronologically (`"oldest"` = oldest first) |
| `commit_parsers` | Map commit types to CHANGELOG sections; skip automated commits |
| `commit_preprocessors` | Transform commit messages (e.g., auto-link GitHub references) |

See `release-plz.toml` at repository root for the complete configuration.

### Non-Published Packages

The following packages are excluded from release:

- `reinhardt-test-support` - Test utilities
- `reinhardt-integration-tests` - Integration tests
- `reinhardt-benchmarks` - Benchmark tests
- `examples-*` - Example projects

### Configuration Rationale

Key configuration decisions and the reasons behind them:

**`pr_branch_prefix = "release-plz-"`**

The branch prefix **must** start with `"release-plz-"` for the native two-step release workflow to function correctly. When `release_always = false`, release-plz determines whether to publish by checking if the latest commit originates from a PR whose branch starts with this prefix. Using a different prefix (e.g., `"release/"`) causes `release-plz release` to skip publishing entirely because it cannot detect the merged Release PR. (Ref: [#186](https://github.com/kent8192/reinhardt-web/pull/186))

**`publish_no_verify = true`**

During `cargo publish`, Cargo attempts to build the crate including dev-dependencies. If dev-dependencies reference other workspace crates that have not yet been published to crates.io, the build verification step fails. Setting `publish_no_verify = true` skips this verification, allowing crates to be published in dependency order without false failures. (Ref: [#181](https://github.com/kent8192/reinhardt-web/pull/181))

**`dependencies_update = true`**

Enables release-plz to automatically update explicit `version` fields in workspace dependency declarations when a dependent crate's version is bumped. Without this, workspace members that pin explicit versions would become out-of-sync after a release, causing the next Release PR to carry stale dependency versions. (Ref: [#223](https://github.com/kent8192/reinhardt-web/pull/223))

**`release_always = true`**

Ensures `release-plz release` publishes ALL crates whose local version differs from crates.io, not just those with actual code changes. This prevents the phantom version issue described in [KI-5](#ki-5-phantom-version-bumps-from-dependencies_update): when `dependencies_update = true` bumps versions for dependency-only changes, `release_always = false` would skip publishing those crates, creating versions in git that don't exist on crates.io. Normal code pushes are unaffected since local versions match crates.io; only after a Release PR merge will version differences trigger publishing. (Ref: [#185](https://github.com/kent8192/reinhardt-web/pull/185), [#186](https://github.com/kent8192/reinhardt-web/pull/186), [#246](https://github.com/kent8192/reinhardt-web/issues/246))

**`reinhardt-test` workspace dependency without `version` field**

The `reinhardt-test` crate (`publish = false`) is used as a workspace dependency by publishable crates. Its workspace dependency entry in the root `Cargo.toml` intentionally omits the `version` field. Adding a `version` field would cause `cargo publish` to attempt resolving `reinhardt-test` from crates.io (where it does not exist), breaking the publish of any crate that depends on it via dev-dependencies. This is related to a Cargo regression tracked in [cargo#15151](https://github.com/rust-lang/cargo/issues/15151). (Ref: [#185](https://github.com/kent8192/reinhardt-web/pull/185), [#223](https://github.com/kent8192/reinhardt-web/pull/223))

---

## Known Issues & Pitfalls

### KI-1: Circular Publish Dependencies

**Problem**: `cargo publish` resolves all dependencies (including dev-dependencies) from crates.io. If crate A has a dev-dependency on crate B, and crate B has a dev-dependency on crate A, neither can be published first — creating a deadlock.

**Impact on Reinhardt**: The `reinhardt-test` crate provides test fixtures used across the workspace. If a functional crate (e.g., `reinhardt-orm`) adds `reinhardt-test` to its `[dev-dependencies]`, and `reinhardt-test` already depends on that functional crate, a circular publish dependency is created.

**Rule**: Functional crates **must not** include other Reinhardt crates in `[dev-dependencies]`. Tests requiring cross-crate fixtures belong in the `reinhardt-integration-tests` crate.

**Detection**: Run `cargo publish --dry-run` for each publishable crate before merging changes that modify dev-dependencies.

(Ref: [#181](https://github.com/kent8192/reinhardt-web/pull/181), [#199](https://github.com/kent8192/reinhardt-web/pull/199), [#203](https://github.com/kent8192/reinhardt-web/pull/203), [#216](https://github.com/kent8192/reinhardt-web/pull/216))

### KI-2: Cargo 1.84+ Dev-Dependency Resolution Regression

**Problem**: Starting with Cargo 1.84, `cargo publish` attempts to resolve workspace dev-dependencies from crates.io even when they are marked `publish = false`. If the workspace dependency entry includes a `version` field, Cargo tries to find that version on crates.io and fails when the crate does not exist there.

**Workaround**: Ensure that unpublished workspace crates (e.g., `reinhardt-test`) do **not** have a `version` field in their `[workspace.dependencies]` entry. The `publish_no_verify = true` setting provides additional protection by skipping the verification build.

**Tracking**: [cargo#15151](https://github.com/rust-lang/cargo/issues/15151)

(Ref: [#185](https://github.com/kent8192/reinhardt-web/pull/185), [#207](https://github.com/kent8192/reinhardt-web/pull/207), [#223](https://github.com/kent8192/reinhardt-web/pull/223))

### KI-3: Partial Release Failure Deadlock

**Problem**: When release-plz publishes multiple crates in dependency order, a failure partway through (e.g., network error, crates.io outage) leaves some crates published at their new versions while others remain at their old versions. The next `release-plz release-pr` run sees the already-published crates as released and generates a new Release PR only for the remaining crates — but with potentially incorrect dependency version requirements.

**Symptoms**:
- Release PR contains version bumps for only a subset of crates
- Published crates reference dependency versions that do not exist on crates.io
- Subsequent publish attempts fail with dependency resolution errors

**Resolution**: Follow [RP-1: Partial Release Failure Recovery](#rp-1-partial-release-failure-recovery).

(Ref: [#204](https://github.com/kent8192/reinhardt-web/pull/204), [#223](https://github.com/kent8192/reinhardt-web/pull/223), [#226](https://github.com/kent8192/reinhardt-web/pull/226))

### KI-4: gix/gitoxide Slotmap Overflow

**Problem**: The `gix` library (used internally by release-plz for Git operations) has a known issue where its object cache slotmap can overflow under certain repository conditions, causing a panic during `release-plz release-pr` or `release-plz release`.

**Symptoms**:
- CI workflow fails with a panic in `gix` or `gitoxide` code paths
- Error messages reference slotmap capacity or object cache

**Workaround**: Re-run the workflow (the issue is intermittent). If persistent, clear the GitHub Actions cache for the release-plz workflow. A `workflow_dispatch` trigger has been added to allow manual re-runs.

**Tracking**: [gitoxide#1788](https://github.com/GitoxideLabs/gitoxide/issues/1788)

(Ref: [#225](https://github.com/kent8192/reinhardt-web/pull/225))

### KI-5: Phantom Version Bumps from `dependencies_update`

**Symptom**: Crates are version-bumped in Release PR but not published to crates.io. Downstream crates fail with "dependency not found" errors.

**Root cause**: When `dependencies_update = true`, `release-plz release-pr` bumps versions for dependency-only changes. However, `release-plz release` with `release_always = false` skips those crates since they have no actual code changes. This creates "phantom versions" — versions referenced in git that don't exist on crates.io.

**Impact**: For pre-release semver (`0.x.y-alpha.N`), Cargo's `^` requirement resolves to exact version match, so any downstream crate depending on a phantom version will fail to publish.

**Resolution**: Set `release_always = true` in `release-plz.toml`. This ensures all crates whose local version differs from crates.io are published, including those bumped only for dependency updates. Normal code pushes are unaffected since versions match crates.io; only Release PR merges trigger publishing.

**History**: This issue caused 3+ RP-1 recovery cycles before the root cause was identified.

(Ref: [#246](https://github.com/kent8192/reinhardt-web/issues/246))

---

## Recovery Procedures

### RP-1: Partial Release Failure Recovery

Use this procedure when some crates were published successfully but others failed during a release cycle.

**Step 1: Identify published and unpublished crates**

```bash
# Check which crate versions exist on crates.io
for crate in reinhardt-core reinhardt-database reinhardt-orm reinhardt-web reinhardt-macros reinhardt-test; do
  version=$(curl -s "https://crates.io/api/v1/crates/$crate" | jq -r '.crate.max_version // "not found"')
  echo "$crate: $version"
done
```

Compare the crates.io versions with the versions in the failed Release PR to identify which crates were not published.

**Step 2: Roll back unpublished crate versions**

For each crate that was **not** published, revert its version and CHANGELOG changes to match the current crates.io version:

```bash
# Revert Cargo.toml version for unpublished crates
git checkout main -- crates/<unpublished-crate>/Cargo.toml
git checkout main -- crates/<unpublished-crate>/CHANGELOG.md
```

**Step 3: Push and wait for new Release PR**

```bash
git add -A
git commit -m "fix(release): roll back unpublished crate versions after partial release failure"
git push origin main
```

release-plz will detect the version discrepancies and create a new Release PR containing only the unpublished crates with correct dependency versions.

**Step 4: Review and merge the new Release PR**

Verify that:
- Only unpublished crates have version bumps
- Dependency versions reference published versions
- CHANGELOG entries are correct

(Ref: [#204](https://github.com/kent8192/reinhardt-web/pull/204), [#223](https://github.com/kent8192/reinhardt-web/pull/223), [#226](https://github.com/kent8192/reinhardt-web/pull/226))

### RP-2: Circular Dependency Deadlock Recovery

Use this procedure when `cargo publish` fails due to circular dev-dependency chains.

**Step 1: Identify the circular chain**

```bash
# Check dev-dependencies of each publishable crate
for crate_dir in crates/*/; do
  crate_name=$(basename "$crate_dir")
  echo "=== $crate_name ==="
  grep -A 20 '\[dev-dependencies\]' "$crate_dir/Cargo.toml" 2>/dev/null | head -20
done
```

Look for cycles: if crate A dev-depends on crate B, and crate B dev-depends on crate A (directly or transitively).

**Step 2: Break the cycle**

Choose one of these strategies:
1. **Remove the unnecessary dev-dependency**: If the dev-dependency is not actually needed, remove it.
2. **Move tests to integration test crate**: Move tests that require the cross-crate dependency to `reinhardt-integration-tests`.
3. **Create local test helpers**: Replace the imported test fixtures with crate-local equivalents.

**Step 3: Verify and publish**

```bash
# Verify no circular dependencies remain
cargo publish --dry-run -p <crate-name>
```

If the previous release was partially completed, also follow [RP-1](#rp-1-partial-release-failure-recovery).

(Ref: [#203](https://github.com/kent8192/reinhardt-web/pull/203), [#216](https://github.com/kent8192/reinhardt-web/pull/216))

### RP-3: gix Cache Failure Recovery

Use this procedure when the release-plz CI workflow fails due to `gix`/`gitoxide` panics.

**Step 1: Re-run the workflow**

The gix slotmap overflow is intermittent. Navigate to the GitHub Actions page and re-run the failed workflow:

```bash
# List recent workflow runs
gh run list --workflow=release-plz.yml --limit=5

# Re-run a specific failed run
gh run rerun <run-id>
```

**Step 2: Clear cache if persistent**

If the failure persists across multiple re-runs:

```bash
# List GitHub Actions caches
gh cache list

# Delete release-plz related caches
gh cache delete <cache-key>
```

**Step 3: Manual dispatch**

The release-plz workflow supports `workflow_dispatch` for manual triggering:

```bash
gh workflow run release-plz.yml
```

(Ref: [#225](https://github.com/kent8192/reinhardt-web/pull/225))

### RP-4: reinhardt-test Version Reintroduced

Use this procedure when a `version` field is accidentally added to the `reinhardt-test` workspace dependency.

**Detection**: `cargo publish --dry-run` fails for crates that dev-depend on `reinhardt-test`, with errors indicating that `reinhardt-test` cannot be found on crates.io.

**Step 1: Remove the version field**

In the root `Cargo.toml`, locate the `[workspace.dependencies]` section and remove the `version` field from the `reinhardt-test` entry:

```toml
# Before (broken)
reinhardt-test = { path = "crates/reinhardt-test", version = "0.1.0" }

# After (correct)
reinhardt-test = { path = "crates/reinhardt-test" }
```

**Step 2: Verify**

```bash
cargo publish --dry-run -p reinhardt-orm  # or any crate that dev-depends on reinhardt-test
```

(Ref: [#185](https://github.com/kent8192/reinhardt-web/pull/185), [#223](https://github.com/kent8192/reinhardt-web/pull/223))

---

## Troubleshooting

### Common Issues

**No Release PR Created:**
- Verify commits use conventional commit format
- Check that changes affect publishable crates
- Review release-plz workflow logs

**Wrong Version Bump:**
- Ensure commit messages follow conventions
- Use `feat!:` or `BREAKING CHANGE:` for major bumps
- Edit the Release PR to correct version

**Publish Failed:**
- Check `CARGO_REGISTRY_TOKEN` secret is set
- Verify crate metadata is complete
- Review crates.io for existing version conflicts
- **Already Published**: release-plz automatically skips already-published versions, so retry is safe
- **Circular Dependency**: See [KI-1: Circular Publish Dependencies](#ki-1-circular-publish-dependencies) and [RP-2](#rp-2-circular-dependency-deadlock-recovery)
- **Dev-Dependency Resolution**: See [KI-2: Cargo 1.84+ Dev-Dependency Resolution Regression](#ki-2-cargo-184-dev-dependency-resolution-regression) and [RP-4](#rp-4-reinhardt-test-version-reintroduced)
- **Partial Failure**: See [KI-3: Partial Release Failure Deadlock](#ki-3-partial-release-failure-deadlock) and [RP-1](#rp-1-partial-release-failure-recovery)
- **gix Panic**: See [KI-4: gix/gitoxide Slotmap Overflow](#ki-4-gixgitoxide-slotmap-overflow) and [RP-3](#rp-3-gix-cache-failure-recovery)
- **Phantom Version (dependency not found)**: See [KI-5: Phantom Version Bumps from `dependencies_update`](#ki-5-phantom-version-bumps-from-dependencies_update)

**CHANGELOG Not Updated:**
- Ensure `changelog_update = true` in config
- Verify commit messages are properly formatted

### Verification Commands

```bash
# Check release-plz config
cat release-plz.toml

# Verify conventional commits
git log --oneline -10

# Check crates.io version
curl -s "https://crates.io/api/v1/crates/<crate-name>" | jq '.crate.max_version'
```

---

## Related Documentation

- **Main Quick Reference**: @CLAUDE.md
- **Commit Guidelines**: @COMMIT_GUIDELINE.md
- **release-plz Documentation**: https://release-plz.ieni.dev/docs
