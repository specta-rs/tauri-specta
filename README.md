# Tauri Specta

Typesafe Tauri Commands.

Warning: This repo is currently just a technical demo.

## Run Example

```bash
cd example/
pnpm tauri dev
```

## Future Work:

 - Tauri event support
 - Support more than 2 arguments on a command. A macro to generate the implementations for up to 16 arguments would be nice.
 - Either publish code as package or merge into Tauri
 - Reexport all the deps that the macro stuff needs.
 - Would be nice for it to be a single macro.
 - Stable OpenAPI support - Currently will crash if your types have generics.
 - Write exports for many different languages. Maybe for support with something like [tauri-sys](https://github.com/JonasKruckenberg/tauri-sys).
 - Move more of the type exporting into Specta. Right now this package uses strings to generate some Typescript which isn't great.
 - Clean up code
 - Proper unit tests