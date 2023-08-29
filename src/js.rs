use std::path::Path;

use specta::{functions::FunctionDataType, ts::TsExportError, ExportError, TypeDefs};

use crate::{ExportConfiguration, ExportLanguage};

/// Building blocks for [`export`] and [`export_with_cfg`].
///
/// These are made available for advanced use cases where you may combine Tauri Specta with another
/// Specta-enabled library.
pub mod internal {
    use heck::ToLowerCamelCase;
    use indoc::formatdoc;

    use crate::{ExportConfiguration, DO_NOT_EDIT};
    use specta::{
        functions::{FunctionDataType, SpectaFunctionResultVariant},
        ts::{self, TsExportError},
        TypeDefs,
    };

    /// Constants that the generated functions rely on
    pub fn globals() -> String {
        formatdoc! {
            r#"
            // Function avoids 'window not defined' in SSR
            const invoke = () => window.__TAURI_INVOKE__;"#
        }
    }

    /// Renders a collection of [`FunctionDataType`] into a JavaScript string.
    pub fn render_functions(
        (function_types, type_map): (Vec<FunctionDataType>, TypeDefs),
        cfg: &ExportConfiguration,
    ) -> Result<String, TsExportError> {
        function_types
            .into_iter()
            .map(|function| {
                let name_camel = function.name.to_lower_camel_case();

                let arg_list = function
                    .args
                    .iter()
                    .map(|(name, _)| name.to_lower_camel_case())
                    .collect::<Vec<_>>();

                let arg_defs = arg_list.join(", ");

                let jsdoc = {
                    let ret_type = match &function.result {
                        SpectaFunctionResultVariant::Value(t) => {
                            ts::datatype(&cfg.spectra_config, t, &type_map)?
                        }
                        SpectaFunctionResultVariant::Result(t, e) => {
                            format!(
                                "[{}, undefined] | [undefined, {}]",
                                ts::datatype(&cfg.spectra_config, t, &type_map)?,
                                ts::datatype(&cfg.spectra_config, e, &type_map)?
                            )
                        }
                    };

                    let vec = []
                        .into_iter()
                        .chain(
                            function
                                .docs
                                .into_iter()
                                .map(str::to_owned)
                                .collect::<Vec<_>>(),
                        )
                        .chain(function.args.iter().flat_map(|(name, typ)| {
                            ts::datatype(&cfg.spectra_config, typ, &type_map).map(|typ| {
                                let name = name.to_lower_camel_case();

                                format!("@param {{ {typ} }} {name}")
                            })
                        }))
                        .chain([format!("@returns {{ Promise<{ret_type}> }}")])
                        .collect::<Vec<_>>();

                    specta::ts::js_doc(&vec.iter().map(|s| s.as_str()).collect::<Vec<_>>())
                };

                let body = {
                    let name = if let Some(ref plugin_name) = cfg.plugin_name {
                        format!("plugin:{}|{}", plugin_name, function.name)
                    } else {
                        function.name.to_string()
                    };

                    let arg_usages = arg_list
                        .is_empty()
                        .then(Default::default)
                        .unwrap_or_else(|| format!(", {{ {} }}", arg_list.join(", ")));

                    let invoke = format!("await invoke()(\"{name}\"{arg_usages})");

                    match &function.result {
                        SpectaFunctionResultVariant::Value(_) => format!("return {invoke};"),
                        SpectaFunctionResultVariant::Result(_, _) => formatdoc!(
                            r#"
                            try {{
                                return [{invoke}, undefined];
                            }} catch (e) {{
                                if(e instanceof Error) throw e;
                                else return [undefined, e];
                            }}"#
                        ),
                    }
                };

                Ok(formatdoc! {
                    r#"
                    {jsdoc}export async function {name_camel}({arg_defs}) {{
                    {body}
                    }}"#
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.join("\n\n"))
    }

    /// Renders the output of [`globals`] and [`render_functions`] into a TypeScript string.
    pub fn render(
        macro_data: (Vec<FunctionDataType>, TypeDefs),
        cfg: &ExportConfiguration,
    ) -> Result<String, TsExportError> {
        let globals = globals();

        let functions = render_functions(macro_data, cfg)?;

        Ok(formatdoc! {
            r#"
                {DO_NOT_EDIT}

                {globals}

                {functions}
            "#
        })
    }
}

/// Implements [`ExportLanguage`] for JS exporting
pub struct Language;

/// [`Exporter`](crate::Exporter) for JavaScript
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

/// Exports the output of [`internal::render`] for a collection of [`FunctionDataType`] into a JavaScript file.
/// Allows for specifying a custom [`ExportConfiguration`](specta::ts::ExportConfiguration).
pub fn export_with_cfg(
    result: (Vec<FunctionDataType>, TypeDefs),
    export_path: impl AsRef<Path>,
    cfg: ExportConfiguration,
) -> Result<(), TsExportError> {
    Exporter::new(Ok(result), export_path)
        .with_cfg(cfg)
        .export()
}

/// Exports the output of [`internal::render`] for a collection of [`FunctionDataType`] into a JavaScript file.
pub fn export(
    macro_data: Result<(Vec<FunctionDataType>, TypeDefs), ExportError>,
    export_path: impl AsRef<Path>,
) -> Result<(), TsExportError> {
    Exporter::new(macro_data, export_path).export()
}
