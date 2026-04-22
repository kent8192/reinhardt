import { defineConfig } from "vitest/config";

export default defineConfig({
	test: {
		include: ["test/**/*.test.ts"],
		environment: "node",
		coverage: {
			provider: "v8",
			reporter: ["text", "json", "html"],
			include: ["src/**/*.ts"],
			exclude: ["src/index.ts"],
			thresholds: {
				lines: 90,
				branches: 85,
				functions: 90,
				statements: 90,
			},
		},
	},
});
