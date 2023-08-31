import { invoke as TAURI_INVOKE } from "@tauri-apps/api";
import * as TAURI_API_EVENT from "@tauri-apps/api/event";
import { type WebviewWindowHandle as __WebviewWindowHandle__ } from "@tauri-apps/api/window";

type __EventObj__<T> = {
  listen: (
    cb: TAURI_API_EVENT.EventCallback<T>
  ) => ReturnType<typeof TAURI_API_EVENT.listen<T>>;
  listenFor: (
    window: __WebviewWindowHandle__,
    cb: TAURI_API_EVENT.EventCallback<T>
  ) => ReturnType<typeof TAURI_API_EVENT.listen<T>>;
  once: (
    cb: TAURI_API_EVENT.EventCallback<T>
  ) => ReturnType<typeof TAURI_API_EVENT.once<T>>;
  onceFor: (
    window: __WebviewWindowHandle__,
    cb: TAURI_API_EVENT.EventCallback<T>
  ) => ReturnType<typeof TAURI_API_EVENT.once<T>>;
  emit: T extends null
    ? (payload?: T) => ReturnType<typeof TAURI_API_EVENT.emit>
    : (payload: T) => ReturnType<typeof TAURI_API_EVENT.emit>;
  emitFor: T extends null
    ? (
        window: __WebviewWindowHandle__,
        payload?: T
      ) => ReturnType<typeof TAURI_API_EVENT.emit>
    : (
        window: __WebviewWindowHandle__,
        payload: T
      ) => ReturnType<typeof TAURI_API_EVENT.emit>;
};

type __Result__<T, E> = [T, undefined] | [undefined, E];

function __makeEvents__<T extends Record<string, any>>(
  mappings: Record<keyof T, string>
) {
  return new Proxy(
    {} as unknown as {
      [K in keyof T]: __EventObj__<T[K]>;
    },
    {
      get: (_, event) =>
        new Proxy({} as __EventObj__<any>, {
          get: (_, command: keyof __EventObj__<any>) => {
            const name = mappings[event as keyof T];

            switch (command) {
              case "listen":
                return (arg: any) => TAURI_API_EVENT.listen(name, arg);
              case "listenFor":
                return (window: __WebviewWindowHandle__, arg: any) =>
                  window.listen(name, arg);
              case "once":
                return (arg: any) => TAURI_API_EVENT.once(name, arg);
              case "onceFor":
                return (window: __WebviewWindowHandle__, arg: any) =>
                  window.once(name, arg);
              case "emit":
                return (arg: any) => TAURI_API_EVENT.emit(name, arg);
              case "emitFor":
                return (window: __WebviewWindowHandle__, arg: any) =>
                  window.emit(name, arg);
            }
          },
        }),
    }
  );
}
