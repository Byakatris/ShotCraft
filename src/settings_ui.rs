use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{CornerRadius, Pos2, Rect, Stroke, Vec2};
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::keyboard::{ModifiersState, PhysicalKey};
use winit::window::{WindowAttributes, WindowId};

use crate::annotations::ROBOTO_FONT_BYTES;
use crate::capture::pick_folder_dialog;
use crate::diagnostics;
use crate::gl_window::{GlEguiWindow, WindowChrome};
use crate::material::{
    m3_accent_slider, m3_bg_slider, morph_radius, spring_animate, spring_scale, ThemeColors,
};
use crate::settings::{
    default_desktop_dir, AppSettings, HotkeyCode, HotkeySlot, HotkeySpec, Language, Strings,
    TextFont, ToolbarLayout,
};
use crate::updater::UpdateState;
use crate::AppEvent;

pub enum SettingsOutcome {
    KeepOpen,
    Updated,
    Close,
}

pub struct SettingsWindow {
    gl_win: GlEguiWindow,
    event_proxy: EventLoopProxy<AppEvent>,
    modifiers: ModifiersState,
    recording: Option<HotkeySlot>,
    hotkey_warning: Option<(String, Instant)>,
    picking_folder: bool,
    lang_dropdown_open: bool,
    font_dropdown_open: bool,
    angle_editing: bool,
    angle_buf: String,
    angle_drag_accum: f32,
    update_state: UpdateState,
}

impl SettingsWindow {
    pub fn open(
        event_loop: &ActiveEventLoop,
        lang: Language,
        event_proxy: EventLoopProxy<AppEvent>,
        update_state: UpdateState,
    ) -> Result<Self, String> {
        let t = Strings::for_lang(lang);
        let title = format!("ShotCraft v{} — {}", env!("CARGO_PKG_VERSION"), t.settings_title);
        let height = if cfg!(target_os = "linux")
            && std::env::var("XDG_SESSION_TYPE").ok().as_deref() == Some("wayland")
        {
            778.0
        } else {
            758.0
        };
        let win_attrs = WindowAttributes::default()
            .with_title(title)
            .with_resizable(false)
            .with_inner_size(LogicalSize::new(500.0, height));

        let gl_win = GlEguiWindow::open(
            event_loop,
            win_attrs,
            WindowChrome::Standard,
            false,
            configure_m3_fonts,
        )?;
        gl_win.window.focus_window();
        gl_win.window.request_redraw();

        Ok(Self {
            gl_win,
            event_proxy,
            modifiers: ModifiersState::empty(),
            recording: None,
            hotkey_warning: None,
            picking_folder: false,
            lang_dropdown_open: false,
            font_dropdown_open: false,
            angle_editing: false,
            angle_buf: String::new(),
            angle_drag_accum: 0.0,
            update_state,
        })
    }

    pub fn set_update_state(&mut self, state: UpdateState) {
        self.update_state = state;
        self.gl_win.window.request_redraw();
    }

    pub fn window_id(&self) -> WindowId {
        self.gl_win.window_id()
    }

    pub fn focus(&self) {
        self.gl_win.window.set_minimized(false);
        self.gl_win.window.focus_window();
    }

    pub fn on_folder_picked(&mut self, dir: Option<PathBuf>, settings: &mut AppSettings) -> bool {
        self.picking_folder = false;
        let updated = if let Some(d) = dir {
            settings.save_dir = d;
            settings.save();
            true
        } else {
            false
        };
        self.gl_win.window.request_redraw();
        updated
    }

