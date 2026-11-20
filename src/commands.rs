use std::{borrow::Cow, collections::BTreeSet, fmt, sync::Arc};

use std::future::Future;

use specta::Type;
use specta::{
    Types,
    datatype::{DataType, Deprecated, Reference},
};
use tauri::{
    Runtime,
    ipc::{CommandScope, GlobalScope, Invoke, ScopeObject},
};

/// A command as seen by the binding exporters.
///
/// Produced by `collect_commands!` and exposed through
/// [`BuilderConfiguration::commands`](crate::BuilderConfiguration::commands).
#[derive(Debug, Clone)]
pub struct Command {
    /// The command name Tauri's invoke handler accepts, after
    /// `#[tauri::command(rename = "...")]`. The plugin prefix is added at export time.
    pub name: Cow<'static, str>,
    /// The Rust identifier the frontend accessor is derived from.
    pub accessor: Cow<'static, str>,
    /// The arguments the frontend sends, in declaration order.
    pub args: Vec<CommandArg>,
    /// The command result, or `None` when it resolves to `void`.
    pub result: Option<DataType>,
    /// Rust-doc comments on the command.
    pub docs: Cow<'static, str>,
    /// Rust deprecation metadata on the command.
    pub deprecated: Option<Deprecated>,
}

/// A frontend-visible command argument.
#[derive(Debug, Clone)]
pub struct CommandArg {
    /// The key Tauri reads this argument from, after `rename_all`.
    pub name: Cow<'static, str>,
    /// The frontend type of the argument.
    pub ty: DataType,
}

/// The command metadata produced by a [`CommandTypeCollector`].
#[doc(hidden)]
#[derive(Default)]
pub struct CollectedCommands {
    /// The registered commands, in registration order.
    pub commands: Vec<Command>,
}

impl CollectedCommands {
    /// An empty collection with room for `capacity` commands.
    ///
    /// `collect_commands!` knows the command count at compile time, so it can size the
    /// vector once instead of growing it while pushing each command.
    #[doc(hidden)]
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            commands: Vec::with_capacity(capacity),
        }
    }
}

/// Type-erased collector used by [`Commands`] to register command metadata.
#[doc(hidden)]
pub type CommandTypeCollector = dyn Fn(&mut Types) -> CollectedCommands + Send + Sync + 'static;

/// A wrapper around the output of the `collect_commands` macro.
///
/// This acts to seal the implementation details of the macro.
pub struct Commands<R: Runtime>(
    // TODO: Explain these being public
    // Bounds copied from `tauri::Builder::invoke_handler`
    pub Arc<dyn Fn(Invoke<R>) -> bool + Send + Sync + 'static>,
    pub Arc<CommandTypeCollector>,
);

impl<R: Runtime> fmt::Debug for Commands<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Commands").finish()
    }
}

impl<R: Runtime> Default for Commands<R> {
    fn default() -> Self {
        Self(
            Arc::new(tauri::generate_handler![]),
            Arc::new(|_: &mut Types| CollectedCommands::default()),
        )
    }
}

impl<R: Runtime> Clone for Commands<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

/// The first command name registered more than once, if any.
///
/// Checked when commands are registered rather than when bindings are exported, so the
/// mistake is caught in release builds too — `export` is usually `#[cfg(debug_assertions)]`.
/// Collisions between *differently* named commands are a separate, export-time check,
/// because the accessor they share depends on the final `function_casing`.
pub(crate) fn duplicate_command_name(commands: &[Command]) -> Option<&str> {
    let mut seen = BTreeSet::new();
    for command in commands {
        if !seen.insert(command.name.as_ref()) {
            return Some(command.name.as_ref());
        }
    }
    None
}

