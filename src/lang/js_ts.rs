use std::{borrow::Cow, path::Path};

use heck::ToLowerCamelCase;
use specta::ResolvedTypes;
use specta::datatype::{DataType, Field, Fields, Function, Primitive, Reference, Struct};
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
                        TYPED_ERROR_IMPL_TS
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
                        TYPED_ERROR_IMPL_JS
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
    let mut types = cfg.types.clone();

    types.iter_mut(|ndt| {
        rewrite_bigints_in_datatype(
            ndt.ty_mut(),
            cfg.enable_nuanced_types,
            !cfg.disable_serde_phases,
        )
    });

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
    let all_commands: Vec<&Function> = cfg
        .commands
        .iter()
        .chain(&cfg.queries)
        .chain(&cfg.mutations)
        .collect();
    let enabled_commands = !all_commands.is_empty();
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

    let is_channel_used = all_commands.iter().any(|command| {
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
        && all_commands.iter().any(|command| {
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
    if let Some(framework) = &cfg.tanstack {
        let has_queries = !cfg.queries.is_empty();
        let has_mutations = !cfg.mutations.is_empty();
        if has_queries || has_mutations {
            let mut tanstack_imports = Vec::new();
            if has_queries {
                tanstack_imports.push("queryOptions as __TANSTACK_QUERY_OPTIONS");
            }
            if has_mutations {
                tanstack_imports.push("mutationOptions as __TANSTACK_MUTATION_OPTIONS");
            }
            out.push_str(&format!(
                "import {{ {} }} from \"{}\";\n",
                tanstack_imports.join(", "),
                framework.package_name()
            ));
        }
    }

    // Commands (includes queries and mutations)
    if enabled_commands {
        let mut s = Struct::named();
        for command in &all_commands {
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

    // TanStack Query
    let has_unwrap_typed_error = cfg.tanstack.is_some()
        && cfg
            .queries
            .iter()
            .chain(&cfg.mutations)
            .any(|cmd| command_uses_typed_error(cmd, exporter.types, cfg));

    if let Some(_framework) = &cfg.tanstack {
        let has_queries = !cfg.queries.is_empty();
        let has_mutations = !cfg.mutations.is_empty();

        // Queries + Query Keys
        if has_queries {
            let mut queries_struct = Struct::named();
            let mut query_keys_struct = Struct::named();
            for command in &cfg.queries {
                // skip validation here since we validate the same commands for the `commands` export
                let command_name = command.name().to_lower_camel_case();
                let has_no_args = command.args().is_empty();

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

                let call_args = if has_no_args {
                    String::new()
                } else {
                    arguments
                        .iter()
                        .map(|(name, _)| name.to_lower_camel_case())
                        .collect::<Vec<_>>()
                        .join(", ")
                };

                let key_prefix = if let Some(plugin_name) = cfg.plugin_name {
                    format!("\"plugin:{plugin_name}\", \"{command_name}\"")
                } else {
                    format!("\"{command_name}\"")
                };

                let as_const = if jsdoc { "" } else { " as const" };
                let key_body = if has_no_args {
                    format!("() => [{key_prefix}]{as_const}")
                } else {
                    let first_arg = &arguments[0].0;
                    let args_obj = format!("{{ {} }}", call_args);

                    format!(
                        "({fn_arguments}) => {first_arg} !== undefined ? [{key_prefix}, {args_obj}]{as_const} : [{key_prefix}]{as_const}",
                    )
                };

                query_keys_struct = query_keys_struct
                    .field(command_name.clone(), Field::new(define(key_body).into()));

                let (data_type, error_type) =
                    extract_tanstack_result_types(command, &exporter, cfg)?;

                let generics = if jsdoc {
                    String::new()
                } else {
                    match &error_type {
                        Some(e) => format!("<{data_type}, {e}>"),
                        None => format!("<{data_type}>"),
                    }
                };

                let query_fn_body = if command_uses_typed_error(command, exporter.types, cfg) {
                    format!("unwrapTypedError(commands.{command_name}({call_args}))")
                } else {
                    format!("commands.{command_name}({call_args})")
                };
                let query_body = format!(
                    "({fn_arguments}) => __TANSTACK_QUERY_OPTIONS{generics}({{ queryKey: queryKeys.{command_name}({call_args}), queryFn: () => {query_fn_body} }})"
                );

                queries_struct = queries_struct
                    .field(command_name.clone(), Field::new(define(query_body).into()));
            }

            out.push_str("\n/** Query Keys */");
            out.push_str("\nexport const queryKeys = ");
            out.push_str(&match &query_keys_struct.build() {
                DataType::Reference(r) => exporter.reference(&r)?,
                dt => exporter.inline(dt)?,
            });
            out.push_str(";\n");

            out.push_str("\n/** Queries */");
            out.push_str("\nexport const queries = ");
            out.push_str(&match &queries_struct.build() {
                DataType::Reference(r) => exporter.reference(&r)?,
                dt => exporter.inline(dt)?,
            });
            out.push_str(";\n");
        }

        // Mutations + Mutation Keys
        if has_mutations {
            let mut mutations_struct = Struct::named();
            let mut mutation_keys_struct = Struct::named();
            for command in &cfg.mutations {
                // skip validation here since we validate the same commands for the `commands` export
                let command_name = command.name().to_lower_camel_case();
                let has_no_args = command.args().is_empty();

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

                let call_args = if has_no_args {
                    String::new()
                } else {
                    arguments
                        .iter()
                        .map(|(name, _)| name.to_lower_camel_case())
                        .collect::<Vec<_>>()
                        .join(", ")
                };

                let key_prefix = if let Some(plugin_name) = cfg.plugin_name {
                    format!("\"plugin:{plugin_name}\", \"{command_name}\"")
                } else {
                    format!("\"{command_name}\"")
                };

                let as_const = if jsdoc { "" } else { " as const" };
                let key_body = format!("() => [{key_prefix}]{as_const}");

                mutation_keys_struct = mutation_keys_struct
                    .field(command_name.clone(), Field::new(define(key_body).into()));

                let (data_type, error_type) =
                    extract_tanstack_result_types(command, &exporter, cfg)?;

                let variables_type = if has_no_args {
                    "void".to_string()
                } else {
                    format!(
                        "{{ {} }}",
                        arguments
                            .iter()
                            .map(|(name, dt)| format!("{name}: {dt}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };

                let generics = if jsdoc {
                    String::new()
                } else {
                    match &error_type {
                        Some(e) => format!("<{data_type}, {e}, {variables_type}>"),
                        None => format!("<{data_type}, Error, {variables_type}>"),
                    }
                };

                let mutation_fn_param = if has_no_args {
                    String::new()
                } else if jsdoc {
                    format!("{{ {call_args} }}")
                } else {
                    format!("{{ {call_args} }}: {variables_type}")
                };

                let mutation_fn_body = if command_uses_typed_error(command, exporter.types, cfg) {
                    format!("unwrapTypedError(commands.{command_name}({call_args}))")
                } else {
                    format!("commands.{command_name}({call_args})")
                };
                let mutation_body = format!(
                    "() => __TANSTACK_MUTATION_OPTIONS{generics}({{ mutationKey: mutationKeys.{command_name}(), mutationFn: ({mutation_fn_param}) => {mutation_fn_body} }})"
                );

                mutations_struct = mutations_struct.field(
                    command_name.clone(),
                    Field::new(define(mutation_body).into()),
                );
            }

            out.push_str("\n/** Mutation Keys */");
            out.push_str("\nexport const mutationKeys = ");
            out.push_str(&match &mutation_keys_struct.build() {
                DataType::Reference(r) => exporter.reference(&r)?,
                dt => exporter.inline(dt)?,
            });
            out.push_str(";\n");

            out.push_str("\n/** Mutations */");
            out.push_str("\nexport const mutations = ");
            out.push_str(&match &mutations_struct.build() {
                DataType::Reference(r) => exporter.reference(&r)?,
                dt => exporter.inline(dt)?,
            });
            out.push_str(";\n");
        }
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
            if jsdoc {
                field.set_docs(
                    format!(
                        "@type {{ReturnType<typeof makeEvent<{}>>}}",
                        exporter.reference(r)?
                    )
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
    if has_typed_error || has_unwrap_typed_error || enabled_events {
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
        }
        if has_unwrap_typed_error {
            out.push('\n');
            if jsdoc {
                out.push_str(UNWRAP_TYPED_ERROR_IMPL_JS);
            } else {
                out.push_str(UNWRAP_TYPED_ERROR_IMPL_TS);
            }
            out.push('\n');
        }
        if enabled_events {
            out.push('\n');
            out.push_str(make_event_impl);
            out.push('\n');
        }
    }

    Ok(Cow::Owned(out))
}

/// Whether a command's generated body uses `typedError` wrapping.
fn command_uses_typed_error(
    command: &Function,
    types: &ResolvedTypes,
    cfg: &BuilderConfiguration,
) -> bool {
    cfg.error_handling == ErrorHandlingMode::Result
        && command
            .result()
            .and_then(|dt| extract_std_result(dt, types))
            .is_some()
}

/// Extract data type and optional error type for TanStack Query generics.
/// For Result<T, E>, returns (T, Some(E)). For plain T, returns (T, None).
/// TanStack always throws on error, so we unwrap Result.
fn extract_tanstack_result_types(
    command: &Function,
    exporter: &FrameworkExporter,
    cfg: &BuilderConfiguration,
) -> Result<(String, Option<String>), Error> {
    match command.result() {
        Some(dt) => {
            if let Some((ok, err)) = extract_std_result(dt, exporter.types) {
                let data = render_reference_dt_for_phase(ok, Phase::Serialize, exporter, cfg)?;
                let error = render_reference_dt_for_phase(err, Phase::Serialize, exporter, cfg)?;
                Ok((data, Some(error)))
            } else {
                let data = render_reference_dt_for_phase(dt, Phase::Serialize, exporter, cfg)?;
                Ok((data, None))
            }
        }
        None => Ok(("void".to_string(), None)),
    }
}

fn rewrite_bigints_in_datatype(dt: &mut DataType, nuanced: bool, phased: bool) {
    fn rewrite_bigints_in_fields(fields: &mut Fields, nuanced: bool, phased: bool) {
        match fields {
            Fields::Unit => {}
            Fields::Unnamed(fields) => {
                for field in fields.fields_mut() {
                    if let Some(ty) = field.ty_mut() {
                        rewrite_bigints_in_datatype(ty, nuanced, phased);
                    }
                }
            }
            Fields::Named(fields) => {
                for (_, field) in fields.fields_mut() {
                    if let Some(ty) = field.ty_mut() {
                        rewrite_bigints_in_datatype(ty, nuanced, phased);
                    }
                }
            }
        }
    }

    match dt {
        DataType::Primitive(primitive) => match primitive {
            Primitive::usize
            | Primitive::isize
            | Primitive::u64
            | Primitive::i64
            | Primitive::u128
            | Primitive::i128 => {
                *dt = if !nuanced {
                    // TODO: This is temporary until we have official support for large number types in Tauri.
                    DataType::Primitive(Primitive::u32)
                } else if phased {
                    specta_serde::phased(define("bigint | number").into(), define("bigint").into())
                } else {
                    define("bigint | number").into()
                };
            }
            Primitive::f128 => {
                *dt = DataType::Primitive(Primitive::f64);
            }
            _ => {}
        },
        DataType::List(list) => rewrite_bigints_in_datatype(list.ty_mut(), nuanced, phased),
        DataType::Map(map) => {
            rewrite_bigints_in_datatype(map.key_ty_mut(), nuanced, phased);
            rewrite_bigints_in_datatype(map.value_ty_mut(), nuanced, phased);
        }
        DataType::Struct(strct) => rewrite_bigints_in_fields(strct.fields_mut(), nuanced, phased),
        DataType::Enum(enm) => {
            for (_, variant) in enm.variants_mut() {
                rewrite_bigints_in_fields(variant.fields_mut(), nuanced, phased);
            }
        }
        DataType::Tuple(tuple) => {
            for item in tuple.elements_mut() {
                rewrite_bigints_in_datatype(item, nuanced, phased);
            }
        }
        DataType::Nullable(inner) => rewrite_bigints_in_datatype(inner, nuanced, phased),
        DataType::Reference(Reference::Named(reference)) => {
            for (_, generic) in reference.generics_mut() {
                rewrite_bigints_in_datatype(generic, nuanced, phased);
            }
        }
        DataType::Reference(Reference::Generic(_)) | DataType::Reference(Reference::Opaque(_)) => {}
    }
}

fn rewrite_bigints_for_export(dt: &DataType, cfg: &BuilderConfiguration) -> DataType {
    let mut dt = dt.clone();
    rewrite_bigints_in_datatype(&mut dt, cfg.enable_nuanced_types, !cfg.disable_serde_phases);
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
    "__TANSTACK_QUERY_OPTIONS",
    "__TANSTACK_MUTATION_OPTIONS",
    "typedError",
    "unwrapTypedError",
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

const UNWRAP_TYPED_ERROR_IMPL_TS: &str = r#"async function unwrapTypedError<T, E>(result: Promise<{ status: "ok"; data: T } | { status: "error"; error: E }>): Promise<T> {
    const v = await result;
    if (v.status === "error") throw v.error;
    return v.data;
}"#;

const UNWRAP_TYPED_ERROR_IMPL_JS: &str = r#"/**
  * @template T
  * @template E
  * @param {Promise<{ status: "ok"; data: T } | { status: "error"; error: E }>} result
  * @returns {Promise<T>}
  */
async function unwrapTypedError(result) {
    const v = await result;
    if (v.status === "error") throw v.error;
    return v.data;
}"#;

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
