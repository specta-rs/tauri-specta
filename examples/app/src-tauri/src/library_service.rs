use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use souchy_tauri_specta::*;
use specta::Type;
use specta_typescript::Typescript;
use tauri::{AppHandle, Runtime, State};
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

pub struct MyStruct {
    pub field: String,
}

impl MyStruct {
    pub fn new(field: String) -> Self {
        Self { field }
    }
    /// On both attributes:
    /// 	macro definition is not supported in `trait`s or `impl`s
    /// 	consider moving the macro definition out to a nearby module scope
    /// 	`use` import is not supported in `trait`s or `impl`s
    /// 	consider moving the `use` import out to a nearby module scope
    // #[tauri::command]
    // #[specta::specta]
    fn my_method(this: &MyStruct) -> String {
        "Hello from MyStruct!".into()
    }
}

// #[souchy::class]
pub mod blue_struct {
    use super::*;

    #[derive(Clone, Eq, Hash, PartialEq, Deserialize, Serialize, Type, Debug)]
    pub struct Id(String);
    #[derive(Default)]
    pub struct BlueStructInstances(pub RwLock<HashMap<Id, BlueStruct>>);
    #[derive(Clone, Default)]
    pub struct BlueStruct {
        pub some_field: String,
        some_private_field: String,
    }

    /// `constructor` or `new`
    /// Actually should be static method that returns an instance and make the ctor private, so call it instance.
    ///
    /// We can either pass the BlueStruct instance around, or use a State to store instances.
    /// If we use multiple instances in State, we need a way to identify/key them.
    /// Dont really want to mix both, because that's 2 sources of truth for the struct's data.
    /// Issue with passing instance, is doing stuff here in rust doesnt affect the instance in TS.
    /// So State it is.
    /// Some classes may be singletons, other will have keys.
    /// The constructor can return the key when needed.
    /// And generate getters/setters for the other fields.
    #[tauri::command]
    #[specta::specta]
    pub fn instance(blue_instances: State<'_, BlueStructInstances>, some_field: String) -> Id {
        let id = Id("uuid::Uuid::new_v4()".to_string());
        let blue = BlueStruct {
            some_field,
            some_private_field: "default".into(),
        };
        blue_instances
            .0
            .blocking_write()
            .insert(id.clone(), blue.clone());
        id
    }

    /// Now we can ignore State and Id parameters in the TS function.
    /// The class will hold the Id and pass it to the .invoke().
    #[tauri::command]
    #[specta::specta]
    pub async fn get_field(
        blue_instances: State<'_, BlueStructInstances>,
        struct_id: Id,
    ) -> Result<String, String> {
        let instances = blue_instances.0.read().await;
        match instances.get(&struct_id) {
            Some(db) => Ok(db.some_field.clone()),
            None => Err("BlueStruct not found!".into()),
        }
    }

    #[tauri::command]
    #[specta::specta]
    pub async fn set_field(
        blue_instances: State<'_, BlueStructInstances>,
        struct_id: Id,
        value: String,
    ) -> Result<String, String> {
        let mut instances = blue_instances.0.write().await;
        match instances.get_mut(&struct_id) {
            Some(instance) => {
                instance.some_field = value;
                Ok("Updated BlueStruct!".into())
            }
            None => Err("BlueStruct not found!".into()),
        }
    }

    // #[tauri::command]
    // #[specta::specta]
    // pub async fn blue_struct_get_struct(
    //     blue_instances: State<'_, BlueStructInstances>,
    //     struct_id: Id,
    // ) -> Result<BlueStruct, String> {
    //     let instances = blue_instances.0.read().await;
    //     match instances.get(&struct_id) {
    //         Some(db) => Ok(db.clone()),
    //         None => Err("Db not loaded".to_string()),
    //     }
    // }
}
