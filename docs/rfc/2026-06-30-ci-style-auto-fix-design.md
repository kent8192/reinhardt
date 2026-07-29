# RFC - CI Style Auto-Fix

| Key | Value |
|---|---|
| Status | Draft |
| Date | 2026-06-30 |
| Scope | GitHub Actions CI for pull-request style gates |
| Author | kent8192 |

---

## 1. Context

The CI workflow currently fails pull requests when either the format gate or the
Clippy gate finds style drift. The repository already has the authoritative fix
commands:

- `cargo make fmt-fix`
- `cargo make clippy-fix`
- `cargo make auto-fix`

The format command is repository-specific. It runs the Reinhardt formatter and
therefore covers `page!` DSL formatting in addition to regular Rust formatting.
Using plain `cargo fmt` would not match the existing CI gate.

## 2. Goals

Automatically commit safe style fixes when a pull request fails the format or
Clippy gate.

Keep the write boundary narrow:

- only same-repository pull requests are eligible;
- fork pull requests remain read-only;
- branches with classic or write-blocking protection remain read-only;
- a non-fast-forward-only ruleset remains eligible because it permits the
  normal commit used by auto-fix while still rejecting history rewrites;
- release-managed branches remain read-only;
- no pull-request comments are posted by the auto-fix workflow.

## 3. Non-Goals

- Fixing compile, test, WASM, docs, publish, audit, or SemVer failures.
- Running auto-fix on fork pull requests.
- Pushing to `main`, `master`, `develop/*`, `release/*`, `release-plz-*`, or
  `develop-release-plz-*`.
- Force-pushing, rebasing, or rewriting existing branch history.
- Creating pull requests, closing issues, or posting GitHub comments.

## 4. Selected Approach

Add three downstream jobs to `.github/workflows/ci.yml`:

- `auto-fix-style-target` checks whether the pull-request head is eligible;
- `auto-fix-style` runs the formatter and Clippy fix commands without write
  credentials and exports a patch artifact when changes are produced;
- `commit-auto-fix-style` applies the patch in a clean checkout and writes the
  commit through GitHub GraphQL.

The target job runs only when the reusable `fmt` or `clippy` job fails. It is
gated to `pull_request` events where the pull-request head repository is the
same as the base repository. It checks the repository branch summary and active
branch rules so the write path is disabled for classic or write-blocking
protection, not just when a name matches a local denylist. GitHub's aggregate
`protected` flag also covers non-fast-forward-only rulesets, so that
force-push-only case remains eligible. Both target checks load the shared
policy from the pull request's base commit, so a pull-request head cannot alter
the eligibility code that guards the write path.

The fix job checks out the pull-request head SHA that triggered the workflow,
installs the same tools and environment used by the style gates, runs the
matching fix command, and uploads a binary patch only when the fix command
leaves a worktree diff.

## 5. Execution Flow

1. `ci.yml` runs the existing `fmt` and `clippy` reusable workflows.
2. If either job fails, `auto-fix-style-target` loads the shared policy from the
   trusted base commit and evaluates the same-repository, release-managed,
   project read-only branch, classic protection, and active branch-rule gates.
   A `non_fast_forward`-only ruleset remains eligible.
3. If the pull request is ineligible, the downstream fix and write jobs are
   skipped.
4. If eligible, `auto-fix-style` checks out the PR head SHA that triggered the
   workflow.
5. The fix job installs Rust, `rustfmt`, `clippy`, `protoc`, and `cargo-make`.
6. The fix job runs:
   - `cargo make fmt-fix` when only formatting failed;
   - `cargo make clippy-fix` when only Clippy failed;
   - `cargo make auto-fix` when both failed.
7. If there is no diff after the fix command, the workflow exits without
   committing.
8. If there is a diff, the fix job uploads a binary patch artifact.
9. `commit-auto-fix-style` checks out a clean copy of the same PR head SHA and
   applies the patch without executing PR-controlled build or make code.
