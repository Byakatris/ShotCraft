#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod annotations;
mod capture;
mod diagnostics;
mod gl_window;
mod icons;
mod material;
mod overlay;
mod settings;
mod settings_ui;
mod tray;
mod updater;

use std::path::PathBuf;

use global_hotkey::{GlobalHotKeyEvent, HotKeyState};
use muda::MenuEvent;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
#[cfg(target_os = "macos")]
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
use winit::window::WindowId;

use crate::annotations::{render_export, Annotation};
use crate::capture::{
    capture_all_monitors, capture_primary_screen, composite_monitors_metadata, copy_to_clipboard,
    list_monitors_as_blank_screens, list_windows_on_screen, open_folder, save_to_dir_async,
    wait_pending_saves, CapturedScreen,
};
use crate::material::ThemeColors;
use crate::overlay::{CaptureFlashWindow, OverlayConfig, OverlayOutcome, OverlayWindow};
use crate::settings::{AppSettings, Strings};
use crate::settings_ui::{SettingsOutcome, SettingsWindow};
use crate::tray::{build_tray, TrayState};

pub enum AppEvent {
    Menu(MenuEvent),
    HotKey(GlobalHotKeyEvent),
    FolderPicked(Option<PathBuf>),
    UpdateCheckFinished {
        result: Result<Option<updater::ReleaseInfo>, String>,
        manual: bool,
    },
    UpdateInstallFinished(Result<updater::InstallAction, String>),
    TriggerUpdateCheck,
    TriggerUpdateInstall,
}

#[derive(Clone, Copy)]
enum PendingCaptureKind {
    Fullscreen { copy: bool, save: bool },
    Area,
    Window,
    LiveBlurRefresh,
}

struct PendingLiveCapture {
    kind: PendingCaptureKind,
    annotations: Vec<Annotation>,
    step_counter: u32,
    live_virt: Option<CapturedScreen>,
    ready_at: std::time::Instant,
}

struct ShotCraftApp {
    settings: AppSettings,
    event_proxy: EventLoopProxy<AppEvent>,
    tray: Option<TrayState>,
    overlay: Option<OverlayWindow>,
    flash: Option<CaptureFlashWindow>,
    settings_win: Option<SettingsWindow>,
    pending_live_capture: Option<PendingLiveCapture>,
    update_state: updater::UpdateState,
    available_release: Option<updater::ReleaseInfo>,
    startup_update_checked: bool,
}

impl ShotCraftApp {
    fn new(event_proxy: EventLoopProxy<AppEvent>) -> Self {
        Self {
            settings: AppSettings::load(),
            event_proxy,
            tray: None,
            overlay: None,
            flash: None,
            settings_win: None,
            pending_live_capture: None,
            update_state: updater::UpdateState::Idle,
            available_release: None,
            startup_update_checked: false,
        }
    }

    fn available_version(&self) -> Option<&str> {
        self.available_release.as_ref().map(|r| r.version.as_str())
    }

    fn sync_update_state(&mut self, state: updater::UpdateState) {
        self.update_state = state.clone();
        if let Some(settings_win) = &mut self.settings_win {
            settings_win.set_update_state(state);
        }
    }

    fn refresh_tray(&mut self) {
        let ver = self
            .available_release
            .as_ref()
            .map(|r| r.version.clone());
        if let Some(tray) = &mut self.tray {
            let _ = tray.apply_settings(&self.settings, ver.as_deref());
        }
    }

    fn spawn_update_check(&mut self, manual: bool) {
        if matches!(
            self.update_state,
            updater::UpdateState::Checking | updater::UpdateState::Downloading(_)
        ) {
            return;
        }
        self.sync_update_state(updater::UpdateState::Checking);
        let proxy = self.event_proxy.clone();
        std::thread::spawn(move || {
            let result = updater::check_for_updates();
            let _ = proxy.send_event(AppEvent::UpdateCheckFinished { result, manual });
        });
    }

