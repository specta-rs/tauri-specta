use std::borrow::Cow;

pub(crate) fn resolve_tauri_event_name<'a>(
    plugin_name: Option<&'a str>,
    name: &'a str,
) -> Cow<'a, str> {
    match plugin_name {
        Some(p) => resolve_tauri_plugin_event_name(p, name),
        None => Cow::Borrowed(name),
    }
}

fn resolve_tauri_plugin_event_name(plugin_name: &str, name: &str) -> Cow<'static, str> {
    Cow::Owned(format!("plugin:{plugin_name}:{name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_tauri_event_name() {
        assert_eq!(resolve_tauri_event_name(None, "my_event"), "my_event");
        assert_eq!(
            resolve_tauri_event_name(Some("my_plugin"), "my_event"),
            "plugin:my_plugin:my_event"
        );
    }
}
