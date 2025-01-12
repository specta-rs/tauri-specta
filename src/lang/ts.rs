use std::path::Path;

use crate::{lang::js_ts, ExportContext, LanguageExt};
use heck::ToLowerCamelCase;
use specta::datatype::FunctionResultVariant;
use specta_typescript::{self as ts, Typescript};
use specta_typescript::{js_doc, ExportError};

const GLOBALS: &str = include_str!("./globals.ts");

impl LanguageExt for specta_typescript::Typescript {
    type Error = ExportError;

    fn render(&self, cfg: &ExportContext) -> Result<String, ExportError> {
        let dependant_types = cfg
            .type_map
            .into_iter()
            .map(|(_sid, ndt)| ts::export_named_datatype(&self, ndt, &cfg.type_map))
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.join("\n"))?;

        js_ts::render_all_parts::<Self>(
            cfg,
            &dependant_types,
            GLOBALS,
            &self.header,
            render_commands(self, cfg)?,
            render_events(self, cfg)?,
            true,
        )
    }

    fn format(&self, path: &Path) -> Result<(), Self::Error> {
        if let Some(formatter) = self.formatter {
            formatter(path)?;
        }
        Ok(())
    }
}

fn render_commands(ts: &Typescript, cfg: &ExportContext) -> Result<String, ExportError> {
    let commands = cfg
        .commands
        .iter()
        .map(|function| {
            let arg_defs = function
                .args()
                .map(|(name, typ)| {
                    ts::datatype(
                        ts,
                        &FunctionResultVariant::Value(typ.clone()),
                        &cfg.type_map,
                    )
                    .map(|ty| format!("{}: {}", name.to_lower_camel_case(), ty))
                })
                .collect::<Result<Vec<_>, _>>()?;

            let ret_type = js_ts::handle_result(function, &cfg.type_map, ts, cfg.error_handling)?;

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
                &js_ts::command_body(&cfg.plugin_name, function, true, cfg.error_handling),
            ))
        })
        .collect::<Result<Vec<_>, ExportError>>()?
        .join(",\n");

    Ok(format! {
        r#"
export const commands = {{
{commands}
}}"#
    })
}

fn render_events(ts: &Typescript, cfg: &ExportContext) -> Result<String, ExportError> {
    if cfg.events.is_empty() {
        return Ok(Default::default());
    }

    let (events_types, events_map) =
        js_ts::events_data(&cfg.events, ts, &cfg.plugin_name, &cfg.type_map)?;

    let events_types = events_types.join(",\n");

    Ok(format! {
        r#"
export const events = __makeEvents__<{{
{events_types}
}}>({{
{events_map}
}})"#
    })
}
