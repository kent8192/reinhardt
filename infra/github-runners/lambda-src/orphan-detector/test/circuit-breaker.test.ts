import { describe, expect, it } from "vitest";
import { shouldHalt } from "../src/circuit-breaker.js";

describe("shouldHalt", () => {
	it("returns false when orphan count below threshold", () => {
		expect(shouldHalt(5, 10)).toBe(false);
	});

	it("returns false when orphan count equals threshold (boundary)", () => {
		expect(shouldHalt(10, 10)).toBe(false);
	});

	it("returns true when orphan count exceeds threshold", () => {
		expect(shouldHalt(11, 10)).toBe(true);
	});

	it("handles zero threshold", () => {
		expect(shouldHalt(1, 0)).toBe(true);
		expect(shouldHalt(0, 0)).toBe(false);
	});
});