/// A Tauri command whose frontend types are inferred from its Rust signature.
///
/// Implemented for function references that Tauri's `#[tauri::command]` macro accepts,
/// so commands only need `#[tauri::command]`. Every argument the frontend sends, and
/// the result, must implement `specta::Type`, and a command may take at most ten
/// arguments. The marker parameter keeps the argument-count and result-kind
/// implementations from overlapping.
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "this function cannot be collected as a Tauri command",
    label = "not a supported command signature",
    note = "commands may take at most ten arguments",
    note = "commands must be free functions whose argument and result types implement `specta::Type`"
)]
pub trait CommandSignature<TMarker> {
    /// The command's argument tuple.
    type Args;
    /// Marker types selecting an argument implementation per position.
    type ArgMarkers;
    /// The command's declared result type.
    type Result;
    /// Marker selecting a synchronous or asynchronous result implementation.
    type ResultMarker;

    /// Convert the command into binding metadata using Tauri's command metadata.
    fn into_command(
        self,
        metadata: tauri::ipc::CommandMetadata,
        accessor: Cow<'static, str>,
        types: &mut Types,
        arg_types: &mut Vec<Option<DataType>>,
    ) -> Command
    where
        Self::Args: CommandArguments<Self::ArgMarkers>,
        Self::Result: CommandResult<Self::ResultMarker>;
}

/// All of a command's arguments, as a tuple.
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "one or more Tauri command arguments cannot be exported by specta",
    note = "derive or implement `specta::Type` for every argument the frontend sends",
    note = "`State`, `AppHandle`, `Window`, `Webview`, `WebviewWindow`, `CommandScope` and `GlobalScope` are injected by Tauri and are not sent from the frontend"
)]
pub trait CommandArguments<TMarker> {
    /// Append one entry per argument, in declaration order. `None` marks an injected
    /// argument.
    ///
    /// Entries are appended to the caller's buffer rather than returned in a fresh
    /// `Vec` so the buffer can be reused across every command in a collection.
    fn to_datatypes(types: &mut Types, out: &mut Vec<Option<DataType>>);
}

/// A single command argument.
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be exported as a Tauri command argument",
    label = "this argument is missing `specta::Type`",
    note = "derive or implement `specta::Type` for `{Self}`",
    note = "if Tauri injects this argument instead of reading it from the invoke payload, please report it: tauri-specta does not know about it"
)]
pub trait CommandArgument<TMarker> {
    /// `None` when Tauri injects the argument rather than sending it from the frontend.
    fn to_datatype(types: &mut Types) -> Option<DataType>;
}

/// Marker for arguments the frontend sends.
#[doc(hidden)]
pub enum SpectaArgumentMarker {}

impl<T: Type> CommandArgument<SpectaArgumentMarker> for T {
    fn to_datatype(types: &mut Types) -> Option<DataType> {
        Some(T::definition(types))
    }
}

/// Marker for arguments Tauri injects.
#[doc(hidden)]
pub enum InjectedArgumentMarker {}

impl<T: Send + Sync + 'static> CommandArgument<InjectedArgumentMarker> for tauri::State<'_, T> {
    fn to_datatype(_: &mut Types) -> Option<DataType> {
        None
    }
}

impl<R: Runtime> CommandArgument<InjectedArgumentMarker> for tauri::AppHandle<R> {
    fn to_datatype(_: &mut Types) -> Option<DataType> {
        None
    }
}

impl<R: Runtime> CommandArgument<InjectedArgumentMarker> for tauri::Window<R> {
    fn to_datatype(_: &mut Types) -> Option<DataType> {
        None
    }
}

impl<R: Runtime> CommandArgument<InjectedArgumentMarker> for tauri::Webview<R> {
    fn to_datatype(_: &mut Types) -> Option<DataType> {
        None
    }
}

impl<R: Runtime> CommandArgument<InjectedArgumentMarker> for tauri::WebviewWindow<R> {
    fn to_datatype(_: &mut Types) -> Option<DataType> {
        None
    }
}

impl<T: ScopeObject> CommandArgument<InjectedArgumentMarker> for CommandScope<T> {
    fn to_datatype(_: &mut Types) -> Option<DataType> {
        None
    }
}

impl<T: ScopeObject> CommandArgument<InjectedArgumentMarker> for GlobalScope<T> {
    fn to_datatype(_: &mut Types) -> Option<DataType> {
        None
    }
}

