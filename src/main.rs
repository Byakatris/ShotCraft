#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod annotations;
mod capture;
mod icons;
mod material;
mod overlay;
mod settings;
mod settings_ui;
mod tray;

use global_hotkey::{GlobalHotKeyEvent, HotKeyState};
use muda::MenuEvent;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
#[cfg(target_os = "macos")]
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
use winit::window::WindowId;

use crate::annotations::{render_export, Annotation};
use crate::capture::{
    capture_all_monitors, capture_primary_screen, composite_monitors, copy_to_clipboard,
    list_monitors_as_blank_screens, list_windows_on_screen, open_folder, save_to_dir,
};
use crate::material::ThemeColors;
use crate::overlay::{CaptureFlashWindow, OverlayOutcome, OverlayWindow};
use crate::settings::AppSettings;
use crate::settings_ui::{SettingsOutcome, SettingsWindow};
use crate::tray::{build_tray, TrayState};

enum AppEvent {
    Menu(MenuEvent),
    HotKey(GlobalHotKeyEvent),
}

#[derive(Clone, Copy)]
enum PendingCaptureKind {
    Fullscreen { copy: bool, save: bool },
    Area,
    Window,
}

struct PendingLiveCapture {
    kind: PendingCaptureKind,
    annotations: Vec<Annotation>,
    step_counter: u32,
    vx: i32,
    vy: i32,
    ready_at: std::time::Instant,
}

struct ShotCraftApp {
    settings: AppSettings,
    tray: Option<TrayState>,
    overlay: Option<OverlayWindow>,
    flash: Option<CaptureFlashWindow>,
    settings_win: Option<SettingsWindow>,
    pending_live_capture: Option<PendingLiveCapture>,
}

impl ShotCraftApp {
    fn new() -> Self {
        Self {
            settings: AppSettings::load(),
            tray: None,
            overlay: None,
            flash: None,
            settings_win: None,
            pending_live_capture: None,
        }
    }

