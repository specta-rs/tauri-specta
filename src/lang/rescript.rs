use std::path::Path;

use heck::ToLowerCamelCase;
use specta::{
    Types,
    datatype::{DataType, Function, Reference},
};
use specta_rescript::{Error, ReScript, primitives::datatype_to_rescript};

use crate::{BuilderConfiguration, ErrorHandlingMode, LanguageExt};

/// Implements [`LanguageExt`] for [`specta_rescript::ReScript`], allowing Tauri Specta to export
/// commands and events as a ReScript `.res` bindings file.
///
/// # Example
///
/// ```rust
/// use tauri_specta::{Builder, collect_commands};
/// use specta_rescript::ReScript;
///
/// let builder = Builder::<tauri::Wry>::new();
///
/// #[cfg(debug_assertions)]
/// builder
///     .export(ReScript::default(), "../src/Bindings.res")
///     .expect("Failed to export ReScript bindings");
/// ```
impl LanguageExt for ReScript {
    type Error = Error;

    fn export(self, config: &BuilderConfiguration, path: &Path) -> Result<(), Self::Error> {
        let content = render(&self, config)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Only write if content changed to avoid triggering file watchers unnecessarily.
        if std::fs::read_to_string(path).ok().as_deref() != Some(&content) {
            std::fs::write(path, content)?;
        }
        Ok(())
    }
}

fn render(exporter: &ReScript, config: &BuilderConfiguration) -> Result<String, Error> {
    let mut out = String::new();
    render_header(exporter, &mut out);
    render_types(exporter, &config.types, &mut out)?;
    render_commands(config, &mut out)?;
    render_events(config, &mut out)?;
    render_constants(config, &mut out);
    Ok(out)
}

fn render_header(exporter: &ReScript, out: &mut String) {
    if exporter.header.is_empty() {
        return;
    }
    out.push_str(&exporter.header);
    out.push('\n');
}

fn render_types(exporter: &ReScript, types: &Types, out: &mut String) -> Result<(), Error> {
    let output = ReScript::export(&exporter.clone().header(""), types)?;
    if !output.trim().is_empty() {
        out.push_str(&output);
    }
    Ok(())
}

fn render_channel(config: &BuilderConfiguration, out: &mut String) {
    let has_channel = config.commands.iter().any(|cmd| {
        cmd.args()
            .iter()
            .any(|(_, dt)| is_tauri_channel(&config.types, dt))
    });

    if has_channel {
        out.push_str("\ntype tauriChannel__<'a>\n");
    }
}

fn render_typed_error(config: &BuilderConfiguration, out: &mut String) {
    if has_typed_error(config) {
        out.push_str(TYPED_ERROR_IMPL);
    }
}
fn render_events(config: &BuilderConfiguration, out: &mut String) -> Result<(), Error> {
    if config.events.is_empty() {
        return Ok(());
    }

    out.push_str("\ntype tauriEventCallback__<'a> = {\"payload\": 'a} => unit\n");
    out.push_str("type tauriEvent__<'a> = {\n  \"listen\": tauriEventCallback__<'a> => promise<unit => unit>,\n  \"once\": tauriEventCallback__<'a> => promise<unit => unit>,\n  \"emit\": 'a => promise<unit>,\n}\n");
    out.push_str("@module(\"@tauri-apps/api/event\") external tauriListen__: (string, tauriEventCallback__<'a>) => promise<unit => unit> = \"listen\"\n");
    out.push_str("@module(\"@tauri-apps/api/event\") external tauriOnce__: (string, tauriEventCallback__<'a>) => promise<unit => unit> = \"once\"\n");
    out.push_str("@module(\"@tauri-apps/api/event\") external tauriEmit__: (string, 'a) => promise<unit> = \"emit\"\n");
    out.push_str(MAKE_EVENT_IMPL);
    out.push_str("\nmodule Events = {\n");
    for (name, (_, r)) in &config.events {
        let name_json = format_name_json(config.plugin_name, name, ':');
        let fn_name = name.to_lower_camel_case();
        let payload_type = format_dt(&config.types, &DataType::Reference(r.clone()))?;

        out.push_str(&format!(
            "  let {fn_name}: tauriEvent__<{payload_type}> = makeEvent__({name_json})\n"
        ));
    }
    out.push_str("}\n");
    Ok(())
}

