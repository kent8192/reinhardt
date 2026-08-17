#!/usr/bin/env bash
# Generate a wasm SPA-shaped consumer using reinhardt-admin templates and
# verify native target gating and WASM macro re-exports gated by Issue #4161.
#
# The scaffold itself (`startproject --with-pages` + `startapp --with-pages`)
# exercises the core re-exports (app_config, routes macro, AppLabel via
# InstalledApp). The previous augment patch for mode=unified|ws was removed
# after the URL routing simplification (Issue #4784).
#
# Usage:
#   build-wasm-consumer-fixture.sh                # workspace-path form (CI alpha)
#   build-wasm-consumer-fixture.sh --use-packaged # publish-form (CI beta)
#
# Tracks: kent8192/reinhardt-web#4161
set -euo pipefail

USE_PACKAGED=0
if [[ "${1:-}" == "--use-packaged" ]]; then
	USE_PACKAGED=1
fi

: "${GITHUB_WORKSPACE:?GITHUB_WORKSPACE must be set (path to reinhardt-web checkout)}"
WORK="${RUNNER_TEMP:-/tmp}/wasm-consumer-fixture"
PKG_STAGE="${PKG_STAGE:-/tmp/pkg-stage}"

rm -rf "$WORK"
mkdir -p "$WORK"

echo "::group::1) Scaffold project via reinhardt-admin startproject --with-pages"
cargo run --quiet \
	--manifest-path "$GITHUB_WORKSPACE/Cargo.toml" \
	-p reinhardt-admin-cli -- \
	startproject verifier --with-pages "$WORK/verifier"
echo "::endgroup::"

cd "$WORK/verifier"

echo "::group::2) Scaffold demo app via reinhardt-admin startapp --with-pages"
cargo run --quiet \
	--manifest-path "$GITHUB_WORKSPACE/Cargo.toml" \
	-p reinhardt-admin-cli -- startapp demo --with-pages
echo "::endgroup::"

echo "::group::3) Rewrite Cargo.toml to point at PR HEAD (or packaged tarballs)"
if [[ $USE_PACKAGED -eq 1 ]]; then
	python3 "$GITHUB_WORKSPACE/.github/scripts/patch-fixture-cargo-toml.py" \
		--manifest Cargo.toml \
		--use-packaged \
		--pkg-stage "$PKG_STAGE"
else
	python3 "$GITHUB_WORKSPACE/.github/scripts/patch-fixture-cargo-toml.py" \
		--manifest Cargo.toml \
		--reinhardt-path "$GITHUB_WORKSPACE"
fi
echo "::endgroup::"

echo "::group::4) cargo check --lib (native client-gating gate)"
cargo check --lib
echo "::endgroup::"

echo "::group::5) cargo check --target wasm32-unknown-unknown --lib (WASM style gate)"
cargo check --target wasm32-unknown-unknown --lib
echo "::endgroup::"

echo "wasm-consumer-fixture: PASS"
