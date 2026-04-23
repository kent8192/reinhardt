import {
	type CloudWatchClient,
	PutMetricDataCommand,
} from "@aws-sdk/client-cloudwatch";
import { logger } from "./logger.js";

export async function emitMetric(
	cw: CloudWatchClient,
	namespace: string,
	metricName: string,
	value: number,
	dims: Record<string, string> = {},
): Promise<void> {
	try {
		await cw.send(
			new PutMetricDataCommand({
				Namespace: namespace,
				MetricData: [
					{
						MetricName: metricName,
						Value: value,
						Unit: guessUnit(metricName),
						Timestamp: new Date(),
						Dimensions: Object.entries(dims).map(([Name, Value]) => ({ Name, Value })),
					},
				],
			}),
		);
	} catch (err) {
		logger.warn({ msg: "metrics.emit_failed", metricName, err: String(err) });
	}
}

function guessUnit(name: string): "Count" | "Milliseconds" | "None" {
	if (name.endsWith("Ms")) return "Milliseconds";
	if (
		name.endsWith("Count") ||
		name.includes("Jobs") ||
		name.endsWith("Tripped") ||
		name.endsWith("Remaining") ||
		name.endsWith("Entries") ||
		name.endsWith("Failures")
	) {
		return "Count";
	}
	return "None";
}
