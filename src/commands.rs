use std::{borrow::Cow, fmt, future::Future, sync::Arc};

use specta::{
    Type, Types,
    datatype::{DataType, Deprecated},
};
use tauri::{Runtime, ipc::Invoke};

/// Metadata about a Tauri command used for binding generation.
#[derive(Debug, Clone)]
pub struct Command {
    /// The command name passed to Tauri's invoke API.
    pub name: Cow<'static, str>,
    /// Frontend-visible command arguments.
    pub args: Vec<CommandArg>,
    /// The command result type, or `None` for `void`.
    pub result: Option<DataType>,
    /// Rust-doc comments on the command.
    pub docs: Cow<'static, str>,
    /// Rust deprecation metadata on the command.
    pub deprecated: Option<Deprecated>,
}

/// Metadata about a frontend-visible Tauri command argument.
#[derive(Debug, Clone)]
pub struct CommandArg {
    /// The argument name expected by Tauri.
    pub name: Cow<'static, str>,
    /// The frontend type for the argument.
    pub ty: DataType,
}

/// A wrapper around the output of the `collect_commands` macro.
///
/// This acts to seal the implementation details of the macro.
pub struct Commands<R: Runtime>(
    // Bounds copied from `tauri::Builder::invoke_handler`
    pub(crate) Arc<dyn Fn(Invoke<R>) -> bool + Send + Sync + 'static>,
    pub(crate) fn(&mut Types) -> Vec<Command>,
);

impl<R: Runtime> fmt::Debug for Commands<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Commands").finish()
    }
}

impl<R: Runtime> Default for Commands<R> {
    fn default() -> Self {
        fn commands(_: &mut Types) -> Vec<Command> {
            Vec::new()
        }

        Self(Arc::new(tauri::generate_handler![]), commands)
    }
}

impl<R: Runtime> Clone for Commands<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1)
    }
}

#[doc(hidden)]
pub trait CommandSignature<TMarker> {
    type Args;
    type ArgMarkers;
    type Result;
    type ResultMarker;

    fn into_command(self, definition: tauri::ipc::CommandDefinition, types: &mut Types) -> Command
    where
        Self::Args: CommandArguments<Self::ArgMarkers>,
        Self::Result: CommandResult<Self::ResultMarker>;
}

#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "one or more Tauri command arguments cannot be exported by specta",
    note = "derive or implement `specta::Type` for frontend-visible command argument types",
    note = "Tauri-injected arguments like `State`, `AppHandle`, `Window`, `Webview`, and `WebviewWindow` are ignored"
)]
pub trait CommandArguments<TMarker> {
    fn to_datatypes(types: &mut Types) -> Vec<Option<DataType>>;
}

#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be exported as a Tauri command argument",
    label = "this command argument is missing `specta::Type`",
    note = "derive or implement `specta::Type` for `{Self}`"
)]
pub trait CommandArgument<TMarker> {
    fn to_datatype(types: &mut Types) -> Option<DataType>;
}

#[doc(hidden)]
pub enum SpectaArgumentMarker {}

impl<T: Type> CommandArgument<SpectaArgumentMarker> for T {
    fn to_datatype(types: &mut Types) -> Option<DataType> {
        Some(T::definition(types))
    }
}

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

#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be exported as a Tauri command result",
    label = "this command result is missing `specta::Type`",
    note = "derive or implement `specta::Type` for `{Self}`"
)]
pub trait CommandResult<TMarker> {
    fn to_datatype(types: &mut Types) -> Option<DataType>;
}

#[doc(hidden)]
pub enum SpectaResultMarker {}

impl<T: Type> CommandResult<SpectaResultMarker> for T {
    fn to_datatype(types: &mut Types) -> Option<DataType> {
        top_level_unit_as_void(T::definition(types))
    }
}

#[doc(hidden)]
pub enum FutureResultMarker {}

impl<F> CommandResult<FutureResultMarker> for F
where
    F: Future,
    F::Output: Type,
{
    fn to_datatype(types: &mut Types) -> Option<DataType> {
        top_level_unit_as_void(F::Output::definition(types))
    }
}

fn top_level_unit_as_void(dt: DataType) -> Option<DataType> {
    match &dt {
        DataType::Tuple(tuple) if tuple.elements().is_empty() => None,
        _ => Some(dt),
    }
}

fn deprecated_from_definition(
    deprecated: Option<tauri::ipc::CommandDefinitionDeprecated>,
) -> Option<Deprecated> {
    deprecated.map(|deprecated| {
        let mut out = Deprecated::new();
        out.set_note(deprecated.note.map(Cow::Borrowed));
        out.set_since(deprecated.since.map(Cow::Borrowed));
        out
    })
}

fn command_from_parts(
    definition: tauri::ipc::CommandDefinition,
    types: &mut Types,
    args: impl IntoIterator<Item = Option<DataType>>,
    result: Option<DataType>,
) -> Command {
    let mut names = definition.arguments.iter();
    let args = args
        .into_iter()
        .filter_map(|ty| ty.map(|ty| (names.next(), ty)))
        .filter_map(|(name, ty)| {
            name.map(|name| CommandArg {
                name: Cow::Borrowed(*name),
                ty,
            })
        })
        .collect();

    let _ = types;

    Command {
        name: Cow::Borrowed(definition.name),
        args,
        result,
        docs: Cow::Borrowed(definition.docs),
        deprecated: deprecated_from_definition(definition.deprecated),
    }
}

macro_rules! impl_command_signature {
    () => {
        impl CommandArguments<()> for () {
            fn to_datatypes(_: &mut Types) -> Vec<Option<DataType>> {
                Vec::new()
            }
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
                definition: tauri::ipc::CommandDefinition,
                types: &mut Types,
            ) -> Command
            where
                Self::Args: CommandArguments<Self::ArgMarkers>,
                Self::Result: CommandResult<Self::ResultMarker>,
            {
                let _ = self;
                let result = Self::Result::to_datatype(types);
                command_from_parts(definition, types, [], result)
            }
        }
    };
    ($($ty:ident : $marker:ident),+) => {
        #[allow(non_snake_case)]
        impl<$($ty, $marker),+> CommandArguments<($($marker,)+)> for ($($ty,)+)
        where
            $($ty: CommandArgument<$marker>,)+
        {
            fn to_datatypes(types: &mut Types) -> Vec<Option<DataType>> {
                vec![$($ty::to_datatype(types)),+]
            }
        }

        #[allow(non_snake_case)]
        impl<F, TResult, TResultMarker, $($ty, $marker),+> CommandSignature<($($ty, $marker,)+ TResult, TResultMarker)> for F
        where
            for<'a> &'a F: Fn($($ty),+) -> TResult,
        {
            type Args = ($($ty,)+);
            type ArgMarkers = ($($marker,)+);
            type Result = TResult;
            type ResultMarker = TResultMarker;

            fn into_command(
                self,
                definition: tauri::ipc::CommandDefinition,
                types: &mut Types,
            ) -> Command
            where
                Self::Args: CommandArguments<Self::ArgMarkers>,
                Self::Result: CommandResult<Self::ResultMarker>,
            {
                let _ = self;
                let args = Self::Args::to_datatypes(types);
                let result = Self::Result::to_datatype(types);
                command_from_parts(definition, types, args, result)
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
