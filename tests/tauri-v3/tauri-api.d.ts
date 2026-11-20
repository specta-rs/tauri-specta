// `Channel.deserialize` (tauri-apps/tauri#16583) is not in the `@tauri-apps/api`
// prerelease these bindings are checked against yet, so the generated semantic
// channel binding can't type-check without it. Delete this file once the minimum
// supported v3 API has the method.
export {}

declare module "@tauri-apps/api/core" {
  interface Channel<T> {
    deserialize<TWire = unknown>(deserialize: (payload: TWire) => T): this
  }
}
