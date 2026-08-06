#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(git rev-parse --show-toplevel)"
WORKFLOW="$ROOT_DIR/.github/workflows/coverage.yml"
ruby - "$WORKFLOW" <<'RUBY'
require "yaml"

workflow = YAML.load_file(ARGV.fetch(0))
steps = workflow
  .fetch("jobs")
  .fetch("intra-crate-integration-coverage")
  .fetch("steps")

step = lambda do |name|
  steps.find { |candidate| candidate["name"] == name } ||
    raise("missing workflow step: #{name}")
end

configure = step.call("Configure mold linker for coverage").fetch("run")
test_run = step.call("Run intra-crate integration tests with coverage").fetch("run")
report = step.call("Generate intra-crate coverage report").fetch("run")
upload = steps.find { |candidate| candidate["uses"] == "codecov/codecov-action@v5" } ||
  raise("missing intra-crate Codecov upload step")
upload_index = steps.index(upload)
report_index = steps.index { |candidate| candidate["name"] == "Generate intra-crate coverage report" }

raise "intra-crate job must export COVERAGE_HOST_TARGET exactly once" unless
  configure.scan("COVERAGE_HOST_TARGET=$HOST_TARGET").length == 1
raise "test phase must use COVERAGE_HOST_TARGET exactly once" unless
  test_run.scan('--target "$COVERAGE_HOST_TARGET"').length == 1
raise "report phase must use COVERAGE_HOST_TARGET exactly once" unless
  report.scan('--target "$COVERAGE_HOST_TARGET"').length == 1
raise "test phase must use --coverage-target-only exactly once" unless
  test_run.scan("--coverage-target-only").length == 1
raise "report phase must use --coverage-target-only exactly once" unless
  report.scan("--coverage-target-only").length == 1
raise "intra-crate LCOV must be validated exactly once" unless
  report.scan("bash scripts/validate-lcov-hits.sh /tmp/intra-crate-lcov.info").length == 1
raise "LCOV validation must run before Codecov upload" unless
  report_index && upload_index && report_index < upload_index
raise "intra-crate Codecov upload must remain fail-closed" if upload.key?("if")

puts "PASS: intra-crate coverage target symmetry"
RUBY
