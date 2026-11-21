use std::collections::HashMap;
use std::ffi::CString;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;

use egui::{Color32, ColorImage, CornerRadius, Key, Pos2, Rect, Stroke, Vec2};
use glutin::config::{ConfigTemplateBuilder, GlConfig};
use glutin::context::{
    ContextApi, ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext,
    PossiblyCurrentGlContext,
};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::surface::{GlSurface, Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface};
use glutin_winit::DisplayBuilder;
use raw_window_handle::HasWindowHandle;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{CursorIcon, Window, WindowAttributes, WindowId, WindowLevel};

use crate::annotations::{
    bake_blur_patch, callout_anchor, draw_egui_arrow, paint_annotations, render_export,
    text_content_rect, Annotation, TextPlacement, Tool, GOST_TYPE_AU_BYTES, ROBOTO_FONT_BYTES,
};
use crate::capture::{
    capture_all_monitors, composite_monitors, composite_monitors_metadata, copy_to_clipboard,
    save_to_dir, CapturedScreen, WindowCandidate,
};
use crate::icons;
use crate::material::{
    color_from_slider, draw_m3_panel, draw_m3_selection_frame, draw_rounded_texture_uv,
    m3_action_button, m3_color_slider, m3_color_slider_vertical, m3_size_slider,
    m3_size_slider_vertical, m3_tool_button, make_strong_text_blur_image, spring_animate,
    ThemeColors,
};
use crate::settings::{Language, Strings, TextFont, ToolbarLayout};

pub enum OverlayOutcome {
    KeepOpen,
    Close,
    LiveCopy,
    LiveSave,
}

#[derive(PartialEq, Eq)]
enum Mode {
    Selecting,
    WindowSelect,
    Annotating,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ResizeHandle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

#[derive(Clone, Copy)]
enum DragAction {
    DrawAnnotation,
    TextCallout,
    MoveAnnotation { idx: usize, last_pos: Pos2 },
    EditAnnotationHandle { idx: usize, handle_idx: usize },
    ResizeSelection { handle: ResizeHandle },
    MoveSelectionAndAll { last_pos: Pos2 },
}

struct PendingText {
    pos: Pos2,
    arrow_to: Option<Pos2>,
    placement: TextPlacement,
    buf: String,
    just_opened: bool,
    original: Option<(usize, Annotation)>,
}

struct OverlayShared {
    virtual_screen: CapturedScreen,
    live_draw: bool,
    windows: Vec<WindowCandidate>,
    hovered_window_label: Option<String>,
    save_dir: PathBuf,
    lang: Language,
    theme: ThemeColors,
    snap_angle_deg: u16,
    text_font: TextFont,
    toolbar_layout: ToolbarLayout,

    modifiers: ModifiersState,
    mode: Mode,
    hover_pos: Option<Pos2>,
    drag_start: Option<Pos2>,
    drag_curr: Option<Pos2>,
    drag_action: Option<DragAction>,
    selection: Option<Rect>,
    toolbar_pos: Option<Pos2>,

    active_tool: Tool,
    color_pos: f32,
    active_color: Color32,
    thickness: f32,
    annotations: Vec<Annotation>,
    in_progress: Option<Annotation>,
    selected_annotation: Option<usize>,
    last_edited_idx: Option<usize>,
    pending_text: Option<PendingText>,
    step_counter: u32,

    blur_revision: u64,
    baked_blurs: HashMap<usize, ColorImage>,
    text_blur_img: Option<Arc<ColorImage>>,
}

struct OverlayPane {
    window: Window,
    live_draw: bool,
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,
    gl: Arc<glow::Context>,
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_painter: egui_glow::Painter,

    screen: CapturedScreen,
    mon_offset: Vec2,
    sharp_tex: Option<egui::TextureHandle>,
    text_blur_tex: Option<egui::TextureHandle>,
    uploaded_blur_rev: u64,
    blur_textures: HashMap<usize, egui::TextureHandle>,
    blur_tex_ids: HashMap<usize, egui::TextureId>,
}

impl Drop for OverlayPane {
    fn drop(&mut self) {
        let _ = self.gl_context.make_current(&self.gl_surface);
        self.egui_painter.destroy();
    }
}

pub struct OverlayWindow {
    panes: Vec<OverlayPane>,
    shared: OverlayShared,
}

impl OverlayWindow {
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        event_loop: &ActiveEventLoop,
        screens: Vec<CapturedScreen>,
        windows: Vec<WindowCandidate>,
        window_select_mode: bool,
        live_draw: bool,
        save_dir: PathBuf,
        lang: Language,
        theme: ThemeColors,
        initial_color_pos: f32,
        initial_thickness: f32,
        snap_angle_deg: u16,
        text_font: TextFont,
        toolbar_layout: ToolbarLayout,
        initial_tool: Tool,
        initial_annotations: Vec<Annotation>,
        initial_step_counter: u32,
    ) -> Result<Self, String> {
        let virtual_screen = if live_draw {
            composite_monitors_metadata(&screens)
        } else {
            composite_monitors(&screens)
        };
        let mut panes = Vec::with_capacity(screens.len());

        for screen in screens {
            let pane = OverlayPane::open(event_loop, screen, &virtual_screen, text_font, live_draw)?;
            panes.push(pane);
        }

        if let Some(first) = panes.first() {
            first.window.focus_window();
        }

        let mode = if live_draw {
            Mode::Annotating
        } else if window_select_mode {
            Mode::WindowSelect
        } else {
            Mode::Selecting
        };

        let selection = if live_draw {
            Some(Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(virtual_screen.logical_width, virtual_screen.logical_height),
            ))
        } else {
            None
        };

        let color_pos = initial_color_pos.clamp(0.0, 1.0);
        let thickness = initial_thickness.clamp(1.5, 20.0);

        let mut shared = OverlayShared {
            virtual_screen,
            live_draw,
            windows,
            hovered_window_label: None,
            save_dir,
            lang,
            theme,
            snap_angle_deg: snap_angle_deg.clamp(1, 90),
            text_font,
            toolbar_layout,
            modifiers: ModifiersState::empty(),
            mode,
            hover_pos: None,
            drag_start: None,
            drag_curr: None,
            drag_action: None,
            selection,
            toolbar_pos: None,
            active_tool: initial_tool,
            color_pos,
            active_color: color_from_slider(color_pos),
            thickness,
            annotations: initial_annotations,
            in_progress: None,
            selected_annotation: None,
            last_edited_idx: None,
            pending_text: None,
            step_counter: initial_step_counter.max(1),
            blur_revision: 0,
            baked_blurs: HashMap::new(),
            text_blur_img: None,
        };
        if !live_draw && !shared.annotations.is_empty() {
            shared.rebake_blurs();
        }