    fn spawn_update_install(&mut self) {
        if matches!(self.update_state, updater::UpdateState::Downloading(_)) {
            return;
        }
        let Some(info) = self.available_release.clone() else {
            self.spawn_update_check(true);
            return;
        };
        self.sync_update_state(updater::UpdateState::Downloading(info.version.clone()));
        let proxy = self.event_proxy.clone();
        std::thread::spawn(move || {
            let res = updater::download_and_install(&info);
            let _ = proxy.send_event(AppEvent::UpdateInstallFinished(res));
        });
    }

    fn schedule_live_draw_capture(&mut self, kind: PendingCaptureKind) -> bool {
        let is_live = self.overlay.as_ref().is_some_and(|o| o.is_live_draw());
        if !is_live {
            return false;
        }
        self.save_overlay_tool_prefs();

        if matches!(kind, PendingCaptureKind::LiveBlurRefresh) {
            if let Some(ov) = self.overlay.as_ref() {
                ov.hide_all();
                self.pending_live_capture = Some(PendingLiveCapture {
                    kind,
                    annotations: Vec::new(),
                    step_counter: 1,
                    live_virt: None,
                    ready_at: std::time::Instant::now() + std::time::Duration::from_millis(90),
                });
                return true;
            }
            return false;
        }

        if let Some(ov) = self.overlay.as_mut() {
            let (annotations, step_counter, live_virt) = ov.take_live_draw_state();
            ov.hide_all();
            self.pending_live_capture = Some(PendingLiveCapture {
                kind,
                annotations,
                step_counter,
                live_virt: Some(live_virt),
                ready_at: std::time::Instant::now() + std::time::Duration::from_millis(90),
            });
        }
        self.overlay = None;
        true
    }

    fn capture_instant_fullscreen(&mut self, event_loop: &ActiveEventLoop) {
        if self.pending_live_capture.is_some() {
            return;
        }
        if self.schedule_live_draw_capture(PendingCaptureKind::Fullscreen {
            copy: true,
            save: true,
        }) {
            return;
        }
        self.finish_fullscreen_capture(event_loop, None, true, true);
    }