/// A command's declared result type.
///
/// The diagnostic deliberately avoids `{Self}` in its notes. For an `async fn` the
/// failing type is the opaque future, so advice phrased as "implement `specta::Type`
/// for `{Self}`" would ask the author to implement a trait for `impl Future<...>`.
/// The type they actually have to fix is the future's output, which rustc prints
/// after `Output =`.
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "this command's result cannot be described for the generated bindings",
    label = "needs a `specta::Type` impl",
    note = "add `#[derive(specta::Type)]`, or implement `specta::Type` by hand",
    note = "`serde::Serialize` only lets Tauri send the value; tauri-specta also needs the type's shape to write the bindings",
    note = "for an `async fn`, the type to fix is the one after `Output =`"
)]
pub trait CommandResult<TMarker> {
    /// `None` for a result that is not sent to the frontend (a top-level unit).
    fn to_datatype(types: &mut Types) -> Option<DataType>;
}

/// Marker for a synchronous result.
#[doc(hidden)]
pub enum SpectaResultMarker {}

impl<T: Type> CommandResult<SpectaResultMarker> for T {
    fn to_datatype(types: &mut Types) -> Option<DataType> {
        top_level_unit_as_void(T::definition(types))
    }
}

/// Marker for an `async fn` result, whose value is the future's output.
#[doc(hidden)]
pub enum FutureResultMarker {}

impl<F> CommandResult<FutureResultMarker> for F
where
    F: Future,
    F::Output: AsyncCommandResult,
{
    fn to_datatype(types: &mut Types) -> Option<DataType> {
        F::Output::to_datatype(types)
    }
}

/// The awaited output of an `async fn` command.
///
/// This exists separately from [`CommandResult`] because the marker parameter picks
/// between the synchronous and asynchronous shapes before the output is involved.
///
/// It has no `#[diagnostic::on_unimplemented]` on purpose. rustc reports the
/// outermost failing obligation, so a missing `specta::Type` here always surfaces as
/// `impl Future<Output = T>: CommandResult<_>` and never reaches this trait. The
/// diagnostic on [`CommandResult`] is the one that has to explain the fix.
#[doc(hidden)]
pub trait AsyncCommandResult {
    /// `None` for a top-level unit, which Tauri sends as `void`.
    fn to_datatype(types: &mut Types) -> Option<DataType>;
}

impl<T: Type> AsyncCommandResult for T {
    fn to_datatype(types: &mut Types) -> Option<DataType> {
        top_level_unit_as_void(T::definition(types))
    }
}

fn top_level_unit_as_void(dt: DataType) -> Option<DataType> {
    match &dt {
        DataType::Tuple(tuple) if tuple.elements.is_empty() => None,
        _ => Some(dt),
    }
}

fn command_from_parts(
    metadata: tauri::ipc::CommandMetadata,
    accessor: Cow<'static, str>,
    arg_types: &mut Vec<Option<DataType>>,
    result: Option<DataType>,
    types: &Types,
) -> Command {
    // `metadata.arguments` lists every argument, including the ones Tauri injects, so the
    // types are zipped with the names before any are dropped. Draining leaves the
    // buffer's allocation in place for the next command in the collection.
    let mut args = Vec::with_capacity(metadata.arguments.len());
    for (name, ty) in metadata.arguments.iter().zip(arg_types.drain(..)) {
        let Some(ty) = ty else {
            continue;
        };
        let name = if !name.is_empty() {
            *name
        } else if let DataType::Reference(Reference::Named(reference)) = &ty
            && types.get(reference).is_some_and(|ndt| {
                ndt.name == "TAURI_REQUEST" && ndt.module_path.starts_with("tauri::")
            })
        {
            // Tauri supplies the whole request regardless of the argument pattern.
            // This is a frontend parameter name, not a key in the IPC payload.
            "request"
        } else {
            continue;
        };
        args.push(CommandArg {
            name: Cow::Borrowed(name),
            ty,
        });
    }

    Command {
        name: Cow::Borrowed(metadata.name),
        accessor,
        args,
        result,
        docs: Cow::Borrowed(metadata.docs),
        deprecated: metadata.deprecated.map(|deprecated| {
            let mut out = Deprecated::new();
            out.note = deprecated.note.map(Cow::Borrowed);
            out.since = deprecated.since.map(Cow::Borrowed);
            out
        }),
    }
}