        Ok(Self { panes, shared })
    }

    pub fn has_window_id(&self, id: WindowId) -> bool {
        self.panes.iter().any(|p| p.window.id() == id)
    }

    pub fn color_pos(&self) -> f32 {
        self.shared.color_pos
    }

    pub fn thickness(&self) -> f32 {
        self.shared.thickness
    }

    pub fn active_tool(&self) -> Tool {
        self.shared.active_tool
    }

    pub fn is_live_draw(&self) -> bool {
        self.shared.live_draw
    }

    pub fn hide_all(&self) {
        for p in &self.panes {
            p.window.set_visible(false);
            #[cfg(target_os = "macos")]
            if let Ok(handle) = p.window.window_handle() {
                if let raw_window_handle::RawWindowHandle::AppKit(appkit) = handle.as_raw() {
                    unsafe {
                        let ns_view = appkit.ns_view.as_ptr() as *mut objc2::runtime::AnyObject;
                        let ns_win: *mut objc2::runtime::AnyObject = objc2::msg_send![ns_view, window];
                        if !ns_win.is_null() {
                            let nil: *mut objc2::runtime::AnyObject = std::ptr::null_mut();
                            let _: () = objc2::msg_send![ns_win, orderOut: nil];
                        }
                    }
                }
            }
        }
    }

    pub fn take_live_draw_state(&mut self) -> (Vec<Annotation>, u32, i32, i32) {
        if self.shared.pending_text.is_some() {
            self.shared.commit_pending_text();
        }
        (
            std::mem::take(&mut self.shared.annotations),
            self.shared.step_counter,
            self.shared.virtual_screen.x,
            self.shared.virtual_screen.y,
        )
    }

    fn request_all_redraws(&self) {
        for p in &self.panes {
            p.window.request_redraw();
        }
    }

    pub fn handle_window_event(&mut self, window_id: WindowId, event: &WindowEvent) -> OverlayOutcome {
        let Some(pane_idx) = self.panes.iter().position(|p| p.window.id() == window_id) else {
            return OverlayOutcome::KeepOpen;
        };

        if let WindowEvent::CloseRequested = event {
            return OverlayOutcome::Close;
        }

        if let WindowEvent::ModifiersChanged(mods) = event {
            self.shared.modifiers = mods.state();
            self.request_all_redraws();
        }

        if let WindowEvent::CursorMoved { position, .. } = event {
            let sf = self.panes[pane_idx].window.scale_factor() as f32;
            let local = Pos2::new(position.x as f32 / sf, position.y as f32 / sf);
            self.shared.hover_pos = Some(local + self.panes[pane_idx].mon_offset);
            self.request_all_redraws();
        }

        if let WindowEvent::KeyboardInput { event: key_ev, .. } = event {
            if matches!(
                key_ev.physical_key,
                PhysicalKey::Code(
                    KeyCode::ControlLeft
                        | KeyCode::ControlRight
                        | KeyCode::SuperLeft
                        | KeyCode::SuperRight
                )
            ) {
                self.request_all_redraws();
            }

            let primary_mod = primary_mod_held(self.shared.modifiers, None);
            if key_ev.state == ElementState::Pressed && primary_mod {
                match key_ev.physical_key {
                    PhysicalKey::Code(KeyCode::KeyC) => {
                        if self.shared.export_and_copy() {
                            return OverlayOutcome::Close;
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyS) => {
                        if self.shared.export_and_save() {
                            return OverlayOutcome::Close;
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyZ) if self.shared.pending_text.is_none() => {
                        self.shared.undo();
                        self.request_all_redraws();
                        return OverlayOutcome::KeepOpen;
                    }
                    _ => {}
                }
            } else if key_ev.state == ElementState::Pressed
                && self.shared.mode == Mode::Annotating
                && self.shared.pending_text.is_none()
                && !primary_mod
                && !self.shared.modifiers.alt_key()
            {
                let picked = match key_ev.physical_key {
                    PhysicalKey::Code(KeyCode::Digit1) => Some(Tool::Move),
                    PhysicalKey::Code(KeyCode::Digit2) => Some(Tool::Pen),
                    PhysicalKey::Code(KeyCode::Digit3) => Some(Tool::Marker),
                    PhysicalKey::Code(KeyCode::Digit4) => Some(Tool::Line),
                    PhysicalKey::Code(KeyCode::Digit5) => Some(Tool::Arrow),
                    PhysicalKey::Code(KeyCode::Digit6) => Some(Tool::Text),
                    PhysicalKey::Code(KeyCode::Digit7) => Some(Tool::Ellipse),
                    PhysicalKey::Code(KeyCode::Digit8) => Some(Tool::Rect),
                    PhysicalKey::Code(KeyCode::Digit9) => Some(Tool::Step),
                    PhysicalKey::Code(KeyCode::Digit0) => Some(Tool::Blur),
                    _ => None,
                };
                if let Some(tool) = picked {
                    self.shared.active_tool = tool;
                    self.request_all_redraws();
                }
            }
        }

        if let WindowEvent::Resized(PhysicalSize { width, height }) = event {
            if *width > 0 && *height > 0 {
                let pane = &mut self.panes[pane_idx];
                let _ = pane.gl_context.make_current(&pane.gl_surface);
                pane.gl_surface.resize(
                    &pane.gl_context,
                    NonZeroU32::new(*width).unwrap(),
                    NonZeroU32::new(*height).unwrap(),
                );
            }
        }

        let repaint = {
            let pane = &mut self.panes[pane_idx];
            pane.egui_state.on_window_event(&pane.window, event).repaint
        };
        if repaint {
            self.request_all_redraws();
        }

        if matches!(event, WindowEvent::RedrawRequested) {
            let toolbar_pane = self.toolbar_pane_index();
            let outcome = self.panes[pane_idx].draw_frame(&mut self.shared, pane_idx == toolbar_pane);
            return outcome;
        }

        OverlayOutcome::KeepOpen
    }

    fn toolbar_pane_index(&self) -> usize {
        if let Some(sel) = self.shared.selection {
            let c = sel.center();
            if let Some(idx) = self.panes.iter().position(|p| {
                let r = Rect::from_min_size(
                    p.mon_offset.to_pos2(),
                    Vec2::new(p.screen.logical_width, p.screen.logical_height),
                );
                r.contains(c)
            }) {
                return idx;
            }
        }
        0
    }
}

impl OverlayShared {
    fn text_font_size(&self) -> f32 {
        (13.0 + self.thickness * 2.2).clamp(16.0, 57.0)
    }

    fn step_radius(&self) -> f32 {
        (9.5 + self.thickness * 0.95).clamp(11.0, 28.5)
    }

    fn blur_radius(&self) -> f32 {
        (2.5 + self.thickness * 0.55).clamp(3.0, 14.0)
    }

    fn rebake_blurs(&mut self) {
        self.baked_blurs.clear();
        if self.live_draw {
            let has_blur = self
                .annotations
                .iter()
                .any(|a| matches!(a, Annotation::Blur { .. }));
            if !has_blur {
                self.blur_revision = self.blur_revision.wrapping_add(1);
                return;
            }
            if self.virtual_screen.image.width() <= 2 {
                if let Ok(screens) = capture_all_monitors() {
                    self.virtual_screen = composite_monitors(&screens);
                }
            }
        }
        for (idx, ann) in self.annotations.iter().enumerate() {
            match ann {
                Annotation::Blur { rect, radius } => {
                    let img = bake_blur_patch(
                        &self.virtual_screen.image,
                        *rect,
                        self.virtual_screen.scale_factor,
                        *radius,
                        &self.annotations[..idx],
                    );
                    self.baked_blurs.insert(idx, img);
                }
                Annotation::Text {
                    pos,
                    text,
                    size,
                    thickness,
                    placement,
                    font,
                    box_size,
                    ..
                } if !self.live_draw => {
                    let text_r = text_content_rect(*pos, text, *size, *placement, *font, *box_size);
                    let bg = text_r.expand(6.0);
                    let radius = (*thickness * 1.1 + 10.0).clamp(12.0, 28.0);
                    let img = bake_blur_patch(
                        &self.virtual_screen.image,
                        bg,
                        self.virtual_screen.scale_factor,
                        radius,
                        &self.annotations[..idx],
                    );
                    self.baked_blurs.insert(idx, img);
                }
                _ => {}
            }
        }
        self.blur_revision = self.blur_revision.wrapping_add(1);
    }

    fn undo(&mut self) {
        self.selected_annotation = None;
        self.last_edited_idx = None;
        if let Some(Annotation::Step { .. }) = self.annotations.pop() {
            self.step_counter = self.step_counter.saturating_sub(1).max(1);
        }
        self.rebake_blurs();
    }

    fn commit_pending_text(&mut self) {
        if let Some(pending) = self.pending_text.take() {
            let trimmed = pending.buf.trim().to_string();
            if !trimmed.is_empty() {
                let (idx, ann) = if let Some((orig_idx, Annotation::Text {
                    color,
                    size,
                    thickness,
                    font,
                    box_size,
                    ..
                })) = pending.original
                {
                    let insert_at = orig_idx.min(self.annotations.len());
                    (
                        insert_at,
                        Annotation::Text {
                            pos: pending.pos,
                            text: trimmed,
                            color,
                            size,
                            arrow_to: pending.arrow_to,
                            thickness,
                            placement: pending.placement,
                            font,
                            box_size,
                        },
                    )
                } else {
                    (
                        self.annotations.len(),
                        Annotation::Text {
                            pos: pending.pos,
                            text: trimmed,
                            color: self.active_color,
                            size: self.text_font_size(),
                            arrow_to: pending.arrow_to,
                            thickness: self.thickness,
                            placement: pending.placement,
                            font: self.text_font,
                            box_size: None,
                        },
                    )
                };
                self.annotations.insert(idx, ann);
                self.last_edited_idx = Some(idx);
                self.selected_annotation = None;
                self.rebake_blurs();
            } else if pending.original.is_some() {
                self.rebake_blurs();
            }
        }
    }

    fn cancel_pending_text(&mut self) {
        if let Some(pending) = self.pending_text.take() {
            if let Some((orig_idx, orig_ann)) = pending.original {
                let insert_at = orig_idx.min(self.annotations.len());
                self.annotations.insert(insert_at, orig_ann);
                self.last_edited_idx = Some(insert_at);
                self.rebake_blurs();
            }
        }
    }

    fn open_text_for_edit(&mut self, idx: usize) {
        self.commit_pending_text();
        if idx >= self.annotations.len() {
            return;
        }
        if let Annotation::Text {
            pos,
            text,
            arrow_to,
            placement,
            ..
        } = &self.annotations[idx]
        {
            let pos = *pos;
            let buf = text.clone();
            let arrow_to = *arrow_to;
            let placement = *placement;
            let orig = self.annotations.remove(idx);
            self.selected_annotation = None;
            self.pending_text = Some(PendingText {
                pos,
                arrow_to,
                placement,
                buf,
                just_opened: true,
                original: Some((idx, orig)),
            });
            self.rebake_blurs();
        }
    }

    fn export_and_copy(&mut self) -> bool {
        self.commit_pending_text();
        if let Some(sel) = self.selection {
            let img = render_export(
                &self.virtual_screen.image,
                sel,
                self.virtual_screen.scale_factor,
                &self.annotations,
            );
            if let Err(e) = copy_to_clipboard(&img) {
                eprintln!("Copy failed: {e}");
            }
            return true;
        }
        false
    }

    fn export_and_save(&mut self) -> bool {
        self.commit_pending_text();
        if let Some(sel) = self.selection {
            let img = render_export(
                &self.virtual_screen.image,
                sel,
                self.virtual_screen.scale_factor,
                &self.annotations,
            );
            match save_to_dir(&img, &self.save_dir) {
                Ok(path) => println!("Saved screenshot: {}", path.display()),
                Err(e) => eprintln!("Save failed: {e}"),
            }
            return true;
        }
        false
    }
}

impl OverlayPane {
    fn open(
        event_loop: &ActiveEventLoop,
        screen: CapturedScreen,
        virtual_screen: &CapturedScreen,
        text_font: TextFont,
        live_draw: bool,
    ) -> Result<Self, String> {
        #[allow(unused_mut)]
        let mut win_attrs = WindowAttributes::default()
            .with_title("ShotCraft Capture")
            .with_decorations(false)
            .with_transparent(live_draw)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop);

        if screen.physical_coords {
            let pw = (screen.logical_width * screen.scale_factor).round().max(1.0) as u32;
            let ph = (screen.logical_height * screen.scale_factor).round().max(1.0) as u32;
            win_attrs = win_attrs
                .with_position(PhysicalPosition::new(screen.x, screen.y))
                .with_inner_size(PhysicalSize::new(pw, ph));
        } else {
            win_attrs = win_attrs
                .with_position(LogicalPosition::new(screen.x as f64, screen.y as f64))
                .with_inner_size(LogicalSize::new(
                    screen.logical_width as f64,
                    screen.logical_height as f64,
                ));
        }

        #[cfg(target_os = "windows")]
        {
            use winit::platform::windows::WindowAttributesExtWindows;
            win_attrs = win_attrs.with_skip_taskbar(true);
        }

        let template = ConfigTemplateBuilder::new().with_alpha_size(8);
        let (window, gl_config) = DisplayBuilder::new()
            .with_window_attributes(Some(win_attrs))
            .build(event_loop, template, |configs| {
                configs
                    .reduce(|accum, config| {
                        if config.num_samples() < accum.num_samples() {
                            config
                        } else {
                            accum
                        }
                    })
                    .expect("No GL config found")
            })
            .map_err(|e| format!("Failed to create overlay window: {e}"))?;

        let window = window.ok_or_else(|| "Overlay window was not created".to_string())?;

        let raw_handle = window
            .window_handle()
            .map_err(|e| format!("Window handle error: {e}"))?
            .as_raw();

        #[cfg(target_os = "macos")]
        if let raw_window_handle::RawWindowHandle::AppKit(appkit) = raw_handle {
            unsafe {
                let ns_view = appkit.ns_view.as_ptr() as *mut objc2::runtime::AnyObject;
                let ns_win: *mut objc2::runtime::AnyObject = objc2::msg_send![ns_view, window];
                if !ns_win.is_null() {
                    let _: () = objc2::msg_send![ns_win, setLevel: 1000_isize];
                    if live_draw {
                        let _: () = objc2::msg_send![ns_win, setHasShadow: false];
                        let _: () = objc2::msg_send![ns_win, setOpaque: false];
                        if let Some(ns_color_cls) = objc2::runtime::AnyClass::get(c"NSColor") {
                            let clear: *mut objc2::runtime::AnyObject =
                                objc2::msg_send![ns_color_cls, clearColor];
                            let _: () = objc2::msg_send![ns_win, setBackgroundColor: clear];
                        }
                        let layer: *mut objc2::runtime::AnyObject =
                            objc2::msg_send![ns_view, layer];
                        if !layer.is_null() {
                            let _: () = objc2::msg_send![layer, setOpaque: false];
                        }
                    }
                }
            }
        }

        let gl_display = gl_config.display();
        let context_attrs = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(None))
            .build(Some(raw_handle));

        let not_current = unsafe {
            gl_display
                .create_context(&gl_config, &context_attrs)
                .map_err(|e| format!("GL context error: {e}"))?
        };

        let size = window.inner_size();
        let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            raw_handle,
            NonZeroU32::new(size.width.max(1)).unwrap(),
            NonZeroU32::new(size.height.max(1)).unwrap(),
        );
        let gl_surface = unsafe {
            gl_display
                .create_window_surface(&gl_config, &attrs)
                .map_err(|e| format!("GL surface error: {e}"))?
        };
        let gl_context = not_current
            .make_current(&gl_surface)
            .map_err(|e| format!("Make current error: {e}"))?;
        let _ = gl_surface.set_swap_interval(&gl_context, SwapInterval::DontWait);

        #[cfg(target_os = "macos")]
        if live_draw {
            unsafe {
                if let Some(cls) = objc2::runtime::AnyClass::get(c"NSOpenGLContext") {
                    let ctx: *mut objc2::runtime::AnyObject =
                        objc2::msg_send![cls, currentContext];
                    if !ctx.is_null() {
                        // NSOpenGLContextParameterSurfaceOpacity = 236 -> 0 for transparent GL surface
                        let zero: i32 = 0;
                        let _: () = objc2::msg_send![
                            ctx,
                            setValues: &zero as *const i32,
                            forParameter: 236_isize
                        ];
                    }
                }
            }
        }

        let gl = Arc::new(unsafe {
            glow::Context::from_loader_function(|s| {
                let c = CString::new(s).unwrap();
                gl_display.get_proc_address(&c) as *const _
            })
        });

        let egui_ctx = egui::Context::default();
        configure_m3_style(&egui_ctx, text_font);

        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let egui_painter = egui_glow::Painter::new(gl.clone(), "", None, false)
            .map_err(|e| format!("egui_glow error: {e}"))?;

        let mon_offset = Vec2::new(
            (screen.x - virtual_screen.x) as f32,
            (screen.y - virtual_screen.y) as f32,
        );

        window.request_redraw();

        Ok(Self {
            window,
            live_draw,
            gl_context,
            gl_surface,
            gl,
            egui_ctx,
            egui_state,
            egui_painter,
            screen,
            mon_offset,
            sharp_tex: None,
            text_blur_tex: None,
            uploaded_blur_rev: 0,
            blur_textures: HashMap::new(),
            blur_tex_ids: HashMap::new(),
        })
    }

    fn ensure_sharp_texture(&mut self) -> egui::TextureId {
        if let Some(sharp) = &self.sharp_tex {
            return sharp.id();
        }
        let size = [
            self.screen.image.width() as usize,
            self.screen.image.height() as usize,
        ];
        let sharp_img = ColorImage::from_rgba_unmultiplied(size, self.screen.image.as_raw());
        let sharp = self
            .egui_ctx
            .load_texture("mon_sharp", sharp_img, egui::TextureOptions::LINEAR);
        let id = sharp.id();
        self.sharp_tex = Some(sharp);
        id
    }

    fn ensure_text_blur_texture(&mut self, shared: &mut OverlayShared) -> egui::TextureId {
        if let Some(tex) = &self.text_blur_tex {
            return tex.id();
        }
        let img = shared
            .text_blur_img
            .get_or_insert_with(|| Arc::new(make_strong_text_blur_image(&shared.virtual_screen.image)))
            .as_ref()
            .clone();
        let tex = self
            .egui_ctx
            .load_texture("virtual_text_blur", img, egui::TextureOptions::LINEAR);
        let id = tex.id();
        self.text_blur_tex = Some(tex);
        id
    }

    fn sync_blur_textures(&mut self, shared: &OverlayShared) {
        if self.uploaded_blur_rev != shared.blur_revision {
            self.blur_textures.clear();
            self.blur_tex_ids.clear();
            for (&idx, img) in &shared.baked_blurs {
                let tex = self.egui_ctx.load_texture(
                    format!("blur_patch_{idx}"),
                    img.clone(),
                    egui::TextureOptions::LINEAR,
                );
                self.blur_tex_ids.insert(idx, tex.id());
                self.blur_textures.insert(idx, tex);
            }
            self.uploaded_blur_rev = shared.blur_revision;
        }
    }

    fn draw_frame(&mut self, shared: &mut OverlayShared, draw_toolbar_here: bool) -> OverlayOutcome {
        let _ = self.gl_context.make_current(&self.gl_surface);

        let sharp_tex = if shared.live_draw {
            None
        } else {
            Some(self.ensure_sharp_texture())
        };
        let needs_text_blur = !shared.live_draw
            && (shared.pending_text.is_some()
                || shared
                    .annotations
                    .iter()
                    .any(|a| matches!(a, Annotation::Text { .. }))
                || matches!(shared.in_progress, Some(Annotation::Blur { .. })));
        let text_blur_tex = if needs_text_blur {
            Some(self.ensure_text_blur_texture(shared))
        } else {
            None
        };
        self.sync_blur_textures(shared);
        let blur_patch_ids = &self.blur_tex_ids;

        let raw_input = self.egui_state.take_egui_input(&self.window);
        let mut outcome = OverlayOutcome::KeepOpen;

        let local_size = Vec2::new(self.screen.logical_width, self.screen.logical_height);
        let local_rect = Rect::from_min_size(Pos2::ZERO, local_size);
        let virtual_size = Vec2::new(
            shared.virtual_screen.logical_width,
            shared.virtual_screen.logical_height,
        );
        let virtual_rect = Rect::from_min_size(Pos2::ZERO, virtual_size);
        let offset = -self.mon_offset;

        let egui_ctx = self.egui_ctx.clone();
        let full_output = egui_ctx.run(raw_input, |ctx| {
            let editing_text = shared.pending_text.is_some();
            let (esc, undo, copy, save, del, enter_commit, shift_held, cmd_held) = ctx.input(|i| {
                let cmd = primary_mod_held(shared.modifiers, Some(i.modifiers));
                (
                    i.key_pressed(Key::Escape),
                    !editing_text && cmd && i.key_pressed(Key::Z),
                    (!editing_text && i.events.iter().any(|e| matches!(e, egui::Event::Copy)))
                        || (cmd && i.key_pressed(Key::C)),
                    cmd && i.key_pressed(Key::S),
                    !editing_text && (i.key_pressed(Key::Backspace) || i.key_pressed(Key::Delete)),
                    i.key_pressed(Key::Enter) && !i.modifiers.shift,
                    i.modifiers.shift,
                    cmd,
                )
            });

            if esc {
                if shared.pending_text.is_some() {
                    shared.cancel_pending_text();
                } else {
                    outcome = OverlayOutcome::Close;
                    return;
                }
            }
            if copy {
                if shared.live_draw {
                    outcome = OverlayOutcome::LiveCopy;
                    return;
                } else if shared.export_and_copy() {
                    outcome = OverlayOutcome::Close;
                    return;
                }
            }
            if save {
                if shared.live_draw {
                    outcome = OverlayOutcome::LiveSave;
                    return;
                } else if shared.export_and_save() {
                    outcome = OverlayOutcome::Close;
                    return;
                }
            }
            if enter_commit {
                if shared.pending_text.is_some() {
                    shared.commit_pending_text();
                } else if !shared.live_draw
                    && shared.selection.is_some()
                    && shared.mode == Mode::Annotating
                {
                    if shared.export_and_copy() {
                        outcome = OverlayOutcome::Close;
                        return;
                    }
                }
            }
            if undo {
                shared.undo();
            }
            if del {
                if let Some(idx) = shared.selected_annotation.take() {
                    if idx < shared.annotations.len() {
                        shared.annotations.remove(idx);
                        shared.last_edited_idx = None;
                        shared.rebake_blurs();
                    }
                }
            }

            // 1. Sharp monitor canvas
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    let painter = ui.painter();

                    if let Some(tex) = sharp_tex {
                        painter.image(
                            tex,
                            local_rect,
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                        painter.rect_filled(
                            local_rect,
                            0.0,
                            Color32::from_rgba_unmultiplied(6, 10, 20, 96),
                        );
                    }

                    if shared.live_draw {
                        // Transparent live-draw mode: no dimming, no selection frame, no size badge.
                    } else if let Some(sel_global) = shared.selection {
                        let sel_local = sel_global.translate(offset);
                        if let Some(tex) = sharp_tex {
                            draw_m3_selection_frame(
                                painter,
                                tex,
                                local_size,
                                sel_local,
                                shared.mode == Mode::Annotating && shared.active_tool == Tool::Move,
                                &shared.theme,
                            );
                        }

                        if draw_toolbar_here {
                            let px_w =
                                (sel_global.width() * shared.virtual_screen.scale_factor).round() as i32;
                            let px_h =
                                (sel_global.height() * shared.virtual_screen.scale_factor).round() as i32;
                            let badge_text = if shared.mode == Mode::WindowSelect {
                                if let Some(lbl) = &shared.hovered_window_label {
                                    format!("{lbl}  •  {px_w} × {px_h}")
                                } else {
                                    format!("{px_w} × {px_h}")
                                }
                            } else {
                                format!("{px_w} × {px_h}")
                            };
                            let badge_w =
                                (badge_text.chars().count() as f32 * 7.2 + 24.0).clamp(100.0, 380.0);
                            let badge_pos = Pos2::new(
                                sel_local.min.x + 6.0,
                                if sel_local.min.y > 38.0 {
                                    sel_local.min.y - 34.0
                                } else {
                                    sel_local.min.y + 10.0
                                },
                            );
                            let badge_rect =
                                Rect::from_min_size(badge_pos, Vec2::new(badge_w, 26.0));
                            draw_m3_panel(painter, badge_rect, 8.0, &shared.theme);
                            painter.text(
                                badge_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                badge_text,
                                egui::FontId::proportional(12.0),
                                shared.theme.accent,
                            );
                        }
                    } else {
                        if draw_toolbar_here {
                            let hint_rect =
                                Rect::from_center_size(local_rect.center(), Vec2::new(390.0, 44.0));
                            draw_m3_panel(painter, hint_rect, 10.0, &shared.theme);
                            let t = Strings::for_lang(shared.lang);
                            let hint_text = if shared.mode == Mode::WindowSelect {
                                t.hint_window
                            } else {
                                t.hint_area
                            };
                            painter.text(
                                hint_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                hint_text,
                                egui::FontId::proportional(14.0),
                                shared.theme.on_surface,
                            );
                        }

                        if shared.mode == Mode::Selecting {
                            if let Some(hover_global) = shared.hover_pos {
                                let hover = hover_global + offset;
                                let dark = Color32::from_rgba_unmultiplied(0, 0, 0, 95);
                                let light = Color32::from_rgba_unmultiplied(255, 255, 255, 130);
                                painter.line_segment(
                                    [
                                        Pos2::new(0.0, hover.y + 1.0),
                                        Pos2::new(local_rect.max.x, hover.y + 1.0),
                                    ],
                                    Stroke::new(1.0_f32, dark),
                                );
                                painter.line_segment(
                                    [
                                        Pos2::new(hover.x + 1.0, 0.0),
                                        Pos2::new(hover.x + 1.0, local_rect.max.y),
                                    ],
                                    Stroke::new(1.0_f32, dark),
                                );
                                painter.line_segment(
                                    [Pos2::new(0.0, hover.y), Pos2::new(local_rect.max.x, hover.y)],
                                    Stroke::new(1.0_f32, light),
                                );
                                painter.line_segment(
                                    [Pos2::new(hover.x, 0.0), Pos2::new(hover.x, local_rect.max.y)],
                                    Stroke::new(1.0_f32, light),
                                );
                            }
                        }
                    }

                    // Render committed + in-progress annotations
                    paint_annotations(
                        painter,
                        &shared.annotations,
                        if shared.active_tool == Tool::Move {
                            shared.selected_annotation
                        } else {
                            None
                        },
                        offset,
                        &blur_patch_ids,
                        text_blur_tex,
                        virtual_size,
                        shared.theme.accent,
                    );
                    if let Some(ann) = &shared.in_progress {
                        let empty_blurs = HashMap::new();
                        paint_annotations(
                            painter,
                            std::slice::from_ref(ann),
                            None,
                            offset,
                            &empty_blurs,
                            text_blur_tex,
                            virtual_size,
                            shared.theme.accent,
                        );
                    }

                    // Live Text callout arrow preview (8-direction placement)
                    if matches!(shared.drag_action, Some(DragAction::TextCallout)) {
                        if let (Some(start), Some(curr)) = (shared.drag_start, shared.drag_curr) {
                            let box_w = 104.0;
                            let box_h = shared.text_font_size() + 10.0;
                            let placement = TextPlacement::from_drag(start, curr);
                            let preview_box = placement.box_rect(curr, box_w, box_h);
                            painter.rect_stroke(
                                preview_box.translate(offset),
                                8.0,
                                Stroke::new(1.4_f32, shared.active_color),
                                egui::StrokeKind::Inside,
                            );
                            if start.distance(curr) >= 8.0 {
                                let anchor = callout_anchor(preview_box, start);
                                draw_egui_arrow(
                                    painter,
                                    anchor + offset,
                                    start + offset,
                                    Stroke::new(
                                        (shared.thickness * 0.85).max(2.0),
                                        shared.active_color,
                                    ),
                                );
                            }
                        }
                    } else if let Some(pending) = &shared.pending_text {
                        let (inner_w, inner_h, _, _, _) = pending_editor_metrics(
                            ctx,
                            pending,
                            shared.thickness,
                            shared.text_font,
                        );
                        let frame_w = inner_w + 12.0;
                        let frame_h = inner_h + 8.0;
                        let live_box = pending.placement.box_rect(pending.pos, frame_w, frame_h);
                        if let Some(tex) = text_blur_tex {
                            draw_rounded_texture_uv(
                                painter,
                                tex,
                                virtual_size,
                                live_box,
                                live_box.translate(offset),
                                8.0,
                                Color32::WHITE,
                            );
                        } else {
                            let bg_local = live_box.translate(offset);
                            painter.rect_filled(
                                bg_local,
                                8.0,
                                Color32::from_rgba_unmultiplied(14, 18, 28, 210),
                            );
                            painter.rect_stroke(
                                bg_local,
                                8.0,
                                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 38)),
                                egui::StrokeKind::Middle,
                            );
                        }
                        if let Some(target) = pending.arrow_to {
                            let anchor = callout_anchor(live_box, target);
                            draw_egui_arrow(
                                painter,
                                anchor + offset,
                                target + offset,
                                Stroke::new(
                                    (shared.thickness * 0.85).max(2.0),
                                    shared.active_color,
                                ),
                            );
                        }
                    }

                    // Pointer interaction (converted to global virtual coordinates)
                    let resp = ui.allocate_rect(local_rect, egui::Sense::click_and_drag());
                    let to_global = |p: Pos2| p + self.mon_offset;

                    if shared.mode == Mode::WindowSelect {
                        self.window.set_cursor(CursorIcon::Pointer);
                        if let Some(hover_global) = shared.hover_pos {
                            if let Some(win) =
                                shared.windows.iter().find(|w| w.rect.contains(hover_global))
                            {
                                shared.selection = Some(win.rect);
                                shared.hovered_window_label = Some(win.label.clone());
                            } else {
                                shared.selection = None;
                                shared.hovered_window_label = None;
                            }
                        }
                        if resp.clicked() && shared.selection.is_some() {
                            shared.mode = Mode::Annotating;
                        }
                    } else if shared.mode == Mode::Selecting {
                        self.window.set_cursor(CursorIcon::Crosshair);
                        if resp.drag_started() {
                            shared.drag_start = ctx
                                .input(|i| i.pointer.press_origin())
                                .or_else(|| resp.interact_pointer_pos())
                                .map(to_global);
                        }
                        if resp.dragged() {
                            if let (Some(a), Some(b)) =
                                (shared.drag_start, resp.interact_pointer_pos().map(to_global))
                            {
                                shared.selection =
                                    Some(Rect::from_two_pos(a, b).intersect(virtual_rect));
                            }
                        }
                        if resp.drag_stopped() {
                            if let Some(sel) = shared.selection {
                                if sel.width() >= 12.0 && sel.height() >= 12.0 {
                                    shared.mode = Mode::Annotating;
                                } else {
                                    shared.selection = None;
                                }
                            }
                            shared.drag_start = None;
                        }
                    } else if shared.mode == Mode::Annotating {
                        if shared.active_tool == Tool::Text && resp.clicked() {
                            if let Some(pos) = resp.interact_pointer_pos().map(to_global) {
                                if shared.pending_text.is_some() {
                                    shared.commit_pending_text();
                                } else if let Some(idx) = shared.annotations.iter().rposition(|a| {
                                    matches!(a, Annotation::Text { .. }) && a.hit_test(pos)
                                }) {
                                    shared.open_text_for_edit(idx);
                                } else {
                                    shared.pending_text = Some(PendingText {
                                        pos,
                                        arrow_to: None,
                                        placement: TextPlacement::LeftTop,
                                        buf: String::new(),
                                        just_opened: true,
                                        original: None,
                                    });
                                }
                            }
                        }

                        if (shared.active_tool == Tool::Pen || shared.active_tool == Tool::Marker)
                            && resp.clicked()
                        {
                            if let Some(pos) = resp.interact_pointer_pos().map(to_global) {
                                let ann = match shared.active_tool {
                                    Tool::Pen => Annotation::Pen {
                                        points: vec![pos],
                                        color: shared.active_color,
                                        thickness: shared.thickness,
                                    },
                                    _ => Annotation::Marker {
                                        points: vec![pos],
                                        color: shared.active_color,
                                        thickness: shared.thickness,
                                    },
                                };
                                shared.annotations.push(ann);
                                shared.last_edited_idx = Some(shared.annotations.len() - 1);
                                shared.selected_annotation = None;
                            }
                        }

                        if shared.active_tool == Tool::Step && resp.clicked() {
                            if let Some(pos) = resp.interact_pointer_pos().map(to_global) {
                                shared.annotations.push(Annotation::Step {
                                    pos,
                                    number: shared.step_counter,
                                    color: shared.active_color,
                                    radius: shared.step_radius(),
                                });
                                shared.last_edited_idx = Some(shared.annotations.len() - 1);
                                shared.selected_annotation = None;
                                shared.step_counter += 1;
                            }
                        }

                        if shared.active_tool == Tool::Move && resp.double_clicked() {
                            if let Some(pos) = resp.interact_pointer_pos().map(to_global) {
                                if let Some(idx) = shared.annotations.iter().rposition(|a| {
                                    matches!(a, Annotation::Text { .. }) && a.hit_test(pos)
                                }) {
                                    shared.open_text_for_edit(idx);
                                }
                            }
                        } else if shared.active_tool == Tool::Move && resp.clicked() {
                            if let Some(pos) = resp.interact_pointer_pos().map(to_global) {
                                shared.selected_annotation =
                                    shared.annotations.iter().rposition(|a| a.hit_test(pos));
                                if shared.selected_annotation.is_some() {
                                    shared.last_edited_idx = shared.selected_annotation;
                                }
                            }
                        }

                        let cursor = match shared.active_tool {
                            Tool::Move => {
                                if shared.drag_action.is_some() {
                                    CursorIcon::Grabbing
                                } else {
                                    CursorIcon::Default
                                }
                            }
                            Tool::Text => CursorIcon::Text,
                            _ => CursorIcon::Crosshair,
                        };
                        self.window.set_cursor(cursor);

                        if resp.drag_started() {
                            let start_pos = ctx
                                .input(|i| i.pointer.press_origin())
                                .or_else(|| resp.interact_pointer_pos())
                                .map(to_global);
                            if let Some(pos) = start_pos {
                                shared.commit_pending_text();
                                shared.drag_start = Some(pos);
                                shared.drag_curr = Some(pos);

                                match shared.active_tool {
                                    Tool::Move => {
                                        let hit_ctrl = shared.selected_annotation.and_then(|idx| {
                                            shared.annotations.get(idx).and_then(|ann| {
                                                ann.control_points()
                                                    .into_iter()
                                                    .enumerate()
                                                    .find(|(_, cp)| cp.distance(pos) <= 12.0)
                                                    .map(|(h, _)| (idx, h))
                                            })
                                        });
                                        if let Some((idx, handle_idx)) = hit_ctrl {
                                            shared.last_edited_idx = Some(idx);
                                            shared.drag_action =
                                                Some(DragAction::EditAnnotationHandle {
                                                    idx,
                                                    handle_idx,
                                                });
                                        } else if !shared.live_draw
                                            && shared
                                                .selection
                                                .and_then(|s| hit_resize_handle(s, pos))
                                                .is_some()
                                        {
                                            let handle = shared
                                                .selection
                                                .and_then(|s| hit_resize_handle(s, pos))
                                                .unwrap();
                                            shared.drag_action =
                                                Some(DragAction::ResizeSelection { handle });
                                        } else if !shared.live_draw
                                            && shared
                                                .selection
                                                .is_some_and(|s| hit_selection_border(s, pos))
                                        {
                                            shared.drag_action =
                                                Some(DragAction::MoveSelectionAndAll {
                                                    last_pos: pos,
                                                });
                                        } else if let Some(idx) =
                                            shared.annotations.iter().rposition(|a| a.hit_test(pos))
                                        {
                                            shared.selected_annotation = Some(idx);
                                            shared.last_edited_idx = Some(idx);
                                            shared.drag_action = Some(DragAction::MoveAnnotation {
                                                idx,
                                                last_pos: pos,
                                            });
                                        } else {
                                            shared.selected_annotation = None;
                                        }
                                    }
                                    Tool::Pen => {
                                        shared.drag_action = Some(DragAction::DrawAnnotation);
                                        shared.in_progress = Some(Annotation::Pen {
                                            points: vec![pos],
                                            color: shared.active_color,
                                            thickness: shared.thickness,
                                        });
                                    }
                                    Tool::Marker => {
                                        shared.drag_action = Some(DragAction::DrawAnnotation);
                                        shared.in_progress = Some(Annotation::Marker {
                                            points: vec![pos],
                                            color: shared.active_color,
                                            thickness: shared.thickness,
                                        });
                                    }
                                    Tool::Text => {
                                        shared.drag_action = Some(DragAction::TextCallout);
                                    }
                                    Tool::Step => {
                                        shared.annotations.push(Annotation::Step {
                                            pos,
                                            number: shared.step_counter,
                                            color: shared.active_color,
                                            radius: shared.step_radius(),
                                        });
                                        shared.last_edited_idx = Some(shared.annotations.len() - 1);
                                        shared.selected_annotation = None;
                                        shared.step_counter += 1;
                                        shared.drag_start = None;
                                    }
                                    Tool::Blur => {
                                        let hit_ctrl = shared.selected_annotation.and_then(|idx| {
                                            shared.annotations.get(idx).and_then(|ann| {
                                                if matches!(ann, Annotation::Blur { .. }) {
                                                    ann.control_points()
                                                        .into_iter()
                                                        .enumerate()
                                                        .find(|(_, cp)| cp.distance(pos) <= 12.0)
                                                        .map(|(h, _)| (idx, h))
                                                } else {
                                                    None
                                                }
                                            })
                                        });
                                        if let Some((idx, handle_idx)) = hit_ctrl {
                                            shared.last_edited_idx = Some(idx);
                                            shared.drag_action =
                                                Some(DragAction::EditAnnotationHandle {
                                                    idx,
                                                    handle_idx,
                                                });
                                        } else {
                                            if shared.live_draw
                                                && shared.virtual_screen.image.width() <= 2
                                            {
                                                if let Ok(screens) = capture_all_monitors() {
                                                    shared.virtual_screen =
                                                        composite_monitors(&screens);
                                                }
                                            }
                                            shared.drag_action = Some(DragAction::DrawAnnotation);
                                        }
                                    }
                                    Tool::Line
                                    | Tool::Arrow
                                    | Tool::Ellipse
                                    | Tool::Rect => {
                                        shared.drag_action = Some(DragAction::DrawAnnotation);
                                    }
                                }
                            }
                        }

                        if resp.dragged() {
                            if let Some(curr) = resp.interact_pointer_pos().map(to_global) {
                                shared.drag_curr = Some(curr);
                                match shared.drag_action {
                                    Some(DragAction::EditAnnotationHandle { idx, handle_idx }) => {
                                        if let Some(ann) = shared.annotations.get_mut(idx) {
                                            ann.move_control_point(handle_idx, curr);
                                        }
                                        if matches!(
                                            shared.annotations.get(idx),
                                            Some(Annotation::Blur { .. } | Annotation::Text { .. })
                                        ) {
                                            shared.rebake_blurs();
                                        }
                                    }
                                    Some(DragAction::ResizeSelection { handle }) => {
                                        if let Some(sel) = &mut shared.selection {
                                            *sel = apply_resize(*sel, handle, curr, virtual_rect);
                                        }
                                    }
                                    Some(DragAction::MoveSelectionAndAll { last_pos }) => {
                                        if let Some(sel) = &mut shared.selection {
                                            let raw_delta = curr - last_pos;
                                            let dx = raw_delta.x.clamp(
                                                virtual_rect.left() - sel.left(),
                                                virtual_rect.right() - sel.right(),
                                            );
                                            let dy = raw_delta.y.clamp(
                                                virtual_rect.top() - sel.top(),
                                                virtual_rect.bottom() - sel.bottom(),
                                            );
                                            let delta = Vec2::new(dx, dy);
                                            *sel = sel.translate(delta);
                                            for ann in &mut shared.annotations {
                                                ann.translate(delta);
                                            }
                                        }
                                        shared.drag_action = Some(DragAction::MoveSelectionAndAll {
                                            last_pos: curr,
                                        });
                                    }
                                    Some(DragAction::MoveAnnotation { idx, last_pos }) => {
                                        let delta = curr - last_pos;
                                        if let Some(ann) = shared.annotations.get_mut(idx) {
                                            ann.translate(delta);
                                        }
                                        shared.drag_action = Some(DragAction::MoveAnnotation {
                                            idx,
                                            last_pos: curr,
                                        });
                                    }
                                    Some(DragAction::TextCallout) => {}
                                    Some(DragAction::DrawAnnotation) => {
                                        if let Some(start) = shared.drag_start {
                                            match shared.active_tool {
                                                Tool::Pen => {
                                                    if let Some(Annotation::Pen { points, .. }) =
                                                        &mut shared.in_progress
                                                    {
                                                        if points
                                                            .last()
                                                            .is_none_or(|p| p.distance(curr) > 3.5)
                                                        {
                                                            points.push(curr);
                                                        }
                                                    }
                                                }
                                                Tool::Marker => {
                                                    if let Some(Annotation::Marker {
                                                        points, ..
                                                    }) = &mut shared.in_progress
                                                    {
                                                        if points
                                                            .last()
                                                            .is_none_or(|p| p.distance(curr) > 3.5)
                                                        {
                                                            points.push(curr);
                                                        }
                                                    }
                                                }
                                                Tool::Line => {
                                                    let end = if shift_held {
                                                        snap_angle(
                                                            start,
                                                            curr,
                                                            shared.snap_angle_deg as f32,
                                                        )
                                                    } else {
                                                        curr
                                                    };
                                                    shared.in_progress = Some(Annotation::Line {
                                                        start,
                                                        end,
                                                        color: shared.active_color,
                                                        thickness: shared.thickness,
                                                    });
                                                }
                                                Tool::Arrow => {
                                                    let end = if shift_held {
                                                        snap_angle(
                                                            start,
                                                            curr,
                                                            shared.snap_angle_deg as f32,
                                                        )
                                                    } else {
                                                        curr
                                                    };
                                                    shared.in_progress = Some(Annotation::Arrow {
                                                        start,
                                                        end,
                                                        color: shared.active_color,
                                                        thickness: shared.thickness,
                                                    });
                                                }
                                                Tool::Ellipse => {
                                                    let end = if shift_held {
                                                        snap_square(start, curr)
                                                    } else {
                                                        curr
                                                    };
                                                    shared.in_progress = Some(Annotation::Ellipse {
                                                        rect: Rect::from_two_pos(start, end),
                                                        color: shared.active_color,
                                                        thickness: shared.thickness,
                                                        filled: cmd_held,
                                                    });
                                                }
                                                Tool::Rect => {
                                                    let end = if shift_held {
                                                        snap_square(start, curr)
                                                    } else {
                                                        curr
                                                    };
                                                    shared.in_progress = Some(Annotation::Rect {
                                                        rect: Rect::from_two_pos(start, end),
                                                        color: shared.active_color,
                                                        thickness: shared.thickness,
                                                        filled: cmd_held,
                                                    });
                                                }
                                                Tool::Blur => {
                                                    shared.in_progress = Some(Annotation::Blur {
                                                        rect: Rect::from_two_pos(start, curr),
                                                        radius: shared.blur_radius(),
                                                    });
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                    None => {}
                                }
                            }
                        }

                        if resp.drag_stopped() {
                            if matches!(shared.drag_action, Some(DragAction::TextCallout)) {
                                if let (Some(start), Some(curr)) = (shared.drag_start, shared.drag_curr)
                                {
                                    let (text_pos, arrow_to, placement) =
                                        if start.distance(curr) >= 10.0 {
                                            (curr, Some(start), TextPlacement::from_drag(start, curr))
                                        } else {
                                            (start, None, TextPlacement::LeftTop)
                                        };
                                    shared.pending_text = Some(PendingText {
                                        pos: text_pos,
                                        arrow_to,
                                        placement,
                                        buf: String::new(),
                                        just_opened: true,
                                        original: None,
                                    });
                                }
                            }
                            if let Some(ann) = shared.in_progress.take() {
                                let select_after = matches!(ann, Annotation::Blur { .. });
                                shared.annotations.push(ann);
                                let new_idx = shared.annotations.len() - 1;
                                shared.last_edited_idx = Some(new_idx);
                                shared.selected_annotation = if select_after {
                                    Some(new_idx)
                                } else {
                                    None
                                };
                                shared.rebake_blurs();
                            } else if matches!(
                                shared.drag_action,
                                Some(
                                    DragAction::MoveAnnotation { .. }
                                        | DragAction::EditAnnotationHandle { .. }
                                        | DragAction::MoveSelectionAndAll { .. }
                                )
                            ) {
                                shared.rebake_blurs();
                            }
                            shared.drag_start = None;
                            shared.drag_curr = None;
                            shared.drag_action = None;
                        }
                    }
                });

            // 2. Inline Text Editor (shown on the pane containing pending_text.pos)
            if let Some(pending) = &mut shared.pending_text {
                let local_pt = pending.pos + offset;
                if local_rect.expand(40.0).contains(local_pt) {
                    let default_thick = shared.thickness;
                    let default_font = shared.text_font;
                    let (box_w, box_h, font_size, edit_font, orig_wrap) =
                        pending_editor_metrics(ctx, pending, default_thick, default_font);
                    let frame_w = box_w + 12.0;
                    let frame_h = box_h + 8.0;
                    let editor_rect = pending.placement.box_rect(local_pt, frame_w, frame_h);
                    let editor_pos = Pos2::new(
                        editor_rect.min.x.clamp(8.0, (local_rect.max.x - frame_w - 8.0).max(8.0)),
                        editor_rect.min.y.clamp(8.0, (local_rect.max.y - frame_h - 8.0).max(8.0)),
                    );
                    let h_align = pending.placement.h_align();
                    let just_opened = pending.just_opened;
                    pending.just_opened = false;

                    egui::Area::new(egui::Id::new("inline_text_editor"))
                        .fixed_pos(editor_pos)
                        .order(egui::Order::Foreground)
                        .show(ctx, |ui| {
                            egui::Frame::NONE
                                .fill(Color32::from_rgba_unmultiplied(
                                    shared.theme.surface_container.r(),
                                    shared.theme.surface_container.g(),
                                    shared.theme.surface_container.b(),
                                    110,
                                ))
                                .stroke(Stroke::new(1.4_f32, shared.theme.accent))
                                .corner_radius(CornerRadius::same(8))
                                .inner_margin(egui::Margin::symmetric(6, 4))
                                .show(ui, |ui| {
                                    ui.set_min_width(box_w);
                                    ui.set_max_width(box_w);
                                    let edit = egui::TextEdit::multiline(&mut pending.buf)
                                        .desired_width(orig_wrap.unwrap_or(f32::INFINITY))
                                        .desired_rows(1)
                                        .font(egui::FontId::new(
                                            font_size,
                                            egui::FontFamily::Name(
                                                edit_font.egui_family().into(),
                                            ),
                                        ))
                                        .horizontal_align(h_align)
                                        .text_color(shared.active_color)
                                        .frame(false);
                                    let resp = ui.add(edit);
                                    if just_opened {
                                        resp.request_focus();
                                    }
                                });
                        });
                }
            }

            // 3. Floating Toolbar (Horizontal or Vertical based on Settings)
            if draw_toolbar_here && shared.mode == Mode::Annotating {
                if let Some(sel_global) = shared.selection {
                    let sel = sel_global.translate(offset);
                    let is_vert = shared.toolbar_layout == ToolbarLayout::Vertical;
                    let grip_pad = 10.0;
                    let (bar_w, bar_h) = if is_vert {
                        (96.0, 524.0 + grip_pad)
                    } else {
                        (554.0 + grip_pad, 84.0)
                    };

                    let (default_x, default_y) = if is_vert {
                        let right_fits = sel.max.x + bar_w + 18.0 <= local_rect.max.x;
                        let x = if right_fits {
                            sel.max.x + 12.0
                        } else if sel.min.x - bar_w - 18.0 >= 0.0 {
                            sel.min.x - bar_w - 12.0
                        } else {
                            (local_rect.max.x - bar_w - 14.0).max(12.0)
                        };
                        let y = (sel.center().y - bar_h * 0.5)
                            .clamp(12.0, (local_rect.max.y - bar_h - 12.0).max(12.0));
                        (x, y)
                    } else {
                        let x = (sel.center().x - bar_w * 0.5)
                            .clamp(12.0, (local_rect.max.x - bar_w - 12.0).max(12.0));
                        let below = sel.max.y + bar_h + 26.0 <= local_rect.max.y;
                        let y = if below {
                            sel.max.y + 12.0
                        } else if sel.min.y - bar_h - 18.0 >= 0.0 {
                            sel.min.y - bar_h - 12.0
                        } else {
                            (local_rect.max.y - bar_h - 28.0).max(16.0)
                        };
                        (x, y)
                    };

                    let (tx, ty) = if let Some(pos) = shared.toolbar_pos {
                        (
                            pos.x.clamp(8.0, (local_rect.max.x - bar_w - 8.0).max(8.0)),
                            pos.y.clamp(8.0, (local_rect.max.y - bar_h - 8.0).max(8.0)),
                        )
                    } else {
                        (default_x, default_y)
                    };

                    let pop_id = egui::Id::new("toolbar_pop_in");
                    if ctx.data(|d| d.get_temp::<(f32, f32)>(pop_id)).is_none() {
                        ctx.data_mut(|d| d.insert_temp(pop_id, (0.0_f32, 0.0_f32)));
                    }
                    let pop_t = spring_animate(ctx, pop_id, 1.0, 0.28, 0.72);
                    let pop_offset_y = (1.0 - pop_t) * 10.0;
                    let bar_rect = Rect::from_min_size(
                        Pos2::new(tx, ty + pop_offset_y),
                        Vec2::new(bar_w, bar_h),
                    );
                    let bar_radius = 14.0;

                    egui::Area::new(egui::Id::new("m3_toolbar"))
                        .fixed_pos(bar_rect.min)
                        .order(egui::Order::Foreground)
                        .show(ctx, |ui| {
                            draw_m3_panel(ui.painter(), bar_rect, bar_radius, &shared.theme);

                            let grip_rect = if is_vert {
                                Rect::from_min_size(bar_rect.min, Vec2::new(bar_w, grip_pad + 4.0))
                            } else {
                                Rect::from_min_size(bar_rect.min, Vec2::new(grip_pad + 4.0, bar_h))
                            };
                            let grip_resp = ui.interact(
                                grip_rect,
                                egui::Id::new("toolbar_grip"),
                                egui::Sense::drag(),
                            );
                            if grip_resp.dragged() {
                                ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
                                let d = grip_resp.drag_delta();
                                shared.toolbar_pos = Some(Pos2::new(
                                    (bar_rect.min.x + d.x)
                                        .clamp(8.0, (local_rect.max.x - bar_w - 8.0).max(8.0)),
                                    (bar_rect.min.y + d.y)
                                        .clamp(8.0, (local_rect.max.y - bar_h - 8.0).max(8.0)),
                                ));
                            } else if grip_resp.hovered() {
                                ctx.set_cursor_icon(egui::CursorIcon::Grab);
                            }

                            let grip_hov = spring_animate(
                                ctx,
                                egui::Id::new("toolbar_grip_hov"),
                                if grip_resp.hovered() || grip_resp.dragged() {
                                    1.0
                                } else {
                                    0.0
                                },
                                0.22,
                                0.65,
                            )
                            .clamp(0.0, 1.2);
                            let grip_alpha = (205.0 + 50.0 * grip_hov).round() as u8;
                            let ac = shared.theme.accent;
                            let grip_col =
                                Color32::from_rgba_unmultiplied(ac.r(), ac.g(), ac.b(), grip_alpha);
                            let half_w = 1.0 + 0.4 * grip_hov;
                            let end_pad = 12.0 - 3.0 * grip_hov;
                            let grip_pill = if is_vert {
                                Rect::from_min_max(
                                    Pos2::new(grip_rect.left() + end_pad, grip_rect.top() + 7.0 - half_w),
                                    Pos2::new(grip_rect.right() - end_pad, grip_rect.top() + 7.0 + half_w),
                                )
                            } else {
                                Rect::from_min_max(
                                    Pos2::new(grip_rect.left() + 8.0 - half_w, grip_rect.top() + end_pad),
                                    Pos2::new(grip_rect.left() + 8.0 + half_w, grip_rect.bottom() - end_pad),
                                )
                            };
                            ui.painter()
                                .rect_filled(grip_pill, CornerRadius::same(2), grip_col);

                            let tools: [(Tool, &str, &str, fn(&egui::Painter, Rect, Color32)); 10] = [
                                (Tool::Move, "t_move", "Move / Resize [1]", icons::draw_move_icon),
                                (Tool::Pen, "t_pen", "Pen [2]", icons::draw_pen_icon),
                                (Tool::Marker, "t_marker", "Marker [3]", icons::draw_marker_icon),
                                (Tool::Line, "t_line", "Line [4] (Shift — snap angle)", icons::draw_line_icon),
                                (Tool::Arrow, "t_arrow", "Arrow [5] (Shift — snap angle)", icons::draw_arrow_icon),
                                (Tool::Text, "t_text", "Text / Callout [6]", icons::draw_text_icon),
                                (
                                    Tool::Ellipse,
                                    "t_ellipse",
                                    if cfg!(target_os = "macos") {
                                        "Ellipse [7] (Cmd — filled, Shift — circle)"
                                    } else {
                                        "Ellipse [7] (Ctrl — filled, Shift — circle)"
                                    },
                                    if cmd_held {
                                        |p, r, c| icons::draw_ellipse_icon(p, r, c, true)
                                    } else {
                                        |p, r, c| icons::draw_ellipse_icon(p, r, c, false)
                                    },
                                ),
                                (
                                    Tool::Rect,
                                    "t_rect",
                                    if cfg!(target_os = "macos") {
                                        "Rectangle [8] (Cmd — filled, Shift — square)"
                                    } else {
                                        "Rectangle [8] (Ctrl — filled, Shift — square)"
                                    },
                                    if cmd_held {
                                        |p, r, c| icons::draw_rect_icon(p, r, c, true)
                                    } else {
                                        |p, r, c| icons::draw_rect_icon(p, r, c, false)
                                    },
                                ),
                                (Tool::Step, "t_step", "Numbering [9]", icons::draw_step_icon),
                                (Tool::Blur, "t_blur", "Blur [0]", icons::draw_blur_icon),
                            ];

                            let prev_color_pos = shared.color_pos;
                            let prev_thick = shared.thickness;

                            if is_vert {
                                ui.add_space(grip_pad);
                                ui.allocate_ui_with_layout(
                                    Vec2::new(bar_w, bar_h - grip_pad),
                                    egui::Layout::left_to_right(egui::Align::Min),
                                    |ui| {
                                        ui.add_space(6.0);
                                        // Left column: Tools + Divider + Actions
                                        ui.allocate_ui_with_layout(
                                            Vec2::new(40.0, bar_h),
                                            egui::Layout::top_down(egui::Align::Center),
                                            |ui| {
                                                ui.add_space(8.0);
                                                ui.spacing_mut().item_spacing.y = 3.5;
                                                let btn_sz = Vec2::splat(32.0);

                                                for (tool, id_salt, tip, draw_fn) in tools {
                                                    if m3_tool_button(
                                                        ui,
                                                        id_salt,
                                                        btn_sz,
                                                        shared.active_tool == tool,
                                                        tip,
                                                        &shared.theme,
                                                        draw_fn,
                                                    )
                                                    .clicked()
                                                    {
                                                        shared.commit_pending_text();
                                                        shared.active_tool = tool;
                                                    }
                                                }

                                                m3_divider(ui, &shared.theme, true);

                                                if m3_action_button(
                                                    ui,
                                                    btn_sz,
                                                    false,
                                                    if cfg!(target_os = "macos") { "Undo (Cmd+Z)" } else { "Undo (Ctrl+Z)" },
                                                    &shared.theme,
                                                    icons::draw_undo_icon,
                                                )
                                                .clicked()
                                                {
                                                    shared.undo();
                                                }
                                                if m3_action_button(
                                                    ui,
                                                    btn_sz,
                                                    true,
                                                    if cfg!(target_os = "macos") { "Copy (Cmd+C / Enter)" } else { "Copy (Ctrl+C / Enter)" },
                                                    &shared.theme,
                                                    icons::draw_copy_icon,
                                                )
                                                .clicked()
                                                {
                                                    if shared.live_draw {
                                                        outcome = OverlayOutcome::LiveCopy;
                                                    } else if shared.export_and_copy() {
                                                        outcome = OverlayOutcome::Close;
                                                    }
                                                }
                                                if m3_action_button(
                                                    ui,
                                                    btn_sz,
                                                    false,
                                                    if cfg!(target_os = "macos") { "Save (Cmd+S)" } else { "Save (Ctrl+S)" },
                                                    &shared.theme,
                                                    icons::draw_save_icon,
                                                )
                                                .clicked()
                                                {
                                                    if shared.live_draw {
                                                        outcome = OverlayOutcome::LiveSave;
                                                    } else if shared.export_and_save() {
                                                        outcome = OverlayOutcome::Close;
                                                    }
                                                }
                                                if m3_action_button(
                                                    ui,
                                                    btn_sz,
                                                    false,
                                                    "Close (Esc)",
                                                    &shared.theme,
                                                    icons::draw_close_icon,
                                                )
                                                .clicked()
                                                {
                                                    outcome = OverlayOutcome::Close;
                                                }
                                            },
                                        );

                                        // Vertical hairline divider between tools and sliders
                                        let sep_x = bar_rect.left() + 51.0;
                                        ui.painter().line_segment(
                                            [
                                                Pos2::new(sep_x, bar_rect.top() + grip_pad + 10.0),
                                                Pos2::new(sep_x, bar_rect.bottom() - 10.0),
                                            ],
                                            Stroke::new(1.0_f32, shared.theme.outline),
                                        );

                                        // Right column: Color slider (with preset dots) + 1.5x Size slider
                                        ui.add_space(6.0);
                                        ui.allocate_ui_with_layout(
                                            Vec2::new(36.0, bar_h),
                                            egui::Layout::top_down(egui::Align::Center),
                                            |ui| {
                                                ui.add_space(10.0);
                                                ui.spacing_mut().item_spacing.y = 10.0;
                                                shared.active_color = m3_color_slider_vertical(
                                                    ui,
                                                    344.0,
                                                    &mut shared.color_pos,
                                                    &shared.theme,
                                                );
                                                m3_size_slider_vertical(
                                                    ui,
                                                    140.0,
                                                    &mut shared.thickness,
                                                    1.5,
                                                    20.0,
                                                    &shared.theme,
                                                );
                                            },
                                        );
                                    },
                                );
                            } else {
                                let content_rect = Rect::from_min_max(
                                    bar_rect.min + Vec2::new(grip_pad, 0.0),
                                    bar_rect.max,
                                );
                                ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
                                    ui.allocate_ui_with_layout(
                                        content_rect.size(),
                                        egui::Layout::top_down(egui::Align::Min),
                                        |ui| {
                                        ui.add_space(8.0);
                                        let pad_x = 10.0;
                                        let btn_sz = Vec2::splat(32.0);

                                        // Row 1: All tool & action buttons
                                        ui.horizontal(|ui| {
                                            ui.add_space(pad_x);
                                            ui.spacing_mut().item_spacing.x = 4.5;

                                            for (tool, id_salt, tip, draw_fn) in tools {
                                                if m3_tool_button(
                                                    ui,
                                                    id_salt,
                                                    btn_sz,
                                                    shared.active_tool == tool,
                                                    tip,
                                                    &shared.theme,
                                                    draw_fn,
                                                )
                                                .clicked()
                                                {
                                                    shared.commit_pending_text();
                                                    shared.active_tool = tool;
                                                }
                                            }

                                            m3_divider(ui, &shared.theme, false);

                                            if m3_action_button(
                                                ui,
                                                btn_sz,
                                                false,
                                                if cfg!(target_os = "macos") { "Undo (Cmd+Z)" } else { "Undo (Ctrl+Z)" },
                                                &shared.theme,
                                                icons::draw_undo_icon,
                                            )
                                            .clicked()
                                            {
                                                shared.undo();
                                            }

                                            if m3_action_button(
                                                ui,
                                                btn_sz,
                                                true,
                                                if cfg!(target_os = "macos") { "Copy (Cmd+C / Enter)" } else { "Copy (Ctrl+C / Enter)" },
                                                &shared.theme,
                                                icons::draw_copy_icon,
                                            )
                                            .clicked()
                                            {
                                                if shared.live_draw {
                                                    outcome = OverlayOutcome::LiveCopy;
                                                } else if shared.export_and_copy() {
                                                    outcome = OverlayOutcome::Close;
                                                }
                                            }

                                            if m3_action_button(
                                                ui,
                                                btn_sz,
                                                false,
                                                if cfg!(target_os = "macos") { "Save (Cmd+S)" } else { "Save (Ctrl+S)" },
                                                &shared.theme,
                                                icons::draw_save_icon,
                                            )
                                            .clicked()
                                            {
                                                if shared.live_draw {
                                                    outcome = OverlayOutcome::LiveSave;
                                                } else if shared.export_and_save() {
                                                    outcome = OverlayOutcome::Close;
                                                }
                                            }

                                            if m3_action_button(
                                                ui,
                                                btn_sz,
                                                false,
                                                "Close (Esc)",
                                                &shared.theme,
                                                icons::draw_close_icon,
                                            )
                                            .clicked()
                                            {
                                                outcome = OverlayOutcome::Close;
                                            }
                                        });

                                        ui.add_space(4.0);
                                        let sep_y = ui.cursor().top();
                                        ui.painter().line_segment(
                                            [
                                                Pos2::new(bar_rect.left() + grip_pad + pad_x, sep_y),
                                                Pos2::new(bar_rect.right() - pad_x, sep_y),
                                            ],
                                            Stroke::new(1.0_f32, shared.theme.outline),
                                        );
                                        ui.add_space(5.0);

                                        // Row 2: Stretched color slider + preset dots on left, 1.5x size slider on right
                                        ui.horizontal(|ui| {
                                            ui.add_space(pad_x);
                                            ui.spacing_mut().item_spacing.x = 12.0;
                                            let color_w = (content_rect.width() - pad_x * 2.0 - 138.0 - 12.0).max(200.0);
                                            shared.active_color = m3_color_slider(
                                                ui,
                                                color_w,
                                                &mut shared.color_pos,
                                                &shared.theme,
                                            );
                                            m3_size_slider(
                                                ui,
                                                &mut shared.thickness,
                                                1.5,
                                                20.0,
                                                &shared.theme,
                                            );
                                        });
                                    },
                                );
                                });
                            }

                            let target_idx = shared.selected_annotation.or(shared.last_edited_idx);
                            if let Some(idx) = target_idx {
                                let mut needs_rebake = false;
                                if let Some(ann) = shared.annotations.get_mut(idx) {
                                    if (shared.color_pos - prev_color_pos).abs() > 1e-4 {
                                        ann.set_color(shared.active_color);
                                        needs_rebake = true;
                                    }
                                    if (shared.thickness - prev_thick).abs() > 1e-4 {
                                        ann.apply_thickness(shared.thickness);
                                        needs_rebake = true;
                                    }
                                }
                                if needs_rebake
                                    && shared
                                        .annotations
                                        .iter()
                                        .any(|a| matches!(a, Annotation::Blur { .. } | Annotation::Text { .. }))
                                {
                                    shared.rebake_blurs();
                                }
                            }
                        });
                }
            }
        });

        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);

        let clipped = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        let size = self.window.inner_size();
        unsafe {
            use glow::HasContext;
            self.gl.viewport(0, 0, size.width as i32, size.height as i32);
            self.gl.clear_color(0.0, 0.0, 0.0, if self.live_draw { 0.0 } else { 1.0 });
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }

        self.egui_painter.paint_and_update_textures(
            [size.width, size.height],
            full_output.pixels_per_point,
            &clipped,
            &full_output.textures_delta,
        );
        let _ = self.gl_surface.swap_buffers(&self.gl_context);

        if full_output
            .viewport_output
            .values()
            .any(|v| v.repaint_delay.is_zero())
        {
            self.window.request_redraw();
        }

        outcome
    }
}

