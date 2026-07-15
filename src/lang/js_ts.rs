use std::{borrow::Cow, path::Path};

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
        let types = hide_unused_std_result_type(&cfg, cfg.types.clone());
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
                            ErrorHandlingMode::Throw | ErrorHandlingMode::Result => {
                                TYPED_ERROR_IMPL_TS
                            }
                        }
                    } else {
                        &cfg.typed_error_impl
                    },
                    TYPED_ERROR_ASSERTION_TS,
                    MAKE_EVENT_IMPL_TS,
                    MAP_CHANNEL_IMPL_TS,
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
                            ErrorHandlingMode::Throw | ErrorHandlingMode::Result => {
                                TYPED_ERROR_IMPL_JS
                            }
                        }
                    } else {
                        &cfg.typed_error_impl
                    },
                    "",
                    MAKE_EVENT_IMPL_JS,
                    MAP_CHANNEL_IMPL_JS,
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
    map_channel_impl: &str,
) -> Result<Cow<'static, str>, Error> {
    let enabled_commands = !cfg.commands.is_empty();
    let enabled_events = !cfg.events.is_empty();
    let semantic_types_runtime_types = semantic_types_runtime_types(cfg)?;
    let semantic_types_runtime_types = semantic_types_runtime_types
        .as_ref()
        .unwrap_or(exporter.types);

    let mut out = String::new();

    if let Some((ndt, name)) = exporter
        .types
        .into_unsorted_iter()
        .filter(|ndt| ndt.ty.is_some())
        .find_map(|ndt| {
            runtime_scope_name(jsdoc, exporter.layout, ndt)
                .filter(|name| RESERVED_NDT_NAMES.contains(&name.as_ref()))
                .map(|name| (ndt, name))
        })
    {
        return Err(Error::framework(
            "",
            format!(
                "User defined type '{}' defined in {} must be renamed so it doesn't conflict with Tauri Specta runtime.",
                name, ndt.location
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
    let is_channel_transform_used = cfg.commands.iter().any(|command| {
        command.args().iter().any(|(_, dt)| {
            channel_generic_type(dt, exporter.types).is_some_and(|dt| {
                render_result_transform_for_phase(
                    dt,
                    Phase::Deserialize,
                    "v",
                    &exporter,
                    cfg,
                    semantic_types_runtime_types,
                )
                .is_some()
            })
        })
    });
    let has_typed_error = enabled_commands
        && cfg.error_handling != ErrorHandlingMode::Throw
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
                        render_reference_dt_for_phase(
                            dt,
                            Phase::Deserialize,
                            Phase::Serialize,
                            &exporter,
                            cfg,
                            semantic_types_runtime_types,
                        )?,
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
                        .map(|(name, dt)| {
                            let name = name.to_lower_camel_case();
                            let value = if let Some(generic) =
                                channel_generic_type(dt, exporter.types)
                            {
                                render_result_transform_for_phase(
                                    generic,
                                    Phase::Deserialize,
                                    "v",
                                    &exporter,
                                    cfg,
                                    semantic_types_runtime_types,
                                )
                                .map(|transform| jsdoc_transform(transform, "v", jsdoc))
                                .map(|transform| format!("mapChannel({name}, (v) => {transform})"))
                            } else {
                                render_result_transform_for_phase(
                                    dt,
                                    Phase::Serialize,
                                    &name,
                                    &exporter,
                                    cfg,
                                    semantic_types_runtime_types,
                                )
                                .map(|transform| jsdoc_transform(transform, &name, jsdoc))
                            };

                            value.map_or(name.clone(), |value| format!("{name}: {value}"))
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };

            let invoke_args = format!("({command_name_escaped}{arguments_invoke_obj})",);

            let body = if cfg.error_handling != ErrorHandlingMode::Throw
                && let Some(result) = command.result()
                && let Some((dt_ok, dt_err)) = extract_std_result(result, exporter.types)
            {
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
                        command.name().to_string(),
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
                let err_transform = render_result_transform_for_phase(
                    dt_err,
                    Phase::Deserialize,
                    "v.error",
                    &exporter,
                    cfg,
                    semantic_types_runtime_types,
                )
                .map(|transform| jsdoc_transform(transform, "v.error", jsdoc));

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
                let output_dt = command
                    .result()
                    .and_then(|dt| extract_std_result(dt, exporter.types).map(|(ok, _)| ok))
                    .or(command.result());

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
                    invoke_ts.push_str(&match command.result() {
                        Some(dt) => Cow::Owned(render_reference_dt_for_phase(
                            extract_std_result(dt, exporter.types)
                                .map(|(ok, _)| ok)
                                .unwrap_or(dt),
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
                            .map(|(name, dt)| format!("@param {{{dt}}} {name}"))
                            .collect::<Vec<_>>()
                            .join("\n"),
                    );

                    if !arguments.is_empty() {
                        docs.push('\n');
                    }

                    let returns = if cfg.error_handling != ErrorHandlingMode::Throw
                        && let Some(result) = command.result()
                        && let Some((dt_ok, dt_err)) = extract_std_result(result, exporter.types)
                    {
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
                        let output_dt = command
                            .result()
                            .and_then(|dt| extract_std_result(dt, exporter.types).map(|(ok, _)| ok))
                            .or(command.result());
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
                cfg.function_casing.apply(command.name()).into_owned(),
                field,
            );
        }

        out.push_str("\n/** Commands */");
        out.push_str("\nexport const commands = ");
        out.push_str(&exporter.reference(&s.build())?);
        out.push_str(";\n");
    }

    // Events
    if enabled_events {
        let mut s = Struct::named();
        for (name, (_, r)) in &cfg.events {
            let event_name_escaped =
                serde_json::to_string(&resolve_tauri_event_name(cfg.plugin_name, name))
                    .expect("failed to serialize string");

            let event_dt = DataType::Reference(r.clone());
            let mut field_ts = "makeEvent".to_string();
            if !jsdoc {
                field_ts.push('<');
                field_ts.push_str(&render_reference_dt_for_phase(
                    &event_dt,
                    Phase::Serialize,
                    Phase::Deserialize,
                    &exporter,
                    cfg,
                    semantic_types_runtime_types,
                )?);
                field_ts.push_str(", ");
                field_ts.push_str(&render_reference_dt_for_phase(
                    &event_dt,
                    Phase::Deserialize,
                    Phase::Serialize,
                    &exporter,
                    cfg,
                    semantic_types_runtime_types,
                )?);
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
                field.docs = format!(
                    "@type {{ReturnType<typeof makeEvent<{}>>}}",
                    render_reference_dt_for_phase(
                        &DataType::Reference(r.clone()),
                        Phase::Deserialize,
                        Phase::Deserialize,
                        &exporter,
                        cfg,
                        semantic_types_runtime_types,
                    )?
                )
                .into();
            }
            s = s.field(cfg.function_casing.apply(name).into_owned(), field);
        }

        out.push_str("\n/** Events */");
        out.push_str("\nexport const events = ");
        out.push_str(&exporter.reference(&s.build())?);
        out.push_str(";\n");
    }

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
    if has_typed_error || enabled_events || is_channel_transform_used {
        out.push_str("\n/* Tauri Specta runtime */\n");

        if is_channel_transform_used {
            out.push_str(map_channel_impl);
            out.push('\n');
            if has_typed_error || enabled_events {
                out.push('\n');
            }
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
            .args()
            .iter()
            .any(|(_, dt)| datatype_contains_std_result(dt, &types))
            || command.result().is_some_and(|dt| {
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

fn is_channel_type(dt: &DataType, types: &Types) -> bool {
    channel_generic_type(dt, types).is_some()
}

fn channel_generic_type<'a>(dt: &'a DataType, types: &Types) -> Option<&'a DataType> {
    let DataType::Reference(Reference::Named(r)) = dt else {
        return None;
    };
    let ndt = types.get(r)?;
    if ndt.name != "TAURI_CHANNEL" || !ndt.module_path.starts_with("tauri::") {
        return None;
    }
    match &r.inner {
        NamedReferenceType::Reference { generics, .. } => generics.first().map(|(_, dt)| dt),
        NamedReferenceType::Inline { .. } | NamedReferenceType::Recursive(_) => None,
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
    let dt = specta_serde::select_phase_datatype(dt, semantic_types_runtime_types, phase);
    let dt = if let DataType::Reference(Reference::Named(r)) = &dt
        && let Some(ndt) = semantic_types_runtime_types.get(r)
        && let Some(dt) = &ndt.ty
    {
        dt.clone()
    } else {
        dt
    };
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
        (ErrorHandlingMode::Result, Some(ok), Some(err)) => format!(
            "(v.status === \"ok\" ? {{ ...v, data: {ok} }} : v.status === \"error\" ? {{ ...v, error: {err} }} : v)"
        ),
        (ErrorHandlingMode::Result, Some(ok), None) => {
            format!("(v.status === \"ok\" ? {{ ...v, data: {ok} }} : v)")
        }
        (ErrorHandlingMode::Result, None, Some(err)) => {
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
    if let DataType::Reference(Reference::Named(r)) = dt
        && let Some(ndt) = exporter.types.get(r)
        && ndt.name == "TAURI_CHANNEL"
        && ndt.module_path.starts_with("tauri::")
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
    "typedError",
    "makeEvent",
    "mapChannel",
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

const DATA_ERROR_IMPL_TS: &str = r#"async function typedError<T, E>(result: Promise<T>): Promise<{ data: T; error: null } | { data: null; error: E }> {
    try {
        return { data: await result, error: null };
    } catch (e) {
        if (e instanceof Error) throw e;
        return { data: null, error: e as any };
    }
}"#;

const MAP_CHANNEL_IMPL_TS: &str = r#"function mapChannel<T>(channel: Channel<T>, deserialize: (payload: any) => T): Channel<T> {
    return new Channel((payload) => channel.onmessage(deserialize(payload)));
}"#;

const MAP_CHANNEL_IMPL_JS: &str = r#"/**
 * @template T
 * @param {Channel<T>} channel
 * @param {(payload: any) => T} deserialize
 * @returns {Channel<T>}
 */
function mapChannel(channel, deserialize) {
    return new Channel((payload) => channel.onmessage(deserialize(payload)));
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
        return { data: null, error: e };
    }
}"#;

const TYPED_ERROR_ASSERTION_TS: &str = "const _assertTypedErrorFollowsContract: <T, E>(result: Promise<T>) => Promise<any> = typedError;";

const MAKE_EVENT_IMPL_TS: &str = r#"type EventEmit<T> = [T] extends [null] ? () => Promise<void> : (payload: T) => Promise<void>;

function makeEvent<TListen, TEmit = TListen>(name: string, serialize?: (payload: TEmit) => unknown, deserialize?: (payload: any) => TListen) {
    const mapEvent = (cb: __TAURI_EVENT.EventCallback<TListen>) => (event: __TAURI_EVENT.Event<any>) => cb({ ...event, payload: deserialize ? deserialize(event.payload) : event.payload });
    const mapPayload = (payload: TEmit) => serialize ? serialize(payload) : payload;

    const base = {
        listen: (cb: __TAURI_EVENT.EventCallback<TListen>) => __TAURI_EVENT.listen(name, mapEvent(cb)),
        once: (cb: __TAURI_EVENT.EventCallback<TListen>) => __TAURI_EVENT.once(name, mapEvent(cb)),
        emit: ((payload: TEmit) => __TAURI_EVENT.emit(name, mapPayload(payload)) as unknown) as EventEmit<TEmit>
    };

    const fn = (target: import("@tauri-apps/api/webview").Webview | import("@tauri-apps/api/window").Window) => ({
        listen: (cb: __TAURI_EVENT.EventCallback<TListen>) => target.listen(name, mapEvent(cb)),
        once: (cb: __TAURI_EVENT.EventCallback<TListen>) => target.once(name, mapEvent(cb)),
        emit: ((payload: TEmit) => target.emit(name, mapPayload(payload)) as unknown) as EventEmit<TEmit>
    });

    return Object.assign(fn, base);
}"#;

const MAKE_EVENT_IMPL_JS: &str = r#"/**
 * @template T
 * @param {string} name
 * @param {(payload: T) => unknown} [serialize]
 * @param {(payload: any) => T} [deserialize]
 */
function makeEvent(name, serialize, deserialize) {
    const mapEvent = (cb) => (event) => cb({ ...event, payload: deserialize ? deserialize(event.payload) : event.payload });
    const mapPayload = (payload) => serialize ? serialize(payload) : payload;

    const base = {
        /** @param {__TAURI_EVENT.EventCallback<T>} cb */
        listen: (cb) => __TAURI_EVENT.listen(name, mapEvent(cb)),
        /** @param {__TAURI_EVENT.EventCallback<T>} cb */
        once: (cb) => __TAURI_EVENT.once(name, mapEvent(cb)),
        /** @param {T} payload */
        emit: (payload) => __TAURI_EVENT.emit(name, mapPayload(payload)),
    };

    /** @param {import("@tauri-apps/api/webview").Webview | import("@tauri-apps/api/window").Window} target */
    const fn = (target) => ({
        /** @param {__TAURI_EVENT.EventCallback<T>} cb */
        listen: (cb) => target.listen(name, mapEvent(cb)),
        /** @param {__TAURI_EVENT.EventCallback<T>} cb */
        once: (cb) => target.once(name, mapEvent(cb)),
        /** @param {T} payload */
        emit: (payload) => target.emit(name, mapPayload(payload)),
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
    use specta_typescript::{JSDoc, Layout, Typescript};

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
}
