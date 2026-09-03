#!/usr/bin/env python3
"""Behavior tests for patch-fixture-cargo-toml.py."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = Path(__file__).with_name("patch-fixture-cargo-toml.py")


class WorkspaceFormTests(unittest.TestCase):
	def run_script(self, manifest_text: str) -> tuple[subprocess.CompletedProcess[str], str]:
		with tempfile.TemporaryDirectory() as temporary:
			manifest = Path(temporary) / "Cargo.toml"
			manifest.write_text(manifest_text)
			result = subprocess.run(
				[
					sys.executable,
					str(SCRIPT),
					"--manifest",
					str(manifest),
					"--reinhardt-path",
					str(REPOSITORY_ROOT),
				],
				capture_output=True,
				check=False,
				text=True,
			)
			return result, manifest.read_text()

	def test_rewrites_direct_alias_target_and_table_dependencies(self) -> None:
		result, rewritten = self.run_script(
			"""
[package]
name = "fixture"
version = "0.1.0"

[dependencies]
reinhardt-commands = { version = "0.4", features = ["shell"] }
framework = { version = "0.4", package = "reinhardt-web", features = ["pages"] }

[target.'cfg(target_arch = "wasm32")'.dependencies]
reinhardt = { version = "0.4", package = "reinhardt-web", features = ["pages"] }

[dev-dependencies.query]
version = "0.4"
package = "reinhardt-query"
"""
		)

		self.assertEqual(result.returncode, 0, result.stderr)
		manifest = tomllib.loads(rewritten)
		self.assertEqual(
			manifest["dependencies"]["reinhardt-commands"]["path"],
			str(REPOSITORY_ROOT / "crates/reinhardt-commands"),
		)
		self.assertEqual(
			manifest["dependencies"]["framework"]["path"], str(REPOSITORY_ROOT)
		)
		self.assertEqual(
			manifest["target"]['cfg(target_arch = "wasm32")']["dependencies"]["reinhardt"][
				"path"
			],
			str(REPOSITORY_ROOT),
		)
		self.assertEqual(
			manifest["dev-dependencies"]["query"]["path"],
			str(REPOSITORY_ROOT / "crates/reinhardt-query"),
		)
		self.assertEqual(manifest["dependencies"]["tinyvec"], "=1.12.0")

	def test_resolves_nested_workspace_crate(self) -> None:
		result, rewritten = self.run_script(
			"""
[package]
name = "fixture"
version = "0.1.0"

[dependencies]
reinhardt-commands = { version = "0.4", features = ["shell"] }
reinhardt = { version = "0.4", package = "reinhardt-web", features = ["pages"] }
query-macros = { version = "0.4", package = "reinhardt-query-macros" }

[features]
default = []
"""
		)

		self.assertEqual(result.returncode, 0, result.stderr)
		manifest = tomllib.loads(rewritten)
		self.assertEqual(
			manifest["dependencies"]["query-macros"]["path"],
			str(REPOSITORY_ROOT / "crates/reinhardt-query/macros"),
		)
		self.assertEqual(manifest["dependencies"]["tinyvec"], "=1.12.0")

	def test_rejects_any_unresolved_reinhardt_dependency(self) -> None:
		original = """
[package]
name = "fixture"
version = "0.1.0"

[dependencies]
reinhardt-commands = { version = "0.4", features = ["shell"] }
reinhardt = { version = "0.4", package = "reinhardt-web", features = ["pages"] }
missing = { version = "0.4", package = "reinhardt-missing" }
"""
		result, rewritten = self.run_script(original)

		self.assertEqual(result.returncode, 2)
		self.assertIn("reinhardt-missing", result.stderr)
		self.assertEqual(rewritten, original)


if __name__ == "__main__":
	unittest.main()
