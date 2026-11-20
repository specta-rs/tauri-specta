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

/// Whether Tauri accepts `name` as an event name.
///
/// Tauri's event system only accepts alphanumeric characters, `-`, `/`, `:` and
/// `_`. Anything else is rejected when the event is first used: `emit` returns
/// an error and `listen`/`once` panic, both far from the configuration that
/// produced the name.
pub(crate) fn is_valid_tauri_event_name(name: &str) -> bool {
    name.chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '-' | '/' | ':' | '_'))
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

    #[test]
    fn tauri_event_names() {
        for name in [
            "my-event",
            "plugin:kitchen-sink:message-event",
            "tauri://resize",
            "events/one_two",
            "café",
        ] {
            assert!(is_valid_tauri_event_name(name), "{name}");
        }

        for name in ["my event", "my.event", "r#type", "event?"] {
            assert!(!is_valid_tauri_event_name(name), "{name}");
        }

        // Tauri's rule is a per-character check, so an empty name passes it.
        assert!(is_valid_tauri_event_name(""));
    }
}