    fn finish_fullscreen_capture(
        &mut self,
        event_loop: &ActiveEventLoop,
        live_state: Option<(Vec<Annotation>, CapturedScreen)>,
        do_copy: bool,
        do_save: bool,
    ) {
        let t = Strings::for_lang(self.settings.language);
        match capture_primary_screen() {
            Ok(mut screen) => {
                if let Some((mut anns, live_virt)) = live_state {
                    if !anns.is_empty() {
                        if live_virt.physical_coords && screen.physical_coords {
                            let scale_ratio =
                                live_virt.scale_factor / screen.scale_factor.max(0.1);
                            let shift_x =
                                (live_virt.x - screen.x) as f32 / screen.scale_factor.max(0.1);
                            let shift_y =
                                (live_virt.y - screen.y) as f32 / screen.scale_factor.max(0.1);
                            for ann in &mut anns {
                                if (scale_ratio - 1.0).abs() > 1e-3 {
                                    ann.scale_coords(scale_ratio);
                                }
                                ann.translate(egui::Vec2::new(shift_x, shift_y));
                            }
                        } else {
                            let delta = egui::Vec2::new(
                                (live_virt.x - screen.x) as f32,
                                (live_virt.y - screen.y) as f32,
                            );
                            for ann in &mut anns {
                                ann.translate(delta);
                            }
                        }
                        let sel = egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::Vec2::new(screen.logical_width, screen.logical_height),
                        );
                        screen.image =
                            render_export(&screen.image, sel, screen.scale_factor, &anns);
                    }
                }
                if do_copy {
                    if let Err(e) = copy_to_clipboard(&screen.image) {
                        diagnostics::notify_error(
                            "ShotCraft",
                            &format!("{}: {e}", t.err_copy_failed),
                        );
                    }
                }
                if do_save {
                    match save_to_dir_async(screen.image.clone(), &self.settings.save_dir) {
                        Ok(path) => println!("Fullscreen saved: {}", path.display()),
                        Err(e) => {
                            diagnostics::notify_error(
                                "ShotCraft",
                                &format!("{}: {e}", t.err_save_failed),
                            );
                        }
                    }
                }
                let theme = ThemeColors::from_positions(
                    self.settings.bg_color_pos,
                    self.settings.accent_color_pos,
                );
                match CaptureFlashWindow::open(event_loop, screen, theme.accent) {
                    Ok(win) => self.flash = Some(win),
                    Err(e) => diagnostics::log_error("CaptureFlashWindow::open", &e),
                }
            }
            Err(e) => {
                diagnostics::notify_error("ShotCraft", &format!("{}: {e}", t.err_capture_failed));
            }
        }
    }

    fn overlay_config(
        &self,
        screens: Vec<CapturedScreen>,
        windows: Vec<crate::capture::WindowCandidate>,
        window_select_mode: bool,
        live_draw: bool,
        initial_annotations: Vec<Annotation>,
        initial_step_counter: u32,
    ) -> OverlayConfig {
        let theme = ThemeColors::from_positions(
            self.settings.bg_color_pos,
            self.settings.accent_color_pos,
        );
        OverlayConfig {
            screens,
            windows,
            window_select_mode,
            live_draw,
            save_dir: self.settings.save_dir.clone(),
            lang: self.settings.language,
            theme,
            initial_color_pos: self.settings.draw_color_pos,
            initial_thickness: self.settings.draw_thickness,
            snap_angle_deg: self.settings.snap_angle_deg,
            text_font: self.settings.text_font,
            toolbar_layout: self.settings.toolbar_layout,
            initial_tool: self.settings.last_tool,
            initial_annotations,
            initial_step_counter,
        }
    }

    fn start_area_capture(&mut self, event_loop: &ActiveEventLoop) {
        if self.pending_live_capture.is_some() {
            return;
        }
        if self.schedule_live_draw_capture(PendingCaptureKind::Area) {
            return;
        }
        if self.overlay.is_some() {
            return;
        }
        self.open_area_capture(event_loop, Vec::new(), 1);
    }

    fn open_area_capture(
        &mut self,
        event_loop: &ActiveEventLoop,
        initial_anns: Vec<Annotation>,
        initial_step: u32,
    ) {
        let t = Strings::for_lang(self.settings.language);
        match capture_all_monitors() {
            Ok(screens) => {
                let cfg = self.overlay_config(
                    screens,
                    Vec::new(),
                    false,
                    false,
                    initial_anns,
                    initial_step,
                );
                match OverlayWindow::open(event_loop, cfg) {
                    Ok(win) => self.overlay = Some(win),
                    Err(e) => diagnostics::notify_error("ShotCraft", &e),
                }
            }
            Err(e) => {
                diagnostics::notify_error("ShotCraft", &format!("{}: {e}", t.err_capture_failed));
            }
        }
    }

    fn start_window_capture(&mut self, event_loop: &ActiveEventLoop) {
        if self.pending_live_capture.is_some() {
            return;
        }
        if self.schedule_live_draw_capture(PendingCaptureKind::Window) {
            return;
        }
        if self.overlay.is_some() {
            return;
        }
        self.open_window_capture(event_loop, Vec::new(), 1);
    }

    fn open_window_capture(
        &mut self,
        event_loop: &ActiveEventLoop,
        initial_anns: Vec<Annotation>,
        initial_step: u32,
    ) {
        let t = Strings::for_lang(self.settings.language);
        match capture_all_monitors() {
            Ok(screens) => {
                let virt = composite_monitors_metadata(&screens);
                let windows = list_windows_on_screen(&virt);
                let cfg =
                    self.overlay_config(screens, windows, true, false, initial_anns, initial_step);
                match OverlayWindow::open(event_loop, cfg) {
                    Ok(win) => self.overlay = Some(win),
                    Err(e) => diagnostics::notify_error("ShotCraft", &e),
                }
            }
            Err(e) => {
                diagnostics::notify_error("ShotCraft", &format!("{}: {e}", t.err_capture_failed));
            }
        }
    }

    fn save_overlay_tool_prefs(&mut self) {
        if let Some(overlay) = &self.overlay {
            let new_color = overlay.color_pos();
            let new_thick = overlay.thickness();
            let new_tool = overlay.active_tool();
            if (self.settings.draw_color_pos - new_color).abs() > 1e-4
                || (self.settings.draw_thickness - new_thick).abs() > 1e-4
                || self.settings.last_tool != new_tool
            {
                self.settings.draw_color_pos = new_color;
                self.settings.draw_thickness = new_thick;
                self.settings.last_tool = new_tool;
                self.settings.save();
            }
        }
    }

    fn start_live_draw(&mut self, event_loop: &ActiveEventLoop) {
        if self.pending_live_capture.is_some() {
            return;
        }
        if let Some(overlay) = &self.overlay {
            if overlay.is_live_draw() {
                self.save_overlay_tool_prefs();
                self.overlay = None;
            }
            return;
        }
        match list_monitors_as_blank_screens() {
            Ok(screens) => {
                let cfg = self.overlay_config(screens, Vec::new(), false, true, Vec::new(), 1);
                match OverlayWindow::open(event_loop, cfg) {
                    Ok(win) => self.overlay = Some(win),
                    Err(e) => diagnostics::notify_error("ShotCraft", &e),
                }
            }
            Err(e) => diagnostics::notify_error("ShotCraft", &e),
        }
    }

    fn open_settings(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(win) = &self.settings_win {
            win.focus();
            return;
        }
        match SettingsWindow::open(
            event_loop,
            self.settings.language,
            self.event_proxy.clone(),
            self.update_state.clone(),
        ) {
            Ok(win) => self.settings_win = Some(win),
            Err(e) => diagnostics::notify_error("ShotCraft", &e),
        }
    }
}

