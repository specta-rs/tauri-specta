# Tauri Specta Query

Generate type-safe TanStack Query helpers for commands exported by
[`tauri-specta`](https://crates.io/crates/tauri-specta).

Commands are split into queries and mutations with `CommandSet`. The generated
helpers and the returned Tauri Specta builder share the same argument casing:

```rust
use tauri_specta::Casing;
use tauri_specta_query::CommandSet;

# fn configure<R: tauri::Runtime>(commands: CommandSet<R>) {
let commands = commands.argument_casing(Casing::SnakeCase);
# }
```

Use `SnakeCase` when the corresponding Tauri commands specify
`#[tauri::command(rename_all = "snake_case")]`.
