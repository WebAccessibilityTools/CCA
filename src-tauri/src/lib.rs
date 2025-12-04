// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[cfg(target_os = "macos")]
mod macos;
mod config;

#[derive(Clone, serde::Serialize)]
pub struct ColorResult {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub hex: String,
}

#[tauri::command]
fn pick_color() -> Option<ColorResult> {
    #[cfg(target_os = "macos")]
    {
        // Le picker doit s'exécuter sur le thread principal
        macos::run().map(|(r, g, b)| ColorResult {
            r,
            g,
            b,
            hex: format!("#{:02X}{:02X}{:02X}", r, g, b),
        })
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![pick_color])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