    pub fn handle_window_event(
        &mut self,
        event: &WindowEvent,
        settings: &mut AppSettings,
    ) -> SettingsOutcome {
        if let WindowEvent::CloseRequested = event {
            return SettingsOutcome::Close;
        }

        if let WindowEvent::ModifiersChanged(mods) = event {
            self.modifiers = mods.state();
        }

        if let Some(slot) = self.recording {
            if let WindowEvent::KeyboardInput { event: key_ev, .. } = event {
                if key_ev.state == ElementState::Pressed {
                    if let PhysicalKey::Code(winit::keyboard::KeyCode::Escape) = key_ev.physical_key
                    {
                        self.recording = None;
                        self.gl_win.window.request_redraw();
                        return SettingsOutcome::KeepOpen;
                    }
                    if let PhysicalKey::Code(code) = key_ev.physical_key {
                        if let Some(hk_code) = HotkeyCode::from_winit(code) {
                            let t = Strings::for_lang(settings.language);
                            let spec = HotkeySpec {
                                ctrl: self.modifiers.control_key(),
                                alt: self.modifiers.alt_key(),
                                shift: self.modifiers.shift_key(),
                                super_key: self.modifiers.super_key(),
                                key: hk_code,
                            };
                            if !spec.has_modifier() {
                                self.hotkey_warning =
                                    Some((t.hotkey_need_modifier.to_string(), Instant::now()));
                                self.recording = None;
                                self.gl_win.window.request_redraw();
                                return SettingsOutcome::KeepOpen;
                            }
                            if settings.hotkey_conflict(slot, spec).is_some() {
                                self.hotkey_warning =
                                    Some((t.hotkey_conflict.to_string(), Instant::now()));
                                self.recording = None;
                                self.gl_win.window.request_redraw();
                                return SettingsOutcome::KeepOpen;
                            }
                            match slot {
                                HotkeySlot::Fullscreen => settings.hk_fullscreen = spec,
                                HotkeySlot::Area => settings.hk_area = spec,
                                HotkeySlot::Window => settings.hk_window = spec,
                                HotkeySlot::LiveDraw => settings.hk_live_draw = spec,
                            }
                            settings.save();
                            self.hotkey_warning = None;
                            self.recording = None;
                            self.gl_win.window.request_redraw();
                            return SettingsOutcome::Updated;
                        }
                    }
                }
            }
        }

        if let WindowEvent::Focused(true) = event {
            self.gl_win.window.request_redraw();
        }

        if self.gl_win.on_window_event(event) {
            self.gl_win.window.request_redraw();
        }

        if matches!(event, WindowEvent::RedrawRequested) {
            return self.draw_frame(settings);
        }

        SettingsOutcome::KeepOpen
    }