fn m3_divider(ui: &mut egui::Ui, theme: &ThemeColors, vertical_bar: bool) {
    if vertical_bar {
        let (div_rect, _) = ui.allocate_exact_size(Vec2::new(24.0, 6.0), egui::Sense::hover());
        ui.painter().line_segment(
            [
                Pos2::new(div_rect.left(), div_rect.center().y),
                Pos2::new(div_rect.right(), div_rect.center().y),
            ],
            Stroke::new(1.0_f32, theme.outline),
        );
    } else {
        let (div_rect, _) = ui.allocate_exact_size(Vec2::new(6.0, 22.0), egui::Sense::hover());
        ui.painter().line_segment(
            [
                Pos2::new(div_rect.center().x, div_rect.top()),
                Pos2::new(div_rect.center().x, div_rect.bottom()),
            ],
            Stroke::new(1.0_f32, theme.outline),
        );
    }
}

fn hit_selection_border(sel: Rect, pos: Pos2) -> bool {
    let outer = sel.expand(10.0);
    let inner = sel.shrink(10.0);
    outer.contains(pos) && !inner.contains(pos)
}

fn hit_resize_handle(sel: Rect, pos: Pos2) -> Option<ResizeHandle> {
    let handles = [
        (ResizeHandle::TopLeft, sel.left_top()),
        (ResizeHandle::Top, Pos2::new(sel.center().x, sel.top())),
        (ResizeHandle::TopRight, sel.right_top()),
        (ResizeHandle::Right, Pos2::new(sel.right(), sel.center().y)),
        (ResizeHandle::BottomRight, sel.right_bottom()),
        (ResizeHandle::Bottom, Pos2::new(sel.center().x, sel.bottom())),
        (ResizeHandle::BottomLeft, sel.left_bottom()),
        (ResizeHandle::Left, Pos2::new(sel.left(), sel.center().y)),
    ];
    handles
        .into_iter()
        .find(|(_, p)| p.distance(pos) <= 12.0)
        .map(|(h, _)| h)
}

