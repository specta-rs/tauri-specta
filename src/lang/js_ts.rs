use std::{borrow::Cow, path::Path};

use heck::ToLowerCamelCase;
use specta::{
    Format, Types,
    datatype::{DataType, Field, Fields, NamedReferenceType, Primitive, Reference, Struct},
};
use specta_serde::Phase;
use specta_tags::TransformPlan;
use specta_typescript::{Error, Exporter, FrameworkExporter, define};

use crate::name::{resolve_tauri_command_name, resolve_tauri_event_name};
use crate::{BuilderConfiguration, ErrorHandlingMode, LanguageExt};

#[derive(Debug, Clone, Copy)]
struct SerdeExportFormat {
    disable_serde_phases: bool,
    enable_nuanced_types: bool,
}

impl Format for SerdeExportFormat {
    fn map_types(&'_ self, types: &Types) -> Result<Cow<'_, Types>, specta::FormatError> {
        if self.disable_serde_phases {
            specta_serde::Format.map_types(types)
        } else {
            specta_serde::PhasesFormat.map_types(types)
        }
    }

    fn map_type(
        &'_ self,
        types: &Types,
        dt: &DataType,
    ) -> Result<Cow<'_, DataType>, specta::FormatError> {
        let dt = if self.disable_serde_phases {
            specta_serde::Format.map_type(types, dt)?
        } else {
            specta_serde::PhasesFormat.map_type(types, dt)?
        };

        Ok(rewrite_bigints_in_datatype(
            dt,
            self.enable_nuanced_types,
            !self.disable_serde_phases,
        ))
    }
}

impl LanguageExt for specta_typescript::Typescript {
    type Error = Error;

    fn export(self, cfg: &BuilderConfiguration, path: &Path) -> Result<(), Self::Error> {
        let cfg = cfg.clone();
        let types = hide_unused_std_result_type(&cfg, cfg.types.clone());
        let format = SerdeExportFormat {
            disable_serde_phases: cfg.disable_serde_phases,
            enable_nuanced_types: cfg.enable_nuanced_types,
        };

        Exporter::from(self)
            .framework_prelude(FRAMEWORK_HEADER)
            .framework_runtime(move |exporter| {
                runtime(
                    exporter,
                    &cfg,
                    false,
                    if cfg.typed_error_impl.is_empty() {
                        TYPED_ERROR_IMPL_TS
                    } else {
                        &cfg.typed_error_impl
                    },
                    TYPED_ERROR_ASSERTION_TS,
                    MAKE_EVENT_IMPL_TS,
                )
            })
            .export_to(path, &types, format)
    }
}

impl LanguageExt for specta_typescript::JSDoc {
    type Error = Error;

    fn export(self, cfg: &BuilderConfiguration, path: &Path) -> Result<(), Self::Error> {
        let cfg = cfg.clone();
        let types = hide_unused_std_result_type(&cfg, cfg.types.clone());
        let format = SerdeExportFormat {
            disable_serde_phases: cfg.disable_serde_phases,
            enable_nuanced_types: cfg.enable_nuanced_types,
        };

        Exporter::from(self)
            .framework_prelude(FRAMEWORK_HEADER)
            .framework_runtime(move |exporter| {
                runtime(
                    exporter,
                    &cfg,
                    true,
                    if cfg.typed_error_impl.is_empty() {
                        TYPED_ERROR_IMPL_JS
                    } else {
                        &cfg.typed_error_impl
                    },
                    "",
                    MAKE_EVENT_IMPL_JS,
                )
            })
            .export_to(path, &types, format)
    }
}

