import { beforeEach, describe, expect, it, vi } from "vitest";
import { PublishCommand, type SNSClient } from "@aws-sdk/client-sns";
import { type AlertBody, publishAlert } from "../src/alert.js";

describe("publishAlert", () => {
	const sns = { send: vi.fn() } as unknown as SNSClient;
	const topicArn = "arn:aws:sns:us-east-1:1:t";
	const body: AlertBody = {
		severity: "critical",
		repository: "o/r",
		orphanCount: 60,
		threshold: 52,
		thresholdBasis: "runner_max_count + margin",
		stalenessMin: 60,
		sampleJobIds: [1, 2, 3],
		scanStartTime: "2026-04-22T00:00:00Z",
		circuitBreakerReason: "orphan_count_exceeds_threshold",
		recommendedActions: ["a", "b"],
	};

	beforeEach(() => {
		vi.clearAllMocks();
	});

	it("publishes with CRITICAL subject and serialized body", async () => {
		(sns.send as ReturnType<typeof vi.fn>).mockResolvedValueOnce({});
		await publishAlert(sns, topicArn, body);
		const cmd = (sns.send as ReturnType<typeof vi.fn>).mock.calls[0]?.[0] as PublishCommand;
		expect(cmd).toBeInstanceOf(PublishCommand);
		expect(cmd.input.TopicArn).toBe(topicArn);
		expect(cmd.input.Subject).toContain("[CRITICAL]");
		expect(cmd.input.Subject).toContain("60 orphans");
		const parsed = JSON.parse(cmd.input.Message!);
		expect(parsed.orphanCount).toBe(60);
	});

	it("does not throw on SNS failure (log-only)", async () => {
		(sns.send as ReturnType<typeof vi.fn>).mockRejectedValueOnce(new Error("ServiceUnavailable"));
		await expect(publishAlert(sns, topicArn, body)).resolves.toBeUndefined();
	});
});
