import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { afterEach, test } from "node:test";

import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";

globalThis.window = { crypto: webcrypto };
afterEach(() => clearMocks());

for (const [format, load] of [
	[
		"TypeScript",
		() => import("../snapshots/error_result_unknown_semantic_ts/bindings.ts"),
	],
	[
		"JavaScript",
		() => import("../snapshots/error_result_unknown_semantic_jsdoc/bindings.js"),
	],
]) {
	test(`${format}: ResultUnknown preserves errors and still converts success values`, async () => {
		const { commands } = await load();
		for (const error of [new Error("transport failed"), "permission denied", 42]) {
			mockIPC(() => Promise.reject(error));
			const result = await commands.bigintResult();
			assert.equal(result.status, "error");
			assert.equal(result.error, error);
		}

		mockIPC(() => 42);
		assert.deepEqual(await commands.bigintResult(), { status: "ok", data: 42n });
	});
}
