use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use heck::ToLowerCamelCase;
use specta::{
    Format, Type, Types,
    datatype::{
        DataType, Field, Fields, NamedDataType, NamedReferenceType, Primitive, Reference, Struct,
    },
};
use specta_serde::Phase;
use specta_typescript::{Error, Exporter, FrameworkExporter, Layout, define, semantic};
use specta_util::Remapper;

use crate::name::{resolve_tauri_command_name, resolve_tauri_event_name};
use crate::{BuilderConfiguration, ErrorHandlingMode, LanguageExt};

impl LanguageExt for specta_typescript::Typescript {
    type Error = Error;

    fn export(self, cfg: &BuilderConfiguration, path: &Path) -> Result<(), Self::Error> {
        let cfg = cfg.clone();
        let types = hide_ipc_standins(hide_unused_std_result_type(&cfg, cfg.types.clone()));
        let format = SpectaFormat::new(&cfg);

        Exporter::from(self)
            .framework_prelude(FRAMEWORK_HEADER)
            .framework_runtime(move |exporter| {
                runtime(
                    exporter,
                    &cfg,
                    false,
                    if cfg.typed_error_impl.is_empty() {
                        match cfg.error_handling {
                            ErrorHandlingMode::DataError => DATA_ERROR_IMPL_TS,
                            ErrorHandlingMode::ResultUnknown => TYPED_ERROR_UNKNOWN_IMPL_TS,
                            ErrorHandlingMode::Throw | ErrorHandlingMode::Result => {
                                TYPED_ERROR_IMPL_TS
                            }
                        }
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
        let types = hide_ipc_standins(hide_unused_std_result_type(&cfg, cfg.types.clone()));
        let format = SpectaFormat::new(&cfg);

        Exporter::from(self)
            .framework_prelude(FRAMEWORK_HEADER)
            .framework_runtime(move |exporter| {
                runtime(
                    exporter,
                    &cfg,
                    true,
                    if cfg.typed_error_impl.is_empty() {
                        match cfg.error_handling {
                            ErrorHandlingMode::DataError => DATA_ERROR_IMPL_JS,
                            ErrorHandlingMode::ResultUnknown => TYPED_ERROR_UNKNOWN_IMPL_JS,
                            ErrorHandlingMode::Throw | ErrorHandlingMode::Result => {
                                TYPED_ERROR_IMPL_JS
                            }
                        }
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

fn runtime(
    mut exporter: FrameworkExporter,
    cfg: &BuilderConfiguration,
    jsdoc: bool,
    typed_error_impl: &str,
    typed_error_assertion: &str,
    make_event_impl: &str,
) -> Result<Cow<'static, str>, Error> {
    let enabled_commands = !cfg.commands.is_empty();
    let has_user_events = !cfg.events.is_empty();
    let is_raw_request_used = cfg.commands.iter().any(|command| {
        command
            .args
            .iter()
            .any(|arg| is_ipc_type(&arg.ty, exporter.types, "TAURI_REQUEST"))
    });
    // `mockCommands` is not only a command stub: the `mockIPC` it installs is also what
    // enables Tauri's event mock, so it has something to do even when there are no
    // commands and no user events. The accessors for Tauri's own events are always
    // emitted and need that mock under test.
    let enabled_mocks = cfg.mocks;
    let semantic_types_runtime_types = semantic_types_runtime_types(cfg)?;
    let semantic_types_runtime_types = semantic_types_runtime_types
        .as_ref()
        .unwrap_or(exporter.types);

    let mut out = String::new();
    let mut needs_channel_mapping = false;

    if let Some((ndt, name)) = exporter
        .types
        .into_unsorted_iter()
        .filter(|ndt| ndt.ty.is_some())
        .find_map(|ndt| {
            runtime_scope_name(jsdoc, exporter.layout, ndt)
                .filter(|name| {
                    is_reserved_ndt_name(name, jsdoc, enabled_mocks)
                        || (is_raw_request_used && name == "InvokeArgs")
                        || (enabled_commands && name == "InvokeOptions")
                })
                .map(|name| (ndt, name))
        })
    {
        return Err(Error::framework(
            format!(
                "User defined type '{}' defined in {} must be renamed so it doesn't conflict with Tauri Specta runtime.",
                name, ndt.location
            ),
            "",
        ));
    }

    {
        // Two differently named commands can still produce one accessor (e.g. `my_cmd` and
        // `MyCmd`), which silently keeps only the last. This has to run at export because
        // the accessor depends on the final `function_casing`, which can be set after the
        // commands were registered. Literal duplicates are rejected earlier, when they are
        // registered (see `Builder::commands`).
        let mut seen: BTreeMap<String, &str> = BTreeMap::new();
        for command in cfg.commands.iter() {
            let accessor = cfg
                .function_casing
                .apply(cfg.command_accessor_name(command))
                .into_owned();
            if let Some(first) = seen.get(&accessor) {
                return Err(Error::framework(
                    format!(
                        "Commands '{first}' and '{}' would both export as '{accessor}'. Rename one of them so the generated bindings don't conflict.",
                        command.name.as_ref()
                    ),
                    "",
                ));
            }
            seen.insert(accessor, command.name.as_ref());
        }
    }

    // A `Channel` is only usable as a whole command argument or result. Anywhere else it
    // renders as a bare reference name with no definition, so fail the export rather than
    // emitting bindings that don't type-check. This runs for both the TypeScript and JSDoc
    // exporters because it is part of the shared runtime path.
    validate_channel_placement(cfg, exporter.types)?;

    let is_channel_used = cfg.commands.iter().any(|command| {
        // Check if any argument is a Channel
        command
            .args
            .iter()
            .any(|arg| is_channel_type(&arg.ty, exporter.types))
            // Check if result contains a Channel
            || command.result.as_ref().is_some_and(|dt| is_channel_type(dt, exporter.types))
    });
    let has_typed_error = enabled_commands
        && cfg.error_handling != ErrorHandlingMode::Throw
        && cfg.commands.iter().any(|command| {
            command
                .result
                .as_ref()
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
    if enabled_commands {
        if jsdoc {
            if is_raw_request_used {
                out.push_str(
                    "/** @typedef {import(\"@tauri-apps/api/core\").InvokeArgs} InvokeArgs */\n",
                );
            }
            out.push_str(
                "/** @typedef {import(\"@tauri-apps/api/core\").InvokeOptions} InvokeOptions */\n",
            );
        } else {
            if is_raw_request_used {
                out.push_str(
                    "import type { InvokeArgs, InvokeOptions } from \"@tauri-apps/api/core\";\n",
                );
            } else {
                out.push_str("import type { InvokeOptions } from \"@tauri-apps/api/core\";\n");
            }
        }
    }

    // `tauriEvents` is always emitted, so the event namespace is always imported.
    out.push_str("import * as __TAURI_EVENT from \"@tauri-apps/api/event\";\n");
    if enabled_mocks {
        out.push_str("import { mockIPC as __TAURI_MOCK_IPC } from \"@tauri-apps/api/mocks\";\n");
    }

    // Commands
    if enabled_commands {
        let mut s = Struct::named();
        for command in &cfg.commands {
            let named_args = command
                .args
                .iter()
                .filter(|arg| !is_ipc_type(&arg.ty, exporter.types, "TAURI_REQUEST"))
                .collect::<Vec<_>>();
            let raw_request = named_args.len() != command.args.len();
            // A payload object cannot carry two independent values under the same key.
            // Check Tauri's normalized keys, not the generated parameter names.
            let mut seen_keys = BTreeSet::new();
            for arg in &named_args {
                if !seen_keys.insert(arg.name.as_ref()) {
                    return Err(Error::framework(
                        format!(
                            "Command '{}' uses the IPC payload key '{}' more than once. Rename one of the parameters or change `rename_all` so their keys are distinct.",
                            command.name, arg.name
                        ),
                        "",
                    ));
                }
            }
            if raw_request && cfg.mocks {
                return Err(Error::framework(
                    command.name.as_ref().to_string(),
                    "raw IPC request commands are not supported by generated mocks",
                ));
            }
            let command_name_escaped = serde_json::to_string(&resolve_tauri_command_name(
                cfg.plugin_name,
                command.name.as_ref(),
            ))
            .map_err(|err| {
                Error::framework("failed to serialize a command name for bindings", err)
            })?;

            let result = command
                .result
                .as_ref()
                .and_then(|dt| extract_std_result(dt, exporter.types));
            let typed_result = result.filter(|_| cfg.error_handling != ErrorHandlingMode::Throw);
            let mut used_names = BTreeSet::from(["__TAURI_INVOKE".to_string()]);
            if typed_result.is_some() {
                used_names.insert("typedError".into());
            }
            if cfg.semantic_types.is_some()
                && command
                    .args
                    .iter()
                    .any(|arg| is_channel_type(&arg.ty, exporter.types))
            {
                used_names.insert("mapChannel".into());
            }
            let mut arguments = command_arguments(
                command,
                &exporter,
                cfg,
                semantic_types_runtime_types,
                &mut used_names,
            )?;
            let mixed_request = raw_request && !named_args.is_empty();
            if mixed_request {
                let fields = arguments
                    .iter()
                    .zip(&command.args)
                    .filter(|(_, arg)| !is_ipc_type(&arg.ty, exporter.types, "TAURI_REQUEST"))
                    // `value` is the IPC payload object, so its keys are Tauri's keys.
                    .map(|((_, ty), arg)| Ok(format!("{}: {ty}", js_object_key(&arg.name)?)))
                    .collect::<Result<Vec<_>, Error>>()?
                    .join("; ");
                arguments = vec![("value".into(), format!("{{ {fields} }}"))];
            } else if raw_request {
                arguments.truncate(1);
            }
            let mut options_name = "options".to_string();
            while arguments.iter().any(|(name, _)| name == &options_name) {
                options_name = format!("_{options_name}");
            }
            arguments.push((format!("{options_name}?"), "InvokeOptions".into()));

            let fn_arguments = arguments
                .iter()
                .map(|(name, dt)| {
                    let mut arg = if jsdoc {
                        name.trim_end_matches('?').to_string()
                    } else {
                        name.to_string()
                    };
                    if !jsdoc {
                        arg.push_str(": ");
                        arg.push_str(dt);
                    }
                    arg
                })
                .collect::<Vec<_>>()
                .join(", ");

            let arguments_invoke_obj = if mixed_request {
                let mut fields = Vec::new();
                for arg in named_args {
                    // Read the original IPC key, not a generated parameter name.
                    let input = js_member_access("value", &arg.name)?;
                    if let Some(transform) = invoke_argument(
                        &input,
                        &arg.ty,
                        &exporter,
                        cfg,
                        semantic_types_runtime_types,
                        jsdoc,
                    ) {
                        needs_channel_mapping |= is_channel_type(&arg.ty, exporter.types);
                        fields.push(format!("{}: {transform}", js_object_key(&arg.name)?));
                    }
                }
                let value = if fields.is_empty() {
                    "value".to_string()
                } else {
                    format!("{{ ...value, {} }}", fields.join(", "))
                };
                format!(", {value}, {options_name}")
            } else if raw_request {
                let name = &arguments[0].0;
                format!(", Array.isArray({name}) ? new Uint8Array({name}) : {name}, {options_name}")
            } else if command.args.is_empty() {
                format!(", undefined, {options_name}")
            } else {
                format!(
                    ", {{ {} }}, {options_name}",
                    command
                        .args
                        .iter()
                        .zip(&arguments)
                        .map(|(arg, (param, _))| {
                            let key = arg.name.as_ref();
                            let value = invoke_argument(
                                param,
                                &arg.ty,
                                &exporter,
                                cfg,
                                semantic_types_runtime_types,
                                jsdoc,
                            );
                            needs_channel_mapping |=
                                value.is_some() && is_channel_type(&arg.ty, exporter.types);

                            Ok(match value {
                                None if key == param => param.clone(),
                                value => format!(
                                    "{}: {}",
                                    js_object_key(key)?,
                                    value.as_deref().unwrap_or(param)
                                ),
                            })
                        })
                        .collect::<Result<Vec<_>, Error>>()?
                        .join(", ")
                )
            };

            let invoke_args = format!("({command_name_escaped}{arguments_invoke_obj})",);

            let body = if let Some((dt_ok, dt_err)) = typed_result {
                let semantic_err_type = apply_semantic_type_for_phase(
                    dt_err,
                    Phase::Deserialize,
                    "v.error",
                    semantic_types_runtime_types,
                    cfg,
                )
                .and_then(|(dt, _)| dt);
                if cfg.error_handling == ErrorHandlingMode::DataError
                    && (datatype_can_be_null(dt_err, exporter.types)
                        || semantic_err_type.as_ref().is_some_and(|dt| {
                            datatype_can_be_null(dt, semantic_types_runtime_types)
                        }))
                {
                    return Err(Error::framework(
                        command.name.as_ref().to_string(),
                        "DataError mode requires a non-nullable command error type because null marks a successful result",
                    ));
                }

                let ok_semantic_type = has_semantic_type_for_phase(
                    dt_ok,
                    Phase::Deserialize,
                    "v.data",
                    &exporter,
                    cfg,
                    semantic_types_runtime_types,
                );
                let err_semantic_type = has_semantic_type_for_phase(
                    dt_err,
                    Phase::Deserialize,
                    "v.error",
                    &exporter,
                    cfg,
                    semantic_types_runtime_types,
                );
                let ok_transform = render_result_transform_for_phase(
                    dt_ok,
                    Phase::Deserialize,
                    "v.data",
                    &exporter,
                    cfg,
                    semantic_types_runtime_types,
                )
                .map(|transform| jsdoc_transform(transform, "v.data", jsdoc));
                // An invoke rejection can also be a framework error. In ResultUnknown
                // pass it through as-is rather than assuming the command's error type.
                let err_transform = match cfg.error_handling {
                    ErrorHandlingMode::ResultUnknown => None,
                    _ => render_result_transform_for_phase(
                        dt_err,
                        Phase::Deserialize,
                        "v.error",
                        &exporter,
                        cfg,
                        semantic_types_runtime_types,
                    )
                    .map(|transform| jsdoc_transform(transform, "v.error", jsdoc)),
                };

                let mut invoke_ts = "typedError".to_string();
                if !jsdoc {
                    invoke_ts.push('<');
                    invoke_ts.push_str(&render_reference_dt_for_phase(
                        dt_ok,
                        if ok_semantic_type {
                            Phase::Deserialize
                        } else {
                            Phase::Serialize
                        },
                        Phase::Deserialize,
                        &exporter,
                        cfg,
                        semantic_types_runtime_types,
                    )?);
                    invoke_ts.push_str(", ");
                    invoke_ts.push_str(&render_reference_dt_for_phase(
                        dt_err,
                        if err_semantic_type {
                            Phase::Deserialize
                        } else {
                            Phase::Serialize
                        },
                        Phase::Deserialize,
                        &exporter,
                        cfg,
                        semantic_types_runtime_types,
                    )?);
                    invoke_ts.push('>');
                }
                invoke_ts.push_str("(__TAURI_INVOKE");
                invoke_ts.push_str(&invoke_args);
                invoke_ts.push(')');

                if ok_transform.is_none() && err_transform.is_none() {
                    invoke_ts
                } else {
                    let mapper = result_mapper(cfg.error_handling, ok_transform, err_transform);

                    if jsdoc {
                        format!("{invoke_ts}.then((v) => {mapper})")
                    } else {
                        format!("{invoke_ts}.then((v) => ({mapper} as typeof v))")
                    }
                }
            } else {
                let mut invoke_ts = "__TAURI_INVOKE".to_string();
                let output_dt = result.map(|(ok, _)| ok).or(command.result.as_ref());

                if !jsdoc {
                    let output_semantic_type = output_dt.is_some_and(|dt| {
                        has_semantic_type_for_phase(
                            dt,
                            Phase::Deserialize,
                            "v",
                            &exporter,
                            cfg,
                            semantic_types_runtime_types,
                        )
                    });
                    invoke_ts.push('<');
                    invoke_ts.push_str(&match output_dt {
                        Some(dt) => Cow::Owned(render_reference_dt_for_phase(
                            dt,
                            if output_semantic_type {
                                Phase::Deserialize
                            } else {
                                Phase::Serialize
                            },
                            Phase::Deserialize,
                            &exporter,
                            cfg,
                            semantic_types_runtime_types,
                        )?),
                        None => Cow::Borrowed("void"),
                    });
                    invoke_ts.push('>');
                }
                invoke_ts.push_str(&invoke_args);

                if let Some(dt) = output_dt
                    && let Some(mapped) = render_result_transform_for_phase(
                        dt,
                        Phase::Deserialize,
                        "v",
                        &exporter,
                        cfg,
                        semantic_types_runtime_types,
                    )
                    .map(|transform| jsdoc_transform(transform, "v", jsdoc))
                {
                    if jsdoc {
                        format!("{invoke_ts}.then((v) => {mapped})")
                    } else {
                        format!("{invoke_ts}.then((v) => ({mapped} as typeof v))")
                    }
                } else {
                    invoke_ts
                }
            };

            let mut field = Field::new(define(format!("({fn_arguments}) => {body}")).into());
            field.deprecated = command.deprecated.clone();
            field.docs = {
                let mut docs = command.docs.to_string();

                if jsdoc {
                    if !docs.is_empty() {
                        docs.push('\n');
                    }

                    docs.push_str(
                        &arguments
                            .iter()
                            .map(|(name, dt)| {
                                if let Some(name) = name.strip_suffix('?') {
                                    format!("@param {{{dt}}} [{name}]")
                                } else {
                                    format!("@param {{{dt}}} {name}")
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n"),
                    );

                    if !arguments.is_empty() {
                        docs.push('\n');
                    }

                    let returns = if let Some((dt_ok, dt_err)) = typed_result {
                        let ok_semantic_type = has_semantic_type_for_phase(
                            dt_ok,
                            Phase::Deserialize,
                            "v.data",
                            &exporter,
                            cfg,
                            semantic_types_runtime_types,
                        );
                        let err_semantic_type = has_semantic_type_for_phase(
                            dt_err,
                            Phase::Deserialize,
                            "v.error",
                            &exporter,
                            cfg,
                            semantic_types_runtime_types,
                        );
                        let ok = render_reference_dt_for_phase(
                            dt_ok,
                            if ok_semantic_type {
                                Phase::Deserialize
                            } else {
                                Phase::Serialize
                            },
                            Phase::Deserialize,
                            &exporter,
                            cfg,
                            semantic_types_runtime_types,
                        )?;
                        let err = render_reference_dt_for_phase(
                            dt_err,
                            if err_semantic_type {
                                Phase::Deserialize
                            } else {
                                Phase::Serialize
                            },
                            Phase::Deserialize,
                            &exporter,
                            cfg,
                            semantic_types_runtime_types,
                        )?;

                        result_type(cfg.error_handling, &ok, &err)
                    } else {
                        let output_dt = result.map(|(ok, _)| ok).or(command.result.as_ref());
                        let output_semantic_type = output_dt.is_some_and(|dt| {
                            has_semantic_type_for_phase(
                                dt,
                                Phase::Deserialize,
                                "v",
                                &exporter,
                                cfg,
                                semantic_types_runtime_types,
                            )
                        });

                        match output_dt {
                            Some(dt) => render_reference_dt_for_phase(
                                dt,
                                if output_semantic_type {
                                    Phase::Deserialize
                                } else {
                                    Phase::Serialize
                                },
                                Phase::Deserialize,
                                &exporter,
                                cfg,
                                semantic_types_runtime_types,
                            )?,
                            None => "void".to_string(),
                        }
                    };

                    docs.push_str(&format!("@returns {{Promise<{returns}>}}"));
                }

                docs.into()
            };
            s = s.field(
                cfg.function_casing
                    .apply(cfg.command_accessor_name(command))
                    .into_owned(),
                field,
            );
        }

        out.push_str("\n/** Commands */");
        out.push_str("\nexport const commands = ");
        out.push_str(&exporter.reference(&s.build())?);
        out.push_str(";\n");
    }

    // Events
    if has_user_events {
        let mut s = Struct::named();
        let mut accessors = BTreeMap::new();
        for (name, (_, r)) in &cfg.events {
            let accessor = cfg.function_casing.apply(name);
            if let Some(first) = accessors.insert(accessor.clone(), name) {
                return Err(Error::framework(
                    format!(
                        "Events '{first}' and '{name}' would both export as '{accessor}'. Rename one of them so the generated bindings don't conflict."
                    ),
                    "",
                ));
            }
            let event_name_escaped =
                serde_json::to_string(&resolve_tauri_event_name(cfg.plugin_name, name)).map_err(
                    |err| Error::framework("failed to serialize an event name for bindings", err),
                )?;

            let event_dt = DataType::Reference(r.clone());
            let listen_ty = render_reference_dt_for_phase(
                &event_dt,
                Phase::Serialize,
                Phase::Deserialize,
                &exporter,
                cfg,
                semantic_types_runtime_types,
            )?;
            let emit_ty = render_reference_dt_for_phase(
                &event_dt,
                Phase::Deserialize,
                Phase::Serialize,
                &exporter,
                cfg,
                semantic_types_runtime_types,
            )?;
            let mut field_ts = "makeEvent".to_string();
            if !jsdoc {
                field_ts.push('<');
                field_ts.push_str(&listen_ty);
                field_ts.push_str(", ");
                field_ts.push_str(&emit_ty);
                field_ts.push('>');
            }
            field_ts.push('(');
            field_ts.push_str(&event_name_escaped);
            let serialize_transform = render_result_transform_for_phase(
                &event_dt,
                Phase::Serialize,
                "v",
                &exporter,
                cfg,
                semantic_types_runtime_types,
            )
            .map(|transform| jsdoc_transform(transform, "v", jsdoc));
            let deserialize_transform = render_result_transform_for_phase(
                &event_dt,
                Phase::Deserialize,
                "v",
                &exporter,
                cfg,
                semantic_types_runtime_types,
            )
            .map(|transform| jsdoc_transform(transform, "v", jsdoc));

            if serialize_transform.is_some() || deserialize_transform.is_some() {
                field_ts.push_str(", ");
                field_ts.push_str(
                    &serialize_transform
                        .map(|transform| format!("(v) => {transform}"))
                        .unwrap_or_else(|| "undefined".to_string()),
                );
                field_ts.push_str(", ");
                field_ts.push_str(
                    &deserialize_transform
                        .map(|transform| format!("(v) => {transform}"))
                        .unwrap_or_else(|| "undefined".to_string()),
                );
            }
            field_ts.push(')');

            let mut field = Field::new(define(field_ts).into());
            if jsdoc {
                let type_args = if listen_ty == emit_ty {
                    listen_ty
                } else {
                    format!("{listen_ty}, {emit_ty}")
                };
                field.docs = format!("@type {{ReturnType<typeof makeEvent<{type_args}>>}}").into();
            }
            s = s.field(accessor.into_owned(), field);
        }

        out.push_str("\n/** Events */");
        out.push_str("\nexport const events = ");
        out.push_str(&exporter.reference(&s.build())?);
        out.push_str(";\n");
    }

    // Built-in events. These are always emitted: Tauri sends them whether or not
    // the user registered any of their own.
    let mut s = Struct::named();
    for event in BUILTIN_EVENTS {
        // A unit payload is `null` in both emitters, which is the same shape a user's
        // `null` event payload gets. In TypeScript, `EventEmit` then types `emit` as
        // taking no argument.
        let payload = event.payload.unwrap_or("null");
        let accessor = if jsdoc {
            format!("makeEvent(\"{}\")", event.name)
        } else {
            format!("makeEvent<{payload}>(\"{}\")", event.name)
        };

        let mut field = Field::new(define(accessor).into());
        field.docs = if jsdoc {
            format!(
                "{}\n@type {{ReturnType<typeof makeEvent<{payload}>>}}",
                event.docs
            )
            .into()
        } else {
            event.docs.into()
        };
        s = s.field(event.accessor, field);
    }

    out.push_str("\n/** Built-in Tauri events */");
    out.push_str("\nexport const tauriEvents = ");
    out.push_str(&exporter.reference(&s.build())?);
    out.push_str(";\n");

    // Constants
    if !cfg.constants.is_empty() {
        out.push_str("\n/* Constants */");

        let mut constants = cfg.constants.iter().collect::<Vec<_>>();
        constants.sort_by_key(|(a, _)| *a);
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
                    .map_err(|err| Error::framework("failed to serialize a constant", err))?,
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
    // `tauriEvents` always needs `makeEvent`, so this block is always emitted.
    out.push_str("\n/* Tauri Specta runtime */\n");

    if needs_channel_mapping {
        out.push_str(if jsdoc {
            MAP_CHANNEL_IMPL_JS
        } else {
            MAP_CHANNEL_IMPL_TS
        });
        out.push('\n');
    }

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
        out.push('\n');
    }
    out.push_str(make_event_impl);
    out.push('\n');

    // Mocks
    if enabled_mocks {
        out.push_str("\n/* Tauri Specta mocks */\n\n");
        out.push_str(&mock_handlers_declaration(
            cfg,
            &exporter,
            semantic_types_runtime_types,
            jsdoc,
        )?);
        out.push('\n');
        out.push_str(&mock_commands_impl(cfg, jsdoc)?);
        if enabled_commands {
            out.push('\n');
            out.push_str(if jsdoc {
                MOCK_COMMAND_HANDLER_IMPL_JS
            } else {
                MOCK_COMMAND_HANDLER_IMPL_TS
            });
            out.push('\n');
        }
    }

    // Single-file layouts get one newline appended after the framework runtime by the
    // exporter, so ending our body with a newline too would leave a blank line at the end
    // of the file. The `Files` layout writes this runtime into its own file without that
    // appended newline, so it keeps ours.
    if exporter.layout != Layout::Files {
        while out.ends_with('\n') {
            out.pop();
        }
    }

    Ok(Cow::Owned(out))
}

/// One of Tauri's own events, as it is serialized by the Tauri runtime.
struct BuiltinEvent {
    /// The accessor name under `tauriEvents`, matching the event's `TauriEvent` variant.
    accessor: &'static str,
    /// The Tauri event name.
    name: &'static str,
    /// The TypeScript payload type. `None` for a unit payload, which Tauri sends as `null`.
    payload: Option<&'static str>,
    /// One-line description of the event for the generated docs.
    docs: &'static str,
}

/// Tauri's built-in events, with the payload each emitter serializes.
///
/// Verified against tauri 2.12.5: window and drag-and-drop events in
/// `src/manager/window.rs` (`on_window_event`), `tauri://window-created` in
/// `src/window/mod.rs` (`WindowBuilder::build_internal`), `tauri://webview-created` in
/// `src/manager/webview.rs` (`prepare_webview`), and `Theme`'s bare-string
/// serialization in `tauri-utils`' `src/lib.rs`.
///
/// Keep this in step with `TauriEvent` in `@tauri-apps/api/event`; the accessor names here
/// are that enum's variants in camel case.
const BUILTIN_EVENTS: &[BuiltinEvent] = &[
    BuiltinEvent {
        accessor: "windowResized",
        name: "tauri://resize",
        payload: Some("{ width: number; height: number }"),
        docs: "A window was resized. Payload: the new inner size, in physical pixels.",
    },
    BuiltinEvent {
        accessor: "windowMoved",
        name: "tauri://move",
        payload: Some("{ x: number; y: number }"),
        docs: "A window moved. Payload: the new outer position, in physical pixels.",
    },
    BuiltinEvent {
        accessor: "windowCloseRequested",
        name: "tauri://close-requested",
        payload: None,
        docs: "The user requested that a window close.",
    },
    BuiltinEvent {
        accessor: "windowDestroyed",
        name: "tauri://destroyed",
        payload: None,
        docs: "A window was destroyed.",
    },
    BuiltinEvent {
        accessor: "windowFocus",
        name: "tauri://focus",
        payload: None,
        docs: "A window gained focus.",
    },
    BuiltinEvent {
        accessor: "windowBlur",
        name: "tauri://blur",
        payload: None,
        docs: "A window lost focus.",
    },
    BuiltinEvent {
        accessor: "windowScaleFactorChanged",
        name: "tauri://scale-change",
        payload: Some("{ scaleFactor: number; size: { width: number; height: number } }"),
        docs: "The scale factor of the monitor a window is on changed. Payload: the new scale factor and inner size, in physical pixels.",
    },
    BuiltinEvent {
        accessor: "windowThemeChanged",
        name: "tauri://theme-changed",
        // `Theme` serializes through `Display` to a bare string, not an object.
        payload: Some("\"light\" | \"dark\""),
        docs: "The system or window theme changed.",
    },
    BuiltinEvent {
        accessor: "windowCreated",
        name: "tauri://window-created",
        payload: Some("{ label: string }"),
        docs: "A window was created.",
    },
    BuiltinEvent {
        accessor: "windowSuspended",
        name: "tauri://suspended",
        payload: None,
        docs: "A window's event loop was suspended. Mobile only.",
    },
    BuiltinEvent {
        accessor: "windowResumed",
        name: "tauri://resumed",
        payload: None,
        docs: "A window's event loop was resumed. Mobile only.",
    },
    BuiltinEvent {
        accessor: "webviewCreated",
        name: "tauri://webview-created",
        payload: Some("{ label: string }"),
        docs: "A webview was created.",
    },
    BuiltinEvent {
        accessor: "dragEnter",
        name: "tauri://drag-enter",
        payload: Some("{ paths: string[]; position: { x: number; y: number } }"),
        docs: "The user dragged files onto a window. Payload: the dragged paths and the pointer position.",
    },
    BuiltinEvent {
        accessor: "dragOver",
        name: "tauri://drag-over",
        // Tauri skips serializing `paths` when the payload comes from an `Over` event.
        payload: Some("{ position: { x: number; y: number } }"),
        docs: "The user is moving dragged files over a window. Payload: the pointer position.",
    },
    BuiltinEvent {
        accessor: "dragDrop",
        name: "tauri://drag-drop",
        payload: Some("{ paths: string[]; position: { x: number; y: number } }"),
        docs: "The user dropped files onto a window. Payload: the dropped paths and the pointer position.",
    },
    BuiltinEvent {
        accessor: "dragLeave",
        name: "tauri://drag-leave",
        payload: None,
        docs: "The drag operation left a window or was cancelled.",
    },
];

/// An accessor or IPC payload key as it should appear in an object literal or type.
///
/// Accessors are usually plain identifiers, but [`Casing::KebabCase`](crate::Casing)
/// produces keys like `hello-world`, which have to be quoted. This is the same rule
/// `specta_typescript` applies to struct field names.
fn js_object_key(accessor: &str) -> Result<String, Error> {
    if is_js_identifier(accessor) {
        Ok(accessor.to_string())
    } else {
        serde_json::to_string(accessor)
            .map_err(|err| Error::framework("failed to serialize a JavaScript property key", err))
    }
}

/// A read of `object`'s [`js_object_key`] property named `accessor`.
fn js_member_access(object: &str, accessor: &str) -> Result<String, Error> {
    if is_js_identifier(accessor) {
        Ok(format!("{object}.{accessor}"))
    } else {
        Ok(format!("{object}[{}]", js_object_key(accessor)?))
    }
}

fn is_js_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    (first.is_ascii_alphabetic() || first == '_' || first == '$')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

/// The `MockCommandHandlers` type, listing one handler per generated command.
///
/// Handlers are keyed by command accessor (the name under `commands.*`) rather than the
/// Tauri command name, so `#[tauri::command(rename = "...")]` doesn't leak into the
/// frontend-facing API here any more than it does for `commands` itself.
fn mock_handlers_declaration(
    cfg: &BuilderConfiguration,
    exporter: &FrameworkExporter,
    semantic_types_runtime_types: &Types,
    jsdoc: bool,
) -> Result<String, Error> {
    let mut handlers = Vec::new();

    for command in cfg.commands.iter() {
        let key = js_object_key(
            &cfg.function_casing
                .apply(cfg.command_accessor_name(command)),
        )?;
        let (arguments, result) =
            mock_handler_signature(command, exporter, cfg, semantic_types_runtime_types)?;
        handlers.push(format!(
            "{key}: {arguments} => {result} | Promise<{result}>,"
        ));
    }

    let mut out = String::new();

    // JSDoc gets one object literal rather than an `@property` per command: an
    // `@property` name has to be an identifier, which `Casing::KebabCase` accessors
    // aren't.
    if jsdoc {
        out.push_str("/**\n * @typedef {{\n");
        for handler in &handlers {
            out.push_str(&format!(" *     {handler}\n"));
        }
        out.push_str(" * }} MockCommandHandlers\n */\n");
    } else {
        out.push_str(
            "/** A handler for each generated command, keyed by its `commands` accessor. */\n",
        );
        out.push_str("export type MockCommandHandlers = {\n");
        for handler in &handlers {
            out.push_str(&format!("    {handler}\n"));
        }
        out.push_str("};\n");
    }

    Ok(out)
}

/// The generated parameter name and type for each argument, in declaration order.
/// Payload keys are kept separately in `CommandArg::name`.
fn command_arguments(
    command: &crate::Command,
    exporter: &FrameworkExporter,
    cfg: &BuilderConfiguration,
    semantic_types_runtime_types: &Types,
    used_names: &mut BTreeSet<String>,
) -> Result<Vec<(String, String)>, Error> {
    command
        .args
        .iter()
        .map(|arg| {
            Ok((
                command_parameter_name(&arg.name, used_names),
                render_reference_dt_for_phase(
                    &arg.ty,
                    Phase::Deserialize,
                    Phase::Serialize,
                    exporter,
                    cfg,
                    semantic_types_runtime_types,
                )?,
            ))
        })
        .collect()
}

fn command_parameter_name(key: &str, used_names: &mut BTreeSet<String>) -> String {
    let mut name = key.to_lower_camel_case();
    if !is_js_identifier(&name) {
        name.insert(0, '_');
        if !is_js_identifier(&name) {
            // Be conservative for non-ASCII names; the original key remains in the payload.
            name = "_arg".into();
        }
    }
    while is_reserved_parameter_name(&name) || !used_names.insert(name.clone()) {
        name.insert(0, '_');
    }
    name
}

fn is_reserved_parameter_name(name: &str) -> bool {
    // The generated bindings are modules: strict-mode restrictions apply to parameters.
    matches!(
        name,
        "arguments"
            | "await"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "eval"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "implements"
            | "import"
            | "in"
            | "instanceof"
            | "interface"
            | "let"
            | "new"
            | "null"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "return"
            | "static"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "yield"
    )
}

/// The `(arguments, result)` signature of one command's mock handler.
///
/// This mirrors how `commands` types the same command, so a handler is written in the
/// same terms as the call it stands in for, rather than in terms of the IPC payload.
fn mock_handler_signature(
    command: &crate::Command,
    exporter: &FrameworkExporter,
    cfg: &BuilderConfiguration,
    semantic_types_runtime_types: &Types,
) -> Result<(String, String), Error> {
    validate_mock_transforms(command, exporter, cfg, semantic_types_runtime_types)?;

    // A handler is called with the invoke payload, so its keys are the payload keys
    // rather than the camelCased names the generated accessor uses.
    let arguments = command
        .args
        .iter()
        .map(|arg| {
            Ok((
                arg.name.to_string(),
                render_reference_dt_for_phase(
                    &arg.ty,
                    Phase::Deserialize,
                    Phase::Serialize,
                    exporter,
                    cfg,
                    semantic_types_runtime_types,
                )?,
            ))
        })
        .collect::<Result<Vec<_>, Error>>()?;

    let arguments = if arguments.is_empty() {
        "()".to_string()
    } else {
        format!(
            "(args: {{ {} }})",
            arguments
                .iter()
                .map(|(name, dt)| Ok(format!("{}: {dt}", js_object_key(name)?)))
                .collect::<Result<Vec<_>, Error>>()?
                .join("; ")
        )
    };

    // A handler returns the command's success value: for a `Result`, that is the `Ok`
    // side, since a mock fails a command by throwing rather than by returning `Err`.
    let result = match command
        .result
        .as_ref()
        .and_then(|dt| extract_std_result(dt, exporter.types).map(|(ok, _)| ok))
        .or(command.result.as_ref())
    {
        Some(dt) => {
            let result_semantic_type = has_semantic_type_for_phase(
                dt,
                Phase::Deserialize,
                "v",
                exporter,
                cfg,
                semantic_types_runtime_types,
            );
            render_reference_dt_for_phase(
                dt,
                if result_semantic_type {
                    Phase::Deserialize
                } else {
                    Phase::Serialize
                },
                Phase::Deserialize,
                exporter,
                cfg,
                semantic_types_runtime_types,
            )?
        }
        None => "void".to_string(),
    };

    Ok((arguments, result))
}

/// Rejects a command whose mock handler could not be typed honestly.
///
/// A [`semantic`](specta_typescript::semantic) rule can give a value one type in the
/// generated bindings and another on the wire, applying a transform in between (a struct
/// field of a rule-backed type is enough — a bare argument does not get one). A mock
/// handler sits on the wire side of that transform, so the type it declares would be a
/// lie, and the transform's output type can't be recovered from the rule. Fail the export
/// rather than emit a binding that lies, like the other export-time validations do.
fn validate_mock_transforms(
    command: &crate::Command,
    exporter: &FrameworkExporter,
    cfg: &BuilderConfiguration,
    semantic_types_runtime_types: &Types,
) -> Result<(), Error> {
    let reject = |dt: &DataType, phase: Phase, subject: &str| {
        if render_result_transform_for_phase(
            dt,
            phase,
            "v",
            exporter,
            cfg,
            semantic_types_runtime_types,
        )
        .is_some()
        {
            return Err(Error::framework(
                format!(
                    "`Builder::mocks` can't type a mock for command '{}': {subject} is reshaped by a semantic type transform, so the handler wouldn't receive or return the type it declares. Export the mocks without `Builder::semantic_types`, or drop `Builder::mocks`.",
                    command.name.as_ref()
                ),
                "",
            ));
        }

        Ok(())
    };

    for arg in &command.args {
        // A channel's payload is transformed as messages arrive, so either way the handler
        // sees the `Channel<T>` it declares.
        if channel_generic_type(&arg.ty, exporter.types).is_some() {
            continue;
        }

        reject(
            &arg.ty,
            Phase::Serialize,
            &format!("argument '{}'", arg.name),
        )?;
    }

    if let Some(result) = command.result.as_ref() {
        let (ok, err) = extract_std_result(result, exporter.types)
            .map_or((result, None), |(ok, err)| (ok, Some(err)));

        reject(ok, Phase::Deserialize, "the result")?;

        // Only modes that assume the rejection has the declared error type transform it.
        if let Some(err) = err
            && matches!(
                cfg.error_handling,
                ErrorHandlingMode::Result | ErrorHandlingMode::DataError
            )
        {
            reject(err, Phase::Deserialize, "the error")?;
        }
    }

    Ok(())
}

/// The `mockCommands` installer, which routes each registered Tauri command name to its
/// handler.
fn mock_commands_impl(cfg: &BuilderConfiguration, jsdoc: bool) -> Result<String, Error> {
    let mut out = String::new();
    out.push_str(if jsdoc {
        MOCK_CALL_TYPES_JS
    } else {
        MOCK_CALL_TYPES_TS
    });
    out.push('\n');
    out.push_str(MOCK_COMMANDS_DOC);
    if jsdoc {
        out.push_str("\n *\n * @param {Partial<MockCommandHandlers>} [handlers]");
        out.push_str("\n * @returns {MockCommandsHandle}");
    }
    out.push_str("\n */\n");

    if jsdoc {
        out.push_str("export function mockCommands(handlers = {}) {\n");
    } else {
        out.push_str(
            "export function mockCommands(handlers: Partial<MockCommandHandlers> = {}): MockCommandsHandle {\n",
        );
    }
    out.push_str(if jsdoc {
        MOCK_HANDLER_REGISTRY_IMPL_JS
    } else {
        MOCK_HANDLER_REGISTRY_IMPL_TS
    });
    out.push_str(if jsdoc {
        "    /** @type {MockCall[]} */\n    const calls = [];\n"
    } else {
        "    const calls: MockCall[] = [];\n"
    });
    out.push_str("    __TAURI_MOCK_IPC((cmd, args) => {\n");
    out.push_str("        switch (cmd) {\n");

    for command in cfg.commands.iter() {
        let accessor = cfg
            .function_casing
            .apply(cfg.command_accessor_name(command))
            .into_owned();
        let access = js_member_access("mockHandlers", &accessor)?;
        // The recorded name is the accessor, not the Tauri command name, so a test can
        // assert against the same name it uses for `commands`.
        let accessor_literal = serde_json::to_string(&accessor)
            .map_err(|err| Error::framework("failed to serialize a command accessor", err))?;
        let command_name = serde_json::to_string(&resolve_tauri_command_name(
            cfg.plugin_name,
            command.name.as_ref(),
        ))
        .map_err(|err| Error::framework("failed to serialize a command name for bindings", err))?;
        out.push_str(&format!(
            "            case {command_name}: calls.push({{ command: {accessor_literal}, args }}); return mockCommandHandler({access}, {command_name}, args);\n"
        ));
    }

    // Record before dispatching, so a call that fails is still part of the history. A
    // command the bindings don't generate has no accessor to record, so it keeps the raw
    // Tauri command name the frontend passed to `invoke`.
    out.push_str("            default:\n");
    out.push_str("                calls.push({ command: cmd, args });\n");
    out.push_str(
        "                throw new Error(`No mock handler registered for command '${cmd}'`);\n",
    );
    out.push_str("        }\n");
    out.push_str("    }, { shouldMockEvents: true });\n");
    // Frozen so `handle.calls = []` can't silently orphan the array the installed
    // callback keeps appending to. The array itself stays mutable: it is the history.
    out.push_str(
        "    return Object.freeze({\n        calls,\n        clear: () => {\n            calls.length = 0;\n        },\n    });\n",
    );
    out.push_str("}\n");

    Ok(out)
}

fn runtime_scope_name(
    jsdoc: bool,
    layout: Layout,
    ndt: &NamedDataType,
) -> Option<Cow<'static, str>> {
    match (jsdoc, layout) {
        (_, Layout::FlatFile) | (true, Layout::ModulePrefixedName) => Some(ndt.name.clone()),
        (false, Layout::ModulePrefixedName) => Some(Cow::Owned(format!(
            "{}_{}",
            ndt.module_path.replace("::", "_"),
            ndt.name
        ))),
        (_, Layout::Namespaces) => Some(
            ndt.module_path
                .split("::")
                .next()
                .filter(|module| !module.is_empty())
                .map_or_else(|| ndt.name.clone(), |module| Cow::Owned(module.to_owned())),
        ),
        (_, Layout::Files) => ndt.module_path.is_empty().then(|| ndt.name.clone()),
    }
}

/// Applies `specta_serde` format + also remaps `DataType`'s and does other transformations!
#[derive(Debug, Clone)]
struct SpectaFormat {
    disable_serde_phases: bool,
    semantic_types: Option<semantic::Configuration>,
    remapper: Remapper,
}

impl SpectaFormat {
    fn new(cfg: &BuilderConfiguration) -> Self {
        let mut remapper = Remapper::new();

        if cfg.dangerously_cast_bigints_to_number {
            // Creating a virtual `Types` here is a bad pattern but given we know Specta doesn't use it, it's safe.
            let number = <specta_typescript::Number as Type>::definition(&mut Types::default());
            remapper = remapper
                .rule(DataType::Primitive(Primitive::usize), number.clone())
                .rule(DataType::Primitive(Primitive::isize), number.clone())
                .rule(DataType::Primitive(Primitive::u64), number.clone())
                .rule(DataType::Primitive(Primitive::i64), number.clone())
                .rule(DataType::Primitive(Primitive::u128), number.clone())
                .rule(DataType::Primitive(Primitive::i128), number.clone())
                .rule(
                    <specta_typescript::BigInt as Type>::definition(&mut Types::default()),
                    number,
                );
        }

        Self {
            disable_serde_phases: cfg.disable_serde_phases,
            semantic_types: cfg.semantic_types.clone(),
            remapper,
        }
    }
}

impl Format for SpectaFormat {
    fn map_types(&'_ self, types: &Types) -> Result<Cow<'_, Types>, specta::FormatError> {
        let types = if self.disable_serde_phases {
            specta_serde::Format.map_types(types)
        } else {
            specta_serde::PhasesFormat.map_types(types)
        }?;

        Ok(match &self.semantic_types {
            Some(semantic_types) => Cow::Owned(
                self.remapper
                    .remap_types(semantic_types.apply_types(&types).into_owned()),
            ),
            None => Cow::Owned(self.remapper.remap_types(types.into_owned())),
        })
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

        Ok(Cow::Owned(self.remapper.remap_dt(dt.into_owned())))
    }
}

fn extract_std_result<'a>(
    dt: &'a DataType,
    types: &'a Types,
) -> Option<(&'a DataType, &'a DataType)> {
    if let DataType::Reference(Reference::Named(r)) = dt
        && let Some(ndt) = types.get(r)
        && is_result_ndt(ndt)
        && let NamedReferenceType::Reference { generics, .. } = &r.inner
        && let [(_, ok), (_, err), ..] = generics.as_slice()
    {
        return Some((ok, err));
    }

    None
}

fn hide_unused_std_result_type(cfg: &BuilderConfiguration, mut types: Types) -> Types {
    let is_std_result_used_after_command_result_flattening = cfg.commands.iter().any(|command| {
        command
            .args
            .iter()
            .any(|arg| datatype_contains_std_result(&arg.ty, &types))
            || command.result.as_ref().is_some_and(|dt| {
                if let Some((ok, err)) = extract_std_result(dt, &types) {
                    datatype_contains_std_result(ok, &types)
                        || datatype_contains_std_result(err, &types)
                } else {
                    datatype_contains_std_result(dt, &types)
                }
            })
    }) || cfg
        .events
        .values()
        .any(|(_, r)| datatype_contains_std_result(&DataType::Reference(r.clone()), &types))
        || types.into_unsorted_iter().any(|ndt| {
            if is_result_ndt(ndt) {
                false
            } else {
                ndt.ty
                    .as_ref()
                    .is_some_and(|dt| datatype_contains_std_result(dt, &types))
            }
        });

    if is_std_result_used_after_command_result_flattening {
        return types;
    }

    types.iter_mut(|ndt| {
        if is_result_ndt(ndt) {
            ndt.ty = None;
        }
    });

    types
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
                NamedReferenceType::Recursive(_) => false,
            };

            generic_contains_result || types.get(r).is_some_and(is_result_ndt)
        }
        DataType::Reference(Reference::Opaque(_)) => false,
    }
}

fn datatype_can_be_null(dt: &DataType, types: &Types) -> bool {
    datatype_can_be_null_with_generics(dt, types, &[])
}

fn datatype_can_be_null_with_generics(
    dt: &DataType,
    types: &Types,
    generic_scopes: &[&[(specta::datatype::Generic, DataType)]],
) -> bool {
    match dt {
        DataType::Nullable(_) => true,
        DataType::Tuple(tuple) => tuple.elements.is_empty(),
        DataType::Struct(s) => fields_can_be_null(&s.fields, types, generic_scopes),
        DataType::Enum(e) => {
            let all_variants_untagged = e.attributes.contains_key("serde:container:untagged");
            e.variants.iter().any(|(_, variant)| {
                !variant.skip
                    && (all_variants_untagged
                        || variant.attributes.contains_key("serde:variant:untagged"))
                    && fields_can_be_null(&variant.fields, types, generic_scopes)
            })
        }
        DataType::Generic(generic) => generic_scopes
            .iter()
            .enumerate()
            .rev()
            .find_map(|(scope_index, scope)| {
                scope
                    .iter()
                    .find(|(candidate, _)| candidate == generic)
                    .map(|(_, dt)| (scope_index, dt))
            })
            .is_none_or(|(scope_index, dt)| {
                datatype_can_be_null_with_generics(dt, types, &generic_scopes[..scope_index])
            }),
        DataType::Reference(Reference::Named(r)) => match &r.inner {
            NamedReferenceType::Inline { dt, .. } => {
                datatype_can_be_null_with_generics(dt, types, generic_scopes)
            }
            NamedReferenceType::Reference { generics, .. } => types
                .get(r)
                .and_then(|ndt| ndt.ty.as_ref())
                .is_some_and(|dt| {
                    let mut scopes = generic_scopes.to_vec();
                    scopes.push(generics);
                    datatype_can_be_null_with_generics(dt, types, &scopes)
                }),
            NamedReferenceType::Recursive(_) => false,
        },
        DataType::Primitive(Primitive::f16 | Primitive::f32 | Primitive::f64 | Primitive::f128) => {
            true
        }
        DataType::Intersection(dts) => {
            dts.is_empty()
                || dts
                    .iter()
                    .all(|dt| datatype_can_be_null_with_generics(dt, types, generic_scopes))
        }
        DataType::Primitive(_) | DataType::List(_) | DataType::Map(_) => false,
        DataType::Reference(Reference::Opaque(_)) => true,
    }
}

fn fields_can_be_null(
    fields: &Fields,
    types: &Types,
    generic_scopes: &[&[(specta::datatype::Generic, DataType)]],
) -> bool {
    match fields {
        Fields::Unit => true,
        Fields::Unnamed(fields) if fields.fields.len() == 1 => fields.fields[0]
            .ty
            .as_ref()
            .is_none_or(|dt| datatype_can_be_null_with_generics(dt, types, generic_scopes)),
        Fields::Unnamed(_) | Fields::Named(_) => false,
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

fn is_result_ndt(ndt: &NamedDataType) -> bool {
    ndt.name == "Result" && matches!(&*ndt.module_path, "std::result" | "core::result")
}

/// Whether `ndt` is the stand-in Tauri derives for `tauri::ipc::Channel`.
fn is_channel_ndt(ndt: &NamedDataType) -> bool {
    ndt.name == "TAURI_CHANNEL" && ndt.module_path.starts_with("tauri::")
}

fn is_ipc_type(dt: &DataType, types: &Types, name: &str) -> bool {
    let DataType::Reference(Reference::Named(reference)) = dt else {
        return false;
    };
    types
        .get(reference)
        .is_some_and(|ndt| ndt.name == name && ndt.module_path.starts_with("tauri::"))
}

fn hide_ipc_standins(mut types: Types) -> Types {
    types.iter_mut(|ndt| {
        if matches!(ndt.name.as_ref(), "TAURI_REQUEST" | "TAURI_RESPONSE")
            && ndt.module_path.starts_with("tauri::")
        {
            ndt.ty = None;
        }
    });
    types
}

fn is_channel_type(dt: &DataType, types: &Types) -> bool {
    channel_generic_type(dt, types).is_some()
}

fn channel_generic_type<'a>(dt: &'a DataType, types: &Types) -> Option<&'a DataType> {
    let DataType::Reference(Reference::Named(r)) = dt else {
        return None;
    };
    let ndt = types.get(r)?;
    if !is_channel_ndt(ndt) {
        return None;
    }
    match &r.inner {
        NamedReferenceType::Reference { generics, .. } => generics.first().map(|(_, dt)| dt),
        NamedReferenceType::Inline { .. } | NamedReferenceType::Recursive(_) => None,
    }
}

/// Where a `Channel` was found in a position Tauri can't use it.
enum ChannelSite<'a> {
    /// The channel is nested in this named type's definition.
    NamedType(&'a NamedDataType),
    /// The channel is nested in a command or event signature without an intervening
    /// named type.
    Signature,
}

/// Walk `dt` looking for a `Channel` reference that is not the root datatype.
///
/// Named references are resolved through `types` so a channel reached through another
/// struct or alias is caught. `owner` is the named type whose body is being walked, so the
/// error can point at it. `visited` holds the addresses of named type bodies that were
/// already walked, which keeps recursive type definitions finite.
fn find_nested_channel<'a>(
    dt: &DataType,
    types: &'a Types,
    owner: Option<&'a NamedDataType>,
    visited: &mut BTreeSet<usize>,
) -> Option<ChannelSite<'a>> {
    match dt {
        DataType::Primitive(_) | DataType::Generic(_) => None,
        DataType::List(list) => find_nested_channel(&list.ty, types, owner, visited),
        DataType::Map(map) => find_nested_channel(map.key_ty(), types, owner, visited)
            .or_else(|| find_nested_channel(map.value_ty(), types, owner, visited)),
        DataType::Struct(s) => fields_nested_channel(&s.fields, types, owner, visited),
        DataType::Enum(e) => e
            .variants
            .iter()
            .find_map(|(_, variant)| fields_nested_channel(&variant.fields, types, owner, visited)),
        DataType::Tuple(tuple) => tuple
            .elements
            .iter()
            .find_map(|dt| find_nested_channel(dt, types, owner, visited)),
        DataType::Nullable(dt) => find_nested_channel(dt, types, owner, visited),
        DataType::Intersection(parts) => parts
            .iter()
            .find_map(|dt| find_nested_channel(dt, types, owner, visited)),
        DataType::Reference(Reference::Opaque(_)) => None,
        DataType::Reference(Reference::Named(r)) => {
            let ndt = types.get(r)?;
            if is_channel_ndt(ndt) {
                return Some(owner.map_or(ChannelSite::Signature, ChannelSite::NamedType));
            }

            match &r.inner {
                NamedReferenceType::Recursive(_) => None,
                NamedReferenceType::Inline { dt, .. } => {
                    find_nested_channel(dt, types, owner, visited)
                }
                NamedReferenceType::Reference { generics, .. } => generics
                    .iter()
                    .find_map(|(_, dt)| find_nested_channel(dt, types, owner, visited))
                    .or_else(|| nested_channel_in_named_type(ndt, types, visited)),
            }
        }
    }
}

/// Walk a named type's body for a nested channel, skipping bodies that were already seen.
fn nested_channel_in_named_type<'a>(
    ndt: &'a NamedDataType,
    types: &'a Types,
    visited: &mut BTreeSet<usize>,
) -> Option<ChannelSite<'a>> {
    let body = ndt.ty.as_ref()?;
    if !visited.insert(std::ptr::from_ref(ndt).addr()) {
        return None;
    }
    find_nested_channel(body, types, Some(ndt), visited)
}

fn fields_nested_channel<'a>(
    fields: &Fields,
    types: &'a Types,
    owner: Option<&'a NamedDataType>,
    visited: &mut BTreeSet<usize>,
) -> Option<ChannelSite<'a>> {
    match fields {
        Fields::Unit => None,
        Fields::Unnamed(fields) => fields
            .fields
            .iter()
            .filter_map(|field| field.ty.as_ref())
            .find_map(|dt| find_nested_channel(dt, types, owner, visited)),
        Fields::Named(fields) => fields
            .fields
            .iter()
            .filter_map(|(_, field)| field.ty.as_ref())
            .find_map(|dt| find_nested_channel(dt, types, owner, visited)),
    }
}

/// Reject `Channel`s that aren't the whole command argument or result.
fn validate_channel_placement(cfg: &BuilderConfiguration, types: &Types) -> Result<(), Error> {
    // Covers events, `Builder::typ`, and command results that are named types: a channel
    // anywhere in a definition renders as an undefined reference name.
    for ndt in types.into_unsorted_iter() {
        let mut visited = BTreeSet::new();
        if let Some(site) = nested_channel_in_named_type(ndt, types, &mut visited) {
            return Err(channel_nested_error(site, ""));
        }
    }

    for command in cfg.commands.iter() {
        for arg in &command.args {
            validate_channel_signature_datatype(
                &arg.ty,
                types,
                &format!(
                    "argument '{}' of command '{}'",
                    arg.name,
                    command.name.as_ref()
                ),
            )?;
        }
        if let Some(dt) = command.result.as_ref() {
            validate_channel_signature_datatype(
                dt,
                types,
                &format!("the result of command '{}'", command.name.as_ref()),
            )?;
        }
    }

    // Event payloads are exported as plain types, so a channel at their root is just as
    // invalid as a channel nested in one.
    for (name, (_, reference)) in cfg.events.iter() {
        validate_channel_signature_datatype(
            &DataType::Reference(reference.clone()),
            types,
            &format!("event '{name}'"),
        )?;
    }

    Ok(())
}

fn validate_channel_signature_datatype(
    dt: &DataType,
    types: &Types,
    subject: &str,
) -> Result<(), Error> {
    // A channel is allowed as the whole argument or result, but its payload is plain
    // serialized data so a channel can't hide in there either.
    if let Some(payload) = channel_generic_type(dt, types) {
        let mut visited = BTreeSet::new();
        if let Some(site) = find_nested_channel(payload, types, None, &mut visited) {
            return Err(channel_nested_error(site, subject));
        }
        return Ok(());
    }

    let mut visited = BTreeSet::new();
    match find_nested_channel(dt, types, None, &mut visited) {
        Some(site) => Err(channel_nested_error(site, subject)),
        None => Ok(()),
    }
}

fn channel_nested_error(site: ChannelSite<'_>, signature_subject: &str) -> Error {
    let subject = match site {
        ChannelSite::NamedType(ndt) => {
            format!("type '{}' defined in {}", ndt.name, ndt.location)
        }
        ChannelSite::Signature => signature_subject.to_string(),
    };

    Error::framework(
        format!(
            "`tauri::ipc::Channel` is nested inside {subject}. A channel must be a direct command argument (or the whole command result), so move it to the top level. To send a channel id inside a payload, use `tauri::ipc::JavaScriptChannelId` and resolve it with `channel_on` instead."
        ),
        "",
    )
}

/// The JavaScript expression that adapts one command argument for the IPC.
///
/// A `Channel` payload is decoded as messages arrive, so the transform is
/// registered on the channel itself: `mapChannel` keeps the channel the caller
/// passed (and the callback id Rust addresses) intact on released APIs too. Every other
/// argument is transformed before it is sent.
fn invoke_argument(
    input: &str,
    dt: &DataType,
    exporter: &FrameworkExporter,
    cfg: &BuilderConfiguration,
    semantic_types_runtime_types: &Types,
    jsdoc: bool,
) -> Option<String> {
    if let Some(payload) = channel_generic_type(dt, exporter.types) {
        render_result_transform_for_phase(
            payload,
            Phase::Deserialize,
            "v",
            exporter,
            cfg,
            semantic_types_runtime_types,
        )
        .map(|transform| jsdoc_transform(transform, "v", jsdoc))
        .map(|transform| format!("mapChannel({input}, (v) => {transform})"))
    } else {
        render_result_transform_for_phase(
            dt,
            Phase::Serialize,
            input,
            exporter,
            cfg,
            semantic_types_runtime_types,
        )
        .map(|transform| jsdoc_transform(transform, input, jsdoc))
    }
}

fn render_result_transform_for_phase(
    dt: &DataType,
    phase: Phase,
    input: &str,
    _exporter: &FrameworkExporter,
    cfg: &BuilderConfiguration,
    semantic_types_runtime_types: &Types,
) -> Option<String> {
    // Keep named references so Specta can match registered semantic rules before
    // descending into the type's fields.
    let dt = specta_serde::select_phase_datatype(dt, semantic_types_runtime_types, phase);
    apply_semantic_type_for_phase(&dt, phase, input, semantic_types_runtime_types, cfg)
        .map(|(_, mapped)| mapped)
        .filter(|mapped| mapped != input)
}

fn jsdoc_transform(transform: String, input: &str, jsdoc: bool) -> String {
    if jsdoc {
        transform.replace(&format!(" as typeof {input}"), "")
    } else {
        transform
    }
}

fn result_mapper(
    mode: ErrorHandlingMode,
    ok_transform: Option<String>,
    err_transform: Option<String>,
) -> String {
    match (mode, ok_transform, err_transform) {
        (ErrorHandlingMode::Result | ErrorHandlingMode::ResultUnknown, Some(ok), Some(err)) => {
            format!(
                "(v.status === \"ok\" ? {{ ...v, data: {ok} }} : v.status === \"error\" ? {{ ...v, error: {err} }} : v)"
            )
        }
        (ErrorHandlingMode::Result | ErrorHandlingMode::ResultUnknown, Some(ok), None) => {
            format!("(v.status === \"ok\" ? {{ ...v, data: {ok} }} : v)")
        }
        (ErrorHandlingMode::Result | ErrorHandlingMode::ResultUnknown, None, Some(err)) => {
            format!("(v.status === \"error\" ? {{ ...v, error: {err} }} : v)")
        }
        (ErrorHandlingMode::DataError, Some(ok), Some(err)) => {
            format!("(v.error === null ? {{ ...v, data: {ok} }} : {{ ...v, error: {err} }})")
        }
        (ErrorHandlingMode::DataError, Some(ok), None) => {
            format!("(v.error === null ? {{ ...v, data: {ok} }} : v)")
        }
        (ErrorHandlingMode::DataError, None, Some(err)) => {
            format!("(v.error !== null ? {{ ...v, error: {err} }} : v)")
        }
        (ErrorHandlingMode::Throw, _, _) | (_, None, None) => "v".to_string(),
    }
}

fn result_type(mode: ErrorHandlingMode, ok: &str, err: &str) -> String {
    match mode {
        ErrorHandlingMode::Throw => ok.to_string(),
        ErrorHandlingMode::Result => {
            format!("{{ status: \"ok\"; data: {ok} }} | {{ status: \"error\"; error: {err} }}")
        }
        ErrorHandlingMode::ResultUnknown => {
            format!("{{ status: \"ok\"; data: {ok} }} | {{ status: \"error\"; error: unknown }}")
        }
        ErrorHandlingMode::DataError => {
            format!("{{ data: {ok}; error: null }} | {{ data: null; error: {err} }}")
        }
    }
}

fn has_semantic_type_for_phase(
    dt: &DataType,
    phase: Phase,
    input: &str,
    _exporter: &FrameworkExporter,
    cfg: &BuilderConfiguration,
    semantic_types_runtime_types: &Types,
) -> bool {
    apply_semantic_type_for_phase(dt, phase, input, semantic_types_runtime_types, cfg).is_some()
}

fn render_reference_dt_for_phase(
    dt: &DataType,
    serde_phase: Phase,
    semantic_type_phase: Phase,
    exporter: &FrameworkExporter,
    cfg: &BuilderConfiguration,
    semantic_types_runtime_types: &Types,
) -> Result<String, Error> {
    let dt = specta_serde::select_phase_datatype(dt, exporter.types, serde_phase);
    let dt = apply_semantic_type_for_phase(
        &dt,
        semantic_type_phase,
        "v",
        semantic_types_runtime_types,
        cfg,
    )
    .and_then(|(dt, _)| dt)
    .unwrap_or(dt);

    render_reference_dt(&dt, exporter)
}

fn apply_semantic_type_for_phase(
    dt: &DataType,
    phase: Phase,
    input: &str,
    types: &Types,
    cfg: &BuilderConfiguration,
) -> Option<(Option<DataType>, String)> {
    let semantic_types = cfg.semantic_types.as_ref()?;
    match phase {
        Phase::Serialize => semantic_types.apply_serialize(types, dt, input),
        Phase::Deserialize => semantic_types.apply_deserialize(types, dt, input),
    }
}

fn semantic_types_runtime_types(cfg: &BuilderConfiguration) -> Result<Option<Types>, Error> {
    if cfg.semantic_types.is_none() {
        return Ok(None);
    }

    let types = hide_unused_std_result_type(cfg, cfg.types.clone());
    let types = if cfg.disable_serde_phases {
        specta_serde::Format.map_types(&types)
    } else {
        specta_serde::PhasesFormat.map_types(&types)
    }
    .map_err(|err| Error::framework("failed to format semantic types runtime types", err))?;

    Ok(Some(types.into_owned()))
}

// Render a `DataType` as a reference (or fallback to inline).
// Also handles Tauri channel references.
fn render_reference_dt(dt: &DataType, exporter: &FrameworkExporter) -> Result<String, Error> {
    if is_ipc_type(dt, exporter.types, "TAURI_REQUEST") {
        return Ok("InvokeArgs".into());
    }
    if is_ipc_type(dt, exporter.types, "TAURI_RESPONSE") {
        return Ok("unknown".into());
    }
    if let DataType::Reference(Reference::Named(r)) = dt
        && let Some(ndt) = exporter.types.get(r)
        && is_channel_ndt(ndt)
    {
        let generics = match &r.inner {
            NamedReferenceType::Reference { generics, .. } => generics.as_slice(),
            NamedReferenceType::Inline { .. } | NamedReferenceType::Recursive(_) => &[],
        };
        let generic = if let Some((_, dt)) = generics.first() {
            exporter.reference(dt)?.into()
        } else {
            Cow::Borrowed("never")
        };
        Ok(format!("Channel<{generic}>"))
    } else {
        exporter.reference(dt)
    }
}

const RESERVED_NDT_NAMES: &[&str] = &[
    "Channel",
    "__TAURI_EVENT",
    "__TAURI_INVOKE",
    "tauriEvents",
    "typedError",
    "makeEvent",
    "mapChannel",
    "channelTransform",
];

/// Names only the mocks runtime declares, and so only reserved on an export that
/// [`Builder::mocks`](crate::Builder::mocks) actually emits them for.
const RESERVED_NDT_NAMES_MOCKS_ONLY: &[&str] = &[
    "__TAURI_MOCK_IPC",
    "MockCall",
    "MockCommandHandlers",
    "MockCommandsHandle",
    "mockCommandHandler",
    "mockCommands",
];

/// Names only the TypeScript runtime declares. The JSDoc runtime has no
/// corresponding typedefs, so user types may use them there (and a JSDoc export
/// must not fail over a name it never emits).
const RESERVED_NDT_NAMES_TS_ONLY: &[&str] = &["EventEmit", "EventEmitTo"];

fn is_reserved_ndt_name(name: &str, jsdoc: bool, mocks: bool) -> bool {
    RESERVED_NDT_NAMES.contains(&name)
        || (!jsdoc && RESERVED_NDT_NAMES_TS_ONLY.contains(&name))
        || (mocks && RESERVED_NDT_NAMES_MOCKS_ONLY.contains(&name))
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

const TYPED_ERROR_UNKNOWN_IMPL_TS: &str = r#"async function typedError<T, E>(result: Promise<T>): Promise<{ status: "ok"; data: T } | { status: "error"; error: unknown }> {
    try {
        return { status: "ok", data: await result };
    } catch (e) {
        return { status: "error", error: e };
    }
}"#;

const DATA_ERROR_IMPL_TS: &str = r#"async function typedError<T, E>(result: Promise<T>): Promise<{ data: T; error: null } | { data: null; error: E }> {
    try {
        return { data: await result, error: null };
    } catch (e) {
        if (e instanceof Error) throw e;
        return { data: null, error: e as any };
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
        return { status: "error", error: /** @type {E} */ (e) };
    }
}"#;

const TYPED_ERROR_UNKNOWN_IMPL_JS: &str = r#"/**
  * @template T
  * @template E
  * @param {Promise<T>} result
  * @returns {Promise<{ status: "ok"; data: T } | { status: "error"; error: unknown }>}
  */
async function typedError(result) {
    try {
        return { status: "ok", data: await result };
    } catch (e) {
        return { status: "error", error: e };
    }
}"#;

const DATA_ERROR_IMPL_JS: &str = r#"/**
  * @template T
  * @template E
  * @param {Promise<T>} result
  * @returns {Promise<{ data: T; error: null } | { data: null; error: E }>}
  */
async function typedError(result) {
    try {
        return { data: await result, error: null };
    } catch (e) {
        if (e instanceof Error) throw e;
        return { data: null, error: /** @type {E} */ (e) };
    }
}"#;

const TYPED_ERROR_ASSERTION_TS: &str = "const _assertTypedErrorFollowsContract: <T, E>(result: Promise<T>) => Promise<any> = typedError;";

const MOCK_CALL_TYPES_TS: &str = r#"/**
 * One command invocation observed by a set of mock handlers.
 *
 * `command` is the `commands` accessor that was called, e.g. `greet`. A command the
 * bindings do not generate has no accessor to record, so it keeps the raw Tauri command
 * name the frontend passed to `invoke` — which is not cased like an accessor. That is why
 * it is `string` rather than a union of accessor names: such a union would be a lie for
 * those calls.
 */
export type MockCall = {
    /**
     * The generated command accessor, e.g. `greet`, or, for a command the bindings don't
     * generate, its raw Tauri command name.
     */
    readonly command: string;
    /**
     * The arguments `invoke` received, not a copy, so it is `unknown`: the generated
     * `commands` accessor passes `{}` for a command with no arguments, and a raw `invoke`
     * can also pass Tauri's array and buffer payload shapes.
     */
    readonly args: unknown;
};

/** The handle `mockCommands` returns for the handlers it installed. */
export type MockCommandsHandle = {
    /**
     * Every call observed since this handle's handlers were installed, oldest first. The
     * array is live, so it keeps growing for as long as those handlers stay installed.
     */
    readonly calls: readonly MockCall[];
    /** Drop the recorded calls. The handlers stay installed. */
    clear(): void;
};
"#;

const MOCK_CALL_TYPES_JS: &str = r#"/**
 * One command invocation observed by a set of mock handlers.
 *
 * `command` is the `commands` accessor that was called, e.g. `greet`. A command the
 * bindings do not generate has no accessor to record, so it keeps the raw Tauri command
 * name the frontend passed to `invoke` — which is not cased like an accessor.
 *
 * @typedef {object} MockCall
 * @property {string} command The generated command accessor, e.g. `greet`, or, for a
 * command the bindings don't generate, its raw Tauri command name.
 * @property {unknown} args The arguments `invoke` received, not a copy; `{}` when the
 * generated `commands` accessor was called without arguments.
 */

/**
 * The handle `mockCommands` returns for the handlers it installed.
 *
 * @typedef {object} MockCommandsHandle
 * @property {readonly MockCall[]} calls Every call observed since this handle's handlers
 * were installed, oldest first. The array is live, so it keeps growing for as long as
 * those handlers stay installed.
 * @property {() => void} clear Drop the recorded calls. The handlers stay installed.
 */
"#;

// Keep the original channel and its IPC callback. The symbol shares the mapping
// across generated modules without retaining channels in a global registry.
const MAP_CHANNEL_IMPL_TS: &str = r#"const channelTransform = Symbol.for("tauri-specta.channelTransform");
function mapChannel<T>(channel: Channel<T>, transform: (value: any) => T): Channel<T> {
    const mapped: Channel<T> & {
        [channelTransform]?: { handler: (value: T) => void; transform: (value: any) => T };
    } = channel;
    const existing = mapped[channelTransform];
    if (existing) {
        existing.transform = transform;
        return channel;
    }
    const state = { handler: channel.onmessage, transform };
    channel.onmessage = (value) => state.handler.call(channel, state.transform(value));
    Object.defineProperty(channel, "onmessage", {
        configurable: true,
        get: () => state.handler,
        set: (handler: (value: T) => void) => { state.handler = handler; },
    });
    Object.defineProperty(channel, channelTransform, { value: state });
    return channel;
}
"#;

const MAP_CHANNEL_IMPL_JS: &str = r#"const channelTransform = Symbol.for("tauri-specta.channelTransform");
/**
 * @template T
 * @param {Channel<T>} channel
 * @param {(value: any) => T} transform
 * @returns {Channel<T>}
 */
function mapChannel(channel, transform) {
    /** @type {Channel<T> & { [channelTransform]?: { handler: (value: T) => void; transform: (value: any) => T } }} */
    const mapped = channel;
    const existing = mapped[channelTransform];
    if (existing) {
        existing.transform = transform;
        return channel;
    }
    const state = { handler: channel.onmessage, transform };
    channel.onmessage = (value) => state.handler.call(channel, state.transform(value));
    Object.defineProperty(channel, "onmessage", {
        configurable: true,
        get: () => state.handler,
        set: (/** @type {(value: T) => void} */ handler) => { state.handler = handler; },
    });
    Object.defineProperty(channel, channelTransform, { value: state });
    return channel;
}
"#;

const MOCK_COMMANDS_DOC: &str = r#"/**
 * Install typed mock handlers for the generated commands.
 *
 * Handlers are installed through `@tauri-apps/api/mocks`'s `mockIPC`, so the application
 * under test keeps importing the real `commands` object and still runs the result transform
 * and channel mapping the bindings wrap around each call.
 *
 * A handler receives the command's arguments and returns its success value; throw (or
 * return a rejected promise) to make the command fail. A command with no handler rejects
 * with an error rather than resolving `undefined`, so a test can't pass by accident.
 *
 * The returned handle records every call these handlers observe, so a test can assert
 * what was invoked:
 *
 * ```ts
 * const calls = mockCommands({ greet: ({ myName }) => `Hello, ${myName}!` });
 * await commands.greet("Brendan");
 * expect(calls.calls).toEqual([{ command: "greet", args: { myName: "Brendan" } }]);
 * ```
 *
 * `calls` is the live history, oldest first, and grows for as long as these handlers stay
 * installed. Each entry holds the `commands` accessor that was called and the arguments
 * `invoke` received — not a copy, and not narrowed to an object, since a raw `invoke` may
 * pass Tauri's array or buffer payload shapes. `clear()` empties it without uninstalling
 * anything. Every call is recorded before it is dispatched, including one that fails
 * because it has no handler or because its handler throws. A command the bindings do not
 * generate is recorded under its raw Tauri command name, since it has no `commands`
 * accessor.
 *
 * Calling `mockCommands` again installs a fresh set of handlers and returns a fresh handle
 * with an empty history; the previous handle keeps its calls but stops growing.
 *
 * Tauri's event mocking is enabled as part of the install (`shouldMockEvents`), so
 * `events` and `tauriEvents` keep working under test without a second installer fighting
 * this one, and the event plugin's internal invokes never reach the record. Note that
 * Tauri's own mock does not implement `emitTo`, which resolves without reaching any
 * listener. Call `clearMocks` from `@tauri-apps/api/mocks` between tests; it uninstalls
 * the handlers but leaves a handle's history alone."#;

const MOCK_HANDLER_REGISTRY_IMPL_TS: &str = r#"    // A copy with no prototype, so a command named after an `Object.prototype` member
    // (`toString`, `constructor`, …) reports that it is unregistered instead of calling
    // whatever the prototype happens to provide.
    const mockHandlers: Partial<MockCommandHandlers> = Object.create(null);
    Object.assign(mockHandlers, handlers);
"#;

const MOCK_HANDLER_REGISTRY_IMPL_JS: &str = r#"    // A copy with no prototype, so a command named after an `Object.prototype` member
    // (`toString`, `constructor`, …) reports that it is unregistered instead of calling
    // whatever the prototype happens to provide.
    /** @type {Partial<MockCommandHandlers>} */
    const mockHandlers = Object.create(null);
    Object.assign(mockHandlers, handlers);
"#;

const MOCK_COMMAND_HANDLER_IMPL_TS: &str = r#"function mockCommandHandler(handler: ((args: any) => unknown) | undefined, name: string, args: unknown): unknown {
    if (handler === undefined) throw new Error(`No mock handler registered for command '${name}'`);
    return handler(args);
}"#;

const MOCK_COMMAND_HANDLER_IMPL_JS: &str = r#"/**
 * @param {((args: any) => unknown) | undefined} handler
 * @param {string} name
 * @param {unknown} args
 * @returns {unknown}
 */
function mockCommandHandler(handler, name, args) {
    if (handler === undefined) throw new Error(`No mock handler registered for command '${name}'`);
    return handler(args);
}"#;

const MAKE_EVENT_IMPL_TS: &str = r#"type EventEmit<T> = [T] extends [null] ? () => Promise<void> : (payload: T) => Promise<void>;

type EventEmitTo<T> = [T] extends [null] ? (target: __TAURI_EVENT.EventTarget | string) => Promise<void> : (target: __TAURI_EVENT.EventTarget | string, payload: T) => Promise<void>;

function makeEvent<TListen, TEmit = TListen>(name: string, serialize?: (payload: TEmit) => unknown, deserialize?: (payload: any) => TListen) {
    const mapEvent = (cb: __TAURI_EVENT.EventCallback<TListen>) => (event: __TAURI_EVENT.Event<any>) => cb({ ...event, payload: deserialize ? deserialize(event.payload) : event.payload });
    const mapPayload = (payload: TEmit) => serialize ? serialize(payload) : payload;

    const base = {
        listen: (cb: __TAURI_EVENT.EventCallback<TListen>) => __TAURI_EVENT.listen(name, mapEvent(cb)),
        once: (cb: __TAURI_EVENT.EventCallback<TListen>) => __TAURI_EVENT.once(name, mapEvent(cb)),
        emit: ((payload: TEmit) => __TAURI_EVENT.emit(name, mapPayload(payload)) as unknown) as EventEmit<TEmit>,
        emitTo: ((to: __TAURI_EVENT.EventTarget | string, payload: TEmit) => __TAURI_EVENT.emitTo(to, name, mapPayload(payload)) as unknown) as EventEmitTo<TEmit>
    };

    const fn = (target: import("@tauri-apps/api/webview").Webview | import("@tauri-apps/api/window").Window) => ({
        listen: (cb: __TAURI_EVENT.EventCallback<TListen>) => target.listen(name, mapEvent(cb)),
        once: (cb: __TAURI_EVENT.EventCallback<TListen>) => target.once(name, mapEvent(cb)),
        emit: ((payload: TEmit) => target.emit(name, mapPayload(payload)) as unknown) as EventEmit<TEmit>,
        emitTo: ((to: __TAURI_EVENT.EventTarget | string, payload: TEmit) => target.emitTo(to, name, mapPayload(payload)) as unknown) as EventEmitTo<TEmit>
    });

    return Object.assign(fn, base);
}"#;

const MAKE_EVENT_IMPL_JS: &str = r#"/**
 * @template TListen
 * @template [TEmit=TListen]
 * @param {string} name
 * @param {(payload: TEmit) => unknown} [serialize]
 * @param {(payload: any) => TListen} [deserialize]
 */
function makeEvent(name, serialize, deserialize) {
    /**
     * @param {__TAURI_EVENT.EventCallback<TListen>} cb
     * @returns {(event: __TAURI_EVENT.Event<any>) => void}
     */
    const mapEvent = (cb) => (event) => cb({ ...event, payload: deserialize ? deserialize(event.payload) : event.payload });
    /** @param {TEmit} payload */
    const mapPayload = (payload) => serialize ? serialize(payload) : payload;

    const base = {
        /** @param {__TAURI_EVENT.EventCallback<TListen>} cb */
        listen: (cb) => __TAURI_EVENT.listen(name, mapEvent(cb)),
        /** @param {__TAURI_EVENT.EventCallback<TListen>} cb */
        once: (cb) => __TAURI_EVENT.once(name, mapEvent(cb)),
        /** @param {TEmit} payload */
        emit: (payload) => __TAURI_EVENT.emit(name, mapPayload(payload)),
        /**
         * @param {__TAURI_EVENT.EventTarget | string} to
         * @param {TEmit} payload
         */
        emitTo: (to, payload) => __TAURI_EVENT.emitTo(to, name, mapPayload(payload)),
    };

    /** @param {import("@tauri-apps/api/webview").Webview | import("@tauri-apps/api/window").Window} target */
    const fn = (target) => ({
        /** @param {__TAURI_EVENT.EventCallback<TListen>} cb */
        listen: (cb) => target.listen(name, mapEvent(cb)),
        /** @param {__TAURI_EVENT.EventCallback<TListen>} cb */
        once: (cb) => target.once(name, mapEvent(cb)),
        /** @param {TEmit} payload */
        emit: (payload) => target.emit(name, mapPayload(payload)),
        /**
         * @param {__TAURI_EVENT.EventTarget | string} to
         * @param {TEmit} payload
         */
        emitTo: (to, payload) => target.emitTo(to, name, mapPayload(payload)),
    });

    return Object.assign(fn, base);
}"#;

#[cfg(test)]
mod tests {
    use std::fs;

