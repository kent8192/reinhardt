#!/usr/bin/env python3
"""Rewrite a generated fixture's Cargo.toml to point at this PR's HEAD
(workspace path) or at extracted .crate tarballs (publish-form mode).
Registry dependencies retain their consumer-declared versions.

Usage:
  patch-fixture-cargo-toml.py --manifest Cargo.toml --reinhardt-path /path/to/repo
  patch-fixture-cargo-toml.py --manifest Cargo.toml --use-packaged --pkg-stage /tmp/pkg-stage

Tracks: kent8192/reinhardt-web#4161
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tarfile
from pathlib import Path


def enable_features(manifest: Path, extra_features: list[str]) -> None:
	"""Append additional features to the fixture's `reinhardt = { ..., features = [...] }`
	array. Idempotent: features already present are skipped.

	Operates on the raw text because the multi-line features array spans the
	template-rendered manifest. The regex is bounded to the `reinhardt = { ... }`
	dependency block to avoid touching unrelated arrays.
	"""
	text = manifest.read_text()
	pattern = re.compile(
		r'(reinhardt\s*=\s*\{[^}]*?features\s*=\s*\[)([^\]]*?)(\][^}]*?\})',
		re.DOTALL,
	)
	match = pattern.search(text)
	if not match:
		print(
			"error: could not locate `reinhardt = { ..., features = [...] }` in manifest",
			file=sys.stderr,
		)
		sys.exit(4)
	prefix, body, suffix = match.group(1), match.group(2), match.group(3)
	existing = {tok.strip().strip(",").strip('"') for tok in body.split() if tok.strip()}
	additions = [f for f in extra_features if f not in existing]
	if not additions:
		return
	insertion = "".join(f'\t"{f}",\n' for f in additions)
	stripped_body = body.rstrip()
	separator = ""
	if stripped_body and not stripped_body.endswith(","):
		separator = ","
	new_body = stripped_body + separator + "\n" + insertion
	manifest.write_text(text[: match.start()] + prefix + new_body + suffix + text[match.end() :])


def workspace_form(manifest: Path, reinhardt_path: Path) -> None:
	"""Repoint every Reinhardt dependency at the workspace checkout.

	The forms produced by the project_pages_template include:
	  1. `reinhardt = { version = "...", package = "reinhardt-web", ... }`
	  2. `reinhardt-shell = { version = "...", package = "reinhardt-web", ... }`
	  3. `reinhardt-commands = { version = "...", ... }`
	  4. `[dev-dependencies.reinhardt] \\n version = "..."` (table form)
	All must be rewritten — leaving one at `version = "..."` while another
	uses `path = "..."` triggers Cargo's
	`Dependency 'reinhardt' has different source paths depending on the build target`
	error, since dev-deps and prod-deps are treated as separate resolution targets.
	Leaving a direct crate or renamed alias on crates.io also makes the fixture
	resolve it against the last published feature set instead of PR HEAD.
	"""
	metadata = subprocess.run(
		[
			"cargo",
			"metadata",
			"--format-version",
			"1",
			"--no-deps",
			"--manifest-path",
			str(reinhardt_path / "Cargo.toml"),
		],
		capture_output=True,
		check=True,
		text=True,
	)
	package_paths = {
		package["name"]: Path(package["manifest_path"]).parent
		for package in json.loads(metadata.stdout)["packages"]
		if package["name"].startswith("reinhardt-")
	}

	def dependency_package(name: str, body: str) -> str:
		package_match = re.search(r'\bpackage\s*=\s*"([^"]+)"', body)
		return package_match.group(1) if package_match else name

	text = manifest.read_text()
	# Inline dependencies, including renamed dependencies identified by `package`.
	inline_pattern = re.compile(
		r'^(?P<name>[A-Za-z0-9_-]+)\s*=\s*\{(?P<body>[^}]*)\}',
		re.MULTILINE,
	)
	inline_count = 0
	unresolved_packages: set[str] = set()

	def rewrite_inline(match: re.Match[str]) -> str:
		nonlocal inline_count
		body = match.group("body")
		package = dependency_package(match.group("name"), body)
		if (
			not package.startswith("reinhardt-")
			or re.search(r'\bversion\s*=\s*"[^"]*"', body) is None
		):
			return match.group(0)
		path = package_paths.get(package)
		if path is None:
			unresolved_packages.add(package)
			return match.group(0)
		inline_count += 1
		new_body = re.sub(
			r'\bversion\s*=\s*"[^"]*"',
			f'path = "{path}"',
			body,
			count=1,
		)
		return f'{match.group("name")} = {{{new_body}}}'

	new_text = inline_pattern.sub(rewrite_inline, text)
	# Table dependencies, including target-specific and renamed dependencies.
	table_pattern = re.compile(
		r'(?P<header>^\[[^\]]*dependencies\.(?P<name>[A-Za-z0-9_-]+)\]\n)'
		r'(?P<body>.*?)(?=^\[|\Z)',
		re.MULTILINE | re.DOTALL,
	)
	table_count = 0

	def rewrite_table(match: re.Match[str]) -> str:
		nonlocal table_count
		body = match.group("body")
		package = dependency_package(match.group("name"), body)
		if (
			not package.startswith("reinhardt-")
			or re.search(r'^version\s*=\s*"[^"]*"', body, re.MULTILINE) is None
		):
			return match.group(0)
		path = package_paths.get(package)
		if path is None:
			unresolved_packages.add(package)
			return match.group(0)
		table_count += 1
		new_body = re.sub(
			r'^version\s*=\s*"[^"]*"',
			f'path = "{path}"',
			body,
			count=1,
			flags=re.MULTILINE,
		)
		return match.group("header") + new_body

	new_text = table_pattern.sub(rewrite_table, new_text)
	if unresolved_packages:
		print(
			"error: unresolved versioned Reinhardt dependencies: "
			+ ", ".join(sorted(unresolved_packages)),
			file=sys.stderr,
		)
		sys.exit(2)
	if inline_count == 0 and table_count == 0:
		print(
			"error: no versioned Reinhardt dependency found in manifest",
			file=sys.stderr,
		)
		sys.exit(2)
	manifest.write_text(new_text)
	# UnifiedRouter (used by `mode = unified` in the augment patch) is gated
	# `cfg(any(native, feature = "client-router"))` in reinhardt-urls. The
	# fixture's `["full", "admin"]` features pulled from project_pages_template
	# do NOT include client-router, so wasm builds cannot resolve UnifiedRouter
	# without this augmentation. Tracked as a follow-up template fix.
	enable_features(manifest, ["client-router"])


def _safe_extract(tf: tarfile.TarFile, dest: Path) -> None:
	"""Extract a tar archive into ``dest`` rejecting any member whose path
	would escape the destination (path traversal / zip-slip).

	`cargo package` tarballs are not attacker-controlled in our CI, but the
	guard is cheap insurance and keeps static analysers happy.
	"""
	dest_resolved = dest.resolve()
	for member in tf.getmembers():
		# Reject absolute paths, parent traversal, and device/symlink members.
		member_name = member.name
		if member_name.startswith("/") or ".." in Path(member_name).parts:
			raise RuntimeError(f"unsafe member in tarball: {member_name!r}")
		if member.issym() or member.islnk() or member.isdev():
			raise RuntimeError(f"unsupported member type in tarball: {member_name!r}")
		target = (dest / member_name).resolve()
		if not str(target).startswith(str(dest_resolved) + "/") and target != dest_resolved:
			raise RuntimeError(f"member escapes destination: {member_name!r}")
		tf.extract(member, dest)  # noqa: S202 — path validated above


def packaged_form(manifest: Path, pkg_stage: Path) -> None:
	"""Extract every `*.crate` under `pkg_stage` and append a `[patch.crates-io]`
	block to the manifest pointing each `reinhardt-*` crate at its extracted dir.
	"""
	extract_dir = pkg_stage / "extracted"
	extract_dir.mkdir(parents=True, exist_ok=True)

	crates: dict[str, Path] = {}
	for crate in sorted(pkg_stage.glob("*.crate")):
		with tarfile.open(crate, "r:gz") as tf:
			_safe_extract(tf, extract_dir)
		stem = crate.stem  # e.g. "reinhardt-web-0.1.0-rc.26"
		extracted = extract_dir / stem
		# Split on the first `-N` boundary — semver versions always start with a digit.
		m = re.match(r"^(?P<name>.+?)-(?P<version>\d.+)$", stem)
		if not m:
			print(f"warn: cannot parse crate stem {stem}", file=sys.stderr)
			continue
		crates[m.group("name")] = extracted

	patch_lines = ["", "[patch.crates-io]"]
	for name, path in sorted(crates.items()):
		if not name.startswith("reinhardt"):
			continue
		patch_lines.append(f'{name} = {{ path = "{path}" }}')

	if len(patch_lines) <= 2:
		print(
			"error: no reinhardt-* crates found in pkg-stage; nothing to patch",
			file=sys.stderr,
		)
		sys.exit(3)

	manifest.write_text(manifest.read_text() + "\n" + "\n".join(patch_lines) + "\n")
	# Same client-router rationale as in workspace_form: the augment patch's
	# `mode = unified` invocation requires UnifiedRouter, which is only
	# re-exported on wasm when reinhardt-urls/client-router is enabled.
	enable_features(manifest, ["client-router"])


def main() -> int:
	ap = argparse.ArgumentParser()
	ap.add_argument("--manifest", required=True, type=Path)
	ap.add_argument("--reinhardt-path", type=Path)
	ap.add_argument("--use-packaged", action="store_true")
	ap.add_argument("--pkg-stage", type=Path)
	args = ap.parse_args()

	if not args.manifest.exists():
		print(f"error: manifest not found: {args.manifest}", file=sys.stderr)
		return 2

	if args.use_packaged:
		if args.pkg_stage is None:
			ap.error("--use-packaged requires --pkg-stage")
		packaged_form(args.manifest, args.pkg_stage)
	else:
		if args.reinhardt_path is None:
			ap.error("workspace form requires --reinhardt-path")
		workspace_form(args.manifest, args.reinhardt_path)

	return 0


if __name__ == "__main__":
	sys.exit(main())