macro_rules! impl_command_signature {
    () => {
        impl CommandArguments<()> for () {
            fn to_datatypes(_: &mut Types, _: &mut Vec<Option<DataType>>) {}
        }

        impl<F, TResult, TResultMarker> CommandSignature<(TResult, TResultMarker)> for F
        where
            for<'a> &'a F: Fn() -> TResult,
        {
            type Args = ();
            type ArgMarkers = ();
            type Result = TResult;
            type ResultMarker = TResultMarker;

            fn into_command(
                self,
                metadata: tauri::ipc::CommandMetadata,
                accessor: Cow<'static, str>,
                types: &mut Types,
                arg_types: &mut Vec<Option<DataType>>,
            ) -> Command
            where
                Self::Args: CommandArguments<Self::ArgMarkers>,
                Self::Result: CommandResult<Self::ResultMarker>,
            {
                let _ = self;
                arg_types.clear();
                let result = Self::Result::to_datatype(types);
                command_from_parts(metadata, accessor, arg_types, result, types)
            }
        }
    };
    ($($ty:ident : $marker:ident),+) => {
        impl<$($ty, $marker),+> CommandArguments<($($marker,)+)> for ($($ty,)+)
        where
            $($ty: CommandArgument<$marker>,)+
        {
            fn to_datatypes(types: &mut Types, out: &mut Vec<Option<DataType>>) {
                out.extend([$($ty::to_datatype(types)),+]);
            }
        }

        impl<F, TResult, TResultMarker, $($ty, $marker),+>
            CommandSignature<($($ty, $marker,)+ TResult, TResultMarker)> for F
        where
            for<'a> &'a F: Fn($($ty),+) -> TResult,
        {
            type Args = ($($ty,)+);
            type ArgMarkers = ($($marker,)+);
            type Result = TResult;
            type ResultMarker = TResultMarker;

            fn into_command(
                self,
                metadata: tauri::ipc::CommandMetadata,
                accessor: Cow<'static, str>,
                types: &mut Types,
                arg_types: &mut Vec<Option<DataType>>,
            ) -> Command
            where
                Self::Args: CommandArguments<Self::ArgMarkers>,
                Self::Result: CommandResult<Self::ResultMarker>,
            {
                let _ = self;
                arg_types.clear();
                Self::Args::to_datatypes(types, arg_types);
                let result = Self::Result::to_datatype(types);
                command_from_parts(metadata, accessor, arg_types, result, types)
            }
        }
    };
}

impl_command_signature!();
impl_command_signature!(T1: M1);
impl_command_signature!(T1: M1, T2: M2);
impl_command_signature!(T1: M1, T2: M2, T3: M3);
impl_command_signature!(T1: M1, T2: M2, T3: M3, T4: M4);
impl_command_signature!(T1: M1, T2: M2, T3: M3, T4: M4, T5: M5);
impl_command_signature!(T1: M1, T2: M2, T3: M3, T4: M4, T5: M5, T6: M6);
impl_command_signature!(T1: M1, T2: M2, T3: M3, T4: M4, T5: M5, T6: M6, T7: M7);
impl_command_signature!(T1: M1, T2: M2, T3: M3, T4: M4, T5: M5, T6: M6, T7: M7, T8: M8);
impl_command_signature!(T1: M1, T2: M2, T3: M3, T4: M4, T5: M5, T6: M6, T7: M7, T8: M8, T9: M9);
impl_command_signature!(T1: M1, T2: M2, T3: M3, T4: M4, T5: M5, T6: M6, T7: M7, T8: M8, T9: M9, T10: M10);
