import { invoke as TAURI_INVOKE } from "@tauri-apps/api";
import * as TAURI_API_EVENT from "@tauri-apps/api/event";

type __EventObj__<T> = {
  listen: (
    cb: TAURI_API_EVENT.EventCallback<T>
  ) => ReturnType<typeof TAURI_API_EVENT.listen<T>>;
  once: (
    cb: TAURI_API_EVENT.EventCallback<T>
  ) => ReturnType<typeof TAURI_API_EVENT.once<T>>;
  emit: T extends null
    ? (payload?: T) => ReturnType<typeof TAURI_API_EVENT.emit>
    : (payload: T) => ReturnType<typeof TAURI_API_EVENT.emit>;
};

function __makeEvent__<T>(name: string): __EventObj__<T> {
  return {
    listen: (cb) => TAURI_API_EVENT.listen(name, cb),
    once: (cb) => TAURI_API_EVENT.once(name, cb),
    emit: ((payload?: T) => TAURI_API_EVENT.emit(name, payload)) as any,
  };
}
