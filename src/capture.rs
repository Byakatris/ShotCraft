use std::borrow::Cow;
use std::fs::{self, File, OpenOptions};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::thread::JoinHandle;

use arboard::{Clipboard, ImageData};
use chrono::Local;
use egui::{Pos2, Rect, Vec2};
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ImageEncoder, RgbaImage};
use xcap::{Monitor, Window};

use crate::diagnostics;

static PENDING_SAVES: Mutex<Vec<JoinHandle<()>>> = Mutex::new(Vec::new());

pub struct CapturedScreen {
    pub image: RgbaImage,
    pub x: i32,
    pub y: i32,
    pub logical_width: f32,
    pub logical_height: f32,
    pub scale_factor: f32,
    pub physical_width: u32,
    pub physical_height: u32,
    pub physical_coords: bool,
}

#[derive(Debug, Clone)]
pub struct WindowCandidate {
    pub rect: Rect,
    pub label: String,
}

/// Pure helper computing logical/physical screen dimensions and coordinate convention.
pub fn compute_screen_metrics(
    raw_w: u32,
    raw_h: u32,
    img_w: u32,
    img_h: u32,
    reported_scale: f32,
    native_physical_coords: bool,
) -> (f32, f32, f32, u32, u32, bool) {
    let raw_wf = raw_w.max(1) as f32;
    let raw_hf = raw_h.max(1) as f32;
    let rep_scale = reported_scale.max(1.0);

    // macOS reports logical points in Monitor::width/height while captured image is physical pixels.
    // Windows and Linux X11 report physical pixels in both Monitor::width/height and captured image.
    if raw_w > 0 && (img_w as f32) > raw_wf * 1.05 {
        let scale = (img_w as f32 / raw_wf).max(1.0);
        (raw_wf, raw_hf, scale, img_w.max(1), img_h.max(1), false)
    } else {
        let phys_w = raw_w.max(img_w).max(1);
        let phys_h = raw_h.max(img_h).max(1);
        (
            phys_w as f32 / rep_scale,
            phys_h as f32 / rep_scale,
            rep_scale,
            phys_w,
            phys_h,
            native_physical_coords,
        )
    }
}

/// Get the current global mouse cursor position on macOS, Windows, or Linux (X11/GDK).
fn cursor_position() -> Option<(i32, i32)> {
    #[cfg(target_os = "macos")]
    {
        #[repr(C)]
        #[derive(Clone, Copy)]
        struct CGPoint {
            x: f64,
            y: f64,
        }
        unsafe extern "C" {
            fn CGEventCreate(source: *const std::ffi::c_void) -> *const std::ffi::c_void;
            fn CGEventGetLocation(event: *const std::ffi::c_void) -> CGPoint;
            fn CFRelease(cf: *const std::ffi::c_void);
        }
        unsafe {
            let ev = CGEventCreate(std::ptr::null());
            if ev.is_null() {
                return None;
            }
            let pt = CGEventGetLocation(ev);
            CFRelease(ev);
            Some((pt.x.round() as i32, pt.y.round() as i32))
        }
    }

    #[cfg(target_os = "windows")]
    {
        #[repr(C)]
        struct Point {
            x: i32,
            y: i32,
        }
        unsafe extern "system" {
            fn GetCursorPos(lp_point: *mut Point) -> i32;
        }
        let mut pt = Point { x: 0, y: 0 };
        let ok = unsafe { GetCursorPos(&mut pt) };
        if ok != 0 {
            Some((pt.x, pt.y))
        } else {
            None
        }
    }

    #[cfg(target_os = "linux")]
    {
        if std::env::var("XDG_SESSION_TYPE").ok().as_deref() == Some("wayland") {
            return None;
        }
        use gtk::gdk::prelude::{DeviceExt, SeatExt};
        let display = gtk::gdk::Display::default()?;
        let seat = display.default_seat()?;
        let pointer = seat.pointer()?;
        let (_, x, y) = pointer.position();
        Some((x, y))
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        None
    }
}

