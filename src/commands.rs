use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

use specta::{Types, datatype};
use tauri::{Runtime, ipc::Invoke};

/// The command metadata produced by a [`CommandTypeCollector`].
#[doc(hidden)]
#[derive(Default)]
pub struct CollectedCommands {
    /// The Specta functions for the registered commands. Each function's name is the
    /// effective Tauri command name accepted by [`tauri::generate_handler`].
    pub functions: Vec<datatype::Function>,
    /// Maps the effective Tauri name of a renamed command back to the Rust identifier
    /// its generated frontend accessor is derived from. Empty for commands without a
    /// rename.
    pub accessor_names: BTreeMap<Cow<'static, str>, Cow<'static, str>>,
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
            Arc::new(|types: &mut Types| CollectedCommands {
                functions: ::specta::function::collect_functions![](types),
                accessor_names: BTreeMap::new(),
            }),
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
pub(crate) fn duplicate_command_name(commands: &[datatype::Function]) -> Option<&str> {
    let mut seen = BTreeSet::new();
    for command in commands {
        if !seen.insert(command.name()) {
            return Some(command.name());
        }
    }
    None
}

/// Applies each command's effective Tauri command name to its collected Specta function.
///
/// `#[tauri::command(rename = "...")]` changes the name the invoke handler matches against,
/// while `#[specta::specta]` names the function after its Rust identifier. `renames` pairs
/// the Rust identifier with the name Tauri's hidden `__tauri_command_name_*!()` macro
/// reports, so the generated bindings invoke the name the handler actually accepts.
///
/// The rename is applied by matching on the *original* Specta name, so the order of
/// `renames` can't shift a name onto the wrong function.
///
/// # Panics
///
/// Panics if a renamed command doesn't correspond to exactly one collected function, rather
/// than silently leaving the binding with a name the handler rejects.
#[allow(clippy::panic)]
pub(crate) fn apply_command_renames(
    collected: &mut CollectedCommands,
    renames: &[(&'static str, &'static str)],
) {
    // Snapshot the names so a rename can't match a function an earlier rename already
    // touched.
    let original: Vec<Cow<'static, str>> = collected
        .functions
        .iter()
        .map(|function| function.name.clone())
        .collect();

    for (rust_name, tauri_name) in renames {
        // Without an explicit `rename`, Tauri's name is the Rust identifier and the
        // Specta name already matches.
        if rust_name == tauri_name {
            continue;
        }

        let mut matched = None;
        for (index, name) in original.iter().enumerate() {
            if name == rust_name {
                if matched.is_some() {
                    panic!(
                        "Command '{rust_name}' is registered more than once, so its `#[tauri::command(rename = \"{tauri_name}\")]` can't be applied unambiguously."
                    );
                }
                matched = Some(index);
            }
        }

        match matched {
            Some(index) => {
                collected.functions[index].name = Cow::Borrowed(tauri_name);
                collected
                    .accessor_names
                    .insert(Cow::Borrowed(tauri_name), Cow::Borrowed(rust_name));
            }
            None => panic!(
                "Command '{rust_name}' has `#[tauri::command(rename = \"{tauri_name}\")]`, but no collected command has that name. Make sure the command is also annotated with `#[specta::specta]`."
            ),
        }
    }
}
