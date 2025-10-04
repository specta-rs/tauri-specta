import { getCurrentWebview } from "@tauri-apps/api/webview";
import { commands } from "./bindings3/commands";
import { events } from "./bindings3/events";
import { BlueStruct } from './bindings3/commands/blue_struct';
import { library_service } from "./bindings3/commands/library_service";
import { nested } from "./bindings3/commands/nested";

const appWindow = getCurrentWebview();

let greetInputEl: HTMLInputElement | null;
let greetMsgEl: HTMLElement | null;

async function greet() {
  if (greetMsgEl && greetInputEl) {
    greetMsgEl.textContent = await commands.helloWorld(greetInputEl.value);
    nested.someStruct().then(s => console.log(s));
    library_service.getLibrary().then(() => console.log("getLibrary done"));

    await library_service.helloApp().then(res => console.log("helloApp", res));
    await library_service.getDb("mydb").then(res => console.log("getDb", res));
    await library_service.addDb("mydb").then(res => console.log("addDb", res));
    await library_service.getDb("mydb").then(res => console.log("getDb", res));
    await library_service.addDb("mydb").then(res => console.log("addDb", res));
    await commands.hasError().then(res => console.log("hasError", res));

    BlueStruct.instance("default value").then(b => {
      console.log("BlueStruct", b);
      b.getField().then(f => console.log("getField", f)).then(() => {
        b.setField(greetInputEl?.value ?? "").then(res => console.log("setField done", res)).then(() => {
          b.getField().then(f => console.log("getField", f));
        });
      });
    });

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
