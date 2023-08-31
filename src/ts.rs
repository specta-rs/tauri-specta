use crate::{EventDataType, ExportLanguage, ItemType, PluginName, DO_NOT_EDIT};
use heck::ToLowerCamelCase;
use indoc::formatdoc;
use specta::{
    functions::FunctionDataType,
    ts::{self, TsExportError},
    DataType, TypeMap,
};

/// Implements [`ExportLanguage`] for TypeScript exporting
pub struct Language;

/// [`Exporter`](crate::Exporter) for TypeScript
pub type Exporter<TCommands, TEvents> = crate::Exporter<Language, TCommands, TEvents>;

impl ExportLanguage for Language {
    fn globals() -> String {
        include_str!("./globals.ts").to_string()
    }

    /// Renders a collection of [`FunctionDataType`] into a TypeScript string.
    fn render_commands(
        commands: &[FunctionDataType],
        type_map: &TypeMap,
        cfg: &ExportConfig,
    ) -> Result<String, TsExportError> {
        let commands = commands
            .into_iter()
            .map(|function| {
                let docs = specta::ts::js_doc(&function.docs);

                let name_camel = function.name.to_lower_camel_case();

                let arg_defs = function
                    .args
                    .iter()
                    .map(|(name, typ)| {
                        ts::datatype(&cfg.inner, typ, &type_map)
                            .map(|ty| format!("{}: {}", name.to_lower_camel_case(), ty))
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ");

                let ok_type = match &function.result {
                    DataType::Result(t) => {
                        let (t, _) = t.as_ref();

                        ts::datatype(&cfg.inner, t, &type_map)?
                    }
                    t => ts::datatype(&cfg.inner, t, &type_map)?,
                };

                let ret_type = match &function.result {
                    DataType::Result(t) => {
                        let (_, e) = t.as_ref();

                        format!(
                            "__Result__<{ok_type}, {}>",
                            ts::datatype(&cfg.inner, e, &type_map)?
                        )
                    }
                    _ => ok_type.clone(),
                };

                let body = {
                    let name = cfg
                        .plugin_name
                        .apply_as_prefix(&function.name, ItemType::Command);

                    let arg_usages = function
                        .args
                        .iter()
                        .map(|(name, _)| name.to_lower_camel_case())
                        .collect::<Vec<_>>();

                    let arg_usages = arg_usages
                        .is_empty()
                        .then(Default::default)
                        .unwrap_or_else(|| format!(", {{ {} }}", arg_usages.join(",")));

                    let invoke = format!("await TAURI_INVOKE<{ok_type}>(\"{name}\"{arg_usages})");

                    match &function.result {
                        DataType::Result(_) => formatdoc!(
                            r#"
	                        try {{
	                            return [{invoke}, undefined];
	                        }} catch (e: any) {{
	                            if(e instanceof Error) throw e;
	                            else return [undefined, e];
	                        }}"#
                        ),
                        _ => format!("return {invoke};"),
                    }
                };

                Ok(formatdoc!(
                    r#"
	                {docs}async {name_camel}({arg_defs}): Promise<{ret_type}> {{
	                {body}
	                }}"#
                ))
            })
            .collect::<Result<Vec<_>, TsExportError>>()?
            .join(",\n");

        Ok(formatdoc! {
            r#"
            export const commands = {{
            {commands}
            }}"#
        })
    }

    fn render_events(
        events: &[EventDataType],
        type_map: &TypeMap,
        cfg: &ExportConfig,
    ) -> Result<String, TsExportError> {
        if events.is_empty() {
            return Ok(Default::default());
        }

        let events_map = events
            .iter()
            .map(|event| {
                let name_str = cfg.plugin_name.apply_as_prefix(event.name, ItemType::Event);
                let name_camel = event.name.to_lower_camel_case();

                format!(r#"{name_camel}: "{name_str}""#)
            })
            .collect::<Vec<_>>()
            .join(",\n");

        let events = events
            .iter()
            .map(|event| {
                let name_camel = event.name.to_lower_camel_case();

                let typ = ts::datatype(&cfg.inner, &event.typ, type_map)?;

                Ok(format!(r#"	{name_camel}: {typ}"#))
            })
            .collect::<Result<Vec<_>, TsExportError>>()?
            .join(",\n");

        Ok(formatdoc! {
            r#"
            export const events = __makeEvents__<{{
            {events}
            }}>({{
            {events_map}
            }})"#
        })
    }

    fn render(
        commands: &[FunctionDataType],
        events: &[EventDataType],
        type_map: &TypeMap,
        cfg: &ExportConfig,
    ) -> Result<String, TsExportError> {
        let globals = Self::globals();

        let commands = Self::render_commands(commands, &type_map, cfg)?;
        let events = Self::render_events(events, &type_map, cfg)?;

        let dependant_types = type_map
            .values()
            .filter_map(|v| v.as_ref())
            .map(|v| ts::export_named_datatype(&cfg.inner, v, &type_map))
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.join("\n"))?;

        Ok(formatdoc! {
            r#"
	            {DO_NOT_EDIT}

				{commands}

				{events}

	            {dependant_types}

				{globals}
	        "#
        })
    }
}

/// The configuration for the generator
#[derive(Default)]
pub struct ExportConfig {
    /// The name of the plugin to invoke.
    ///
    /// If there is no plugin name (i.e. this is an app), this should be `None`.
    pub(crate) plugin_name: PluginName,
    /// The specta export configuration
    pub(crate) inner: specta::ts::ExportConfig,
}

impl ExportConfig {
    /// Creates a new [`ExportConfiguration`] from a [`specta::ts::ExportConfiguration`]
    pub fn new(config: specta::ts::ExportConfig) -> Self {
        Self {
            inner: config,
            ..Default::default()
        }
    }
}

impl From<specta::ts::ExportConfig> for ExportConfig {
    fn from(config: specta::ts::ExportConfig) -> Self {
        Self {
            inner: config,
            ..Default::default()
        }
    }
}