/// Capture the monitor currently under the mouse cursor (falling back to primary monitor).
pub fn capture_primary_screen() -> Result<CapturedScreen, String> {
    let monitor = cursor_position()
        .and_then(|(cx, cy)| Monitor::from_point(cx, cy).ok())
        .or_else(|| {
            let monitors = Monitor::all().ok()?;
            monitors
                .iter()
                .find(|m| m.is_primary().unwrap_or(false))
                .cloned()
                .or_else(|| monitors.into_iter().next())
        })
        .ok_or_else(|| "No monitors found".to_string())?;

    let x = monitor.x().map_err(|e| e.to_string())?;
    let y = monitor.y().map_err(|e| e.to_string())?;
    let raw_w = monitor.width().map_err(|e| e.to_string())?;
    let raw_h = monitor.height().map_err(|e| e.to_string())?;
    let reported_scale = monitor.scale_factor().unwrap_or(1.0).max(1.0);

    let img = monitor
        .capture_image()
        .map_err(|e| format!("Screen capture failed (check Screen Recording permission): {e}"))?;

    let native_physical = cfg!(any(target_os = "windows", target_os = "linux"));
    let (logical_width, logical_height, scale_factor, physical_width, physical_height, physical_coords) =
        compute_screen_metrics(raw_w, raw_h, img.width(), img.height(), reported_scale, native_physical);

    Ok(CapturedScreen {
        image: img,
        x,
        y,
        logical_width,
        logical_height,
        scale_factor,
        physical_width,
        physical_height,
        physical_coords,
    })
}

/// Capture every connected monitor separately (one `CapturedScreen` per display).
pub fn capture_all_monitors() -> Result<Vec<CapturedScreen>, String> {
    let monitors = Monitor::all().map_err(|e| format!("Failed to list monitors: {e}"))?;
    let mut caps = Vec::with_capacity(monitors.len());
    let native_physical = cfg!(any(target_os = "windows", target_os = "linux"));
    for m in monitors {
        let Ok(x) = m.x() else { continue };
        let Ok(y) = m.y() else { continue };
        let Ok(rw) = m.width() else { continue };
        let Ok(rh) = m.height() else { continue };
        let Ok(img) = m.capture_image() else { continue };
        let reported_scale = m.scale_factor().unwrap_or(1.0).max(1.0);
        let (
            logical_width,
            logical_height,
            scale_factor,
            physical_width,
            physical_height,
            physical_coords,
        ) = compute_screen_metrics(rw, rh, img.width(), img.height(), reported_scale, native_physical);
        caps.push(CapturedScreen {
            image: img,
            x,
            y,
            logical_width,
            logical_height,
            scale_factor,
            physical_width,
            physical_height,
            physical_coords,
        });
    }
    if caps.is_empty() {
        caps.push(capture_primary_screen()?);
    }
    Ok(caps)
}

/// Enumerate connected monitors with 1x1 blank images (no screenshot taken) for live overlay drawing.
pub fn list_monitors_as_blank_screens() -> Result<Vec<CapturedScreen>, String> {
    let monitors = Monitor::all().map_err(|e| format!("Failed to list monitors: {e}"))?;
    let mut caps = Vec::with_capacity(monitors.len());
    let native_physical = cfg!(any(target_os = "windows", target_os = "linux"));
    for m in monitors {
        let Ok(x) = m.x() else { continue };
        let Ok(y) = m.y() else { continue };
        let Ok(rw) = m.width() else { continue };
        let Ok(rh) = m.height() else { continue };
        let reported_scale = m.scale_factor().unwrap_or(1.0).max(1.0);
        let (logical_width, logical_height, physical_width, physical_height, physical_coords) =
            if native_physical {
                (
                    rw as f32 / reported_scale,
                    rh as f32 / reported_scale,
                    rw.max(1),
                    rh.max(1),
                    true,
                )
            } else {
                (
                    rw as f32,
                    rh as f32,
                    ((rw as f32) * reported_scale).round().max(1.0) as u32,
                    ((rh as f32) * reported_scale).round().max(1.0) as u32,
                    false,
                )
            };
        caps.push(CapturedScreen {
            image: RgbaImage::new(1, 1),
            x,
            y,
            logical_width,
            logical_height,
            scale_factor: reported_scale,
            physical_width,
            physical_height,
            physical_coords,
        });
    }
    if caps.is_empty() {
        return Err("No monitors found".to_string());
    }
    Ok(caps)
}

