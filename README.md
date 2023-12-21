<div align="center">
    <img height="150" src=".github/logo.png" alt="Specta Logo"></img>
    <h1>Tauri Specta</h1>
    <p><b>Typesafe Tauri commands</b></p>
    <a href="https://discord.gg/4V9M5sksw8"><img src="https://img.shields.io/discord/1011665225809924136?style=flat-square" alt="Discord"></a>
    <a href="https://crates.io/crates/tauri-specta"><img src="https://img.shields.io/crates/d/tauri-specta?style=flat-square" alt="Crates.io"></a>
    <a href="https://crates.io/crates/tauri-specta"><img src="https://img.shields.io/crates/v/tauri-specta.svg?style=flat-square"
    alt="crates.io" /></a>
    <a href="https://docs.rs/tauri-specta"><img src="https://img.shields.io/badge/docs-latest-blue.svg?style=flat-square" alt="docs.rs" /></a>
    <a href="/LICENSE.md"><img src="https://img.shields.io/crates/l/tauri-specta?style=flat-square" alt="License"></a>
</div>

<br>

> This branch contains the code for tauri-specta v2. You can check the [v1.0.2 git tag](https://github.com/oscartbeaumont/tauri-specta/tree/v1.0.2) for the v1 code.

## Install

## Specta v1

```bash
cargo add specta
cargo add tauri-specta --features javascript,typescript
```

## Specta v2

Specta v2 hasn't officially launched yet but it can be used through the release candidate (`rc`) versions.

You must **ensure** you lock your Specta version to avoid breaking changes.

```bash
cargo add specta@=2.0.0-rc.7
cargo add tauri-specta@=2.0.0-rc.4 --features javascript,typescript
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
    pub some_field: String,
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
#[specta::specta] // <-- This bit here
fn greet3() -> MyCustomReturnType {
    MyCustomReturnType {
        some_field: "Hello World".into(),
    }
}

#[tauri::command]
#[specta::specta] // <-- This bit here
fn greet(name: String) -> String {
  format!("Hello {name}!")
}
```

## Export your bindings

```rust
use specta::collect_types;
use tauri_specta::{ts, js};

// this example exports your types on startup when in debug mode. You can do whatever.

fn main() {
    let specta_builder = {
        // You can use `tauri_specta::js::builder` for exporting JS Doc instead of Typescript!`
        let specta_builder = tauri_specta::ts::builder()
            .commands(tauri_specta::collect_commands![greet, greet2, greet3 ]); // <- Each of your comments


        #[cfg(debug_assertions)] // <- Only export on non-release builds
        let specta_builder = specta_builder.path("../src/bindings.ts");

        specta_builder.into_plugin()
    };

    tauri::Builder::default()
        .plugin(specta_builder)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

## Usage on frontend

```ts
import * as commands from "./bindings"; // This should point to the file we export from Rust

await commands.greet("Brendan");
```

## Events

> To use Events you must be using [Specta v2 and Tauri Specta v2](#specta-v2).

Firstly you have to define your event types. You can add as many of these as you want.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct DemoEvent(String);
```

Next you must add it to the builder like the following:

```rust
let specta_builder = ts::builder()
        .events(tauri_specta::collect_events![DemoEvent]); // This should contain all your events.
```

Then it can be used in Rust like the following:

```rust
tauri::Builder::default()
    .setup(|app| {
        let handle = app.handle();

        DemoEvent::listen_global(&handle, |event| {
            dbg!(event.payload);
        });

        DemoEvent("Test".to_string()).emit_all(&handle).unwrap();
    });
```

and it can be used in TS like the following:

```ts
import { commands, events } from "./bindings";
import { appWindow } from "@tauri-apps/api/window";

// For all windows
events.demoEvent.listen((e) => console.log(e));

// For a single window
events.demoEvent(appWindow).listen((e) => console.log(e));

// Emit to the backend and all windows
await events.demoEvent.emit("Test")

// Emit to a window
await events.demoEvent(appWindow).emit("Test")
```


## Known limitations

 - Your command can only take up to 10 arguments. Any more and you'll get a compile error. If you need more just use a struct.
 - Exporting your schema within a directory tracked by Tauri's hot reload will cause an infinite reload loop.

## Development

Run the example:

```bash
pnpm i
cd examples/app/
pnpm dev
```

## Credit

Created by [oscartbeaumont](https://github.com/oscartbeaumont) and [Brendonovich](https://github.com/brendonovich).
