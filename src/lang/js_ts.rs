use std::{borrow::Cow, path::Path};

use heck::ToLowerCamelCase;
use specta::TypeCollection;
use specta::datatype::{DataType, Field, FunctionReturnType, Reference, Struct};
use specta_typescript::{Error, Exporter, FrameworkExporter, define};

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
                        true,
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
                        false,
                        if !cfg.typed_error_impl.is_empty() {
                            &cfg.typed_error_impl
                        } else {
                            TYPED_ERROR_IMPL_JS
                        },
                        TYPED_ERROR_ASSERTION_JS,
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
    as_const: bool,
    typed_error_impl: &str,
    typed_error_assertion: &str,
    make_event_impl: &str,
) -> Result<Cow<'static, str>, Error> {
    let enabled_commands = !cfg.commands.is_empty();
    let enabled_events = !cfg.events.is_empty();

    let mut out = String::new();

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

    // TODO: Apply rename to `TAURI_CHANNEl`.
    // Will be hard cause of this not being `&mut`
    // cfg.types = types.map(f);

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

        out.push_str(" } from '@tauri-apps/api/core';\n");
    }
    if enabled_events {
        out.push_str(
            r#"import * as __TAURI_EVENT from "@tauri-apps/api/event";
    import type { Webview } from "@tauri-apps/api/webview";
    import type { Window } from "@tauri-apps/api/window";
    "#,
        );
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
                    Ok(format!(
                        "{}: {}",
                        name.to_lower_camel_case(),
                        match &dt {
                            DataType::Reference(r) => exporter.reference(&r)?,
                            dt => exporter.inline(dt)?,
                        }
                    ))
                })
                .collect::<Result<Vec<_>, Error>>()?
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

            let body = if cfg.error_handling == ErrorHandlingMode::Result
                && let Some(FunctionReturnType::Result(dt_ok, dt_err)) = command.result()
            {
                format!(
                    "typedError<{}, {}>(__TAURI_INVOKE({command_name_escaped}{arguments_invoke_obj}))",
                    match &dt_ok {
                        DataType::Reference(r) => exporter.reference(&r)?,
                        dt => exporter.inline(dt)?,
                    },
                    match &dt_err {
                        DataType::Reference(r) => exporter.reference(&r)?,
                        dt => exporter.inline(dt)?,
                    }
                )
            } else {
                format!(
                    "__TAURI_INVOKE<{}>({command_name_escaped}{arguments_invoke_obj})",
                    match command.result() {
                        Some(FunctionReturnType::Value(dt) | FunctionReturnType::Result(dt, _)) =>
                            Cow::Owned(match dt {
                                DataType::Reference(r) => exporter.reference(r)?,
                                dt => exporter.inline(dt)?,
                            }),
                        None => Cow::Borrowed("void"),
                    },
                )
            };

            let mut field = Field::new(define(format!("({arguments}) => {body}")).into());
            field.set_deprecated(command.deprecated().cloned());
            field.set_docs(command.docs().clone());
            s = s.field(command.name().to_lower_camel_case(), field);
        }

        out.push_str("\n/* Commands */");
        // TODO: JSDoc support
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

            let field = Field::new(
                define(format!(
                    "makeEvent<{}>({event_name_escaped})",
                    exporter.reference(r)?
                ))
                .into(),
            );
            s = s.field(name.to_lower_camel_case(), field);
        }

        out.push_str("\n/* Events */");
        out.push_str("\nexport const events = ");
        out.push_str(&exporter.inline(&s.build())?); // TODO: JSDoc support
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

            // TODO: Don't use `format!`
            out.push_str(&format!(
                "\nexport const {name} = {}{};\n",
                serde_json::to_string(&value)
                    .expect("failed to serialize from `serde_json::Value`"),
                as_constt.unwrap_or("")
            ));
        }
    }

    // User types
    let types = exporter.render_types()?;
    if !types.is_empty() {
        out.push_str("\n/* Types */");
        out.push_str(&types);
    }

    // Runtime
    if enabled_commands || enabled_events {
        out.push_str("\n/* Tauri Specta runtime */\n");

        if enabled_commands {
            out.push_str(typed_error_impl);
            out.push('\n');
            if !cfg.typed_error_impl.is_empty() {
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

// TODO: JSDoc types
const TYPED_ERROR_IMPL_JS: &str = r#"async function typedError(result) {
    try {
        return { status: "ok", data: await result };
    } catch (e) {
        if (e instanceof Error) throw e;
        return { status: "error", error: e };
    }
}"#;

const TYPED_ERROR_ASSERTION_TS: &str = "const _assertTypedErrorFollowsContract: <T, E>(result: Promise<T>) => Promise<any> = typedError;";

// TODO: JSDoc types
const TYPED_ERROR_ASSERTION_JS: &str = "const _assertTypedErrorFollowsContract: <T, E>(result: Promise<T>) => Promise<any> = typedError;";

const MAKE_EVENT_IMPL_TS: &str = r#"function makeEvent<T>(name: string) {
    const base = {
        listen: (cb: __TAURI_EVENT.EventCallback<T>) => __TAURI_EVENT.listen(name, cb),
        once: (cb: __TAURI_EVENT.EventCallback<T>) => __TAURI_EVENT.once(name, cb),
        emit: (payload: T) => __TAURI_EVENT.emit(name, payload) as unknown as (T extends null ? () => Promise<void> : (payload: T) => Promise<void>)
    };

    const fn = (target: Webview | Window) => ({
        listen: (cb: __TAURI_EVENT.EventCallback<T>) => target.listen(name, cb),
        once: (cb: __TAURI_EVENT.EventCallback<T>) => target.once(name, cb),
        emit: (payload: T) => target.emit(name, payload) as unknown as (T extends null ? () => Promise<void> : (payload: T) => Promise<void>)
    });

    return Object.assign(fn, base);
}"#;

// TODO: JSDoc types
const MAKE_EVENT_IMPL_JS: &str = r#"function makeEvent(name) {
    const base = {
        listen: (cb) => __TAURI_EVENT.listen(name, cb),
        once: (cb) => __TAURI_EVENT.once(name, cb),
        emit: (payload) => __TAURI_EVENT.emit(name, payload),
    };

    const fn = (target) => ({
        listen: (cb) => target.listen(name, cb),
        once: (cb) => target.once(name, cb),
        emit: (payload) => target.emit(name, payload),
    });

    return Object.assign(fn, base);
}"#;