    use serde::{Deserialize, Serialize};
    use specta::{
        Type,
        datatype::{DataType, Primitive},
        specta,
    };
    use specta_typescript::{JSDoc, Layout, Typescript, define, semantic};

    use crate::{Builder, ErrorHandlingMode, collect_commands};

    #[tauri::command]
    #[specta]
    fn nullable_result() -> Result<Option<String>, String> {
        Ok(None)
    }

    #[tauri::command]
    #[specta]
    fn nullable_error() -> Result<String, ()> {
        Ok(String::new())
    }

    #[derive(Serialize, Type)]
    struct UnitError;

    #[derive(Serialize, Type)]
    struct GenericError<T>(T);

    #[derive(Serialize, Type)]
    struct SemanticError(String);

    #[derive(Serialize, Deserialize, Type)]
    #[serde(rename = "BuildChannel")]
    #[allow(dead_code)]
    enum Channel {
        Production,
    }

    mod unrenamed {
        use super::*;

        #[derive(Serialize, Deserialize, Type)]
        #[allow(dead_code)]
        pub enum Channel {
            Production,
        }
    }

    /// A user type named after something the mocks runtime declares.
    mod mock_names {
        use super::*;

        #[derive(Serialize, Deserialize, Type)]
        #[allow(dead_code)]
        pub struct MockCommandHandlers {
            value: String,
        }

