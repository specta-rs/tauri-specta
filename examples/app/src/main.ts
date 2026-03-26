import { getCurrentWebview } from "@tauri-apps/api/webview";
import { commands, events } from "./bindings";
import { Channel, invoke } from "@tauri-apps/api/core";
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



invoke("special_types", {
  input: {
    //   // TODO: Fix all these to big types
    //   // TODO: Fix all these types
    //   // TODO: Date working
    //   // TODO: Make them `number | BigInt` for arguments

    //   // u128_max: (1n << 128n) - 1n,
    //   // u128_min: 0n,
    //   // i128_max: (1n << 127n) - 1n,
    //   // i128_min: -(1n << 127n),
    //   // TODO: The types for these are not correct at all
    //   // bytes: [4, 3, 2, 1],
    //   // bytes_from_vec: new Uint8Array([4, 3, 2, 1]),
    //   // TODO: Fix these types as `string | Date`
    //   // date: new Date(),
    //   // datetime: new Date(),
    //
    u128_max: (1n << 128n) - 1n,
    u128_min: 0n,
    i128_max: (1n << 127n) - 1n,
    i128_min: -(1n << 127n),
} }, {
  reviver: (key, value, context) => {
   // console.log("DEBUG REVIVER", key, value, context);

   if (
     context &&
     typeof value === "number" &&
     Number.isInteger(value) &&
     /^-?\d+$/.test(context.source) &&
     !Number.isSafeInteger(value)
   ) {
     // console.log("FOUND BIGINT", context.source);
     return BigInt(context.source);
   }

   return value;
 }
}).then(([echo, from_rs]) => {
    console.log("SPECIAL TYPES:", echo, from_rs);
    console.log(
      "ECHO ASSERTIONS:",
      echo.u128_max == (1n << 128n) - 1n,
      echo.u128_min == 0n,
      echo.i128_max == (1n << 127n) - 1n,
      echo.i128_min == -(1n << 127n)
    );
    console.log(
      "FROM_RS ASSERTIONS:",
      from_rs.u128_max == (1n << 128n) - 1n,
      from_rs.u128_min == 0n,
      from_rs.i128_max == (1n << 127n) - 1n,
      from_rs.i128_min == -(1n << 127n)
    );
});

// const channel = new Channel<bigint>();
// channel.onmessage = (value) => {
//   console.log("channel bigint:", value);
//   console.log("is bigint:", typeof value === "bigint");
//   console.log("matches u128::MAX:", value === (1n << 128n) - 1n);
// };
// await commands.specialTypesWChannel(channel);
