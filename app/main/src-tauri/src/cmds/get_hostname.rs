#[tauri::command]
pub fn get_hostname() -> String {
    rqs_lib::get_hostname()
}