        #[derive(Serialize, Deserialize, Type)]
        #[allow(dead_code)]
        pub struct MockCall {
            value: String,
        }

        #[derive(Serialize, Deserialize, Type)]
        #[allow(dead_code)]
        pub struct MockCommandsHandle {
            value: String,
        }
    }

    /// A user type named after something the built-in events runtime declares.
    mod builtin_event_names {
        use super::*;

        #[derive(Serialize, Deserialize, Type)]
        #[serde(rename = "tauriEvents")]
        #[allow(dead_code)]
        pub struct TauriEvents {
            value: String,
        }
    }

    /// Mirrors the built-in `bytes::Bytes` semantic rule, which transforms in both
    /// directions.
    #[derive(Serialize, Deserialize, Type)]
    struct SemanticBytes(Vec<u8>);

    #[derive(Serialize, Deserialize, Type)]
    struct NestedSemanticBytes {
        value: SemanticBytes,
    }

    #[tauri::command]
    #[specta]
    fn nested_semantic_bytes(input: NestedSemanticBytes) -> NestedSemanticBytes {
        input
    }

    fn semantic_bytes() -> semantic::Configuration {
        semantic::Configuration::empty().define::<SemanticBytes>(
            |_| define("Uint8Array").into(),
            Some(semantic::Transform::new(|i| format!("[...{i}]"))),
            Some(semantic::Transform::new(|i| format!("new Uint8Array({i})"))),
        )
    }

