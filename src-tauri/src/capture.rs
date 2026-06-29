//! One-shot desktop screen capture for the overlay's frozen-snapshot blur.
//!
//! Why this exists: a transparent WebView2 overlay window cannot blur the OS
//! desktop behind it (`backdrop-filter` only sees in-page DOM). To render a real
//! Gaussian blur of the user's screen, we capture a single frame of the desktop
//! the instant the break begins (before the overlay is shown) and hand it to the
//! overlay as a data URL. The overlay blurs *those pixels* behind a radial mask.
//!
//! Implementation: GDI `StretchBlt` (downscaled — the result is heavily blurred
//! anyway) + `GetDIBits` into a top-down 32bpp buffer, wrapped in a BMP container
//! and base64-encoded. Uses the `windows` GDI bindings already in the project, so
//! no heavy screen-capture crate is pulled in.

/// Capture the primary monitor and return it as a `data:image/bmp;base64,...`
/// URL. `scale` downsamples each axis (e.g. 2 => half resolution); the snapshot
/// is only ever shown blurred, so 2 is plenty and keeps the payload small.
#[cfg(target_os = "windows")]
pub fn capture_primary_screen_data_url(scale: i32) -> Result<String, String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
        ReleaseDC, SelectObject, SetStretchBltMode, StretchBlt, BITMAPINFO, BITMAPINFOHEADER,
        BI_RGB, DIB_RGB_COLORS, HALFTONE, HGDIOBJ, SRCCOPY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

    let null_hwnd = HWND(std::ptr::null_mut());

    unsafe {
        let sw = GetSystemMetrics(SM_CXSCREEN);
        let sh = GetSystemMetrics(SM_CYSCREEN);
        if sw <= 0 || sh <= 0 {
            return Err("invalid screen metrics".into());
        }
        let scale = scale.max(1);
        let cw = (sw / scale).max(1);
        let ch = (sh / scale).max(1);

        let hdc_screen = GetDC(null_hwnd);
        if hdc_screen.is_invalid() {
            return Err("GetDC failed".into());
        }
        let hdc_mem = CreateCompatibleDC(hdc_screen);
        let hbmp = CreateCompatibleBitmap(hdc_screen, cw, ch);
        let old = SelectObject(hdc_mem, HGDIOBJ(hbmp.0));

        SetStretchBltMode(hdc_mem, HALFTONE);
        let blt_ok = StretchBlt(
            hdc_mem, 0, 0, cw, ch, hdc_screen, 0, 0, sw, sh, SRCCOPY,
        )
        .as_bool();

        // Top-down (negative height) 32bpp so rows come out in natural order.
        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = cw;
        bmi.bmiHeader.biHeight = -ch;
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0;

        let stride = cw as usize * 4;
        let data_len = stride * ch as usize;
        let mut pixels = vec![0u8; data_len];

        let scanned = GetDIBits(
            hdc_mem,
            hbmp,
            0,
            ch as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        // Release GDI resources regardless of outcome.
        SelectObject(hdc_mem, old);
        let _ = DeleteObject(HGDIOBJ(hbmp.0));
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(null_hwnd, hdc_screen);

        if !blt_ok || scanned == 0 {
            return Err("screen blit/copy failed".into());
        }

        // GDI leaves the alpha byte 0; force opaque so the BMP decoder doesn't
        // treat the frame as transparent.
        for px in pixels.chunks_exact_mut(4) {
            px[3] = 0xFF;
        }

        Ok(format!("data:image/bmp;base64,{}", encode_bmp(cw, ch, &pixels)))
    }
}

/// Wrap a top-down 32bpp BGRA buffer in a BMP container and base64-encode it.
#[cfg(target_os = "windows")]
fn encode_bmp(w: i32, h: i32, pixels: &[u8]) -> String {
    use base64::Engine;
    let data_len = pixels.len();
    let file_size = 14 + 40 + data_len;
    let mut bmp = Vec::with_capacity(file_size);
    // BITMAPFILEHEADER (14 bytes)
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&(file_size as u32).to_le_bytes());
    bmp.extend_from_slice(&0u16.to_le_bytes()); // reserved1
    bmp.extend_from_slice(&0u16.to_le_bytes()); // reserved2
    bmp.extend_from_slice(&54u32.to_le_bytes()); // pixel data offset
    // BITMAPINFOHEADER (40 bytes)
    bmp.extend_from_slice(&40u32.to_le_bytes());
    bmp.extend_from_slice(&w.to_le_bytes());
    bmp.extend_from_slice(&(-h).to_le_bytes()); // top-down
    bmp.extend_from_slice(&1u16.to_le_bytes());
    bmp.extend_from_slice(&32u16.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    bmp.extend_from_slice(&(data_len as u32).to_le_bytes());
    bmp.extend_from_slice(&0i32.to_le_bytes());
    bmp.extend_from_slice(&0i32.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(pixels);

    base64::engine::general_purpose::STANDARD.encode(&bmp)
}

/// Mark a window as excluded from screen capture (Win10 2004+). The periodic
/// re-capture for the live blur would otherwise capture our own blurred overlay,
/// creating a feedback loop; excluding the overlay makes the capture see only the
/// desktop behind it. Does not affect what the user sees on screen.
#[cfg(target_os = "windows")]
pub fn exclude_from_capture(hwnd_ptr: *mut core::ffi::c_void) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
    };
    unsafe {
        SetWindowDisplayAffinity(HWND(hwnd_ptr), WDA_EXCLUDEFROMCAPTURE).map_err(|e| e.to_string())
    }
}

/// Non-Windows stub so the crate still compiles elsewhere.
#[cfg(not(target_os = "windows"))]
pub fn capture_primary_screen_data_url(_scale: i32) -> Result<String, String> {
    Err("screen capture only implemented on Windows".into())
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use base64::Engine;

    /// Manual smoke test: captures the live desktop and writes the decoded BMP to
    /// the repo's .designref folder for visual inspection. Ignored by default
    /// (needs an interactive desktop session).
    /// Run with: `cargo test -- --ignored capture_writes_bmp --nocapture`
    #[test]
    #[ignore]
    fn capture_writes_bmp() {
        let url = capture_primary_screen_data_url(2).expect("capture failed");
        assert!(url.starts_with("data:image/bmp;base64,"));
        let b64 = url.trim_start_matches("data:image/bmp;base64,");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .expect("bad base64");
        assert_eq!(&bytes[0..2], b"BM", "missing BMP magic");
        assert!(bytes.len() > 54 + 1000, "suspiciously small capture");
        let out = r"D:\claudeCodePorjects\BLINK\.designref\capture_test.bmp";
        std::fs::write(out, &bytes).expect("write failed");
        println!("wrote {} bytes to {out}", bytes.len());
    }
}