/// Compute unified virtual-desktop bounds across all monitors.
/// Returns `(min_x, min_y, total_log_w, total_log_h, unified_scale, out_w, out_h, physical_coords)`.
pub fn compute_virtual_bounds(caps: &[CapturedScreen]) -> (i32, i32, f32, f32, f32, u32, u32, bool) {
    if caps.is_empty() {
        return (0, 0, 1920.0, 1080.0, 1.0, 1920, 1080, false);
    }
    if caps.len() == 1 {
        let c = &caps[0];
        return (
            c.x,
            c.y,
            c.logical_width,
            c.logical_height,
            c.scale_factor,
            c.physical_width.max(1),
            c.physical_height.max(1),
            c.physical_coords,
        );
    }

    let any_physical = caps.iter().any(|c| c.physical_coords);
    let unified_scale = caps.iter().map(|c| c.scale_factor).fold(1.0_f32, f32::max);
    let min_x = caps.iter().map(|c| c.x).min().unwrap_or(0);
    let min_y = caps.iter().map(|c| c.y).min().unwrap_or(0);

    if any_physical {
        let max_x = caps
            .iter()
            .map(|c| c.x + c.physical_width as i32)
            .max()
            .unwrap_or(1920);
        let max_y = caps
            .iter()
            .map(|c| c.y + c.physical_height as i32)
            .max()
            .unwrap_or(1080);
        let out_w = (max_x - min_x).max(1) as u32;
        let out_h = (max_y - min_y).max(1) as u32;
        let total_log_w = out_w as f32 / unified_scale;
        let total_log_h = out_h as f32 / unified_scale;
        (
            min_x,
            min_y,
            total_log_w,
            total_log_h,
            unified_scale,
            out_w,
            out_h,
            true,
        )
    } else {
        let max_x = caps
            .iter()
            .map(|c| c.x + c.logical_width.round() as i32)
            .max()
            .unwrap_or(1920);
        let max_y = caps
            .iter()
            .map(|c| c.y + c.logical_height.round() as i32)
            .max()
            .unwrap_or(1080);
        let total_log_w = (max_x - min_x).max(1) as f32;
        let total_log_h = (max_y - min_y).max(1) as f32;
        let out_w = (total_log_w * unified_scale).round().max(1.0) as u32;
        let out_h = (total_log_h * unified_scale).round().max(1.0) as u32;
        (
            min_x,
            min_y,
            total_log_w,
            total_log_h,
            unified_scale,
            out_w,
            out_h,
            false,
        )
    }
}

/// Map a monitor pane's origin into unified virtual-desktop logical coordinates.
pub fn pane_virtual_offset(screen: &CapturedScreen, virtual_screen: &CapturedScreen) -> Vec2 {
    if virtual_screen.physical_coords {
        Vec2::new(
            (screen.x - virtual_screen.x) as f32 / virtual_screen.scale_factor,
            (screen.y - virtual_screen.y) as f32 / virtual_screen.scale_factor,
        )
    } else {
        Vec2::new(
            (screen.x - virtual_screen.x) as f32,
            (screen.y - virtual_screen.y) as f32,
        )
    }
}

/// Scale factor converting local egui pane points to unified virtual-desktop logical points.
pub fn pane_virtual_scale(screen: &CapturedScreen, virtual_screen: &CapturedScreen) -> f32 {
    if virtual_screen.physical_coords {
        (screen.scale_factor / virtual_screen.scale_factor).max(0.1)
    } else {
        1.0
    }
}

