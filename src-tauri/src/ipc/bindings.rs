use std::path::Path;

use heck::ToLowerCamelCase;
use specta::datatype::FunctionResultVariant;
use specta_typescript::{BigIntExportBehavior, ExportError, Typescript};
use tauri_specta::{ExportContext, LanguageExt};

pub(super) struct FrontendBindings;

impl LanguageExt for FrontendBindings {
    type Error = ExportError;

    fn render(&self, contract: &ExportContext) -> Result<String, ExportError> {
        // Numeric settings are range-validated; durations and counters are bounded.
        // Revision strings retain full u64 precision.
        let ts = Typescript::default().bigint(BigIntExportBehavior::Number);
        let declarations = ts.export(&contract.type_map)?;

        let commands = contract
            .commands
            .iter()
            .map(|command| {
                let mut parameters = Vec::new();
                let mut arguments = Vec::new();
                for (name, datatype) in command.args() {
                    let name = name.to_lower_camel_case();
                    let datatype = specta_typescript::datatype(
                        &ts,
                        &FunctionResultVariant::Value(datatype.clone()),
                        &contract.type_map,
                    )?;
                    parameters.push(format!("{name}: {datatype}"));
                    arguments.push(name);
                }
                let result = match command.result() {
                    Some(
                        FunctionResultVariant::Result(value, _)
                        | FunctionResultVariant::Value(value),
                    ) => specta_typescript::datatype(
                        &ts,
                        &FunctionResultVariant::Value(value.clone()),
                        &contract.type_map,
                    )?,
                    None => "void".to_string(),
                };
                let name = command.name();
                let arguments = if arguments.is_empty() {
                    String::new()
                } else {
                    format!(", {{ {} }}", arguments.join(", "))
                };
                Ok(format!(
                    "{}: ({}): Promise<{result}> => invoke<{result}>(\"{name}\"{arguments})",
                    name.to_lower_camel_case(),
                    parameters.join(", ")
                ))
            })
            .collect::<Result<Vec<_>, ExportError>>()?;

        let events = contract.events.iter().map(|(name, datatype)| {
            let datatype = specta_typescript::datatype(&ts, &FunctionResultVariant::Value(datatype.clone()), &contract.type_map)?;
            Ok(format!("{}: {{ listen: (callback: EventCallback<{datatype}>): Promise<UnlistenFn> => listen<{datatype}>(\"{name}\", callback) }}",
                name.to_lower_camel_case()))
        }).collect::<Result<Vec<_>, ExportError>>()?;

        Ok(format!(
            "// Generated from the Rust IPC contract.\nimport {{ invoke }} from '@tauri-apps/api/core';\nimport {{ listen, type EventCallback, type UnlistenFn }} from '@tauri-apps/api/event';\n\n{}\n\nexport const commands = {{\n{}\n}};\n\nexport const events = {{\n{}\n}};\n",
            declarations, commands.join(",\n"), events.join(",\n"),
        ))
    }

    fn format(&self, _path: &Path) -> Result<(), ExportError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    mod first {
        #[derive(specta::Type)]
        pub(super) struct Collision;
    }
    mod second {
        #[derive(specta::Type)]
        pub(super) struct Collision;
    }

    #[test]
    fn conflicting_type_names_fail_generation() {
        let result = tauri_specta::Builder::<tauri::Wry>::new()
            .typ::<first::Collision>()
            .typ::<second::Collision>()
            .export_str(super::FrontendBindings);
        assert!(matches!(
            result,
            Err(specta_typescript::ExportError::DuplicateTypeName(..))
        ));
    }

    #[test]
    fn export_contract() {
        let contract = super::super::builder::<tauri::Wry>()
            .export_str(super::FrontendBindings)
            .expect("IPC contract should export");
        if let Some(path) = std::env::var_os("POMOLICE_IPC_EXPORT") {
            std::fs::write(path, contract).expect("IPC contract should be written");
        }
    }
}
