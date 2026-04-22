import { beforeEach, describe, expect, it, vi } from "vitest";

// Mock all dependencies before importing the handler.
vi.mock("../src/github-client.js", () => ({
	buildAppOctokit: vi.fn().mockResolvedValue({ rest: {} }),
	listQueuedJobs: vi.fn(),
	getRateLimitRemaining: vi.fn().mockResolvedValue(4500),
}));
vi.mock("../src/dedup.js", () => ({
	// Use mockImplementation to return a FRESH Map on every call, avoiding
	// state leaks across tests.
	loadProcessedState: vi.fn().mockImplementation(async () => new Map()),
	saveProcessedState: vi.fn().mockResolvedValue(undefined),
	TTL_MS: 2 * 60 * 60 * 1000,
}));
vi.mock("../src/webhook-signer.js", () => ({
	postSignedWebhook: vi.fn(),
}));
vi.mock("../src/metrics.js", () => ({
	emitMetric: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("../src/alert.js", () => ({
	publishAlert: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("@aws-sdk/client-ssm", () => ({
	GetParameterCommand: vi.fn((args) => ({ input: args })),
	SSMClient: vi.fn().mockImplementation(() => ({
		send: vi.fn().mockResolvedValue({
			Parameter: {
				Value: "-----BEGIN RSA PRIVATE KEY-----\nfake\n-----END RSA PRIVATE KEY-----",
			},
		}),
	})),
}));
vi.mock("@aws-sdk/client-cloudwatch", () => ({
	CloudWatchClient: vi.fn().mockImplementation(() => ({})),
}));
vi.mock("@aws-sdk/client-sns", () => ({
	SNSClient: vi.fn().mockImplementation(() => ({})),
}));

import { handler } from "../src/index.js";
import { listQueuedJobs } from "../src/github-client.js";
import { postSignedWebhook } from "../src/webhook-signer.js";
import { emitMetric } from "../src/metrics.js";
import { publishAlert } from "../src/alert.js";
import { saveProcessedState } from "../src/dedup.js";

const setEnv = () => {
	process.env.GITHUB_APP_ID = "1";
	process.env.GITHUB_APP_KEY_SSM_PARAM = "/k";
	process.env.GITHUB_OWNER = "o";
	process.env.GITHUB_REPO = "r";
	process.env.WEBHOOK_URL = "https://w/";
	process.env.WEBHOOK_SECRET_SSM_PARAM = "/s";
	process.env.STALENESS_MIN = "60";
	process.env.CIRCUIT_BREAKER_MAX = "52";
	process.env.SSM_DEDUP_PARAM = "/d";
	process.env.SNS_ALERT_TOPIC_ARN = "arn:aws:sns:us-east-1:1:t";
	process.env.METRIC_NAMESPACE = "NS";
	process.env.GITHUB_APP_INSTALLATION_ID = "112540205";
	delete process.env.DRY_RUN;
};

const mkJob = (id: number, minutesAgo: number) => ({
	id,
	run_id: 999,
	status: "queued" as const,
	created_at: new Date(Date.now() - minutesAgo * 60_000).toISOString(),
	labels: ["self-hosted", "reinhardt-ci"],
	workflow_name: "CI",
	name: `Job ${id}`,
});

describe("handler", () => {
	beforeEach(() => {
		vi.clearAllMocks();
		setEnv();
	});

	it("happy path: detects 3 orphans, republishes all, updates dedup", async () => {
		(listQueuedJobs as ReturnType<typeof vi.fn>).mockResolvedValueOnce([
			mkJob(1, 90),
			mkJob(2, 90),
			mkJob(3, 90),
		]);
		(postSignedWebhook as ReturnType<typeof vi.fn>).mockResolvedValue({
			ok: true,
			status: 202,
		});

		await handler();

		expect(postSignedWebhook).toHaveBeenCalledTimes(3);
		expect(emitMetric).toHaveBeenCalledWith(
			expect.anything(),
			"NS",
			"OrphanJobsDetected",
			3,
			expect.objectContaining({ Repository: "o/r" }),
		);
		expect(emitMetric).toHaveBeenCalledWith(
			expect.anything(),
			"NS",
			"OrphanJobsRepublished",
			3,
			expect.anything(),
		);
		expect(saveProcessedState).toHaveBeenCalledTimes(1);
		expect(publishAlert).not.toHaveBeenCalled();
	});

	it("trips circuit breaker when orphan count > threshold", async () => {
		const manyJobs = Array.from({ length: 60 }, (_, i) => mkJob(i + 1, 90));
		(listQueuedJobs as ReturnType<typeof vi.fn>).mockResolvedValueOnce(manyJobs);

		await handler();

		expect(publishAlert).toHaveBeenCalledTimes(1);
		const alertBody = (publishAlert as ReturnType<typeof vi.fn>).mock.calls[0]?.[2];
		expect(alertBody.orphanCount).toBe(60);
		expect(alertBody.sampleJobIds.length).toBeLessThanOrEqual(10);
		expect(postSignedWebhook).not.toHaveBeenCalled();
	});

	it("dry-run mode skips webhook POST but still emits metrics", async () => {
		process.env.DRY_RUN = "true";
		(listQueuedJobs as ReturnType<typeof vi.fn>).mockResolvedValueOnce([
			mkJob(1, 90),
		]);

		await handler();

		expect(postSignedWebhook).not.toHaveBeenCalled();
		expect(emitMetric).toHaveBeenCalledWith(
			expect.anything(),
			"NS",
			"OrphanJobsDetected",
			1,
			expect.anything(),
		);
		delete process.env.DRY_RUN;
	});

	it("partial failure: only successful republishes enter dedup state", async () => {
		(listQueuedJobs as ReturnType<typeof vi.fn>).mockResolvedValueOnce([
			mkJob(1, 90),
			mkJob(2, 90),
		]);
		(postSignedWebhook as ReturnType<typeof vi.fn>)
			.mockResolvedValueOnce({ ok: true, status: 202 })
			.mockResolvedValueOnce({ ok: false, status: 500 });

		await handler();

		const savedMap = (saveProcessedState as ReturnType<typeof vi.fn>).mock.calls[0]?.[2] as Map<
			number,
			number
		>;
		expect(savedMap.has(1)).toBe(true);
		expect(savedMap.has(2)).toBe(false);
	});

	it("skips under-staleness jobs", async () => {
		(listQueuedJobs as ReturnType<typeof vi.fn>).mockResolvedValueOnce([
			mkJob(1, 10), // 10 min < 60 min threshold
		]);

		await handler();

		expect(postSignedWebhook).not.toHaveBeenCalled();
		expect(emitMetric).toHaveBeenCalledWith(
			expect.anything(),
			"NS",
			"OrphanJobsDetected",
			0,
			expect.anything(),
		);
	});
});
