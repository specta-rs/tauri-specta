// Guards that `mockCommands` looks handlers up by own property.
//
// A handler is keyed by the command's accessor, which comes from its Rust identifier, so
// `to_string` and `value_of` produce the `Object.prototype` members `toString` and
// `valueOf`. A lookup that fell through to the prototype would call an inherited function
// instead of reporting the command as unmocked, so a test could pass with no handler.
//
// The fixture is imported as `.ts`, which Node strips types from — hence the
// `--experimental-strip-types` in the `test:mocks` script.
//
// Run with `pnpm run test:mocks`.

import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { afterEach, test } from "node:test";

import { clearMocks } from "@tauri-apps/api/mocks";

globalThis.window = { crypto: webcrypto };

const { commands, mockCommands } = await import(
	"../snapshots/mocks_prototype_keys_ts/bindings.ts"
);

afterEach(() => clearMocks());

/** `[accessor, tauri command name]` pairs for the fixture's commands. */
const prototypeKeys = [
	["toString", "to_string"],
	["valueOf", "value_of"],
];

for (const [accessor, command] of prototypeKeys) {
	test(`an unmocked '${command}' command rejects rather than calling the prototype`, async () => {
		mockCommands({});

		await assert.rejects(
			commands[accessor](),
			new RegExp(`No mock handler registered for command '${command}'`),
		);
	});
}

test("a handler for a prototype-named command is still called", async () => {
	for (const [accessor] of prototypeKeys) {
		mockCommands({ [accessor]: () => "mine" });

		assert.equal(await commands[accessor](), "mine");
	}
});