fn render_commands(config: &BuilderConfiguration, out: &mut String) -> Result<(), Error> {
    if config.commands.is_empty() {
        return Ok(());
    }

    render_channel(config, out);
    out.push_str(
        "\n@module(\"@tauri-apps/api/core\") external tauriInvoke__: (string, {..}) => promise<'a> = \"invoke\"\n",
    );
    render_typed_error(config, out);

    // TODO: emit doc comments (/** ... */ and @deprecated) for each command.
    // We should be able to reuse the existing JSDoc code.
    out.push_str("\nmodule Commands = {\n");
    for command in &config.commands {
        let fn_name = command.name().to_lower_camel_case();

        let (labeled, fields) = get_arg_map(command, config)?;
        let return_type = format_return(command, config)?;

        let fn_args = format_fn_args(&labeled);
        let invoke_obj = format_invoke_obj(&fields);
        let body = format_body(&command, &config, &invoke_obj);

        out.push_str(&format!(
            "  let {fn_name} = {fn_args}: promise<{return_type}> =>\n    {body}\n"
        ));
    }
    out.push_str("}\n");
    Ok(())
}

fn get_arg_map(
    command: &Function,
    config: &BuilderConfiguration,
) -> Result<(Vec<String>, Vec<String>), Error> {
    command
        .args()
        .iter()
        .map(|(n, dt)| {
            let name = n.to_lower_camel_case();
            let ty = format_dt(&config.types, dt)?;
            Ok((format!("~{name}: {ty}"), format!("\"{name}\": {name}")))
        })
        .collect::<Result<Vec<(String, String)>, Error>>()
        .map(|v| v.into_iter().unzip())
}

fn format_return(command: &Function, config: &BuilderConfiguration) -> Result<String, Error> {
    match command.result() {
        None => Ok("unit".to_string()),
        Some(dt) if config.error_handling == ErrorHandlingMode::Result => {
            format_dt(&config.types, dt)
        }
        Some(dt) => match extract_std_result(dt, &config.types) {
            None => format_dt(&config.types, dt),
            Some((ok_dt, _)) => format_dt(&config.types, ok_dt),
        },
    }
}

fn format_fn_args(labeled: &[String]) -> String {
    if labeled.is_empty() {
        return "()".to_string();
    }
    format!("({})", labeled.join(", "))
}

fn format_invoke_obj(fields: &[String]) -> String {
    if fields.is_empty() {
        return "Object.make()".to_string();
    }
    format!("{{{}}}", fields.join(", "))
}

fn format_body(command: &Function, config: &BuilderConfiguration, invoke_obj: &str) -> String {
    let name_json = format_name_json(config.plugin_name, command.name(), '|');

    if has_typed_error(config)
        && command
            .result()
            .and_then(|dt| extract_std_result(dt, &config.types))
            .is_some()
    {
        format!("typedError__(tauriInvoke__({name_json}, {invoke_obj}))")
    } else {
        format!("tauriInvoke__({name_json}, {invoke_obj})")
    }
}

fn format_name_json(plugin_name: Option<&'static str>, name: &str, sep: char) -> String {
    let full = plugin_name.map_or_else(
        || name.to_string(),
        |p| format!("plugin{sep}{p}{sep}{name}"),
    );
    serde_json::to_string(&full).expect("failed to serialize string")
}

fn has_typed_error(config: &BuilderConfiguration) -> bool {
    config.error_handling == ErrorHandlingMode::Result
        && config.commands.iter().any(|cmd| {
            cmd.result()
                .and_then(|dt| extract_std_result(dt, &config.types))
                .is_some()
        })
}

