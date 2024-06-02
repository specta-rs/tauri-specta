use crate::{js_ts, *};
use heck::ToLowerCamelCase;
use indoc::formatdoc;
use specta::{
    function::FunctionDataType,
    js_doc,
    ts::{self, ExportError},
    TypeMap,
};
use tauri::Runtime;

/// Implements [`ExportLanguage`] for TypeScript exporting
pub struct Language;

pub fn builder<TRuntime: Runtime>() -> Builder<Language, NoCommands<TRuntime>, NoEvents> {
    Builder::default()
}

pub const GLOBALS: &str = include_str!("./globals.ts");

type Config = specta::ts::ExportConfig;
pub type ExportConfig = crate::ExportConfig<Config>;

impl ExportLanguage for Language {
    type Config = Config;
    type Error = ts::ExportError;

    fn run_format(path: PathBuf, cfg: &ExportConfig) {
        cfg.inner.run_format(path).ok();
    }

    /// Renders a collection of [`FunctionDataType`] into a TypeScript string.
    fn render_commands(
        commands: &[FunctionDataType],
        type_map: &TypeMap,
        cfg: &ExportConfig,
    ) -> Result<String, ExportError> {
        let commands = commands
            .iter()
            .map(|function| {
                let arg_defs = function
                    .args
                    .iter()
                    .map(|(name, typ)| {
                        ts::datatype(&cfg.inner, typ, type_map)
                            .map(|ty| format!("{}: {}", name.to_lower_camel_case(), ty))
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                let ret_type = js_ts::handle_result(function, type_map, cfg)?;

                let docs = {
                    let mut builder = js_doc::Builder::default();

                    if let Some(d) = &function.deprecated {
                        builder.push_deprecated(d);
                    }

                    if !function.docs.is_empty() {
                        builder.extend(function.docs.split("\n"));
                    }

                    builder.build()
                };
                Ok(js_ts::function(
                    &docs,
                    &function.name.to_lower_camel_case(),
                    &arg_defs,
                    Some(&ret_type),
                    &js_ts::command_body(cfg, function, true),
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

        let events_types = events_types.join(",\n");

        Ok(formatdoc! {
            r#"
            export const events = __makeEvents__<{{
            {events_types}
            }}>({{
            {events_map}
            }})"#
        })
    }

    fn render(
        commands: &[FunctionDataType],
        events: &[EventDataType],
        type_map: &TypeMap,
        statics: &StaticCollection,
        cfg: &ExportConfig,
    ) -> Result<String, ExportError> {
        let dependant_types = type_map
            .iter()
            .map(|(_sid, ndt)| ts::export_named_datatype(&cfg.inner, ndt, type_map))
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.join("\n"))?;

        js_ts::render_all_parts::<Self>(
            commands,
            events,
            type_map,
            statics,
            cfg,
            &dependant_types,
            GLOBALS,
        )
    }
}
