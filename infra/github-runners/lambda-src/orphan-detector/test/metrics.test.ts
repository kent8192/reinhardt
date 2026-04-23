import { beforeEach, describe, expect, it, vi } from "vitest";
import {
	type CloudWatchClient,
	PutMetricDataCommand,
} from "@aws-sdk/client-cloudwatch";
import { emitMetric } from "../src/metrics.js";

describe("emitMetric", () => {
	const cw = { send: vi.fn() } as unknown as CloudWatchClient;

	beforeEach(() => {
		vi.clearAllMocks();
	});

	it("sends PutMetricData with correct namespace and metric name", async () => {
		(cw.send as ReturnType<typeof vi.fn>).mockResolvedValueOnce({});
		await emitMetric(cw, "TestNamespace", "OrphanJobsDetected", 3, {
			Repository: "kent8192/reinhardt-web",
		});
		const cmd = (cw.send as ReturnType<typeof vi.fn>).mock.calls[0]?.[0] as PutMetricDataCommand;
		expect(cmd.input.Namespace).toBe("TestNamespace");
		expect(cmd.input.MetricData?.[0]?.MetricName).toBe("OrphanJobsDetected");
		expect(cmd.input.MetricData?.[0]?.Value).toBe(3);
		expect(cmd.input.MetricData?.[0]?.Dimensions).toEqual([
			{ Name: "Repository", Value: "kent8192/reinhardt-web" },
		]);
	});

	it("omits Dimensions when dims arg is empty", async () => {
		(cw.send as ReturnType<typeof vi.fn>).mockResolvedValueOnce({});
		await emitMetric(cw, "NS", "Metric", 1);
		const cmd = (cw.send as ReturnType<typeof vi.fn>).mock.calls[0]?.[0] as PutMetricDataCommand;
		expect(cmd.input.MetricData?.[0]?.Dimensions).toEqual([]);
	});

	it("does not throw when CloudWatch send fails (log-only)", async () => {
		(cw.send as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
			new Error("ServiceException"),
		);
		await expect(emitMetric(cw, "N", "M", 1)).resolves.toBeUndefined();
	});
});
