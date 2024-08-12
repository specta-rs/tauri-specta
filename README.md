<div align="center">
    <img height="150" src=".github/logo.png" alt="Specta Logo"></img>
    <h1>Tauri Specta</h1>
    <p><b>Typesafe Tauri commands</b></p>
    <a href="https://discord.gg/4V9M5sksw8"><img src="https://img.shields.io/discord/1011665225809924136?style=flat-square" alt="Discord"></a>
    <a href="https://crates.io/crates/tauri-specta"><img src="https://img.shields.io/crates/v/tauri-specta.svg?style=flat-square"
    alt="crates.io" /></a>
    <a href="/LICENSE.md"><img src="https://img.shields.io/crates/l/tauri-specta?style=flat-square" alt="License"></a>
</div>

<br>

> [!NOTE]  
> This branch contains tauri-specta v2, you can checkout the git tag [v1.0.2](https://github.com/oscartbeaumont/tauri-specta/tree/v1.0.2) for the previous version.

## Getting Started

First you will need to choose a version:

|           | Tauri v1                             | Tauri v2                             |
| --------- | ------------------------------------ | ------------------------------------ |
| Specta v1 | Tauri Specta v1 [docs](https://docs.rs/tauri-specta/%5E1.0.2/tauri_specta/index.html) | Unsupported                          |
| Specta v2 | Unsupported                          | Tauri Specta v2 [docs](https://docs.rs/tauri-specta/^2.0.0-rc.11/tauri_specta/index.html) |

Tauri Specta v2 also comes with support for generating types for events.

Follow the documentation links above for help getting started.

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
