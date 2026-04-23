# ==============================================================
# Orphan Job Detector: scheduled Lambda scanning for stranded
# GitHub Actions jobs. Fixes #3903.
#
# Build prerequisite: `dist/index.mjs` must exist before
# `terraform apply`. `init.sh` handles this (npm ci + npm run build);
# CI pipelines should invoke it or the equivalent npm steps first.
# ==============================================================

locals {
  orphan_detector_enabled = var.orphan_detector_enabled ? 1 : 0
  orphan_detector_src_dir = "${path.module}/lambda-src/orphan-detector"
  orphan_detector_dist    = "${path.module}/lambda-src/orphan-detector/dist"
  orphan_detector_repo_dim = {
    Repository = "${var.github_owner}/${var.github_repository}"
  }
}

# --- Lambda zip (built out-of-band by init.sh) -----------------
data "archive_file" "orphan_detector" {
  count       = local.orphan_detector_enabled
  type        = "zip"
  source_dir  = local.orphan_detector_dist
  output_path = "${path.module}/lambdas/orphan-detector.zip"
}

# --- Dedup state (SSM) -----------------------------------------
resource "aws_ssm_parameter" "orphan_detector_processed" {
  count       = local.orphan_detector_enabled
  name        = "/${var.prefix}/orphan-detector/processed"
  type        = "String"
  value       = "{}"
  description = "Orphan detector dedup state: job_id -> republished_at (ms since epoch)"

  lifecycle {
    ignore_changes = [value]
  }
}

# --- SNS subscription (reuses shared ci_alerts topic from alerting.tf) -------
# The orphan detector publishes to the existing aws_sns_topic.ci_alerts topic
# created by alerting.tf (PR #3902), keeping all CI infra alerts on one channel.
# If orphan_detector_alert_email is set AND differs from budget_alert_email,
# register it as an additional subscriber to the shared topic.
resource "aws_sns_topic_subscription" "orphan_detector_extra_email" {
  count     = local.orphan_detector_enabled > 0 && length(var.orphan_detector_alert_email) > 0 && var.orphan_detector_alert_email != var.budget_alert_email ? 1 : 0
  topic_arn = aws_sns_topic.ci_alerts.arn
  protocol  = "email"
  endpoint  = var.orphan_detector_alert_email
}

# --- IAM role and policy ---------------------------------------
data "aws_iam_policy_document" "orphan_detector_assume" {
  count = local.orphan_detector_enabled
  statement {
    actions = ["sts:AssumeRole"]
    principals {
      type        = "Service"
      identifiers = ["lambda.amazonaws.com"]
    }
  }
}

resource "aws_iam_role" "orphan_detector" {
  count              = local.orphan_detector_enabled
  name               = "${var.prefix}-orphan-detector-role"
  assume_role_policy = data.aws_iam_policy_document.orphan_detector_assume[0].json
}

data "aws_iam_policy_document" "orphan_detector" {
  count = local.orphan_detector_enabled

  statement {
    sid       = "CloudWatchLogs"
    actions   = ["logs:CreateLogStream", "logs:PutLogEvents"]
    resources = ["${aws_cloudwatch_log_group.orphan_detector[0].arn}:*"]
  }

  # GitHub App key (base64 PEM) + webhook secret, read-only.
  # ARNs come from the upstream module outputs (stable across upgrades).
  statement {
    sid     = "ReadGitHubSecrets"
    actions = ["ssm:GetParameter", "ssm:GetParameters"]
    resources = [
      module.github_runner.ssm_parameters["key_base64"].arn,
      module.github_runner.ssm_parameters["webhook_secret"].arn,
    ]
  }

  statement {
    sid       = "DedupState"
    actions   = ["ssm:GetParameter", "ssm:PutParameter"]
    resources = [aws_ssm_parameter.orphan_detector_processed[0].arn]
  }

  statement {
    sid       = "PublishAlerts"
    actions   = ["sns:Publish"]
    resources = [aws_sns_topic.ci_alerts.arn]
  }

  # PutMetricData does not support resource-level permissions; scope by namespace.
  statement {
    sid       = "PutMetrics"
    actions   = ["cloudwatch:PutMetricData"]
    resources = ["*"]
    condition {
      test     = "StringEquals"
      variable = "cloudwatch:namespace"
      values   = ["ReinhardtCI/OrphanDetector"]
    }
  }

  statement {
    sid = "XRayTracing"
    actions = [
      "xray:PutTraceSegments",
      "xray:PutTelemetryRecords",
    ]
    resources = ["*"]
  }
}

