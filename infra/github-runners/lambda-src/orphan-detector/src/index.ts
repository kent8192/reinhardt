import { randomUUID } from "node:crypto";
import { CloudWatchClient } from "@aws-sdk/client-cloudwatch";
import { SNSClient } from "@aws-sdk/client-sns";
import { GetParameterCommand, SSMClient } from "@aws-sdk/client-ssm";
import type { Context } from "aws-lambda";
import pMap from "p-map";
import { publishAlert } from "./alert.js";
import { shouldHalt } from "./circuit-breaker.js";
import { loadConfig } from "./config.js";
import { loadProcessedState, saveProcessedState } from "./dedup.js";
import { filterOrphans } from "./detector.js";
import {
	buildAppOctokit,
	getRateLimitRemaining,
	listQueuedJobs,
} from "./github-client.js";
import { logger } from "./logger.js";
import { emitMetric } from "./metrics.js";
import { buildSyntheticWebhook } from "./payload-builder.js";
import { postSignedWebhook } from "./webhook-signer.js";

const ssm = new SSMClient({});
const cw = new CloudWatchClient({});
const sns = new SNSClient({});

async function getSecret(name: string): Promise<string> {
	const resp = await ssm.send(
		new GetParameterCommand({ Name: name, WithDecryption: true }),
	);
	const v = resp.Parameter?.Value;
	if (!v) throw new Error(`SSM parameter ${name} has no value`);
	return v;
}

export const handler = async (
	_event?: unknown,
	_context?: Context,
): Promise<void> => {
	const scanId = randomUUID();
	const scanStart = Date.now();
	const cfg = loadConfig();
	const repoDim = { Repository: `${cfg.owner}/${cfg.repo}` };

	logger.info({ msg: "scan.start", scanId, stalenessMin: cfg.stalenessMin });

	try {
		// Load secrets — private key is base64-encoded PEM in SSM by convention,
		// but accept raw PEM too for resilience.
		const [privateKeyRaw, webhookSecret] = await Promise.all([
			getSecret(cfg.githubAppKeySSMParam),
			getSecret(cfg.webhookSecretSSMParam),
		]);
		const privateKey = privateKeyRaw.startsWith("-----BEGIN")
			? privateKeyRaw
			: Buffer.from(privateKeyRaw, "base64").toString("utf8");

		const installationId = Number.parseInt(
			process.env.GITHUB_APP_INSTALLATION_ID ?? "0",
			10,
		);
		if (!Number.isFinite(installationId) || installationId <= 0) {
			throw new Error(
				"GITHUB_APP_INSTALLATION_ID must be set to a positive integer",
			);
		}

		// Load dedup state
		const processed = await loadProcessedState(ssm, cfg.ssmDedupParam, Date.now());

		// Fetch queued jobs
		const octokit = await buildAppOctokit({
			appId: cfg.githubAppId,
			privateKey,
			installationId,
		});
		const queuedJobs = await listQueuedJobs(octokit, cfg.owner, cfg.repo);
		const rateLimit = await getRateLimitRemaining(octokit);
		await emitMetric(
			cw,
			cfg.metricNamespace,
			"GitHubApiRateLimitRemaining",
			rateLimit,
		);

		// Filter orphans
		const orphans = filterOrphans(
			queuedJobs,
			Date.now(),
			cfg.stalenessMin,
			processed,
		);
		await emitMetric(
			cw,
			cfg.metricNamespace,
			"OrphanJobsDetected",
			orphans.length,
			repoDim,
		);
		await emitMetric(cw, cfg.metricNamespace, "DedupStateEntries", processed.size);

		// Circuit breaker
		if (shouldHalt(orphans.length, cfg.circuitBreakerMax)) {
			logger.warn({
				msg: "scan.circuit_breaker",
				scanId,
				orphanCount: orphans.length,
				threshold: cfg.circuitBreakerMax,
			});
			await publishAlert(sns, cfg.snsAlertTopicArn, {
				severity: "critical",
				repository: `${cfg.owner}/${cfg.repo}`,
				orphanCount: orphans.length,
				threshold: cfg.circuitBreakerMax,
				thresholdBasis:
					"CIRCUIT_BREAKER_MAX env var (runner_max_count + margin)",
				stalenessMin: cfg.stalenessMin,
				sampleJobIds: orphans.slice(0, 10).map((j) => j.id),
				scanStartTime: new Date(scanStart).toISOString(),
				circuitBreakerReason: "orphan_count_exceeds_threshold",
				recommendedActions: [
					"Check CloudWatch logs: /aws/lambda/reinhardt-ci-orphan-detector",
					"Inspect SQS reinhardt-ci-queued-builds depth and ApproximateAgeOfOldestMessage",
					"Check EC2 Spot capacity: aws ec2 describe-spot-fleet-instances",
					"Manual rescue: gh run cancel + gh run rerun on affected PR",
				],
			});
			await emitMetric(
				cw,
				cfg.metricNamespace,
				"CircuitBreakerTripped",
				1,
				repoDim,
			);
			return;
		}

		// Dry-run: skip republish
		if (cfg.dryRun) {
			logger.info({
				msg: "dry_run.skip_republish",
				scanId,
				wouldRepublish: orphans.length,
			});
			return;
		}

		// Republish orphans (concurrency 5 to avoid DDoS-ing webhook Lambda)
		const results = await pMap(
			orphans,
			async (job) => {
				const { deliveryId, body } = buildSyntheticWebhook({
					job,
					installationId,
					owner: cfg.owner,
					repo: cfg.repo,
				});
				const res = await postSignedWebhook({
					url: cfg.webhookUrl,
					secret: webhookSecret,
					deliveryId,
					bodyObj: body,
					jobId: job.id,
				});
				if (!res.ok) {
					await emitMetric(
						cw,
						cfg.metricNamespace,
						"RepublishFailures",
						1,
						{ ...repoDim, FailureType: classifyStatus(res.status) },
					);
				}
				return res;
			},
			{ concurrency: 5 },
		);

		const success = results.filter((r) => r.ok).length;
		for (const [i, job] of orphans.entries()) {
			if (results[i]?.ok) processed.set(job.id, Date.now());
		}
		await saveProcessedState(ssm, cfg.ssmDedupParam, processed);
		await emitMetric(
			cw,
			cfg.metricNamespace,
			"OrphanJobsRepublished",
			success,
			repoDim,
		);

		const durationMs = Date.now() - scanStart;
		await emitMetric(cw, cfg.metricNamespace, "ScanDurationMs", durationMs);
		logger.info({
			msg: "scan.end",
			scanId,
			republished: success,
			failed: results.length - success,
			durationMs,
		});
	} catch (err) {
		logger.error({ msg: "scan.error", scanId, err: String(err) });
		throw err; // Lambda fails → EventBridge retries next scan
	}
};

function classifyStatus(status: number): string {
	if (status === 0) return "network_or_timeout";
	if (status >= 500) return "server_5xx";
	if (status >= 400) return "client_4xx";
	return "other";
}
