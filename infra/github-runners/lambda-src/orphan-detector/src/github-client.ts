import { createAppAuth } from "@octokit/auth-app";
import { retry } from "@octokit/plugin-retry";
import { throttling } from "@octokit/plugin-throttling";
import { Octokit } from "@octokit/rest";
import type { WorkflowJob } from "./detector.js";
import { logger } from "./logger.js";

const OctokitWithPlugins = Octokit.plugin(retry, throttling);

export async function buildAppOctokit(args: {
	appId: string;
	privateKey: string;
	installationId: number;
}): Promise<Octokit> {
	return new OctokitWithPlugins({
		authStrategy: createAppAuth,
		auth: {
			appId: args.appId,
			privateKey: args.privateKey,
			installationId: args.installationId,
		},
		throttle: {
			onRateLimit: (retryAfter, _options, _octokit, retryCount) => {
				logger.warn({ msg: "github.rate_limit", retryAfter, retryCount });
				return retryCount < 2;
			},
			onSecondaryRateLimit: (retryAfter, _options, _octokit) => {
				logger.warn({ msg: "github.secondary_rate_limit", retryAfter });
				return true;
			},
		},
		retry: { doNotRetry: ["400", "401", "403", "404", "422"] },
	});
}

export interface QueuedJobExtended extends WorkflowJob {
	// Narrow status to the literal 'queued' since listQueuedJobs filters on it.
	status: "queued";
	workflow_name: string;
	name: string;
}

export async function listQueuedJobs(
	octokit: Octokit,
	owner: string,
	repo: string,
): Promise<QueuedJobExtended[]> {
	// 1. List all queued runs (paginated)
	const runsIter = octokit.paginate.iterator(
		octokit.rest.actions.listWorkflowRunsForRepo,
		{ owner, repo, status: "queued", per_page: 100 },
	);

	const queuedJobs: QueuedJobExtended[] = [];

	for await (const { data: runs } of runsIter) {
		for (const run of runs) {
			// 2. Fetch jobs for each queued run (paginated)
			const jobsIter = octokit.paginate.iterator(
				octokit.rest.actions.listJobsForWorkflowRun,
				{ owner, repo, run_id: run.id, per_page: 100 },
			);
			for await (const { data: jobs } of jobsIter) {
				for (const job of jobs) {
					if (job.status !== "queued") continue;
					queuedJobs.push({
						id: job.id,
						run_id: job.run_id,
						status: "queued",
						created_at: job.created_at,
						labels: job.labels ?? [],
						workflow_name: job.workflow_name ?? "",
						name: job.name,
					});
				}
			}
		}
	}
	return queuedJobs;
}

export function getRateLimitRemaining(octokit: Octokit): Promise<number> {
	return octokit.rest.rateLimit
		.get()
		.then((r) => r.data.rate.remaining)
		.catch(() => -1);
}
