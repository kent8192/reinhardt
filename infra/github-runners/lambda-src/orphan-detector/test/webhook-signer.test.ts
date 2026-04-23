import { describe, expect, it } from "vitest";
import { computeSignature } from "../src/webhook-signer.js";

describe("computeSignature", () => {
	it("matches GitHub docs example vector", () => {
		// From https://docs.github.com/en/webhooks/using-webhooks/validating-webhook-deliveries
		const secret = "It's a Secret to Everybody";
		const body = "Hello, World!";
		const expected =
			"sha256=757107ea0eb2509fc211221cce984b8a37570b6d7586c22c46f4379c8b043e17";
		expect(computeSignature(secret, body)).toBe(expected);
	});

	it("produces different signature for 1-byte body difference", () => {
		const secret = "secret";
		const a = computeSignature(secret, "hello");
		const b = computeSignature(secret, "hellO");
		expect(a).not.toBe(b);
	});

	it("prefixes with 'sha256='", () => {
		expect(computeSignature("s", "x")).toMatch(/^sha256=[0-9a-f]{64}$/);
	});

	it("is stable for same input", () => {
		expect(computeSignature("s", "x")).toBe(computeSignature("s", "x"));
	});
});
