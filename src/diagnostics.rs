use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
#[cfg(not(target_os = "windows"))]
use std::process::{Command, Stdio};

use chrono::Local;

const MAX_LOG_BYTES: u64 = 256 * 1024;

/// Return the path to the persistent ShotCraft log file.
pub fn log_path() -> PathBuf {
    let base = dirs::config_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("ShotCraft").join("shotcraft.log")
}

/// Ensure the log parent directory exists and open it in the system file manager.
pub fn open_log_dir() {
    let path = log_path();
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    if !path.exists() {
        append_log_entry(&path, "INFO", "diagnostics", "Log file initialized");
    }
    crate::capture::open_folder(dir);
}

fn append_log_entry(path: &Path, level: &str, context: &str, msg: &str) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let truncate = fs::metadata(path)
        .map(|m| m.len() > MAX_LOG_BYTES)
        .unwrap_or(false);

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(!truncate)
        .truncate(truncate)
        .open(path);

    if let Ok(mut f) = file {
        let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
        let _ = writeln!(f, "[{ts}] {level} [{context}]: {msg}");
    }
}

/// Append a timestamped informational entry to `shotcraft.log`.
pub fn log_info(context: &str, msg: &str) {
    println!("[ShotCraft] {context}: {msg}");
    append_log_entry(&log_path(), "INFO", context, msg);
}

/// Append a timestamped error entry to `shotcraft.log` (rotating when > 256 KiB) and print to stderr.
pub fn log_error(context: &str, msg: &str) {
    eprintln!("[ShotCraft] {context}: {msg}");
    append_log_entry(&log_path(), "ERROR", context, msg);
}

/// Log an informational message and show a non-blocking native OS notification when supported.
pub fn notify_info(title: &str, msg: &str) {
    log_info(title, msg);
    show_os_notification(title, msg, false);
}

/// Log an error and surface a non-blocking native OS notification when supported.
pub fn notify_error(title: &str, msg: &str) {
    log_error(title, msg);
    show_os_notification(title, msg, true);
}

fn show_os_notification(title: &str, msg: &str, is_error: bool) {
    #[cfg(target_os = "macos")]
    {
        let _ = is_error;
        let esc_title = escape_applescript(title);
        let esc_msg = escape_applescript(msg);
        let script = format!("display notification \"{esc_msg}\" with title \"{esc_title}\"");
        let _ = Command::new("osascript")
            .arg("-e")
            .arg(script)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }

    #[cfg(target_os = "windows")]
    {
        let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let msg_w: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
        // MB_OK | (MB_ICONERROR or MB_ICONINFORMATION) | MB_SETFOREGROUND
        let flags: u32 = if is_error { 0x0001_0010 } else { 0x0001_0040 };
        std::thread::spawn(move || {
            #[link(name = "user32")]
            unsafe extern "system" {
                fn MessageBoxW(
                    hwnd: *mut std::ffi::c_void,
                    text: *const u16,
                    caption: *const u16,
                    utype: u32,
                ) -> i32;
            }
            unsafe {
                MessageBoxW(std::ptr::null_mut(), msg_w.as_ptr(), title_w.as_ptr(), flags);
            }
        });
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let urgency = if is_error { "normal" } else { "low" };
        let _ = Command::new("notify-send")
            .args(["-u", urgency, title, msg])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}

#[cfg(target_os = "macos")]
fn escape_applescript(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_log_entry_creates_and_rotates_oversized_file() {
        let dir = std::env::temp_dir().join(format!("shotcraft_diag_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let log_file = dir.join("shotcraft.log");

        append_log_entry(&log_file, "INFO", "boot", "started cleanly");
        let content = fs::read_to_string(&log_file).unwrap();
        assert!(content.contains("INFO [boot]: started cleanly"));

        // Force file over MAX_LOG_BYTES and ensure next append rotates cleanly
        let filler = vec![b'A'; (MAX_LOG_BYTES as usize) + 512];
        fs::write(&log_file, filler).unwrap();

        append_log_entry(&log_file, "ERROR", "test", "rotated entry");
        let rotated = fs::read_to_string(&log_file).unwrap();
        assert!(rotated.contains("ERROR [test]: rotated entry"));
        assert!(rotated.len() < 1024);

        let _ = fs::remove_dir_all(&dir);
    }
}
