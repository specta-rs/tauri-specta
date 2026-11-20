// `Channel.deserialize` (tauri-apps/tauri#16583) is not in a published
// `@tauri-apps/api` yet, so the generated `semantic_channels_ts` fixture can't
// type-check against the version CI installs. Augmenting the published class is
// enough to check our output; delete this file once the release that ships the
// method is the minimum supported version.
export {}

declare module "@tauri-apps/api/core" {
  interface Channel<T> {
    deserialize<TWire = unknown>(deserialize: (payload: TWire) => T): this
  }
}
