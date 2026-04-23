import { PublishCommand, type SNSClient } from "@aws-sdk/client-sns";
import { logger } from "./logger.js";

export interface AlertBody {
	severity: "critical";
	repository: string;
	orphanCount: number;
	threshold: number;
	thresholdBasis: string;
	stalenessMin: number;
	sampleJobIds: number[];
	scanStartTime: string;
	circuitBreakerReason: string;
	recommendedActions: string[];
}

export async function publishAlert(
	sns: SNSClient,
	topicArn: string,
	body: AlertBody,
): Promise<void> {
	try {
		await sns.send(
			new PublishCommand({
				TopicArn: topicArn,
				Subject: `[reinhardt-ci][CRITICAL] Orphan detector circuit breaker tripped (${body.orphanCount} orphans)`,
				Message: JSON.stringify(body, null, 2),
			}),
		);
	} catch (err) {
		logger.error({ msg: "alert.publish_failed", err: String(err) });
	}
}
