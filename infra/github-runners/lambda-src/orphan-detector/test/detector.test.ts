import { describe, expect, it } from "vitest";
import { filterOrphans, type WorkflowJob } from "../src/detector.js";

const mkJob = (overrides: Partial<WorkflowJob> = {}): WorkflowJob => ({
	id: 100,
	status: "queued",
	created_at: "2026-04-22T10:00:00Z",
	labels: ["self-hosted", "reinhardt-ci"],
	run_id: 999,
	...overrides,
});

describe("filterOrphans", () => {
	const now = Date.parse("2026-04-22T11:30:00Z"); // 90 min after 10:00
	const staleness = 60; // minutes

	it("returns job queued > staleness minutes", () => {
		const jobs = [mkJob({ id: 1, created_at: "2026-04-22T10:00:00Z" })];
		expect(filterOrphans(jobs, now, staleness, new Map())).toHaveLength(1);
	});

	it("excludes job queued < staleness minutes", () => {
		const jobs = [mkJob({ id: 2, created_at: "2026-04-22T11:00:00Z" })]; // 30 min ago
		expect(filterOrphans(jobs, now, staleness, new Map())).toHaveLength(0);
	});

	it("excludes jobs with status != queued", () => {
		const jobs = [mkJob({ id: 3, status: "in_progress" })];
		expect(filterOrphans(jobs, now, staleness, new Map())).toHaveLength(0);
	});

	it("excludes jobs already processed (dedup)", () => {
		const jobs = [mkJob({ id: 4 })];
		const processed = new Map<number, number>([[4, now - 1000]]);
		expect(filterOrphans(jobs, now, staleness, processed)).toHaveLength(0);
	});

	it("boundary: exactly staleness minutes (equal) is NOT orphan", () => {
		// created_at = 60 minutes before now, exactly on threshold
		const jobs = [mkJob({ id: 5, created_at: "2026-04-22T10:30:00Z" })];
		expect(filterOrphans(jobs, now, staleness, new Map())).toHaveLength(0);
	});

	it("boundary: staleness + 1 second is orphan", () => {
		const jobs = [mkJob({ id: 6, created_at: "2026-04-22T10:29:59Z" })];
		expect(filterOrphans(jobs, now, staleness, new Map())).toHaveLength(1);
	});

	it("empty input returns empty output", () => {
		expect(filterOrphans([], now, staleness, new Map())).toEqual([]);
	});

	it("handles ISO 8601 UTC with milliseconds", () => {
		const jobs = [mkJob({ id: 7, created_at: "2026-04-22T10:00:00.123Z" })];
		expect(filterOrphans(jobs, now, staleness, new Map())).toHaveLength(1);
	});
});
