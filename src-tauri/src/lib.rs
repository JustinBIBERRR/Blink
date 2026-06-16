pub mod daemon;
pub mod db;
pub mod silence;
pub mod state;

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
