
#[tauri::command]
#[specta::specta]
pub fn get_library() {
	println!("get_library called");
}
