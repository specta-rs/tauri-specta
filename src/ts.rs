use std::{borrow::Cow, path::Path};

use specta::{functions::FunctionDataType, ts::TsExportError, ExportError, TypeDefs};

use crate::ExportLanguage;

/// Building blocks for [`export`] and [`export_with_cfg`].
///
/// These are made available for advanced use cases where you may combine Tauri Specta with another
/// Specta-enabled library.
pub mod internal {
    use heck::ToLowerCamelCase;
    use indoc::formatdoc;

    use crate::DO_NOT_EDIT;
    use specta::{
        functions::{FunctionDataType, SpectaFunctionResultVariant},
        ts::{self, TsExportError},
        TypeDefs,
    };

    use super::ExportConfiguration;
    /// Type definitions and constants that the generated functions rely on
    pub fn globals() -> String {
        formatdoc! {
            r#"
            declare global {{
                interface Window {{
                    __TAURI_INVOKE__(cmd: string, args?: Record<string, unknown>): Promise<any>;
                }}
            }}

            // Function avoids 'window not defined' in SSR
            const invoke = () => window.__TAURI_INVOKE__;"#
        }
    }

    /// Renders a collection of [`FunctionDataType`] into a TypeScript string.
    pub fn render_functions(
        (function_types, type_map): (Vec<FunctionDataType>, TypeDefs),
        cfg: &ExportConfiguration,
    ) -> Result<String, TsExportError> {
        function_types
            .into_iter()
            .map(|function| {
                let docs = specta::ts::js_doc(&function.docs);

                let name_camel = function.name.to_lower_camel_case();

                let arg_defs = function
                    .args
                    .iter()
                    .map(|(name, typ)| {
                        ts::datatype(&cfg.inner, typ, &type_map)
                            .map(|ty| format!("{}: {}", name.to_lower_camel_case(), ty))
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ");

                let ret_type = match &function.result {
                    SpectaFunctionResultVariant::Value(t) => {
                        ts::datatype(&cfg.inner, t, &type_map)?
                    }
                    SpectaFunctionResultVariant::Result(t, e) => {
                        format!(
                            "[{}, undefined] | [undefined, {}]",
                            ts::datatype(&cfg.inner, t, &type_map)?,
                            ts::datatype(&cfg.inner, e, &type_map)?
                        )
                    }
                };

                let body = {
                    let name = if let Some(plugin_name) = &cfg.plugin_name {
                        format!("plugin:{}|{}", plugin_name, function.name)
                    } else {
                        function.name.to_string()
                    };

                    let arg_usages = function
                        .args
                        .iter()
                        .map(|(name, _)| name.to_lower_camel_case())
                        .collect::<Vec<_>>();

                    let arg_usages = arg_usages
                        .is_empty()
                        .then(Default::default)
                        .unwrap_or_else(|| format!(", {{ {} }}", arg_usages.join(",")));

                    let invoke = format!("await invoke()(\"{name}\"{arg_usages})");

                    match &function.result {
                        SpectaFunctionResultVariant::Value(_) => format!("return {invoke};"),
                        SpectaFunctionResultVariant::Result(_, _) => formatdoc!(
                            r#"
                            try {{
                                return [{invoke}, undefined];
                            }} catch (e: any) {{
                                if(e instanceof Error) throw e;
                                else return [undefined, e];
                            }}"#
                        ),
                    }
                };

                Ok(formatdoc!(
                    r#"
                    {docs}export async function {name_camel}({arg_defs}): Promise<{ret_type}> {{
                    {body}
                    }}"#
                ))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.join("\n\n"))
    }

    /// Renders the output of [`globals`], [`render_functions`] and all dependant types into a TypeScript string.
    pub fn render(
        macro_data: (Vec<FunctionDataType>, TypeDefs),
        cfg: &ExportConfiguration,
    ) -> Result<String, TsExportError> {
        let globals = globals();

        let dependant_types = macro_data
            .1
            .values()
            .filter_map(|v| v.as_ref())
            .map(|v| ts::export_datatype(&cfg.inner, v, &macro_data.1))
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.join("\n"))?;

        let functions = render_functions(macro_data, cfg)?;

        Ok(formatdoc! {
            r#"
                {DO_NOT_EDIT}

                {globals}

                {functions}

                {dependant_types}
            "#
        })
    }
}

/// The configuration for the generator
#[derive(Default)]
pub struct ExportConfiguration {
    /// The name of the plugin to invoke.
    ///
    /// If there is no plugin name (i.e. this is an app), this should be `None`.
    pub(crate) plugin_name: Option<Cow<'static, str>>,
    /// The specta export configuration
    pub(crate) inner: specta::ts::ExportConfiguration,
}

impl ExportConfiguration {
    /// Creates a new [`ExportConfiguration`] from a [`specta::ts::ExportConfiguration`]
    pub fn new(specta_config: specta::ts::ExportConfiguration) -> Self {
        Self {
            inner: specta_config,
            ..Default::default()
        }
    }

    /// Sets the plugin name for this [`ExportConfiguration`].
    pub fn plugin_name(mut self, plugin_name: impl Into<Cow<'static, str>>) -> Self {
        self.plugin_name = Some(plugin_name.into());
        self
    }
}

impl From<specta::ts::ExportConfiguration> for ExportConfiguration {
    fn from(spectra_config: specta::ts::ExportConfiguration) -> Self {
        Self {
            inner: spectra_config,
            ..Default::default()
        }
    }
}

/// Implements [`ExportLanguage`] for TypeScript exporting
pub struct Language;

/// [`Exporter`](crate::Exporter) for TypeScript
pub type Exporter = crate::Exporter<Language>;

impl ExportLanguage for Language {
    fn globals() -> String {
        internal::globals()
    }

    fn render_functions(
        macro_data: (Vec<FunctionDataType>, TypeDefs),
        cfg: &ExportConfiguration,
    ) -> Result<String, TsExportError> {
        internal::render_functions(macro_data, cfg)
    }

    fn render(
        macro_data: (Vec<FunctionDataType>, TypeDefs),
        cfg: &ExportConfiguration,
    ) -> Result<String, TsExportError> {
        internal::render(macro_data, cfg)
    }
}

/// Exports the output of [`internal::render`] for a collection of [`FunctionDataType`] into a TypeScript file.
/// Allows for specifying a custom [`ExportConfiguration`](ExportConfiguration).
pub fn export_with_cfg(
    result: (Vec<FunctionDataType>, TypeDefs),
    cfg: ExportConfiguration,
    export_path: impl AsRef<Path>,
) -> Result<(), TsExportError> {
    Exporter::new(Ok(result), export_path)
        .with_cfg(cfg)
        .export()
}

/// Exports the output of [`internal::render`] for a collection of [`FunctionDataType`] into a TypeScript file.
pub fn export(
    macro_data: Result<(Vec<FunctionDataType>, TypeDefs), ExportError>,
    export_path: impl AsRef<Path>,
) -> Result<(), TsExportError> {
    Exporter::new(macro_data, export_path).export()
}