    mod inline {
        use super::*;

        #[derive(Serialize, Deserialize, Type)]
        #[specta(inline)]
        pub struct Channel {
            value: String,
        }
    }

    #[derive(Serialize, Type)]
    #[serde(untagged)]
    #[allow(dead_code)]
    enum UntaggedError {
        Unit,
        Message(String),
    }

    #[tauri::command]
    #[specta]
    fn unit_struct_error() -> Result<String, UnitError> {
        Ok(String::new())
    }

    #[tauri::command]
    #[specta]
    fn generic_nullable_error() -> Result<String, GenericError<()>> {
        Ok(String::new())
    }

    #[tauri::command]
    #[specta]
    fn untagged_nullable_error() -> Result<String, UntaggedError> {
        Ok(String::new())
    }

    #[tauri::command]
    #[specta]
    fn floating_point_error() -> Result<String, f64> {
        Ok(String::new())
    }

    #[tauri::command]
    #[specta]
    fn semantic_nullable_error() -> Result<String, SemanticError> {
        Ok(String::new())
    }

    #[tauri::command]
    #[specta]
    fn bigint_error() -> Result<String, u64> {
        Ok(String::new())
    }

    #[test]
    fn reserved_runtime_names_use_exported_type_names() {
        let output_dir = std::path::Path::new("target/tests/reserved-runtime-names");
        let _ = fs::remove_dir_all(output_dir);

        let renamed = Builder::<tauri::Wry>::new().typ::<Channel>();
        for (name, layout) in [
            ("flat", Layout::FlatFile),
            ("namespaces", Layout::Namespaces),
            ("module-prefixed", Layout::ModulePrefixedName),
            ("files", Layout::Files),
        ] {
            renamed
                .export(Typescript::default().layout(layout), output_dir.join(name))
                .expect("serde-renamed Channel should export as TypeScript");
        }
        assert!(
            fs::read_to_string(output_dir.join("flat"))
                .expect("failed to read TypeScript bindings")
                .contains("export type BuildChannel")
        );

        for (name, layout) in [
            ("jsdoc-flat", Layout::FlatFile),
            ("jsdoc-module-prefixed", Layout::ModulePrefixedName),
            ("jsdoc-files", Layout::Files),
        ] {
            renamed
                .export(JSDoc::default().layout(layout), output_dir.join(name))
                .expect("serde-renamed Channel should export as JSDoc");
        }
        assert!(
            fs::read_to_string(output_dir.join("jsdoc-flat"))
                .expect("failed to read JSDoc bindings")
                .contains("BuildChannel")
        );

        let inline_path = output_dir.join("inline.ts");
        Builder::<tauri::Wry>::new()
            .typ::<inline::Channel>()
            .export(Typescript::default(), &inline_path)
            .expect("an inline type named Channel should not conflict with the runtime");
        assert!(
            !fs::read_to_string(inline_path)
                .expect("failed to read inline TypeScript bindings")
                .contains("export type Channel")
        );

        let err = Builder::<tauri::Wry>::new()
            .typ::<unrenamed::Channel>()
            .export(Typescript::default(), output_dir.join("unrenamed.ts"))
            .expect_err("a flat export named Channel should conflict with the runtime");
        assert!(err.to_string().contains("User defined type 'Channel'"));

        Builder::<tauri::Wry>::new()
            .typ::<unrenamed::Channel>()
            .export(JSDoc::default(), output_dir.join("unrenamed.js"))
            .expect_err("a flat JSDoc export named Channel should conflict with the runtime");

        for (name, layout) in [
            ("unrenamed-namespaces", Layout::Namespaces),
            ("unrenamed-module-prefixed", Layout::ModulePrefixedName),
            ("unrenamed-files", Layout::Files),
        ] {
            Builder::<tauri::Wry>::new()
                .typ::<unrenamed::Channel>()
                .export(Typescript::default().layout(layout), output_dir.join(name))
                .expect("a scoped Channel should not conflict with the runtime");
        }

        Builder::<tauri::Wry>::new()
            .typ::<unrenamed::Channel>()
            .export(
                JSDoc::default().layout(Layout::ModulePrefixedName),
                output_dir.join("unrenamed-jsdoc-module-prefixed"),
            )
            .expect_err("JSDoc module-prefixed typedefs should retain their bare names");

        Builder::<tauri::Wry>::new()
            .typ::<unrenamed::Channel>()
            .export(
                JSDoc::default().layout(Layout::Files),
                output_dir.join("unrenamed-jsdoc-files"),
            )
            .expect("a Channel in a separate JSDoc file should not conflict with the runtime");

        fs::remove_dir_all(output_dir).expect("failed to remove test output directory");
    }

