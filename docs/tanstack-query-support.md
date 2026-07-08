# TanStack Query Support Plan

## Goal

Add TanStack Query support through the separate `tauri-specta-query` crate, while keeping `tauri-specta` focused on Tauri command/event/type metadata and normal bindings export.

The query crate should be able to generate a bindings file shaped like:

```ts
export const queries = {
  someQuery: {
    queryKey: (...args) => [...],
    queryFn: (...args) => ...,
  },
};

export const mutations = {
  someMutation: {
    mutationKey: (...args) => [...],
    mutationFn: (...args) => ...,
  },
};
```

instead of the core shape:

```ts
export const commands = {
  someCommand: (...args) => ...,
};
```

The generated query bindings should still share all of the hard parts already solved by `tauri-specta`: Tauri command names, plugin command names, argument casing, serde serialize/deserialize phases, semantic runtime transforms, channels, typed errors, events, constants, and type emission.

## Recommended Architecture

Keep `tauri-specta-query` as a thin extension crate that depends on primitives exported from `tauri-specta`.

Core responsibilities:

- Collect command invoke handlers.
- Collect command metadata as `specta::datatype::Function`.
- Collect events, constants, manual types, plugin name, serde phase config, semantic config, and error handling mode.
- Provide reusable TypeScript/JSDoc command rendering primitives.
- Provide a way for extension crates to export using the same `BuilderConfiguration`.

Query crate responsibilities:

- Define a query-aware command grouping API:

```rust
pub struct CommandSet<R: Runtime> {
    queries: Commands<R>,
    mutations: Commands<R>,
    events: Events,
    types: Types,
    constants: BTreeMap<Cow<'static, str>, serde_json::Value>,
}
```

- Merge query and mutation invoke handlers into one Tauri handler.
- Register query and mutation function metadata into the shared type registry.
- Export a custom TypeScript/JSDoc runtime that renders `queries` and `mutations`.
- Own all TanStack-specific API names and helper shapes.

## Core Changes

### 1. Expose `Commands` composition primitives

`tauri-specta-query` needs to combine two `Commands<R>` values into a single invoke handler while preserving both metadata lists separately.

Add public methods on `Commands<R>`:

```rust
impl<R: Runtime> Commands<R> {
    pub fn invoke_handler(&self) -> Arc<dyn Fn(Invoke<R>) -> bool + Send + Sync + 'static>;
    pub fn functions(&self, types: &mut Types) -> Vec<Function>;
}
```

Optionally add:

```rust
pub fn merge(self, other: Self) -> Self;
```

`merge` should call the first handler and, when it returns `false`, call the second handler. Metadata should concatenate in registration order.

This keeps the tuple fields private and avoids forcing extension crates to know how `collect_commands!` is implemented.

### 2. Expose `BuilderConfiguration` access safely

Extension exporters need the same config that core exporters receive. Add:

```rust
impl<R: Runtime> Builder<R> {
    pub fn configuration(&self) -> &BuilderConfiguration;
    pub fn into_configuration(self) -> BuilderConfiguration;
}
```

This is mainly useful for crates that want to reuse a built `Builder` as input to a custom exporter. The existing `LanguageExt` path can remain unchanged.

### 3. Add command metadata registration without replacing invoke handlers

Today `Builder::commands(commands)` updates both the runtime invoke handler and `cfg.commands`. The query crate needs to register combined command metadata while also keeping query/mutation groups.

Add a low-level method:

```rust
impl<R: Runtime> Builder<R> {
    pub fn command_functions(mut self, commands: impl IntoIterator<Item = Function>) -> Self;
}
```

or a more constrained helper:

```rust
pub fn extend_command_functions(mut self, commands: &[Function]) -> Self;
```

The query crate can then build a normal `Builder` with merged invoke handling and all command metadata, while its exporter uses separate query/mutation metadata stored outside core.

### 4. Extract TypeScript command rendering into reusable primitives

The current command rendering lives inline in `src/lang/js_ts.rs`. Extract the command-specific render result into a public, feature-gated module, for example:

