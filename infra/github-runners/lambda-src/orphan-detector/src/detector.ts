export interface WorkflowJob {
	id: number;
	run_id: number;
	status: string;
	created_at: string; // ISO 8601 UTC
	labels: string[];
}

// Generic over the job type so callers with richer types (e.g. QueuedJobExtended
// in github-client.ts) retain their extra fields through the filter.
export function filterOrphans<J extends WorkflowJob>(
	jobs: readonly J[],
	nowMs: number,
	stalenessMin: number,
	processed: ReadonlyMap<number, number>,
): J[] {
	const cutoffMs = nowMs - stalenessMin * 60_000;
	return jobs.filter((job) => {
		if (job.status !== "queued") return false;
		if (processed.has(job.id)) return false;
		const createdMs = Date.parse(job.created_at);
		if (Number.isNaN(createdMs)) return false;
		return createdMs < cutoffMs; // strictly less than: exactly-on-threshold is not orphan
	});
}