    #[test]
    fn data_error_mode_exports_discriminated_results() {
        let output_dir = std::env::temp_dir().join(format!(
            "tauri-specta-data-error-test-{}",
            std::process::id()
        ));
        fs::create_dir_all(&output_dir).expect("failed to create test output directory");

        let builder = Builder::<tauri::Wry>::new()
            .commands(collect_commands![nullable_result])
            .error_handling(ErrorHandlingMode::DataError);

        let ts_path = output_dir.join("bindings.ts");
        builder
            .clone()
            .export(Typescript::default(), &ts_path)
            .expect("failed to export TypeScript bindings");
        let ts = fs::read_to_string(ts_path).expect("failed to read TypeScript bindings");
        assert!(ts.contains("typedError<string | null, string>"));
        assert!(ts.contains("Promise<{ data: T; error: null } | { data: null; error: E }>"));
        assert!(ts.contains("return { data: await result, error: null };"));
        assert!(ts.contains("return { data: null, error: e as any };"));

        let js_path = output_dir.join("bindings.js");
        builder
            .export(JSDoc::default(), &js_path)
            .expect("failed to export JSDoc bindings");
        let js = fs::read_to_string(js_path).expect("failed to read JSDoc bindings");
        assert!(js.contains(
            "@returns {Promise<{ data: string | null; error: null } | { data: null; error: string }>}"
        ));
        assert!(
            js.contains("@returns {Promise<{ data: T; error: null } | { data: null; error: E }>}")
        );

        fs::remove_dir_all(output_dir).expect("failed to remove test output directory");
    }

