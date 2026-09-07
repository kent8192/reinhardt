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

Run the capture from the Reinhardt workspace root. The first command only
prints and validates the proposed 27-file inventory. The second command runs
the independent oracle before writing any fixture:

```bash
python3 /tmp/reinhardt-6143-sdd-xlhw1o2t/capture-fixtures.py
python3 /tmp/reinhardt-6143-sdd-xlhw1o2t/capture-fixtures.py --write
```

The exact capture program is:

```python
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import subprocess
import tarfile
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument("--write", action="store_true")
args = parser.parse_args()
workspace = Path.cwd().resolve()
cloud = Path("/Users/kent8192/Projects/reinhardt-cloud")
cloud_commit = "f562c5942c567e273dd02f350858db97189ec015"
twitter_commit = "46033a5937dbd2e5f7cfdeea1cd2ef96d65cf834"
framework_ref = "reinhardt-web@v0.3.0-rc.2"
destination = workspace / "tests/integration/tests/fixtures/migration_source"

def git(repo, *arguments):
    return subprocess.check_output(["git", "-C", str(repo), *arguments])

cloud_paths = sorted(
    p for p in git(cloud, "ls-tree", "-r", "--name-only", cloud_commit,
                   "dashboard/migrations").decode().splitlines()
    if PurePosixPath(p).suffix == ".rs"
    and PurePosixPath(p).name[0].isdigit()
)
assert len(cloud_paths) == 23, cloud_paths
inputs = [
    (cloud, "kent8192/reinhardt-cloud", cloud_commit, p,
     "cloud/" + p.removeprefix("dashboard/migrations/"))
    for p in cloud_paths
]
twitter_prefix = "crates/reinhardt-db/tests/fixtures/migration_source/v0_1_4/twitter/"
inputs += [
    (workspace, "kent8192/reinhardt-web", twitter_commit,
     twitter_prefix + app + "/0001_initial.rs",
     "twitter/" + app + "/0001_initial.rs")
    for app in ["auth", "dm", "profile", "tweet"]
]
rows, sources = [], {}
for repo, repository, commit, original_path, relative_path in inputs:
    source = git(repo, "show", commit + ":" + original_path)
    fixture_path = (
        "tests/integration/tests/fixtures/migration_source/" + relative_path
        if relative_path.startswith("cloud/") else original_path
    )
    if relative_path.startswith("twitter/"):
        assert (workspace / fixture_path).read_bytes() == source
    sources[relative_path] = source
    rows.append({
        "relative_path": relative_path, "fixture_path": fixture_path,
        "repository": repository, "commit": commit, "original_path": original_path,
        "sha256": hashlib.sha256(source).hexdigest(),
    })
framework_commit = git(
    workspace, "rev-parse", framework_ref + "^{commit}"
).decode().strip()
manifest = {"framework_commit": framework_commit, "files": rows}
print(json.dumps(manifest, indent=2))
if not args.write:
    raise SystemExit(0)

with tempfile.TemporaryDirectory(prefix="reinhardt-6143-oracle-") as temporary:
    temporary = Path(temporary)
    framework = temporary / "framework"
    framework.mkdir()
    archive = git(workspace, "archive", "--format=tar", framework_commit)
    with tarfile.open(fileobj=io.BytesIO(archive)) as packed:
        packed.extractall(framework, filter="data")
    oracle = temporary / "oracle"
    (oracle / "src").mkdir(parents=True)
    modules, entries = [], []
    for index, row in enumerate(rows):
        relative_path = row["relative_path"]
        source_path = oracle / "src" / ("migration_" + str(index) + ".rs")
        source_path.write_bytes(sources[relative_path])
        modules.append("mod migration_" + str(index) + ";")
        entries.append(
            "(" + json.dumps(relative_path) + ", migration_" + str(index) + "::migration())"
        )
    main = "\n".join(modules) + "\nfn main() {\n"
    main += "let migrations = std::collections::BTreeMap::from(["
    main += ",\n".join(entries) + "]);\n"
    main += 'println!("{}", serde_json::to_string(&migrations).unwrap());\n}\n'
    (oracle / "src/main.rs").write_text(main)
    (oracle / "Cargo.toml").write_text(
        '[package]\nname="legacy-migration-oracle"\nversion="0.0.0"\nedition="2024"\n'
        '[dependencies]\nreinhardt={package="reinhardt-web",path='
        + json.dumps(str(framework))
        + ',default-features=false,features=["database"]}\nserde_json="1"\n'
    )
    output = subprocess.run(
        ["cargo", "run", "--quiet", "--manifest-path", str(oracle / "Cargo.toml")],
        cwd=oracle, capture_output=True, text=True, check=True,
    )
    expected = json.loads(output.stdout)
    assert sorted(expected) == sorted(sources)
    manifest["oracle_lock_sha256"] = hashlib.sha256(
        (oracle / "Cargo.lock").read_bytes()
    ).hexdigest()

# No fixture mutation occurs before the independent oracle succeeds.
destination.mkdir(parents=True, exist_ok=True)
for row in rows:
    if row["relative_path"].startswith("cloud/"):
        output_path = workspace / row["fixture_path"]
        output_path.parent.mkdir(parents=True, exist_ok=True)
        source = sources[row["relative_path"]]
        if output_path.exists():
            assert output_path.read_bytes() == source
        else:
            output_path.write_bytes(source)
for name, value in [("manifest.json", manifest), ("expected.json", expected)]:
    target = destination / name
    content = json.dumps(value, indent=2, sort_keys=True) + "\n"
    if target.exists():
        assert json.loads(target.read_text()) == value
    else:
        target.write_text(content)
```

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

## Task 4 downstream acceptance

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

The pinned Cloud snapshot check was previewed and then run from the same
framework worktree:

```bash
python3 /tmp/reinhardt-6143-sdd-xlhw1o2t/check-cloud-snapshot.py
python3 /tmp/reinhardt-6143-sdd-xlhw1o2t/check-cloud-snapshot.py --run
```

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
