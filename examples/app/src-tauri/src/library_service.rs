use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Typescript;
use tauri::{AppHandle, Runtime, State};
use tauri_specta::*;
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};

#[derive(Default)]
pub struct DbInstances(pub RwLock<HashMap<String, String>>);

#[tauri::command]
#[specta::specta]
pub fn get_library() {
    println!("get_library called");
}

#[tauri::command]
#[specta::specta]
pub fn hello_app<T: Runtime>(app: AppHandle<T>) -> Result<String, String> {
    let id = app.config().identifier.clone();
    Ok(id)
}

/// type annotations needed
/// cannot infer type
/// TODO: Add an attribute to add generics to Typescript's .invoke<T>('my_command')
// #[tauri::command]
// #[specta::specta]
pub fn hello_generic<T>(opt: Option<T>) -> Result<String, String> {
	match opt {
		Some(opt) => Ok("Got opt".to_string()),
		None => Err("No opt provided".to_string()),
	}
}

/// Execute a command against the database
#[tauri::command]
#[specta::specta]
pub async fn add_db(db_instances: State<'_, DbInstances>, db: String) -> Result<String, String> {
    let inserted = db_instances.0.write().await.insert(db.clone(), db.clone());
    match inserted {
        Some(_) => Err("Db already loaded".to_string()),
        None => Ok(db),
    }
}

/// Execute a command against the database
#[tauri::command]
#[specta::specta]
pub async fn get_db(db_instances: State<'_, DbInstances>, db: String) -> Result<String, String> {
    let instances = db_instances.0.read().await;
    match instances.get(&db) {
        Some(db) => Ok(db.to_string()),
        None => Err("Db not loaded".to_string()),
    }
}

pub trait MyTrait {
    /// On '&self':
    /// 	Functions with `#[specta]` cannot take 'self'
    /// On `#[tauri::command]`:
    /// 	macro definition is not supported in `trait`s or `impl`s
    /// 	consider moving the macro definition out to a nearby module scope
    /// TODO: Add an attribute to impl functions in TS classes (a rust struct).
    /// 	Pass the class instance (`this`) to the invoke as the first argument.
    // #[tauri::command]
    // #[specta::specta]
    fn my_method(&self) -> String {
        "Hello from MyTrait!".into()
    }
}
