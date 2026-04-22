import {
	GetParameterCommand,
	PutParameterCommand,
	type SSMClient,
} from "@aws-sdk/client-ssm";
import { logger } from "./logger.js";

export const TTL_MS = 2 * 60 * 60 * 1000; // 2 hours

/**
 * Load processed state from SSM, drop entries older than TTL.
 * Fail-open: returns empty Map on any error (favoring detection over dedup).
 */
export async function loadProcessedState(
	ssm: SSMClient,
	paramName: string,
	nowMs: number,
): Promise<Map<number, number>> {
	try {
		const resp = await ssm.send(new GetParameterCommand({ Name: paramName }));
		const value = resp.Parameter?.Value ?? "{}";
		const parsed = JSON.parse(value) as Record<string, number>;
		const cutoff = nowMs - TTL_MS;
		const map = new Map<number, number>();
		for (const [k, v] of Object.entries(parsed)) {
			if (typeof v !== "number") continue;
			if (v < cutoff) continue;
			const id = Number.parseInt(k, 10);
			if (!Number.isFinite(id)) continue;
			map.set(id, v);
		}
		return map;
	} catch (err) {
		logger.warn({ msg: "dedup.load_failed", err: String(err) });
		return new Map();
	}
}

/**
 * Save processed state to SSM. Log-only on failure (next scan will retry).
 */
export async function saveProcessedState(
	ssm: SSMClient,
	paramName: string,
	state: ReadonlyMap<number, number>,
): Promise<void> {
	try {
		const obj: Record<string, number> = {};
		for (const [k, v] of state) {
			obj[String(k)] = v;
		}
		const value = JSON.stringify(obj);
		await ssm.send(
			new PutParameterCommand({
				Name: paramName,
				Value: value,
				Overwrite: true,
				Type: "String",
			}),
		);
	} catch (err) {
		logger.warn({ msg: "dedup.save_failed", err: String(err) });
	}
}
