//! Best-effort native delivery. OS authorization/DND never governs the reminder.
pub fn submit(app: &tauri::AppHandle, body: &'static str) {
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        if let Ok(body) = std::ffi::CString::new(body) {
            unsafe {
                blink_silent_notification(body.as_ptr());
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        let app_id = app.config().identifier.clone();
        std::thread::spawn(move || {
            if let Err(error) = windows_submit(&app_id, body) {
                log::warn!("Silent notification unavailable: {error}");
            }
        });
    }
}

#[cfg(target_os = "macos")]
extern "C" {
    fn blink_silent_notification(body: *const std::ffi::c_char);
}

#[cfg(target_os = "windows")]
fn windows_submit(app_id: &str, body: &str) -> windows::core::Result<()> {
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
    use windows::{
        core::HSTRING,
        Data::Xml::Dom::XmlDocument,
        UI::Notifications::{ToastNotification, ToastNotificationManager},
    };
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
    }
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    let _apartment = Apartment;
    // Both messages are application constants, not user-controlled XML.
    let xml = XmlDocument::new()?;
    xml.LoadXml(&HSTRING::from(format!(
        "<toast><visual><binding template=\"ToastGeneric\"><text>Blink</text><text>{body}</text></binding></visual><audio silent=\"true\"/></toast>"
    )))?;
    let notification = ToastNotification::CreateToastNotification(&xml)?;
    ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(app_id))?.Show(&notification)
}