impl ApplicationHandler<AppEvent> for ShotCraftApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Wait);
        if self.tray.is_none() {
            match build_tray(&self.settings, self.available_version()) {
                Ok(state) => self.tray = Some(state),
                Err(e) => {
                    diagnostics::notify_error("ShotCraft Tray Error", &e);
                    event_loop.exit();
                }
            }
        }
        if !self.startup_update_checked {
            self.startup_update_checked = true;
            let proxy = self.event_proxy.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(4));
                let result = updater::check_for_updates();
                let _ = proxy.send_event(AppEvent::UpdateCheckFinished {
                    result,
                    manual: false,
                });
            });
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::FolderPicked(dir) => {
                let mut changed = false;
                if let Some(settings_win) = &mut self.settings_win {
                    changed = settings_win.on_folder_picked(dir, &mut self.settings);
                }
                if changed {
                    self.refresh_tray();
                }
            }
            AppEvent::TriggerUpdateCheck => {
                self.spawn_update_check(true);
            }
            AppEvent::TriggerUpdateInstall => {
                self.spawn_update_install();
            }
            AppEvent::UpdateCheckFinished { result, manual } => {
                let t = Strings::for_lang(self.settings.language);
                match result {
                    Ok(Some(info)) => {
                        diagnostics::log_info(
                            "updater",
                            &format!("Update available: v{}", info.version),
                        );
                        self.available_release = Some(info.clone());
                        self.sync_update_state(updater::UpdateState::Available(info));
                        self.refresh_tray();
                        if manual && self.settings_win.is_none() {
                            self.open_settings(event_loop);
                        }
                    }
                    Ok(None) => {
                        self.available_release = None;
                        self.sync_update_state(updater::UpdateState::UpToDate);
                        self.refresh_tray();
                        if manual && self.settings_win.is_none() {
                            diagnostics::notify_info(
                                "ShotCraft",
                                &format!("v{} — {}", env!("CARGO_PKG_VERSION"), t.update_up_to_date),
                            );
                        }
                    }
                    Err(err) => {
                        diagnostics::log_error("updater.check", &err);
                        if manual {
                            self.sync_update_state(updater::UpdateState::Failed(err.clone()));
                            if self.settings_win.is_none() {
                                diagnostics::notify_error(
                                    "ShotCraft",
                                    &format!("{}: {err}", t.update_failed),
                                );
                            }
                        } else if matches!(self.update_state, updater::UpdateState::Checking) {
                            self.sync_update_state(updater::UpdateState::Idle);
                        }
                    }
                }
            }
            AppEvent::UpdateInstallFinished(result) => match result {
                Ok(updater::InstallAction::ExitToComplete(ver)) => {
                    diagnostics::log_info(
                        "updater",
                        &format!("Applying v{ver} update and restarting"),
                    );
                    self.sync_update_state(updater::UpdateState::Installed(ver));
                    wait_pending_saves();
                    self.overlay = None;
                    self.settings_win = None;
                    event_loop.exit();
                }
                Ok(updater::InstallAction::OpenedManual(ver)) => {
                    diagnostics::log_info(
                        "updater",
                        &format!("Opened installer for v{ver}"),
                    );
                    self.sync_update_state(updater::UpdateState::OpenedManual(ver));
                }
                Err(err) => {
                    diagnostics::notify_error("ShotCraft Update Failed", &err);
                    self.sync_update_state(updater::UpdateState::Failed(err));
                }
            },
            AppEvent::Menu(ev) => {
                enum MenuChoice {
                    Full,
                    Area,
                    Window,
                    LiveDraw,
                    OpenFolder,
                    Settings,
                    CheckUpdates,
                    Quit,
                    None,
                }
                let choice = if let Some(tray) = &self.tray {
                    if ev.id == tray.id_capture_full {
                        MenuChoice::Full
                    } else if ev.id == tray.id_capture_area {
                        MenuChoice::Area
                    } else if ev.id == tray.id_capture_window {
                        MenuChoice::Window
                    } else if ev.id == tray.id_live_draw {
                        MenuChoice::LiveDraw
                    } else if ev.id == tray.id_open_folder {
                        MenuChoice::OpenFolder
                    } else if ev.id == tray.id_settings {
                        MenuChoice::Settings
                    } else if ev.id == tray.id_check_updates {
                        MenuChoice::CheckUpdates
                    } else if ev.id == tray.id_quit {
                        MenuChoice::Quit
                    } else {
                        MenuChoice::None
                    }
                } else {
                    MenuChoice::None
                };
                match choice {
                    MenuChoice::Full => self.capture_instant_fullscreen(event_loop),
                    MenuChoice::Area => self.start_area_capture(event_loop),
                    MenuChoice::Window => self.start_window_capture(event_loop),
                    MenuChoice::LiveDraw => self.start_live_draw(event_loop),
                    MenuChoice::OpenFolder => open_folder(&self.settings.save_dir),
                    MenuChoice::Settings => self.open_settings(event_loop),
                    MenuChoice::CheckUpdates => {
                        if self.available_release.is_some() {
                            self.spawn_update_install();
                        } else {
                            self.spawn_update_check(true);
                        }
                    }
                    MenuChoice::Quit => event_loop.exit(),
                    MenuChoice::None => {}
                }
            }
            AppEvent::HotKey(ev) => {
                let Some(tray) = &self.tray else { return };
                if ev.state == HotKeyState::Pressed {
                    if ev.id == tray.hk_full_id {
                        self.capture_instant_fullscreen(event_loop);
                    } else if ev.id == tray.hk_area_id {
                        self.start_area_capture(event_loop);
                    } else if ev.id == tray.hk_window_id {
                        self.start_window_capture(event_loop);
                    } else if ev.id == tray.hk_live_draw_id {
                        self.start_live_draw(event_loop);
                    }
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(target_os = "linux")]
        while gtk::events_pending() {
            gtk::main_iteration_do(false);
        }

        if let Some(ready_at) = self.pending_live_capture.as_ref().map(|p| p.ready_at) {
            if std::time::Instant::now() >= ready_at {
                if let Some(pending) = self.pending_live_capture.take() {
                    match pending.kind {
                        PendingCaptureKind::Fullscreen { copy, save } => {
                            let live_state = pending
                                .live_virt
                                .map(|virt| (pending.annotations, virt));
                            self.finish_fullscreen_capture(event_loop, live_state, copy, save);
                        }
                        PendingCaptureKind::Area => {
                            self.open_area_capture(
                                event_loop,
                                pending.annotations,
                                pending.step_counter,
                            );
                        }
                        PendingCaptureKind::Window => {
                            self.open_window_capture(
                                event_loop,
                                pending.annotations,
                                pending.step_counter,
                            );
                        }
                        PendingCaptureKind::LiveBlurRefresh => {
                            let t = Strings::for_lang(self.settings.language);
                            match capture_all_monitors() {
                                Ok(screens) => {
                                    if let Some(ov) = &mut self.overlay {
                                        ov.set_live_background(screens);
                                        ov.show_all();
                                    }
                                }
                                Err(e) => {
                                    if let Some(ov) = &self.overlay {
                                        ov.show_all();
                                    }
                                    diagnostics::notify_error(
                                        "ShotCraft",
                                        &format!("{}: {e}", t.err_capture_failed),
                                    );
                                }
                            }
                        }
                    }
                }
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(ready_at));
                return;
            }
        }

        if let Some(flash) = &self.flash {
            flash.request_redraw();
            event_loop.set_control_flow(ControlFlow::Poll);
        } else {
            #[cfg(target_os = "linux")]
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                std::time::Instant::now() + std::time::Duration::from_millis(120),
            ));
            #[cfg(not(target_os = "linux"))]
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(flash) = &mut self.flash {
            if flash.window_id() == window_id {
                if flash.handle_window_event(&event) {
                    self.flash = None;
                }
                return;
            }
        }

        if let Some(overlay) = &mut self.overlay {
            if overlay.has_window_id(window_id) {
                match overlay.handle_window_event(window_id, &event) {
                    OverlayOutcome::KeepOpen => {}
                    OverlayOutcome::Close => {
                        self.save_overlay_tool_prefs();
                        self.overlay = None;
                    }
                    OverlayOutcome::LiveCopy => {
                        self.schedule_live_draw_capture(PendingCaptureKind::Fullscreen {
                            copy: true,
                            save: false,
                        });
                    }
                    OverlayOutcome::LiveSave => {
                        self.schedule_live_draw_capture(PendingCaptureKind::Fullscreen {
                            copy: false,
                            save: true,
                        });
                    }
                    OverlayOutcome::CaptureLiveBlurBackground => {
                        self.schedule_live_draw_capture(PendingCaptureKind::LiveBlurRefresh);
                    }
                }
                return;
            }
        }

        if let Some(settings_win) = &mut self.settings_win {
            if settings_win.window_id() == window_id {
                match settings_win.handle_window_event(&event, &mut self.settings) {
                    SettingsOutcome::KeepOpen => {}
                    SettingsOutcome::Updated => {
                        self.refresh_tray();
                    }
                    SettingsOutcome::Close => {
                        self.settings_win = None;
                    }
                }
            }
        }
    }
}