/// Returns `true` if `dt` is a reference to the Tauri Channel type.
fn is_tauri_channel(types: &Types, dt: &DataType) -> bool {
    let DataType::Reference(Reference::Named(r)) = dt else {
        return false;
    };
    let Some(ndt) = r.get(types) else {
        return false;
    };
    ndt.name() == "TAURI_CHANNEL" && ndt.module_path().starts_with("tauri::")
}

/// Render a DataType to a ReScript type string, with special handling for Tauri's Channel type
/// and `std::result::Result`.
fn format_dt(types: &Types, dt: &DataType) -> Result<String, Error> {
    if let DataType::Reference(Reference::Named(r)) = dt {
        if is_tauri_channel(types, dt) {
            let generic = r
                .generics()
                .first()
                .map(|(_, g)| format_dt(types, g))
                .transpose()?
                .unwrap_or_else(|| "'a".to_string());
            return Ok(format!("tauriChannel__<{generic}>"));
        }
        if let Some((ok_dt, err_dt)) = extract_std_result(dt, types) {
            return Ok(format!(
                "result<{}, {}>",
                format_dt(types, ok_dt)?,
                format_dt(types, err_dt)?
            ));
        }
    }
    datatype_to_rescript(types, &[], dt)
}

fn extract_std_result<'a>(
    dt: &'a DataType,
    types: &'a Types,
) -> Option<(&'a DataType, &'a DataType)> {
    let DataType::Reference(Reference::Named(r)) = dt else {
        return None;
    };
    let ndt = r.get(types)?;
    if ndt.name() != "Result" {
        return None;
    }
    let module_path = ndt.module_path();
    if module_path != "std::result" && module_path != "core::result" {
        return None;
    }
    let mut generics = r.generics().iter();
    let (_, ok) = generics.next()?;
    let (_, err) = generics.next()?;
    Some((ok, err))
}

fn render_constants(config: &BuilderConfiguration, out: &mut String) {
    if config.constants.is_empty() {
        return;
    }
    out.push_str("\n/* Constants */\n");
    let mut constants: Vec<_> = config.constants.iter().collect();
    constants.sort_by(|(a, _), (b, _)| a.cmp(b));
    for (name, value) in constants {
        let rescript_value = constant_value_to_rescript(value);
        out.push_str(&format!("let {name} = {rescript_value}\n"));
    }
}

fn constant_value_to_rescript(value: &serde_json::Value) -> String {
    serde_json::to_string(value).expect("failed to serialize constant value")
}

/// Typed-error helper: wraps a promise into `result<ok, err>` using ReScript's built-in type.
/// Uses ReScript v11 async/await with `switch await` exception handling.
/// Tauri rejects with the raw error value, so `Obj.magic` is used to coerce the caught exception.
const TYPED_ERROR_IMPL: &str = r#"
%%private(
  let typedError__: promise<'a> => promise<result<'a, 'e>> = async promise =>
    switch await promise {
    | exception e => Error(Obj.magic(e))
    | v => Ok(v)
    }
)
"#;