    #[test]
    fn result_unknown_mode_types_error_as_unknown_without_rethrow() {
        let output_dir = std::env::temp_dir().join(format!(
            "tauri-specta-result-unknown-test-{}",
            std::process::id()
        ));
        fs::create_dir_all(&output_dir).expect("failed to create test output directory");

        let builder = Builder::<tauri::Wry>::new()
            .commands(collect_commands![nullable_result])
            .error_handling(ErrorHandlingMode::ResultUnknown);

        let ts_path = output_dir.join("bindings.ts");
        builder
            .clone()
            .export(Typescript::default(), &ts_path)
            .expect("failed to export TypeScript bindings");
        let ts = fs::read_to_string(ts_path).expect("failed to read TypeScript bindings");
        assert!(
            ts.contains(
                "Promise<{ status: \"ok\"; data: T } | { status: \"error\"; error: unknown }>"
            ),
            "TypeScript runtime should declare the error as `unknown`:\n{ts}"
        );
        assert!(
            ts.contains("return { status: \"error\", error: e };"),
            "TypeScript runtime should return the rejection in the result object:\n{ts}"
        );
        assert!(
            !ts.contains("instanceof Error"),
            "TypeScript runtime should not rethrow:\n{ts}"
        );
        // The `Err` type argument is still passed even though it no longer types the error.
        assert!(ts.contains("typedError<string | null, string>"));

        let js_path = output_dir.join("bindings.js");
        builder
            .export(JSDoc::default(), &js_path)
            .expect("failed to export JSDoc bindings");
        let js = fs::read_to_string(js_path).expect("failed to read JSDoc bindings");
        assert!(
            js.contains(
                "@returns {Promise<{ status: \"ok\"; data: string | null } | { status: \"error\"; error: unknown }>}"
            ),
            "JSDoc command typedef should declare the error as `unknown`:\n{js}"
        );
        assert!(
            js.contains(
                "@returns {Promise<{ status: \"ok\"; data: T } | { status: \"error\"; error: unknown }>}"
            ),
            "JSDoc runtime typedef should declare the error as `unknown`:\n{js}"
        );
        assert!(
            !js.contains("instanceof Error"),
            "JSDoc runtime should not rethrow:\n{js}"
        );

        fs::remove_dir_all(output_dir).expect("failed to remove test output directory");
    }

