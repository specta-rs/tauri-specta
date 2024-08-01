use crate::{js_ts, Configuration, LanguageExt};
use heck::ToLowerCamelCase;
use indoc::formatdoc;
use specta::datatype::FunctionResultVariant;
use specta_typescript as ts;
use specta_typescript::{js_doc, ExportError};

// TODO: Make private
pub(crate) const GLOBALS: &str = include_str!("./globals.ts");

impl LanguageExt for specta_typescript::Typescript {
    fn render_commands(&self, cfg: &Configuration) -> Result<String, ExportError> {
        let commands = cfg
            .commands
            .iter()
            .map(|function| {
                let arg_defs = function
                    .args()
                    .map(|(name, typ)| {
                        ts::datatype(
                            self,
                            &FunctionResultVariant::Value(typ.clone()),
                            &cfg.type_map,
                        )
                        .map(|ty| format!("{}: {}", name.to_lower_camel_case(), ty))
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                let ret_type = js_ts::handle_result(function, &cfg.type_map, self)?;

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
                    &js_ts::command_body(&cfg.plugin_name, function, true),
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

    fn render_events(&self, cfg: &Configuration) -> Result<String, ExportError> {
        if cfg.events.is_empty() {
            return Ok(Default::default());
        }

        let (events_types, events_map) =
            js_ts::events_data(&cfg.events, self, &cfg.plugin_name, &cfg.type_map)?;

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

    fn render(&self, cfg: &Configuration) -> Result<String, ExportError> {
        let dependant_types = cfg
            .type_map
            .iter()
            .map(|(_sid, ndt)| ts::export_named_datatype(&self, ndt, &cfg.type_map))
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.join("\n"))?;

        js_ts::render_all_parts::<Self>(self, cfg, &dependant_types, GLOBALS)
    }
}
