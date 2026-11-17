// Drives the generated `channelStream` helper for real.
//
// `pnpm run check:bindings` type-checks the helper but says nothing about whether
// ordering, termination and single-consumer behaviour actually hold. This imports
// the helper from a committed snapshot that uses a channel and simulates Rust-side
// sends by assigning `stream.channel.onmessage` directly — no IPC is involved.
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

/** The value `promise` rejects with, or `undefined` if it resolves. */
const rejectionOf = (promise) => promise.then(() => undefined, (reason) => reason);

/** Iterate `stream` without letting `for await` swallow assertions. */
const collect = async (iterable) => {
	const values = [];
	for await (const value of iterable) values.push(value);
	return values;
};

test("messages are delivered in arrival order", async () => {
	const stream = channelStream();
	stream.channel.onmessage("first");
	stream.channel.onmessage("second");
	stream.close();

	const values = await collect(stream);
	assert.deepEqual(values, ["first", "second"]);
});

test("a message arriving while `next()` is pending resolves it", async () => {
	const stream = channelStream();
	const iterator = stream[Symbol.asyncIterator]();

	const pending = iterator.next();
	stream.channel.onmessage("live");

	assert.deepEqual(await pending, { done: false, value: "live" });
});

test("`close()` resolves a pending `next()` as done", async () => {
	const stream = channelStream();
	const iterator = stream[Symbol.asyncIterator]();

	const pending = iterator.next();
	stream.close();

	assert.deepEqual(await pending, { done: true, value: undefined });
});

test("`close()` drains buffered messages before reporting done", async () => {
	const stream = channelStream();
	stream.channel.onmessage(1);
	stream.channel.onmessage(2);
	stream.close();

	assert.deepEqual(await collect(stream), [1, 2]);
});

test("`fail()` rejects a pending `next()` with the error", async () => {
	const stream = channelStream();
	const iterator = stream[Symbol.asyncIterator]();
	const error = new Error("backend died");

	const pending = iterator.next();
	stream.fail(error);

	assert.equal(await rejectionOf(pending), error);
	assert.equal(await rejectionOf(iterator.next()), error);
});

test("`fail()` drops buffered messages instead of delivering them", async () => {
	const stream = channelStream();
	stream.channel.onmessage("buffered");
	stream.fail("boom");

	assert.equal(await rejectionOf(stream[Symbol.asyncIterator]().next()), "boom");
});

test("messages after either terminal are ignored", async () => {
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

test("the first terminal wins and later calls are no-ops", async () => {
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

test("a second iterator throws instead of splitting the stream", () => {
	const stream = channelStream();
	stream[Symbol.asyncIterator]();

	assert.throws(
		() => stream[Symbol.asyncIterator](),
		/ChannelStream can only be consumed once/,
	);
});

test("`for await` works and `break` leaves the stream usable", async () => {
	const stream = channelStream();
	for (const value of [1, 2, 3, 4]) stream.channel.onmessage(value);

	const seen = [];
	for await (const value of stream) {
		seen.push(value);
		if (value === 2) break;
	}

	assert.deepEqual(seen, [1, 2]);
	// Breaking does not close the channel, and closing afterwards must still work.
	stream.close();
});
