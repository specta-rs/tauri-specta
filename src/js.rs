use crate::{
    js_ts, ts::ExportConfig, EventDataType, ExportLanguage, NoCommands, NoEvents, PluginBuilder,
};
use heck::ToLowerCamelCase;
use indoc::formatdoc;
use specta::{
    functions::FunctionDataType,
    ts::{self, js_doc, TsExportError},
    TypeMap,
};
use tauri::Runtime;

/// Implements [`ExportLanguage`] for JS exporting
pub struct Language;

pub fn builder<TRuntime: Runtime>() -> PluginBuilder<Language, NoCommands<TRuntime>, NoEvents> {
    PluginBuilder::default()
}

pub const GLOBALS: &str = include_str!("./globals.js");

impl ExportLanguage for Language {
    /// Renders a collection of [`FunctionDataType`] into a JavaScript string.
    fn render_commands(
        commands: &[FunctionDataType],
        type_map: &TypeMap,
        cfg: &ExportConfig,
    ) -> Result<String, TsExportError> {
        let commands = commands
            .iter()
            .map(|function| {
                let jsdoc = {
                    let ret_type = js_ts::handle_result(function, type_map, cfg)?;

                    let vec = []
                        .into_iter()
                        .chain(function.docs.iter().map(|s| s.to_string()))
                        .chain(function.args.iter().flat_map(|(name, typ)| {
                            ts::datatype(&cfg.inner, typ, type_map).map(|typ| {
                                let name = name.to_lower_camel_case();

                                format!("@param {{ {typ} }} {name}")
                            })
                        }))
                        .chain([format!("@returns {{ Promise<{ret_type}> }}")])
                        .map(Into::into)
                        .collect::<Vec<_>>();

                    js_doc(&vec)
                };

                Ok(js_ts::function(
                    &jsdoc,
                    &function.name.to_lower_camel_case(),
                    &js_ts::arg_names(&function.args),
                    None,
                    &js_ts::command_body(cfg, function, false),
                ))
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

        let (events_types, events_map) = js_ts::events_data(events, cfg, type_map)?;

        let events = js_doc(
            &[].into_iter()
                .chain(["@type {typeof __makeEvents__<{".to_string()])
                .chain(events_types)
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
        let dependant_types = type_map
            .values()
            .filter_map(|v| v.as_ref())
            .map(|v| {
                ts::named_datatype(&cfg.inner, v, type_map).map(|typ| {
                    let name = v.name();

                    js_doc(&[format!("@typedef {{ {typ} }} {name}").into()])
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.join("\n"))?;

        js_ts::render_all_parts::<Self>(commands, events, type_map, cfg, &dependant_types, GLOBALS)
    }
}
