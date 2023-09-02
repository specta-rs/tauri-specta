use crate::{
    ts::ExportConfig, EventDataType, ExportLanguage, ItemType, NoCommands, NoEvents, DO_NOT_EDIT,
};
use heck::ToLowerCamelCase;
use indoc::formatdoc;
use specta::{
    functions::FunctionDataType,
    ts::{self, js_doc, TsExportError},
    DataType, TypeMap,
};
use tauri::Runtime;

/// Implements [`ExportLanguage`] for JS exporting
pub struct Language;

/// [`Exporter`](crate::Exporter) for JavaScript
pub type PluginBuilder<TCommands, TEvents> = crate::PluginBuilder<Language, TCommands, TEvents>;

pub fn builder<TRuntime: Runtime>() -> PluginBuilder<NoCommands<TRuntime>, NoEvents> {
    PluginBuilder::default()
}

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
                        .map(Into::into)
                        .collect::<Vec<_>>();

                    js_doc(&vec)
                };

                let name_camel = function.name.to_lower_camel_case();

                let arg_list = function
                    .args
                    .iter()
                    .map(|(name, _)| name.to_lower_camel_case())
                    .collect::<Vec<_>>();

                let arg_defs = arg_list.join(", ");

                let body = {
                    let name = cfg
                        .plugin_name
                        .apply_as_prefix(&function.name, ItemType::Command);

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
        events: &[EventDataType],
        type_map: &TypeMap,
        cfg: &ExportConfig,
    ) -> Result<String, TsExportError> {
        if events.is_empty() {
            return Ok(Default::default());
        }

        let events_map = events
            .iter()
            .map(|event| {
                let name_str = cfg
                    .plugin_name
                    .apply_as_prefix(&event.name, ItemType::Event);
                let name_camel = event.name.to_lower_camel_case();

                format!(r#"	{name_camel}: "{name_str}""#)
            })
            .collect::<Vec<_>>()
            .join(",\n");

        let events = events
            .iter()
            .map(|event| {
                let typ = ts::datatype(&cfg.inner, &event.typ, type_map)?;

                let name_camel = event.name.to_lower_camel_case();

                Ok(format!(r#"{name_camel}: {typ},"#))
            })
            .collect::<Result<Vec<_>, TsExportError>>()?;

        let events = js_doc(
            &[].into_iter()
                .chain(["@type {typeof __makeEvents__<{".to_string()])
                .chain(events)
                .chain(["}>}".to_string()])
                .map(Into::into)
                .collect::<Vec<_>>(),
        );

        Ok(formatdoc! {
            r#"
            {events}
            const __typedMakeEvents__ = __makeEvents__;

	        export const events = __typedMakeEvents__({{
	        {events_map}
	        }})"#
        })
    }

    fn render(
        commands: &[FunctionDataType],
        events: &[EventDataType],
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
                    let name = v.name();

                    js_doc(&[format!("@typedef {{ {typ} }} {name}").into()])
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
