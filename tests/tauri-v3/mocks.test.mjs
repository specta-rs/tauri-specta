import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { afterEach, test } from "node:test";
import { clearMocks } from "@tauri-apps/api/mocks";

globalThis.window = { crypto: webcrypto };

const bindings = await import("./target/bindings/bindings.ts");
const jsdoc = await import("./target/bindings/bindings.js");
const result = await import("./target/bindings/result.ts");
const unknown = await import("./target/bindings/result-unknown.ts");
const dataError = await import("./target/bindings/data-error.ts");

afterEach(() => clearMocks());

for (const [name, api] of [["TypeScript", bindings], ["JSDoc", jsdoc]]) {
  test(`${name} mocks dispatch renamed and async commands`, async () => {
    const calls = api.mockCommands({
      greet: ({ name }) => `Hello, ${name}`,
      double: async ({ value }) => value * 2,
    });
    assert.equal(await api.commands.greet("Tauri"), "Hello, Tauri");
    assert.equal(await api.commands.double(21), 42);
    assert.deepEqual(calls.calls[0], { command: "greet", args: { name: "Tauri" } });
  });

  test(`${name} events and built-in events round-trip and unlisten`, async () => {
    api.mockCommands({});
    const received = [];
    const unlisten = await api.events.progress.listen((event) => received.push(event.payload));
    await api.events.progress.emit({ completed: 42 });
    unlisten();
    await api.events.progress.emit({ completed: 99 });
    assert.deepEqual(received, [{ completed: 42 }]);

    const sizes = [];
    const stop = await api.tauriEvents.windowResized.listen((event) => sizes.push(event.payload));
    await api.tauriEvents.windowResized.emit({ width: 800, height: 600 });
    stop();
    await api.tauriEvents.windowResized.emit({ width: 1, height: 1 });
    assert.deepEqual(sizes, [{ width: 800, height: 600 }]);
  });
}

// The published mock forwards emit args without an event id. Tauri's once()
// therefore cannot remove its listener. Keep the failing assertion visible until
// upstream fixes the mock; do not change the generated event wrapper to hide it.
test("v3 event mocks implement once", {
  todo: "@tauri-apps/api alpha.3 mock event payloads omit the listener id",
}, async () => {
  bindings.mockCommands({});
  const sizes = [];
  await bindings.tauriEvents.windowResized.once((event) => sizes.push(event.payload));
  await bindings.tauriEvents.windowResized.emit({ width: 800, height: 600 });
  await bindings.tauriEvents.windowResized.emit({ width: 1, height: 1 });
  assert.deepEqual(sizes, [{ width: 800, height: 600 }]);
});

test("throw mode rejects command errors", async () => {
  bindings.mockCommands({ fail: () => { throw "failed"; } });
  await assert.rejects(bindings.commands.fail(), (error) => error === "failed");
});

for (const [name, api, expected] of [
  ["result", result, { status: "error", error: "failed" }],
  ["result-unknown", unknown, { status: "error", error: "failed" }],
  ["data-error", dataError, { data: null, error: "failed" }],
]) {
  test(`${name} wraps command errors`, async () => {
    api.mockCommands({ fail: () => { throw "failed"; } });
    assert.deepEqual(await api.commands.fail(), expected);
  });
}
