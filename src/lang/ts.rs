use std::collections::HashMap;
use std::path::Path;

use crate::{lang::js_ts, ExportContext, LanguageExt};
use heck::ToLowerCamelCase;
use specta::datatype::{Function, FunctionResultVariant};
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
    let commands_by_module: HashMap<String, Vec<&Function>> = cfg
        .commands
        .iter()
        .zip(cfg.command_modules.iter())
        .fold(HashMap::new(), |mut map, (function, path)| {
            // ns should be equal to the string before the last ::
            // e.g. for "hello::world::foo", ns should be "hello::world"
            // otherwise, it should be "" (global namespace)
            // We replace :: with _ to make it a valid namespace in TS
            let ns = if let Some(pos) = path.rfind("::") {
                path[..pos].to_string()
            } else {
                "".to_string()
            };
            map.entry(ns.replace("::", "_")).or_default().push(function);
            map
        });

    let all_commands = commands_by_module
        .iter()
        .map(|(module_path, functions)| {
            // Render each function in the module
            let functions_str = functions
                .iter()
                .map(|function| render_command(function, ts, cfg))
                .collect::<Result<Vec<_>, ExportError>>()?
                .join("\n");
            // Wrap module in a namespace if needed
            let str = if !module_path.is_empty() {
                format!("export namespace {} {{\n\t{}\n}}", module_path, functions_str.replace("\n", "\n\t").replace("\r", "\r\t"))
            } else {
                functions_str
            };
            Ok(str)
        })
        .collect::<Result<Vec<_>, ExportError>>()?
        .join("\n").replace("\n", "\n\t").replace("\r", "\r\t");

    // wrap all commands in a single export namespace commands
    Ok(format!("export namespace commands {{\n\t{}\n}}", all_commands))
}

fn render_command(
    function: &Function,
    ts: &Typescript,
    cfg: &ExportContext,
) -> Result<String, ExportError> {
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
    let str = js_ts::function(
        &docs,
        &function.name().to_lower_camel_case(),
        &arg_defs,
        Some(&ret_type),
        &js_ts::command_body(&cfg.plugin_name, function, true, cfg.error_handling),
    );
    Ok(str)
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