    #[test]
    fn data_error_mode_rejects_nullable_error_types() {
        for (name, builder) in [
            (
                "unit",
                Builder::<tauri::Wry>::new().commands(collect_commands![nullable_error]),
            ),
            (
                "unit-struct",
                Builder::<tauri::Wry>::new().commands(collect_commands![unit_struct_error]),
            ),
            (
                "generic-unit",
                Builder::<tauri::Wry>::new().commands(collect_commands![generic_nullable_error]),
            ),
            (
                "untagged-unit",
                Builder::<tauri::Wry>::new().commands(collect_commands![untagged_nullable_error]),
            ),
            (
                "floating-point",
                Builder::<tauri::Wry>::new().commands(collect_commands![floating_point_error]),
            ),
        ] {
            let output_path = std::env::temp_dir().join(format!(
                "tauri-specta-nullable-error-{name}-test-{}.ts",
                std::process::id()
            ));
            let err = builder
                .error_handling(ErrorHandlingMode::DataError)
                .export(Typescript::default(), output_path)
                .expect_err("nullable error type should fail to export");

            assert!(err.to_string().contains(
                "DataError mode requires a non-nullable command error type because null marks a successful result"
            ));
        }

        let semantic_types = specta_typescript::semantic::Configuration::empty()
            .define::<SemanticError>(
                |_| {
                    DataType::Intersection(vec![DataType::Nullable(Box::new(DataType::Primitive(
                        Primitive::str,
                    )))])
                },
                None,
                Some(specta_typescript::semantic::Transform::new(|_| {
                    "null".to_string()
                })),
            );
        let output_path = std::env::temp_dir().join(format!(
            "tauri-specta-nullable-semantic-error-test-{}.ts",
            std::process::id()
        ));
        let err = Builder::<tauri::Wry>::new()
            .commands(collect_commands![semantic_nullable_error])
            .semantic_types(semantic_types)
            .error_handling(ErrorHandlingMode::DataError)
            .export(Typescript::default(), output_path)
            .expect_err("nullable semantic error type should fail to export");
        assert!(err.to_string().contains(
            "DataError mode requires a non-nullable command error type because null marks a successful result"
        ));
    }

