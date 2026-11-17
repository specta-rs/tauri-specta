// Drives the generated mock helpers for real.
//
// `pnpm run check:bindings` type-checks the committed fixtures but says nothing about
// whether `mockCommands` actually routes a call, so this runs the `kitchen_sink_mocks_ts`
// and `mocks_result_ts` fixtures through `@tauri-apps/api`'s own mock IPC layer. The
// bindings under test are the snapshots the Rust suite writes, so a change to the
// generated runtime or the command names it switches on fails here too.
//
// The fixtures cover both error handling modes the mocks have to survive: the default
// (`Throw`, where a handler's value passes straight back to the caller) and `Result`
// (where the injected `typedError` runtime wraps it).
//
// The fixtures are imported as `.ts`, which Node strips types from — hence the
// `--experimental-strip-types` in the `test:mocks` script.
//
// Run with `pnpm run test:mocks`.

import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { afterEach, test } from "node:test";

import { invoke } from "@tauri-apps/api/core";
import { clearMocks } from "@tauri-apps/api/mocks";

// `@tauri-apps/api` expects browser globals. The mock layer only reaches for `crypto`.
globalThis.window = { crypto: webcrypto };

const { commands, events, mockCommands } = await import(
	"../snapshots/kitchen_sink_mocks_ts/bindings.ts"
);
const wrapped = await import("../snapshots/mocks_result_ts/bindings.ts");

afterEach(() => clearMocks());

/** The value `promise` rejects with, or `undefined` if it resolves. */
const rejectionOf = (promise) => promise.then(() => undefined, (reason) => reason);

/**
 * Record the `plugin:event|unlisten` IPC calls, which is how a generated `unlisten`
 * removes a listener.
 */
const trackUnlistens = () => {
	const invoke = window.__TAURI_INTERNALS__.invoke;
	const calls = [];
	window.__TAURI_INTERNALS__.invoke = (cmd, args, options) => {
		if (cmd === "plugin:event|unlisten") calls.push(args);
		return invoke(cmd, args, options);
	};
	return calls;
};

/**
 * Run `fn` with `console.warn` silenced.
 *
 * `@tauri-apps/api/mocks` 2.12.4 looks the listener up by `args.id` when handling
 * `plugin:event|unlisten`, while `event.listen` sends the id as `eventId`. The mock's
 * private registry therefore keeps a stale id, and emitting that event again warns
 * about a callback that no longer exists — which is exactly what the signal tests
 * assert (our callback is gone), so the warning is expected noise.
 */
const quietly = async (fn) => {
	const warn = console.warn;
	console.warn = () => {};
	try {
		return await fn();
	} finally {
		console.warn = warn;
	}
};

test("a handler receives the command's arguments", async () => {
	mockCommands({ greet: ({ myName }) => `Hello, ${myName}!` });

	assert.equal(await commands.greet("Brendan"), "Hello, Brendan!");
});

test("a handler may be async", async () => {
	mockCommands({ asyncCommand: async ({ delayMs }) => delayMs * 2 });

	assert.equal(await commands.asyncCommand(21), 42);
});

test("commands without arguments take no handler argument", async () => {
	mockCommands({ maybe: () => "value" });

	assert.equal(await commands.maybe(), "value");
});

test("the handler map is partial", async () => {
	// Only `greet` is mocked; `mockCommands` must install regardless.
	mockCommands({ greet: () => "hi" });

	assert.equal(await commands.greet("x"), "hi");
});

test("throwing from a handler fails the command with the thrown value", async () => {
	mockCommands({
		plainError: () => {
			throw { PermissionDenied: "no" };
		},
	});

	assert.deepEqual(await rejectionOf(commands.plainError()), {
		PermissionDenied: "no",
	});
});

test("rejecting from a handler fails the command with the rejection", async () => {
	mockCommands({ taggedError: () => Promise.reject({ Io: "boom" }) });

	assert.deepEqual(await rejectionOf(commands.taggedError()), { Io: "boom" });
});

test("a command with no handler rejects instead of resolving undefined", async () => {
	mockCommands({});

	await assert.rejects(
		commands.greet("Brendan"),
		/No mock handler registered for command 'greet'/,
	);
});

