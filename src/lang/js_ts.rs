use std::{borrow::Cow, path::Path};

use heck::ToLowerCamelCase;
use specta::TypeCollection;
use specta::datatype::{
    DataType, Field, FunctionReturnType, NamedDataTypeBuilder, Reference, Struct,
};
use specta_typescript::{Error, Exporter, FrameworkExporter, define, primitives};

use crate::{BuilderConfiguration, ErrorHandlingMode, LanguageExt};

impl LanguageExt for specta_typescript::Typescript {
    type Error = specta_typescript::Error;

    fn export(self, cfg: &BuilderConfiguration, path: &Path) -> Result<(), Self::Error> {
        Exporter::from(self)
            .framework_prelude(FRAMEWORK_HEADER)
            .framework_runtime({
                let cfg = cfg.clone();
                move |exporter| {
                    runtime(
                        exporter,
                        &cfg,
                        false,
                        if !cfg.typed_error_impl.is_empty() {
                            &cfg.typed_error_impl
                        } else {
                            TYPED_ERROR_IMPL_TS
                        },
                        TYPED_ERROR_ASSERTION_TS,
                        MAKE_EVENT_IMPL_TS,
                    )
                }
            })
            .export_to(path, &cfg.types)
    }
}

impl LanguageExt for specta_typescript::JSDoc {
    type Error = specta_typescript::Error;

    fn export(self, cfg: &BuilderConfiguration, path: &Path) -> Result<(), Self::Error> {
        Exporter::from(self)
            .framework_prelude(FRAMEWORK_HEADER)
            .framework_runtime({
                let cfg = cfg.clone();
                move |exporter| {
                    runtime(
                        exporter,
                        &cfg,
                        true,
                        if !cfg.typed_error_impl.is_empty() {
                            &cfg.typed_error_impl
                        } else {
                            TYPED_ERROR_IMPL_JS
                        },
                        "",
                        MAKE_EVENT_IMPL_JS,
                    )
                }
            })
            .export_to(path, &cfg.types)
    }
}

