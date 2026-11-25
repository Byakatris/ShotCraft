use global_hotkey::hotkey::HotKey;
use global_hotkey::GlobalHotKeyManager;
use muda::{Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::diagnostics;
use crate::settings::{AppSettings, Strings};

pub const ID_CAPTURE_FULL: &str = "capture_full";
pub const ID_CAPTURE_AREA: &str = "capture_area";
pub const ID_CAPTURE_WINDOW: &str = "capture_window";
pub const ID_LIVE_DRAW: &str = "live_draw";
pub const ID_OPEN_FOLDER: &str = "open_folder";
pub const ID_SETTINGS: &str = "settings";
pub const ID_CHECK_UPDATES: &str = "check_updates";
pub const ID_QUIT: &str = "quit";

pub struct TrayState {
    pub tray: TrayIcon,
    pub hotkey_manager: Option<GlobalHotKeyManager>,
    pub registered_hotkeys: Vec<HotKey>,
    pub hk_full_id: Option<u32>,
    pub hk_area_id: Option<u32>,
    pub hk_window_id: Option<u32>,
    pub hk_live_draw_id: Option<u32>,
}

impl TrayState {
    pub fn update_menu(
        &mut self,
        settings: &AppSettings,
        available_version: Option<&str>,
    ) -> Result<(), String> {
        let menu = build_menu(settings, available_version)?;
        self.tray.set_menu(Some(Box::new(menu)));
        Ok(())
    }

    pub fn update_hotkeys(&mut self, settings: &AppSettings) {
        if let Some(mgr) = &self.hotkey_manager {
            if !self.registered_hotkeys.is_empty() {
                let _ = mgr.unregister_all(&self.registered_hotkeys);
                self.registered_hotkeys.clear();
            }
            let (registered, ids) = register_hotkeys(mgr, settings);
            self.registered_hotkeys = registered;
            self.hk_full_id = ids[0];
            self.hk_area_id = ids[1];
            self.hk_window_id = ids[2];
            self.hk_live_draw_id = ids[3];
        }
    }
}

fn register_hotkeys(
    mgr: &GlobalHotKeyManager,
    settings: &AppSettings,
) -> (Vec<HotKey>, [Option<u32>; 4]) {
    let specs = [
        settings.hk_fullscreen.to_hotkey(),
        settings.hk_area.to_hotkey(),
        settings.hk_window.to_hotkey(),
        settings.hk_live_draw.to_hotkey(),
    ];
    let mut registered = Vec::with_capacity(4);
    let mut ids = [None; 4];
    for (idx, hk) in specs.into_iter().enumerate() {
        if let Err(e) = mgr.register(hk) {
            diagnostics::log_error(
                "tray.hotkey",
                &format!("Could not register hotkey {}: {e}", hk.id()),
            );
        } else {
            ids[idx] = Some(hk.id());
            registered.push(hk);
        }
    }
    (registered, ids)
}

pub fn build_tray(
    settings: &AppSettings,
    available_version: Option<&str>,
) -> Result<TrayState, String> {
    let menu = build_menu(settings, available_version)?;

    let icon = make_shotcraft_icon()?;
    #[allow(unused_mut)]
    let mut tray_builder = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(true)
        .with_tooltip("ShotCraft")
        .with_icon(icon);
    #[cfg(target_os = "macos")]
    {
        tray_builder = tray_builder.with_icon_as_template(true);
    }
    let tray = tray_builder
        .build()
        .map_err(|e| format!("Failed to create tray icon: {e}"))?;

    let mut registered_hotkeys = Vec::new();
    let mut ids = [None; 4];
    let hotkey_manager = match GlobalHotKeyManager::new() {
        Ok(mgr) => {
            let (reg, reg_ids) = register_hotkeys(&mgr, settings);
            registered_hotkeys = reg;
            ids = reg_ids;
            Some(mgr)
        }
        Err(e) => {
            #[cfg(target_os = "linux")]
            diagnostics::log_error(
                "tray.hotkey",
                &format!(
                    "Global hotkeys unavailable (Wayland requires X11/XWayland; tray menu remains active): {e}"
                ),
            );
            #[cfg(not(target_os = "linux"))]
            diagnostics::log_error(
                "tray.hotkey",
                &format!("GlobalHotKeyManager unavailable: {e}"),
            );
            None
        }
    };

    Ok(TrayState {
        tray,
        hotkey_manager,
        registered_hotkeys,
        hk_full_id: ids[0],
        hk_area_id: ids[1],
        hk_window_id: ids[2],
        hk_live_draw_id: ids[3],
    })
}

pub fn format_update_menu_label(t: &Strings, available_version: Option<&str>) -> String {
    match available_version {
        Some(ver) => format!("{} (v{ver})", t.menu_update_available),
        None => t.menu_check_updates.to_string(),
    }
}

fn build_menu(settings: &AppSettings, available_version: Option<&str>) -> Result<Menu, String> {
    let menu = Menu::new();
    let t = Strings::for_lang(settings.language);

    let item_full = MenuItem::with_id(
        ID_CAPTURE_FULL,
        t.menu_full,
        true,
        Some(settings.hk_fullscreen.to_accelerator()),
    );
    let item_area = MenuItem::with_id(
        ID_CAPTURE_AREA,
        t.menu_area,
        true,
        Some(settings.hk_area.to_accelerator()),
    );
    let item_win = MenuItem::with_id(
        ID_CAPTURE_WINDOW,
        t.menu_window,
        true,
        Some(settings.hk_window.to_accelerator()),
    );
    let item_live_draw = MenuItem::with_id(
        ID_LIVE_DRAW,
        t.menu_live_draw,
        true,
        Some(settings.hk_live_draw.to_accelerator()),
    );
    let item_open = MenuItem::with_id(ID_OPEN_FOLDER, t.menu_open_folder, true, None);
    let item_settings = MenuItem::with_id(ID_SETTINGS, t.menu_settings, true, None);
    let update_label = format_update_menu_label(t, available_version);
    let item_updates = MenuItem::with_id(ID_CHECK_UPDATES, update_label, true, None);
    let item_quit = MenuItem::with_id(ID_QUIT, t.menu_quit, true, None);

    menu.append(&item_full).map_err(|e| e.to_string())?;
    menu.append(&item_area).map_err(|e| e.to_string())?;
    menu.append(&item_win).map_err(|e| e.to_string())?;
    menu.append(&item_live_draw).map_err(|e| e.to_string())?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| e.to_string())?;
    menu.append(&item_open).map_err(|e| e.to_string())?;
    menu.append(&item_settings).map_err(|e| e.to_string())?;
    menu.append(&item_updates).map_err(|e| e.to_string())?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| e.to_string())?;
    menu.append(&item_quit).map_err(|e| e.to_string())?;

    Ok(menu)
}

