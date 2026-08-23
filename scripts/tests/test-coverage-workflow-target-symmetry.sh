#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(git rev-parse --show-toplevel)"
WORKFLOW="$ROOT_DIR/.github/workflows/coverage.yml"
ruby - "$WORKFLOW" <<'RUBY'
require "yaml"

workflow = YAML.load_file(ARGV.fetch(0))
jobs = workflow.fetch("jobs")
steps = jobs
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
raise "intra-crate LCOV must use the non-empty validator exactly once" unless
  report.scan("bash scripts/validate-lcov-hits.sh /tmp/intra-crate-lcov.info").length == 1
raise "LCOV validation must run before Codecov upload" unless
  report_index && upload_index && report_index < upload_index
raise "intra-crate Codecov upload must remain fail-closed" if upload.key?("if")

unit_steps = jobs.fetch("unit-coverage").fetch("steps")
unit_run = unit_steps
  .find { |candidate| candidate["name"] == "Run unit tests with coverage" }
  &.fetch("run") || raise("missing unit coverage test step")
raise "unit coverage must select --bins exactly once" unless unit_run.scan("--bins").length == 1
raise "unit coverage must select --lib exactly once" unless unit_run.scan("--lib").length == 1

reports = [
  ["unit-coverage", "Generate unit coverage report", "/tmp/unit-lcov.info", "unit-lcov"],
  ["intra-crate-integration-coverage", "Generate intra-crate coverage report", "/tmp/intra-crate-lcov.info", "intra-crate-lcov"],
  ["cross-crate-integration-coverage", "Generate cross-crate coverage report", "/tmp/cross-crate-lcov.info", "cross-crate-lcov"],
]
reports.each do |job_name, report_name, lcov_path, artifact_name|
  job = jobs.fetch(job_name)
  job_steps = job.fetch("steps")
  report_step = job_steps.find { |candidate| candidate["name"] == report_name } ||
    raise("missing workflow step: #{report_name}")
  artifact = job_steps.find { |candidate| candidate["uses"] == "actions/upload-artifact@v4" } ||
    raise("missing #{job_name} LCOV artifact")
  codecov = job_steps.find { |candidate| candidate["uses"] == "codecov/codecov-action@v5" } ||
    raise("missing #{job_name} Codecov upload")
  report_index = job_steps.index(report_step)
  artifact_index = job_steps.index(artifact)
  codecov_index = job_steps.index(codecov)

  report_run = report_step.fetch("run")
  raise "#{job_name} LCOV must use the non-empty validator exactly once" unless
    report_run.scan("bash scripts/validate-lcov-hits.sh #{lcov_path}").length == 1 &&
    !report_run.include?("--require-complete")
  raise "#{job_name} must not use complete validation anywhere in the job" if
    YAML.dump(job).include?("--require-complete")
  raise "#{job_name} validation must precede artifact and Codecov uploads" unless
    report_index < artifact_index && report_index < codecov_index
  raise "#{job_name} artifact must retain for one day" unless artifact.dig("with", "retention-days") == 1
  raise "#{job_name} artifact must use its exact LCOV name" unless artifact.dig("with", "name") == artifact_name
  raise "#{job_name} artifact must upload only its LCOV report" unless artifact.dig("with", "path") == lcov_path
  raise "#{job_name} Codecov upload must use its LCOV report" unless codecov.dig("with", "files") == lcov_path
  %w[fail_ci_if_error use_oidc disable_search].each do |input|
    raise "#{job_name} Codecov upload must set #{input}: true" unless codecov.dig("with", input) == true
  end
  raise "#{job_name} Codecov upload must not bypass a failed coverage job" if codecov.key?("if")
end

aggregate = jobs.fetch("aggregate-coverage")
raise "aggregate coverage must depend on every coverage job" unless
  aggregate.fetch("needs").sort == reports.map(&:first).sort
agg_if = aggregate.fetch("if").to_s.gsub(/\s+/, " ").strip
raise "aggregate coverage must run after failed dependency jobs" unless agg_if.include?("always()")
raise "aggregate coverage must skip a cancelled workflow" unless agg_if.include?("!cancelled()")
reports.each do |job_name, *_|
  unless agg_if.include?("needs.#{job_name}.result != 'cancelled'")
    raise "aggregate coverage must skip when #{job_name} is cancelled"
  end
end
aggregate_steps = aggregate.fetch("steps")
raise "aggregate coverage must check out the repository" unless
  aggregate_steps.any? { |candidate| candidate["uses"] == "actions/checkout@v6" }
download = aggregate_steps.find { |candidate| candidate["uses"] == "actions/download-artifact@v5" } ||
  raise("missing aggregate LCOV artifact download")
raise "aggregate coverage must merge the LCOV artifacts" unless
  download.dig("with", "pattern") == "*-lcov" &&
  download.dig("with", "merge-multiple") == true &&
  download.dig("with", "path") == "/tmp/combined-lcov"
aggregate_validation = aggregate_steps
  .find { |candidate| candidate["run"]&.include?("scripts/validate-lcov-hits.sh") }
  &.fetch("run") || raise("missing aggregate LCOV validation")
expected_paths = reports.map { |_, _, lcov_path, _| "/tmp/combined-lcov/#{File.basename(lcov_path)}" }
raise "aggregate coverage must validate exactly the three LCOV reports" unless
  !aggregate_validation.include?("--require-complete") &&
  expected_paths.all? { |path| aggregate_validation.scan(path).length == 1 } &&
  aggregate_validation.scan("/tmp/combined-lcov/").length == 3

puts "PASS: intra-crate coverage target symmetry"
RUBY
