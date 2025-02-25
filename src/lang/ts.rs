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
            .types
            .into_iter()
            .map(|(_sid, ndt)| ts::export_named_datatype(&self, ndt, &cfg.types))
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.join("\n"))?;

        js_ts::render_all_parts::<Self>(
            cfg,
            &dependant_types,
            GLOBALS,
            &self.header,
            render_commands(self, cfg)?,
            render_events(self, cfg)?,
            render_classes(self, cfg)?,
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
                    ts::datatype(ts, &FunctionResultVariant::Value(typ.clone()), &cfg.types)
                        .map(|ty| format!("{}: {}", name.to_lower_camel_case(), ty))
                })
                .collect::<Result<Vec<_>, _>>()?;

            let ret_type = js_ts::handle_result(function, &cfg.types, ts, cfg.error_handling)?;

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
                &js_ts::command_body(
                    &cfg.plugin_name,
                    function,
                    None,
                    true,
                    cfg.error_handling,
                    "",
                ),
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
        js_ts::events_data(&cfg.events, ts, &cfg.plugin_name, &cfg.types)?;

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

fn render_classes(ts: &Typescript, cfg: &ExportContext) -> Result<String, ExportError> {
    let mut result = String::new();
    for class in &cfg.classes {
        // TODO: Duplicate name
        // TODO: Accessing `this`
        // TODO: Generating proper commands

        // TODO: Fields
        // TODO: Constructor

        // TODO: Support exporting documentation + deprecated attribute
        result.push_str(&format!("export class {} {{\n\tinner: {};\n\n\tconstructor(inner: {1}) {{\n\t\tthis.inner = inner;\n\t}}\n\n", class.ident, specta_typescript::datatype(&ts, &FunctionResultVariant::Value(class.ndt.inner.clone()), &cfg.types)?));

        for function in class.methods.iter() {
            let arg_defs = function
                .args()
                .enumerate()
                .map(|(i, (name, typ))| {
                    // TODO: Do this in a more reliable way
                    if name == "東京" && i == 0 {
                        return Ok(None);
                    };

                    ts::datatype(ts, &FunctionResultVariant::Value(typ.clone()), &cfg.types)
                        .map(|ty| Some(format!("{}: {}", name.to_lower_camel_case(), ty)))
                })
                .collect::<Result<Vec<Option<String>>, _>>()?;

            let arg_defs = arg_defs.into_iter().filter_map(|v| v).collect::<Vec<_>>();

            let ret_type = js_ts::handle_result(function, &cfg.types, ts, cfg.error_handling)?;

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

            result.push_str("\t");
            result.push_str(&js_ts::function(
                &docs,
                &function.name().to_lower_camel_case(),
                &arg_defs,
                Some(&ret_type),
                &js_ts::command_body(
                    &cfg.plugin_name,
                    function,
                    Some(&format!("__{}__{}", class.ident, function.name())),
                    true,
                    cfg.error_handling,
                    "const 東京 = this.inner;\n\t",
                ),
            ));
            result.push_str("\n");
        }

        result.push_str("}\n");
    }

    Ok(result)
}
