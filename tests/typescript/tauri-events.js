// Compile-time checks for `tauriEvents` in the JSDoc output, run by
// `pnpm run check:bindings`.
//
// `tauri-events.ts` proves the same thing for the TypeScript emitter. This file exists
// because the JSDoc emitter reaches the payload types through a `@type` comment instead
// of a type argument, which is exactly the kind of thing that can silently degrade to
// `any`. The rejected cases carry the proof: if the payload were `any`, the
// `@ts-expect-error` directives below would be unused.
//
// The fixture is command-free because the JSDoc fixtures that have commands carry
// unrelated pre-existing `checkJs` diagnostics (see `tsconfig.jsdoc.json`). Every JSDoc
// export carries the same `tauriEvents`, with or without user events.

import { tauriEvents } from "../snapshots/events_only_jsdoc/bindings.js";

const aNumber = (/** @type {number} */ value) => value;

export function payloads() {
	tauriEvents.windowResized.listen((event) => {
		aNumber(event.payload.width);
		aNumber(event.payload.height);
	});

	tauriEvents.windowMoved.listen((event) => {
		aNumber(event.payload.x);
		aNumber(event.payload.y);
	});

	tauriEvents.windowScaleFactorChanged.listen((event) => {
		aNumber(event.payload.scaleFactor);
		aNumber(event.payload.size.height);
	});

	tauriEvents.windowThemeChanged.listen((event) => {
		/** @type {"light" | "dark"} */
		const theme = event.payload;
		void theme;
	});

	tauriEvents.dragEnter.listen((event) => {
		void event.payload.paths.map((path) => path.toUpperCase());
		aNumber(event.payload.position.x);
	});

	tauriEvents.dragOver.listen((event) => aNumber(event.payload.position.y));

	// A unit payload is typed as `null`, not `any`.
	tauriEvents.windowDestroyed.listen((event) => {
		/** @type {null} */
		const payload = event.payload;
		void payload;
	});

	// The JSDoc runtime has no equivalent of `EventEmit`'s null special case, so a unit
	// payload is passed explicitly instead of omitted. It is still typed, which is what
	// the rejections below show.
	tauriEvents.windowDestroyed.emit(null);
}

export function rejections() {
	// @ts-expect-error `drag-over` carries no `paths`.
	tauriEvents.dragOver.listen((event) => event.payload.paths);

	// @ts-expect-error the theme payload is a string, not `{ theme: ... }`.
	tauriEvents.windowThemeChanged.listen((event) => event.payload.theme);

	// @ts-expect-error a null-payload event takes no emit payload.
	tauriEvents.windowDestroyed.emit({});

	// @ts-expect-error nor an `emitTo` payload.
	tauriEvents.windowDestroyed.emitTo("main", {});

	// @ts-expect-error a null-payload event still needs its `null` argument in JSDoc.
	tauriEvents.windowDestroyed.emit();
}