fn runtime(
    mut exporter: FrameworkExporter,
    cfg: &BuilderConfiguration,
    jsdoc: bool,
    typed_error_impl: &str,
    typed_error_assertion: &str,
    make_event_impl: &str,
) -> Result<Cow<'static, str>, Error> {
    let enabled_commands = !cfg.commands.is_empty();
    let enabled_events = !cfg.events.is_empty();

    let mut out = String::new();

    if let Some(ndt) = cfg
        .types
        .into_unsorted_iter()
        .find(|ndt| RESERVED_NDT_NAMES.contains(&&**ndt.name()))
    {
        return Err(Error::Framework(format!(
            "User defined type '{}' defined in {} must be renamed so it doesn't conflict with Tauri Specta runtime.",
            ndt.name(),
            ndt.location()
        ).into()));
    }

    let is_channel_used = cfg.commands.iter().any(|command| {
        // Check if any argument is a Channel
        command.args().iter().any(|(_, dt)| is_channel_type(dt, &cfg.types))
            // Check if result contains a Channel
            || command.result().is_none_or(|result| match result {
                FunctionReturnType::Value(dt) => is_channel_type(dt, &cfg.types),
                FunctionReturnType::Result(ok, err) =>
                    is_channel_type(ok, &cfg.types) || is_channel_type(err, &cfg.types),
            })
    });
    let has_typed_error = enabled_commands
        && cfg.error_handling == ErrorHandlingMode::Result
        && cfg
            .commands
            .iter()
            .any(|command| matches!(command.result(), Some(FunctionReturnType::Result(_, _))));

    if enabled_commands || is_channel_used {
        out.push_str("import { ");

        let imports = [
            enabled_commands.then_some("invoke as __TAURI_INVOKE"),
            is_channel_used.then_some("Channel"),
        ];

        let mut first = true;
        for import in imports.iter().flatten() {
            if !first {
                out.push_str(", ");
            }
            out.push_str(import);
            first = false;
        }

        out.push_str(" } from \"@tauri-apps/api/core\";\n");
    }
    if enabled_events {
        out.push_str("import * as __TAURI_EVENT from \"@tauri-apps/api/event\";\n");
    }

    // Commands
    if enabled_commands {
        let mut s = Struct::named();
        for command in &cfg.commands {
            let command_name_escaped = serde_json::to_string(
                &cfg.plugin_name
                    .map(|plugin_name| format!("plugin|{plugin_name}|{}", command.name()).into())
                    .unwrap_or_else(|| command.name().clone()),
            )
            .expect("failed to serialize string");

            let arguments = command
                .args()
                .iter()
                .map(|(name, dt)| {
                    Ok((
                        name.to_lower_camel_case(),
                        render_reference_dt(dt, &exporter)?,
                    ))
                })
                .collect::<Result<Vec<_>, Error>>()?;

            let fn_arguments = arguments
                .iter()
                .map(|(name, dt)| {
                    let mut arg = name.to_string();
                    if !jsdoc {
                        arg.push_str(": ");
                        arg.push_str(dt);
                    }
                    arg
                })
                .collect::<Vec<_>>()
                .join(", ");

            let arguments_invoke_obj = if command.args().is_empty() {
                Default::default()
            } else {
                format!(
                    ", {{ {} }}",
                    command
                        .args()
                        .iter()
                        .map(|(name, _)| name.to_lower_camel_case())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };

            let invoke_args = format!("({command_name_escaped}{arguments_invoke_obj})",);

            let body = if cfg.error_handling == ErrorHandlingMode::Result
                && let Some(FunctionReturnType::Result(dt_ok, dt_err)) = command.result()
            {
                let mut invoke_ts = "typedError".to_string();
                if !jsdoc {
                    invoke_ts.push('<');
                    invoke_ts.push_str(&render_reference_dt(dt_ok, &exporter)?);
                    invoke_ts.push_str(", ");
                    invoke_ts.push_str(&render_reference_dt(dt_err, &exporter)?);
                    invoke_ts.push('>');
                }
                invoke_ts.push_str("(__TAURI_INVOKE");
                invoke_ts.push_str(&invoke_args);
                invoke_ts.push(')');
                invoke_ts
            } else {
                let mut invoke_ts = "__TAURI_INVOKE".to_string();
                if !jsdoc {
                    invoke_ts.push('<');
                    invoke_ts.push_str(&match command.result() {
                        Some(FunctionReturnType::Value(dt) | FunctionReturnType::Result(dt, _)) => {
                            Cow::Owned(render_reference_dt(dt, &exporter)?)
                        }
                        None => Cow::Borrowed("void"),
                    });
                    invoke_ts.push('>');
                }
                invoke_ts.push_str(&invoke_args);
                invoke_ts
            };

            let mut field = Field::new(define(format!("({fn_arguments}) => {body}")).into());
            field.set_deprecated(command.deprecated().cloned());
            field.set_docs({
                let mut docs = command.docs().to_string();

                if jsdoc {
                    if !docs.is_empty() {
                        docs.push('\n');
                    }

                    docs.push_str(
                        &arguments
                            .iter()
                            .map(|(name, dt)| format!("@param {{{dt}}} {name}"))
                            .collect::<Vec<_>>()
                            .join("\n"),
                    );

                    if !arguments.is_empty() {
                        docs.push('\n');
                    }

                    docs.push_str("@returns {string} myName");
                }

                docs.into()
            });
            s = s.field(command.name().to_lower_camel_case(), field);
        }

        out.push_str("\n/** Commands */");
        out.push_str("\nexport const commands = ");
        out.push_str(&match &s.build() {
            DataType::Reference(r) => exporter.reference(r)?,
            dt => exporter.inline(dt)?,
        });
        out.push_str(";\n");
    }

    // Events
    if enabled_events {
        let mut s = Struct::named();
        for (name, (_, r)) in &cfg.events {
            let event_name_escaped = serde_json::to_string(
                &cfg.plugin_name
                    .map(|plugin_name| format!("plugin:{plugin_name}:{name}"))
                    .unwrap_or_else(|| name.to_string()),
            )
            .expect("failed to serialize string");

            let mut field_ts = "makeEvent".to_string();
            if !jsdoc {
                field_ts.push('<');
                field_ts.push_str(&exporter.reference(r)?);
                field_ts.push('>');
            }
            field_ts.push('(');
            field_ts.push_str(&event_name_escaped);
            field_ts.push(')');

            let mut field = Field::new(define(field_ts).into());
            field.set_docs(
                format!(
                    "@type {{ReturnType<typeof makeEvent<{}>>}}",
                    exporter.reference(r)?
                )
                .into(),
            );
            s = s.field(name.to_lower_camel_case(), field);
        }

        out.push_str("\n/** Events */");
        out.push_str("\nexport const events = ");
        out.push_str(&exporter.inline(&s.build())?);
        out.push_str(";\n");
    }

    // Constants
    if !cfg.constants.is_empty() {
        out.push_str("\n/* Constants */");

        let mut constants = cfg.constants.iter().collect::<Vec<_>>();
        constants.sort_by(|(a, _), (b, _)| a.cmp(b));
        for (name, value) in constants.iter() {
            let mut as_constt = None;
            // `as const` isn't supported in JS so are conditional on that.
            if !jsdoc {
                match &value {
                    serde_json::Value::Null => {}
                    serde_json::Value::Bool(_)
                    | serde_json::Value::Number(_)
                    | serde_json::Value::String(_)
                    | serde_json::Value::Array(_)
                    | serde_json::Value::Object(_) => as_constt = Some(" as const"),
                }
            }

            out.push_str("\nexport const ");
            out.push_str(name);
            out.push_str(" = ");
            out.push_str(
                &serde_json::to_string(&value)
                    .expect("failed to serialize from `serde_json::Value`"),
            );
            out.push_str(as_constt.unwrap_or(""));
            out.push_str(";\n");
        }
    }

    // User types
    let types = exporter.render_types()?;
    if !types.is_empty() {
        out.push_str("\n/* Types */");
        out.push_str(&types);
    }

    // Runtime
    if has_typed_error || enabled_events {
        out.push_str("\n/* Tauri Specta runtime */\n");

        if has_typed_error {
            out.push_str(typed_error_impl);
            out.push('\n');
            // We only include assertion for user-provided impl.
            // It's assumed the internal one is correct.
            if !cfg.typed_error_impl.is_empty() {
                out.push('\n');
                out.push_str(typed_error_assertion);
                out.push('\n');
            }
            if enabled_events {
                out.push('\n');
            }
        }
        if enabled_events {
            out.push_str(make_event_impl);
            out.push('\n');
        }
    }

    Ok(Cow::Owned(out))
}

