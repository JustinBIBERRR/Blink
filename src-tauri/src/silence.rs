/// Windows fullscreen game and presentation detection.
/// Windowed meetings are controlled by the user's manual pause.
pub struct SilenceChecker;

impl SilenceChecker {
    pub fn new() -> Self {
        Self
    }

    pub fn is_silenced(&self) -> bool {
        self.check_notification_state()
    }

    #[cfg(target_os = "windows")]
    fn check_notification_state(&self) -> bool {
        use windows::Win32::UI::Shell::{
            SHQueryUserNotificationState, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
        };
        unsafe {
            match SHQueryUserNotificationState() {
                Ok(state) => {
                    state == QUNS_RUNNING_D3D_FULL_SCREEN || state == QUNS_PRESENTATION_MODE
                }
                Err(_) => false,
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn check_notification_state(&self) -> bool {
        false
    }
}

impl Default for SilenceChecker {
    fn default() -> Self {
        Self::new()
    }
}
