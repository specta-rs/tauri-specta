use heck::ToLowerCamelCase;
use specta::datatype::FunctionReturnType;
use specta_typescript::{Typescript, primitives};

use crate::{ExportContext, LanguageExt};

use super::js_ts;

const GLOBALS: &str = include_str!("./globals.js");

impl LanguageExt for specta_typescript::JSDoc {
    type Error = specta_typescript::Error;

    fn render(&self, cfg: &ExportContext) -> Result<String, Self::Error> {
        let dependant_types = cfg
            .types
            .into_sorted_iter()
            .map(|ndt| primitives::typedef(self, &cfg.types, &ndt))
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.join("\n"))?;

        js_ts::render_all_parts::<Self>(
            cfg,
            &dependant_types,
            GLOBALS,
            &self.inner_ref().header,
            render_commands(self.inner_ref(), cfg)?,
            render_events(self.inner_ref(), cfg)?,
            false,
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
            let jsdoc = {
                let ret_type = js_ts::handle_result(function, &cfg.types, ts, cfg.error_handling)?;

                // let mut builder = js_doc::Builder::default();

                // if let Some(d) = function.deprecated() {
                //     builder.push_deprecated(d);
                // }

                // if !function.docs().is_empty() {
                //     builder.extend(function.docs().split("\n"));
                // }

                // builder.extend(function.args().into_iter().flat_map(|(name, typ)| {
                //     specta_typescript::datatype(
                //         ts,
                //         &FunctionReturnType::Value(typ.clone()),
                //         &cfg.types,
                //     )
                //     .map(|typ| {
                //         let name = name.to_lower_camel_case();

                //         format!("@param {{ {typ} }} {name}")
                //     })
                // }));
                // builder.push(&format!("@returns {{ Promise<{ret_type}> }}"));

                // builder.build()

                format!("") // TODO
            };

            Ok(js_ts::function(
                &jsdoc,
                &function.name().to_lower_camel_case(),
                // TODO: Don't `collect` the whole thing
                &js_ts::arg_names(&function.args().into_iter().cloned().collect::<Vec<_>>()),
                None,
                &js_ts::command_body(&cfg.plugin_name, &function, false, cfg.error_handling),
            ))
        })
        .collect::<Result<Vec<_>, specta_typescript::Error>>()?
        .join(",\n");

    Ok(format!(
        r#"export const commands = {{
        {commands}
    }}"#
    ))
}

fn render_events(ts: &Typescript, cfg: &ExportContext) -> Result<String, specta_typescript::Error> {
    if cfg.events.is_empty() {
        return Ok(Default::default());
    }

    let (events_types, events_map) =
        js_ts::events_data(&cfg.events, ts, &cfg.plugin_name, &cfg.types)?;

    let events = {
        // let mut builder = js_doc::Builder::default();

        // builder.push("@type {typeof __makeEvents__<{");
        // builder.extend(events_types);
        // builder.push("}>}");

        // builder.build()

        format!("") // TODO
    };

    Ok(format! {
        r#"
    {events}
    const __typedMakeEvents__ = __makeEvents__;

    export const events = __typedMakeEvents__({{
    {events_map}
    }})"#
    })
}