fn is_channel_type(dt: &DataType, types: &TypeCollection) -> bool {
    match dt {
        DataType::Reference(Reference::Named(r)) => r
            .get(types)
            .map(|ndt| ndt.name() == "TAURI_CHANNEL" && ndt.module_path().starts_with("tauri::"))
            .unwrap_or(false),
        _ => false,
    }
}

// Render a `DataType` as a reference (or fallback to inline).
// Also handles Tauri channel references.
fn render_reference_dt(dt: &DataType, exporter: &FrameworkExporter) -> Result<String, Error> {
    if let DataType::Reference(Reference::Named(r)) = dt
        && let Some(ndt) = r.get(exporter.types)
        && ndt.name() == "TAURI_CHANNEL"
        && ndt.module_path().starts_with("tauri::")
    {
        let generic = if let Some((_, dt)) = r.generics().first() {
            match &dt {
                DataType::Reference(r) => exporter.reference(r)?,
                dt => exporter.inline(dt)?,
            }
            .into()
        } else {
            Cow::Borrowed("never")
        };
        Ok(format!("Channel<{generic}>"))
    } else {
        match &dt {
            DataType::Reference(r) => exporter.reference(r),
            dt => exporter.inline(dt),
        }
    }
}

const RESERVED_NDT_NAMES: &[&str] = &[
    "Channel",
    "__TAURI_EVENT",
    "__TAURI_INVOKE",
    "typedError",
    "makeEvent",
];