fn run_smoke_test() -> Result<(), String> {
    // 1. Settings hotkey serialize/deserialize roundtrip
    let hk = crate::settings::HotkeySpec {
        ctrl: true,
        alt: true,
        shift: true,
        super_key: false,
        key: crate::settings::HotkeyCode::KeyS,
    };
    let parsed_hk = crate::settings::HotkeySpec::deserialize(&hk.serialize())
        .ok_or_else(|| "Hotkey roundtrip failed".to_string())?;
    if parsed_hk != hk {
        return Err("Hotkey mismatch after roundtrip".to_string());
    }

    // 2. Updater release manifest parser check
    let sample = r#"{"tag_name":"v9.9.9","html_url":"https://github.com/Byakatris/ShotCraft/releases/tag/v9.9.9","assets":[{"name":"ShotCraft-v9.9.dmg","size":2048,"browser_download_url":"https://github.com/Byakatris/ShotCraft/releases/download/v9.9.9/ShotCraft-v9.9.dmg"},{"name":"ShotCraft-v9.9-Setup.exe","size":2048,"browser_download_url":"https://github.com/Byakatris/ShotCraft/releases/download/v9.9.9/ShotCraft-v9.9-Setup.exe"},{"name":"ShotCraft-v9.9-linux-x86_64.tar.gz","size":2048,"browser_download_url":"https://github.com/Byakatris/ShotCraft/releases/download/v9.9.9/ShotCraft-v9.9-linux-x86_64.tar.gz"}]}"#;
    let rel = updater::parse_github_release_json(sample, updater::current_platform_target())?;
    if !updater::is_newer_version(updater::CURRENT_VERSION, &rel.version) {
        return Err("Semver check failed".to_string());
    }

    // 3. Annotation pipeline + PNG fast encoding into temp file
    let src = image::RgbaImage::from_pixel(160, 120, image::Rgba([24, 32, 48, 255]));
    let anns = vec![
        Annotation::Arrow {
            start: egui::Pos2::new(10.0, 60.0),
            end: egui::Pos2::new(90.0, 60.0),
            color: egui::Color32::from_rgb(255, 70, 70),
            thickness: 4.0,
        },
        Annotation::Blur {
            rect: egui::Rect::from_min_max(
                egui::Pos2::new(95.0, 20.0),
                egui::Pos2::new(145.0, 70.0),
            ),
            radius: 6.0,
            pixelate: true,
        },
        Annotation::Step {
            pos: egui::Pos2::new(130.0, 95.0),
            number: 1,
            color: egui::Color32::from_rgb(80, 200, 180),
            radius: 12.0,
        },
    ];
    let rendered = render_export(
        &src,
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(160.0, 120.0)),
        1.0,
        &anns,
    );
    let smoke_png = std::env::temp_dir().join(format!("shotcraft_smoke_{}.png", std::process::id()));
    let f = std::fs::File::create(&smoke_png).map_err(|e| e.to_string())?;
    crate::capture::encode_png_fast(&rendered, f)?;
    let _ = std::fs::remove_file(&smoke_png);
    Ok(())
}

