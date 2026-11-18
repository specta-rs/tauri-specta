# Tauri v3 consumer tests

This standalone workspace tests the real library manifests against published
`tauri = 3.0.0-alpha.6`, without inheriting the main workspace's v2 patch. Its
frontend install uses `@tauri-apps/api = 3.0.0-alpha.3`. Both lockfiles are
committed so a new alpha cannot silently change the tested APIs.

V2 applications remain in the main workspace. Cargo permits only one Tauri
package per resolver graph because both versions declare `links = "Tauri"`;
optional dependencies or a version-selection feature cannot bypass that rule.

## Run

From the repository root, with Rust 1.95+, Node 22, and pnpm 10:

```sh
cargo test --locked --manifest-path tests/tauri-v3/Cargo.toml
pnpm --dir tests/tauri-v3 install --frozen-lockfile
pnpm --dir tests/tauri-v3 run check:bindings
pnpm --dir tests/tauri-v3 test
```

Rust tests generate the TypeScript, JSDoc, and query bindings under
`target/bindings/`. Run them before the frontend checks, even when the Rust
target directory is redirected. Tests use `MockRuntime`, so they do not launch a
webview or require Wry/GTK. A separate compile check covers collecting commands
and attaching their handler with v3's default `DynRuntime`.

Coverage includes renamed and async command dispatch, injected state and app
handles, command failures, typed event registration/emission/unlistening/once,
channel argument types, plugin-prefixed exports, query generation, and generated
mocks against the real v3 frontend API in all four error-handling modes.

## Known gaps

- IPC `Request` and `Response` lack upstream Specta impls in this published
  prerelease. The main workspace's v2 patch does not apply to v3 and would not
  propagate to downstream applications anyway. Track
  [tauri-apps/tauri#16374](https://github.com/tauri-apps/tauri/pull/16374) and its
  v3 release/backport before claiming binary IPC support without patches.
- Semantic channel payload transforms require `Channel.deserialize`, absent
  from frontend alpha.3. Plain channel arguments are exported and type-checked;
  channel delivery through a real webview and semantic transforms are not tested.
- The published frontend event mock omits the listener id in emitted payloads,
  so `once()` cannot unregister itself. The frontend suite retains this assertion
  as a TODO/expected failure; ordinary listen/emit/unlisten tests pass. Rust-side
  typed `once()` is covered separately and passes.
- Frontend type-checking uses bundler resolution, matching the root suite. The
  prerelease API declarations contain extensionless imports that fail under
  NodeNext resolution. `ESNext.Disposable` supplies the API's `Symbol.asyncDispose`
  declaration without skipping library checks.
- These tests do not validate a real Wry/CEF application, mobile targets, or
  runtime-specific APIs. The application must add a matching runtime crate and
  select it with `tauri::Builder::runtime`; `tauri::Wry` and `tauri/wry` are gone.
- V2-oriented library doctests still use v2 application setup. They run in the
  main workspace, not this consumer fixture.

When advancing the alpha, update the shared Tauri upper bound in the root
`Cargo.toml`, the exact fixture version, the matching frontend version, both
fixture lockfiles, and the compatibility documentation. Keep the v2 examples'
explicit major constraints and verify both resolver graphs still select exactly
one Tauri package of the intended major.
