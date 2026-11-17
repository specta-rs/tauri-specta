// Drives the generated `tauriEvents` accessors against Tauri's own event mock.
//
// `pnpm run check:bindings` proves the payload types compile; this proves the accessors
// reach the Tauri event API and keep their payloads. Both fixtures are built-in-events
// exports, so this also shows `tauriEvents` works without a single user event registered.
//
// Run with `pnpm run test:mocks`.

import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { afterEach, test } from "node:test";

import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";

// `@tauri-apps/api` expects browser globals. The mock layer only reaches for `crypto`.
globalThis.window = { crypto: webcrypto };

const { tauriEvents } = await import(
	"../snapshots/builtin_events_only_ts/bindings.ts"
);
// The same builder with `.mocks()`: no commands to stub, but `mockCommands` is still
// what installs the mock IPC that delivers events.
const { mockCommands } = await import(
	"../snapshots/builtin_events_mocks_ts/bindings.ts"
);

afterEach(() => clearMocks());

test("a null-payload built-in event listens and emits with no argument", async () => {
	// No handlers: this only exists to install the mock IPC.
	mockCommands();

	const received = [];
	const unlisten = await tauriEvents.windowDestroyed.listen((event) =>
		received.push(event),
	);
	await tauriEvents.windowDestroyed.emit();
	unlisten();

	assert.equal(received.length, 1);
	assert.equal(received[0].event, "tauri://destroyed");
	// Tauri's Rust side emits `null` here; the JS `emit` path has no payload to send and
	// the mock runtime passes it through rather than inventing one.
	assert.equal(received[0].payload, undefined);
});

test("a payload-carrying built-in event keeps its generated shape", async () => {
	mockIPC(() => {}, { shouldMockEvents: true });

	const received = [];
	const unlisten = await tauriEvents.windowResized.listen((event) =>
		received.push(event.payload),
	);
	await tauriEvents.windowResized.emit({ width: 800, height: 600 });
	unlisten();

	assert.deepEqual(received, [{ width: 800, height: 600 }]);
});

test("a drag event reaches the listener with its paths and position", async () => {
	mockIPC(() => {}, { shouldMockEvents: true });

	const received = [];
	const unlisten = await tauriEvents.dragDrop.listen((event) =>
		received.push(event.payload),
	);
	await tauriEvents.dragDrop.emit({
		paths: ["/tmp/a.txt"],
		position: { x: 1.5, y: 2.5 },
	});
	unlisten();

	assert.deepEqual(received, [
		{ paths: ["/tmp/a.txt"], position: { x: 1.5, y: 2.5 } },
	]);
});
