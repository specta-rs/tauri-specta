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
});
