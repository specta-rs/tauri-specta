//! Consumer tests for the published Tauri v3 prerelease, without the workspace's v2 patch.

#[cfg(test)]
#[path = "../../support/semantic_channels.rs"]
mod semantic_channels;

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use serde::{Deserialize, Serialize};
    use specta::Type;
    use specta_typescript::{JSDoc, Typescript};
    use tauri::test::MockRuntime;
    use tauri_specta::{Builder, ErrorHandlingMode, Event, collect_commands, collect_events};
    use tauri_specta_query::{CommandSet, TanstackQueryFramework};

    #[derive(Clone, Debug, Deserialize, Serialize, Type, Event)]
    struct Progress {
        completed: u32,
    }

    #[tauri::command(rename = "greet-v3")]
    #[specta::specta]
    fn greet(name: String, state: tauri::State<'_, u32>) -> String {
        format!("Hello, {name}: {}", *state)
    }

    #[tauri::command]
    #[specta::specta]
    async fn double(value: u32) -> u32 {
        value * 2
    }

    #[tauri::command]
    #[specta::specta]
    fn fail() -> Result<String, String> {
        Err("failed".into())
    }

    #[tauri::command]
    #[specta::specta]
    fn stream(on_progress: tauri::ipc::Channel<u32>) {
        let _ = on_progress.send(42);
    }

    #[tauri::command]
    #[specta::specta]
    fn handle<R: tauri::Runtime>(_app: tauri::AppHandle<R>) {}

    fn builder() -> Builder<MockRuntime> {
        Builder::new()
            .commands(collect_commands![
                greet,
                double,
                fail,
                stream,
                handle::<MockRuntime>
            ])
            .events(collect_events![Progress])
    }

    fn app(builder: &Builder<MockRuntime>) -> tauri::App<MockRuntime> {
        let app = tauri::test::mock_builder()
            .manage(7_u32)
            .invoke_handler(builder.invoke_handler())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        builder.mount_events(&app);
        app
    }

    fn invoke(
        webview: &tauri::WebviewWindow<MockRuntime>,
        command: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, serde_json::Value> {
        tauri::test::get_ipc_response(
            webview,
            tauri::webview::InvokeRequest {
                cmd: command.into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                // V3's mock runtime uses this origin on every platform.
                url: "tauri://localhost".parse().unwrap(),
                body: tauri::ipc::InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.into(),
            },
        )
        .map(|body| body.deserialize().unwrap())
    }

    #[test]
    fn dispatches_renamed_async_and_failing_commands() {
        let app = app(&builder());
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();

        assert_eq!(
            invoke(&webview, "greet-v3", serde_json::json!({ "name": "Tauri" })),
            Ok(serde_json::json!("Hello, Tauri: 7"))
        );
        assert_eq!(
            invoke(&webview, "double", serde_json::json!({ "value": 21 })),
            Ok(serde_json::json!(42))
        );
        assert_eq!(
            invoke(&webview, "fail", serde_json::json!({})),
            Err(serde_json::json!("failed"))
        );
        assert_eq!(
            invoke(&webview, "handle", serde_json::json!({})),
            Ok(serde_json::Value::Null)
        );
        assert!(invoke(&webview, "greet", serde_json::json!({ "name": "Tauri" })).is_err());
    }

    #[test]
    fn mounts_listens_emits_and_unlistens_to_typed_events() {
        let app = app(&builder());
        let values = Arc::new(Mutex::new(Vec::new()));
        let received = values.clone();
        let id = Progress::listen(&app, move |event| {
            received.lock().unwrap().push(event.payload.completed);
        });
        Progress { completed: 42 }.emit(&app).unwrap();
        tauri::Listener::unlisten(&app, id);
        Progress { completed: 99 }.emit(&app).unwrap();
        assert_eq!(*values.lock().unwrap(), [42]);

        let received = values.clone();
        Progress::once(&app, move |event| {
            received.lock().unwrap().push(event.payload.completed);
        });
        Progress { completed: 1 }.emit(&app).unwrap();
        Progress { completed: 2 }.emit(&app).unwrap();
        assert_eq!(*values.lock().unwrap(), [42, 1]);
    }

    #[test]
    fn accepts_the_default_type_erased_runtime() {
        let builder = Builder::<tauri::DynRuntime>::new()
            .commands(collect_commands![greet, handle::<tauri::DynRuntime>]);
        // No concrete webview runtime is needed to collect commands or attach a handler.
        let _ = tauri::Builder::default().invoke_handler(builder.invoke_handler());
    }

    #[test]
    fn exports_bindings_for_frontend_checks() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/bindings");
        std::fs::create_dir_all(&dir).unwrap();

        let builder = builder().mocks();
        builder
            .export(Typescript::default(), dir.join("bindings.ts"))
            .unwrap();
        builder
            .export(JSDoc::default(), dir.join("bindings.js"))
            .unwrap();
        super::semantic_channels::builder()
            .export(Typescript::default(), dir.join("semantic.ts"))
            .unwrap();
        super::semantic_channels::builder()
            .export(JSDoc::default(), dir.join("semantic.js"))
            .unwrap();
        let bindings = std::fs::read_to_string(dir.join("bindings.ts")).unwrap();
        assert!(bindings.contains("\"greet-v3\""));
        assert!(bindings.contains("onProgress: Channel<number>"));
        assert!(!bindings.contains("state:"));

        for (name, mode) in [
            ("result", ErrorHandlingMode::Result),
            ("result-unknown", ErrorHandlingMode::ResultUnknown),
            ("data-error", ErrorHandlingMode::DataError),
        ] {
            builder
                .clone()
                .error_handling(mode)
                .export(Typescript::default(), dir.join(format!("{name}.ts")))
                .unwrap();
        }

        let (query, builder) =
            CommandSet::<MockRuntime>::new(collect_commands![greet], collect_commands![double])
                .build(TanstackQueryFramework::React);
        assert!(query.contains("commands.greet"));
        builder
            .export(Typescript::default().with_raw(query), dir.join("query.ts"))
            .unwrap();

        builder
            .clone()
            .plugin_name("demo")
            .export(Typescript::default(), dir.join("plugin.ts"))
            .unwrap();
        let bindings = std::fs::read_to_string(dir.join("plugin.ts")).unwrap();
        assert!(bindings.contains("plugin:demo|greet-v3"));
    }
}
