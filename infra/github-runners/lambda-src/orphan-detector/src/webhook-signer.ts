import { createHmac } from "node:crypto";
import { request } from "undici";
import { logger } from "./logger.js";

export function computeSignature(secret: string, body: string): string {
	const hmac = createHmac("sha256", secret);
	hmac.update(body, "utf8");
	return `sha256=${hmac.digest("hex")}`;
}

export interface PostResult {
	ok: boolean;
	status: number;
	jobId?: number;
}

/**
 * POST a signed synthetic webhook to the upstream webhook Lambda URL.
 * 3 exponential-backoff retries on 5xx/network errors. 4xx propagates immediately (no retry).
 */
export async function postSignedWebhook(args: {
	url: string;
	secret: string;
	deliveryId: string;
	bodyObj: unknown;
	jobId: number;
	timeoutMs?: number;
}): Promise<PostResult> {
	const { url, secret, deliveryId, bodyObj, jobId, timeoutMs = 10_000 } = args;
	const body = JSON.stringify(bodyObj);
	const signature = computeSignature(secret, body);

	const headers = {
		"Content-Type": "application/json",
		"User-Agent": "reinhardt-ci-orphan-detector/1.0",
		"X-GitHub-Event": "workflow_job",
		"X-GitHub-Delivery": deliveryId,
		"X-Hub-Signature-256": signature,
	};

	const maxAttempts = 3;
	let lastErr: unknown;
	let lastStatus = 0; // Retain final 5xx status for final return

	for (let attempt = 1; attempt <= maxAttempts; attempt++) {
		try {
			const resp = await request(url, {
				method: "POST",
				headers,
				body,
				bodyTimeout: timeoutMs,
				headersTimeout: timeoutMs,
			});
			const status = resp.statusCode;
			// Consume body to free the connection
			await resp.body.text();

			if (status >= 200 && status < 300) {
				return { ok: true, status, jobId };
			}
			if (status >= 400 && status < 500) {
				// Do not retry client errors.
				logger.warn({ msg: "webhook.4xx", jobId, status });
				return { ok: false, status, jobId };
			}
			// 5xx: retry
			lastStatus = status;
			logger.warn({ msg: "webhook.5xx_retry", jobId, status, attempt });
		} catch (err) {
			lastErr = err;
			logger.warn({ msg: "webhook.network_retry", jobId, attempt, err: String(err) });
		}
		if (attempt < maxAttempts) {
			await sleep(100 * 2 ** (attempt - 1)); // 100ms, 200ms
		}
	}
	logger.error({ msg: "webhook.giving_up", jobId, status: lastStatus, err: String(lastErr) });
	return { ok: false, status: lastStatus, jobId };
}

function sleep(ms: number): Promise<void> {
	return new Promise((r) => setTimeout(r, ms));
}
