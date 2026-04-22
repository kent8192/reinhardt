export interface Config {
	githubAppId: string;
	githubAppKeySSMParam: string;
	owner: string;
	repo: string;
	webhookUrl: string;
	webhookSecretSSMParam: string;
	stalenessMin: number;
	circuitBreakerMax: number;
	ssmDedupParam: string;
	snsAlertTopicArn: string;
	metricNamespace: string;
	dryRun: boolean;
}

const requiredString = (key: string): string => {
	const v = process.env[key];
	if (!v || v.length === 0) {
		throw new Error(`Required env var missing: ${key}`);
	}
	return v;
};

const requiredNonNegativeInt = (key: string): number => {
	const raw = requiredString(key);
	const n = Number.parseInt(raw, 10);
	if (!Number.isFinite(n) || n < 0 || String(n) !== raw.trim()) {
		throw new Error(`Env var ${key} must be a non-negative integer, got: ${raw}`);
	}
	return n;
};

export const loadConfig = (): Config => ({
	githubAppId: requiredString("GITHUB_APP_ID"),
	githubAppKeySSMParam: requiredString("GITHUB_APP_KEY_SSM_PARAM"),
	owner: requiredString("GITHUB_OWNER"),
	repo: requiredString("GITHUB_REPO"),
	webhookUrl: requiredString("WEBHOOK_URL"),
	webhookSecretSSMParam: requiredString("WEBHOOK_SECRET_SSM_PARAM"),
	stalenessMin: requiredNonNegativeInt("STALENESS_MIN"),
	circuitBreakerMax: requiredNonNegativeInt("CIRCUIT_BREAKER_MAX"),
	ssmDedupParam: requiredString("SSM_DEDUP_PARAM"),
	snsAlertTopicArn: requiredString("SNS_ALERT_TOPIC_ARN"),
	metricNamespace: requiredString("METRIC_NAMESPACE"),
	dryRun: process.env.DRY_RUN === "true",
});
