# Makefile.toml for Reinhardt Project
#
# This file provides cargo-make task definitions for common development operations.
# Install cargo-make: `cargo install cargo-make`
# Usage: `cargo make <task>`

[config]
default_to_workspace = false
skip_core_tasks = true

# ============================================================================
# Environment Variables
# ============================================================================

[env]
# Use local target directory for each example (independent from workspace)
CARGO_TARGET_DIR = { value = "target", condition = { env_not_set = ["CARGO_TARGET_DIR"] } }
WASM_TARGET = "wasm32-unknown-unknown"
WASM_BINDGEN_VERSION = "0.2.100"

# ============================================================================
# Development Server
# ============================================================================

[tasks.runserver]
description = "Start the development server with static files"
command = "cargo"
args = ["run", "--bin", "manage", "runserver", "--with-pages"]
dependencies = ["wasm-build-dev"]

[tasks.runserver-watch]
description = "Start the development server with auto-reload (requires bacon)"
command = "bacon"
args = ["runserver"]

# ============================================================================
# WASM Build
# ============================================================================

[tasks.wasm-compile-dev]
description = "Compile WASM binary (debug mode)"
command = "cargo"
args = ["build", "--target", "wasm32-unknown-unknown"]

[tasks.collectstatic-wasm]
description = "Collect static files to dist/ for WASM frontend"
command = "cargo"
args = ["run", "--bin", "manage", "collectstatic", "--noinput"]
dependencies = ["wasm-compile-dev"]

[tasks.wasm-bindgen-dev]
description = "Generate WASM bindings (debug mode)"
script = '''
WASM_FILE=$(ls target/wasm32-unknown-unknown/debug/*.wasm 2>/dev/null | head -1)
if [ -n "$WASM_FILE" ]; then
	wasm-bindgen --target web "$WASM_FILE" --out-dir dist --debug
	echo "✓ WASM bindings generated"
else
	echo "❌ No WASM file found"
	exit 1
fi
'''
dependencies = ["collectstatic-wasm"]

[tasks.wasm-finalize-dev]
description = "Finalize WASM build"
script = '''
echo "✓ WASM build completed: dist/"
'''
dependencies = ["wasm-bindgen-dev"]

[tasks.wasm-build-dev]
description = "Build WASM in debug mode"
dependencies = ["wasm-finalize-dev"]

[tasks.wasm-compile-release]
description = "Compile WASM binary (release mode)"
command = "cargo"
args = ["build", "--target", "wasm32-unknown-unknown", "--release"]

[tasks.wasm-bindgen-release]
description = "Generate WASM bindings (release mode)"
script = '''
WASM_FILE=$(ls target/wasm32-unknown-unknown/release/*.wasm 2>/dev/null | head -1)
if [ -n "$WASM_FILE" ]; then
	wasm-bindgen --target web "$WASM_FILE" --out-dir dist
	echo "✓ WASM bindings generated"
else
	echo "❌ No WASM file found"
	exit 1
fi
'''
dependencies = ["collectstatic-wasm"]

[tasks.wasm-finalize-release]
description = "Finalize WASM build (optimize with wasm-opt)"
script = '''
# Optimize with wasm-opt if available
if command -v wasm-opt &> /dev/null; then
	echo "Running wasm-opt..."
	WASM_BG=$(ls dist/*_bg.wasm 2>/dev/null | head -1)
	if [ -n "$WASM_BG" ]; then
		wasm-opt -O3 "$WASM_BG" -o "$WASM_BG"
		echo "✓ WASM optimized"
	fi
else
	echo "⚠️  wasm-opt not found, skipping optimization"
fi

echo "✓ WASM build completed: dist/"
'''
dependencies = ["wasm-bindgen-release"]

[tasks.wasm-build-release]
description = "Build WASM in release mode with optimization"
dependencies = ["wasm-finalize-release"]

[tasks.wasm-watch]
description = "Watch and rebuild WASM on changes"
command = "cargo"
args = ["watch", "-w", "src/client", "-s", "cargo make wasm-build-dev"]

[tasks.wasm-clean]
description = "Clean WASM build artifacts"
script = '''
rm -rf dist/
echo "WASM build artifacts cleaned"
'''

[tasks.clean-cache]
description = "Clean WASM artifacts and Rust incremental build cache"
script = '''
echo "🧹 Cleaning build cache..."

# WASM artifacts
if [ -d "dist" ]; then
	rm -rf dist
	echo "  ✓ Removed dist/"
fi

# Rust incremental build cache
if [ -d "target/debug/incremental" ]; then
	rm -rf target/debug/incremental
	echo "  ✓ Removed target/debug/incremental/"
fi

# WASM target build cache
if [ -d "target/wasm32-unknown-unknown" ]; then
	rm -rf target/wasm32-unknown-unknown
	echo "  ✓ Removed target/wasm32-unknown-unknown/"
fi

echo "✨ Build cache cleaned"
'''

# ============================================================================
# Database Migrations
# ============================================================================

[tasks.makemigrations]
description = "Create new migrations based on model changes"
command = "cargo"
args = ["run", "--bin", "manage", "makemigrations"]

[tasks.makemigrations-app]
description = "Create new migrations for a specific app (usage: cargo make makemigrations-app -- <app_label>)"
command = "cargo"
args = ["run", "--bin", "manage", "makemigrations", "${@}"]

[tasks.migrate]
description = "Apply database migrations"
command = "cargo"
args = ["run", "--bin", "manage", "migrate"]

# ============================================================================
# Static Files
# ============================================================================

[tasks.collectstatic]
description = "Collect static files into STATIC_ROOT"
command = "cargo"
args = ["run", "--bin", "manage", "collectstatic"]

