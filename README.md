# Tauri Specta

Typesafe Tauri Commands.

Warning: This repo is currently just a technical demo.

## Using Specta

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

### Features

TODO: Link to all of the features supported by Specta.

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
 - Move more of the type exporting into Specta. Right now this package uses strings to generate some Typescript which isn't great.
 - Clean up code
 - Proper unit tests