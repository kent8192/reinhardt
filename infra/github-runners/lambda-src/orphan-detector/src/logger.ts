import pino from "pino";

// Lambda CloudWatch Logs expects plain stdout JSON.
// redact prevents accidental leakage of secrets in log records.
export const logger = pino({
	level: process.env.LOG_LEVEL ?? "info",
	formatters: {
		level: (label) => ({ level: label }),
	},
	redact: {
		paths: [
			"*.authorization",
			"*.Authorization",
			"*.headers.authorization",
			'*.headers["x-hub-signature-256"]',
			"*.secret",
			"*.privateKey",
			"*.githubAppKey",
			"*.webhookSecret",
		],
		censor: "[REDACTED]",
	},
	base: {
		service: "orphan-detector",
	},
});
