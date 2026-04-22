import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { http, HttpResponse } from "msw";
import { setupServer } from "msw/node";
import { Octokit } from "@octokit/rest";
import { getRateLimitRemaining, listQueuedJobs } from "../src/github-client.js";

const server = setupServer();

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

describe("listQueuedJobs", () => {
	it("returns empty when no queued runs", async () => {
		server.use(
			http.get("https://api.github.com/repos/o/r/actions/runs", () =>
				HttpResponse.json({
					total_count: 0,
					workflow_runs: [],
				}),
			),
		);
		const octokit = new Octokit({ auth: "test-token" });
		const jobs = await listQueuedJobs(octokit, "o", "r");
		expect(jobs).toEqual([]);
	});

	it("fetches jobs for each queued run and returns queued jobs only", async () => {
		server.use(
			http.get("https://api.github.com/repos/o/r/actions/runs", () =>
				HttpResponse.json({
					total_count: 1,
					workflow_runs: [{ id: 999, status: "queued" }],
				}),
			),
			http.get("https://api.github.com/repos/o/r/actions/runs/999/jobs", () =>
				HttpResponse.json({
					total_count: 2,
					jobs: [
						{
							id: 1,
							run_id: 999,
							status: "queued",
							created_at: "2026-04-22T10:00:00Z",
							labels: ["self-hosted", "reinhardt-ci"],
							workflow_name: "CI",
							name: "Feature Check",
						},
						{
							id: 2,
							run_id: 999,
							status: "in_progress",
							created_at: "2026-04-22T10:00:00Z",
							labels: ["self-hosted"],
							workflow_name: "CI",
							name: "Test",
						},
					],
				}),
			),
		);
		const octokit = new Octokit({ auth: "test-token" });
		const jobs = await listQueuedJobs(octokit, "o", "r");
		expect(jobs.map((j) => j.id)).toEqual([1]); // only queued, not in_progress
	});

	it("getRateLimitRemaining returns remaining count", async () => {
		server.use(
			http.get("https://api.github.com/rate_limit", () =>
				HttpResponse.json({ rate: { limit: 5000, remaining: 4321, reset: 123 } }),
			),
		);
		const octokit = new Octokit({ auth: "test-token" });
		const remaining = await getRateLimitRemaining(octokit);
		expect(remaining).toBe(4321);
	});

	it("getRateLimitRemaining returns -1 on error", async () => {
		server.use(
			http.get("https://api.github.com/rate_limit", () =>
				HttpResponse.error(),
			),
		);
		const octokit = new Octokit({ auth: "test-token" });
		const remaining = await getRateLimitRemaining(octokit);
		expect(remaining).toBe(-1);
	});

	it("handles pagination via Link header", async () => {
		let callCount = 0;
		server.use(
			http.get("https://api.github.com/repos/o/r/actions/runs", () => {
				callCount++;
				if (callCount === 1) {
					return HttpResponse.json(
						{
							total_count: 2,
							workflow_runs: [{ id: 1, status: "queued" }],
						},
						{
							headers: {
								Link: '<https://api.github.com/repos/o/r/actions/runs?page=2>; rel="next"',
							},
						},
					);
				}
				return HttpResponse.json({
					total_count: 2,
					workflow_runs: [{ id: 2, status: "queued" }],
				});
			}),
			http.get("https://api.github.com/repos/o/r/actions/runs/:runId/jobs", () =>
				HttpResponse.json({ total_count: 0, jobs: [] }),
			),
		);
		const octokit = new Octokit({ auth: "test-token" });
		await listQueuedJobs(octokit, "o", "r");
		expect(callCount).toBeGreaterThanOrEqual(2);
	});
});
