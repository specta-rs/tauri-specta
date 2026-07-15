# Tauri Specta Query

Generate type-safe TanStack Query helpers for commands exported by
[`tauri-specta`](https://crates.io/crates/tauri-specta).

Commands are split into queries and mutations with `CommandSet`. The generated
helpers and the returned Tauri Specta builder share the same function casing:

```rust
use tauri_specta::Casing;
use tauri_specta_query::CommandSet;

# fn configure<R: tauri::Runtime>(commands: CommandSet<R>) {
let commands = commands.function_casing(Casing::SnakeCase);
# }
```
