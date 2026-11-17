// Pins the surface `mockCommands` returns in the JSDoc exporter.
//
// The JSDoc bindings declare `MockCall` and `MockCommandsHandle` as `@typedef`s instead of
// `export type`s, so nothing else in the suite proves they resolve under `checkJs`. Checked
// by `pnpm run check:bindings` through `tsconfig.jsdoc.json`.

import { mockCommands } from "../snapshots/kitchen_sink_mocks_jsdoc/bindings.js";

const handle = mockCommands({ greet: ({ myName }) => `Hello, ${myName}!` });

/** @type {string} */
const command = handle.calls[0].command;
/** @type {unknown} */
const args = handle.calls[0].args;

handle.clear();

// @ts-expect-error the handle only exposes `calls` and `clear`
handle.nope;
// @ts-expect-error the history is readonly
handle.calls.push({ command: "greet", args: {} });
// @ts-expect-error `command` is a string, not an accessor union or a number
handle.calls[0].command = 1;

void command;
void args;
