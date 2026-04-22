import * as esbuild from "esbuild";

await esbuild.build({
	entryPoints: ["src/index.ts"],
	bundle: true,
	platform: "node",
	target: "node20",
	format: "esm",
	outfile: "dist/index.mjs",
	// Lambda Node 20 runtime supports ESM; inject a CommonJS `require` shim so
	// transitive deps that use `require()` keep working inside the ESM bundle.
	banner: {
		js: "import { createRequire } from 'module'; const require = createRequire(import.meta.url);",
	},
	minify: false, // keep stack traces readable in CloudWatch logs
	sourcemap: "inline",
	external: [
		// @aws-sdk/* is pre-installed in the Lambda Node 20 runtime;
		// marking external shrinks the bundle by ~5MB.
		"@aws-sdk/*",
	],
	logLevel: "info",
});
