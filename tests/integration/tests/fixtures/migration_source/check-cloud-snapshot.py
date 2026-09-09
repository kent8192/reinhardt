"""Check upgraded migration sources against a pinned, temporary Cloud snapshot."""

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import tarfile
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--cloud-repo", required=True, type=Path,
                    help="local reinhardt-cloud repository containing the pinned commit")
parser.add_argument("--framework", type=Path, default=Path(__file__).resolve().parents[5],
                    help="Reinhardt workspace whose CLI and public facade will be checked")
parser.add_argument("--run", action="store_true",
                    help="execute the previewed acceptance check in a temporary snapshot")
args = parser.parse_args()
framework = args.framework.resolve()
cloud = args.cloud_repo.resolve()
commit = "f562c5942c567e273dd02f350858db97189ec015"
subprocess.run(["git", "-C", str(cloud), "cat-file", "-e", commit + "^{commit}"], check=True)
if not (framework / "Cargo.toml").is_file():
    parser.error("--framework must identify a Reinhardt workspace")
print("Export pinned Cloud; upgrade 23 copied sources; use local framework; "
      "compile manage; check unchanged models; verify source hashes.")
if not args.run:
    raise SystemExit(0)

def hashes(root):
    return {
        str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(root.rglob("*.rs"))
    }

with tempfile.TemporaryDirectory(prefix="reinhardt-cloud-source-check-") as temporary:
    root = Path(temporary)
    snapshot = root / "cloud"
    snapshot.mkdir()
    archive = subprocess.check_output(
        ["git", "-C", str(cloud), "archive", "--format=tar", commit]
    )
    with tarfile.open(fileobj=io.BytesIO(archive)) as packed:
        packed.extractall(snapshot, filter="data")
    dashboard = snapshot / "dashboard"
    migrations = dashboard / "migrations"
    original_models = hashes(dashboard / "src")
    original_migrations = hashes(migrations)
    assert len(original_migrations) == 23

    admin = [
        "cargo", "run", "--quiet", "--manifest-path", str(framework / "Cargo.toml"),
        "-p", "reinhardt-admin-cli", "--bin", "reinhardt-admin", "--",
        "migrations", "upgrade-source",
    ]
    subprocess.run(admin + [str(migrations)], check=True, cwd=framework)
    upgraded_migrations = hashes(migrations)
    subprocess.run(admin + ["--check", str(migrations)], check=True, cwd=framework)
    assert hashes(migrations) == upgraded_migrations

    manifest = snapshot / "Cargo.toml"
    text = manifest.read_text()
    lines = text.splitlines(keepends=True)
    replacements = 0
    for index, line in enumerate(lines):
        if line.startswith("reinhardt = {"):
            assert 'package = "reinhardt-web"' in line
            lines[index], count = re.subn(
                r'version\s*=\s*"[^"]+"',
                "path = " + json.dumps(str(framework)), line, count=1
            )
            assert count == 1
            replacements += 1
    assert replacements == 1
    manifest.write_text("".join(lines))

    config = root / "settings"
    config.mkdir()
    (config / "local.toml").write_text('''
[core]
debug = true
secret_key = "source-upgrade-fixture-only-key"
allowed_hosts = ["localhost"]
root_urlconf = ""
middleware = []
[core.security]
append_slash = true
session_cookie_secure = false
csrf_cookie_secure = false
secure_ssl_redirect = false
secure_hsts_include_subdomains = false
secure_hsts_preload = false
[core.databases.default]
engine = "postgresql"
host = "127.0.0.1"
port = 1
name = "source_upgrade"
user = "postgres"
password = { secret = "fixture-only" }
options = {}
[cors]
allow_origins = ["http://localhost:8000"]
''')
    environment = os.environ.copy()
    environment.update({
        "REINHARDT_ENV": "local",
        "REINHARDT_CLOUD_CONFIG_DIR": str(config),
        "DATABASE_URL": "postgresql://postgres:fixture-only@127.0.0.1:1/source_upgrade",
        "CARGO_TARGET_DIR": str(root / "target"),
    })
    try:
        subprocess.run([
            "cargo", "check", "--manifest-path", str(dashboard / "Cargo.toml"),
            "--bin", "manage",
        ], check=True, cwd=dashboard, env=environment)
        subprocess.run([
            "cargo", "run", "--manifest-path", str(dashboard / "Cargo.toml"),
            "--bin", "manage", "--", "makemigrations", "--check",
            "--migrations-dir", str(migrations),
        ], check=True, cwd=dashboard, env=environment)
    finally:
        assert hashes(dashboard / "src") == original_models
        assert hashes(migrations) == upgraded_migrations