fn apply_resize(sel: Rect, handle: ResizeHandle, pos: Pos2, bounds: Rect) -> Rect {
    let p = pos.clamp(bounds.min, bounds.max);
    let min_size = 24.0;
    let mut left = sel.left();
    let mut right = sel.right();
    let mut top = sel.top();
    let mut bottom = sel.bottom();

    match handle {
        ResizeHandle::TopLeft => {
            left = p.x.min(right - min_size);
            top = p.y.min(bottom - min_size);
        }
        ResizeHandle::Top => {
            top = p.y.min(bottom - min_size);
        }
        ResizeHandle::TopRight => {
            right = p.x.max(left + min_size);
            top = p.y.min(bottom - min_size);
        }
        ResizeHandle::Right => {
            right = p.x.max(left + min_size);
        }
        ResizeHandle::BottomRight => {
            right = p.x.max(left + min_size);
            bottom = p.y.max(top + min_size);
        }
        ResizeHandle::Bottom => {
            bottom = p.y.max(top + min_size);
        }
        ResizeHandle::BottomLeft => {
            left = p.x.min(right - min_size);
            bottom = p.y.max(top + min_size);
        }
        ResizeHandle::Left => {
            left = p.x.min(right - min_size);
        }
    }

    Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, bottom))
}

fn snap_square(start: Pos2, curr: Pos2) -> Pos2 {
    let dx = curr.x - start.x;
    let dy = curr.y - start.y;
    let side = dx.abs().max(dy.abs());
    Pos2::new(start.x + side * dx.signum(), start.y + side * dy.signum())
}

