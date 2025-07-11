//! Core utilities for the JS & TS language exporters.
//!
//! Typescript is a superset of Javascript so they share a lot of logic.

use std::{borrow::Cow, collections::BTreeMap};

use heck::ToLowerCamelCase;
use specta::{
    datatype::{self, DataType, FunctionResultVariant},
    TypeMap,
};
use specta_typescript::{self as ts};
use specta_typescript::{ExportError, Typescript};

use crate::{apply_as_prefix, ErrorHandlingMode, ExportContext, ItemType, LanguageExt};

const DO_NOT_EDIT: &str = "// This file was generated by [tauri-specta](https://github.com/specta-rs/tauri-specta). Do not edit this file manually.";

pub fn render_all_parts<L: LanguageExt>(
    cfg: &ExportContext,
    dependant_types: &str,
    globals: &str,
    header: &str,
    commands: String,
    events: String,
    as_const: bool,
) -> Result<String, L::Error> {
    let mut constants = cfg.constants.iter().collect::<Vec<_>>();
    constants.sort_by(|(a, _), (b, _)| a.cmp(b));
    let constants = constants
        .into_iter()
        .map(|(name, value)| {
            let mut as_constt = None;
            if as_const {
                match &value {
                    serde_json::Value::Null => {}
                    serde_json::Value::Bool(_)
                    | serde_json::Value::Number(_)
                    | serde_json::Value::String(_)
                    | serde_json::Value::Array(_)
                    | serde_json::Value::Object(_) => as_constt = Some(" as const"),
                }
            }

            format!(
                "export const {name} = {}{};",
                serde_json::to_string(&value)
                    .expect("failed to serialize from `serde_json::Value`"),
                as_constt.unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(format! {
        r#"{header}
{DO_NOT_EDIT}

/** user-defined commands **/

{commands}

/** user-defined events **/

{events}

/** user-defined constants **/

{constants}

/** user-defined types **/

{dependant_types}

/** tauri-specta globals **/

{globals}"#
    })
}

pub fn arg_names(args: &[(Cow<'static, str>, DataType)]) -> Vec<String> {
    args.iter()
        .map(|(name, _)| name.to_lower_camel_case())
        .collect::<Vec<_>>()
}

pub fn arg_usages(args: &[String]) -> Option<String> {
    (!args.is_empty()).then(|| format!("{{ {} }}", args.join(", ")))
}

fn return_as_result_tuple(expr: &str, as_any: bool) -> String {
    let as_any = as_any.then_some(" as any").unwrap_or_default();

    format!(
        r#"try {{
    return {{ status: "ok", data: {expr} }};
}} catch (e) {{
    if(e instanceof Error) throw e;
    else return {{ status: "error", error: e {as_any} }};
}}"#
    )
}

pub fn maybe_return_as_result_tuple(
    expr: &str,
    typ: Option<&FunctionResultVariant>,
    as_any: bool,
    error_handling: ErrorHandlingMode,
) -> String {
    match typ {
        Some(FunctionResultVariant::Result(_, _)) => match error_handling {
            ErrorHandlingMode::Throw => {
                format!("return {expr};")
            }
            ErrorHandlingMode::Result => return_as_result_tuple(expr, as_any),
        },
        Some(FunctionResultVariant::Value(_)) => format!("return {expr};"),
        None => format!("{expr};"),
    }
}

pub fn function(
    docs: &str,
    name: &str,
    args: &[String],
    return_type: Option<&str>,
    body: &str,
) -> String {
    let args = args.join(", ");
    let return_type = return_type
        .map(|t| format!(": Promise<{}>", t))
        .unwrap_or_default();

    format!(
        r#"{docs}async {name}({args}) {return_type} {{
    {body}
}}"#
    )
}

fn tauri_invoke(name: &str, arg_usages: Option<String>) -> String {
    let arg_usages = arg_usages.map(|u| format!(", {u}")).unwrap_or_default();

    format!(r#"await TAURI_INVOKE("{name}"{arg_usages})"#)
}

pub fn handle_result(
    function: &datatype::Function,
    type_map: &TypeMap,
    cfg: &Typescript,
    error_handling: ErrorHandlingMode,
) -> Result<String, ExportError> {
    Ok(match &function.result() {
        Some(FunctionResultVariant::Result(t, e)) => match error_handling {
            ErrorHandlingMode::Result => {
                format!(
                    "Result<{}, {}>",
                    ts::datatype(cfg, &FunctionResultVariant::Value(t.clone()), type_map)?,
                    ts::datatype(cfg, &FunctionResultVariant::Value(e.clone()), type_map)?
                )
            }
            ErrorHandlingMode::Throw => {
                ts::datatype(cfg, &FunctionResultVariant::Value(t.clone()), type_map)?
            }
        },
        Some(FunctionResultVariant::Value(t)) => {
            ts::datatype(cfg, &FunctionResultVariant::Value(t.clone()), type_map)?
        }
        None => "void".to_string(),
    })
}

pub fn command_body(
    plugin_name: &Option<&'static str>,
    function: &datatype::Function,
    as_any: bool,
    error_handling: ErrorHandlingMode,
) -> String {
    let name = plugin_name
        .as_ref()
        .map(|n| apply_as_prefix(&n, &function.name(), ItemType::Command))
        .unwrap_or_else(|| function.name().to_string());

    maybe_return_as_result_tuple(
        &tauri_invoke(
            &name,
            arg_usages(&arg_names(
                // TODO: Don't collect
                &function.args().cloned().collect::<Vec<_>>(),
            )),
        ),
        function.result(),
        as_any,
        error_handling,
    )
}

pub fn events_map(
    events: &BTreeMap<&'static str, DataType>,
    plugin_name: &Option<&'static str>,
) -> String {
    events
        .iter()
        .map(|(name, _)| {
            let name_str = plugin_name
                .as_ref()
                .map(|n| apply_as_prefix(n, name, ItemType::Event))
                .unwrap_or_else(|| name.to_string());
            let name_camel = name.to_lower_camel_case();

            format!(r#"{name_camel}: "{name_str}""#)
        })
        .collect::<Vec<_>>()
        .join(",\n")
}

pub fn events_types(
    events: &BTreeMap<&'static str, DataType>,
    cfg: &Typescript,
    type_map: &TypeMap,
) -> Result<Vec<String>, ExportError> {
    events
        .iter()
        .map(|(name, typ)| {
            let name_camel = name.to_lower_camel_case();

            let typ = ts::datatype(cfg, &FunctionResultVariant::Value(typ.clone()), type_map)?;

            Ok(format!(r#"{name_camel}: {typ}"#))
        })
        .collect()
}

pub fn events_data(
    events: &BTreeMap<&'static str, DataType>,
    cfg: &Typescript,
    plugin_name: &Option<&'static str>,
    type_map: &TypeMap,
) -> Result<(Vec<String>, String), ExportError> {
    Ok((
        events_types(events, cfg, type_map)?,
        events_map(events, plugin_name),
    ))
}