    fn draw_frame(&mut self, settings: &mut AppSettings) -> SettingsOutcome {
        self.gl_win.make_current();
        if let Some((_, ts)) = &self.hotkey_warning {
            if ts.elapsed() > Duration::from_secs(4) {
                self.hotkey_warning = None;
            }
        }

        let raw_input = self.gl_win.take_egui_input();
        let egui_ctx = self.gl_win.egui_ctx.clone();
        let mut updated = false;
        let mut lang_changed = false;

        let full_output = egui_ctx.run(raw_input, |ctx| {
            let t = Strings::for_lang(settings.language);
            let theme =
                ThemeColors::from_positions(settings.bg_color_pos, settings.accent_color_pos);
            let mut dropdown_anchor = Rect::NOTHING;
            let mut font_anchor = Rect::NOTHING;
            let prev_recording = self.recording;

            egui::CentralPanel::default()
                .frame(
                    egui::Frame::NONE
                        .fill(theme.surface)
                        .inner_margin(egui::Margin::same(18)),
                )
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(
                        egui::RichText::new(t.settings_title)
                            .size(20.0)
                            .color(theme.on_surface)
                            .strong(),
                    );
                    ui.add_space(12.0);

                    // 1. Interface Card: Language, Text Font, Toolbar Orientation
                    m3_card(ui, &theme, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(t.settings_lang)
                                    .size(14.0)
                                    .color(theme.on_surface),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let arrow = if self.lang_dropdown_open { "^" } else { "v" };
                                let label = format!("{}  {}", settings.language.label(), arrow);
                                let (rect, resp) = draw_dropdown_trigger(
                                    ui,
                                    &label,
                                    13.0,
                                    self.lang_dropdown_open,
                                    &theme,
                                );
                                dropdown_anchor = rect;
                                if resp.clicked() {
                                    self.lang_dropdown_open = !self.lang_dropdown_open;
                                    self.font_dropdown_open = false;
                                }
                            });
                        });
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(t.settings_text_font)
                                    .size(13.5)
                                    .color(theme.on_surface),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let arrow = if self.font_dropdown_open { "^" } else { "v" };
                                let label = format!("{}  {}", settings.text_font.label(), arrow);
                                let (rect, resp) = draw_dropdown_trigger(
                                    ui,
                                    &label,
                                    12.5,
                                    self.font_dropdown_open,
                                    &theme,
                                );
                                font_anchor = rect;
                                if resp.clicked() {
                                    self.font_dropdown_open = !self.font_dropdown_open;
                                    self.lang_dropdown_open = false;
                                }
                            });
                        });
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(t.settings_toolbar_layout)
                                    .size(13.5)
                                    .color(theme.on_surface),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let (seg_rect, _) = ui.allocate_exact_size(
                                    Vec2::new(186.0, 30.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(
                                    seg_rect,
                                    15.0,
                                    theme.surface_container_high,
                                );
                                ui.painter().rect_stroke(
                                    seg_rect,
                                    15.0,
                                    Stroke::new(1.0_f32, theme.outline),
                                    egui::StrokeKind::Inside,
                                );

                                let is_vert = settings.toolbar_layout == ToolbarLayout::Vertical;
                                let slide_t = spring_animate(
                                    ui.ctx(),
                                    egui::Id::new("settings_layout_slide"),
                                    if is_vert { 1.0 } else { 0.0 },
                                    0.26,
                                    0.72,
                                )
                                .clamp(-0.04, 1.04);
                                let half_w = seg_rect.width() * 0.5;
                                let left_rect = Rect::from_min_size(
                                    seg_rect.min,
                                    Vec2::new(half_w, seg_rect.height()),
                                );
                                let right_rect = Rect::from_min_size(
                                    Pos2::new(seg_rect.left() + half_w, seg_rect.top()),
                                    Vec2::new(half_w, seg_rect.height()),
                                );

                                let resp_h = ui.interact(
                                    left_rect,
                                    egui::Id::new("seg_horiz"),
                                    egui::Sense::click(),
                                );
                                let resp_v = ui.interact(
                                    right_rect,
                                    egui::Id::new("seg_vert"),
                                    egui::Sense::click(),
                                );

                                let pill_x = seg_rect.left() + 2.0 + slide_t * half_w;
                                let pill_rect = Rect::from_min_size(
                                    Pos2::new(pill_x, seg_rect.top() + 2.0),
                                    Vec2::new(half_w - 4.0, seg_rect.height() - 4.0),
                                );
                                let active_resp = if is_vert { &resp_v } else { &resp_h };
                                let pill_scale = spring_scale(
                                    ui.ctx(),
                                    egui::Id::new("seg_pill_spring"),
                                    active_resp.hovered(),
                                    active_resp.is_pointer_button_down_on(),
                                );
                                let pill_draw = Rect::from_center_size(
                                    pill_rect.center(),
                                    pill_rect.size() * pill_scale,
                                );
                                ui.painter().rect_filled(pill_draw, 7.0, theme.accent);

                                let h_col = if !is_vert {
                                    theme.on_accent
                                } else {
                                    theme.on_surface
                                };
                                let v_col = if is_vert {
                                    theme.on_accent
                                } else {
                                    theme.on_surface
                                };
                                ui.painter().text(
                                    left_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    t.layout_horizontal,
                                    egui::FontId::proportional(12.0),
                                    h_col,
                                );
                                ui.painter().text(
                                    right_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    t.layout_vertical,
                                    egui::FontId::proportional(12.0),
                                    v_col,
                                );

                                if resp_h.clicked() {
                                    settings.toolbar_layout = ToolbarLayout::Horizontal;
                                    settings.save();
                                    updated = true;
                                }
                                if resp_v.clicked() {
                                    settings.toolbar_layout = ToolbarLayout::Vertical;
                                    settings.save();
                                    updated = true;
                                }
                            });
                        });
                    });

                    ui.add_space(10.0);

                    // 2. Theme Colors & Shift Angle Card
                    m3_card(ui, &theme, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(t.settings_bg_color)
                                    .size(13.5)
                                    .color(theme.on_surface),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if m3_bg_slider(ui, &mut settings.bg_color_pos, &theme) {
                                    settings.save();
                                    updated = true;
                                }
                            });
                        });
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(t.settings_accent_color)
                                    .size(13.5)
                                    .color(theme.on_surface),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if m3_accent_slider(ui, &mut settings.accent_color_pos, &theme) {
                                    settings.save();
                                    updated = true;
                                }
                            });
                        });
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(t.settings_shift_angle)
                                    .size(13.5)
                                    .color(theme.on_surface),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if angle_step_widget(
                                    ui,
                                    &mut settings.snap_angle_deg,
                                    &mut self.angle_editing,
                                    &mut self.angle_buf,
                                    &mut self.angle_drag_accum,
                                    t.settings_angle_hint,
                                    &theme,
                                ) {
                                    settings.save();
                                    updated = true;
                                }
                            });
                        });
                    });

                    ui.add_space(10.0);

                    // 3. Save Folder Card
                    m3_card(ui, &theme, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(t.settings_folder)
                                    .size(13.5)
                                    .color(theme.on_surface),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if m3_pill_btn(
                                    ui,
                                    t.settings_choose,
                                    !self.picking_folder,
                                    &theme,
                                ) && !self.picking_folder
                                {
                                    self.picking_folder = true;
                                    let proxy = self.event_proxy.clone();
                                    std::thread::spawn(move || {
                                        let dir = pick_folder_dialog();
                                        let _ = proxy.send_event(AppEvent::FolderPicked(dir));
                                    });
                                }
                            });
                        });
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            let path_str = settings.save_dir.display().to_string();
                            let display_path = if path_str.chars().count() > 30 {
                                let tail: String = path_str
                                    .chars()
                                    .rev()
                                    .take(28)
                                    .collect::<Vec<_>>()
                                    .into_iter()
                                    .rev()
                                    .collect();
                                format!("…{tail}")
                            } else {
                                path_str.clone()
                            };
                            ui.label(
                                egui::RichText::new(display_path)
                                    .size(12.0)
                                    .color(theme.accent),
                            )
                            .on_hover_text(path_str);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if m3_pill_btn(ui, t.settings_desktop, false, &theme) {
                                    settings.save_dir = default_desktop_dir();
                                    settings.save();
                                    updated = true;
                                }
                            });
                        });
                    });

                    ui.add_space(10.0);

                    // 4. Hotkeys Card
                    m3_card(ui, &theme, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(t.settings_hotkeys)
                                    .size(13.5)
                                    .color(theme.on_surface),
                            );
                            if let Some((warn, _)) = &self.hotkey_warning {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new(warn)
                                                .size(11.5)
                                                .color(egui::Color32::from_rgb(255, 130, 130)),
                                        );
                                    },
                                );
                            }
                        });
                        ui.add_space(10.0);

                        hotkey_row(
                            ui,
                            t.hk_full,
                            settings.hk_fullscreen,
                            self.recording == Some(HotkeySlot::Fullscreen),
                            t.hk_press,
                            &theme,
                            || {
                                self.hotkey_warning = None;
                                self.recording = Some(HotkeySlot::Fullscreen);
                            },
                        );
                        ui.add_space(8.0);
                        hotkey_row(
                            ui,
                            t.hk_area,
                            settings.hk_area,
                            self.recording == Some(HotkeySlot::Area),
                            t.hk_press,
                            &theme,
                            || {
                                self.hotkey_warning = None;
                                self.recording = Some(HotkeySlot::Area);
                            },
                        );
                        ui.add_space(8.0);
                        hotkey_row(
                            ui,
                            t.hk_window,
                            settings.hk_window,
                            self.recording == Some(HotkeySlot::Window),
                            t.hk_press,
                            &theme,
                            || {
                                self.hotkey_warning = None;
                                self.recording = Some(HotkeySlot::Window);
                            },
                        );
                        ui.add_space(8.0);
                        hotkey_row(
                            ui,
                            t.hk_live_draw,
                            settings.hk_live_draw,
                            self.recording == Some(HotkeySlot::LiveDraw),
                            t.hk_press,
                            &theme,
                            || {
                                self.hotkey_warning = None;
                                self.recording = Some(HotkeySlot::LiveDraw);
                            },
                        );

                        let muted = egui::Color32::from_rgba_unmultiplied(
                            theme.on_surface.r(),
                            theme.on_surface.g(),
                            theme.on_surface.b(),
                            175,
                        );
                        if cfg!(target_os = "linux")
                            && std::env::var("XDG_SESSION_TYPE").ok().as_deref() == Some("wayland")
                        {
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new(t.wayland_hotkeys_note)
                                    .size(11.0)
                                    .color(muted),
                            );
                        }
                    });

                    ui.add_space(10.0);

                    // 5. Updates & Diagnostics Card
                    m3_card(ui, &theme, |ui| {
                        let muted = egui::Color32::from_rgba_unmultiplied(
                            theme.on_surface.r(),
                            theme.on_surface.g(),
                            theme.on_surface.b(),
                            175,
                        );
                        let (status_text, status_col, hover_err) = match &self.update_state {
                            UpdateState::Idle => (
                                format!("v{}", env!("CARGO_PKG_VERSION")),
                                muted,
                                None,
                            ),
                            UpdateState::Checking => (
                                t.update_checking.to_string(),
                                muted,
                                None,
                            ),
                            UpdateState::UpToDate => (
                                format!("{} (v{})", t.update_up_to_date, env!("CARGO_PKG_VERSION")),
                                theme.accent,
                                None,
                            ),
                            UpdateState::Available(info) => (
                                format!("{}: v{}", t.update_available, info.version),
                                theme.accent,
                                Some(info.html_url.clone()),
                            ),
                            UpdateState::Downloading(ver) => (
                                format!("{} v{ver}", t.update_downloading),
                                theme.accent,
                                None,
                            ),
                            UpdateState::Installed(ver) => (
                                format!("{}: v{ver}", t.update_available),
                                theme.accent,
                                None,
                            ),
                            UpdateState::OpenedManual(ver) => (
                                format!("{} (v{ver})", t.update_opened),
                                theme.accent,
                                None,
                            ),
                            UpdateState::Failed(err) => (
                                t.update_failed.to_string(),
                                egui::Color32::from_rgb(255, 130, 130),
                                Some(err.clone()),
                            ),
                        };

                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{}  v{}",
                                        t.settings_updates,
                                        env!("CARGO_PKG_VERSION")
                                    ))
                                    .size(13.5)
                                    .color(theme.on_surface),
                                );
                                ui.add_space(2.0);
                                let clickable_link = matches!(
                                    self.update_state,
                                    UpdateState::Available(_) | UpdateState::Failed(_)
                                );
                                let status_resp = ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(status_text)
                                            .size(11.5)
                                            .color(status_col),
                                    )
                                    .sense(if clickable_link {
                                        egui::Sense::click()
                                    } else {
                                        egui::Sense::hover()
                                    }),
                                );
                                if clickable_link {
                                    let resp = if let Some(tip) = hover_err {
                                        status_resp
                                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                                            .on_hover_text(tip)
                                    } else {
                                        status_resp.on_hover_cursor(egui::CursorIcon::PointingHand)
                                    };
                                    if resp.clicked() {
                                        crate::updater::open_releases_page();
                                    }
                                } else if let Some(tip) = hover_err {
                                    status_resp.on_hover_text(tip);
                                }
                            });

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if m3_sized_pill_btn(
                                    ui,
                                    74.0,
                                    t.settings_open_log,
                                    false,
                                    true,
                                    &theme,
                                ) {
                                    diagnostics::open_log_dir();
                                }
                                ui.add_space(8.0);
                                let busy = matches!(
                                    self.update_state,
                                    UpdateState::Checking | UpdateState::Downloading(_)
                                );
                                match &self.update_state {
                                    UpdateState::Available(info) => {
                                        if m3_sized_pill_btn(
                                            ui,
                                            118.0,
                                            t.update_install,
                                            true,
                                            true,
                                            &theme,
                                        ) {
                                            self.update_state =
                                                UpdateState::Downloading(info.version.clone());
                                            let _ = self
                                                .event_proxy
                                                .send_event(AppEvent::TriggerUpdateInstall);
                                        }
                                    }
                                    _ => {
                                        let btn_text = if matches!(
                                            self.update_state,
                                            UpdateState::Downloading(_)
                                        ) {
                                            t.update_downloading
                                        } else if matches!(self.update_state, UpdateState::Checking)
                                        {
                                            t.update_checking
                                        } else {
                                            t.update_check
                                        };
                                        if m3_sized_pill_btn(
                                            ui,
                                            118.0,
                                            btn_text,
                                            false,
                                            !busy,
                                            &theme,
                                        ) && !busy
                                        {
                                            self.update_state = UpdateState::Checking;
                                            let _ = self
                                                .event_proxy
                                                .send_event(AppEvent::TriggerUpdateCheck);
                                        }
                                    }
                                }
                            });
                        });
                    });
                });

            // Language dropdown overlay (10 translated languages)
            let lang_pop_t = spring_animate(
                ctx,
                egui::Id::new("lang_drop_anim"),
                if self.lang_dropdown_open { 1.0 } else { 0.0 },
                0.24,
                0.72,
            )
            .clamp(0.0, 1.08);
            let mut lang_popup_rect = Rect::NOTHING;
            if self.lang_dropdown_open && dropdown_anchor != Rect::NOTHING {
                let drop_pos = Pos2::new(
                    dropdown_anchor.left(),
                    dropdown_anchor.bottom() + 6.0 - (1.0 - lang_pop_t) * 8.0,
                );
                let area_resp = egui::Area::new(egui::Id::new("lang_dropdown_popup"))
                    .fixed_pos(drop_pos)
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        dropdown_popup_frame(ui, dropdown_anchor.width() - 12.0, &theme, |ui| {
                            for lang in Language::ALL {
                                let selected = settings.language == lang;
                                if dropdown_item(ui, lang.label(), 13.0, selected, &theme) {
                                    settings.language = lang;
                                    settings.save();
                                    self.lang_dropdown_open = false;
                                    updated = true;
                                    lang_changed = true;
                                }
                            }
                        });
                    });
                lang_popup_rect = area_resp.response.rect;
            }

            let font_pop_t = spring_animate(
                ctx,
                egui::Id::new("font_drop_anim"),
                if self.font_dropdown_open { 1.0 } else { 0.0 },
                0.24,
                0.72,
            )
            .clamp(0.0, 1.08);
            let mut font_popup_rect = Rect::NOTHING;
            if self.font_dropdown_open && font_anchor != Rect::NOTHING {
                let drop_pos = Pos2::new(
                    font_anchor.left(),
                    font_anchor.bottom() + 6.0 - (1.0 - font_pop_t) * 8.0,
                );
                let area_resp = egui::Area::new(egui::Id::new("font_dropdown_popup"))
                    .fixed_pos(drop_pos)
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        dropdown_popup_frame(ui, font_anchor.width() - 12.0, &theme, |ui| {
                            for tf in TextFont::ALL {
                                let selected = settings.text_font == tf;
                                if dropdown_item(ui, tf.label(), 12.5, selected, &theme) {
                                    settings.text_font = tf;
                                    settings.save();
                                    self.font_dropdown_open = false;
                                    updated = true;
                                }
                            }
                        });
                    });
                font_popup_rect = area_resp.response.rect;
            }

            if let Some(click_pos) = ctx.input(|i| {
                if i.pointer.primary_clicked() {
                    i.pointer.interact_pos()
                } else {
                    None
                }
            }) {
                if self.lang_dropdown_open
                    && !lang_popup_rect.contains(click_pos)
                    && !dropdown_anchor.contains(click_pos)
                {
                    self.lang_dropdown_open = false;
                }
                if self.font_dropdown_open
                    && !font_popup_rect.contains(click_pos)
                    && !font_anchor.contains(click_pos)
                {
                    self.font_dropdown_open = false;
                }
                if self.recording.is_some() && self.recording == prev_recording {
                    self.recording = None;
                }
            }
        });

        if lang_changed {
            let t = Strings::for_lang(settings.language);
            self.gl_win.window.set_title(&format!(
                "ShotCraft v{} — {}",
                env!("CARGO_PKG_VERSION"),
                t.settings_title
            ));
        }

        let clear_col =
            ThemeColors::from_positions(settings.bg_color_pos, settings.accent_color_pos).surface;
        let needs_repaint = self.gl_win.paint_frame(
            [
                clear_col.r() as f32 / 255.0,
                clear_col.g() as f32 / 255.0,
                clear_col.b() as f32 / 255.0,
                1.0,
            ],
            full_output,
        );

        if updated
            || self.recording.is_some()
            || self.hotkey_warning.is_some()
            || needs_repaint
        {
            self.gl_win.window.request_redraw();
        }

        if updated {
            SettingsOutcome::Updated
        } else {
            SettingsOutcome::KeepOpen
        }
    }
}

