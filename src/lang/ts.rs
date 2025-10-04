use std::collections::BTreeMap;
use std::path::Path;

use crate::lang::js_ts::render_constants;
use crate::{lang::js_ts, ExportContext, LanguageExt};
use heck::{ToLowerCamelCase, ToPascalCase};
use specta::datatype::{Function, FunctionResultVariant};
use specta_typescript::{self as ts, Typescript};
use specta_typescript::{js_doc, ExportError};

const GLOBALS: &str = include_str!("./globals.ts");

impl LanguageExt for specta_typescript::Typescript {
    type Error = ExportError;

    fn render(&self, cfg: &ExportContext) -> Result<String, ExportError> {
        js_ts::render_all_parts::<Self>(
            cfg,
            &render_types(self, cfg)?,
            GLOBALS,
            &self.header,
            render_commands(self, cfg)?.values().cloned().collect::<Vec<_>>().join("\n"),
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

    fn render_per_file(&self, cfg: &ExportContext) -> Result<crate::ExportFiles, Self::Error> {
        let globals = GLOBALS.to_string();
        let modules = render_commands(self, cfg)?;
        let events = render_events(self, cfg)?;
        let constants = render_constants::<Self>(cfg, true)?;
        let types = render_types(self, cfg)?;
        let type_names = cfg
            .type_map
            .into_iter()
            .map(|(_sid, ndt)| {
                let name = ndt.name().to_string();
                name
            })
            .collect::<Vec<_>>()
            .join(", ");

        // Add header to each file if specified
        let mut header = self.header.to_string();
        if !header.is_empty() {
            header.push('\n');
        }
        header += &self.framework_header;

        let events = format!(
            "import {{ __makeEvents__ }} from './globals';
import {{ {type_names} }} from './types';\n
{events}"
        );

        let mut files = crate::ExportFiles::default();
        // files.set_commands(commands);
        files.set_events(events);
        files.set_types(types);
        files.set_constants(constants);
        files.set_globals(globals);
        for (module, content) in modules.iter() {
            files.set_module(
                module.to_string(),
                format!(
                    "import {{
  invoke as TAURI_INVOKE,
  Channel as TAURI_CHANNEL,
}} from \"@tauri-apps/api/core\";
import {{ Result }} from '../globals';
import {{ {type_names} }} from '../types';\n
{content}"
                ),
            );
        }
        // Add header to each file if specified
        if !header.is_empty() {
            for (_, content) in files.content_per_file.iter_mut() {
                *content = format!("{header}\n\n{content}");
            }
        }
        Ok(files)
    }
}

fn render_types(ts: &Typescript, cfg: &ExportContext) -> Result<String, ExportError> {
    let dependant_types = cfg
        .type_map
        .into_iter()
        .map(|(_sid, ndt)| ts::export_named_datatype(&ts, ndt, &cfg.type_map))
        .collect::<Result<Vec<_>, _>>()
        .map(|v| v.join("\n"))?;
    Ok(dependant_types)
}

fn render_commands(
    ts: &Typescript,
    cfg: &ExportContext,
) -> Result<BTreeMap<String, String>, ExportError> {
    let commands_by_module: BTreeMap<String, Vec<&Function>> = cfg
        .commands
        .iter()
        .zip(cfg.command_modules.iter())
        .fold(BTreeMap::new(), |mut map, (function, path)| {
            // ns should be equal to the string before the last ::
            // e.g. for "hello::world::foo", ns should be "hello::world"
            // otherwise, it should be "" (global namespace)
            // We replace :: with _ to make it a valid namespace in TS
            let path = path.replace(" ", "");
            let ns = if let Some(pos) = path.rfind("::") {
                path[..pos].to_string()
            } else {
                "".to_string()
            };
            map.entry(ns).or_default().push(function);
            map
        });

    // For each module, render the functions inside it
    let all_modules = commands_by_module
        .iter()
        .map(|(module_path, functions)| {
            let is_class = cfg.class_modules.contains(module_path.as_str());
            let has_namespace = !module_path.is_empty();
            let mut parts: Vec<&str> = module_path.split("::").collect();
            let mut class_name: Option<String> = None;
            // remove the class name from the namespace if it's a class and there's at least `mod::class`
            if is_class && parts.len() >= 2 {
                class_name = Some(parts.pop().clone().unwrap().to_pascal_case());
            }
            let namespace = if has_namespace { parts.join("_") } else { "commands".into() } ;

            // Render each function in the module
            let functions_str = functions
                .iter()
                .map(|function| render_command(function, ts, cfg, class_name.as_deref()))
                .collect::<Result<Vec<_>, ExportError>>()?
                .join("\n");

            let mut str = functions_str;
            if is_class {
                str = format!(
                    "export class {} {{\n\tprivate constructor(private readonly structId: string) {{}}\n\t{}\n}}",
                    class_name.unwrap(),
                    str.replace("\n", "\n\t").replace("\r", "\r\t")
                )
            } else {
                str = format!(
                    "export namespace {} {{\n\t{}\n}}\n",
                    namespace,
                    str.replace("\n", "\n\t").replace("\r", "\r\t")
                );
            }
            Ok(("commands::".to_owned() + &module_path.to_string(), str))
        })
        .collect::<Result<BTreeMap<_, _>, ExportError>>()?;
    Ok(all_modules)
}

fn render_command(
    function: &Function,
    ts: &Typescript,
    cfg: &ExportContext,
    class_name: Option<&str>,
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

    let ret_type = js_ts::handle_result(function, ts, cfg)?;

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
    let str = crate::lang::ts::function(
        &docs,
        &function.name().to_lower_camel_case(),
        &arg_defs,
        Some(&ret_type),
        &js_ts::command_body(&cfg.plugin_name, function, true, cfg.error_handling)
            .replace("structId", "structId: this.structId"),
        class_name,
    );
    Ok(str)
}

fn function(
    docs: &str,
    name: &str,
    args: &[String],
    return_type: Option<&str>,
    body: &str,
    class_name: Option<&str>,
) -> String {
    let mut return_str = return_type
        .map(|t| format!(": Promise<{}>", t))
        .unwrap_or_default();
    let mut export = "export ";
    let mut static_ = "";
    let async_ = "async ";
    let mut function = "function ";
    let mut args_str = args.join(", ");
    let mut body_str = body.to_string();
    if class_name.is_some() {
        export = "";
        function = "";
        args_str = args
            .iter()
            .filter(|a| !a.ends_with(": Id"))
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        // Instantiator
        if name == "instance" {
            // if can have multiple instances:
            // if let Some("Id") = return_type { }
            static_ = "static ";
            return_str = format!(": Promise<{}>", class_name.unwrap());
            body_str = format!(
                "return new {}(await TAURI_INVOKE(\"{}\", {{ {} }}));",
                class_name.unwrap(),
                name,
                args.iter()
                    .map(|a| a.split(':').next().unwrap())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    format!(
        r#"{docs}{export}{static_}{async_}{function}{name}({args_str}) {return_str} {{
    {body_str}
}}"#
    )
}

fn render_events(ts: &Typescript, cfg: &ExportContext) -> Result<String, ExportError> {
    if cfg.events.is_empty() {
        return Ok(Default::default());
    }

    let (events_types, events_map) =
        js_ts::events_data(&cfg.events, ts, &cfg.plugin_name, &cfg.type_map)?;

    let events_types = events_types.join(",\n");
    let events_map = events_map.join(",\n");

    Ok(format! {
r#"export const events = __makeEvents__<{{
{events_types}
}}>({{
{events_map}
}})"#
    })
}
