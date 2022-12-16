import * as commands from "./bindings";

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
});
