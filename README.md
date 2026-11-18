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
prerelease. **Stable Tauri v2 is the default**, including for code-generation-only
crates that never depend on `tauri` themselves. Selecting v3 takes two explicit
changes: disable default features, enable `v3`, and add the Tauri v3 dependency
yourself.

```toml
# Default (recommended): Tauri v2.
[dependencies]
tauri = "2"
tauri-specta = { version = "=2.0.0-rc.29", features = ["derive", "typescript"] }
```

```toml
# Opt in to the tested Tauri v3 prerelease.
[dependencies]
tauri = "=3.0.0-alpha.6"
tauri-runtime-wry = "=3.0.0-alpha.6"
tauri-specta = { version = "=2.0.0-rc.29", default-features = false, features = [
    "derive",
    "typescript",
    "v3",
] }

[build-dependencies]
tauri-build = "=3.0.0-alpha.4"
```

Use `@tauri-apps/api@3.0.0-alpha.3` on the v3 frontend. The `v2` feature is what
pins an otherwise unconstrained graph to stable v2; `v3` is required so a build
that selected neither fails with a clear error instead of silently picking a
prerelease. If you also use `tauri-specta-query`, give it the same treatment
(`default-features = false, features = ["v3"]`). Both Tauri versions declare
`links = "Tauri"`, so one graph can only contain one of them. Two
misconfigurations are caught rather than silently building the wrong major:

- Enabling `v3` while the graph still resolves stable v2 fails to compile; the
  crate asserts on a v3-only type.
- Adding a Tauri v3 dependency with default features still enabled keeps the v2
  pin, so Cargo refuses the graph instead of mixing the two.

V3 requires Rust 1.95+ and an explicit runtime dependency and selection. Replace
explicit `tauri::Wry` type arguments with `tauri::DynRuntime` for the default
type-erased runtime, and configure the application with
`.runtime(tauri_runtime_wry::Wry::default())` (or another runtime).

Plugins must likewise support Tauri v3; the v2 examples' plugin/build dependencies
are not a v3 application template.

**Known limitations:** published v3 does not yet provide Specta implementations
for IPC `Request`/`Response`. Semantic channel transforms work with the published
frontend APIs through a same-channel fallback; they do not require an unreleased
`Channel.deserialize` method. See the
[v3 consumer fixture](https://github.com/specta-rs/tauri-specta/blob/main/tests/tauri-v3/README.md)
for coverage and details. Newer Tauri alphas **and v3 stable** are excluded until
the dependency upper bound is updated. The range also admits earlier 3.0.0 alphas,
but only alpha.6 is tested; pin that version for experimental v3 support.

Semantic mappings remain attached to their channel, including when passed to an
untransformed command. Use a fresh channel when switching payload contracts, and
leave mapped channels extensible (do not freeze or seal them).

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
[`tests/tauri-v3/README.md`](https://github.com/specta-rs/tauri-specta/blob/main/tests/tauri-v3/README.md). V2 and v3 cannot share one
Cargo lockfile because both Tauri packages declare `links = "Tauri"`. The
[default-feature fixture](https://github.com/specta-rs/tauri-specta/blob/main/tests/tauri-default)
has its own lockfile and asserts that a consumer which never mentions `tauri`
still resolves stable v2.

## Credit

Created by [oscartbeaumont](https://github.com/oscartbeaumont) and [Brendonovich](https://github.com/brendonovich).