fn snap_angle(start: Pos2, curr: Pos2, snap_deg: f32) -> Pos2 {
    let d = curr - start;
    let len = d.length();
    if len < 1.0 {
        return curr;
    }
    let step = snap_deg.clamp(1.0, 90.0).to_radians();
    let angle = (d.y.atan2(d.x) / step).round() * step;
    start + Vec2::new(len * angle.cos(), len * angle.sin())
}

fn configure_m3_style(ctx: &egui::Context, active_text_font: TextFont) {
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

    // Load only the selected text font (zero wasted disk I/O / RAM for unused fonts)
    let fam = egui::FontFamily::Name(active_text_font.egui_family().into());
    match active_text_font {
        TextFont::GostTypeAU => {
            fonts.font_data.insert(
                "text_gost_au".to_owned(),
                Arc::new(egui::FontData::from_static(GOST_TYPE_AU_BYTES)),
            );
            fonts
                .families
                .entry(fam)
                .or_default()
                .push("text_gost_au".to_owned());
        }
        TextFont::Roboto => {
            fonts
                .families
                .entry(fam)
                .or_default()
                .push("roboto_m3".to_owned());
        }
        tf => {
            if let Some(bytes) = tf.system_path().and_then(|p| std::fs::read(p).ok()) {
                let key = tf.egui_family().to_owned();
                fonts
                    .font_data
                    .insert(key.clone(), Arc::new(egui::FontData::from_owned(bytes)));
                fonts.families.entry(fam).or_default().push(key);
            } else {
                fonts
                    .families
                    .entry(fam)
                    .or_default()
                    .push("roboto_m3".to_owned());
            }
        }
    }

    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    style.visuals.dark_mode = true;
    ctx.set_style(style);
}

