import { randomUUID } from "node:crypto";

export interface PayloadInput {
	job: {
		id: number;
		run_id: number;
		status: "queued";
		created_at: string;
		labels: string[];
		workflow_name: string;
		name: string;
	};
	installationId: number;
	owner: string;
	repo: string;
}

export interface SyntheticWebhook {
	deliveryId: string; // for X-GitHub-Delivery header
	body: {
		action: "queued";
		workflow_job: {
			id: number;
			run_id: number;
			status: "queued";
			created_at: string;
			labels: string[];
			workflow_name: string;
			name: string;
		};
		repository: {
			name: string;
			full_name: string;
			owner: { login: string };
		};
		installation: { id: number };
	};
}

export function buildSyntheticWebhook(input: PayloadInput): SyntheticWebhook {
	// Ensure 'self-hosted' is always in labels (defense against upstream assumptions).
	const labels = input.job.labels.includes("self-hosted")
		? [...input.job.labels]
		: ["self-hosted", ...input.job.labels];

	return {
		deliveryId: randomUUID(),
		body: {
			action: "queued",
			workflow_job: {
				id: input.job.id,
				run_id: input.job.run_id,
				status: "queued",
				created_at: input.job.created_at,
				labels,
				workflow_name: input.job.workflow_name,
				name: input.job.name,
			},
			repository: {
				name: input.repo,
				full_name: `${input.owner}/${input.repo}`,
				owner: { login: input.owner },
			},
			installation: { id: input.installationId },
		},
	};
}
