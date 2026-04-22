# CI stuck-queue alerting.
#
# Surfaces scale-up queue stagnation to operators before job-retry exhausts.
# Context: kent8192/reinhardt-web#3902 documented 8 jobs stranded on PR #3901
# when JIT runners silently failed to pick up assignments. By the time the
# retry window closed (~75min), the jobs were unrecoverable without a manual
# `gh run rerun`. This alarm fires when the oldest message in the scale-up
# SQS queue has been waiting longer than 30 minutes, giving operators a
# ~45-minute window (with the raised max_attempts=10) to intervene.

resource "aws_sns_topic" "ci_alerts" {
  name = "${var.prefix}-ci-alerts"
}

resource "aws_sns_topic_subscription" "ci_alerts_email" {
  topic_arn = aws_sns_topic.ci_alerts.arn
  protocol  = "email"
  endpoint  = var.budget_alert_email
}

# ApproximateAgeOfOldestMessage rises when job_retry republishes a stuck job
# to the scale-up queue and scale-up cannot successfully launch a runner that
# picks up the assignment. Sustained elevation past 30 min indicates the
# retry loop is not converging, typically from JIT-runner assignment races
# or Spot capacity exhaustion.
resource "aws_cloudwatch_metric_alarm" "stuck_queued_builds" {
  alarm_name        = "${var.prefix}-stuck-queued-builds"
  alarm_description = "Scale-up SQS queue has a message older than 30 minutes. Indicates CI runner-assignment failure; investigate scale-up and job-retry Lambda logs. See kent8192/reinhardt-web#3902."

  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 3
  metric_name         = "ApproximateAgeOfOldestMessage"
  namespace           = "AWS/SQS"
  period              = 300
  statistic           = "Maximum"
  threshold           = 1800

  dimensions = {
    QueueName = "${var.prefix}-queued-builds"
  }

  # Treat missing data as OK: queue is legitimately empty most of the time.
  treat_missing_data = "notBreaching"

  alarm_actions = [aws_sns_topic.ci_alerts.arn]
  ok_actions    = [aws_sns_topic.ci_alerts.arn]
}

# Secondary alarm on job-retry queue: if messages accumulate here, job_retry
# Lambda itself is failing to drain (e.g. GitHub API rate limit, auth error).
resource "aws_cloudwatch_metric_alarm" "stuck_job_retry" {
  alarm_name        = "${var.prefix}-stuck-job-retry"
  alarm_description = "Job-retry SQS queue has a message older than 30 minutes. Indicates job-retry Lambda is not draining; check /aws/lambda/${var.prefix}-job-retry logs. See kent8192/reinhardt-web#3902."

  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 3
  metric_name         = "ApproximateAgeOfOldestMessage"
  namespace           = "AWS/SQS"
  period              = 300
  statistic           = "Maximum"
  threshold           = 1800

  dimensions = {
    QueueName = "${var.prefix}-job-retry"
  }

  treat_missing_data = "notBreaching"

  alarm_actions = [aws_sns_topic.ci_alerts.arn]
  ok_actions    = [aws_sns_topic.ci_alerts.arn]
}
