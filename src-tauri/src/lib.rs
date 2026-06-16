pub mod daemon;
pub mod db;
pub mod hotkey;
pub mod silence;
pub mod startup;
pub mod state;
pub mod tray;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            tray::setup_tray(&app.handle())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