fn pending_editor_metrics(
    ctx: &egui::Context,
    pending: &PendingText,
    thickness: f32,
    text_font: TextFont,
) -> (f32, f32, f32, TextFont, Option<f32>) {
    let (font_size, edit_font, orig_wrap) = match &pending.original {
        Some((_, Annotation::Text { size, font, box_size, .. })) => {
            (*size, *font, box_size.map(|s| s.x))
        }
        _ => (
            (13.0 + thickness * 2.2).clamp(16.0, 57.0),
            text_font,
            None,
        ),
    };
    let font_id = egui::FontId::new(
        font_size,
        egui::FontFamily::Name(edit_font.egui_family().into()),
    );
    let galley_sz = ctx.fonts(|f| {
        if let Some(w) = orig_wrap {
            f.layout(pending.buf.clone(), font_id, Color32::WHITE, w.max(24.0))
                .size()
        } else {
            f.layout_no_wrap(pending.buf.clone(), font_id, Color32::WHITE)
                .size()
        }
    });
    let box_w = if let Some(w) = orig_wrap {
        w.max(24.0)
    } else if pending.buf.is_empty() {
        10.0
    } else {
        (galley_sz.x + 14.0).clamp(10.0, 640.0)
    };
    let box_h = galley_sz.y.max(font_size * 1.25);
    (box_w, box_h, font_size, edit_font, orig_wrap)
}