fn main() {
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("ShotCraft v{}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--smoke-test" => match run_smoke_test() {
                Ok(()) => {
                    println!("ShotCraft v{} smoke-test OK", env!("CARGO_PKG_VERSION"));
                    return;
                }
                Err(e) => {
                    eprintln!("ShotCraft smoke-test FAILED: {e}");
                    std::process::exit(1);
                }
            },
            _ => {}
        }
    }

    #[cfg(target_os = "linux")]
    let _ = gtk::init();

    let mut builder = EventLoop::<AppEvent>::with_user_event();
    #[cfg(target_os = "macos")]
    builder.with_activation_policy(ActivationPolicy::Accessory);
    let event_loop = match builder.build() {
        Ok(el) => el,
        Err(e) => {
            diagnostics::notify_error("ShotCraft", &format!("Failed to create event loop: {e}"));
            return;
        }
    };

    let proxy_menu = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |ev| {
        let _ = proxy_menu.send_event(AppEvent::Menu(ev));
    }));

    let proxy_hk = event_loop.create_proxy();
    GlobalHotKeyEvent::set_event_handler(Some(move |ev| {
        let _ = proxy_hk.send_event(AppEvent::HotKey(ev));
    }));

    let mut app = ShotCraftApp::new(event_loop.create_proxy());
    if let Err(e) = event_loop.run_app(&mut app) {
        diagnostics::log_error("event_loop", &format!("Event loop error: {e}"));
    }
    wait_pending_saves();
}
