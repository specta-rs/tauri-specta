import * as commands from "./bindings";
import { typedInvoke } from "tauri-specta";

let greetInputEl: HTMLInputElement | null;
let greetMsgEl: HTMLElement | null;

const t = typedInvoke<commands.Commands>();

async function greet() {
  if (greetMsgEl && greetInputEl) {
    greetMsgEl.textContent = await commands.helloWorld(greetInputEl.value);
    // greetMsgEl.textContent = await t.invoke("hello_world", {
    //   myName: "test",
    // });

    setTimeout(async () => console.log(await commands.goodbyeWorld()), 1000);
  }
}

window.addEventListener("DOMContentLoaded", () => {
  greetInputEl = document.querySelector("#greet-input");
  greetMsgEl = document.querySelector("#greet-msg");
  document
    .querySelector("#greet-button")
    ?.addEventListener("click", () => greet());
});
