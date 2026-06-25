use std::borrow::Cow;

use heck::{
    ToKebabCase, ToLowerCamelCase, ToShoutySnakeCase, ToSnakeCase, ToUpperCamelCase,
};

/// The casing convention applied to generated identifiers in the exported bindings.
///
/// By default Tauri Specta renames Rust `snake_case` identifiers to JavaScript-idiomatic
/// [`Casing::CamelCase`] when generating bindings. This enum lets you override that behavior,
/// for example to keep Rust's original `snake_case` naming.
///
/// This is used by [`Builder::function_casing`](crate::Builder::function_casing) and
/// [`Builder::argument_casing`](crate::Builder::argument_casing).
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Casing {
    /// `camelCase`.
    ///
    /// This is the default and matches the JavaScript naming convention.
    #[default]
    CamelCase,
    /// `PascalCase` (also known as `UpperCamelCase`).
    PascalCase,
    /// `snake_case`.
    ///
    /// This preserves the original Rust naming. Use this for argument names together with
    /// [`#[tauri::command(rename_all = "snake_case")]`](https://docs.rs/tauri/latest/tauri/attr.command.html)
    /// so the generated argument keys match what Tauri expects.
    SnakeCase,
    /// `SCREAMING_SNAKE_CASE`.
    ScreamingSnakeCase,
    /// `kebab-case`.
    KebabCase,
}

impl Casing {
    /// Apply the casing convention to a given identifier.
    pub(crate) fn apply<'a>(&self, ident: &'a str) -> Cow<'a, str> {
        match self {
            Casing::CamelCase => Cow::Owned(ident.to_lower_camel_case()),
            Casing::PascalCase => Cow::Owned(ident.to_upper_camel_case()),
            Casing::SnakeCase => Cow::Owned(ident.to_snake_case()),
            Casing::ScreamingSnakeCase => Cow::Owned(ident.to_shouty_snake_case()),
            Casing::KebabCase => Cow::Owned(ident.to_kebab_case()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_each_casing() {
        assert_eq!(Casing::CamelCase.apply("my_command_name"), "myCommandName");
        assert_eq!(Casing::PascalCase.apply("my_command_name"), "MyCommandName");
        assert_eq!(Casing::SnakeCase.apply("my_command_name"), "my_command_name");
        assert_eq!(Casing::SnakeCase.apply("myCommandName"), "my_command_name");
        assert_eq!(
            Casing::ScreamingSnakeCase.apply("my_command_name"),
            "MY_COMMAND_NAME"
        );
        assert_eq!(Casing::KebabCase.apply("my_command_name"), "my-command-name");
    }

    #[test]
    fn default_is_camel_case() {
        assert_eq!(Casing::default(), Casing::CamelCase);
    }
}