```rust
#[cfg(any(feature = "javascript", feature = "typescript"))]
pub mod typescript {
    pub struct RuntimeUsage {
        pub invoke: bool,
        pub channel: bool,
        pub typed_error: bool,
        pub channel_transform: bool,
    }

    pub struct RenderedCommand {
        pub property_name: String,
        pub tauri_name: String,
        pub function_expression: String,
        pub docs: Cow<'static, str>,
        pub deprecated: Option<Cow<'static, str>>,
        pub runtime_usage: RuntimeUsage,
    }

    pub fn render_command(
        exporter: &FrameworkExporter,
        cfg: &BuilderConfiguration,
        command: &Function,
        jsdoc: bool,
    ) -> Result<RenderedCommand, specta_typescript::Error>;
}
```

The exact type can be adjusted to avoid exposing too much of `specta_typescript`, but the important primitive is: given a command `Function`, produce the TypeScript function expression that core currently places inside `commands.foo`.

Then core's existing exporter becomes a consumer of that primitive:

```ts
export const commands = {
  foo: <rendered function expression>,
};
```

The query crate can consume the same primitive differently:

```ts
export const queries = {
  foo: {
    queryKey: (...args) => ["foo", ...args],
    queryFn: (...args) => <rendered function expression called with args>,
  },
};
```

This is the most important core change. Without it, `tauri-specta-query` has to copy the largest and most fragile part of `js_ts.rs`.

### 5. Expose shared TypeScript runtime rendering

The query exporter also needs the same imports and helper functions as core:

- `invoke as __TAURI_INVOKE`
- `Channel`
- event imports
- `typedError`
- `mapChannel`
- `makeEvent`
- semantic type runtime handling

Extract a render context from `js_ts.rs` that can be reused by both exporters:

```rust
pub struct TypescriptRuntimeContext<'a> {
    pub cfg: &'a BuilderConfiguration,
    pub jsdoc: bool,
    pub runtime_usage: RuntimeUsage,
}
```

Then provide helpers for:

- imports
- events block
- constants block
- user types block
- runtime helper block

This can initially be `pub(crate)` plus a narrow public facade for extension crates. The public facade should not expose internal helper function names like `extract_std_result` unless an extension crate genuinely needs them.

### 6. Add an extension exporter hook

There are two viable shapes:

Option A: let `tauri-specta-query` implement `LanguageExt` for its own language wrapper.

```rust
pub struct TanStackQueryTypescript {
    inner: specta_typescript::Typescript,
    queries: Vec<Function>,
    mutations: Vec<Function>,
}

impl LanguageExt for TanStackQueryTypescript {
    type Error = specta_typescript::Error;

    fn export(self, cfg: &BuilderConfiguration, path: &Path) -> Result<(), Self::Error> {
        // use core TS primitives
    }
}
```

Option B: add a core `ExportContext` trait/API and let extensions call that directly.

Option A is preferable because it fits the current `Builder::export(language, path)` model and does not require core to know about TanStack Query.

## Query Crate API Sketch

Use a command set as the Rust-side grouping primitive:

```rust
let command_set = tauri_specta_query::CommandSet::new()
    .queries(collect_commands![get_user, list_projects])
    .mutations(collect_commands![create_project, delete_project])
    .events(collect_events![ProjectUpdated])
    .typ::<SharedType>()
    .constant("apiVersion", 1);

let builder = command_set.builder()
    .semantic_types(semantic::Configuration::default());

#[cfg(debug_assertions)]
builder.export(
    tauri_specta_query::Typescript::new(command_set),
    "../src/bindings.ts",
)?;

tauri::Builder::default()
    .invoke_handler(builder.invoke_handler());
```

There is a small ownership wrinkle here: both the `Builder` and query exporter need command metadata. The cleanest implementation is for `CommandSet` to be `Clone`, with `Commands<R>` already cloneable.

An alternate API is:

