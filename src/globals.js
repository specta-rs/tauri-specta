import { invoke as TAURI_INVOKE } from "@tauri-apps/api";
import * as TAURI_API_EVENT from "@tauri-apps/api/event";

/**
 * @template T
 * @typedef {{
 *   listen: (
 *	   cb: TAURI_API_EVENT.EventCallback<T>
 *	 ) => ReturnType<typeof TAURI_API_EVENT.listen<T>>;
 *	 once: (
 *	   cb: TAURI_API_EVENT.EventCallback<T>
 *	 ) => ReturnType<typeof TAURI_API_EVENT.once<T>>;
 *	 emit: T extends null
 *	   ? (payload?: T) => ReturnType<typeof TAURI_API_EVENT.emit>
 *     : (payload: T) => ReturnType<typeof TAURI_API_EVENT.emit>;
 *	}}__EventObj__<T>
 *	 */

/**
 * @template T
 * @param {string} name
 * @returns {__EventObj__<T>}
 */
function __makeEvent__(name) {
  return {
    listen: (cb) => TAURI_API_EVENT.listen(name, cb),
    once: (cb) => TAURI_API_EVENT.once(name, cb),
    emit: (payload) => TAURI_API_EVENT.emit(name, payload),
  };
}
