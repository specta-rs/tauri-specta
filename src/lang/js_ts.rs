use std::{borrow::Cow, path::Path};

use heck::ToLowerCamelCase;
use specta::datatype::{DataType, Field, Fields, Function, Primitive, Reference, Struct};
use specta::{ResolvedTypes, Types};
use specta_serde::Phase;
use specta_tags::TransformPlan;
use specta_typescript::{Error, Exporter, FrameworkExporter, define};

use crate::{BuilderConfiguration, ErrorHandlingMode, LanguageExt};

impl LanguageExt for specta_typescript::Typescript {
    type Error = Error;

    fn export(self, cfg: &BuilderConfiguration, path: &Path) -> Result<(), Self::Error> {
        let cfg = cfg.clone();
        let types = resolve_types_for_export(&cfg)?;

        Exporter::from(self)
            .framework_prelude(FRAMEWORK_HEADER)
            .framework_runtime(move |exporter| {
                runtime(
                    exporter,
                    &cfg,
                    false,
                    if cfg.typed_error_impl.is_empty() {
                        &Cow::Borrowed(TYPED_ERROR_IMPL_TS)
                    } else {
                        &cfg.typed_error_impl
                    },
                    TYPED_ERROR_ASSERTION_TS,
                    MAKE_EVENT_IMPL_TS,
                )
            })
            .export_to(path, &types)
    }
}

impl LanguageExt for specta_typescript::JSDoc {
    type Error = Error;

    fn export(self, cfg: &BuilderConfiguration, path: &Path) -> Result<(), Self::Error> {
        let cfg = cfg.clone();
        let types = resolve_types_for_export(&cfg)?;

        Exporter::from(self)
            .framework_prelude(FRAMEWORK_HEADER)
            .framework_runtime(move |exporter| {
                runtime(
                    exporter,
                    &cfg,
                    true,
                    if cfg.typed_error_impl.is_empty() {
                        &Cow::Borrowed(TYPED_ERROR_IMPL_JS)
                    } else {
                        &cfg.typed_error_impl
                    },
                    "",
                    MAKE_EVENT_IMPL_JS,
                )
            })
            .export_to(path, &types)
    }
}

