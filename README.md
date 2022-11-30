# Tauri Specta

Typesafe Tauri Commands.

Warning: This repo is under heavy development. Things may change, and quickly.

## Install

```bash
pnpm i tauri-specta

cargo add tauri-specta
```

## Adding Specta to custom types

```rust
use specta::Type;
use serde::{Deserialize, Serialize};

// The `specta::Type` macro allows us to understand your types
// We implement `specta::Type` on primitive types for you.
// If you want to use a type from an external crate you may need to enable the feature on Specta.
#[derive(Serialize, Type)]
pub struct MyCustomReturnType {
    pub foo: String,
    pub bar: i32,
}

#[derive(Deserialize, Type)]
pub struct MyCustomArgumentType {
    pub foo: String,
    pub bar: i32,
}
```

## Annotate your Tauri commands with Specta

```rust
#[tauri::command]
#[specta] // <-- This bit here
fn greet3() -> MyStruct {
    MyStruct {
        some_field: "Hello World".into(),
    }
}
```


## Export your bindings

```rust
// this example exports your types on startup when in debug mode or in a unit test. You can do whatever.

fn main() {
    #[cfg(debug_assertions)]
    export_to_ts(collate_types![greet, greet2, greet3], "../src/bindings.ts").unwrap();
}


#[test]
fn export_bindings() {
    export_to_ts(collate_types![greet, greet2, greet3], "../src/bindings.ts").unwrap();
}
```

## Use on frontend

```ts
import { Commands } from "./bindings"; // This should point to the file we export from Rust

const t = typedInvoke<Commands>();

await t.invoke("greet", { name: 42 });
```

## Known limitations

 - Your command can only take up to 16 arguments. Any more and you'll get a compile error. If you need more just use a struct.
 - Exporting your schema within a directory tracked by Tauri's hot reload will cause an infinite reload loop.

## Development

Run the example:

```bash
pnpm i
pnpm package build
cd example/
pnpm tauri dev
```

## Future Work:

 - Tauri event support
 - Reexport all the deps that the macro stuff needs.
 - Would be nice for it to be a single macro.
 - Stable OpenAPI support - Currently will crash if your types have generics.
 - Write exports for many different languages. Maybe for support with something like [tauri-sys](https://github.com/JonasKruckenberg/tauri-sys).
 - Clean up code
 - Proper unit tests
