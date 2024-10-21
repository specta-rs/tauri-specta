import {
	invoke as TAURI_INVOKE,
} from "@tauri-apps/api/core";


export type Result<T, E> =
	| { status: "ok"; data: T }
	| { status: "error"; error: E };