/// Compute unified virtual-desktop bounds without allocating a full-resolution composite image.
pub fn composite_monitors_metadata(caps: &[CapturedScreen]) -> CapturedScreen {
    let (x, y, logical_width, logical_height, scale_factor, physical_width, physical_height, physical_coords) =
        compute_virtual_bounds(caps);

    CapturedScreen {
        image: RgbaImage::new(1, 1),
        x,
        y,
        logical_width,
        logical_height,
        scale_factor,
        physical_width,
        physical_height,
        physical_coords,
    }
}

/// Composite all captured monitors into one unified virtual-desktop image for export.
pub fn composite_monitors(caps: &[CapturedScreen]) -> CapturedScreen {
    if caps.len() == 1 {
        let c = &caps[0];
        return CapturedScreen {
            image: c.image.clone(),
            x: c.x,
            y: c.y,
            logical_width: c.logical_width,
            logical_height: c.logical_height,
            scale_factor: c.scale_factor,
            physical_width: c.physical_width,
            physical_height: c.physical_height,
            physical_coords: c.physical_coords,
        };
    }

    let (min_x, min_y, total_log_w, total_log_h, unified_scale, out_w, out_h, physical_coords) =
        compute_virtual_bounds(caps);
    let mut canvas = RgbaImage::from_pixel(out_w, out_h, image::Rgba([0, 0, 0, 255]));

    for c in caps {
        let (target_w, target_h, dx, dy) = if physical_coords {
            (
                c.physical_width.max(1),
                c.physical_height.max(1),
                (c.x - min_x) as i64,
                (c.y - min_y) as i64,
            )
        } else {
            (
                (c.logical_width * unified_scale).round().max(1.0) as u32,
                (c.logical_height * unified_scale).round().max(1.0) as u32,
                ((c.x - min_x) as f32 * unified_scale).round() as i64,
                ((c.y - min_y) as f32 * unified_scale).round() as i64,
            )
        };

        if c.image.width() == target_w && c.image.height() == target_h {
            image::imageops::overlay(&mut canvas, &c.image, dx, dy);
        } else {
            let resized = image::imageops::resize(
                &c.image,
                target_w,
                target_h,
                image::imageops::FilterType::Triangle,
            );
            image::imageops::overlay(&mut canvas, &resized, dx, dy);
        }
    }

    CapturedScreen {
        image: canvas,
        x: min_x,
        y: min_y,
        logical_width: total_log_w,
        logical_height: total_log_h,
        scale_factor: unified_scale,
        physical_width: out_w,
        physical_height: out_h,
        physical_coords,
    }
}

/// Consume `screens` without duplicating the single-monitor screenshot buffer in RAM.
pub fn take_composite_virtual_screen(mut screens: Vec<CapturedScreen>) -> (CapturedScreen, Vec<CapturedScreen>) {
    if screens.len() == 1 {
        let mut single = screens.remove(0);
        let full_img = std::mem::replace(&mut single.image, RgbaImage::new(1, 1));
        let virtual_screen = CapturedScreen {
            image: full_img,
            x: single.x,
            y: single.y,
            logical_width: single.logical_width,
            logical_height: single.logical_height,
            scale_factor: single.scale_factor,
            physical_width: single.physical_width,
            physical_height: single.physical_height,
            physical_coords: single.physical_coords,
        };
        (virtual_screen, vec![single])
    } else {
        let virtual_screen = composite_monitors(&screens);
        (virtual_screen, screens)
    }
}

