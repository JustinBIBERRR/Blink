pub fn set_auto_start(enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    {
        set_auto_start_windows(enabled)?;
    }
    #[cfg(not(target_os = "windows"))]
    let _ = enabled;
    Ok(())
}

pub fn is_auto_start_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        return is_auto_start_enabled_windows();
    }
    #[cfg(not(target_os = "windows"))]
    false
}

#[cfg(target_os = "windows")]
fn set_auto_start_windows(enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::*;

    let key_path = encode_wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    let name = encode_wide("Blink");

    unsafe {
        let mut hkey = HKEY::default();
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_path.as_ptr()),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        )
        .ok()?;

        if enabled {
            let exe_path = std::env::current_exe()?;
            let value = encode_wide(exe_path.to_str().unwrap_or("blink-rs.exe"));
            RegSetValueExW(
                hkey,
                PCWSTR(name.as_ptr()),
                0,
                REG_SZ,
                Some(std::slice::from_raw_parts(
                    value.as_ptr() as *const u8,
                    value.len() * 2,
                )),
            )
            .ok()?;
        } else {
            // Best-effort delete — ignore if key doesn't exist
            let _ = RegDeleteValueW(hkey, PCWSTR(name.as_ptr()));
        }
        RegCloseKey(hkey).ok()?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn is_auto_start_enabled_windows() -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::*;

    let key_path = encode_wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    let name = encode_wide("Blink");

    unsafe {
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_path.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        )
        .is_err()
        {
            return false;
        }
        let mut buf = [0u8; 512];
        let mut len = buf.len() as u32;
        let exists = RegQueryValueExW(
            hkey,
            PCWSTR(name.as_ptr()),
            None,
            None,
            Some(buf.as_mut_ptr()),
            Some(&mut len),
        )
        .is_ok();
        let _ = RegCloseKey(hkey);
        exists
    }
}

#[cfg(target_os = "windows")]
fn encode_wide(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "windows")]
    #[test]
    fn auto_start_toggle_writes_and_removes_registry_key() {
        // Write it
        set_auto_start(true).unwrap();
        assert!(is_auto_start_enabled());
        // Remove it
        set_auto_start(false).unwrap();
        assert!(!is_auto_start_enabled());
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn auto_start_noop_on_non_windows() {
        set_auto_start(true).unwrap();
        assert!(!is_auto_start_enabled()); // Always false on non-Windows
    }
}
