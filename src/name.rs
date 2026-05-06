use std::borrow::Cow;

pub(crate) fn resolve_tauri_command_name<'a>(
    plugin_name: Option<&'a str>,
    name: &'a str,
) -> Cow<'a, str> {
    resolve_tauri_name(plugin_name, name, TauriNameType::Command)
}

pub(crate) fn resolve_tauri_event_name<'a>(
    plugin_name: Option<&'a str>,
    name: &'a str,
) -> Cow<'a, str> {
    resolve_tauri_name(plugin_name, name, TauriNameType::Event)
}
enum TauriNameType {
    Event,
    Command,
}

fn resolve_tauri_name<'a>(
    plugin_name: Option<&'a str>,
    name: &'a str,
    name_type: TauriNameType,
) -> Cow<'a, str> {
    match plugin_name {
        Some(p) => resolve_tauri_plugin_name(p, name, name_type),
        None => Cow::Borrowed(name),
    }
}

fn resolve_tauri_plugin_name(
    plugin_name: &str,
    name: &str,
    name_type: TauriNameType,
) -> Cow<'static, str> {
    let delimiter = match name_type {
        TauriNameType::Event => ":",
        TauriNameType::Command => "|",
    };
    Cow::Owned(format!("plugin:{plugin_name}{delimiter}{name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_tauri_command_name() {
        assert_eq!(resolve_tauri_command_name(None, "my_command"), "my_command");
        assert_eq!(
            resolve_tauri_command_name(Some("my_plugin"), "my_command"),
            "plugin:my_plugin|my_command"
        );
    }

    #[test]
    fn test_resolve_tauri_event_name() {
        assert_eq!(resolve_tauri_event_name(None, "my_event"), "my_event");
        assert_eq!(
            resolve_tauri_event_name(Some("my_plugin"), "my_event"),
            "plugin:my_plugin:my_event"
        );
    }
}