#[cfg(target_os = "windows")]
fn win_ctrl_down() -> bool {
    unsafe extern "system" {
        fn GetAsyncKeyState(vkey: i32) -> i16;
    }
    const VK_CONTROL: i32 = 0x11;
    (unsafe { GetAsyncKeyState(VK_CONTROL) } as u16 & 0x8000) != 0
}

fn primary_mod_held(winit_mods: ModifiersState, egui_mods: Option<egui::Modifiers>) -> bool {
    #[cfg(target_os = "macos")]
    {
        winit_mods.super_key()
            || egui_mods.map_or(false, |m| m.mac_cmd || m.command)
    }
    #[cfg(target_os = "windows")]
    {
        winit_mods.control_key()
            || egui_mods.map_or(false, |m| m.ctrl)
            || win_ctrl_down()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        winit_mods.control_key() || egui_mods.map_or(false, |m| m.ctrl)
    }
}

pub struct CaptureFlashWindow {
    window: Window,
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,
    gl: Arc<glow::Context>,
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_painter: egui_glow::Painter,
    tex: egui::TextureHandle,
    logical_size: Vec2,
    accent: Color32,
    started_at: Option<std::time::Instant>,
}

impl Drop for CaptureFlashWindow {
    fn drop(&mut self) {
        let _ = self.gl_context.make_current(&self.gl_surface);
        self.egui_painter.destroy();
    }
}