    fn schedule_live_draw_capture(&mut self, kind: PendingCaptureKind) -> bool {
        let is_live = self.overlay.as_ref().is_some_and(|o| o.is_live_draw());
        if !is_live {
            return false;
        }
        self.save_overlay_tool_prefs();
        if let Some(ov) = self.overlay.as_mut() {
            let (annotations, step_counter, vx, vy) = ov.take_live_draw_state();
            ov.hide_all();
            self.pending_live_capture = Some(PendingLiveCapture {
                kind,
                annotations,
                step_counter,
                vx,
                vy,
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
        live_state: Option<(Vec<Annotation>, i32, i32)>,
        do_copy: bool,
        do_save: bool,
    ) {
        match capture_primary_screen() {
            Ok(mut screen) => {
                if let Some((mut anns, vx, vy)) = live_state {
                    if !anns.is_empty() {
                        let delta = egui::Vec2::new((vx - screen.x) as f32, (vy - screen.y) as f32);
                        for ann in &mut anns {
                            ann.translate(delta);
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
                        eprintln!("Clipboard copy failed: {e}");
                    }
                }
                if do_save {
                    match save_to_dir(&screen.image, &self.settings.save_dir) {
                        Ok(path) => println!("Fullscreen saved: {}", path.display()),
                        Err(e) => eprintln!("Save failed: {e}"),
                    }
                }
                let theme = ThemeColors::from_positions(
                    self.settings.bg_color_pos,
                    self.settings.accent_color_pos,
                );
                if let Ok(win) = CaptureFlashWindow::open(event_loop, screen, theme.accent) {
                    self.flash = Some(win);
                }
            }
            Err(e) => eprintln!("Fullscreen capture error: {e}"),
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
        let theme = ThemeColors::from_positions(
            self.settings.bg_color_pos,
            self.settings.accent_color_pos,
        );
        match capture_all_monitors() {
            Ok(screens) => match OverlayWindow::open(
                event_loop,
                screens,
                Vec::new(),
                false,
                false,
                self.settings.save_dir.clone(),
                self.settings.language,
                theme,
                self.settings.draw_color_pos,
                self.settings.draw_thickness,
                self.settings.snap_angle_deg,
                self.settings.text_font,
                self.settings.toolbar_layout,
                self.settings.last_tool,
                initial_anns,
                initial_step,
            ) {
                Ok(win) => self.overlay = Some(win),
                Err(e) => eprintln!("Failed to open area overlay: {e}"),
            },
            Err(e) => eprintln!("Capture error: {e}"),
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
        let theme = ThemeColors::from_positions(
            self.settings.bg_color_pos,
            self.settings.accent_color_pos,
        );
        match capture_all_monitors() {
            Ok(screens) => {
                let virt = composite_monitors(&screens);
                let windows = list_windows_on_screen(&virt);
                match OverlayWindow::open(
                    event_loop,
                    screens,
                    windows,
                    true,
                    false,
                    self.settings.save_dir.clone(),
                    self.settings.language,
                    theme,
                    self.settings.draw_color_pos,
                    self.settings.draw_thickness,
                    self.settings.snap_angle_deg,
                    self.settings.text_font,
                    self.settings.toolbar_layout,
                    self.settings.last_tool,
                    initial_anns,
                    initial_step,
                ) {
                    Ok(win) => self.overlay = Some(win),
                    Err(e) => eprintln!("Failed to open window overlay: {e}"),
                }
            }
            Err(e) => eprintln!("Capture error: {e}"),
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
        let theme = ThemeColors::from_positions(
            self.settings.bg_color_pos,
            self.settings.accent_color_pos,
        );
        match list_monitors_as_blank_screens() {
            Ok(screens) => match OverlayWindow::open(
                event_loop,
                screens,
                Vec::new(),
                false,
                true,
                self.settings.save_dir.clone(),
                self.settings.language,
                theme,
                self.settings.draw_color_pos,
                self.settings.draw_thickness,
                self.settings.snap_angle_deg,
                self.settings.text_font,
                self.settings.toolbar_layout,
                self.settings.last_tool,
                Vec::new(),
                1,
            ) {
                Ok(win) => self.overlay = Some(win),
                Err(e) => eprintln!("Failed to open live draw overlay: {e}"),
            },
            Err(e) => eprintln!("Monitor list error: {e}"),
        }
    }

    fn open_settings(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(win) = &self.settings_win {
            win.focus();
            return;
        }
        match SettingsWindow::open(event_loop) {
            Ok(win) => self.settings_win = Some(win),
            Err(e) => eprintln!("Failed to open settings window: {e}"),
        }
    }
}

impl ApplicationHandler<AppEvent> for ShotCraftApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Wait);
        if self.tray.is_none() {
            match build_tray(&self.settings) {
                Ok(state) => self.tray = Some(state),
                Err(e) => {
                    eprintln!("Fatal: {e}");
                    event_loop.exit();
                }
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        let Some(tray) = &self.tray else { return };
        match event {
            AppEvent::Menu(ev) => {
                if ev.id == tray.id_capture_full {
                    self.capture_instant_fullscreen(event_loop);
                } else if ev.id == tray.id_capture_area {
                    self.start_area_capture(event_loop);
                } else if ev.id == tray.id_capture_window {
                    self.start_window_capture(event_loop);
                } else if ev.id == tray.id_live_draw {
                    self.start_live_draw(event_loop);
                } else if ev.id == tray.id_open_folder {
                    open_folder(&self.settings.save_dir);
                } else if ev.id == tray.id_settings {
                    self.open_settings(event_loop);
                } else if ev.id == tray.id_quit {
                    event_loop.exit();
                }
            }
            AppEvent::HotKey(ev) => {
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
        if let Some(pending) = &self.pending_live_capture {
            if std::time::Instant::now() >= pending.ready_at {
                let pending = self.pending_live_capture.take().unwrap();
                match pending.kind {
                    PendingCaptureKind::Fullscreen { copy, save } => {
                        self.finish_fullscreen_capture(
                            event_loop,
                            Some((pending.annotations, pending.vx, pending.vy)),
                            copy,
                            save,
                        );
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
                }
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(pending.ready_at));
                return;
            }
        }
        if let Some(flash) = &self.flash {
            flash.request_redraw();
            event_loop.set_control_flow(ControlFlow::Poll);
        } else {
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
                }
                return;
            }
        }

        if let Some(settings_win) = &mut self.settings_win {
            if settings_win.window_id() == window_id {
                match settings_win.handle_window_event(&event, &mut self.settings) {
                    SettingsOutcome::KeepOpen => {}
                    SettingsOutcome::Updated => {
                        if let Some(tray) = &mut self.tray {
                            let _ = tray.apply_settings(&self.settings);
                        }
                    }
                    SettingsOutcome::Close => {
                        self.settings_win = None;
                    }
                }
            }
        }
    }
}

fn main() {
    let mut builder = EventLoop::<AppEvent>::with_user_event();
    #[cfg(target_os = "macos")]
    builder.with_activation_policy(ActivationPolicy::Accessory);
    let event_loop = builder.build().expect("Failed to create event loop");

    let proxy_menu = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |ev| {
        let _ = proxy_menu.send_event(AppEvent::Menu(ev));
    }));

    let proxy_hk = event_loop.create_proxy();
    GlobalHotKeyEvent::set_event_handler(Some(move |ev| {
        let _ = proxy_hk.send_event(AppEvent::HotKey(ev));
    }));

    let mut app = ShotCraftApp::new();
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("Event loop error: {e}");
    }
}
