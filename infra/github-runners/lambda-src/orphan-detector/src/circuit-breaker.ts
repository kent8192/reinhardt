// Returns true when orphanCount > threshold (strictly greater).
// Rationale: threshold = runner_max_count + margin, so equality is not yet
// "systemic failure" territory — legitimate CI bursts may sit at threshold.
export function shouldHalt(orphanCount: number, threshold: number): boolean {
	return orphanCount > threshold;
}
