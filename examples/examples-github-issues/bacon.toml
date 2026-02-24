# Bacon configuration for Reinhardt Project
# https://dystroy.org/bacon/

# Default job to run when `bacon` is called without arguments
default_job = "check"

# ============================================================================
# Development Server Jobs
# ============================================================================

[jobs.runserver]
command = ["cargo", "run", "--bin", "manage", "--", "runserver"]
watch = ["src", "Cargo.toml"]
need_stdout = true

[jobs.runserver-noreload]
command = ["cargo", "run", "--bin", "manage", "--", "runserver", "--noreload"]
watch = ["src", "Cargo.toml"]
need_stdout = true

# ============================================================================
# Core Development Jobs
# ============================================================================

[jobs.check]
command = ["cargo", "check", "--all-features"]
need_stdout = false
watch = ["src", "Cargo.toml"]

[jobs.clippy]
command = ["cargo", "clippy", "--all-features", "--", "-D", "warnings"]
need_stdout = false
watch = ["src", "Cargo.toml"]

[jobs.test]
command = ["cargo", "nextest", "run", "--all-features"]
need_stdout = true
watch = ["src", "tests", "Cargo.toml"]

[jobs.build]
command = ["cargo", "build", "--all-features"]
need_stdout = false
watch = ["src", "Cargo.toml"]
