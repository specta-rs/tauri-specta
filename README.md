<div align="center">
    <img height="150" src=".github/logo.png" alt="Specta Logo"></img>
    <h1>Tauri Specta</h1>
    <p><b>Typesafe Tauri commands</b></p>
    <a href="https://discord.gg/JgqH8b4ycw"><img src="https://img.shields.io/discord/1011665225809924136?style=flat-square" alt="Discord"></a>
    <a href="https://crates.io/crates/tauri-specta"><img src="https://img.shields.io/crates/v/tauri-specta.svg?style=flat-square"
    alt="crates.io" /></a>
    <a href="/LICENSE.md"><img src="https://img.shields.io/crates/l/tauri-specta?style=flat-square" alt="License"></a>
</div>

<br>

> [!NOTE]
> This branch contains tauri-specta v2, you can checkout the git tag [v1.0.2](https://github.com/specta-rs/tauri-specta/tree/v1.0.2) for the previous version.

## Getting Started

First you will need to choose a version:

|           | Tauri v1 | Tauri v2 | Tauri v3 |
| --------- | -------- | -------- | -------- |
| Specta v1 | Tauri Specta v1 [docs](https://docs.rs/tauri-specta/%5E1.0.2/tauri_specta/index.html) | Unsupported | Unsupported |
| Specta v2 | Unsupported | Tauri Specta v2 [docs](https://docs.rs/tauri-specta/^2.0.0-rc.21/tauri_specta/index.html) | Experimental: 3.0.0-alpha.6 |

Tauri Specta v2 also comes with support for generating types for events.

Follow the documentation links above for help getting started.

### Tauri v3 prerelease

The same Tauri Specta implementation supports either Tauri v2 or the tested v3
prerelease. Select `tauri = "=3.0.0-alpha.6"` in your application and use
`@tauri-apps/api@3.0.0-alpha.3` on the frontend; there is no version-selection
feature flag. V2 applications and the existing examples stay on v2.

V3 requires Rust 1.95+ and an explicit runtime dependency and selection. Replace
explicit `tauri::Wry` type arguments with `tauri::DynRuntime` for the default
type-erased runtime, and configure the application with
`.runtime(tauri_runtime_wry::Wry::default())` (or another runtime).

**Known limitations:** published v3 does not yet provide Specta implementations
for IPC `Request`/`Response`, and frontend alpha.3 does not provide the
`Channel.deserialize` method needed for semantic channel transforms. See the
[v3 consumer fixture](tests/tauri-v3/README.md) for coverage and details. Untested
newer Tauri alphas are excluded until the dependency upper bound is updated.

## Development

Run an example:

```bash
pnpm install
cd examples/app # or examples/tanstack-query, or examples/custom-plugin/app
pnpm tauri dev
```

### Running tests

```bash
cargo test --all --all-features
```

Run v3 consumer and frontend checks separately, as described in
[`tests/tauri-v3/README.md`](tests/tauri-v3/README.md). V2 and v3 cannot share one
Cargo lockfile because both Tauri packages declare `links = "Tauri"`.

## Credit

Created by [oscartbeaumont](https://github.com/oscartbeaumont) and [Brendonovich](https://github.com/brendonovich).
