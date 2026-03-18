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

|           | Tauri v1                             | Tauri v2                             |
| --------- | ------------------------------------ | ------------------------------------ |
| Specta v1 | Tauri Specta v1 [docs](https://docs.rs/tauri-specta/%5E1.0.2/tauri_specta/index.html) | Unsupported                          |
| Specta v2 | Unsupported                          | Tauri Specta v2 [docs](https://docs.rs/tauri-specta/^2.0.0-rc.21/tauri_specta/index.html) |

Tauri Specta v2 also comes with support for generating types for events.

Follow the documentation links above for help getting started.

## TanStack Query output

You can generate command bindings as `queryOptions` helpers instead of plain invoke functions:

```rust
use tauri_specta::{Builder, CommandOutputTarget, collect_commands};

let builder = Builder::<tauri::Wry>::new()
    .commands(collect_commands![/* ... */])
    .command_output_target(CommandOutputTarget::TanstackQuery);
```

Generated commands import `queryOptions` from `@tanstack/react-query` and return objects like:

```ts
commands.myCommand(argOne, argTwo)
// -> queryOptions({ queryKey: ["my_command", { argOne, argTwo }] as const, queryFn: ... })
```

You can also mark specific commands as mutations:

```rust
let builder = Builder::<tauri::Wry>::new()
    .commands(collect_commands![create_user, list_users])
    .command_output_target(CommandOutputTarget::TanstackQuery)
    .mutation_commands(["create_user"]);
```

`create_user` will generate `mutationOptions({ mutationKey, mutationFn })` while `list_users`
will still generate `queryOptions({ queryKey, queryFn })`.

## Development

Run the example:

```bash
pnpm i
cd examples/app/
pnpm tauri dev
```

### Running tests

```bash
mkdir _out
OUT_DIR="$(pwd)/_out" cargo test --all --all-features
```

## Credit

Created by [oscartbeaumont](https://github.com/oscartbeaumont) and [Brendonovich](https://github.com/brendonovich).
