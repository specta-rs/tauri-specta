import { getCurrentWebview } from "@tauri-apps/api/webview";
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

events.emptyEvent.listen((e) => console.log(e));
events.emptyEvent(appWindow).listen((e) => console.log(e));
