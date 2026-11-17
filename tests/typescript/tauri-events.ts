// Compile-time checks for `tauriEvents`, run by `pnpm run check:bindings`.
//
// The fixtures themselves only prove the generated file parses. These assertions prove
// the accessors actually carry the payload Tauri sends: a shape instead of a bare theme
// string, a `drag-over` that claims to include `paths`, or a null-payload event whose
// `emit` accepts a payload all fail here.

import { TauriEvent } from "@tauri-apps/api/event";

import { tauriEvents } from "../snapshots/kitchen_sink_ts/bindings";

const aNumber = (value: number): number => value;

/** `WINDOW_SCALE_FACTOR_CHANGED` → `windowScaleFactorChanged`. */
type CamelCase<S extends string> = S extends `${infer Head}_${infer Tail}`
	? `${Lowercase<Head>}${Capitalize<CamelCase<Tail>>}`
	: Lowercase<S>;

/** The accessors `TauriEvent` implies, whatever the generated object happens to hold. */
type AccessorsFromTauriEvent = {
	[K in keyof typeof TauriEvent as CamelCase<K & string>]: true;
};

type UnmatchedAccessor =
	| Exclude<keyof AccessorsFromTauriEvent, keyof typeof tauriEvents>
	| Exclude<keyof typeof tauriEvents, keyof AccessorsFromTauriEvent>;

// `never` only when `tauriEvents` has exactly the accessors `TauriEvent` names. A Tauri
// release that adds or renames one fails here, and so does a dropped row in the table.
export const everyTauriEventIsTyped: [UnmatchedAccessor] extends [never]
	? true
	: UnmatchedAccessor = true;

export function payloads(): void {
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
		aNumber(event.payload.size.width);
		aNumber(event.payload.size.height);
	});

	// `Theme` serializes through `Display`, so the payload is the bare string.
	tauriEvents.windowThemeChanged.listen((event) => {
		const theme: "light" | "dark" = event.payload;
		void theme;
	});

	for (const event of [tauriEvents.windowCreated, tauriEvents.webviewCreated]) {
		event.listen((e) => void e.payload.label.toLowerCase());
	}

	tauriEvents.dragEnter.listen((event) => {
		void event.payload.paths.map((path) => path.toUpperCase());
		aNumber(event.payload.position.x);
		aNumber(event.payload.position.y);
	});

	// `drag-over` skips `paths` when serializing.
	tauriEvents.dragOver.listen((event) => aNumber(event.payload.position.y));

	tauriEvents.dragDrop.listen((event) => {
		void event.payload.paths;
		aNumber(event.payload.position.x);
	});

	// A unit payload is typed as `null` and its `emit` takes no argument.
	tauriEvents.windowDestroyed.listen((event) => {
		const payload: null = event.payload;
		void payload;
	});
	void tauriEvents.windowDestroyed.emit();
	void tauriEvents.windowDestroyed.emitTo("main");
	void tauriEvents.dragLeave.emit();

	// The same null special case applies to `emitTo`, and a payload event keeps its
	// payload there too.
	void tauriEvents.dragDrop.emitTo("main", {
		paths: ["/tmp/a.txt"],
		position: { x: 0, y: 0 },
	});
}

export function rejections(): void {
	// @ts-expect-error `drag-over` carries no `paths`.
	void tauriEvents.dragOver.listen((event) => event.payload.paths);

	// @ts-expect-error the theme payload is a string, not `{ theme: ... }`.
	void tauriEvents.windowThemeChanged.listen((event) => event.payload.theme);

	// @ts-expect-error a null-payload event takes no emit payload.
	void tauriEvents.windowDestroyed.emit({});

	// @ts-expect-error nor an `emitTo` payload.
	void tauriEvents.windowDestroyed.emitTo("main", {});

	// @ts-expect-error a payload event still checks the shape it emits.
	void tauriEvents.windowResized.emit({ width: 800 });
}