/// List visible application windows in front-to-back order mapped onto `screen` virtual coordinates.
pub fn list_windows_on_screen(screen: &CapturedScreen) -> Vec<WindowCandidate> {
    let Ok(windows) = Window::all() else {
        return Vec::new();
    };
    let my_pid = std::process::id();
    let screen_rect = Rect::from_min_size(
        Pos2::ZERO,
        Vec2::new(screen.logical_width, screen.logical_height),
    );

    let mut entries: Vec<(i32, WindowCandidate)> = Vec::new();
    for w in windows {
        if w.pid().ok() == Some(my_pid) {
            continue;
        }
        if w.is_minimized().unwrap_or(false) {
            continue;
        }
        let Ok(wx) = w.x() else { continue };
        let Ok(wy) = w.y() else { continue };
        let Ok(ww) = w.width() else { continue };
        let Ok(wh) = w.height() else { continue };
        if ww < 24 || wh < 24 {
            continue;
        }
        let app = w.app_name().unwrap_or_default();
        if app == "Window Server"
            || app == "Dock"
            || app == "Notification Center"
            || app == "Program Manager"
            || app == "Windows Input Experience"
        {
            continue;
        }

        let div = if screen.physical_coords {
            screen.scale_factor
        } else {
            1.0
        };
        let rect = Rect::from_min_size(
            Pos2::new((wx - screen.x) as f32 / div, (wy - screen.y) as f32 / div),
            Vec2::new(ww as f32 / div, wh as f32 / div),
        )
        .intersect(screen_rect);

        if rect.width() < 24.0 || rect.height() < 24.0 {
            continue;
        }

        let title = w.title().unwrap_or_default();
        let label = if title.is_empty() {
            app
        } else if app.is_empty() {
            title
        } else {
            format!("{app} — {title}")
        };

        let z = w.z().unwrap_or(0);
        entries.push((z, WindowCandidate { rect, label }));
    }

    entries.sort_by(|a, b| b.0.cmp(&a.0));
    entries.into_iter().map(|(_, c)| c).collect()
}

/// Copy an RGBA image to the system clipboard.
pub fn copy_to_clipboard(img: &RgbaImage) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| format!("Clipboard error: {e}"))?;
    let data = ImageData {
        width: img.width() as usize,
        height: img.height() as usize,
        bytes: Cow::Borrowed(img.as_raw()),
    };
    clipboard
        .set_image(data)
        .map_err(|e| format!("Failed to copy image: {e}"))
}

/// Atomically create a unique PNG file (`<stem>.png`, `<stem>_1.png`, ...) in `dir`.
pub fn reserve_unique_screenshot_file(dir: &Path, stem: &str) -> Result<(PathBuf, File), String> {
    fs::create_dir_all(dir).map_err(|e| format!("Failed to create folder {}: {e}", dir.display()))?;

    for idx in 0..10_000u32 {
        let filename = if idx == 0 {
            format!("{stem}.png")
        } else {
            format!("{stem}_{idx}.png")
        };
        let path = dir.join(filename);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Failed to create {}: {e}", path.display())),
        }
    }

    Err("Could not allocate a unique screenshot filename".to_string())
}

/// Encode an RGBA image to an open file handle using fast PNG compression.
pub fn encode_png_fast(img: &RgbaImage, file: File) -> Result<(), String> {
    let writer = BufWriter::new(file);
    let encoder = PngEncoder::new_with_quality(writer, CompressionType::Fast, FilterType::Sub);
    encoder
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("Failed to encode PNG: {e}"))
}

/// Reserve a unique file synchronously (fail-fast) and encode PNG in a background thread.
pub fn save_to_dir_async(img: RgbaImage, dir: &Path) -> Result<PathBuf, String> {
    let stem = format!("ShotCraft_{}", Local::now().format("%Y-%m-%d_%H-%M-%S"));
    let (path, file) = reserve_unique_screenshot_file(dir, &stem)?;
    let path_for_thread = path.clone();

    let handle = std::thread::spawn(move || {
        if let Err(e) = encode_png_fast(&img, file) {
            let _ = fs::remove_file(&path_for_thread);
            diagnostics::notify_error("ShotCraft Save Failed", &e);
        }
    });

    if let Ok(mut pending) = PENDING_SAVES.lock() {
        pending.retain(|h| !h.is_finished());
        pending.push(handle);
    }

    Ok(path)
}

