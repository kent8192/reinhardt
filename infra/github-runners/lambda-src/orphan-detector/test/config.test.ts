import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { loadConfig } from "../src/config.js";

const ORIGINAL_ENV = { ...process.env };

describe("loadConfig", () => {
	beforeEach(() => {
		process.env = { ...ORIGINAL_ENV };
	});
	afterEach(() => {
		process.env = { ...ORIGINAL_ENV };
	});

	const setRequired = () => {
		process.env.GITHUB_APP_ID = "2953037";
		process.env.GITHUB_APP_KEY_SSM_PARAM = "/reinhardt-ci/github-app-key";
		process.env.GITHUB_OWNER = "kent8192";
		process.env.GITHUB_REPO = "reinhardt-web";
		process.env.WEBHOOK_URL = "https://webhook.example.com/";
		process.env.WEBHOOK_SECRET_SSM_PARAM = "/reinhardt-ci/webhook-secret";
		process.env.STALENESS_MIN = "60";
		process.env.CIRCUIT_BREAKER_MAX = "52";
		process.env.SSM_DEDUP_PARAM = "/reinhardt-ci/orphan-detector/processed";
		process.env.SNS_ALERT_TOPIC_ARN =
			"arn:aws:sns:us-east-1:495680546359:reinhardt-ci-ci-alert";
		process.env.METRIC_NAMESPACE = "ReinhardtCI/OrphanDetector";
	};

	it("parses all required env vars into typed Config", () => {
		setRequired();
		const cfg = loadConfig();
		expect(cfg.githubAppId).toBe("2953037");
		expect(cfg.owner).toBe("kent8192");
		expect(cfg.repo).toBe("reinhardt-web");
		expect(cfg.stalenessMin).toBe(60);
		expect(cfg.circuitBreakerMax).toBe(52);
		expect(cfg.dryRun).toBe(false);
	});

	it("dryRun defaults to false when env var absent", () => {
		setRequired();
		delete process.env.DRY_RUN;
		expect(loadConfig().dryRun).toBe(false);
	});

	it("dryRun parses 'true' string", () => {
		setRequired();
		process.env.DRY_RUN = "true";
		expect(loadConfig().dryRun).toBe(true);
	});

	it("dryRun parses 'false' string", () => {
		setRequired();
		process.env.DRY_RUN = "false";
		expect(loadConfig().dryRun).toBe(false);
	});

	it("throws when required env var missing", () => {
		setRequired();
		delete process.env.GITHUB_APP_ID;
		expect(() => loadConfig()).toThrow(/GITHUB_APP_ID/);
	});

	it("throws when STALENESS_MIN is not a positive integer", () => {
		setRequired();
		process.env.STALENESS_MIN = "not-a-number";
		expect(() => loadConfig()).toThrow(/STALENESS_MIN/);
	});

	it("throws when CIRCUIT_BREAKER_MAX is negative", () => {
		setRequired();
		process.env.CIRCUIT_BREAKER_MAX = "-1";
		expect(() => loadConfig()).toThrow(/CIRCUIT_BREAKER_MAX/);
	});
});
