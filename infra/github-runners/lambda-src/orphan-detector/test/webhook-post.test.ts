import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { MockAgent, setGlobalDispatcher, getGlobalDispatcher } from "undici";
import { postSignedWebhook } from "../src/webhook-signer.js";

describe("postSignedWebhook", () => {
	let mockAgent: MockAgent;
	let originalDispatcher: ReturnType<typeof getGlobalDispatcher>;

	beforeEach(() => {
		originalDispatcher = getGlobalDispatcher();
		mockAgent = new MockAgent();
		mockAgent.disableNetConnect();
		setGlobalDispatcher(mockAgent);
	});

	afterEach(async () => {
		await mockAgent.close();
		setGlobalDispatcher(originalDispatcher);
	});

	const args = {
		url: "https://webhook.example.com/",
		secret: "test-secret",
		deliveryId: "00000000-0000-4000-8000-000000000000",
		bodyObj: { action: "queued", workflow_job: { id: 1 } },
		jobId: 1,
		timeoutMs: 5000,
	};

	it("returns ok=true on 2xx", async () => {
		mockAgent
			.get("https://webhook.example.com")
			.intercept({ path: "/", method: "POST" })
			.reply(202, { ok: true });

		const res = await postSignedWebhook(args);
		expect(res.ok).toBe(true);
		expect(res.status).toBe(202);
	});

	it("does NOT retry on 4xx, returns ok=false immediately", async () => {
		let callCount = 0;
		mockAgent
			.get("https://webhook.example.com")
			.intercept({ path: "/", method: "POST" })
			.reply(() => {
				callCount++;
				return { statusCode: 401, data: "unauthorized" };
			})
			.times(3);

		const res = await postSignedWebhook(args);
		expect(res.ok).toBe(false);
		expect(res.status).toBe(401);
		expect(callCount).toBe(1);
	});

	it("retries on 5xx up to 3 attempts", async () => {
		let callCount = 0;
		mockAgent
			.get("https://webhook.example.com")
			.intercept({ path: "/", method: "POST" })
			.reply(() => {
				callCount++;
				return { statusCode: 500, data: "internal error" };
			})
			.times(3);

		const res = await postSignedWebhook(args);
		expect(res.ok).toBe(false);
		expect(res.status).toBe(500);
		expect(callCount).toBe(3);
	});

	it("sets HMAC signature header from body and secret", async () => {
		let capturedSignature: string | undefined;
		mockAgent
			.get("https://webhook.example.com")
			.intercept({ path: "/", method: "POST" })
			.reply((opts) => {
				const headers = opts.headers as Record<string, string | string[]>;
				const sig = headers["x-hub-signature-256"] ?? headers["X-Hub-Signature-256"];
				capturedSignature = Array.isArray(sig) ? sig[0] : sig;
				return { statusCode: 202, data: {} };
			});

		await postSignedWebhook(args);
		expect(capturedSignature).toMatch(/^sha256=[0-9a-f]{64}$/);
	});
});