fn draw_dropdown_trigger(
    ui: &mut egui::Ui,
    label: &str,
    font_size: f32,
    open: bool,
    theme: &ThemeColors,
) -> (Rect, egui::Response) {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(186.0, 30.0), egui::Sense::click());
    let scale = spring_scale(
        ui.ctx(),
        resp.id,
        resp.hovered(),
        resp.is_pointer_button_down_on(),
    );
    let draw_rect = Rect::from_center_size(rect.center(), rect.size() * scale);
    let r = morph_radius(
        ui.ctx(),
        resp.id,
        draw_rect.height(),
        open || resp.is_pointer_button_down_on(),
    );
    ui.painter().rect_filled(
        draw_rect,
        r,
        if open {
            theme.accent
        } else {
            theme.surface_container_high
        },
    );
    ui.painter().rect_stroke(
        draw_rect,
        r,
        Stroke::new(
            1.0_f32,
            if resp.hovered() && !open {
                theme.accent
            } else {
                theme.outline
            },
        ),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        draw_rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(font_size),
        if open {
            theme.on_accent
        } else {
            theme.on_surface
        },
    );
    (rect, resp)
}

fn dropdown_popup_frame(
    ui: &mut egui::Ui,
    width: f32,
    theme: &ThemeColors,
    add_items: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::NONE
        .fill(theme.surface_container)
        .stroke(Stroke::new(1.2_f32, theme.accent))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            ui.set_width(width);
            egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 3.0;
                add_items(ui);
            });
        });
}