const FRAMEWORK_HEADER: &str =
    "// This file has been generated by Tauri Specta. Do not edit this file manually.";

const TYPED_ERROR_IMPL_TS: &str = r#"async function typedError<T, E>(result: Promise<T>): Promise<{ status: "ok"; data: T } | { status: "error"; error: E }> {
    try {
        return { status: "ok", data: await result };
    } catch (e) {
        if (e instanceof Error) throw e;
        return { status: "error", error: e as any };
    }
}"#;

const TYPED_ERROR_IMPL_JS: &str = r#"/**
  * @template T
  * @template E
  * @param {Promise<T>} result
  * @returns {Promise<{ status: "ok"; data: T } | { status: "error"; error: E }>}
  */
async function typedError(result) {
    try {
        return { status: "ok", data: await result };
    } catch (e) {
        if (e instanceof Error) throw e;
        return { status: "error", error: e };
    }
}"#;

const TYPED_ERROR_ASSERTION_TS: &str = "const _assertTypedErrorFollowsContract: <T, E>(result: Promise<T>) => Promise<any> = typedError;";

const MAKE_EVENT_IMPL_TS: &str = r#"function makeEvent<T>(name: string) {
    const base = {
        listen: (cb: __TAURI_EVENT.EventCallback<T>) => __TAURI_EVENT.listen(name, cb),
        once: (cb: __TAURI_EVENT.EventCallback<T>) => __TAURI_EVENT.once(name, cb),
        emit: ((payload: T) => __TAURI_EVENT.emit(name, payload) as unknown) as (T extends null ? () => Promise<void> : (payload: T) => Promise<void>)
    };

    const fn = (target: import("@tauri-apps/api/webview").Webview | import("@tauri-apps/api/window").Window) => ({
        listen: (cb: __TAURI_EVENT.EventCallback<T>) => target.listen(name, cb),
        once: (cb: __TAURI_EVENT.EventCallback<T>) => target.once(name, cb),
        emit: ((payload: T) => target.emit(name, payload) as unknown) as (T extends null ? () => Promise<void> : (payload: T) => Promise<void>)
    });

    return Object.assign(fn, base);
}"#;

const MAKE_EVENT_IMPL_JS: &str = r#"/**
 * @template T
 * @param {string} name
 */
function makeEvent(name) {
    const base = {
        /** @param {__TAURI_EVENT.EventCallback<T>} cb */
        listen: (cb) => __TAURI_EVENT.listen(name, cb),
        /** @param {__TAURI_EVENT.EventCallback<T>} cb */
        once: (cb) => __TAURI_EVENT.once(name, cb),
        /** @param {T} payload */
        emit: (payload) => __TAURI_EVENT.emit(name, payload),
    };

    /** @param {import("@tauri-apps/api/webview").Webview | import("@tauri-apps/api/window").Window} target */
    const fn = (target) => ({
        /** @param {__TAURI_EVENT.EventCallback<T>} cb */
        listen: (cb) => target.listen(name, cb),
        /** @param {__TAURI_EVENT.EventCallback<T>} cb */
        once: (cb) => target.once(name, cb),
        /** @param {T} payload */
        emit: (payload) => target.emit(name, payload),
    });

    return Object.assign(fn, base);
}"#;
