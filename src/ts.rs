use crate::{js_ts, *};
use heck::ToLowerCamelCase;
use indoc::formatdoc;
use specta::{datatype, datatype::FunctionResultVariant};
use specta_typescript as ts;
use specta_typescript::{js_doc, ExportError};

// TODO: Make private
pub(crate) const GLOBALS: &str = include_str!("./globals.ts");

impl LanguageExt for specta_typescript::Typescript {
    /// Renders a collection of [`FunctionDataType`] into a TypeScript string.
    fn render_commands(
        &self,
        commands: &[datatype::Function],
        type_map: &TypeMap,
        plugin_name: &Option<&'static str>,
    ) -> Result<String, ExportError> {
        let commands = commands
            .iter()
            .map(|function| {
                let arg_defs = function
                    .args()
                    .map(|(name, typ)| {
                        ts::datatype(self, &FunctionResultVariant::Value(typ.clone()), type_map)
                            .map(|ty| format!("{}: {}", name.to_lower_camel_case(), ty))
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                let ret_type = js_ts::handle_result(function, type_map, self)?;

                let docs = {
                    let mut builder = js_doc::Builder::default();

                    if let Some(d) = &function.deprecated() {
                        builder.push_deprecated(d);
                    }

                    if !function.docs().is_empty() {
                        builder.extend(function.docs().split("\n"));
                    }

                    builder.build()
                };
                Ok(js_ts::function(
                    &docs,
                    &function.name().to_lower_camel_case(),
                    &arg_defs,
                    Some(&ret_type),
                    &js_ts::command_body(plugin_name, function, true),
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
        &self,
        events: &[EventDataType],
        type_map: &TypeMap,
        plugin_name: &Option<&'static str>,
    ) -> Result<String, ExportError> {
        if events.is_empty() {
            return Ok(Default::default());
        }

        let (events_types, events_map) = js_ts::events_data(events, self, plugin_name, type_map)?;

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
        &self,
        commands: &[datatype::Function],
        events: &[EventDataType],
        type_map: &TypeMap,
        plugin_name: &Option<&'static str>,
    ) -> Result<String, ExportError> {
        let dependant_types = type_map
            .iter()
            .map(|(_sid, ndt)| ts::export_named_datatype(&self, ndt, type_map))
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.join("\n"))?;

        js_ts::render_all_parts::<Self>(
            commands,
            events,
            type_map,
            self,
            plugin_name,
            &dependant_types,
            GLOBALS,
        )
    }
}
