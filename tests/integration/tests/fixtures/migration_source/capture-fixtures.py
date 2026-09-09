"""Capture or validate immutable migration fixtures using the old framework."""

import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import subprocess
import tarfile
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--cloud-repo", required=True, type=Path,
                    help="local reinhardt-cloud repository containing the pinned commit")
parser.add_argument("--workspace", type=Path, default=Path(__file__).resolve().parents[5],
                    help="Reinhardt repository containing the pinned framework and Twitter history")
parser.add_argument("--write", action="store_true",
                    help="run the independent oracle and create missing fixtures after validation")
args = parser.parse_args()
workspace = args.workspace.resolve()
cloud = args.cloud_repo.resolve()
cloud_commit = "f562c5942c567e273dd02f350858db97189ec015"
twitter_commit = "46033a5937dbd2e5f7cfdeea1cd2ef96d65cf834"
framework_ref = "0790cf420cf803a428b704c4711f59ac427dacae"
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
    fixture = workspace / fixture_path
    if relative_path.startswith("twitter/") or fixture.exists():
        if fixture.read_bytes() != source:
            raise SystemExit(f"Fixture differs from its pinned source: {fixture_path}")
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
manifest_path = destination / "manifest.json"
if manifest_path.exists():
    recorded = json.loads(manifest_path.read_text())
    # The digest describes the original capture's dependency resolution. A new
    # execution may resolve newer dependencies but must reproduce the same data.
    recorded.pop("oracle_lock_sha256", None)
    if recorded != manifest:
        raise SystemExit("Fixture inventory differs from manifest.json")
print(json.dumps(manifest, indent=2))
if not args.write:
    raise SystemExit(0)

with tempfile.TemporaryDirectory(prefix="reinhardt-migration-oracle-") as temporary:
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
        cwd=oracle, stdout=subprocess.PIPE, text=True, check=True,
    )
    expected = json.loads(output.stdout)
    assert sorted(expected) == sorted(sources)
    manifest["oracle_lock_sha256"] = hashlib.sha256(
        (oracle / "Cargo.lock").read_bytes()
    ).hexdigest()

# Preflight every existing artifact before creating any missing fixture.
pending = []
for row in rows:
    if row["relative_path"].startswith("cloud/"):
        output_path = workspace / row["fixture_path"]
        source = sources[row["relative_path"]]
        if output_path.exists():
            if output_path.read_bytes() != source:
                raise SystemExit(f"Fixture differs from its pinned source: {output_path}")
        else:
            pending.append((output_path, source))
for name, value in [("manifest.json", manifest), ("expected.json", expected)]:
    target = destination / name
    content = json.dumps(value, indent=2, sort_keys=True) + "\n"
    if target.exists():
        recorded = json.loads(target.read_text())
        if name == "manifest.json":
            recorded.pop("oracle_lock_sha256", None)
            value = {key: entry for key, entry in value.items() if key != "oracle_lock_sha256"}
        if recorded != value:
            raise SystemExit(f"Independent oracle differs from {name}")
    else:
        pending.append((target, content.encode()))

# No fixture mutation occurs before the independent oracle and preflight succeed.
for target, content in pending:
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(content)
print(f"Validated {len(rows)} sources and independent semantics; created {len(pending)} files.")