/// makeEvent__ helper: given an event name, returns a typed `tauriEvent__<'a>` object with
/// listen / once / emit methods.  The `'a` type variable is unified at each call site via the
/// explicit type annotation on the individual event `let` bindings.
const MAKE_EVENT_IMPL: &str = r#"
%%private(
  let makeEvent__ = (name: string): tauriEvent__<'a> => {
    "listen": cb => tauriListen__(name, cb),
    "once": cb => tauriOnce__(name, cb),
    "emit": payload => tauriEmit__(name, payload),
  }
)
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use specta::{Types, function::fn_datatype};
    use specta_rescript::ReScript;

    // ── helper functions for constructing Function instances ──────────────────

    #[specta::specta]
    fn cmd_no_args() {}

    #[specta::specta]
    fn cmd_one_arg(name: String) {}

    #[specta::specta]
    fn cmd_two_args(name: String, age: i32) {}

    #[specta::specta]
    fn cmd_returns_string() -> String {
        unimplemented!()
    }

    #[specta::specta]
    fn cmd_returns_result() -> Result<String, i32> {
        unimplemented!()
    }

    // ── get_arg_map ───────────────────────────────────────────────────────────

    #[test]
    fn get_arg_map_no_args_returns_empty_vecs() {
        let cmd = fn_datatype!(cmd_no_args)(&mut Types::default());
        let config = BuilderConfiguration::default();
        let (labeled, fields) = get_arg_map(&cmd, &config).unwrap();
        assert!(labeled.is_empty());
        assert!(fields.is_empty());
    }

    #[test]
    fn get_arg_map_single_arg() {
        let mut types = Types::default();
        let cmd = fn_datatype!(cmd_one_arg)(&mut types);
        let config = BuilderConfiguration {
            types,
            ..Default::default()
        };
        let (labeled, fields) = get_arg_map(&cmd, &config).unwrap();
        assert_eq!(labeled, vec!["~name: string"]);
        assert_eq!(fields, vec![r#""name": name"#]);
    }

    #[test]
    fn get_arg_map_two_args() {
        let mut types = Types::default();
        let cmd = fn_datatype!(cmd_two_args)(&mut types);
        let config = BuilderConfiguration {
            types,
            ..Default::default()
        };
        let (labeled, fields) = get_arg_map(&cmd, &config).unwrap();
        assert_eq!(labeled, vec!["~name: string", "~age: int"]);
        assert_eq!(fields, vec![r#""name": name"#, r#""age": age"#]);
    }

    // ── format_return ─────────────────────────────────────────────────────────

    #[test]
    fn format_return_no_result_is_unit() {
        let cmd = fn_datatype!(cmd_no_args)(&mut Types::default());
        let config = BuilderConfiguration::default();
        assert_eq!(format_return(&cmd, &config).unwrap(), "unit");
    }

    #[test]
    fn format_return_non_result_type_with_result_mode() {
        let mut types = Types::default();
        let cmd = fn_datatype!(cmd_returns_string)(&mut types);
        let config = BuilderConfiguration {
            error_handling: ErrorHandlingMode::Result,
            types,
            ..Default::default()
        };
        assert_eq!(format_return(&cmd, &config).unwrap(), "string");
    }

    #[test]
    fn format_return_result_type_with_result_mode() {
        let mut types = Types::default();
        let cmd = fn_datatype!(cmd_returns_result)(&mut types);
        let config = BuilderConfiguration {
            error_handling: ErrorHandlingMode::Result,
            types,
            ..Default::default()
        };
        assert_eq!(format_return(&cmd, &config).unwrap(), "result<string, int>");
    }

    #[test]
    fn format_return_result_type_with_throw_mode_unwraps_ok() {
        let mut types = Types::default();
        let cmd = fn_datatype!(cmd_returns_result)(&mut types);
        let config = BuilderConfiguration {
            error_handling: ErrorHandlingMode::Throw,
            types,
            ..Default::default()
        };
        assert_eq!(format_return(&cmd, &config).unwrap(), "string");
    }

    // ── format_body ───────────────────────────────────────────────────────────

    #[test]
    fn format_body_throw_mode_plain_invoke() {
        let cmd = fn_datatype!(cmd_no_args)(&mut Types::default());
        let config = BuilderConfiguration {
            error_handling: ErrorHandlingMode::Throw,
            ..Default::default()
        };
        assert_eq!(
            format_body(&cmd, &config, "Object.make()"),
            r#"tauriInvoke__("cmd_no_args", Object.make())"#
        );
    }

    #[test]
    fn format_body_result_mode_result_command_wraps_typed_error() {
        let mut types = Types::default();
        let cmd = fn_datatype!(cmd_returns_result)(&mut types);
        let config = BuilderConfiguration {
            error_handling: ErrorHandlingMode::Result,
            commands: vec![cmd.clone()],
            types,
            ..Default::default()
        };
        assert_eq!(
            format_body(&cmd, &config, "Object.make()"),
            r#"typedError__(tauriInvoke__("cmd_returns_result", Object.make()))"#
        );
    }

    #[test]
    fn format_body_result_mode_non_result_command_plain_invoke() {
        let mut types = Types::default();
        let result_cmd = fn_datatype!(cmd_returns_result)(&mut types);
        let plain_cmd = fn_datatype!(cmd_no_args)(&mut types);
        // Config has typed error enabled (a Result command exists), but this
        // specific command doesn't return Result so it gets a plain invoke.
        let config = BuilderConfiguration {
            error_handling: ErrorHandlingMode::Result,
            commands: vec![result_cmd],
            types,
            ..Default::default()
        };
        assert_eq!(
            format_body(&plain_cmd, &config, "Object.make()"),
            r#"tauriInvoke__("cmd_no_args", Object.make())"#
        );
    }

    #[test]
    fn format_fn_args_empty_returns_unit() {
        assert_eq!(format_fn_args(&[]), "()");
    }

    #[test]
    fn format_fn_args_single() {
        assert_eq!(
            format_fn_args(&["~foo: string".to_string()]),
            "(~foo: string)"
        );
    }

    #[test]
    fn format_fn_args_multiple() {
        let args = vec!["~foo: string".to_string(), "~bar: int".to_string()];
        assert_eq!(format_fn_args(&args), "(~foo: string, ~bar: int)");
    }

    #[test]
    fn format_invoke_obj_empty_returns_object_make() {
        assert_eq!(format_invoke_obj(&[]), "Object.make()");
    }

    #[test]
    fn format_invoke_obj_single() {
        assert_eq!(
            format_invoke_obj(&["\"foo\": foo".to_string()]),
            "{\"foo\": foo}"
        );
    }

    #[test]
    fn format_invoke_obj_multiple() {
        let fields = vec!["\"foo\": foo".to_string(), "\"bar\": bar".to_string()];
        assert_eq!(format_invoke_obj(&fields), "{\"foo\": foo, \"bar\": bar}");
    }

    #[test]
    fn format_name_json_no_plugin() {
        assert_eq!(format_name_json(None, "myCommand", '|'), "\"myCommand\"");
    }

    #[test]
    fn format_name_json_with_plugin_command_sep() {
        assert_eq!(
            format_name_json(Some("my_plugin"), "myCommand", '|'),
            "\"plugin|my_plugin|myCommand\""
        );
    }

    #[test]
    fn format_name_json_with_plugin_event_sep() {
        assert_eq!(
            format_name_json(Some("my_plugin"), "myEvent", ':'),
            "\"plugin:my_plugin:myEvent\""
        );
    }

    // ── render (integration) ──────────────────────────────────────────────────

    #[test]
    fn render_commands_emits_module_with_binding() {
        let mut types = Types::default();
        let cmd = fn_datatype!(cmd_one_arg)(&mut types);
        let config = BuilderConfiguration {
            commands: vec![cmd],
            types,
            error_handling: ErrorHandlingMode::Throw,
            ..Default::default()
        };
        let out = render(&ReScript::default().header(""), &config).unwrap();
        assert!(out.contains("module Commands"), "missing Commands module");
        assert!(out.contains("let cmdOneArg"), "missing camelCase binding");
        assert!(out.contains("~name: string"), "missing labeled arg");
    }

    #[test]
    fn render_commands_result_mode_emits_typed_error_impl() {
        let mut types = Types::default();
        let cmd = fn_datatype!(cmd_returns_result)(&mut types);
        let config = BuilderConfiguration {
            error_handling: ErrorHandlingMode::Result,
            commands: vec![cmd],
            types,
            ..Default::default()
        };
        let out = render(&ReScript::default().header(""), &config).unwrap();
        assert!(out.contains("typedError__"), "missing typedError__ helper");
        assert!(
            out.contains("result<string, int>"),
            "missing result return type"
        );
    }

    #[test]
    fn render_events_emits_module_with_binding() {
        use specta::Type;
        use std::any::TypeId;
        use std::collections::BTreeMap;

        let mut types = Types::default();
        let DataType::Reference(r) = String::definition(&mut types) else {
            panic!("String::definition should return a Reference");
        };
        let config = BuilderConfiguration {
            events: BTreeMap::from([("my-event", (TypeId::of::<String>(), r))]),
            types,
            ..Default::default()
        };
        let out = render(&ReScript::default().header(""), &config).unwrap();
        assert!(out.contains("module Events"), "missing Events module");
        assert!(
            out.contains("let myEvent: tauriEvent__<string>"),
            "missing typed event binding"
        );
        assert!(
            out.contains("makeEvent__(\"my-event\")"),
            "missing makeEvent__ call"
        );
        assert!(out.contains("tauriListen__"), "missing listen external");
        assert!(out.contains("tauriEmit__"), "missing emit external");
    }

    #[test]
    fn render_constants_emits_let_bindings() {
        use std::collections::BTreeMap;

        let config = BuilderConfiguration {
            constants: BTreeMap::from([
                ("myNumber".into(), serde_json::json!(42)),
                ("myString".into(), serde_json::json!("hello")),
                ("myBool".into(), serde_json::json!(true)),
            ]),
            ..Default::default()
        };
        let out = render(&ReScript::default().header(""), &config).unwrap();
        assert!(out.contains("/* Constants */"), "missing Constants header");
        assert!(out.contains("let myBool = true"), "missing bool constant");
        assert!(out.contains("let myNumber = 42"), "missing number constant");
        assert!(
            out.contains("let myString = \"hello\""),
            "missing string constant"
        );
        // constants are sorted alphabetically
        let bool_pos = out.find("let myBool").unwrap();
        let num_pos = out.find("let myNumber").unwrap();
        assert!(
            bool_pos < num_pos,
            "constants should be sorted alphabetically"
        );
    }

    // ── export ────────────────────────────────────────────────────────────────

    #[test]
    fn export_creates_file_with_correct_content() {
        let path = std::env::temp_dir().join(format!("tauri_specta_export_{}.res", line!()));
        let _ = std::fs::remove_file(&path);

        <ReScript as LanguageExt>::export(
            ReScript::default().header("// export test"),
            &BuilderConfiguration::default(),
            &path,
        )
        .unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("// export test\n"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn export_creates_parent_directories() {
        let dir = std::env::temp_dir().join(format!("tauri_specta_mkdir_{}", line!()));
        let path = dir.join("nested").join("bindings.res");
        let _ = std::fs::remove_dir_all(&dir);

        <ReScript as LanguageExt>::export(
            ReScript::default().header(""),
            &BuilderConfiguration::default(),
            &path,
        )
        .unwrap();

        assert!(path.exists(), "file should have been created in nested dir");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn render_no_content_with_no_header_is_empty() {
        let output = render(
            &ReScript::default().header(""),
            &BuilderConfiguration::default(),
        )
        .unwrap();
        assert_eq!(output, "");
    }

    #[test]
    fn render_default_includes_header_only() {
        let output = render(&ReScript::default(), &BuilderConfiguration::default()).unwrap();
        assert!(output.starts_with("// This file has been generated by Specta. DO NOT EDIT."));
        assert!(!output.contains("module Commands"));
        assert!(!output.contains("module Events"));
        assert!(!output.contains("tauriInvoke__"));
    }

    #[test]
    fn render_custom_header() {
        let config = ReScript::default().header("// My custom header");
        let output = render(&config, &BuilderConfiguration::default()).unwrap();
        assert!(output.starts_with("// My custom header\n"));
        assert!(!output.contains("module Commands"));
    }
}
