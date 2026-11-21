use global_hotkey::hotkey::HotKey;
use global_hotkey::GlobalHotKeyManager;
use muda::{Menu, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::settings::{AppSettings, Strings};

pub struct TrayState {
    pub tray: TrayIcon,
    pub id_capture_full: MenuId,
    pub id_capture_area: MenuId,
    pub id_capture_window: MenuId,
    pub id_live_draw: MenuId,
    pub id_open_folder: MenuId,
    pub id_settings: MenuId,
    pub id_quit: MenuId,
    pub hotkey_manager: Option<GlobalHotKeyManager>,
    pub registered_hotkeys: Vec<HotKey>,
    pub hk_full_id: u32,
    pub hk_area_id: u32,
    pub hk_window_id: u32,
    pub hk_live_draw_id: u32,
}

impl TrayState {
    pub fn apply_settings(&mut self, settings: &AppSettings) -> Result<(), String> {
        let (menu, id_full, id_area, id_window, id_live_draw, id_open, id_settings, id_quit) =
            build_menu(settings)?;

        self.tray.set_menu(Some(Box::new(menu)));
        self.id_capture_full = id_full;
        self.id_capture_area = id_area;
        self.id_capture_window = id_window;
        self.id_live_draw = id_live_draw;
        self.id_open_folder = id_open;
        self.id_settings = id_settings;
        self.id_quit = id_quit;

        if let Some(mgr) = &self.hotkey_manager {
            if !self.registered_hotkeys.is_empty() {
                let _ = mgr.unregister_all(&self.registered_hotkeys);
                self.registered_hotkeys.clear();
            }

            let hk_full = settings.hk_fullscreen.to_hotkey();
            let hk_area = settings.hk_area.to_hotkey();
            let hk_win = settings.hk_window.to_hotkey();
            let hk_live = settings.hk_live_draw.to_hotkey();

            self.hk_full_id = hk_full.id();
            self.hk_area_id = hk_area.id();
            self.hk_window_id = hk_win.id();
            self.hk_live_draw_id = hk_live.id();

            for hk in [hk_full, hk_area, hk_win, hk_live] {
                if let Err(e) = mgr.register(hk) {
                    eprintln!("Warning: could not register hotkey {}: {e}", hk.id());
                } else {
                    self.registered_hotkeys.push(hk);
                }
            }
        }

        Ok(())
    }
}

pub fn build_tray(settings: &AppSettings) -> Result<TrayState, String> {
    let (menu, id_full, id_area, id_window, id_live_draw, id_open, id_settings, id_quit) =
        build_menu(settings)?;

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

    let hk_full = settings.hk_fullscreen.to_hotkey();
    let hk_area = settings.hk_area.to_hotkey();
    let hk_win = settings.hk_window.to_hotkey();
    let hk_live = settings.hk_live_draw.to_hotkey();

    let hk_full_id = hk_full.id();
    let hk_area_id = hk_area.id();
    let hk_window_id = hk_win.id();
    let hk_live_draw_id = hk_live.id();

    let mut registered_hotkeys = Vec::new();
    let hotkey_manager = match GlobalHotKeyManager::new() {
        Ok(mgr) => {
            for hk in [hk_full, hk_area, hk_win, hk_live] {
                if let Err(e) = mgr.register(hk) {
                    eprintln!("Warning: could not register hotkey {}: {e}", hk.id());
                } else {
                    registered_hotkeys.push(hk);
                }
            }
            Some(mgr)
        }
        Err(e) => {
            eprintln!("Warning: GlobalHotKeyManager unavailable: {e}");
            None
        }
    };

    Ok(TrayState {
        tray,
        id_capture_full: id_full,
        id_capture_area: id_area,
        id_capture_window: id_window,
        id_live_draw,
        id_open_folder: id_open,
        id_settings,
        id_quit,
        hotkey_manager,
        registered_hotkeys,
        hk_full_id,
        hk_area_id,
        hk_window_id,
        hk_live_draw_id,
    })
}

#[allow(clippy::type_complexity)]
fn build_menu(
    settings: &AppSettings,
) -> Result<(Menu, MenuId, MenuId, MenuId, MenuId, MenuId, MenuId, MenuId), String> {
    let menu = Menu::new();
    let t = Strings::for_lang(settings.language);

    let item_full = MenuItem::new(t.menu_full, true, Some(settings.hk_fullscreen.to_accelerator()));
    let item_area = MenuItem::new(t.menu_area, true, Some(settings.hk_area.to_accelerator()));
    let item_win = MenuItem::new(t.menu_window, true, Some(settings.hk_window.to_accelerator()));
    let item_live_draw = MenuItem::new(
        t.menu_live_draw,
        true,
        Some(settings.hk_live_draw.to_accelerator()),
    );
    let item_open = MenuItem::new(t.menu_open_folder, true, None);
    let item_settings = MenuItem::new(t.menu_settings, true, None);
    let item_quit = MenuItem::new(t.menu_quit, true, None);

    let id_full = item_full.id().clone();
    let id_area = item_area.id().clone();
    let id_win = item_win.id().clone();
    let id_live_draw = item_live_draw.id().clone();
    let id_open = item_open.id().clone();
    let id_settings = item_settings.id().clone();
    let id_quit = item_quit.id().clone();

    menu.append(&item_full).map_err(|e| e.to_string())?;
    menu.append(&item_area).map_err(|e| e.to_string())?;
    menu.append(&item_win).map_err(|e| e.to_string())?;
    menu.append(&item_live_draw).map_err(|e| e.to_string())?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| e.to_string())?;
    menu.append(&item_open).map_err(|e| e.to_string())?;
    menu.append(&item_settings).map_err(|e| e.to_string())?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| e.to_string())?;
    menu.append(&item_quit).map_err(|e| e.to_string())?;

    Ok((
        menu,
        id_full,
        id_area,
        id_win,
        id_live_draw,
        id_open,
        id_settings,
        id_quit,
    ))
}

/// Generate a minimalist 32x32 tray icon featuring a screenshot viewfinder + "SC" monogram.
/// On macOS: black template icon. On Windows: Dark Navy + Light Teal icon visible on any taskbar.
fn make_shotcraft_icon() -> Result<Icon, String> {
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
        // Dark Navy rounded-square background for Windows taskbar legibility
        let bg: [u8; 4] = [15, 22, 38, 255];
        for y in 1..(SIZE - 1) {
            for x in 1..(SIZE - 1) {
                let corner = (x <= 3 || x >= SIZE - 4) && (y <= 3 || y >= SIZE - 4);
                if !corner {
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

    // Minimalist "S" (x: 9..14, y: 10..21)
    fill_rect(9, 10, 14, 11);
    fill_rect(9, 11, 10, 15);
    fill_rect(9, 15, 14, 16);
    fill_rect(13, 16, 14, 20);
    fill_rect(9, 20, 14, 21);

    // Minimalist "C" (x: 17..22, y: 10..21)
    fill_rect(17, 11, 18, 20);
    fill_rect(18, 10, 22, 11);
    fill_rect(18, 20, 22, 21);

    Icon::from_rgba(rgba, SIZE as u32, SIZE as u32).map_err(|e| e.to_string())
}
