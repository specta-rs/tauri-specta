import { Channel } from "@tauri-apps/api/core";
import { commands, events, type GreetError } from "./bindings";

let greetInputEl: HTMLInputElement | null;
let greetMsgEl: HTMLElement | null;
let countMsgEl: HTMLElement | null;

// `commands` and `events` are generated from the Rust signatures, so the argument names,
// the argument types, and the event payloads are all checked here.
async function greet(name: string) {
  if (!greetMsgEl) return;

  greetMsgEl.textContent = await commands.greet(name);
}

async function greetStrictly(name: string) {
  if (!greetMsgEl) return;

  // Tauri Specta's default error handling returns a result rather than throwing, so the
  // error is narrowed the same way as any other discriminated union.
  const result = await commands.greetStrictly(name);
  greetMsgEl.textContent =
    result.status === "ok" ? result.data : describeGreetError(result.error);
}

function describeGreetError(error: GreetError): string {
  switch (error.type) {
    case "EmptyName":
      return "A name is required.";
    case "Read":
      return "Could not read the name.";
  }
}

function countTo(number: number) {
  // A `Channel` streams values back as Rust produces them.
  const channel = new Channel<number>();
  channel.onmessage = (current) => {
    if (countMsgEl) countMsgEl.textContent = String(current);
  };

  return commands.countTo(number, channel);
}

async function testIpcRoundTrips() {
  const button = document.querySelector<HTMLButtonElement>("#ipc-test-button");
  if (!button) return;

  button.disabled = true;
  const lines: string[] = [];
  const bytes = [0, 1, 127, 128, 255];
  const json = { message: "Hello from JavaScript", values: [1, 2, 3] };

  const tests: [string, () => Promise<void>][] = [
    ["Named argument and header", async () => {
      const received = await commands.rawIpcNamed(
        { todo: "Test mixed request" },
        { headers: { "x-ipc-test": "specta" } },
      );
      console.log("IPC named argument and header", received);
      if (received.status !== "ok" || received.data !== "Test mixed request") {
        throw new Error(`Unexpected response: ${JSON.stringify(received)}`);
      }
    }],
    ["JSON object", async () => {
      const received = await commands.rawIpcEcho(json);
      console.log("IPC JSON object", { sent: json, received });
      if (typeof received !== "object" || received === null || Array.isArray(received)) {
        throw new Error(`Expected a JSON object, got ${String(received)}`);
      }
      const value = received as Record<string, unknown>;
      if (value.message !== json.message || JSON.stringify(value.values) !== JSON.stringify(json.values)) {
        throw new Error(`Response did not match: ${JSON.stringify(received)}`);
      }
    }],
    ...([
      ["Uint8Array", new Uint8Array(bytes)],
      ["ArrayBuffer", new Uint8Array(bytes).buffer],
      ["number[]", bytes],
    ] as const).map(([name, payload]): [string, () => Promise<void>] => [name, async () => {
      const received = await commands.rawIpcEcho(payload);
      console.log(`IPC ${name}`, { sent: payload, received });
      const actual = received instanceof ArrayBuffer
        ? Array.from(new Uint8Array(received))
        : Array.isArray(received) ? received : null;
      if (!actual || actual.length !== bytes.length || !actual.every((value, i) => value === bytes[i])) {
        throw new Error(`Expected ${JSON.stringify(bytes)}, got ${actual ? JSON.stringify(actual) : String(received)}`);
      }
    }]),
  ];

  try {
    for (const [name, test] of tests) {
      try {
        await test();
        lines.push(`${name}: PASS`);
      } catch (error) {
        lines.push(`${name}: FAIL — ${error instanceof Error ? error.message : String(error)}`);
      }
    }
    console.log("IPC round trip results", lines);
    const failed = lines.some((line) => line.includes(": FAIL"));
    alert(`IPC round trips: ${failed ? "FAIL" : "PASS"}\n\n${lines.join("\n")}`);
  } finally {
    button.disabled = false;
  }
}

// `payload` is typed as `Greeted`, not `unknown`.
events.greeted.listen(({ payload }) => {
  console.log(`The Rust side greeted ${payload.name}`);
});

window.addEventListener("DOMContentLoaded", () => {
  greetInputEl = document.querySelector("#greet-input");
  greetMsgEl = document.querySelector("#greet-msg");
  countMsgEl = document.querySelector("#count-msg");

  document.querySelector("#greet-form")?.addEventListener("submit", (event) => {
    event.preventDefault();
    greet(greetInputEl?.value ?? "");
  });

  document
    .querySelector("#greet-strictly-button")
    ?.addEventListener("click", () => {
      greetStrictly(greetInputEl?.value ?? "");
    });

  document.querySelector("#count-button")?.addEventListener("click", () => {
    countTo(5);
  });
  document.querySelector("#ipc-test-button")?.addEventListener("click", () => {
    void testIpcRoundTrips();
  });
});
