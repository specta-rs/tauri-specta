use crate::{ExportContext, LanguageExt, lang::js_ts};
use heck::ToLowerCamelCase;
use specta::datatype::FunctionReturnType;
use specta_typescript::{self as ts, Typescript, primitives};

const GLOBALS: &str = include_str!("./globals.ts");

impl LanguageExt for specta_typescript::Typescript {
    type Error = specta_typescript::Error;

    fn render(&self, cfg: &ExportContext) -> Result<String, Self::Error> {
        let dependant_types = cfg
            .types
            .into_sorted_iter()
            .map(|ndt| primitives::export(&self, &cfg.types, &ndt))
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
}

fn render_commands(
    ts: &Typescript,
    cfg: &ExportContext,
) -> Result<String, specta_typescript::Error> {
    let commands = cfg
        .commands
        .iter()
        .map(|function| {
            let arg_defs = function
                .args()
                .into_iter()
                .map(|(name, typ)| {
                    primitives::inline(ts, &cfg.types, &typ)
                        .map(|ty| format!("{}: {}", name.to_lower_camel_case(), ty))
                })
                .collect::<Result<Vec<_>, _>>()?;

            let ret_type = js_ts::handle_result(function, &cfg.types, ts, cfg.error_handling)?;

            let docs = {
                // let mut builder = js_doc::Builder::default();

                // if let Some(d) = &function.deprecated() {
                //     builder.push_deprecated(d);
                // }

                // if !function.docs().is_empty() {
                //     builder.extend(function.docs().split("\n"));
                // }

                // builder.build()
                format!("") // TODO
            };
            Ok(js_ts::function(
                &docs,
                &function.name().to_lower_camel_case(),
                &arg_defs,
                Some(&ret_type),
                &js_ts::command_body(&cfg.plugin_name, function, true, cfg.error_handling),
            ))
        })
        .collect::<Result<Vec<_>, specta_typescript::Error>>()?
        .join(",\n");

    Ok(format! {
        r#"
export const commands = {{
{commands}
}}"#
    })
}

fn render_events(ts: &Typescript, cfg: &ExportContext) -> Result<String, specta_typescript::Error> {
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