fn rewrite_bigints_in_datatype(
    dt: Cow<'_, DataType>,
    nuanced: bool,
    phased: bool,
) -> Cow<'_, DataType> {
    match dt.as_ref() {
        DataType::Primitive(
            Primitive::usize
            | Primitive::isize
            | Primitive::u64
            | Primitive::i64
            | Primitive::u128
            | Primitive::i128,
        ) => Cow::Owned(if !nuanced {
            DataType::Primitive(Primitive::u32)
        } else if phased {
            specta_serde::phased(define("bigint | number").into(), define("bigint").into())
        } else {
            define("bigint | number").into()
        }),
        DataType::Primitive(Primitive::f128) => Cow::Owned(DataType::Primitive(Primitive::f64)),
        _ => dt,
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
        .find(|ndt| RESERVED_NDT_NAMES.contains(&&*ndt.name))
    {
        return Err(Error::framework(
            "",
            format!(
                "User defined type '{}' defined in {} must be renamed so it doesn't conflict with Tauri Specta runtime.",
                ndt.name, ndt.location
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
            || command.result().is_some_and(|dt| is_channel_type(dt, exporter.types))
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
            let command_name_escaped =
                serde_json::to_string(&resolve_tauri_command_name(cfg.plugin_name, command.name()))
                    .expect("failed to serialize string");

            let arguments = command
                .args()
                .iter()
                .map(|(name, dt)| {
                    Ok((
                        name.to_lower_camel_case(),
                        render_reference_dt_for_phase(dt, Phase::Deserialize, &exporter)?,
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
                                extract_std_result(dt, exporter.types)
                                    .map(|(ok, _)| ok)
                                    .unwrap_or(dt),
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
            field.deprecated = command.deprecated().cloned();
            field.docs = {
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
            };
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
            let event_name_escaped =
                serde_json::to_string(&resolve_tauri_event_name(cfg.plugin_name, name))
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
            if jsdoc {
                field.docs = format!(
                    "@type {{ReturnType<typeof makeEvent<{}>>}}",
                    exporter.reference(r)?
                )
                .into();
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
    let types = filter_unused_std_result_exports(exporter.render_types()?, cfg, exporter.types);
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
            // We check against `cfg` not `typed_error_assertion` as we only include the assertion if the user-provides an impl.
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

fn extract_std_result<'a>(
    dt: &'a DataType,
    types: &'a Types,
) -> Option<(&'a DataType, &'a DataType)> {
    if let DataType::Reference(Reference::Named(r)) = dt
        && let Some(ndt) = types.get(r)
        && ndt.name == "Result"
        && is_std_result_type(&ndt.module_path)
        && let NamedReferenceType::Reference { generics, .. } = &r.inner
        && let [(_, ok), (_, err), ..] = generics.as_slice()
    {
        return Some((ok, err));
    }

    None
}

fn filter_unused_std_result_exports(
    types: Cow<'static, str>,
    cfg: &BuilderConfiguration,
    collected_types: &Types,
) -> Cow<'static, str> {
    let mut has_std_result = false;
    for ndt in collected_types.into_unsorted_iter() {
        if ndt.name == "Result" && ndt.ty.is_some() {
            if is_std_result_type(&ndt.module_path) {
                has_std_result = true;
            }
        }
    }

    if !has_std_result {
        return types;
    }

    if is_std_result_used_after_command_result_flattening(cfg, collected_types) {
        return types;
    }

    let mut skipping_result = false;
    let mut removed_result = false;
    let filtered = types
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("export type Result<") || trimmed.starts_with("type Result<") {
                removed_result = true;
                skipping_result = !trimmed.ends_with(';');
                return false;
            }

            if skipping_result {
                skipping_result = !trimmed.ends_with(';');
                return false;
            }

            true
        })
        .collect::<Vec<_>>()
        .join("\n");

    if removed_result {
        Cow::Owned(filtered)
    } else {
        types
    }
}

fn hide_unused_std_result_type(cfg: &BuilderConfiguration, mut types: Types) -> Types {
    if is_std_result_used_after_command_result_flattening(cfg, &types) {
        return types;
    }

    types.iter_mut(|ndt| {
        if ndt.name == "Result" && is_std_result_type(&ndt.module_path) {
            ndt.ty = None;
        }
    });

    types
}

fn is_std_result_type(module_path: &str) -> bool {
    module_path == "std::result" || module_path == "core::result"
}

fn is_std_result_used_after_command_result_flattening(
    cfg: &BuilderConfiguration,
    types: &Types,
) -> bool {
    cfg.commands.iter().any(|command| {
        command
            .args()
            .iter()
            .any(|(_, dt)| datatype_contains_std_result(dt, types))
            || command.result().is_some_and(|dt| {
                if let Some((ok, err)) = extract_std_result(dt, types) {
                    datatype_contains_std_result(ok, types)
                        || datatype_contains_std_result(err, types)
                } else {
                    datatype_contains_std_result(dt, types)
                }
            })
    }) || cfg
        .events
        .values()
        .any(|(_, r)| datatype_contains_std_result(&DataType::Reference(r.clone()), types))
        || types.into_unsorted_iter().any(|ndt| {
            if ndt.name == "Result" && is_std_result_type(&ndt.module_path) {
                false
            } else {
                ndt.ty
                    .as_ref()
                    .is_some_and(|dt| datatype_contains_std_result(dt, types))
            }
        })
}

fn datatype_contains_std_result(dt: &DataType, types: &Types) -> bool {
    match dt {
        DataType::Primitive(_) | DataType::Generic(_) => false,
        DataType::List(list) => datatype_contains_std_result(&list.ty, types),
        DataType::Map(map) => {
            datatype_contains_std_result(map.key_ty(), types)
                || datatype_contains_std_result(map.value_ty(), types)
        }
        DataType::Struct(s) => fields_contain_std_result(&s.fields, types),
        DataType::Enum(e) => e
            .variants
            .iter()
            .any(|(_, variant)| fields_contain_std_result(&variant.fields, types)),
        DataType::Tuple(tuple) => tuple
            .elements
            .iter()
            .any(|dt| datatype_contains_std_result(dt, types)),
        DataType::Nullable(dt) => datatype_contains_std_result(dt, types),
        DataType::Intersection(dts) => dts.iter().any(|dt| datatype_contains_std_result(dt, types)),
        DataType::Reference(Reference::Named(r)) => {
            let generic_contains_result = match &r.inner {
                NamedReferenceType::Reference { generics, .. } => generics
                    .iter()
                    .any(|(_, dt)| datatype_contains_std_result(dt, types)),
                NamedReferenceType::Inline { dt, .. } => datatype_contains_std_result(dt, types),
                NamedReferenceType::Recursive => false,
            };

            generic_contains_result
                || types
                    .get(r)
                    .is_some_and(|ndt| ndt.name == "Result" && is_std_result_type(&ndt.module_path))
        }
        DataType::Reference(Reference::Opaque(_)) => false,
    }
}

fn fields_contain_std_result(fields: &Fields, types: &Types) -> bool {
    match fields {
        Fields::Unit => false,
        Fields::Unnamed(fields) => fields
            .fields
            .iter()
            .filter_map(|field| field.ty.as_ref())
            .any(|dt| datatype_contains_std_result(dt, types)),
        Fields::Named(fields) => fields
            .fields
            .iter()
            .filter_map(|(_, field)| field.ty.as_ref())
            .any(|dt| datatype_contains_std_result(dt, types)),
    }
}

fn is_channel_type(dt: &DataType, types: &Types) -> bool {
    match dt {
        DataType::Reference(Reference::Named(r)) => types
            .get(r)
            .map(|ndt| ndt.name == "TAURI_CHANNEL" && ndt.module_path.starts_with("tauri::"))
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
) -> Result<String, Error> {
    let dt = specta_serde::select_phase_datatype(dt, exporter.types, phase);
    render_reference_dt(&dt, exporter)
}

// Render a `DataType` as a reference (or fallback to inline).
// Also handles Tauri channel references.
fn render_reference_dt(dt: &DataType, exporter: &FrameworkExporter) -> Result<String, Error> {
    if let DataType::Reference(Reference::Named(r)) = dt
        && let Some(ndt) = exporter.types.get(r)
        && ndt.name == "TAURI_CHANNEL"
        && ndt.module_path.starts_with("tauri::")
    {
        let generics = match &r.inner {
            NamedReferenceType::Reference { generics, .. } => generics.as_slice(),
            NamedReferenceType::Inline { .. } | NamedReferenceType::Recursive => &[],
        };
        let generic = if let Some((_, dt)) = generics.first() {
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

const MAKE_EVENT_IMPL_TS: &str = r#"type EventEmit<T> = [T] extends [null] ? () => Promise<void> : (payload: T) => Promise<void>;

function makeEvent<T>(name: string) {
    const base = {
        listen: (cb: __TAURI_EVENT.EventCallback<T>) => __TAURI_EVENT.listen(name, cb),
        once: (cb: __TAURI_EVENT.EventCallback<T>) => __TAURI_EVENT.once(name, cb),
        emit: ((payload: T) => __TAURI_EVENT.emit(name, payload) as unknown) as EventEmit<T>
    };

    const fn = (target: import("@tauri-apps/api/webview").Webview | import("@tauri-apps/api/window").Window) => ({
        listen: (cb: __TAURI_EVENT.EventCallback<T>) => target.listen(name, cb),
        once: (cb: __TAURI_EVENT.EventCallback<T>) => target.once(name, cb),
        emit: ((payload: T) => target.emit(name, payload) as unknown) as EventEmit<T>
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