resource "aws_iam_role_policy" "orphan_detector" {
  count  = local.orphan_detector_enabled
  name   = "orphan-detector-policy"
  role   = aws_iam_role.orphan_detector[0].id
  policy = data.aws_iam_policy_document.orphan_detector[0].json
}

# --- Lambda function -------------------------------------------
# NOTE on KMS encryption: log group uses AWS-managed keys (default).
# A customer-managed KMS key is NOT warranted here because:
# 1. This is a dedicated CI sub-account (account 495680546359) with no
#    customer data; logs contain only GitHub job IDs and operational state.
# 2. The upstream github-aws-runners module's own log groups also use
#    AWS-managed encryption; matching that convention keeps ops simple.
# 3. A CMK would add $1/month per key + per-request costs for no
#    meaningful security improvement in this threat model.
# nosemgrep: terraform.aws.security.aws-cloudwatch-log-group-unencrypted
resource "aws_cloudwatch_log_group" "orphan_detector" {
  count             = local.orphan_detector_enabled
  name              = "/aws/lambda/${var.prefix}-orphan-detector"
  retention_in_days = 14
}

# NOTE on KMS encryption: Lambda environment uses AWS-managed key (default).
# A customer-managed KMS key is NOT warranted here because:
# 1. No secrets are stored in environment variables — the GitHub App
#    private key and webhook secret are fetched from SSM SecureString at
#    runtime and held only in process memory.
# 2. The env vars contain only SSM parameter *names* and resource ARNs,
#    not secret values.
# 3. IAM policy restricts Lambda config updates to account administrators,
#    so the env var contents cannot be exfiltrated by non-admins.
# nosemgrep: terraform.aws.security.aws-lambda-environment-unencrypted
resource "aws_lambda_function" "orphan_detector" {
  count            = local.orphan_detector_enabled
  function_name    = "${var.prefix}-orphan-detector"
  role             = aws_iam_role.orphan_detector[0].arn
  runtime          = "nodejs20.x"
  architectures    = ["arm64"]
  handler          = "index.handler"
  memory_size      = 256
  timeout          = 60
  filename         = data.archive_file.orphan_detector[0].output_path
  source_code_hash = data.archive_file.orphan_detector[0].output_base64sha256

  # Unreserved concurrency: the CI sub-account quota is only 10 total,
  # shared with other Lambdas (webhook, scale-up, etc.).
  reserved_concurrent_executions = -1

  environment {
    variables = {
      GITHUB_APP_ID              = var.github_app_id
      GITHUB_APP_INSTALLATION_ID = var.github_app_installation_id
      GITHUB_APP_KEY_SSM_PARAM   = module.github_runner.ssm_parameters["key_base64"].name
      GITHUB_OWNER               = var.github_owner
      GITHUB_REPO                = var.github_repository
      WEBHOOK_URL                = module.github_runner.webhook.endpoint
      WEBHOOK_SECRET_SSM_PARAM   = module.github_runner.ssm_parameters["webhook_secret"].name
      STALENESS_MIN              = tostring(var.orphan_detector_staleness_min)
      CIRCUIT_BREAKER_MAX        = tostring(var.runner_max_count + var.orphan_detector_circuit_breaker_margin)
      SSM_DEDUP_PARAM            = aws_ssm_parameter.orphan_detector_processed[0].name
      SNS_ALERT_TOPIC_ARN        = aws_sns_topic.ci_alerts.arn
      METRIC_NAMESPACE           = "ReinhardtCI/OrphanDetector"
      LOG_LEVEL                  = "info"
    }
  }

  tracing_config {
    mode = "Active"
  }

  depends_on = [aws_cloudwatch_log_group.orphan_detector]
}