10. The write job reloads the policy from the trusted base commit and rechecks
    the target branch protection and active rule state before generating a
    write-capable token.
11. If the branch is still eligible, the write job creates the commit with
    GitHub GraphQL `createCommitOnBranch`, using the triggering head SHA as the
    expected branch head.

## 6. Token and Push Model

The fix job runs formatter and Clippy fix commands without a write token
available to the checkout. The checkout uses `persist-credentials: false`, and
the job exports only a patch artifact.

The write job starts from a clean checkout, downloads the patch, and applies it
with `git apply --index`. It does not run `cargo make`, build scripts,
proc-macros, repository hooks, or policy code from the pull-request head before
generating the write token. The eligibility policy is sparse-checked out from
the pull request's base commit into an isolated path.

The write job validates the staged patch before generating the write token.
Unsupported file statuses and symlink additions are rejected, and GraphQL file
contents are read from staged blobs instead of following working-tree paths.
Each blob is streamed through `base64` and `jq`, avoiding process arguments so
large auto-fix files remain within exec limits.

The GitHub App token is generated only in the write job after the patch is
applied and the target branch protection and active rules are rechecked. The
`permission-contents: write` action input grants only the GitHub App
`contents: write` repository permission.

The GraphQL commit uses the triggering pull-request head SHA as
`expectedHeadOid`. If the contributor pushes another commit while the auto-fix
run is in flight, commit creation fails closed instead of applying a stale
patch to the newer branch tip.

The write job uses the existing repository pattern based on GraphQL
`createCommitOnBranch`, instead of local `git commit` plus `git push`. This
matches the release workflow's web-flow commit path and avoids the
`github-actions[bot]` push behavior that can suppress `pull_request:
synchronize` check runs.

## 7. Failure Handling

The auto-fix workflow does not mask the original style failure unless it
successfully creates a new commit.

If the fix command fails, the job fails and pushes nothing. If the fix command
succeeds but leaves no diff, the workflow writes nothing. If patch application
or GraphQL commit creation fails, the write job fails and leaves the pull
request in its original failing state.

This keeps false-positive auto-fix runs visible while still allowing a successful
web-flow commit to trigger a fresh CI run on the corrected branch.

## 8. Commit Shape

The auto-fix commit uses:

- message: `ci: auto-fix fmt and clippy`;
- body: a short note with the workflow run URL.

The workflow does not post PR comments. The GraphQL commit is the audit trail.

## 9. Validation

Local validation for the workflow change:

- `actionlint -shellcheck= .github/workflows/ci.yml`
- `bash scripts/tests/test-ci-auto-fix-style-target.sh`
- shell syntax checks for any extracted multi-line shell script used by the job
- inspection that both target checks use the shared branch policy before the
  write-capable path can run
- inspection that both policy copies are sparse-checked out from the pull
  request's base commit rather than the pull-request head
- unit coverage that non-fast-forward-only protection remains eligible while
  classic and write-blocking protection remain ineligible
- inspection that the App token step grants only the `contents: write`
  repository permission
- inspection that both auto-fix jobs check out the triggering PR head SHA
- inspection that the GraphQL commit uses that same SHA as `expectedHeadOid`
- inspection that staged additions are read from the index and symlinks are
  rejected before token generation

Hosted validation:

- create or update a same-repository test PR with intentional formatter drift;
- confirm a topic branch covered only by the repository-wide non-fast-forward
  rule reaches the fix and write jobs;
- confirm the fix job uploads a patch artifact and the write job creates a
  GraphQL commit;
- confirm the new GraphQL commit triggers a fresh CI run;
- confirm fork, protected-branch, and project read-only branch cases remain
  read-only by inspection of the workflow gates.

## 10. Open Risks

Clippy auto-fix may not repair every warning. That is acceptable: the workflow
pushes only when a concrete diff is produced, and otherwise leaves the original
failure for manual repair.

The auto-fix job can add CI runtime after a style failure. This is acceptable
because it runs only after a failing gate and can save a full manual fix cycle.
