import { Commands } from "./bindings";
import { typedInvoke } from "./runtime";

let greetInputEl: HTMLInputElement | null;
let greetMsgEl: HTMLElement | null;

const t = typedInvoke<Commands>();

async function greet() {
  if (greetMsgEl && greetInputEl) {
    greetMsgEl.textContent = await t.invoke("greet", {
      name: greetInputEl.value,
    });

    // @ts-expect-error
    await t.invoke("greet", { name: 42 });
    // @ts-expect-error
    await t.invoke("not-a-function");
  }
}

window.addEventListener("DOMContentLoaded", () => {
  greetInputEl = document.querySelector("#greet-input");
  greetMsgEl = document.querySelector("#greet-msg");
  document
    .querySelector("#greet-button")
    ?.addEventListener("click", () => greet());
});
