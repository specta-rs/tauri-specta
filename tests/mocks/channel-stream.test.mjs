// Drives the generated `channelStream` helper for real.
//
// `pnpm run check:bindings` type-checks the helper but says nothing about whether
// ordering, termination and single-consumer behaviour actually hold. This imports
// the helper from a committed snapshot that uses a channel and simulates Rust-side
// sends by invoking `stream.channel.onmessage` directly — no IPC is involved.
//
// A `Channel` constructor asks `window.__TAURI_INTERNALS__` for a callback id, so
// the only stub needed is `transformCallback`; it never fires because the test
// invokes the message handler itself.
//
// The fixture is imported as `.ts`, which Node strips types from — hence the
// `--experimental-strip-types` in the `test:mocks` script.
//
// Run with `pnpm run test:mocks`.

import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { test } from "node:test";

globalThis.window = {
	crypto: webcrypto,
	__TAURI_INTERNALS__: {
		transformCallback: () => 1,
	},
};

const { channelStream } = await import(
	"../snapshots/kitchen_sink_mocks_ts/bindings.ts"
);

// Every test here waits on iterator state, so a regression shows up as a promise
// that never settles. Node's test runner has no default timeout and a pending
// promise does not keep the process alive, so bound each test: a broken queue then
// fails loudly instead of wedging the suite.
const streamTest = (name, fn) => test(name, { timeout: 2000 }, fn);

/** The value `promise` rejects with, or `undefined` if it resolves. */
const rejectionOf = (promise) => promise.then(() => undefined, (reason) => reason);

/** Iterate `stream` without letting `for await` swallow assertions. */
const collect = async (iterable) => {
	const values = [];
	for await (const value of iterable) values.push(value);
	return values;
};

streamTest("messages are delivered in arrival order", async () => {
	const stream = channelStream();
	stream.channel.onmessage("first");
	stream.channel.onmessage("second");
	stream.close();

	const values = await collect(stream);
	assert.deepEqual(values, ["first", "second"]);
});

streamTest("a message arriving while `next()` is pending resolves it", async () => {
	const stream = channelStream();
	const iterator = stream[Symbol.asyncIterator]();

	const pending = iterator.next();
	stream.channel.onmessage("live");

	assert.deepEqual(await pending, { done: false, value: "live" });
});

streamTest("concurrent `next()` calls resolve in arrival order", async () => {
	const stream = channelStream();
	const iterator = stream[Symbol.asyncIterator]();

	const first = iterator.next();
	const second = iterator.next();
	const third = iterator.next();
	for (const value of ["a", "b", "c"]) stream.channel.onmessage(value);
	stream.close();

	assert.deepEqual(await first, { done: false, value: "a" });
	assert.deepEqual(await second, { done: false, value: "b" });
	assert.deepEqual(await third, { done: false, value: "c" });
});

streamTest("one message resolves one pending `next()`, not all of them", async () => {
	const stream = channelStream();
	const iterator = stream[Symbol.asyncIterator]();

	const first = iterator.next();
	const second = iterator.next();
	stream.channel.onmessage("only");

	assert.deepEqual(await first, { done: false, value: "only" });

	stream.close();
	assert.deepEqual(await second, { done: true, value: undefined });
});

streamTest("`close()` with no consumer yet leaves the buffered messages", async () => {
	const stream = channelStream();
	stream.channel.onmessage(1);
	stream.channel.onmessage(2);
	stream.close();

	assert.deepEqual(await collect(stream), [1, 2]);
});

streamTest("`close()` drains what was buffered, not what arrives after", async () => {
	const stream = channelStream();
	stream.channel.onmessage(1);
	stream.close();
	stream.channel.onmessage(2);

	assert.deepEqual(await collect(stream), [1]);
});

streamTest("`close()` resolves a pending `next()` as done", async () => {
	const stream = channelStream();
	const iterator = stream[Symbol.asyncIterator]();

	const pending = iterator.next();
	stream.close();

	assert.deepEqual(await pending, { done: true, value: undefined });
});

