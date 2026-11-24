# Tauri v3 consumer tests

> **Temporarily broken.** `collect_commands!` reads Tauri's command metadata, which
> `tauri = 3.0.0-alpha.6` does not expose yet. These tests fail until a Tauri v3
> build contains tauri-apps/tauri#15326.

This standalone workspace tests the real library manifests against published
`tauri = 3.0.0-alpha.6`, without inheriting the main workspace's v2 patch. Its
frontend install uses `@tauri-apps/api = 3.0.0-alpha.3`. Both lockfiles are
committed so a new alpha cannot silently change the tested APIs.

It depends on both libraries with `default-features = false`, which drops the
default v2 pin; its own Tauri v3 dependency decides the version.
`tests/tauri-default` covers the opposite case: a consumer that never mentions
`tauri` and must resolve stable v2.

V2 applications remain in the main workspace. Cargo permits only one Tauri
package per resolver graph because both versions declare `links = "Tauri"`;
optional dependencies or a version-selection feature cannot bypass that rule.

## Run

From the repository root, with Rust 1.95+, Node 22, and pnpm 10 (CI includes a
Linux Rust 1.95.0 job as well as current stable on all three desktop platforms):

```sh
cargo test --locked --manifest-path tests/tauri-v3/Cargo.toml
pnpm --dir tests/tauri-v3 install --frozen-lockfile
pnpm --dir tests/tauri-v3 run check:bindings
pnpm --dir tests/tauri-v3 test
```

## Binding parity with v2

CI compiles **the same** `tests/support/version_parity.rs` fixture against both
Tauri majors, rather than comparing bindings from unrelated examples. The v2
query-crate integration test and v3 consumer test each export 12 files covering
TypeScript/JSDoc, every error mode, renamed/async commands, injected arguments,
plain/semantic channels, typed and built-in events, mocks, plugin prefixes, and
React Query helpers.

Each desktop job uploads its generated tree. The `binding-parity` job downloads
all six v2/v3 × Linux/macOS/Windows artifacts and compares every file's **bytes**
and the complete file sets against the v2 Linux export. Missing/empty artifacts,
added/removed files, line-ending differences, and changed contents fail CI; there
is no snapshot update or normalization that could hide a version difference.
The comparator has separate positive and negative tests.

To run the same comparison locally:

```sh
cargo test --locked -p tauri-specta-query --test version_parity
cargo test --locked --manifest-path tests/tauri-v3/Cargo.toml
python3 -m unittest discover -s tests -p test_compare_bindings.py
python3 tests/compare_bindings.py target/version-parity/v2 tests/tauri-v3/target/bindings
```

The shared fixture deliberately excludes IPC `Request`/`Response`: published v3
cannot collect them yet. Parity covers the common supported API, not an assertion
that the upstream limitation is resolved.

Rust tests generate the TypeScript, JSDoc, and query bindings under
`target/bindings/`. Run them before the frontend checks, even when the Rust
target directory is redirected. Tests use `MockRuntime`, so they do not launch a
webview or require Wry/GTK. A separate compile check covers collecting commands
and attaching their handler with v3's default `DynRuntime`.

Coverage includes renamed and async command dispatch, injected state and app
handles, command failures, typed event registration/emission/unlistening/once,
channel argument types, plugin-prefixed exports, query generation, and generated
mocks against the real v3 frontend API in all four error-handling modes.
Shared semantic-channel tests run against both frontend majors in TypeScript and
JSDoc, checking the original channel/callback id, ordered delivery, end-message
cleanup, late handler assignment, and repeated/cross-module registration without
stacking transforms. The adapter registers the transform by wrapping the
channel's `onmessage`. Mapping remains attached when a channel is passed to an
untransformed command: use a fresh channel when switching payload contracts, and
give commands that need different transforms their own channel. Wrapping needs an
extensible channel, so frozen and sealed channels are not supported.

## Known gaps

- IPC `Request` and `Response` lack upstream Specta impls in this published
  prerelease. The main workspace's v2 patch does not apply to v3 and would not
  propagate to downstream applications anyway. Track
  [tauri-apps/tauri#16374](https://github.com/tauri-apps/tauri/pull/16374) and its
  v3 release/backport before claiming binary IPC support without patches.
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
  Alpha.6 apps also need `tauri-build = "=3.0.0-alpha.4"` and v3-compatible plugins;
  the main workspace's v2 build dependencies should not be copied unchanged.
- V2-oriented library doctests still use v2 application setup. They run in the
  main workspace, not this consumer fixture.

When advancing the alpha, update the shared Tauri upper bound in the root
`Cargo.toml`, the exact fixture version, the matching frontend version, both
fixture lockfiles, and the compatibility documentation. Keep the v2 examples'
explicit major constraints and verify both resolver graphs still select exactly
one Tauri package of the intended major.
