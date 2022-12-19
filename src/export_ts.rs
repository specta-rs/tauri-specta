use heck::ToLowerCamelCase;
use indoc::writedoc;
use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

use specta::{function::FunctionDataType, ts, TypeDefs};

pub fn export_to_ts(
    (function_types, type_map): (Vec<FunctionDataType>, TypeDefs),
    export_path: impl AsRef<Path>,
) -> Result<(), io::Error> {
    let export_path = PathBuf::from(export_path.as_ref());

    if let Some(export_dir) = export_path.parent() {
        fs::create_dir_all(export_dir)?;
    }

    let mut file = File::create(export_path)?;

    writedoc!(
        file,
        r#"
            // This file was generated by [tauri-specta](https://github.com/oscartbeaumont/tauri-specta). Do not edit this file manually.

            declare global {{
                interface Window {{
                    __TAURI_INVOKE__<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
                }}
            }}

            const invoke = window.__TAURI_INVOKE__;
        "#
    )?;

    for function in function_types {
        let name = &function.name;
        let name_camel = function.name.to_lower_camel_case();

        let arg_defs = function
            .args
            .iter()
            .map(|(name, typ)| format!("{}: {}", name.to_lower_camel_case(), ts::datatype(typ)))
            .collect::<Vec<_>>()
            .join(", ");

        let ret_type = ts::datatype(&function.result);

        let arg_usages = function
            .args
            .iter()
            .map(|(name, _)| name.to_lower_camel_case())
            .collect::<Vec<_>>();

        let arg_usages = arg_usages
            .is_empty()
            .then(Default::default)
            .unwrap_or_else(|| format!(", {{ {} }}", arg_usages.join(",")));

        writedoc!(
            file,
            r#"

                export function {name_camel}({arg_defs}) {{
                    return invoke<{ret_type}>("{name}"{arg_usages})
                }}
            "#
        )?;
    }

    for export in type_map
        .values()
        .filter_map(|v| ts::export_datatype(v).ok())
    {
        writeln!(file, "\n{export}")?;
    }

    Ok(())
}
