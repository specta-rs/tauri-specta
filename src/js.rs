use crate::{ts::ExportConfig, EventMeta, ExportLanguage, NoCommands, NoEvents, DO_NOT_EDIT};
use heck::ToLowerCamelCase;
use indoc::formatdoc;
use specta::{
    functions::FunctionDataType,
    ts::{self, TsExportError},
    DataType, TypeMap,
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
        type_map: &TypeMap,
        cfg: &ExportConfig,
    ) -> Result<String, TsExportError> {
        let commands = commands
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
                        DataType::Result(t) => {
                            let (t, e) = t.as_ref();

                            format!(
                                "[{}, undefined] | [undefined, {}]",
                                ts::datatype(&cfg.inner, t, &type_map)?,
                                ts::datatype(&cfg.inner, e, &type_map)?
                            )
                        }
                        t => ts::datatype(&cfg.inner, t, &type_map)?,
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

                    specta::ts::js_doc(&vec.into_iter().map(|s| s.into()).collect::<Vec<_>>())
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
                        DataType::Result(_) => formatdoc!(
                            r#"
	                        try {{
	                            return [{invoke}, undefined];
	                        }} catch (e) {{
	                            if(e instanceof Error) throw e;
	                            else return [undefined, e];
	                        }}"#
                        ),
                        _ => format!("return {invoke};"),
                    }
                };

                Ok(formatdoc! {
                    r#"
	                {jsdoc}async {name_camel}({arg_defs}) {{
	                {body}
	                }}"#
                })
            })
            .collect::<Result<Vec<_>, TsExportError>>()?
            .join(",\n");

        Ok(formatdoc! {
            r#"
            export const commands = {{
            {commands}
            }}"#
        })
    }

    fn render_events(
        events: &[EventMeta],
        type_map: &TypeMap,
        cfg: &ExportConfig,
    ) -> Result<String, TsExportError> {
        if events.is_empty() {
            return Ok(Default::default());
        }

        let events_map = events
            .iter()
            .map(|event| {
                let name = event.name;
                let name_camel = name.to_lower_camel_case();

                format!(r#"	{name_camel}: "{name}""#)
            })
            .collect::<Vec<_>>()
            .join(",\n");

        let events = events
            .iter()
            .map(|event| {
                let typ = ts::datatype(&cfg.inner, &event.typ, type_map)?;

                let name_camel = event.name.to_lower_camel_case();

                Ok(format!(r#" *   {name_camel}: {typ}"#))
            })
            .collect::<Result<Vec<_>, TsExportError>>()?
            .join(",\n");

        Ok(formatdoc! {
            r#"
            /**
             * @type {{typeof __makeEvents__<{{
            {events}
             * }}>}}
             */
            const __typedMakeEvents__ = __makeEvents__;

	        export const events = __typedMakeEvents__({{
	        {events_map}
	        }})"#
        })
    }

    fn render(
        commands: &[FunctionDataType],
        events: &[EventMeta],
        type_map: &TypeMap,
        cfg: &ExportConfig,
    ) -> Result<String, TsExportError> {
        let globals = Self::globals();

        let commands = Self::render_commands(commands, &type_map, cfg)?;
        let events = Self::render_events(events, &type_map, cfg)?;

        let dependant_types = type_map
            .values()
            .filter_map(|v| v.as_ref())
            .map(|v| {
                ts::named_datatype(&cfg.inner, v, &type_map).map(|typ| {
                    let name = &v.name;

                    formatdoc! {
                        r#"
                        /**
                         * @typedef {{ {typ} }} {name}
                         */"#
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.join("\n"))?;

        Ok(formatdoc! {
            r#"
            {DO_NOT_EDIT}

            {commands}

			{events}

			{dependant_types}

            {globals}
	        "#
        })
    }
}
