import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Channel } from "@tauri-apps/api/core";
import { commands, events } from "./bindings";
// import { commands, events } from "./bindings-jsdoc.js";

const appWindow = getCurrentWebview();

let greetInputEl: HTMLInputElement | null;
let greetMsgEl: HTMLElement | null;

async function greet() {
  if (greetMsgEl && greetInputEl) {
    greetMsgEl.textContent = await commands.helloWorld(greetInputEl.value);

    setTimeout(async () => console.log(await commands.goodbyeWorld()), 1000);
  }
}

window.addEventListener("DOMContentLoaded", () => {
  greetInputEl = document.querySelector("#greet-input");
  greetMsgEl = document.querySelector("#greet-msg");
  document
    .querySelector("#greet-button")
    ?.addEventListener("click", () => greet());

  document
    .querySelector("#send-event-button")
    ?.addEventListener("click", () => {
      events.emptyEvent.emit();
    });
});

events.emptyEvent.listen((e) => console.log("Global event", e));
events.emptyEvent(appWindow).listen((e) => console.log("Window event", e));

const date = new Date();
const bytes = new Uint8Array([1, 2, 3, 4]);
const url = new URL("https://specta.dev/docs?example=rich-types");
const channel = new Channel<{ date: Date; bytes: Uint8Array; url: URL }>();

channel.onmessage = (message) => {
  console.log("semanticTypes channel", message);
};

events.semanticTypesEvent.listen((event) => {
  console.log("semanticTypesEvent", event.payload);
  console.log(
    "SEMANTIC EVENT ASSERTIONS",
    event.payload.date instanceof Date,
    event.payload.bytes instanceof Uint8Array,
    event.payload.url instanceof URL,
  );
});

commands.semanticTypes({ date, bytes, url }, channel).then((result) => {
  console.log("semanticTypes", result);
  console.log(
    "SEMANTIC TYPE ASSERTIONS",
    result.date.getTime() === date.getTime(),
    result.bytes.length === bytes.length &&
      result.bytes.every((v, i) => v === bytes[i]),
    result.url.href === url.href,
  );

  events.semanticTypesEvent.emit(result);
});