#[cfg(not(target_os = "macos"))]
fn inside_rounded_rect(x: f32, y: f32, min: f32, max: f32, radius: f32) -> bool {
    let cx = x.clamp(min + radius, max - radius);
    let cy = y.clamp(min + radius, max - radius);
    let dx = x - cx;
    let dy = y - cy;
    dx * dx + dy * dy <= radius * radius
}

/// Generate a minimalist 32x32 tray icon featuring a screenshot viewfinder + "SC" monogram.
pub(crate) fn make_shotcraft_icon_rgba() -> (Vec<u8>, u32) {
    const SIZE: usize = 32;
    let mut rgba = vec![0u8; SIZE * SIZE * 4];

    #[cfg(target_os = "macos")]
    let fg: [u8; 4] = [0, 0, 0, 255];
    #[cfg(not(target_os = "macos"))]
    let fg: [u8; 4] = [94, 234, 212, 255];

    let mut set_px = |x: usize, y: usize, col: [u8; 4]| {
        if x < SIZE && y < SIZE {
            let idx = (y * SIZE + x) * 4;
            rgba[idx..idx + 4].copy_from_slice(&col);
        }
    };

    #[cfg(not(target_os = "macos"))]
    {
        let bg: [u8; 4] = [15, 22, 38, 255];
        for y in 1..(SIZE - 1) {
            for x in 1..(SIZE - 1) {
                if inside_rounded_rect(x as f32 + 0.5, y as f32 + 0.5, 1.0, (SIZE - 1) as f32, 6.0)
                {
                    set_px(x, y, bg);
                }
            }
        }
    }

    let mut fill_rect = |x0: usize, y0: usize, x1: usize, y1: usize| {
        for y in y0..=y1 {
            for x in x0..=x1 {
                set_px(x, y, fg);
            }
        }
    };

    // Viewfinder brackets (2px thick, 6px arms, 3px inset)
    let pad = 3;
    let arm = 6;
    let max = SIZE - 1 - pad;
    fill_rect(pad, pad, pad + arm, pad + 1);
    fill_rect(pad, pad, pad + 1, pad + arm);

    fill_rect(max - arm, pad, max, pad + 1);
    fill_rect(max - 1, pad, max, pad + arm);

    fill_rect(pad, max - 1, pad + arm, max);
    fill_rect(pad, max - arm, pad + 1, max);

    fill_rect(max - arm, max - 1, max, max);
    fill_rect(max - 1, max - arm, max, max);

    // Minimalist "S"
    fill_rect(9, 10, 14, 11);
    fill_rect(9, 11, 10, 15);
    fill_rect(9, 15, 14, 16);
    fill_rect(13, 16, 14, 20);
    fill_rect(9, 20, 14, 21);

    // Minimalist "C"
    fill_rect(17, 11, 18, 20);
    fill_rect(18, 10, 22, 11);
    fill_rect(18, 20, 22, 21);

    (rgba, SIZE as u32)
}

fn make_shotcraft_icon() -> Result<Icon, String> {
    let (rgba, size) = make_shotcraft_icon_rgba();
    Icon::from_rgba(rgba, size, size).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Language;

    #[test]
    fn tray_icon_rgba_has_valid_dimensions_and_opaque_monogram() {
        let (rgba, size) = make_shotcraft_icon_rgba();
        assert_eq!(size, 32);
        assert_eq!(rgba.len(), 32 * 32 * 4);
        let opaque = rgba.as_chunks::<4>().0.iter().filter(|px| px[3] == 255).count();
        assert!(opaque >= 60);
        // Outer corner pixel must be transparent
        assert_eq!(rgba[3], 0);
    }

    #[test]
    fn update_menu_label_reflects_available_version() {
        let t = Strings::for_lang(Language::En);
        assert_eq!(format_update_menu_label(t, None), t.menu_check_updates);
        let with_ver = format_update_menu_label(t, Some("0.7.0"));
        assert!(with_ver.contains("0.7.0") && with_ver.contains(t.menu_update_available));
    }
}
