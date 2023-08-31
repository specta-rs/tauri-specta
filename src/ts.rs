use crate::{EventMeta, ExportLanguage, NoCommands, NoEvents, DO_NOT_EDIT};
use heck::ToLowerCamelCase;
use indoc::formatdoc;
use specta::{
    functions::{FunctionDataType, SpectaFunctionResultVariant},
    ts::{self, TsExportError},
    TypeDefs,
};
use std::borrow::Cow;

/// Implements [`ExportLanguage`] for TypeScript exporting
pub struct Language;

/// [`Exporter`](crate::Exporter) for TypeScript
pub type Exporter<TRuntime> = crate::Exporter<Language, NoCommands<TRuntime>, NoEvents>;

impl ExportLanguage for Language {
    fn globals() -> String {
        include_str!("./globals.ts").to_string()
    }

    /// Renders a collection of [`FunctionDataType`] into a TypeScript string.
    fn render_commands(
        commands: &[FunctionDataType],
        type_map: &TypeDefs,
        cfg: &ExportConfiguration,
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
                    SpectaFunctionResultVariant::Value(t) => {
                        ts::datatype(&cfg.inner, t, &type_map)?
                    }
                    SpectaFunctionResultVariant::Result(t, _) => {
                        ts::datatype(&cfg.inner, t, &type_map)?
                    }
                };

                let ret_type = match &function.result {
                    SpectaFunctionResultVariant::Value(t) => ok_type.clone(),
                    SpectaFunctionResultVariant::Result(t, e) => {
                        format!(
                            "[{ok_type}, undefined] | [undefined, {}]",
                            ts::datatype(&cfg.inner, e, &type_map)?
                        )
                    }
                };

                let body = {
                    let name = match (&cfg.plugin_name, cfg.plugin_prefix) {
                        (Some(plugin_name), true) => {
                            format!("plugin:tauri-specta-{plugin_name}|{}", function.name)
                        }
                        (None, true) => format!("plugin:tauri-specta|{}", function.name),
                        (Some(plugin_name), false) => {
                            format!("plugin:{plugin_name}|{}", function.name)
                        }
                        (None, false) => function.name.to_string(),
                    };

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
                        SpectaFunctionResultVariant::Value(_) => format!("return {invoke};"),
                        SpectaFunctionResultVariant::Result(_, _) => formatdoc!(
                            r#"
	                        try {{
	                            return [{invoke}, undefined];
	                        }} catch (e: any) {{
	                            if(e instanceof Error) throw e;
	                            else return [undefined, e];
	                        }}"#
                        ),
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
        events: &[EventMeta],
        type_map: &TypeDefs,
        cfg: &ExportConfiguration,
    ) -> Result<String, TsExportError> {
        if events.is_empty() {
            return Ok(Default::default());
        }

        let events = events
            .iter()
            .map(|event| {
                let name = event.name;
                let typ = ts::datatype(&cfg.inner, &event.typ, type_map)?;

                let name_camel = name.to_lower_camel_case();

                Ok(format!(r#"	{name_camel}: __makeEvent__<{typ}>("{name}")"#))
            })
            .collect::<Result<Vec<_>, TsExportError>>()?
            .join(",\n");

        Ok(formatdoc! {
            r#"
            export const events = {{
            {events}
            }}"#
        })
    }

    fn render(
        commands: &[FunctionDataType],
        events: &[EventMeta],
        type_map: &TypeDefs,
        cfg: &ExportConfiguration,
    ) -> Result<String, TsExportError> {
        let globals = Self::globals();

        let commands = Self::render_commands(commands, &type_map, cfg)?;
        let events = Self::render_events(events, &type_map, cfg)?;

        let dependant_types = type_map
            .values()
            .filter_map(|v| v.as_ref())
            .map(|v| ts::export_datatype(&cfg.inner, v, &type_map))
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.join("\n"))?;

        Ok(formatdoc! {
            r#"
	            {DO_NOT_EDIT}

				{globals}

				{commands}

				{events}

	            {dependant_types}
	        "#
        })
    }
}

/// The configuration for the generator
#[derive(Default)]
pub struct ExportConfiguration {
    /// The name of the plugin to invoke.
    ///
    /// If there is no plugin name (i.e. this is an app), this should be `None`.
    pub(crate) plugin_name: Option<Cow<'static, str>>,
    /// The specta export configuration
    pub(crate) inner: specta::ts::ExportConfiguration,
    pub(crate) plugin_prefix: bool,
}

impl ExportConfiguration {
    /// Creates a new [`ExportConfiguration`] from a [`specta::ts::ExportConfiguration`]
    pub fn new(config: specta::ts::ExportConfiguration) -> Self {
        Self {
            inner: config,
            ..Default::default()
        }
    }

    /// Sets the plugin name for this [`ExportConfiguration`].
    pub fn plugin_name(mut self, plugin_name: impl Into<Cow<'static, str>>) -> Self {
        self.plugin_name = Some(plugin_name.into());
        self
    }
}

impl From<specta::ts::ExportConfiguration> for ExportConfiguration {
    fn from(config: specta::ts::ExportConfiguration) -> Self {
        Self {
            inner: config,
            ..Default::default()
        }
    }
}