impl CaptureFlashWindow {
    pub fn open(
        event_loop: &ActiveEventLoop,
        screen: CapturedScreen,
        accent: Color32,
    ) -> Result<Self, String> {
        #[allow(unused_mut)]
        let mut win_attrs = WindowAttributes::default()
            .with_title("ShotCraft Flash")
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_position(LogicalPosition::new(screen.x as f64, screen.y as f64))
            .with_inner_size(LogicalSize::new(screen.logical_width, screen.logical_height));

        #[cfg(target_os = "windows")]
        {
            use winit::platform::windows::WindowAttributesExtWindows;
            win_attrs = win_attrs
                .with_position(PhysicalPosition::new(screen.x, screen.y))
                .with_skip_taskbar(true);
        }

        let template = ConfigTemplateBuilder::new().with_alpha_size(8);
        let (window, gl_config) = DisplayBuilder::new()
            .with_window_attributes(Some(win_attrs))
            .build(event_loop, template, |configs| {
                configs
                    .reduce(|accum, config| {
                        if config.num_samples() < accum.num_samples() {
                            config
                        } else {
                            accum
                        }
                    })
                    .expect("No GL config found")
            })
            .map_err(|e| format!("Failed to create flash window: {e}"))?;

        let window = window.ok_or_else(|| "Flash window not created".to_string())?;
        let _ = window.set_cursor_hittest(false);

        let raw_handle = window
            .window_handle()
            .map_err(|e| format!("Window handle error: {e}"))?
            .as_raw();

        #[cfg(target_os = "macos")]
        if let raw_window_handle::RawWindowHandle::AppKit(appkit) = raw_handle {
            unsafe {
                let ns_view = appkit.ns_view.as_ptr() as *mut objc2::runtime::AnyObject;
                let ns_win: *mut objc2::runtime::AnyObject = objc2::msg_send![ns_view, window];
                if !ns_win.is_null() {
                    let _: () = objc2::msg_send![ns_win, setLevel: 1000_isize];
                }
            }
        }

        let gl_display = gl_config.display();
        let context_attrs = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(None))
            .build(Some(raw_handle));

        let not_current = unsafe {
            gl_display
                .create_context(&gl_config, &context_attrs)
                .map_err(|e| format!("GL context error: {e}"))?
        };

        let size = window.inner_size();
        let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            raw_handle,
            NonZeroU32::new(size.width.max(1)).unwrap(),
            NonZeroU32::new(size.height.max(1)).unwrap(),
        );
        let gl_surface = unsafe {
            gl_display
                .create_window_surface(&gl_config, &attrs)
                .map_err(|e| format!("GL surface error: {e}"))?
        };
        let gl_context = not_current
            .make_current(&gl_surface)
            .map_err(|e| format!("Make current error: {e}"))?;

        let _ = gl_surface.set_swap_interval(&gl_context, SwapInterval::DontWait);

        let gl = Arc::new(unsafe {
            glow::Context::from_loader_function(|s| {
                let c = CString::new(s).unwrap();
                gl_display.get_proc_address(&c) as *const _
            })
        });

        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let egui_painter = egui_glow::Painter::new(gl.clone(), "", None, false)
            .map_err(|e| format!("egui_glow error: {e}"))?;

        let img_size = [screen.image.width() as usize, screen.image.height() as usize];
        let color_img = ColorImage::from_rgba_unmultiplied(img_size, screen.image.as_raw());
        let tex = egui_ctx.load_texture("flash_shot", color_img, egui::TextureOptions::LINEAR);

        window.request_redraw();

        Ok(Self {
            window,
            gl_context,
            gl_surface,
            gl,
            egui_ctx,
            egui_state,
            egui_painter,
            tex,
            logical_size: Vec2::new(screen.logical_width, screen.logical_height),
            accent,
            started_at: None,
        })
    }

    pub fn window_id(&self) -> WindowId {
        self.window.id()
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    /// Returns `true` when the 20% shrink & fade-out animation is finished.
    pub fn handle_window_event(&mut self, event: &WindowEvent) -> bool {
        if !matches!(event, WindowEvent::RedrawRequested) {
            return false;
        }
        let start = *self.started_at.get_or_insert_with(std::time::Instant::now);
        let duration = 0.48_f32;
        let t = (start.elapsed().as_secs_f32() / duration).clamp(0.0, 1.0);
        if t >= 1.0 {
            return true;
        }

        let _ = self.gl_context.make_current(&self.gl_surface);
        let local_rect = Rect::from_min_size(Pos2::ZERO, self.logical_size);

        // Phase 1 (0..35% time): shrink by 30% toward screen center (scale 1.0 -> 0.70)
        // Phase 2 (35..100% time): shrink remaining 70% down to 0 toward bottom-center
        let (scale, center, alpha, dim_alpha, radius) = if t < 0.35 {
            let p1 = (t / 0.35).clamp(0.0, 1.0);
            let e1 = 1.0 - (1.0 - p1).powi(3);
            let sc = 1.0 - 0.30 * e1;
            let dim = (96.0 - 36.0 * e1).round() as u8;
            (sc, local_rect.center(), 255u8, dim, 14.0 * e1)
        } else {
            let p2 = ((t - 0.35) / 0.65).clamp(0.0, 1.0);
            let e2 = p2 * p2 * (3.0 - 2.0 * p2);
            let sc = (0.70 * (1.0 - e2)).max(0.002);
            let cy = local_rect.center().y + (local_rect.bottom() - local_rect.center().y) * e2;
            let c = Pos2::new(local_rect.center().x, cy);
            let a = ((1.0 - p2.powi(2)) * 255.0).round().clamp(0.0, 255.0) as u8;
            let dim = ((1.0 - e2) * 60.0).round().clamp(0.0, 60.0) as u8;
            (sc, c, a, dim, 14.0 + 10.0 * e2)
        };

        let raw_input = self.egui_state.take_egui_input(&self.window);
        let frame_rect = Rect::from_center_size(center, self.logical_size * scale);
        let tint = Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
        let border_col = Color32::from_rgba_unmultiplied(
            self.accent.r(),
            self.accent.g(),
            self.accent.b(),
            alpha,
        );

        let tex_id = self.tex.id();
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            egui::Area::new(egui::Id::new("capture_flash_area"))
                .fixed_pos(Pos2::ZERO)
                .show(ctx, |ui| {
                    let painter = ui.painter();
                    // Background: full-screen screenshot with the same dim tint as area selection
                    painter.image(
                        tex_id,
                        local_rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    painter.rect_filled(
                        local_rect,
                        0.0,
                        Color32::from_rgba_unmultiplied(6, 10, 20, dim_alpha),
                    );
                    // Foreground: shrinking 20% rounded frame
                    draw_rounded_texture_uv(
                        painter,
                        tex_id,
                        frame_rect.size(),
                        Rect::from_min_size(Pos2::ZERO, frame_rect.size()),
                        frame_rect,
                        radius,
                        tint,
                    );
                    painter.rect_stroke(
                        frame_rect,
                        radius,
                        Stroke::new(2.8_f32, border_col),
                        egui::StrokeKind::Outside,
                    );
                });
        });

        let clipped = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        let size = self.window.inner_size();
        unsafe {
            use glow::HasContext;
            self.gl.viewport(0, 0, size.width as i32, size.height as i32);
            self.gl.clear_color(0.0, 0.0, 0.0, 0.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }
        self.egui_painter.paint_and_update_textures(
            [size.width, size.height],
            full_output.pixels_per_point,
            &clipped,
            &full_output.textures_delta,
        );
        let _ = self.gl_surface.swap_buffers(&self.gl_context);
        self.window.request_redraw();
        false
    }
}
