import { events } from "../snapshots/events_only_jsdoc/bindings.js";

/** @param {import("@tauri-apps/api/window").Window} window */
export function eventPhases(window) {
	const event = events.phaseSpecificRename;
	event.listen(({ payload }) => {
		payload.serialized_value.toUpperCase();
		// @ts-expect-error Rust only uses this key when deserializing.
		payload.deserialized_value;
	});
	event.emit({ deserialized_value: "send to Rust" });
	// @ts-expect-error Rust only uses this key when serializing.
	event.emit({ serialized_value: "wrong phase" });

	const windowEvent = event(window);
	windowEvent.once(({ payload }) => {
		payload.serialized_value.toUpperCase();
		// @ts-expect-error Rust only uses this key when deserializing.
		payload.deserialized_value;
	});
	windowEvent.emitTo("main", { deserialized_value: "send to Rust" });
	// @ts-expect-error Rust only uses this key when serializing.
	windowEvent.emitTo("main", { serialized_value: "wrong phase" });
}
