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

This branch contains the code for tauri-specta v2. You can check the [v1.0.2 git tag](https://github.com/oscartbeaumont/tauri-specta/tree/v1.0.2) for the v1 code.

## Getting Started

For most use cases, you'll probably want to use [v1](./docs/v1.md), which is stable.

You can also use [v2](./docs/v2.md) which supports generating types for events and utilises Specta v2,
but both it and Specta v2 are still in development.

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
