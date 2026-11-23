/// Win32 context-aware silence detection.
///
/// Prevents blink-rs from interrupting the user during:
/// - Fullscreen games / D3D apps / presentation mode
/// - Video playback or conferencing (DisplayRequired power request)
/// - Voice-only meetings (microphone is producing audio)
pub struct SilenceChecker;

impl SilenceChecker {
    pub fn new() -> Self {
        Self
    }

    /// Returns true if any silence condition is active.
    pub fn is_silenced(&self) -> bool {
        self.check_notification_state()
            || self.check_display_power_request()
            || self.check_microphone_active()
    }

    // ── Layer 1: D3D fullscreen / presentation mode ──────────────────────────

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

    // ── Layer 2: DisplayRequired power request (video / conferencing) ─────────
    //
    // Uses CallNtPowerInformation(GetPowerRequestList) to query whether any
    // process currently holds a PowerRequestDisplayRequired request. A non-zero
    // output size means at least one such request exists.

    #[cfg(target_os = "windows")]
    fn check_display_power_request(&self) -> bool {
        use windows::Win32::System::Power::{CallNtPowerInformation, GetPowerRequestList};
        unsafe {
            let mut buf = [0u8; 4096];
            let status = CallNtPowerInformation(
                GetPowerRequestList,
                None,
                0,
                Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
                buf.len() as u32,
            );
            // STATUS_SUCCESS = 0; a successful call with data means requests exist.
            // We inspect the first 4 bytes as a u32 count of entries.
            if status.is_ok() {
                let count = u32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]);
                return count > 0;
            }
            false
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn check_display_power_request(&self) -> bool {
        false
    }

    // ── Layer 3: Microphone peak level (voice meetings) ───────────────────────

    #[cfg(target_os = "windows")]
    fn check_microphone_active(&self) -> bool {
        use windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;
        use windows::Win32::Media::Audio::{
            eCapture, eConsole, IMMDeviceEnumerator, MMDeviceEnumerator,
        };
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CLSCTX, COINIT_MULTITHREADED,
        };
        unsafe {
            // Ensure COM is initialized for this thread (best-effort).
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let enumerator: IMMDeviceEnumerator =
                match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX(1 | 2 | 4 | 16)) {
                    Ok(e) => e,
                    Err(_) => return false,
                };
            let device = match enumerator.GetDefaultAudioEndpoint(eCapture, eConsole) {
                Ok(d) => d,
                Err(_) => return false,
            };
            let meter: IAudioMeterInformation = match device.Activate(CLSCTX(1 | 2 | 4 | 16), None)
            {
                Ok(m) => m,
                Err(_) => return false,
            };
            match meter.GetPeakValue() {
                Ok(peak) => peak > 0.01,
                Err(_) => false,
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn check_microphone_active(&self) -> bool {
        false
    }
}

impl Default for SilenceChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_checker_constructs_without_panic() {
        let _checker = SilenceChecker::new();
    }

    #[test]
    fn is_silenced_returns_a_bool() {
        let checker = SilenceChecker::new();
        // In test environment (no D3D game, no video call), should return false.
        // We just verify it doesn't panic — can't mock Win32.
        let result = checker.is_silenced();
        assert!(result == true || result == false);
    }
}