```rust
let (builder, query_exporter) = command_set.into_builder_and_exporter();
builder.export(query_exporter.typescript(), "../src/bindings.ts")?;
```

That avoids accidental mismatch between the builder and exporter.

## Generated TypeScript Shape

A good first version should export option factories, not React hooks. That keeps `tauri-specta-query` framework-adjacent rather than React-only:

```ts
export const queries = {
  user: (id: string) => queryOptions({
    queryKey: ["user", id],
    queryFn: () => __TAURI_INVOKE<User>("user", { id }),
  }),
};

export const mutations = {
  createUser: () => mutationOptions({
    mutationKey: ["createUser"],
    mutationFn: (input: CreateUserInput) => __TAURI_INVOKE<User>("create_user", { input }),
  }),
};
```

This supports:

```ts
useQuery(queries.user("1"));
useMutation(mutations.createUser());
```

The generated file can import from `@tanstack/query-core` or `@tanstack/react-query`. Prefer `@tanstack/query-core` if `mutationOptions` and `queryOptions` are available for the desired version; otherwise make the package target configurable.

## Why Alternatives Do Not Work Well

### Put TanStack Query directly in `tauri-specta`

This would work technically, but it couples core to a frontend cache library. Core would need TanStack dependency/version opinions, React/Solid/Vue naming questions, and query/mutation semantics. That would expand the core API around one frontend integration and make future integrations harder to keep separate.

### Make users post-process generated `commands`

A frontend wrapper can turn `commands.foo` into `queryOptions`, but it loses the ability to generate stable query keys and mutation option factories from the same command metadata. It also cannot reliably preserve serde runtime transforms, typed errors, channel transforms, deprecation docs, and JSDoc without parsing generated TypeScript.

### Duplicate `src/lang/js_ts.rs` in `tauri-specta-query`

This is the fastest prototype, but it will drift immediately. The duplicated code would need to track serde phase handling, semantic transforms, typed error behavior, channel support, constants, event helpers, and future exporter changes. Any bug fixed in core would need a second fix in the query crate.

### Use constants to smuggle query/mutation grouping into core export

Constants can expose data, but they cannot change the shape of generated command functions. The special bindings file needs `const queries` and `const mutations`; constants would still leave `commands` as the only generated invoke surface.

### Add query/mutation fields to `BuilderConfiguration`

This preserves grouping, but it puts TanStack concepts into the core configuration. It also creates ambiguous behavior for non-query exporters: should normal TypeScript export `commands`, `queries`, all three, or only grouped commands? Keeping grouping in `tauri-specta-query` avoids that ambiguity.

### Add only a custom template string hook

A template hook could replace `commands` with arbitrary text, but the hard part is not the object literal wrapper. The hard part is rendering each command correctly with phases, semantic transforms, channels, typed errors, docs, and imports. A template hook would either be too weak or become an untyped reimplementation of the exporter API.

## Implementation Order

1. Add read-only `Commands` accessors and optional `Commands::merge`.
2. Add `Builder` configuration accessors.
3. Extract command rendering from `js_ts.rs` behind the TypeScript/JSDoc feature flags.
4. Refactor the existing core exporter to use the extracted renderer without changing generated output.
5. Add snapshot-style tests to prove normal `commands` output is unchanged.
6. Implement `tauri-specta-query::CommandSet` with queries, mutations, events, manual types, and constants.
7. Implement the query TypeScript exporter using the core render primitives.
8. Update the TanStack Query example to export `queries` and `mutations`.
9. Add generated binding snapshots for the query example.

## Acceptance Criteria

- `tauri-specta` has no TanStack-specific API names.
- The normal `commands` bindings output remains unchanged.
- `tauri-specta-query` can generate `queries` and `mutations` in a single special bindings file.
- Query/mutation generated functions preserve command docs, deprecation metadata, plugin names, typed error handling, semantic type transforms, and channel transforms.
- The Tauri invoke handler still receives all query and mutation commands.
- Extension crates do not need to copy `src/lang/js_ts.rs`.
