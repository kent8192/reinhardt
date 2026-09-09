# Historical Migration Source Fixtures

These fixtures exercise source upgrades against immutable migration histories.
The 23 files under `cloud/` come from `kent8192/reinhardt-cloud` commit
`f562c5942c567e273dd02f350858db97189ec015`. The four Twitter files referenced
by `manifest.json` remain in their original shared fixture directory and come
from `kent8192/reinhardt-web` commit
`46033a5937dbd2e5f7cfdeea1cd2ef96d65cf834`.

`manifest.json` records every original repository path and SHA-256 digest. Its
`framework_commit` is `0790cf420cf803a428b704c4711f59ac427dacae`, the commit
resolved by `reinhardt-web@v0.3.0-rc.2`. The old framework directly executes
the original migration functions to produce `expected.json`; it does not use
the current source converter or parser. The Twitter fixtures predate this
capture and are reused byte-for-byte, so the old framework identifies the
compatible execution baseline rather than their generator version.

## Reproduce the fixtures

The checked-in [capture script](capture-fixtures.py) requires Python 3.12 or
newer, Git, Rust/Cargo, and local Reinhardt and Cloud repositories containing
the pinned commits above. Full clones contain that history; shallow clones
must fetch those commits first. Oracle compilation also requires access to
Cargo dependencies. Run from the Reinhardt workspace root and set the Cloud
repository path for your machine:

```bash
CLOUD_REPO=/absolute/path/to/reinhardt-cloud
python3 tests/integration/tests/fixtures/migration_source/capture-fixtures.py --cloud-repo "$CLOUD_REPO"
python3 tests/integration/tests/fixtures/migration_source/capture-fixtures.py --cloud-repo "$CLOUD_REPO" --write
```

The first command validates and prints the proposed 27-file inventory without
writing files. The second executes the original migration functions against
the pinned old framework, validates every existing artifact, and only then
creates missing fixtures. Existing sources and expected semantics must match
exactly. Existing files are never overwritten. The recorded
`oracle_lock_sha256` describes the original capture; validation with a newly
resolved dependency graph preserves that provenance while requiring identical
migration semantics. Temporary source exports are removed when the command
exits; Cargo caches follow the local Cargo configuration. `--workspace` selects
another Reinhardt repository; by default it is inferred from the script location.

## Acceptance gates

The pre-0.4 acceptance cases use `rstest`. The public-facade regression compares
the complete upgraded source, including the format marker, comments,
whitespace, and surrounding text, before compiling and checking the operation.

Run the acceptance gates with:

```bash
docker version
cargo test -p reinhardt-integration-tests --test commands historical_source_upgrade_compiles_and_preserves_semantics -- --nocapture
cargo test -p reinhardt-integration-tests --test commands cloud_source_upgrade_applies_complete_graph -- --nocapture
```

The first test proves source conversion, public-facade compilation, strict
filesystem loading, and equality with the independently executed model. The
second proves dependency ordering, PostgreSQL execution of the complete Cloud
graph, recorder identities, and repeat application. These checks detect
compiler, loader, execution, and model drift separately; they do not prove
release availability or downstream Cloud adoption.

Normal tests use only checked-in fixture data. They do not require the local
Cloud checkout or GitHub network access. The public-facade compiler check still
requires its Cargo dependencies to be available.

## Downstream acceptance

The controlled model boundary uses an independently authored
`ModelMetadata::new("legacy", "Items", "items")` target. It does not derive the
target model from migration replay. Run it from the Reinhardt workspace root:

```bash
cargo test -p reinhardt-integration-tests --test commands upgraded_legacy_history_preserves_independent_model_check -- --nocapture
```

At framework checkout commit
`e89a852818f607bc9547b03f124c2109ba6be9c6`, with the uncommitted issue #6143
implementation, this exited 0: one test passed and 259 were filtered out. The
current-format zero-drift baseline passed without changing its sole
`0001_initial.rs` file. The upgraded legacy source loaded through
`FilesystemSource`, applied once to SQLite, and the unchanged independent model
again reported no drift. Adding one nullable integer `extra` field returned the
exact diagnostic `Execution error: 1 migration(s) would be created`; check mode
left the sole migration filename and its bytes unchanged.

The checked-in [Cloud snapshot checker](check-cloud-snapshot.py) uses the same
prerequisites and Cloud repository path. Its default mode validates the paths
and pinned commit, then prints the proposed steps. `--run` exports an isolated
temporary snapshot, runs the local CLI conversion and repeat check, then
attempts the real Cloud compilation and model check. `--framework` can select
another framework workspace. The original Cloud checkout is only read.

```bash
CLOUD_REPO=/absolute/path/to/reinhardt-cloud
python3 tests/integration/tests/fixtures/migration_source/check-cloud-snapshot.py --cloud-repo "$CLOUD_REPO"
python3 tests/integration/tests/fixtures/migration_source/check-cloud-snapshot.py --cloud-repo "$CLOUD_REPO" --run
```

The following result was recorded against the framework revision above:

The preview exited 0. The run used `kent8192/reinhardt-cloud` commit
`f562c5942c567e273dd02f350858db97189ec015` and exited 1. It upgraded all 23
copied migration sources and the repeat `--check` reported that the source
format was current. The subsequent real Cloud `manage` compile stopped during
Cargo dependency resolution with exit 101: the current framework requires
`libc ^0.2.189`, while the pinned Cloud lock selects `libc 0.2.186` through
`tokio 1.52.3`. The `makemigrations --check` process was therefore not started.

The script's `finally` block hash-checked all 228 Rust files under the copied
`dashboard/src` tree against their original bytes and all 23 migration sources
against their upgraded bytes; both equality assertions passed. This failure is
an application dependency-resolution incompatibility reached after successful
conversion, not converter or graph/model-drift evidence. The controlled
independent-model gate passed, but the pinned Cloud model acceptance gate remains
unvalidated. Release availability and Cloud adoption remain separate work
tracked by `reinhardt-cloud#867`.
