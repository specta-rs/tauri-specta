// use crate::*;
// use heck::ToLowerCamelCase;
// use indoc::formatdoc;
// use specta::{datatype, datatype::FunctionResultVariant};
// use specta_typescript as ts;
// use specta_typescript::js_doc;
// use tauri::Runtime;

// /// Implements [`ExportLanguage`] for JS exporting
// pub struct Language;

// pub fn builder<TRuntime: Runtime>() -> Builder<Language, NoCommands<TRuntime>, NoEvents> {
//     Builder::default()
// }

// pub const GLOBALS: &str = include_str!("./globals.js");

// type Config = specta_typescript::Typescript;

// pub type ExportConfig = crate::ExportConfig<Config>;

// impl ExportLanguage for Language {
//     type Config = Config;
//     type Error = ts::ExportError;

//     fn run_format(path: PathBuf, cfg: &ExportConfig) {
//         cfg.inner.run_format(path).ok();
//     }

//     /// Renders a collection of [`FunctionDataType`] into a JavaScript string.
//     fn render_commands(
//         commands: &[datatype::Function],
//         type_map: &TypeMap,
//         cfg: &ExportConfig,
//     ) -> Result<String, Self::Error> {
//         let commands = commands
//             .iter()
//             .map(|function| {
//                 let jsdoc = {
//                     let ret_type = js_ts::handle_result(function, type_map, cfg)?;

//                     let mut builder = js_doc::Builder::default();

//                     if let Some(d) = function.deprecated() {
//                         builder.push_deprecated(d);
//                     }

//                     if !function.docs().is_empty() {
//                         builder.extend(function.docs().split("\n"));
//                     }

//                     builder.extend(function.args().flat_map(|(name, typ)| {
//                         ts::datatype(
//                             &cfg.inner,
//                             &FunctionResultVariant::Value(typ.clone()),
//                             type_map,
//                         )
//                         .map(|typ| {
//                             let name = name.to_lower_camel_case();

//                             format!("@param {{ {typ} }} {name}")
//                         })
//                     }));
//                     builder.push(&format!("@returns {{ Promise<{ret_type}> }}"));

//                     builder.build()
//                 };

//                 Ok(js_ts::function(
//                     &jsdoc,
//                     &function.name().to_lower_camel_case(),
//                     // TODO: Don't `collect` the whole thing
//                     &js_ts::arg_names(&function.args().cloned().collect::<Vec<_>>()),
//                     None,
//                     &js_ts::command_body(cfg, function, false),
//                 ))
//             })
//             .collect::<Result<Vec<_>, Self::Error>>()?
//             .join(",\n");

//         Ok(formatdoc! {
//             r#"
//             export const commands = {{
//             {commands}
//             }}"#
//         })
//     }

//     fn render_events(
//         events: &[EventDataType],
//         type_map: &TypeMap,
//         cfg: &ExportConfig,
//     ) -> Result<String, Self::Error> {
//         if events.is_empty() {
//             return Ok(Default::default());
//         }

//         let (events_types, events_map) = js_ts::events_data(events, cfg, type_map)?;

//         let events = {
//             let mut builder = js_doc::Builder::default();

//             builder.push("@type {typeof __makeEvents__<{");
//             builder.extend(events_types);
//             builder.push("}>}");

//             builder.build()
//         };

//         Ok(formatdoc! {
//             r#"
//             {events}
//             const __typedMakeEvents__ = __makeEvents__;

// 	        export const events = __typedMakeEvents__({{
// 	        {events_map}
// 	        }})"#
//         })
//     }

//     fn render(
//         commands: &[datatype::Function],
//         events: &[EventDataType],
//         type_map: &TypeMap,
//         statics: &StaticCollection,
//         cfg: &ExportConfig,
//     ) -> Result<String, Self::Error> {
//         let dependant_types = type_map
//             .iter()
//             .map(|(_sid, ndt)| js_doc::typedef_named_datatype(&cfg.inner, ndt, type_map))
//             .collect::<Result<Vec<_>, _>>()
//             .map(|v| v.join("\n"))?;

//         js_ts::render_all_parts::<Self>(
//             commands,
//             events,
//             type_map,
//             statics,
//             cfg,
//             &dependant_types,
//             GLOBALS,
//         )
//     }
// }
