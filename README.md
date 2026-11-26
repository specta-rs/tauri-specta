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

### Command discovery

Commands only need `#[tauri::command]` when the `introspection` feature is enabled:
`collect_commands!` reads the command's arguments, result, docs and deprecation from
Tauri's own metadata, including `rename`, `rename_all` and injected arguments.

```toml
tauri-specta = { version = "=2.0.0-rc.29", features = [
    "derive",
    "typescript",
    "introspection",
] }
```

```rust
#[tauri::command]
fn hello_world(my_name: String) -> String {
    format!("Hello, {my_name}! You've been greeted from Rust!")
}
```

It needs a Tauri build containing
[tauri-apps/tauri#15326](https://github.com/tauri-apps/tauri/pull/15326), which is
merged upstream but not yet released, so it is **opt-in** until Tauri ships it and
will then join the default features. Without it — including on the Tauri v3
prerelease, which has no metadata API — add `#[specta::specta]` to every command:

```rust
#[tauri::command]
#[specta::specta]
fn hello_world(my_name: String) -> String {
    format!("Hello, {my_name}! You've been greeted from Rust!")
}
```

Both paths generate identical bindings, which CI checks byte-for-byte. A
`#[deprecated]` command is referenced by value to infer its signature; if your crate
denies the `deprecated` lint, add `#[allow(deprecated)]` where you call
`collect_commands!`.

### Tauri v3 prerelease

The same Tauri Specta implementation supports either Tauri v2 or the tested v3
prerelease. **Stable Tauri v2 is the default**, including for code-generation-only
crates that never depend on `tauri` themselves.

For an application, the `tauri` dependency you already have selects the major.
Tauri Specta's own requirement spans both, and Cargo picks the highest match, so
one thing is not automatic: a crate that never depends on `tauri` would drift onto
the prerelease. The default `v2` feature prevents that with a `tauri = "2"`
constraint. To use Tauri v3, disable default features (dropping the pin) and add
the Tauri v3 dependency yourself.

```toml
# Default (recommended): Tauri v2.
[dependencies]
tauri = "2"
tauri-specta = { version = "=2.0.0-rc.29", features = [
    "derive",
    "typescript",
    "introspection",
] }
```

```toml
# Tauri v3 prerelease: disable default features and pin v3 yourself.
[dependencies]
tauri = "=3.0.0-alpha.6"
tauri-runtime-wry = "=3.0.0-alpha.6"
tauri-specta = { version = "=2.0.0-rc.29", default-features = false, features = [
    "derive",
    "typescript",
] }
# Same treatment if you use it:
tauri-specta-query = { version = "=2.0.0-rc.29", default-features = false }

[build-dependencies]
tauri-build = "=3.0.0-alpha.4"
```

The Rust API is unchanged. The differences are naming the runtime and selecting it
when building the app; `tauri::Wry` does not exist in v3. V3 commands still need
`#[specta::specta]` because the published prerelease has no command introspection;
on Tauri v2 that requirement disappears once the `introspection` feature is enabled.

```rust
use specta_typescript::Typescript;
use tauri_specta::{Builder, collect_commands};

#[tauri::command]
#[specta::specta]
fn hello_world(my_name: String) -> String {
    format!("Hello, {my_name}! You've been greeted from Rust!")
}

fn main() {
    // v3's type-erased default runtime. A concrete runtime such as
    // `tauri_runtime_wry::WryRuntime` also works for static dispatch.
    let builder = Builder::<tauri::DynRuntime>::new()
        .commands(collect_commands![hello_world]);

    #[cfg(debug_assertions)]
    builder
        .export(Typescript::default(), "../src/bindings.ts")
        .expect("Failed to export typescript bindings");

    tauri::Builder::default()
        // v3 selects the runtime here instead of through the `wry` feature.
        .runtime(tauri_runtime_wry::Wry::default())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app); // still required if you use events
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

`build.rs` is unchanged (`tauri_build::build()`), and generated bindings are
unchanged: import `commands`, `events`, and `tauriEvents` exactly as on v2. For the
covered API, exports are byte-identical between the majors; CI compares them. Use
`@tauri-apps/api@3.0.0-alpha.3` on the v3 frontend, matching the Rust major.

The `v2` feature is what pins an otherwise unconstrained graph to stable v2; the
Rust `tauri` dependency you add chooses the version otherwise. Both Tauri versions
declare `links = "Tauri"`, so one graph can only contain one of them: adding a v3
dependency while default features are still enabled keeps the v2 pin, and
resolution fails rather than mixing the two. Disabling default features without
adding a Tauri dependency of your own leaves Cargo's highest match in charge,
which is the v3 prerelease.

V3 requires Rust 1.95+ and v3-compatible Tauri plugins; the v2 examples' build and
plugin dependencies are not a v3 application template.

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
