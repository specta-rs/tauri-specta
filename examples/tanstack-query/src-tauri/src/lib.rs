//! Example Tauri application pairing [`tauri_specta_query`] with TanStack Query.
//!
//! `CommandSet` splits the commands into queries and mutations, generates the frontend
//! helpers for both, and hands back the Tauri Specta builder for everything else.

use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Typescript;
use tauri_specta::Event;
use tauri_specta_query::{CommandSet, TanstackQueryFramework};

/// A user in the example's in-memory store.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct User {
    pub id: u32,
    pub name: String,
}

/// The store [`list_users`] reads and [`create_user`] writes.
#[derive(Default)]
pub struct Users(Mutex<Vec<User>>);

impl Users {
    fn lock(&self) -> MutexGuard<'_, Vec<User>> {
        self.0.lock().expect("the users lock is poisoned")
    }
}

/// Emitted by [`create_user`]. Exported to the frontend as `events.userCreated`.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct UserCreated {
    pub user: User,
}

/// A query command: reachable as `queries.listUsers()` on the frontend.
#[tauri::command]
#[specta::specta]
fn list_users(users: tauri::State<'_, Users>) -> Vec<User> {
    users.lock().clone()
}

/// A mutation command: reachable as `mutations.createUser()`. Its arguments are passed to
/// `mutate` as one object.
#[tauri::command]
#[specta::specta]
fn create_user(app: tauri::AppHandle, users: tauri::State<'_, Users>, name: String) -> User {
    let user = {
        let mut users = users.lock();

        let user = User {
            id: users.len() as u32 + 1,
            name,
        };
        users.push(user.clone());

        user
    };

    UserCreated { user: user.clone() }.emit(&app).ok();

    user
}

/// Splits the commands into the query set and the mutation set.
fn command_set() -> CommandSet<tauri::Wry> {
    CommandSet::new(
        tauri_specta::collect_commands![list_users],
        tauri_specta::collect_commands![create_user],
    )
    .events(tauri_specta::collect_events![UserCreated])
}

/// Builds the query bindings and the Tauri Specta builder.
pub fn builder() -> (String, tauri_specta::Builder<tauri::Wry>) {
    let (query_bindings, builder) = command_set().build(TanstackQueryFramework::React);

    let builder = builder
        // We recommend returning errors as a value rather than throwing, so a failed
        // command resolves to `{ status: "error"; error: E }` and the frontend can
        // handle every case exhaustively. The default just rejects the promise.
        .error_handling(tauri_specta::ErrorHandlingMode::Result);

    (query_bindings, builder)
}

/// Exports the bindings into `dir`. The generated query helpers go in through `with_raw`.
pub fn export_bindings(
    builder: &tauri_specta::Builder<tauri::Wry>,
    query_bindings: &str,
    dir: &std::path::Path,
) {
    builder
        .export(
            Typescript::default().with_raw(query_bindings.to_owned()),
            dir.join("bindings.ts"),
        )
        .expect("failed to export the TypeScript bindings");
}

/// Runs the example application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (query_bindings, builder) = builder();

    #[cfg(debug_assertions)] // Only export the bindings on development builds.
    export_bindings(&builder, &query_bindings, std::path::Path::new("../src"));

    tauri::Builder::default()
        .manage(Users::default())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn export_bindings_to_disk() {
        let (query_bindings, builder) = builder();
        export_bindings(&builder, &query_bindings, std::path::Path::new("../src"));
    }
}
