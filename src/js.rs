use crate::{
    ts::ExportConfiguration, EventMeta, ExportLanguage, NoCommands, NoEvents, DO_NOT_EDIT,
};
use heck::ToLowerCamelCase;
use indoc::formatdoc;
use specta::{
    functions::{FunctionDataType, SpectaFunctionResultVariant},
    ts::{self, TsExportError},
    TypeDefs,
};

/// Implements [`ExportLanguage`] for JS exporting
pub struct Language;

/// [`Exporter`](crate::Exporter) for JavaScript
pub type Exporter<TRuntime> = crate::Exporter<Language, NoCommands<TRuntime>, NoEvents>;

impl ExportLanguage for Language {
    fn globals() -> String {
        include_str!("./globals.js").to_string()
    }

    /// Renders a collection of [`FunctionDataType`] into a JavaScript string.
    fn render_commands(
        commands: &[FunctionDataType],
        type_map: &TypeDefs,
        cfg: &ExportConfiguration,
    ) -> Result<String, TsExportError> {
        commands
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

                    let vec = []
                        .into_iter()
                        .chain(function.docs.iter().map(|s| s.to_string()))
                        .chain(function.args.iter().flat_map(|(name, typ)| {
                            ts::datatype(&cfg.inner, typ, &type_map).map(|typ| {
                                let name = name.to_lower_camel_case();

                                format!("@param {{ {typ} }} {name}")
                            })
                        }))
                        .chain([format!("@returns {{ Promise<{ret_type}> }}")])
                        .collect::<Vec<_>>();

                    specta::ts::js_doc(&vec.iter().map(|s| s.as_str()).collect::<Vec<_>>())
                };

                let body = {
                    let name = match (&cfg.plugin_name, cfg.plugin_prefix) {
                        (Some(plugin_name), true) => {
                            format!("plugin:tauri-specta-{plugin_name}|{}", function.name)
                        }
                        (None, true) => format!("plugin:tauri-specta|{}", function.name),
                        (Some(plugin_name), false) => {
                            format!("plugin:{plugin_name}|{}", function.name)
                        }
                        (None, false) => function.name.to_string(),
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

    fn render_events(
        events: &[EventMeta],
        type_map: &TypeDefs,
        cfg: &ExportConfiguration,
    ) -> Result<String, TsExportError> {
        if events.is_empty() {
            return Ok(Default::default());
        }

        let events = events
            .iter()
            .map(|event| {
                let name = event.name;
                // let typ = ts::datatype(&cfg.inner, &event.typ, type_map)?;

                let name_camel = name.to_lower_camel_case();

                Ok(format!(r#"	{name_camel}: __makeEvent__("{name}")"#))
            })
            .collect::<Result<Vec<_>, TsExportError>>()?
            .join(",\n");

        Ok(formatdoc! {
            r#"
	        export const events = {{
	        {events}
	        }}"#
        })
    }

    fn render(
        commands: &[FunctionDataType],
        events: &[EventMeta],
        type_map: &TypeDefs,
        cfg: &ExportConfiguration,
    ) -> Result<String, TsExportError> {
        let globals = Self::globals();

        let commands = Self::render_commands(commands, &type_map, cfg)?;
        let events = Self::render_events(events, &type_map, cfg)?;

        Ok(formatdoc! {
            r#"
            {DO_NOT_EDIT}

            {globals}
			{events}

            {commands}
	        "#
        })
    }
}
