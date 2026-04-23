import { beforeEach, describe, expect, it, vi } from "vitest";
import { PutParameterCommand, type SSMClient } from "@aws-sdk/client-ssm";
import { loadProcessedState, saveProcessedState, TTL_MS } from "../src/dedup.js";

describe("dedup", () => {
	const paramName = "/test/orphan-detector/processed";
	const ssmMock = {
		send: vi.fn(),
	} as unknown as SSMClient;

	beforeEach(() => {
		vi.clearAllMocks();
	});

	describe("loadProcessedState", () => {
		it("returns empty map when SSM returns empty JSON", async () => {
			(ssmMock.send as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
				Parameter: { Value: "{}" },
			});
			const result = await loadProcessedState(ssmMock, paramName, Date.now());
			expect(result.size).toBe(0);
		});

		it("returns parsed map for valid JSON", async () => {
			const now = Date.now();
			(ssmMock.send as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
				Parameter: { Value: JSON.stringify({ "100": now - 1000 }) },
			});
			const result = await loadProcessedState(ssmMock, paramName, now);
			expect(result.get(100)).toBe(now - 1000);
		});

		it("drops entries older than TTL", async () => {
			const now = Date.now();
			const payload = {
				"100": now - TTL_MS - 1, // expired
				"101": now - 1000, // fresh
			};
			(ssmMock.send as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
				Parameter: { Value: JSON.stringify(payload) },
			});
			const result = await loadProcessedState(ssmMock, paramName, now);
			expect(result.has(100)).toBe(false);
			expect(result.has(101)).toBe(true);
		});

		it("returns empty map on corrupt JSON (fail-open)", async () => {
			(ssmMock.send as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
				Parameter: { Value: "not-json{" },
			});
			const result = await loadProcessedState(ssmMock, paramName, Date.now());
			expect(result.size).toBe(0);
		});

		it("returns empty map on SSM error (fail-open)", async () => {
			(ssmMock.send as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
				new Error("ThrottlingException"),
			);
			const result = await loadProcessedState(ssmMock, paramName, Date.now());
			expect(result.size).toBe(0);
		});
	});

	describe("saveProcessedState", () => {
		it("serializes map to JSON via PutParameter with Overwrite=true", async () => {
			(ssmMock.send as ReturnType<typeof vi.fn>).mockResolvedValueOnce({});
			const map = new Map<number, number>([[100, 1234567890]]);
			await saveProcessedState(ssmMock, paramName, map);

			const call = (ssmMock.send as ReturnType<typeof vi.fn>).mock.calls[0]?.[0];
			expect(call).toBeInstanceOf(PutParameterCommand);
			expect(call.input.Name).toBe(paramName);
			expect(call.input.Value).toBe('{"100":1234567890}');
			expect(call.input.Overwrite).toBe(true);
		});

		it("does not throw on SSM save error (log-only)", async () => {
			(ssmMock.send as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
				new Error("ThrottlingException"),
			);
			const map = new Map<number, number>([[100, 1]]);
			await expect(saveProcessedState(ssmMock, paramName, map)).resolves.toBeUndefined();
		});
	});
});