    #[test]
    fn builtin_event_runtime_names_are_reserved() {
        let output_dir = std::path::Path::new("target/tests/builtin-events-reserved-names");
        let _ = fs::remove_dir_all(output_dir);
        fs::create_dir_all(output_dir).expect("failed to create test output directory");

        // `tauriEvents` is always emitted, so a user type with that name can never be
        // exported, by either exporter.
        let ts_err = Builder::<tauri::Wry>::new()
            .typ::<builtin_event_names::TauriEvents>()
            .export(Typescript::default(), output_dir.join("bindings.ts"))
            .expect_err("`tauriEvents` should always be reserved");
        assert!(
            ts_err
                .to_string()
                .contains("User defined type 'tauriEvents'"),
            "{ts_err}"
        );

        let jsdoc_err = Builder::<tauri::Wry>::new()
            .typ::<builtin_event_names::TauriEvents>()
            .export(JSDoc::default(), output_dir.join("bindings.js"))
            .expect_err("`tauriEvents` should always be reserved");
        assert!(
            jsdoc_err
                .to_string()
                .contains("User defined type 'tauriEvents'"),
            "{jsdoc_err}"
        );

        fs::remove_dir_all(output_dir).expect("failed to remove test output directory");
    }

    #[test]
    fn builtin_event_names_are_not_plugin_prefixed() {
        // Built-in events are emitted by Tauri itself, so they must not pick up the
        // plugin name the user's own commands and events are prefixed with.
        let output_dir = std::path::Path::new("target/tests/builtin-events-plugin-name");
        let _ = fs::remove_dir_all(output_dir);
        fs::create_dir_all(output_dir).expect("failed to create test output directory");

        Builder::<tauri::Wry>::new()
            .plugin_name("my-plugin")
            .export(Typescript::default(), output_dir.join("bindings.ts"))
            .expect("failed to export TypeScript bindings");

        let ts = fs::read_to_string(output_dir.join("bindings.ts"))
            .expect("failed to read TypeScript bindings");
        assert!(
            ts.contains(r#"makeEvent<{ label: string }>("tauri://window-created")"#),
            "built-in event names should stay unprefixed:\n{ts}"
        );
        assert!(
            !ts.contains(r#""plugin:my-plugin|tauri://"#),
            "built-in event names must not be plugin-prefixed:\n{ts}"
        );

        fs::remove_dir_all(output_dir).expect("failed to remove test output directory");
    }

    #[test]
    fn mock_runtime_names_are_only_reserved_for_a_mocks_export() {
        let output_dir = std::path::Path::new("target/tests/mocks-reserved-names");
        let _ = fs::remove_dir_all(output_dir);
        fs::create_dir_all(output_dir).expect("failed to create test output directory");

        // The mocks runtime is opt-in, so its names must not stop a user type from
        // exporting on a build that never emits it.
        let builder = Builder::<tauri::Wry>::new().typ::<mock_names::MockCommandHandlers>();

        builder
            .export(Typescript::default(), output_dir.join("bindings.ts"))
            .expect("mocks runtime names should not be reserved without `Builder::mocks`");
        assert!(
            fs::read_to_string(output_dir.join("bindings.ts"))
                .expect("failed to read TypeScript bindings")
                .contains("export type MockCommandHandlers = {")
        );

        builder
            .export(JSDoc::default(), output_dir.join("bindings.js"))
            .expect("mocks runtime names should not be reserved without `Builder::mocks`");
        assert!(
            fs::read_to_string(output_dir.join("bindings.js"))
                .expect("failed to read JSDoc bindings")
                .contains("MockCommandHandlers")
        );

        let err = Builder::<tauri::Wry>::new()
            .commands(collect_commands![nullable_result])
            .mocks()
            .typ::<mock_names::MockCommandHandlers>()
            .export(Typescript::default(), output_dir.join("conflict.ts"))
            .expect_err("mocks runtime names should be reserved by `Builder::mocks`");
        assert!(
            err.to_string()
                .contains("User defined type 'MockCommandHandlers'"),
            "{err}"
        );

        // `MockCall` and `MockCommandsHandle` are declared by the mocks runtime too, so
        // they are reserved under the same rule.
        for (name, result) in [
            (
                "MockCall",
                Builder::<tauri::Wry>::new()
                    .commands(collect_commands![nullable_result])
                    .mocks()
                    .typ::<mock_names::MockCall>()
                    .export(Typescript::default(), output_dir.join("conflict-call.ts")),
            ),
            (
                "MockCommandsHandle",
                Builder::<tauri::Wry>::new()
                    .commands(collect_commands![nullable_result])
                    .mocks()
                    .typ::<mock_names::MockCommandsHandle>()
                    .export(Typescript::default(), output_dir.join("conflict-handle.ts")),
            ),
        ] {
            let err =
                result.expect_err("mocks runtime names should be reserved by `Builder::mocks`");
            assert!(
                err.to_string()
                    .contains(&format!("User defined type '{name}'")),
                "{err}"
            );
        }

        fs::remove_dir_all(output_dir).expect("failed to remove test output directory");
    }

    #[test]
    fn mocks_reject_semantic_type_transforms() {
        let output_dir = std::path::Path::new("target/tests/mocks-semantic-types");
        let _ = fs::remove_dir_all(output_dir);
        fs::create_dir_all(output_dir).expect("failed to create test output directory");

        // Without a semantic type there is no transform, so the handler is typed against
        // the value that really crosses the IPC boundary.
        Builder::<tauri::Wry>::new()
            .commands(collect_commands![nested_semantic_bytes])
            .mocks()
            .export(Typescript::default(), output_dir.join("bindings.ts"))
            .expect("mocks without semantic types should export");
        assert!(
            fs::read_to_string(output_dir.join("bindings.ts"))
                .expect("failed to read TypeScript bindings")
                .contains("nestedSemanticBytes: (args: { input: NestedSemanticBytes })"),
        );

        // A rule nested in a struct does get a transform, and the handler would be handed
        // the transformed value. That can't be typed without re-deriving the transform's
        // output type, so it is refused rather than mistyped.
        let err = Builder::<tauri::Wry>::new()
            .commands(collect_commands![nested_semantic_bytes])
            .semantic_types(semantic_bytes())
            .mocks()
            .export(Typescript::default(), output_dir.join("semantic.ts"))
            .expect_err("a semantic type transform should be refused by `Builder::mocks`");
        assert!(
            err.to_string()
                .contains("argument 'input' is reshaped by a semantic type transform"),
            "{err}"
        );

        fs::remove_dir_all(output_dir).expect("failed to remove test output directory");
    }

    #[test]
    fn mocks_dispatch_on_the_plugin_prefixed_command_name() {
        // The mock switch has to match the name Tauri's invoke handler receives, which
        // for a plugin is prefixed the same way the generated `commands` call is.
        let output_dir = std::path::Path::new("target/tests/mocks-plugin");
        let _ = fs::remove_dir_all(output_dir);
        fs::create_dir_all(output_dir).expect("failed to create test output directory");

        Builder::<tauri::Wry>::new()
            .plugin_name("my-plugin")
            .commands(collect_commands![nullable_result])
            .mocks()
            .export(Typescript::default(), output_dir.join("bindings.ts"))
            .expect("failed to export TypeScript bindings");

        let ts = fs::read_to_string(output_dir.join("bindings.ts"))
            .expect("failed to read TypeScript bindings");
        assert!(
            ts.contains(
                r#"case "plugin:my-plugin|nullable_result": calls.push({ command: "nullableResult", args }); return mockCommandHandler(mockHandlers.nullableResult, "plugin:my-plugin|nullable_result", args);"#
            ),
            "{ts}"
        );

        fs::remove_dir_all(output_dir).expect("failed to remove test output directory");
    }

    #[test]
    fn semantic_error_transforms_only_apply_to_typed_errors() {
        let semantic_types =
            specta_typescript::semantic::Configuration::empty().enable_lossless_bigints();

        let unknown_path = std::env::temp_dir().join(format!(
            "tauri-specta-result-unknown-semantic-error-test-{}.ts",
            std::process::id()
        ));
        Builder::<tauri::Wry>::new()
            .commands(collect_commands![bigint_error])
            .semantic_types(semantic_types.clone())
            .error_handling(ErrorHandlingMode::ResultUnknown)
            .mocks()
            .export(Typescript::default(), &unknown_path)
            .expect("mocks are allowed because ResultUnknown leaves errors untouched");
        let ts = fs::read_to_string(&unknown_path).expect("failed to read TypeScript bindings");
        assert!(!ts.contains("BigInt("), "{ts}");

        // `Result` still assumes the error has the declared type and transforms it.
        let result_path = std::env::temp_dir().join(format!(
            "tauri-specta-result-semantic-error-test-{}.ts",
            std::process::id()
        ));
        Builder::<tauri::Wry>::new()
            .commands(collect_commands![bigint_error])
            .semantic_types(semantic_types)
            .error_handling(ErrorHandlingMode::Result)
            .export(Typescript::default(), &result_path)
            .expect("failed to export TypeScript bindings");
        let ts = fs::read_to_string(&result_path).expect("failed to read TypeScript bindings");
        assert!(ts.contains("BigInt(v.error)"), "{ts}");
    }
}
