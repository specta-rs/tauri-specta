import {
	invoke as TAURI_INVOKE,
	Channel as TAURI_CHANNEL,
} from "@tauri-apps/api/core";
import * as TAURI_API_EVENT from "@tauri-apps/api/event";

/** @typedef {typeof import("@tauri-apps/api/webview").Webview} __Webview__ */
/** @typedef {typeof import("@tauri-apps/api/window").Window} __Window__ */

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
 *	}} __EventObj__<T>
 */

/**
 * @template T,E
 * @typedef { { status: "ok", data: T } | { status: "error", error: E } } Result
 */

/**
 * @template {Record<string, any>} T
 * @param {Record<keyof T, string>} mappings
 * @returns {{
 * 	 [K in keyof T]: __EventObj__<T[K]> & {
 *	   (handle: __Webview__ | __Window__): __EventObj__<T[K]>;
 *   };
 * }}
 */
function __makeEvents__(mappings) {
	return new Proxy(
		{},
		{
			get: (_, event) => {
				const name = mappings[event];

				return new Proxy(() => {}, {
					apply: (_, __, [window]) => ({
						listen: (arg) => window.listen(name, arg),
						once: (arg) => window.once(name, arg),
						emit: (arg) => window.emit(name, arg),
					}),
					get: (_, command) => {
						switch (command) {
							case "listen":
								return (arg) => TAURI_API_EVENT.listen(name, arg);
							case "once":
								return (arg) => TAURI_API_EVENT.once(name, arg);
							case "emit":
								return (arg) => TAURI_API_EVENT.emit(name, arg);
						}
					},
				});
			},
		},
	);
}
