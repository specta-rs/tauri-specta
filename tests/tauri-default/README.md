# Default-feature consumer tests

An application-shaped crate that depends on `tauri-specta` and
`tauri-specta-query` but never mentions `tauri` itself. This is the case Cargo's
highest-match rule gets wrong without help: the library accepts v2 and a v3
prerelease, so the prerelease would otherwise win.

`tauri-specta`'s default `v2` feature enables
[`tauri-specta-pin`](../../crates/tauri-specta-pin), a code-less crate whose
`tauri = "2"` edge constrains that shared package back to stable v2.

The committed `Cargo.lock` is part of the test. CI runs:

```sh
cargo check --locked --manifest-path tests/tauri-default/Cargo.toml
python3 tests/assert_tauri_version.py tests/tauri-default/Cargo.toml 2
```

The v3 fixture runs the mirror assertion for alpha.6. Each fixture resolves
exactly one `tauri` package, because both majors declare `links = "Tauri"`.