fn resolve_types_for_export(cfg: &BuilderConfiguration) -> Result<ResolvedTypes, Error> {
    if cfg.enable_nuanced_types && cfg.disable_serde_phases {
        return Err(Error::framework(
            "",
            "`unstable_nuanced_types` requires serde phase splitting; remove `disable_serde_phases()` to export bigint-style types".to_string(),
        ));
    }

    let mut types = cfg.types.clone();
    rewrite_bigints_in_types(&mut types, cfg.enable_nuanced_types);

    let types = if cfg.disable_serde_phases {
        specta_serde::apply(types)
    } else {
        specta_serde::apply_phases(types)
    }
    .map_err(|err| Error::framework("Specta Serde validation failed", err))?;

    Ok(types)
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
        return Err(Error::framework(
            "",
            format!(
                "User defined type '{}' defined in {} must be renamed so it doesn't conflict with Tauri Specta runtime.",
                ndt.name(),
                ndt.location()
            ),
        ));
    }

    let is_channel_used = cfg.commands.iter().any(|command| {
        // Check if any argument is a Channel
        command
            .args()
            .iter()
            .any(|(_, dt)| is_channel_type(dt, exporter.types))
            // Check if result contains a Channel
            || command.result().is_some_and(|result| {
                if let Some((ok, err)) = extract_std_result(result, exporter.types) {
                    is_channel_type(ok, exporter.types) || is_channel_type(err, exporter.types)
                } else {
                    is_channel_type(result, exporter.types)
                }
            })
    });
    let has_typed_error = enabled_commands
        && cfg.error_handling == ErrorHandlingMode::Result
        && cfg.commands.iter().any(|command| {
            command
                .result()
                .and_then(|dt| extract_std_result(dt, exporter.types))
                .is_some()
        });

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
            validate_exported_command(command, exporter.types)?;

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
                        render_reference_dt_for_phase(dt, Phase::Deserialize, &exporter, cfg)?,
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
                && let Some(result) = command.result()
                && let Some((dt_ok, dt_err)) = extract_std_result(result, exporter.types)
            {
                let ok_transform =
                    render_result_transform_for_phase(dt_ok, "v.data", &exporter, cfg);
                let err_transform =
                    render_result_transform_for_phase(dt_err, "v.error", &exporter, cfg);

                let mut invoke_ts = "typedError".to_string();
                if !jsdoc {
                    invoke_ts.push('<');
                    invoke_ts.push_str(&render_reference_dt_for_phase(
                        dt_ok,
                        if ok_transform.is_some() {
                            Phase::Deserialize
                        } else {
                            Phase::Serialize
                        },
                        &exporter,
                        cfg,
                    )?);
                    invoke_ts.push_str(", ");
                    invoke_ts.push_str(&render_reference_dt_for_phase(
                        dt_err,
                        if err_transform.is_some() {
                            Phase::Deserialize
                        } else {
                            Phase::Serialize
                        },
                        &exporter,
                        cfg,
                    )?);
                    invoke_ts.push('>');
                }
                invoke_ts.push_str("(__TAURI_INVOKE");
                invoke_ts.push_str(&invoke_args);
                invoke_ts.push(')');

                if ok_transform.is_none() && err_transform.is_none() {
                    invoke_ts
                } else {
                    let mapper = match (ok_transform, err_transform) {
                        (Some(ok), Some(err)) => format!(
                            "(v.status === \"ok\" ? {{ ...v, data: {ok} }} : v.status === \"error\" ? {{ ...v, error: {err} }} : v)"
                        ),
                        (Some(ok), None) => {
                            format!("(v.status === \"ok\" ? {{ ...v, data: {ok} }} : v)")
                        }
                        (None, Some(err)) => {
                            format!("(v.status === \"error\" ? {{ ...v, error: {err} }} : v)")
                        }
                        (None, None) => "v".to_string(),
                    };

                    format!("{invoke_ts}.then((v) => {mapper})")
                }
            } else {
                let mut invoke_ts = "__TAURI_INVOKE".to_string();
                let output_dt = command
                    .result()
                    .and_then(|dt| extract_std_result(dt, exporter.types).map(|(ok, _)| ok))
                    .or(command.result());

                if !jsdoc {
                    let output_transform = output_dt
                        .and_then(|dt| render_result_transform_for_phase(dt, "v", &exporter, cfg));

                    invoke_ts.push('<');
                    invoke_ts.push_str(&match command.result() {
                        Some(dt) => Cow::Owned(render_reference_dt(
                            &specta_serde::select_phase_datatype(
                                &rewrite_bigints_for_export(
                                    extract_std_result(dt, exporter.types)
                                        .map(|(ok, _)| ok)
                                        .unwrap_or(dt),
                                    cfg,
                                ),
                                exporter.types,
                                if output_transform.is_some() {
                                    Phase::Deserialize
                                } else {
                                    Phase::Serialize
                                },
                            ),
                            &exporter,
                        )?),
                        None => Cow::Borrowed("void"),
                    });
                    invoke_ts.push('>');
                }
                invoke_ts.push_str(&invoke_args);

                if let Some(dt) = output_dt
                    && let Some(mapped) = render_result_transform_for_phase(dt, "v", &exporter, cfg)
                {
                    format!("{invoke_ts}.then((v) => {mapped})")
                } else {
                    invoke_ts
                }
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
                let serialize = render_reference_dt_for_phase(
                    &DataType::Reference(r.clone()),
                    Phase::Serialize,
                    &exporter,
                    cfg,
                )?;
                let deserialize = render_reference_dt_for_phase(
                    &DataType::Reference(r.clone()),
                    Phase::Deserialize,
                    &exporter,
                    cfg,
                )?;
                field_ts.push('<');
                field_ts.push_str(&serialize);
                if serialize != deserialize {
                    field_ts.push_str(", ");
                    field_ts.push_str(&deserialize);
                }
                field_ts.push('>');
            }
            field_ts.push('(');
            field_ts.push_str(&event_name_escaped);
            field_ts.push(')');

            let mut field = Field::new(define(field_ts).into());
            if jsdoc {
                let serialize = render_reference_dt_for_phase(
                    &DataType::Reference(r.clone()),
                    Phase::Serialize,
                    &exporter,
                    cfg,
                )?;
                let deserialize = render_reference_dt_for_phase(
                    &DataType::Reference(r.clone()),
                    Phase::Deserialize,
                    &exporter,
                    cfg,
                )?;
                field.set_docs(
                    if serialize == deserialize {
                        format!("@type {{ReturnType<typeof makeEvent<{}>>}}", serialize)
                    } else {
                        format!(
                            "@type {{ReturnType<typeof makeEvent<{}, {}>>}}",
                            serialize, deserialize
                        )
                    }
                    .into(),
                );
            }
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
        if !types.starts_with('\n') {
            out.push('\n');
        }
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

