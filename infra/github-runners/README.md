# GitHub Runners Infrastructure

Self-hosted GitHub Actions runner pipeline for reinhardt-web CI, plus
the scheduled orphan-job detector Lambda (Issue #3903).

## Components

- **Upstream module**: `github-aws-runners/terraform-aws-github-runner` (v6.10.1)
  - Webhook → SQS → scale-up → EC2 Spot ephemeral runner → terminate.
  - See `runner.tf` for configuration.
- **Hotpath runner**: always-on `t4g.micro` for lightweight CI control jobs
  (see `hotpath-runner.tf`).
- **Orphan detector** (this doc): scheduled Lambda scanning for jobs
  stuck in `queued` state beyond 60 minutes, republishing via signed
  synthetic webhooks.

## Orphan Detector

### Why

The upstream `job_retry` Lambda is reactive: it re-queues webhook events
with delay. If a job's initial `workflow_job.queued` event is consumed
but the runner fails silently (Spot quota, JIT race, mid-assignment
termination), `job_retry` eventually exhausts and the job is stranded.

The orphan detector is an independent safety net that scans GitHub's API
directly every 10 minutes and republishes jobs stranded > 60 minutes.

### Architecture

```
EventBridge (rate: 10 min)
  └→ Lambda: reinhardt-ci-orphan-detector (nodejs20.x, arm64)
       ├→ GitHub App JWT (from SSM)
       ├→ GET /repos/{owner}/{repo}/actions/runs?status=queued (paginated)
       ├→ GET /repos/{owner}/{repo}/actions/runs/{id}/jobs (per run)
       ├→ Filter: status=queued AND created_at < now - STALENESS_MIN
       ├→ Dedup: skip jobs republished in last 2h (SSM state)
       ├→ Circuit breaker: halt + SNS alert if count > CIRCUIT_BREAKER_MAX
       ├→ POST signed synthetic workflow_job.queued to webhook Lambda URL
       └→ Emit CloudWatch metrics, save dedup state to SSM
```

### Files

| Path | Purpose |
|---|---|
| `orphan-detector.tf` | Terraform: Lambda, EventBridge, IAM, SNS, alarms |
| `lambda-src/orphan-detector/` | TypeScript source + tests |
| `lambda-src/orphan-detector/src/index.ts` | Handler entry point |

### Configuration

All tuning knobs are Terraform variables (see `variables.tf`):

| Variable | Default | Purpose |
|---|---|---|
| `orphan_detector_enabled` | `true` | Toggle all resources |
| `orphan_detector_staleness_min` | `60` | Orphan threshold (minutes) |
| `orphan_detector_circuit_breaker_margin` | `15` | threshold = runner_max_count + margin |
| `orphan_detector_schedule_expression` | `rate(10 minutes)` | EventBridge schedule |
| `orphan_detector_alert_email` | (empty) | Falls back to `budget_alert_email` |

### Deploy

```bash
cd infra/github-runners
./init.sh                      # downloads upstream zips, checks Node 20, builds Lambda
terraform plan -out=/tmp/plan
terraform apply /tmp/plan
```

Confirm the SNS email subscription by clicking the link in the AWS
notification email (one-time, required for alerts).

### Observe

- **CloudWatch Logs**: `/aws/lambda/reinhardt-ci-orphan-detector` (14d retention)
- **Metrics namespace**: `ReinhardtCI/OrphanDetector`
  - `OrphanJobsDetected`, `OrphanJobsRepublished`, `RepublishFailures`
  - `CircuitBreakerTripped`, `ScanDurationMs`, `GitHubApiRateLimitRemaining`
  - `DedupStateEntries`
- **Alarms**: 4 (circuit breaker, persistent orphans, Lambda errors, rate limit)

Tail logs:

```bash
aws logs tail /aws/lambda/reinhardt-ci-orphan-detector --follow --since 1h
```

### Runbook

#### Symptom: Email from `${prefix}-ci-alert` with subject "Orphan detector circuit breaker tripped"

1. Check CloudWatch logs: `aws logs tail /aws/lambda/reinhardt-ci-orphan-detector --since 1h`
2. Inspect SQS queue depth:
   `aws sqs get-queue-attributes --queue-url $(aws sqs get-queue-url --queue-name reinhardt-ci-queued-builds --query 'QueueUrl' --output text) --attribute-names All`
3. Check EC2 Spot fleet:
   `aws ec2 describe-spot-fleet-requests --region us-east-1`
4. Manual rescue on affected PR:
   ```bash
   gh run cancel <run-id> -R kent8192/reinhardt-web
   gh run rerun <run-id> --failed -R kent8192/reinhardt-web
   ```
5. Once root cause identified and fixed, clear dedup state if needed:
   `aws ssm put-parameter --name /reinhardt-ci/orphan-detector/processed --value '{}' --overwrite`

#### Symptom: Alarm "Lambda errors 2+ consecutive runs"

The detector itself is failing. Scanning is halted.

1. Check logs for stack trace.
2. If GitHub API rate-limit exhausted, wait 1 hour and disable EventBridge
   rule temporarily:
   `aws events disable-rule --name reinhardt-ci-orphan-detector-schedule`

#### Temporarily disable the detector

Option A (fast, non-destructive):

```bash
aws events disable-rule --name reinhardt-ci-orphan-detector-schedule
```

Option B (via Terraform):

```bash
terraform apply -var='orphan_detector_enabled=false'
```

#### Dry-run mode (shadow traffic)

Useful after a code change to verify scan behavior without republishing:

```bash
aws lambda update-function-configuration \
  --function-name reinhardt-ci-orphan-detector \
  --environment 'Variables={...existing...,DRY_RUN=true}'

# Observe CloudWatch logs for `dry_run.skip_republish` events
# Revert:
aws lambda update-function-configuration \
  --function-name reinhardt-ci-orphan-detector \
  --environment 'Variables={...existing...,DRY_RUN=false}'
```

### Local development

```bash
cd lambda-src/orphan-detector
npm ci
npm test              # run unit tests (vitest)
npm run test:coverage # with coverage thresholds (90% line, 85% branch)
npm run build         # bundle to dist/index.mjs (esbuild)
npm run typecheck     # tsc --noEmit
```

### References

- Issue: [#3903](https://github.com/kent8192/reinhardt-web/issues/3903)
- Companion bug: [#3902](https://github.com/kent8192/reinhardt-web/issues/3902) (job_retry hardening)
- Upstream module: <https://github.com/github-aws-runners/terraform-aws-github-runner>
- GitHub webhooks: <https://docs.github.com/en/webhooks/webhook-events-and-payloads#workflow_job>