fn dropdown_item(
    ui: &mut egui::Ui,
    label: &str,
    font_size: f32,
    selected: bool,
    theme: &ThemeColors,
) -> bool {
    let (r, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), egui::Sense::click());
    let bg = if selected {
        theme.accent
    } else if resp.hovered() {
        theme.surface_container_high
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect_filled(r, 6.0, bg);
    ui.painter().text(
        Pos2::new(r.left() + 10.0, r.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(font_size),
        if selected {
            theme.on_accent
        } else {
            theme.on_surface
        },
    );
    resp.clicked()
}

fn angle_step_widget(
    ui: &mut egui::Ui,
    deg: &mut u16,
    editing: &mut bool,
    buf: &mut String,
    drag_accum: &mut f32,
    hint: &str,
    theme: &ThemeColors,
) -> bool {
    const PRESETS: [u16; 9] = [1, 2, 5, 10, 15, 30, 45, 60, 90];
    let mut changed = false;

    if *editing {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(186.0, 30.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 7.0, theme.surface_container_high);
        ui.painter().rect_stroke(
            rect,
            7.0,
            Stroke::new(1.4_f32, theme.accent),
            egui::StrokeKind::Inside,
        );
        let resp = ui.put(
            rect.shrink2(Vec2::new(10.0, 4.0)),
            egui::TextEdit::singleline(buf)
                .horizontal_align(egui::Align::Center)
                .text_color(theme.accent)
                .frame(false),
        );
        let clicked_outside = ui.input(|i| {
            i.pointer.primary_clicked()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !rect.contains(p))
        });
        if !clicked_outside && !resp.has_focus() {
            resp.request_focus();
        }
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if enter || clicked_outside {
            if let Ok(v) = buf.trim().parse::<u16>() {
                *deg = v.clamp(1, 90);
                changed = true;
            }
            *editing = false;
        } else if esc {
            *editing = false;
        }
        return changed;
    }

    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(186.0, 30.0), egui::Sense::click_and_drag());
    let scale = spring_scale(
        ui.ctx(),
        resp.id,
        resp.hovered(),
        resp.is_pointer_button_down_on(),
    );
    let draw_rect = Rect::from_center_size(rect.center(), rect.size() * scale);
    if resp.drag_started() {
        *drag_accum = 0.0;
    }
    if resp.dragged() {
        *drag_accum += resp.drag_delta().x;
        let step_px = 16.0;
        if drag_accum.abs() >= step_px {
            let dir = if *drag_accum > 0.0 { 1isize } else { -1isize };
            *drag_accum = 0.0;
            let cur_idx = PRESETS
                .iter()
                .enumerate()
                .min_by_key(|(_, p)| p.abs_diff(*deg))
                .map(|(i, _)| i as isize)
                .unwrap_or(8);
            let next_idx = (cur_idx + dir).clamp(0, (PRESETS.len() - 1) as isize) as usize;
            if *deg != PRESETS[next_idx] {
                *deg = PRESETS[next_idx];
                changed = true;
            }
        }
    } else if resp.clicked() {
        *buf = deg.to_string();
        *editing = true;
    }

    let bg = if resp.dragged() {
        theme.accent
    } else {
        theme.surface_container_high
    };
    let fg = if resp.dragged() {
        theme.on_accent
    } else {
        theme.accent
    };
    let r = morph_radius(
        ui.ctx(),
        resp.id,
        draw_rect.height(),
        resp.dragged() || resp.is_pointer_button_down_on(),
    );
    ui.painter().rect_filled(draw_rect, r, bg);
    ui.painter().rect_stroke(
        draw_rect,
        r,
        Stroke::new(
            1.0_f32,
            if resp.hovered() {
                theme.accent
            } else {
                theme.outline
            },
        ),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        draw_rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("<  {}°  >", *deg),
        egui::FontId::proportional(12.5),
        fg,
    );
    resp.on_hover_text(hint);
    changed
}

