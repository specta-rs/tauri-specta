const COMMANDS: &[&str] = &["add_numbers"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