fn rewrite_bigints_in_types(types: &mut Types, nuanced: bool) {
    types.iter_mut(|ndt| rewrite_bigints_in_datatype(ndt.ty_mut(), nuanced));
}

fn rewrite_bigints_in_datatype(dt: &mut DataType, nuanced: bool) {
    fn rewrite_bigints_in_fields(fields: &mut Fields, nuanced: bool) {
        match fields {
            Fields::Unit => {}
            Fields::Unnamed(fields) => {
                for field in fields.fields_mut() {
                    if let Some(ty) = field.ty_mut() {
                        rewrite_bigints_in_datatype(ty, nuanced);
                    }
                }
            }
            Fields::Named(fields) => {
                for (_, field) in fields.fields_mut() {
                    if let Some(ty) = field.ty_mut() {
                        rewrite_bigints_in_datatype(ty, nuanced);
                    }
                }
            }
        }
    }

    match dt {
        DataType::Primitive(primitive) => match primitive {
            Primitive::usize | Primitive::u64 | Primitive::u128 => {
                *dt = if nuanced {
                    specta_serde::phased(define("bigint | number").into(), define("bigint").into())
                } else {
                    DataType::Primitive(Primitive::u32)
                };
            }
            Primitive::isize | Primitive::i64 | Primitive::i128 => {
                *dt = if nuanced {
                    specta_serde::phased(define("bigint | number").into(), define("bigint").into())
                } else {
                    DataType::Primitive(Primitive::i32)
                };
            }
            Primitive::f128 => {
                *dt = DataType::Primitive(Primitive::f64);
            }
            _ => {}
        },
        DataType::List(list) => rewrite_bigints_in_datatype(list.ty_mut(), nuanced),
        DataType::Map(map) => {
            rewrite_bigints_in_datatype(map.key_ty_mut(), nuanced);
            rewrite_bigints_in_datatype(map.value_ty_mut(), nuanced);
        }
        DataType::Struct(strct) => rewrite_bigints_in_fields(strct.fields_mut(), nuanced),
        DataType::Enum(enm) => {
            for (_, variant) in enm.variants_mut() {
                rewrite_bigints_in_fields(variant.fields_mut(), nuanced);
            }
        }
        DataType::Tuple(tuple) => {
            for item in tuple.elements_mut() {
                rewrite_bigints_in_datatype(item, nuanced);
            }
        }
        DataType::Nullable(inner) => rewrite_bigints_in_datatype(inner, nuanced),
        DataType::Reference(Reference::Named(reference)) => {
            for (_, generic) in reference.generics_mut() {
                rewrite_bigints_in_datatype(generic, nuanced);
            }
        }
        DataType::Reference(Reference::Generic(_)) | DataType::Reference(Reference::Opaque(_)) => {}
    }
}

fn rewrite_bigints_for_export(dt: &DataType, cfg: &BuilderConfiguration) -> DataType {
    let mut dt = dt.clone();
    rewrite_bigints_in_datatype(&mut dt, cfg.enable_nuanced_types);
    dt
}

fn validate_exported_command(command: &Function, types: &ResolvedTypes) -> Result<(), Error> {
    for (position, (name, dt)) in command.args().iter().enumerate() {
        specta_serde::validate(dt, types).map_err(|err| {
            Error::framework(
                format!(
                    "Specta Serde validation failed for command '{}' param #{} ('{}')",
                    command.name(),
                    position + 1,
                    name
                ),
                err,
            )
        })?;
    }

    if let Some(result) = command.result() {
        specta_serde::validate(result, types).map_err(|err| {
            Error::framework(
                format!(
                    "Specta Serde validation failed for command '{}' result",
                    command.name()
                ),
                err,
            )
        })?;
    }

    Ok(())
}