# --- EventBridge schedule --------------------------------------
resource "aws_cloudwatch_event_rule" "orphan_detector" {
  count               = local.orphan_detector_enabled
  name                = "${var.prefix}-orphan-detector-schedule"
  description         = "Scheduled scan for orphaned GitHub Actions jobs (> ${var.orphan_detector_staleness_min}min queued)"
  schedule_expression = var.orphan_detector_schedule_expression
}

resource "aws_cloudwatch_event_target" "orphan_detector" {
  count     = local.orphan_detector_enabled
  rule      = aws_cloudwatch_event_rule.orphan_detector[0].name
  target_id = "lambda"
  arn       = aws_lambda_function.orphan_detector[0].arn
}

resource "aws_lambda_permission" "orphan_detector_eventbridge" {
  count         = local.orphan_detector_enabled
  statement_id  = "AllowEventBridgeInvoke"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.orphan_detector[0].function_name
  principal     = "events.amazonaws.com"
  source_arn    = aws_cloudwatch_event_rule.orphan_detector[0].arn
}

# --- CloudWatch alarms ----------------------------------------
resource "aws_cloudwatch_metric_alarm" "orphan_detector_circuit_breaker" {
  count               = local.orphan_detector_enabled
  alarm_name          = "${var.prefix}-orphan-detector-circuit-breaker"
  comparison_operator = "GreaterThanOrEqualToThreshold"
  evaluation_periods  = 1
  metric_name         = "CircuitBreakerTripped"
  namespace           = "ReinhardtCI/OrphanDetector"
  period              = 600
  statistic           = "Sum"
  threshold           = 1
  treat_missing_data  = "notBreaching"
  alarm_actions       = [aws_sns_topic.ci_alerts.arn]
  alarm_description   = "Orphan detector circuit breaker tripped: orphan count exceeded ${var.runner_max_count + var.orphan_detector_circuit_breaker_margin}."
  dimensions          = local.orphan_detector_repo_dim
}

resource "aws_cloudwatch_metric_alarm" "orphan_detector_persistent" {
  count               = local.orphan_detector_enabled
  alarm_name          = "${var.prefix}-orphan-detector-persistent"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 3
  metric_name         = "OrphanJobsDetected"
  namespace           = "ReinhardtCI/OrphanDetector"
  period              = 600
  statistic           = "Maximum"
  threshold           = 0
  treat_missing_data  = "notBreaching"
  alarm_actions       = [aws_sns_topic.ci_alerts.arn]
  alarm_description   = "Orphan jobs detected in 3 consecutive scans - republish is not resolving the issue."
  dimensions          = local.orphan_detector_repo_dim
}

resource "aws_cloudwatch_metric_alarm" "orphan_detector_lambda_errors" {
  count               = local.orphan_detector_enabled
  alarm_name          = "${var.prefix}-orphan-detector-lambda-errors"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 2
  metric_name         = "Errors"
  namespace           = "AWS/Lambda"
  period              = 600
  statistic           = "Sum"
  threshold           = 0
  treat_missing_data  = "notBreaching"
  alarm_actions       = [aws_sns_topic.ci_alerts.arn]
  alarm_description   = "Orphan detector Lambda failed 2+ consecutive runs - scanning halted."
  dimensions = {
    FunctionName = aws_lambda_function.orphan_detector[0].function_name
  }
}

resource "aws_cloudwatch_metric_alarm" "orphan_detector_rate_limit_low" {
  count               = local.orphan_detector_enabled
  alarm_name          = "${var.prefix}-orphan-detector-rate-limit-low"
  comparison_operator = "LessThanThreshold"
  evaluation_periods  = 2
  metric_name         = "GitHubApiRateLimitRemaining"
  namespace           = "ReinhardtCI/OrphanDetector"
  period              = 600
  statistic           = "Minimum"
  threshold           = 500
  treat_missing_data  = "notBreaching"
  alarm_actions       = [aws_sns_topic.ci_alerts.arn]
  alarm_description   = "GitHub App API rate limit low (< 500 remaining) - detector at risk of 429 throttling."
}
