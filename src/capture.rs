use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use arboard::{Clipboard, ImageData};
use chrono::Local;
use egui::{Pos2, Rect, Vec2};
use image::RgbaImage;
use xcap::{Monitor, Window};

pub struct CapturedScreen {
    pub image: RgbaImage,
    pub x: i32,
    pub y: i32,
    pub logical_width: f32,
    pub logical_height: f32,
    pub scale_factor: f32,
    pub physical_coords: bool,
}

#[derive(Debug, Clone)]
pub struct WindowCandidate {
    pub rect: Rect,
    pub label: String,
}

/// Get the current global mouse cursor position on macOS or Windows.
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

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
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
    let raw_w = monitor.width().map_err(|e| e.to_string())? as f32;
    let raw_h = monitor.height().map_err(|e| e.to_string())? as f32;
    let reported_scale = monitor.scale_factor().unwrap_or(1.0).max(1.0);

    let img = monitor
        .capture_image()
        .map_err(|e| format!("Screen capture failed (check Screen Recording permission): {e}"))?;

    // macOS reports logical points in Monitor::width/height while img is physical pixels;
    // Windows reports physical pixels in both Monitor::width/height and img.
    let (logical_width, logical_height, scale_factor, physical_coords) =
        if (img.width() as f32) > raw_w * 1.25 {
            (raw_w, raw_h, img.width() as f32 / raw_w, false)
        } else {
            let physical = cfg!(target_os = "windows");
            (
                raw_w / reported_scale,
                raw_h / reported_scale,
                reported_scale,
                physical,
            )
        };

    Ok(CapturedScreen {
        image: img,
        x,
        y,
        logical_width,
        logical_height,
        scale_factor,
        physical_coords,
    })
}

/// Capture every connected monitor separately (one `CapturedScreen` per display).
pub fn capture_all_monitors() -> Result<Vec<CapturedScreen>, String> {
    let monitors = Monitor::all().map_err(|e| format!("Failed to list monitors: {e}"))?;
    let mut caps = Vec::with_capacity(monitors.len());
    for m in monitors {
        let Ok(x) = m.x() else { continue };
        let Ok(y) = m.y() else { continue };
        let Ok(rw) = m.width() else { continue };
        let Ok(rh) = m.height() else { continue };
        let Ok(img) = m.capture_image() else { continue };
        let reported_scale = m.scale_factor().unwrap_or(1.0).max(1.0);
        let (logical_width, logical_height, scale_factor, physical_coords) =
            if (img.width() as f32) > (rw as f32) * 1.25 {
                (rw as f32, rh as f32, img.width() as f32 / rw as f32, false)
            } else {
                let physical = cfg!(target_os = "windows");
                (
                    rw as f32 / reported_scale,
                    rh as f32 / reported_scale,
                    reported_scale,
                    physical,
                )
            };
        caps.push(CapturedScreen {
            image: img,
            x,
            y,
            logical_width,
            logical_height,
            scale_factor,
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
    for m in monitors {
        let Ok(x) = m.x() else { continue };
        let Ok(y) = m.y() else { continue };
        let Ok(rw) = m.width() else { continue };
        let Ok(rh) = m.height() else { continue };
        let reported_scale = m.scale_factor().unwrap_or(1.0).max(1.0);
        let (logical_width, logical_height, physical_coords) = if cfg!(target_os = "windows") {
            (rw as f32 / reported_scale, rh as f32 / reported_scale, true)
        } else {
            (rw as f32, rh as f32, false)
        };
        caps.push(CapturedScreen {
            image: RgbaImage::new(1, 1),
            x,
            y,
            logical_width,
            logical_height,
            scale_factor: reported_scale,
            physical_coords,
        });
    }
    if caps.is_empty() {
        return Err("No monitors found".to_string());
    }
    Ok(caps)
}

/// Compute unified virtual-desktop bounds without allocating a full-resolution composite image.
pub fn composite_monitors_metadata(caps: &[CapturedScreen]) -> CapturedScreen {
    let min_x = caps.iter().map(|c| c.x).min().unwrap_or(0);
    let min_y = caps.iter().map(|c| c.y).min().unwrap_or(0);
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
    let unified_scale = caps.iter().map(|c| c.scale_factor).fold(1.0_f32, f32::max);

    CapturedScreen {
        image: RgbaImage::new(1, 1),
        x: min_x,
        y: min_y,
        logical_width: total_log_w,
        logical_height: total_log_h,
        scale_factor: unified_scale,
        physical_coords: false,
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
            physical_coords: c.physical_coords,
        };
    }

    let min_x = caps.iter().map(|c| c.x).min().unwrap_or(0);
    let min_y = caps.iter().map(|c| c.y).min().unwrap_or(0);
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
    let unified_scale = caps.iter().map(|c| c.scale_factor).fold(1.0_f32, f32::max);

    let out_w = (total_log_w * unified_scale).round() as u32;
    let out_h = (total_log_h * unified_scale).round() as u32;
    let mut canvas = RgbaImage::from_pixel(out_w, out_h, image::Rgba([0, 0, 0, 255]));

    for c in caps {
        let target_w = (c.logical_width * unified_scale).round() as u32;
        let target_h = (c.logical_height * unified_scale).round() as u32;
        let dx = ((c.x - min_x) as f32 * unified_scale).round() as i64;
        let dy = ((c.y - min_y) as f32 * unified_scale).round() as i64;

        if c.image.width() == target_w && c.image.height() == target_h {
            image::imageops::overlay(&mut canvas, &c.image, dx, dy);
        } else {
            let resized = image::imageops::resize(
                &c.image,
                target_w.max(1),
                target_h.max(1),
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
        physical_coords: false,
    }
}

/// List visible macOS application windows in front-to-back order mapped onto `screen` coordinates.
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

    // Higher z or earlier order: sort descending by z (macOS CGWindowList order is front-to-back,
    // keep stable order when z matches).
    entries.sort_by(|a, b| b.0.cmp(&a.0));
    entries.into_iter().map(|(_, c)| c).collect()
}

/// Copy an RGBA image to the macOS clipboard.
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

/// Save an RGBA image as `ShotCraft_YYYY-MM-DD_HH-MM-SS.png` in `dir`.
pub fn save_to_dir(img: &RgbaImage, dir: &Path) -> Result<PathBuf, String> {
    let _ = fs::create_dir_all(dir);
    let filename = format!("ShotCraft_{}.png", Local::now().format("%Y-%m-%d_%H-%M-%S"));
    let path = dir.join(filename);
    img.save(&path)
        .map_err(|e| format!("Failed to save PNG: {e}"))?;
    Ok(path)
}

/// Open the configured save folder in Finder (macOS) or Explorer (Windows).
pub fn open_folder(dir: &Path) {
    let _ = fs::create_dir_all(dir);
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(dir).spawn();
    #[cfg(target_os = "windows")]
    let _ = Command::new("explorer").arg(dir).spawn();
}

/// Native folder picker (osascript on macOS, PowerShell FolderBrowserDialog on Windows).
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
        None
    }
}