fn extract_std_result<'a>(
    dt: &'a DataType,
    types: &'a ResolvedTypes,
) -> Option<(&'a DataType, &'a DataType)> {
    if let DataType::Reference(Reference::Named(r)) = dt
        && let Some(ndt) = r.get(types.as_types())
        && ndt.name() == "Result"
        && (ndt.module_path() == "std::result" || ndt.module_path() == "core::result")
        && let [(_, ok), (_, err), ..] = r.generics()
    {
        return Some((ok, err));
    }

    None
}

fn is_channel_type(dt: &DataType, types: &ResolvedTypes) -> bool {
    match dt {
        DataType::Reference(Reference::Named(r)) => r
            .get(types.as_types())
            .map(|ndt| ndt.name() == "TAURI_CHANNEL" && ndt.module_path().starts_with("tauri::"))
            .unwrap_or(false),
        _ => false,
    }
}

fn render_result_transform_for_phase(
    dt: &DataType,
    input: &str,
    exporter: &FrameworkExporter,
    cfg: &BuilderConfiguration,
) -> Option<String> {
    if !cfg.enable_nuanced_types {
        return None;
    }

    let mapped = TransformPlan::analyze(dt, exporter.types).map(input);
    (mapped != input).then(|| mapped.into_owned())
}

fn render_reference_dt_for_phase(
    dt: &DataType,
    phase: Phase,
    exporter: &FrameworkExporter,
    cfg: &BuilderConfiguration,
) -> Result<String, Error> {
    let dt = specta_serde::select_phase_datatype(
        &rewrite_bigints_for_export(dt, cfg),
        exporter.types,
        phase,
    );
    render_reference_dt(&dt, exporter)
}

// Render a `DataType` as a reference (or fallback to inline).
// Also handles Tauri channel references.
fn render_reference_dt(dt: &DataType, exporter: &FrameworkExporter) -> Result<String, Error> {
    if let DataType::Reference(Reference::Named(r)) = dt
        && let Some(ndt) = r.get(exporter.types.as_types())
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

const MAKE_EVENT_IMPL_TS: &str = r#"function makeEvent<TListen, TEmit = TListen>(name: string) {
    const base = {
        listen: (cb: __TAURI_EVENT.EventCallback<TListen>) => __TAURI_EVENT.listen(name, cb),
        once: (cb: __TAURI_EVENT.EventCallback<TListen>) => __TAURI_EVENT.once(name, cb),
        emit: ((payload: TEmit) => __TAURI_EVENT.emit(name, payload) as unknown) as (TEmit extends null ? () => Promise<void> : (payload: TEmit) => Promise<void>)
    };

    const fn = (target: import("@tauri-apps/api/webview").Webview | import("@tauri-apps/api/window").Window) => ({
        listen: (cb: __TAURI_EVENT.EventCallback<TListen>) => target.listen(name, cb),
        once: (cb: __TAURI_EVENT.EventCallback<TListen>) => target.once(name, cb),
        emit: ((payload: TEmit) => target.emit(name, payload) as unknown) as (TEmit extends null ? () => Promise<void> : (payload: TEmit) => Promise<void>)
    });

    return Object.assign(fn, base);
}"#;

const MAKE_EVENT_IMPL_JS: &str = r#"/**
 * @template TListen
 * @template [TEmit=TListen]
 * @param {string} name
 */
function makeEvent(name) {
    const base = {
        /** @param {__TAURI_EVENT.EventCallback<TListen>} cb */
        listen: (cb) => __TAURI_EVENT.listen(name, cb),
        /** @param {__TAURI_EVENT.EventCallback<TListen>} cb */
        once: (cb) => __TAURI_EVENT.once(name, cb),
        /** @param {TEmit} payload */
        emit: (payload) => __TAURI_EVENT.emit(name, payload),
    };

    /** @param {import("@tauri-apps/api/webview").Webview | import("@tauri-apps/api/window").Window} target */
    const fn = (target) => ({
        /** @param {__TAURI_EVENT.EventCallback<TListen>} cb */
        listen: (cb) => target.listen(name, cb),
        /** @param {__TAURI_EVENT.EventCallback<TListen>} cb */
        once: (cb) => target.once(name, cb),
        /** @param {TEmit} payload */
        emit: (payload) => target.emit(name, payload),
    });

    return Object.assign(fn, base);
}"#;