streamTest("`close()` settles every pending `next()`", async () => {
	const stream = channelStream();
	const iterator = stream[Symbol.asyncIterator]();

	const pending = [iterator.next(), iterator.next()];
	stream.close();

	assert.deepEqual(await Promise.all(pending), [
		{ done: true, value: undefined },
		{ done: true, value: undefined },
	]);
});

streamTest("`next()` after `done` keeps returning done", async () => {
	const stream = channelStream();
	stream.close();
	const iterator = stream[Symbol.asyncIterator]();

	assert.deepEqual(await iterator.next(), { done: true, value: undefined });
	assert.deepEqual(await iterator.next(), { done: true, value: undefined });
});

streamTest("`fail()` rejects a pending `next()` with the error", async () => {
	const stream = channelStream();
	const iterator = stream[Symbol.asyncIterator]();
	const error = new Error("backend died");

	const pending = iterator.next();
	stream.fail(error);

	assert.equal(await rejectionOf(pending), error);
	assert.equal(await rejectionOf(iterator.next()), error);
});

streamTest("`fail()` rejects every pending `next()`", async () => {
	const stream = channelStream();
	const iterator = stream[Symbol.asyncIterator]();

	const pending = [iterator.next(), iterator.next()];
	stream.fail("boom");

	assert.deepEqual(await Promise.all(pending.map(rejectionOf)), [
		"boom",
		"boom",
	]);
});

streamTest("`fail()` drops buffered messages instead of delivering them", async () => {
	const stream = channelStream();
	stream.channel.onmessage("buffered");
	stream.fail("boom");

	assert.equal(await rejectionOf(stream[Symbol.asyncIterator]().next()), "boom");
});

streamTest("messages after either terminal are ignored", async () => {
	const closed = channelStream();
	closed.close();
	closed.channel.onmessage("late");

	const closedIterator = closed[Symbol.asyncIterator]();
	assert.deepEqual(await closedIterator.next(), { done: true, value: undefined });
	assert.deepEqual(await closedIterator.next(), { done: true, value: undefined });

	const failed = channelStream();
	failed.fail("boom");
	failed.channel.onmessage("late");

	assert.equal(await rejectionOf(failed[Symbol.asyncIterator]().next()), "boom");
});

streamTest("the first terminal wins and later calls are no-ops", async () => {
	const closed = channelStream();
	const closedIterator = closed[Symbol.asyncIterator]();
	closed.close();
	closed.fail("too late");
	assert.deepEqual(await closedIterator.next(), { done: true, value: undefined });

	const failed = channelStream();
	const failedIterator = failed[Symbol.asyncIterator]();
	failed.fail("boom");
	failed.close();
	assert.equal(await rejectionOf(failedIterator.next()), "boom");
});

streamTest("a second iterator throws instead of splitting the stream", async () => {
	const stream = channelStream();
	const iterator = stream[Symbol.asyncIterator]();

	assert.throws(
		() => stream[Symbol.asyncIterator](),
		/ChannelStream can only be consumed once/,
	);

	// The rejection happens before any state moves, so the first iterator still works.
	stream.channel.onmessage("still mine");
	assert.deepEqual(await iterator.next(), { done: false, value: "still mine" });
	stream.close();
});

streamTest("`return()` settles a pending `next()` instead of hanging", async () => {
	const stream = channelStream();
	const iterator = stream[Symbol.asyncIterator]();

	const pending = iterator.next();
	assert.deepEqual(await iterator.return(), { done: true, value: undefined });
	assert.deepEqual(await pending, { done: true, value: undefined });
	assert.deepEqual(await iterator.next(), { done: true, value: undefined });
});

streamTest("`for await` works and `break` disposes the stream", async () => {
	const stream = channelStream();
	for (const value of [1, 2, 3, 4]) stream.channel.onmessage(value);

	const seen = [];
	for await (const value of stream) {
		seen.push(value);
		if (value === 2) break;
	}

	assert.deepEqual(seen, [1, 2]);
	// Breaking calls `return()`, so the one consumer is gone and cannot reattach.
	assert.throws(
		() => stream[Symbol.asyncIterator](),
		/ChannelStream can only be consumed once/,
	);
	// Late sends go nowhere instead of throwing.
	stream.channel.onmessage(5);
});