test("a `Result` command wraps a mocked success in the mode's shape", async () => {
	wrapped.mockCommands({ nullableResult: () => null });

	assert.deepEqual(await wrapped.commands.nullableResult(), {
		status: "ok",
		data: null,
	});
});

test("a `Result` command reports a thrown handler as the mode's error", async () => {
	wrapped.mockCommands({
		plainError: () => {
			// `typedError` rethrows `Error` instances as framework failures, so a command
			// error is a plain value — which is what a real backend sends.
			throw "NotFound";
		},
	});

	assert.deepEqual(await wrapped.commands.plainError(), {
		status: "error",
		error: "NotFound",
	});
});

test("events still round-trip through the mocked backend", async () => {
	mockCommands({});

	const received = [];
	const unlisten = await events.messageEvent.listen((event) =>
		received.push(event.payload),
	);
	await events.messageEvent.emit("hello");
	unlisten();

	assert.deepEqual(received, ["hello"]);
});

test("event payloads keep their generated shape", async () => {
	mockCommands({});

	const received = [];
	const unlisten = await events.structEvent.listen((event) =>
		received.push(event.payload),
	);
	await events.structEvent.emit({ eventId: 7, payload: "seven" });
	unlisten();

	assert.deepEqual(received, [{ eventId: 7, payload: "seven" }]);
});

test("a listener with an already-aborted signal never fires", async () => {
	mockCommands({});
	const unlistens = trackUnlistens();

	const controller = new AbortController();
	controller.abort();

	const received = [];
	const unlisten = await events.messageEvent.listen(
		(event) => received.push(event.payload),
		{ signal: controller.signal },
	);

	// It was registered and unlistened straight away, so nothing is left behind.
	assert.equal(unlistens.length, 1);
	// The returned unlisten is a no-op, and calling it more than once must not throw.
	unlisten();
	unlisten();
	assert.equal(unlistens.length, 1, "the returned unlisten is a no-op");

	await quietly(() => events.messageEvent.emit("hello"));

	assert.deepEqual(received, []);
});

test("aborting a signal unlistens the listener", async () => {
	mockCommands({});
	const unlistens = trackUnlistens();

	const controller = new AbortController();
	const received = [];
	const unlisten = await events.messageEvent.listen(
		(event) => received.push(event.payload),
		{ signal: controller.signal },
	);

	await events.messageEvent.emit("before");
	controller.abort();
	assert.equal(unlistens.length, 1);

	await quietly(() => events.messageEvent.emit("after"));
	unlisten();
	assert.equal(unlistens.length, 1, "unlisten is a no-op after the abort");

	assert.deepEqual(received, ["before"]);
});

test("`once` honours the signal too", async () => {
	mockCommands({});
	const unlistens = trackUnlistens();

	const controller = new AbortController();
	const received = [];
	await events.messageEvent.once(
		(event) => received.push(event.payload),
		{ signal: controller.signal },
	);

	controller.abort();
	assert.equal(unlistens.length, 1);
	await quietly(() => events.messageEvent.emit("hello"));

	assert.deepEqual(received, []);
});

test("the real command keeps running its own transforms around the mock", async () => {
	// `echoProfile` deserializes its argument and serializes its result, so the handler
	// sees the same value the command would and the caller gets the deserialized result.
	mockCommands({ echoProfile: ({ profile }) => profile });

	const profile = { userId: 1, displayName: "Oscar", tags: [], homepage: null, scores: {} };
	assert.deepEqual(await commands.echoProfile(profile), profile);
});

test("the event plugin's own invokes are not recorded", async () => {
	const calls = mockCommands({ greet: () => "hi" });

	// `shouldMockEvents` intercepts `plugin:event|*` inside Tauri's mock, before the
	// callback the bindings install, so event plumbing must never reach the history.
	const unlisten = await events.messageEvent.listen(() => {});
	await events.messageEvent.emit("hello");
	await events.messageEvent.emitTo("main", "hello");
	await unlisten();

	await commands.greet("x");

	// Also proves a command is recorded exactly once.
	assert.deepEqual(calls.calls, [{ command: "greet", args: { myName: "x" } }]);
});

