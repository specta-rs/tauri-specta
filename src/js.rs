use crate::*;
use heck::ToLowerCamelCase;
use indoc::formatdoc;
use specta::{functions::FunctionDataType, js_doc, ts};
use tauri::Runtime;

/// Implements [`ExportLanguage`] for JS exporting
pub struct Language;

pub fn builder<TRuntime: Runtime>() -> PluginBuilder<Language, NoCommands<TRuntime>, NoEvents> {
    PluginBuilder::default()
}

pub const GLOBALS: &str = include_str!("./globals.js");

type Config = specta::ts::ExportConfig;

pub type ExportConfig = crate::ExportConfig<Config>;

impl ExportLanguage for Language {
    type Config = Config;
    type Error = ts::ExportError;

    fn run_format(path: PathBuf, cfg: &ExportConfig) {
        cfg.inner.run_format(path).ok();
    }

    /// Renders a collection of [`FunctionDataType`] into a JavaScript string.
    fn render_commands(
        commands: &[FunctionDataType],
        type_map: &TypeMap,
        cfg: &ExportConfig,
    ) -> Result<String, ExportError> {
        let commands = commands
            .iter()
            .map(|function| {
                let jsdoc = {
                    let ret_type = js_ts::handle_result(function, type_map, cfg)?;

                    let mut builder = js_doc::Builder::default();

                    if !function.docs.is_empty() {
                        builder.extend(function.docs.split("\n"));
                    }

                    builder.extend(function.args.iter().flat_map(|(name, typ)| {
                        ts::datatype(&cfg.inner, typ, type_map).map(|typ| {
                            let name = name.to_lower_camel_case();

                            format!("@param {{ {typ} }} {name}")
                        })
                    }));
                    builder.push(&format!("@returns {{ Promise<{ret_type}> }}"));

                    builder.build()
                };

                Ok(js_ts::function(
                    &jsdoc,
                    &function.name.to_lower_camel_case(),
                    &js_ts::arg_names(&function.args),
                    None,
                    &js_ts::command_body(cfg, function, false),
                ))
            })
            .collect::<Result<Vec<_>, ExportError>>()?
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
    ) -> Result<String, ExportError> {
        if events.is_empty() {
            return Ok(Default::default());
        }

        let (events_types, events_map) = js_ts::events_data(events, cfg, type_map)?;

        let events = {
            let mut builder = js_doc::Builder::default();

            builder.push("@type {typeof __makeEvents__<{");
            builder.extend(events_types);
            builder.push("}>}");

            builder.build()
        };

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
    ) -> Result<String, ExportError> {
        let dependant_types = type_map
            .values()
            .filter_map(|v| v.as_ref())
            .map(|v| js_doc::typedef_named_datatype(&cfg.inner, v, type_map))
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.join("\n"))?;

        js_ts::render_all_parts::<Self>(commands, events, type_map, cfg, &dependant_types, GLOBALS)
    }
}