fn m3_card(ui: &mut egui::Ui, theme: &ThemeColors, add_contents: impl FnOnce(&mut egui::Ui)) {
    let resp = egui::Frame::NONE
        .fill(theme.surface_container)
        .stroke(Stroke::new(1.0_f32, theme.outline))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui);
        });
    ui.painter().rect_stroke(
        resp.response.rect.shrink(1.0),
        11.0,
        Stroke::new(
            0.8_f32,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 16),
        ),
        egui::StrokeKind::Inside,
    );
}

fn m3_pill_btn(ui: &mut egui::Ui, text: &str, primary: bool, theme: &ThemeColors) -> bool {
    m3_sized_pill_btn(ui, 186.0, text, primary, true, theme)
}

fn m3_sized_pill_btn(
    ui: &mut egui::Ui,
    width: f32,
    text: &str,
    primary: bool,
    enabled: bool,
    theme: &ThemeColors,
) -> bool {
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 30.0), sense);
    let hovered = enabled && resp.hovered();
    let pressed = enabled && resp.is_pointer_button_down_on();
    let scale = spring_scale(ui.ctx(), resp.id, hovered, pressed);
    let draw_rect = Rect::from_center_size(rect.center(), rect.size() * scale);
    let r = morph_radius(ui.ctx(), resp.id, draw_rect.height(), primary || pressed);
    let (bg, fg) = if !enabled {
        (
            theme.surface_container_high,
            egui::Color32::from_rgba_unmultiplied(
                theme.on_surface.r(),
                theme.on_surface.g(),
                theme.on_surface.b(),
                140,
            ),
        )
    } else if primary {
        (
            if hovered {
                theme.accent_hover
            } else {
                theme.accent
            },
            theme.on_accent,
        )
    } else {
        (theme.surface_container_high, theme.on_surface)
    };
    ui.painter().rect_filled(draw_rect, r, bg);
    if !primary {
        ui.painter().rect_stroke(
            draw_rect,
            r,
            Stroke::new(
                1.0_f32,
                if hovered { theme.accent } else { theme.outline },
            ),
            egui::StrokeKind::Inside,
        );
    }
    ui.painter().text(
        draw_rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(12.5),
        fg,
    );
    enabled && resp.clicked()
}