test("a call is recorded with the accessor and the arguments it was invoked with", async () => {
	const handle = mockCommands({ greet: ({ myName }) => `Hello, ${myName}!` });

	// The handle's array is live, so a reference taken before the call sees it too.
	const history = handle.calls;
	assert.deepEqual(history, []);

	await commands.greet("Brendan");

	assert.deepEqual(history, [{ command: "greet", args: { myName: "Brendan" } }]);
	assert.equal(handle.calls, history);
});

test("a command invoked without arguments records empty args", async () => {
	const calls = mockCommands({ maybe: () => null });

	await commands.maybe();

	assert.deepEqual(calls.calls, [{ command: "maybe", args: {} }]);
});

test("a call to an unregistered handler is recorded before it rejects", async () => {
	const calls = mockCommands({});

	await assert.rejects(
		commands.greet("Brendan"),
		/No mock handler registered for command 'greet'/,
	);

	assert.deepEqual(calls.calls, [{ command: "greet", args: { myName: "Brendan" } }]);
});

test("a command the bindings do not generate is recorded under its Tauri name", async () => {
	const calls = mockCommands({});

	await assert.rejects(
		invoke("not_generated", { value: 1 }),
		/No mock handler registered for command 'not_generated'/,
	);

	assert.deepEqual(calls.calls, [
		{ command: "not_generated", args: { value: 1 } },
	]);
});

test("a call whose handler throws is still recorded", async () => {
	const calls = mockCommands({
		plainError: () => {
			throw { PermissionDenied: "no" };
		},
	});

	await rejectionOf(commands.plainError());

	assert.deepEqual(calls.calls, [{ command: "plainError", args: {} }]);
});

test("a call whose handler rejects is still recorded", async () => {
	const calls = mockCommands({ taggedError: () => Promise.reject({ Io: "boom" }) });

	await rejectionOf(commands.taggedError());

	assert.deepEqual(calls.calls, [{ command: "taggedError", args: {} }]);
});

test("`clear` empties the history but leaves the handlers installed", async () => {
	const calls = mockCommands({ greet: ({ myName }) => `Hello, ${myName}!` });

	await commands.greet("Brendan");
	calls.clear();

	assert.deepEqual(calls.calls, []);
	assert.equal(await commands.greet("Oscar"), "Hello, Oscar!");
	assert.deepEqual(calls.calls, [{ command: "greet", args: { myName: "Oscar" } }]);
});

test("a fresh install returns a fresh handle and the previous one stops growing", async () => {
	const first = mockCommands({ greet: () => "first" });
	await commands.greet("a");

	const second = mockCommands({ greet: () => "second" });
	assert.deepEqual(second.calls, []);

	assert.equal(await commands.greet("b"), "second");
	assert.deepEqual(second.calls, [{ command: "greet", args: { myName: "b" } }]);
	// The old handle keeps what it recorded, but the replaced handlers never touch it.
	assert.deepEqual(first.calls, [{ command: "greet", args: { myName: "a" } }]);
});

test("`clearMocks` stops recording but keeps the handle's history", async () => {
	const calls = mockCommands({ greet: () => "hi" });
	await commands.greet("a");

	clearMocks();

	// Tauri's own cleanup removes the install without touching this handle: nothing new
	// can reach it, and what it already recorded stays put.
	assert.deepEqual(calls.calls, [{ command: "greet", args: { myName: "a" } }]);
	calls.clear();
	assert.deepEqual(calls.calls, []);
});

test("the handle's history cannot be detached from the recording", async () => {
	const calls = mockCommands({ greet: () => "hi" });
	await commands.greet("a");

	// `readonly` is a compile-time promise; the object is frozen so a caller that ignores
	// it (plain JavaScript, or a cast) can't orphan the array the handler keeps appending
	// to and then see an empty-looking handle.
	assert.throws(() => {
		calls.calls = [];
	}, TypeError);
	assert.deepEqual(calls.calls, [{ command: "greet", args: { myName: "a" } }]);
});