/// Wait for any in-flight background PNG saves before exiting the process.
pub fn wait_pending_saves() {
    let handles = if let Ok(mut pending) = PENDING_SAVES.lock() {
        std::mem::take(&mut *pending)
    } else {
        Vec::new()
    };
    for h in handles {
        let _ = h.join();
    }
}

/// Open the configured save folder in Finder (macOS), Explorer (Windows), or xdg-open (Linux).
pub fn open_folder(dir: &Path) {
    let _ = fs::create_dir_all(dir);
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(dir).spawn();
    #[cfg(target_os = "windows")]
    let _ = Command::new("explorer").arg(dir).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = Command::new("xdg-open").arg(dir).spawn();
}

/// Native folder picker (osascript on macOS, PowerShell FolderBrowserDialog on Windows, zenity/kdialog on Linux).
pub fn pick_folder_dialog() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let out = Command::new("osascript")
            .arg("-e")
            .arg("POSIX path of (choose folder with prompt \"Select ShotCraft Save Folder\")")
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let path_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if path_str.is_empty() {
            None
        } else {
            Some(PathBuf::from(path_str))
        }
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$dlg = New-Object System.Windows.Forms.FolderBrowserDialog
$dlg.Description = 'Select ShotCraft Save Folder'
if ($dlg.ShowDialog() -eq 'OK') { Write-Output $dlg.SelectedPath }
"#;
        let out = Command::new("powershell")
            .args(["-NoProfile", "-STA", "-Command", script])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let path_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if path_str.is_empty() {
            None
        } else {
            Some(PathBuf::from(path_str))
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let out = Command::new("zenity")
            .args([
                "--file-selection",
                "--directory",
                "--title=Select ShotCraft Save Folder",
            ])
            .output()
            .or_else(|_| {
                Command::new("kdialog")
                    .args(["--getexistingdirectory", "."])
                    .output()
            })
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let path_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if path_str.is_empty() {
            None
        } else {
            Some(PathBuf::from(path_str))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_metrics_macos_retina_and_125_scaling() {
        let (lw, lh, sf, pw, ph, phys) = compute_screen_metrics(1440, 900, 2880, 1800, 2.0, false);
        assert_eq!((lw, lh, sf, pw, ph, phys), (1440.0, 900.0, 2.0, 2880, 1800, false));

        // 1.25x Retina on macOS must still be recognized as logical Monitor + physical Image
        let (lw, lh, sf, pw, ph, phys) = compute_screen_metrics(1920, 1080, 2400, 1350, 1.25, false);
        assert_eq!((lw, lh, sf, pw, ph, phys), (1920.0, 1080.0, 1.25, 2400, 1350, false));
    }

    #[test]
    fn screen_metrics_windows_hidpi() {
        let (lw, lh, sf, pw, ph, phys) = compute_screen_metrics(1920, 1080, 1920, 1080, 1.25, true);
        assert!((lw - 1536.0).abs() < 0.01);
        assert!((lh - 864.0).abs() < 0.01);
        assert_eq!((sf, pw, ph, phys), (1.25, 1920, 1080, true));

        let (lw, lh, sf, pw, ph, phys) = compute_screen_metrics(3840, 2160, 3840, 2160, 2.0, true);
        assert_eq!((lw, lh, sf, pw, ph, phys), (1920.0, 1080.0, 2.0, 3840, 2160, true));
    }

    #[test]
    fn composite_windows_mixed_dpi_monitors_exact_alignment() {
        let m0 = CapturedScreen {
            image: RgbaImage::from_pixel(3840, 2160, image::Rgba([10, 20, 30, 255])),
            x: 0,
            y: 0,
            logical_width: 1920.0,
            logical_height: 1080.0,
            scale_factor: 2.0,
            physical_width: 3840,
            physical_height: 2160,
            physical_coords: true,
        };
        let m1 = CapturedScreen {
            image: RgbaImage::from_pixel(1920, 1080, image::Rgba([200, 100, 50, 255])),
            x: 3840,
            y: 0,
            logical_width: 1920.0,
            logical_height: 1080.0,
            scale_factor: 1.0,
            physical_width: 1920,
            physical_height: 1080,
            physical_coords: true,
        };

        let comp = composite_monitors(&[m0, m1]);
        assert_eq!(comp.image.width(), 5760);
        assert_eq!(comp.image.height(), 2160);
        assert!(comp.physical_coords);
        assert_eq!(*comp.image.get_pixel(100, 100), image::Rgba([10, 20, 30, 255]));
        assert_eq!(*comp.image.get_pixel(3900, 100), image::Rgba([200, 100, 50, 255]));
    }

    #[test]
    fn pane_virtual_mapping_mixed_dpi() {
        let m1 = CapturedScreen {
            image: RgbaImage::new(1, 1),
            x: 3840,
            y: 0,
            logical_width: 1920.0,
            logical_height: 1080.0,
            scale_factor: 1.0,
            physical_width: 1920,
            physical_height: 1080,
            physical_coords: true,
        };
        let virt = CapturedScreen {
            image: RgbaImage::new(1, 1),
            x: 0,
            y: 0,
            logical_width: 2880.0,
            logical_height: 1080.0,
            scale_factor: 2.0,
            physical_width: 5760,
            physical_height: 2160,
            physical_coords: true,
        };
        let off = pane_virtual_offset(&m1, &virt);
        let scale = pane_virtual_scale(&m1, &virt);
        assert_eq!(off, Vec2::new(1920.0, 0.0));
        assert_eq!(scale, 0.5);
        // Center of 1080p@100% pane (960, 540) -> virtual (2400, 270) -> physical (4800, 540)
        let gp = off + Vec2::new(960.0, 540.0) * scale;
        assert_eq!(gp.x * virt.scale_factor, 4800.0);
        assert_eq!(gp.y * virt.scale_factor, 540.0);
    }

    #[test]
    fn unique_filename_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("shotcraft_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let (p0, f0) = reserve_unique_screenshot_file(&dir, "ShotCraft_2026").unwrap();
        drop(f0);
        let (p1, f1) = reserve_unique_screenshot_file(&dir, "ShotCraft_2026").unwrap();
        drop(f1);
        let (p2, f2) = reserve_unique_screenshot_file(&dir, "ShotCraft_2026").unwrap();
        drop(f2);

        assert!(p0.ends_with("ShotCraft_2026.png"));
        assert!(p1.ends_with("ShotCraft_2026_1.png"));
        assert!(p2.ends_with("ShotCraft_2026_2.png"));
        assert_ne!(p0, p1);
        assert_ne!(p1, p2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn composite_macos_dual_display_logical_coords() {
        let m0 = CapturedScreen {
            image: RgbaImage::from_pixel(2880, 1800, image::Rgba([10, 20, 30, 255])),
            x: 0,
            y: 0,
            logical_width: 1440.0,
            logical_height: 900.0,
            scale_factor: 2.0,
            physical_width: 2880,
            physical_height: 1800,
            physical_coords: false,
        };
        let m1 = CapturedScreen {
            image: RgbaImage::from_pixel(1920, 1080, image::Rgba([40, 80, 120, 255])),
            x: 1440,
            y: 0,
            logical_width: 1920.0,
            logical_height: 1080.0,
            scale_factor: 1.0,
            physical_width: 1920,
            physical_height: 1080,
            physical_coords: false,
        };
        let comp = composite_monitors(&[m0, m1]);
        assert!(!comp.physical_coords);
        assert_eq!(comp.scale_factor, 2.0);
        assert_eq!(comp.logical_width, 3360.0);
        assert_eq!(comp.logical_height, 1080.0);
        assert_eq!(comp.image.width(), 6720);
        assert_eq!(comp.image.height(), 2160);
    }
}