# ============================================================================
# Project Management
# ============================================================================

[tasks.check]
description = "Check the project for common issues"
command = "cargo"
args = ["run", "--bin", "manage", "check"]

[tasks.showurls]
description = "Display all registered URL patterns"
command = "cargo"
args = ["run", "--bin", "manage", "showurls"]

[tasks.shell]
description = "Run an interactive Rust shell (REPL)"
command = "cargo"
args = ["run", "--bin", "manage", "shell"]

# ============================================================================
# Testing
# ============================================================================

[tasks.test]
description = "Run all tests"
command = "cargo"
args = ["nextest", "run", "--all-features"]

[tasks.test-unit]
description = "Run unit tests only"
command = "cargo"
args = ["nextest", "run", "--lib", "--all-features"]

[tasks.test-integration]
description = "Run integration tests only"
command = "cargo"
args = ["nextest", "run", "--test", "*", "--all-features"]

[tasks.test-watch]
description = "Run tests with auto-reload (requires bacon)"
command = "bacon"
args = ["test"]

# ============================================================================
# Code Quality
# ============================================================================

[tasks.fmt-check]
description = "Check code formatting (rustfmt + page! DSL)"
command = "reinhardt-admin"
args = ["fmt", ".", "--check"]

[tasks.fmt-fix]
description = "Fix code formatting (rustfmt + page! DSL)"
command = "reinhardt-admin"
args = ["fmt", "."]

[tasks.clippy-check]
description = "Check linting rules"
command = "cargo"
args = ["clippy", "--all-features", "--", "-D", "warnings"]

[tasks.clippy-fix]
description = "Fix linting issues automatically"
command = "cargo"
args = ["clippy", "--all-features", "--fix", "--allow-dirty", "--allow-staged"]

[tasks.quality]
description = "Run all code quality checks (format + lint)"
dependencies = ["fmt-check", "clippy-check"]

[tasks.quality-fix]
description = "Fix all code quality issues automatically"
dependencies = ["fmt-fix", "clippy-fix"]

# ============================================================================
# Build & Clean
# ============================================================================

[tasks.build]
description = "Build the project in debug mode"
command = "cargo"
args = ["build", "--all-features"]

[tasks.build-release]
description = "Build the project in release mode"
command = "cargo"
args = ["build", "--release", "--all-features"]

[tasks.clean]
description = "Clean build artifacts"
command = "cargo"
args = ["clean"]

# ============================================================================
# Development Workflow
# ============================================================================

[tasks.dev]
description = "Start development environment (checks, builds WASM, runs server)"
dependencies = ["clean-cache", "quality", "wasm-build-dev", "runserver"]

[tasks.dev-watch]
description = "Start development with auto-reload"
dependencies = ["clean-cache", "quality", "wasm-build-dev", "runserver-watch"]

# ============================================================================
# CI/CD Workflow
# ============================================================================

[tasks.ci]
description = "Run CI pipeline (format, lint, build, test)"
dependencies = ["fmt-check", "clippy-check", "build", "test"]

# ============================================================================
# Verbosity Control
# ============================================================================

[tasks.runserver-v]
description = "Start the development server with verbose output"
command = "cargo"
args = ["run", "--bin", "manage", "runserver", "-v"]

[tasks.runserver-vv]
description = "Start the development server with very verbose output"
command = "cargo"
args = ["run", "--bin", "manage", "runserver", "-vv"]

[tasks.runserver-vvv]
description = "Start the development server with maximum verbosity"
command = "cargo"
args = ["run", "--bin", "manage", "runserver", "-vvv"]

# ============================================================================
# Help
# ============================================================================

[tasks.help]
description = "Show available tasks"
script = '''
echo "Available tasks:"
echo "  Development:"
echo "    runserver          - Start the development server (with WASM)"
echo "    runserver-watch    - Start server with auto-reload"
echo "    dev                - Run checks + build WASM + start server"
echo "    dev-watch          - Development with auto-reload"
echo ""
echo "  WASM Build:"
echo "    wasm-build-dev     - Build WASM (debug mode)"
echo "    wasm-build-release - Build WASM (release + optimize)"
echo "    wasm-watch         - Watch and rebuild WASM on changes"
echo "    wasm-clean         - Clean WASM build artifacts"
echo ""
echo "  Database:"
echo "    makemigrations     - Create new migrations"
echo "    makemigrations-app - Create migrations for specific app"
echo "    migrate            - Apply migrations"
echo ""
echo "  Static Files:"
echo "    collectstatic      - Collect static files"
echo ""
echo "  Project Management:"
echo "    check              - Check project for issues"
echo "    showurls           - Show URL patterns"
echo "    shell              - Interactive REPL"
echo ""
echo "  Testing:"
echo "    test               - Run all tests"
echo "    test-unit          - Run unit tests"
echo "    test-integration   - Run integration tests"
echo "    test-watch         - Tests with auto-reload"
echo ""
echo "  Code Quality:"
echo "    fmt-check          - Check formatting"
echo "    fmt-fix            - Fix formatting"
echo "    clippy-check       - Check linting"
echo "    clippy-fix         - Fix linting issues"
echo "    quality            - Run all checks"
echo "    quality-fix        - Fix all issues"
echo ""
echo "  Build:"
echo "    build              - Build (debug)"
echo "    build-release      - Build (release)"
echo "    clean              - Clean artifacts"
echo "    clean-cache        - Clean WASM + Rust incremental cache"
echo ""
echo "  CI/CD:"
echo "    ci                 - Run CI pipeline"
echo ""
echo "Usage: cargo make <task>"
'''