fn hotkey_row(
    ui: &mut egui::Ui,
    label: &str,
    spec: HotkeySpec,
    recording: bool,
    press_text: &str,
    theme: &ThemeColors,
    on_click: impl FnOnce(),
) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(13.0).color(theme.on_surface));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let text = if recording {
                press_text.to_string()
            } else {
                spec.display()
            };
            let (bg, fg) = if recording {
                (theme.accent_hover, theme.on_accent)
            } else {
                (theme.surface_container_high, theme.accent)
            };
            let (rect, resp) =
                ui.allocate_exact_size(Vec2::new(186.0, 30.0), egui::Sense::click());
            let scale = spring_scale(
                ui.ctx(),
                resp.id,
                resp.hovered(),
                resp.is_pointer_button_down_on(),
            );
            let draw_rect = Rect::from_center_size(rect.center(), rect.size() * scale);
            let r = morph_radius(
                ui.ctx(),
                resp.id,
                draw_rect.height(),
                recording || resp.is_pointer_button_down_on(),
            );
            ui.painter().rect_filled(draw_rect, r, bg);
            let stroke_col = if recording {
                let pulse = (ui.ctx().input(|i| i.time) * 6.0).sin() as f32 * 0.5 + 0.5;
                egui::Color32::from_rgba_unmultiplied(
                    255,
                    255,
                    255,
                    (120.0 + 135.0 * pulse).round() as u8,
                )
            } else if resp.hovered() {
                theme.accent
            } else {
                theme.outline
            };
            ui.painter().rect_stroke(
                draw_rect,
                r,
                Stroke::new(if recording { 1.6_f32 } else { 1.0_f32 }, stroke_col),
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                draw_rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::proportional(12.5),
                fg,
            );
            if resp.clicked() {
                on_click();
            }
        });
    });
}

fn configure_m3_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "roboto_m3".to_owned(),
        Arc::new(egui::FontData::from_static(ROBOTO_FONT_BYTES)),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "roboto_m3".to_owned());
    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    style.visuals.dark_mode = true;
    ctx.set_style(style);
}
