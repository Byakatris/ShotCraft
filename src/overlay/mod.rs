pub mod flash;
pub mod input;
pub mod toolbar;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Color32, ColorImage, Key, Pos2, Rect, Stroke, Vec2};
use image::RgbaImage;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{WindowAttributes, WindowId, WindowLevel};

use crate::annotations::{
    bake_blur_patch, paint_annotations, render_export, text_content_rect, Annotation,
    TextPlacement, Tool, ROBOTO_FONT_BYTES,
};
use crate::capture::{
    composite_monitors_metadata, copy_to_clipboard, pane_virtual_offset, pane_virtual_scale,
    save_to_dir_async, take_composite_virtual_screen, CapturedScreen, WindowCandidate,
};
use crate::diagnostics;
use crate::gl_window::{GlEguiWindow, WindowChrome};
use crate::material::{
    color_from_slider, draw_m3_panel, draw_m3_selection_frame, make_strong_text_blur_image,
    ThemeColors,
};
use crate::settings::{Language, Strings, TextFont, ToolbarLayout};

pub use flash::CaptureFlashWindow;

const MAX_UNDO_STEPS: usize = 50;

pub struct OverlayConfig {
    pub screens: Vec<CapturedScreen>,
    pub windows: Vec<WindowCandidate>,
    pub window_select_mode: bool,
    pub live_draw: bool,
    pub save_dir: PathBuf,
    pub lang: Language,
    pub theme: ThemeColors,
    pub initial_color_pos: f32,
    pub initial_thickness: f32,
    pub snap_angle_deg: u16,
    pub text_font: TextFont,
    pub toolbar_layout: ToolbarLayout,
    pub initial_tool: Tool,
    pub initial_annotations: Vec<Annotation>,
    pub initial_step_counter: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayOutcome {
    KeepOpen,
    Close,
    LiveCopy,
    LiveSave,
    CaptureLiveBlurBackground,
}

#[derive(PartialEq, Eq)]
pub(super) enum Mode {
    Selecting,
    WindowSelect,
    Annotating,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum ResizeHandle {
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
pub(super) enum DragAction {
    DrawAnnotation,
    TextCallout,
    MoveAnnotation { idx: usize, last_pos: Pos2 },
    EditAnnotationHandle { idx: usize, handle_idx: usize },
    ResizeSelection { handle: ResizeHandle },
    MoveSelectionAndAll { last_pos: Pos2 },
}

pub(super) struct PendingText {
    pub(super) pos: Pos2,
    pub(super) arrow_to: Option<Pos2>,
    pub(super) placement: TextPlacement,
    pub(super) buf: String,
    pub(super) just_opened: bool,
    pub(super) original: Option<(usize, Annotation)>,
}

pub(super) struct ToolState {
    pub(super) active_tool: Tool,
    pub(super) color_pos: f32,
    pub(super) active_color: Color32,
    pub(super) thickness: f32,
    pub(super) step_counter: u32,
}

pub(super) struct UndoHistory {
    undo_stack: Vec<(Vec<Annotation>, u32)>,
    redo_stack: Vec<(Vec<Annotation>, u32)>,
    pub(super) slider_undo_pushed: bool,
}

impl UndoHistory {
    pub(super) fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            slider_undo_pushed: false,
        }
    }

    pub(super) fn push(&mut self, annotations: Vec<Annotation>, step_counter: u32) {
        self.undo_stack.push((annotations, step_counter));
        if self.undo_stack.len() > MAX_UNDO_STEPS {
            let overflow = self.undo_stack.len() - MAX_UNDO_STEPS;
            self.undo_stack.drain(0..overflow);
        }
        self.redo_stack.clear();
    }

    pub(super) fn undo(
        &mut self,
        current_anns: &mut Vec<Annotation>,
        current_step: u32,
    ) -> Option<u32> {
        let (prev_anns, prev_step) = self.undo_stack.pop()?;
        self.redo_stack
            .push((std::mem::take(current_anns), current_step));
        *current_anns = prev_anns;
        Some(prev_step)
    }

    pub(super) fn redo(
        &mut self,
        current_anns: &mut Vec<Annotation>,
        current_step: u32,
    ) -> Option<u32> {
        let (next_anns, next_step) = self.redo_stack.pop()?;
        self.undo_stack
            .push((std::mem::take(current_anns), current_step));
        *current_anns = next_anns;
        Some(next_step)
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.undo_stack.len()
    }

    #[cfg(test)]
    pub(super) fn is_redo_empty(&self) -> bool {
        self.redo_stack.is_empty()
    }

    pub(super) fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}

pub(super) struct BlurCache {
    pub(super) revision: u64,
    pub(super) baked_blurs: HashMap<usize, (u64, ColorImage)>,
    pub(super) defer_blur_idx: Option<usize>,
    pub(super) pending_blur_rebake: Option<usize>,
    pub(super) need_live_blur_capture: bool,
    pub(super) text_blur_img: Option<Arc<ColorImage>>,
}

impl BlurCache {
    pub(super) fn new() -> Self {
        Self {
            revision: 0,
            baked_blurs: HashMap::new(),
            defer_blur_idx: None,
            pending_blur_rebake: None,
            need_live_blur_capture: false,
            text_blur_img: None,
        }
    }

    pub(super) fn clear_deferred(&mut self) {
        self.defer_blur_idx = None;
        self.pending_blur_rebake = None;
    }
}

pub(super) struct InteractionState {
    pub(super) modifiers: ModifiersState,
    pub(super) mode: Mode,
    pub(super) hover_pos: Option<Pos2>,
    pub(super) drag_start: Option<Pos2>,
    pub(super) drag_curr: Option<Pos2>,
    pub(super) drag_action: Option<DragAction>,
    pub(super) selection: Option<Rect>,
    pub(super) toolbar_pos: Option<Pos2>,
    pub(super) in_progress: Option<Annotation>,
    pub(super) selected_annotation: Option<usize>,
    pub(super) last_edited_idx: Option<usize>,
    pub(super) pending_text: Option<PendingText>,
}

pub(super) struct OverlayShared {
    pub(super) virtual_screen: CapturedScreen,
    pub(super) live_draw: bool,
    pub(super) windows: Vec<WindowCandidate>,
    pub(super) hovered_window_label: Option<String>,
    pub(super) save_dir: PathBuf,
    pub(super) lang: Language,
    pub(super) theme: ThemeColors,
    pub(super) snap_angle_deg: u16,
    pub(super) text_font: TextFont,
    pub(super) toolbar_layout: ToolbarLayout,
    pub(super) annotations: Vec<Annotation>,
    pub(super) status_error: Option<(String, Instant)>,

    pub(super) tools: ToolState,
    pub(super) history: UndoHistory,
    pub(super) blur: BlurCache,
    pub(super) input: InteractionState,
}

struct OverlayPane {
    gl_win: GlEguiWindow,
    live_draw: bool,
    screen: CapturedScreen,
    use_shared_virtual_image: bool,
    pending_sharp: Option<RgbaImage>,
    mon_offset: Vec2,
    mon_scale: f32,
    sharp_tex: Option<egui::TextureHandle>,
    text_blur_tex: Option<egui::TextureHandle>,
    uploaded_blur_revs: HashMap<usize, u64>,
    blur_textures: HashMap<usize, egui::TextureHandle>,
    blur_tex_ids: HashMap<usize, egui::TextureId>,
}

pub struct OverlayWindow {
    panes: Vec<OverlayPane>,
    shared: OverlayShared,
}

impl OverlayWindow {
    pub fn open(event_loop: &ActiveEventLoop, config: OverlayConfig) -> Result<Self, String> {
        let single_screen = config.screens.len() == 1;
        let (virtual_screen, screens) = if config.live_draw {
            (composite_monitors_metadata(&config.screens), config.screens)
        } else {
            take_composite_virtual_screen(config.screens)
        };

        let mut panes = Vec::with_capacity(screens.len());
        for screen in screens {
            let pane = OverlayPane::open(
                event_loop,
                screen,
                &virtual_screen,
                config.text_font,
                config.live_draw,
                single_screen && !config.live_draw,
            )?;
            panes.push(pane);
        }

        if let Some(first) = panes.first() {
            first.gl_win.window.focus_window();
        }

        let mode = if config.live_draw {
            Mode::Annotating
        } else if config.window_select_mode {
            Mode::WindowSelect
        } else {
            Mode::Selecting
        };

        let selection = if config.live_draw {
            Some(Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(virtual_screen.logical_width, virtual_screen.logical_height),
            ))
        } else {
            None
        };

        let color_pos = config.initial_color_pos.clamp(0.0, 1.0);
        let thickness = config.initial_thickness.clamp(1.5, 20.0);

        let mut shared = OverlayShared {
            virtual_screen,
            live_draw: config.live_draw,
            windows: config.windows,
            hovered_window_label: None,
            save_dir: config.save_dir,
            lang: config.lang,
            theme: config.theme,
            snap_angle_deg: config.snap_angle_deg.clamp(1, 90),
            text_font: config.text_font,
            toolbar_layout: config.toolbar_layout,
            annotations: config.initial_annotations,
            status_error: None,
            tools: ToolState {
                active_tool: config.initial_tool,
                color_pos,
                active_color: color_from_slider(color_pos),
                thickness,
                step_counter: config.initial_step_counter.max(1),
            },
            history: UndoHistory::new(),
            blur: BlurCache::new(),
            input: InteractionState {
                modifiers: ModifiersState::empty(),
                mode,
                hover_pos: None,
                drag_start: None,
                drag_curr: None,
                drag_action: None,
                selection,
                toolbar_pos: None,
                in_progress: None,
                selected_annotation: None,
                last_edited_idx: None,
                pending_text: None,
            },
        };
        if !config.live_draw && !shared.annotations.is_empty() {
            shared.rebake_blurs_from(0);
        }

        Ok(Self { panes, shared })
    }

    pub fn has_window_id(&self, id: WindowId) -> bool {
        self.panes.iter().any(|p| p.gl_win.window_id() == id)
    }

    pub fn color_pos(&self) -> f32 {
        self.shared.tools.color_pos
    }

    pub fn thickness(&self) -> f32 {
        self.shared.tools.thickness
    }

    pub fn active_tool(&self) -> Tool {
        self.shared.tools.active_tool
    }

    pub fn is_live_draw(&self) -> bool {
        self.shared.live_draw
    }

    pub fn hide_all(&self) {
        for p in &self.panes {
            p.gl_win.window.set_visible(false);
            #[cfg(target_os = "macos")]
            {
                use raw_window_handle::HasWindowHandle;
                if let Ok(handle) = p.gl_win.window.window_handle()
                    && let raw_window_handle::RawWindowHandle::AppKit(appkit) = handle.as_raw()
                {
                    // SAFETY: `appkit.ns_view` is a live AppKit NSView pointer obtained from the active winit window handle on the main thread; sending `window` and `orderOut: nil` is valid in AppKit.
                    unsafe {
                        let ns_view = appkit.ns_view.as_ptr() as *mut objc2::runtime::AnyObject;
                        let ns_win: *mut objc2::runtime::AnyObject =
                            objc2::msg_send![ns_view, window];
                        if !ns_win.is_null() {
                            let nil: *mut objc2::runtime::AnyObject = std::ptr::null_mut();
                            let _: () = objc2::msg_send![ns_win, orderOut: nil];
                        }
                    }
                }
            }
        }
    }

    pub fn show_all(&self) {
        for p in &self.panes {
            p.gl_win.window.set_visible(true);
            p.gl_win.window.request_redraw();
        }
        if let Some(first) = self.panes.first() {
            first.gl_win.window.focus_window();
        }
    }

    pub fn set_live_background(&mut self, screens: Vec<CapturedScreen>) {
        let (virt, _) = take_composite_virtual_screen(screens);
        self.shared.virtual_screen = virt;
        self.shared.blur.text_blur_img = None;
        for p in &mut self.panes {
            p.text_blur_tex = None;
        }
        self.shared.rebake_blurs_from(0);
        self.request_all_redraws();
    }

    pub fn take_live_draw_state(&mut self) -> (Vec<Annotation>, u32, CapturedScreen) {
        if self.shared.input.pending_text.is_some() {
            self.shared.commit_pending_text();
        }
        let virt_meta = CapturedScreen {
            image: RgbaImage::new(1, 1),
            x: self.shared.virtual_screen.x,
            y: self.shared.virtual_screen.y,
            logical_width: self.shared.virtual_screen.logical_width,
            logical_height: self.shared.virtual_screen.logical_height,
            scale_factor: self.shared.virtual_screen.scale_factor,
            physical_width: self.shared.virtual_screen.physical_width,
            physical_height: self.shared.virtual_screen.physical_height,
            physical_coords: self.shared.virtual_screen.physical_coords,
        };
        (
            std::mem::take(&mut self.shared.annotations),
            self.shared.tools.step_counter,
            virt_meta,
        )
    }

    pub fn next_wakeup_at(&self) -> Option<Instant> {
        self.shared
            .status_error
            .as_ref()
            .map(|(_, ts)| *ts + Duration::from_secs(5))
    }

    pub fn request_all_redraws(&self) {
        for p in &self.panes {
            p.gl_win.window.request_redraw();
        }
    }

    pub fn handle_window_event(&mut self, window_id: WindowId, event: &WindowEvent) -> OverlayOutcome {
        let Some(pane_idx) = self.panes.iter().position(|p| p.gl_win.window_id() == window_id) else {
            return OverlayOutcome::KeepOpen;
        };

        if let WindowEvent::CloseRequested = event {
            return OverlayOutcome::Close;
        }

        if let WindowEvent::ModifiersChanged(mods) = event {
            self.shared.input.modifiers = mods.state();
            self.request_all_redraws();
        }

        if let WindowEvent::CursorMoved { position, .. } = event {
            let pane = &self.panes[pane_idx];
            let sf = pane.gl_win.window.scale_factor() as f32;
            let local = Pos2::new(position.x as f32 / sf, position.y as f32 / sf);
            self.shared.input.hover_pos = Some(pane.to_global(local));
            if self.shared.input.drag_action.is_some()
                || self.shared.input.mode != Mode::Annotating
            {
                self.request_all_redraws();
            }
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

            let editing_text = self.shared.input.pending_text.is_some();
            let primary_mod = primary_mod_held(self.shared.input.modifiers, None);
            let shift_held = self.shared.input.modifiers.shift_key();

            if key_ev.state == ElementState::Pressed && primary_mod && !editing_text {
                match key_ev.physical_key {
                    PhysicalKey::Code(KeyCode::KeyC) => {
                        if self.shared.live_draw {
                            return OverlayOutcome::LiveCopy;
                        }
                        if self.shared.export_and_copy() {
                            return OverlayOutcome::Close;
                        }
                        self.request_all_redraws();
                        return OverlayOutcome::KeepOpen;
                    }
                    PhysicalKey::Code(KeyCode::KeyS) => {
                        if self.shared.live_draw {
                            return OverlayOutcome::LiveSave;
                        }
                        if self.shared.export_and_save() {
                            return OverlayOutcome::Close;
                        }
                        self.request_all_redraws();
                        return OverlayOutcome::KeepOpen;
                    }
                    PhysicalKey::Code(KeyCode::KeyZ) => {
                        if shift_held {
                            self.shared.redo();
                        } else {
                            self.shared.undo();
                        }
                        self.request_all_redraws();
                        return OverlayOutcome::KeepOpen;
                    }
                    #[cfg(not(target_os = "macos"))]
                    PhysicalKey::Code(KeyCode::KeyY) => {
                        self.shared.redo();
                        self.request_all_redraws();
                        return OverlayOutcome::KeepOpen;
                    }
                    _ => {}
                }
            } else if key_ev.state == ElementState::Pressed
                && self.shared.input.mode == Mode::Annotating
                && !editing_text
                && !primary_mod
                && !self.shared.input.modifiers.alt_key()
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
                    self.shared.tools.active_tool = tool;
                    self.request_all_redraws();
                }
            }
        }

        let repaint = self.panes[pane_idx].gl_win.on_window_event(event);
        if repaint {
            if self.shared.input.drag_action.is_some()
                || self.shared.input.mode != Mode::Annotating
                || self.shared.input.pending_text.is_some()
            {
                self.request_all_redraws();
            } else {
                self.panes[pane_idx].gl_win.window.request_redraw();
            }
        }

        if matches!(event, WindowEvent::RedrawRequested) {
            let toolbar_pane = self.toolbar_pane_index();
            let outcome = self.panes[pane_idx].draw_frame(&mut self.shared, pane_idx == toolbar_pane);
            return outcome;
        }

        OverlayOutcome::KeepOpen
    }

    fn toolbar_pane_index(&self) -> usize {
        if let Some(sel) = self.shared.input.selection {
            let c = sel.center();
            if let Some(idx) = self.panes.iter().position(|p| p.virtual_bounds().contains(c)) {
                return idx;
            }
        }
        0
    }
}

impl OverlayShared {
    pub(super) fn text_font_size(&self) -> f32 {
        (13.0 + self.tools.thickness * 2.2).clamp(16.0, 57.0)
    }

    pub(super) fn step_radius(&self) -> f32 {
        (9.5 + self.tools.thickness * 0.95).clamp(11.0, 28.5)
    }

    pub(super) fn blur_radius(&self) -> f32 {
        (2.5 + self.tools.thickness * 0.55).clamp(3.0, 14.0)
    }

    pub(super) fn set_error(&mut self, msg: String) {
        diagnostics::log_error("overlay", &msg);
        self.status_error = Some((msg, Instant::now()));
    }

    pub(super) fn push_undo_snapshot(&mut self) {
        self.history
            .push(self.annotations.clone(), self.tools.step_counter);
    }

    pub(super) fn rebake_blurs_from(&mut self, from_idx: usize) {
        self.blur
            .baked_blurs
            .retain(|&idx, _| idx < from_idx && idx < self.annotations.len());
        if self.live_draw && self.virtual_screen.image.width() <= 2 {
            self.blur.revision = self.blur.revision.wrapping_add(1);
            return;
        }

        let mut any_rebuilt = false;
        let next_rev = self.blur.revision.wrapping_add(1);
        for (idx, ann) in self.annotations.iter().enumerate().skip(from_idx) {
            match ann {
                Annotation::Blur {
                    rect,
                    radius,
                    pixelate,
                } => {
                    let img = bake_blur_patch(
                        &self.virtual_screen.image,
                        *rect,
                        self.virtual_screen.scale_factor,
                        *radius,
                        *pixelate,
                        &self.annotations[..idx],
                    );
                    self.blur.baked_blurs.insert(idx, (next_rev, img));
                    any_rebuilt = true;
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
                        false,
                        &self.annotations[..idx],
                    );
                    self.blur.baked_blurs.insert(idx, (next_rev, img));
                    any_rebuilt = true;
                }
                _ => {}
            }
        }
        if any_rebuilt || from_idx <= self.annotations.len() {
            self.blur.revision = next_rev;
        }
    }

    pub(super) fn flush_pending_blur_rebake(&mut self) {
        if let Some(from_idx) = self.blur.pending_blur_rebake.take() {
            self.blur.defer_blur_idx = None;
            self.rebake_blurs_from(from_idx);
        }
    }

    pub(super) fn undo(&mut self) {
        if self.input.pending_text.is_some() {
            self.cancel_pending_text();
            return;
        }
        let Some(prev_step) = self
            .history
            .undo(&mut self.annotations, self.tools.step_counter)
        else {
            return;
        };
        self.tools.step_counter = prev_step;
        self.input.selected_annotation = None;
        self.input.last_edited_idx = None;
        self.blur.clear_deferred();
        self.rebake_blurs_from(0);
    }

    pub(super) fn redo(&mut self) {
        if !self.history.can_redo() {
            return;
        }
        if self.input.pending_text.is_some() {
            self.cancel_pending_text();
        }
        let Some(next_step) = self
            .history
            .redo(&mut self.annotations, self.tools.step_counter)
        else {
            return;
        };
        self.tools.step_counter = next_step;
        self.input.selected_annotation = None;
        self.input.last_edited_idx = None;
        self.blur.clear_deferred();
        self.rebake_blurs_from(0);
    }

    pub(super) fn commit_pending_text(&mut self) {
        if let Some(pending) = self.input.pending_text.take() {
            let trimmed = pending.buf.trim().to_string();
            match pending.original {
                Some((orig_idx, orig_ann)) => {
                    let insert_at = orig_idx.min(self.annotations.len());
                    if trimmed.is_empty() {
                        let mut prev_state = self.annotations.clone();
                        prev_state.insert(insert_at, orig_ann);
                        self.history.push(prev_state, self.tools.step_counter);
                        self.rebake_blurs_from(insert_at);
                        return;
                    }
                    let changed = match &orig_ann {
                        Annotation::Text { text, .. } => text != &trimmed,
                        _ => true,
                    };
                    if changed {
                        let mut prev_state = self.annotations.clone();
                        prev_state.insert(insert_at, orig_ann.clone());
                        self.history.push(prev_state, self.tools.step_counter);
                    }
                    if let Annotation::Text {
                        color,
                        size,
                        thickness,
                        font,
                        box_size,
                        ..
                    } = orig_ann
                    {
                        self.annotations.insert(
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
                        );
                        self.input.last_edited_idx = Some(insert_at);
                        self.input.selected_annotation = None;
                        self.rebake_blurs_from(insert_at);
                    }
                }
                None if !trimmed.is_empty() => {
                    self.push_undo_snapshot();
                    let insert_at = self.annotations.len();
                    self.annotations.push(Annotation::Text {
                        pos: pending.pos,
                        text: trimmed,
                        color: self.tools.active_color,
                        size: self.text_font_size(),
                        arrow_to: pending.arrow_to,
                        thickness: self.tools.thickness,
                        placement: pending.placement,
                        font: self.text_font,
                        box_size: None,
                    });
                    self.input.last_edited_idx = Some(insert_at);
                    self.input.selected_annotation = None;
                    self.rebake_blurs_from(insert_at);
                }
                None => {}
            }
        }
    }

    pub(super) fn cancel_pending_text(&mut self) {
        if let Some(pending) = self.input.pending_text.take()
            && let Some((orig_idx, orig_ann)) = pending.original
        {
            let insert_at = orig_idx.min(self.annotations.len());
            self.annotations.insert(insert_at, orig_ann);
            self.input.last_edited_idx = Some(insert_at);
            self.rebake_blurs_from(insert_at);
        }
    }

    pub(super) fn open_text_for_edit(&mut self, idx: usize) {
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
            self.input.selected_annotation = None;
            self.input.pending_text = Some(PendingText {
                pos,
                arrow_to,
                placement,
                buf,
                just_opened: true,
                original: Some((idx, orig)),
            });
            self.rebake_blurs_from(idx);
        }
    }

    pub(super) fn export_and_copy(&mut self) -> bool {
        self.commit_pending_text();
        let Some(sel) = self.input.selection else {
            return false;
        };
        let img = render_export(
            &self.virtual_screen.image,
            sel,
            self.virtual_screen.scale_factor,
            &self.annotations,
        );
        match copy_to_clipboard(&img) {
            Ok(()) => true,
            Err(e) => {
                let prefix = Strings::for_lang(self.lang).err_copy_failed;
                self.set_error(format!("{prefix}: {e}"));
                false
            }
        }
    }

    pub(super) fn export_and_save(&mut self) -> bool {
        self.commit_pending_text();
        let Some(sel) = self.input.selection else {
            return false;
        };
        let img = render_export(
            &self.virtual_screen.image,
            sel,
            self.virtual_screen.scale_factor,
            &self.annotations,
        );
        match save_to_dir_async(img, &self.save_dir) {
            Ok(path) => {
                println!("Saved screenshot: {}", path.display());
                true
            }
            Err(e) => {
                let prefix = Strings::for_lang(self.lang).err_save_failed;
                self.set_error(format!("{prefix}: {e}"));
                false
            }
        }
    }
}

impl OverlayPane {
    fn open(
        event_loop: &ActiveEventLoop,
        mut screen: CapturedScreen,
        virtual_screen: &CapturedScreen,
        text_font: TextFont,
        live_draw: bool,
        use_shared_virtual_image: bool,
    ) -> Result<Self, String> {
        #[allow(unused_mut)]
        let mut win_attrs = WindowAttributes::default()
            .with_title("ShotCraft Capture")
            .with_decorations(false)
            .with_transparent(live_draw)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop);

        if screen.physical_coords {
            win_attrs = win_attrs
                .with_position(PhysicalPosition::new(screen.x, screen.y))
                .with_inner_size(PhysicalSize::new(
                    screen.physical_width.max(1),
                    screen.physical_height.max(1),
                ));
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

        let gl_win = GlEguiWindow::open(
            event_loop,
            win_attrs,
            WindowChrome::Overlay {
                transparent: live_draw,
            },
            true,
            move |ctx| configure_m3_style(ctx, text_font),
        )?;

        let mon_offset = pane_virtual_offset(&screen, virtual_screen);
        let mon_scale = pane_virtual_scale(&screen, virtual_screen);
        let pending_sharp = if live_draw || use_shared_virtual_image {
            None
        } else {
            Some(std::mem::replace(&mut screen.image, RgbaImage::new(1, 1)))
        };

        gl_win.window.request_redraw();

        Ok(Self {
            gl_win,
            live_draw,
            screen,
            use_shared_virtual_image,
            pending_sharp,
            mon_offset,
            mon_scale,
            sharp_tex: None,
            text_blur_tex: None,
            uploaded_blur_revs: HashMap::new(),
            blur_textures: HashMap::new(),
            blur_tex_ids: HashMap::new(),
        })
    }

    fn view_scale(&self) -> f32 {
        1.0 / self.mon_scale.max(0.1)
    }

    fn to_global(&self, local: Pos2) -> Pos2 {
        (self.mon_offset + local.to_vec2() * self.mon_scale).to_pos2()
    }

    fn global_to_local(&self, global: Pos2) -> Pos2 {
        let shifted = global - self.mon_offset;
        let vs = self.view_scale();
        Pos2::new(shifted.x * vs, shifted.y * vs)
    }

    fn virtual_bounds(&self) -> Rect {
        Rect::from_min_size(
            self.mon_offset.to_pos2(),
            Vec2::new(
                self.screen.logical_width * self.mon_scale,
                self.screen.logical_height * self.mon_scale,
            ),
        )
    }

    fn ensure_sharp_texture(&mut self, shared: &OverlayShared) -> egui::TextureId {
        if let Some(sharp) = &self.sharp_tex {
            return sharp.id();
        }
        let sharp_img = if self.use_shared_virtual_image {
            let img = &shared.virtual_screen.image;
            ColorImage::from_rgba_unmultiplied(
                [img.width() as usize, img.height() as usize],
                img.as_raw(),
            )
        } else if let Some(img) = self.pending_sharp.take() {
            ColorImage::from_rgba_unmultiplied(
                [img.width() as usize, img.height() as usize],
                img.as_raw(),
            )
        } else {
            ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 255])
        };
        let sharp = self
            .gl_win
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
            .blur
            .text_blur_img
            .get_or_insert_with(|| Arc::new(make_strong_text_blur_image(&shared.virtual_screen.image)))
            .as_ref()
            .clone();
        let tex = self.gl_win.egui_ctx.load_texture(
            "virtual_text_blur",
            img,
            egui::TextureOptions::LINEAR,
        );
        let id = tex.id();
        self.text_blur_tex = Some(tex);
        id
    }

    fn sync_blur_textures(&mut self, shared: &OverlayShared) {
        self.blur_textures
            .retain(|k, _| shared.blur.baked_blurs.contains_key(k));
        self.blur_tex_ids
            .retain(|k, _| shared.blur.baked_blurs.contains_key(k));
        self.uploaded_blur_revs
            .retain(|k, _| shared.blur.baked_blurs.contains_key(k));

        for (&idx, &(rev, ref img)) in &shared.blur.baked_blurs {
            if self.uploaded_blur_revs.get(&idx) != Some(&rev) {
                let tex = self.gl_win.egui_ctx.load_texture(
                    format!("blur_patch_{idx}"),
                    img.clone(),
                    egui::TextureOptions::LINEAR,
                );
                self.blur_tex_ids.insert(idx, tex.id());
                self.blur_textures.insert(idx, tex);
                self.uploaded_blur_revs.insert(idx, rev);
            }
        }
    }

    fn draw_frame(&mut self, shared: &mut OverlayShared, draw_toolbar_here: bool) -> OverlayOutcome {
        self.gl_win.make_current();

        let sharp_tex = if shared.live_draw {
            None
        } else {
            Some(self.ensure_sharp_texture(shared))
        };
        let has_live_bg = !shared.live_draw || shared.virtual_screen.image.width() > 2;
        let needs_text_blur = has_live_bg
            && (shared.input.pending_text.is_some()
                || shared
                    .annotations
                    .iter()
                    .any(|a| matches!(a, Annotation::Text { .. } | Annotation::Blur { .. }))
                || matches!(shared.input.in_progress, Some(Annotation::Blur { .. })));
        let text_blur_tex = if needs_text_blur {
            Some(self.ensure_text_blur_texture(shared))
        } else {
            None
        };
        self.sync_blur_textures(shared);

        let raw_input = self.gl_win.take_egui_input();
        let mut outcome = OverlayOutcome::KeepOpen;

        let local_size = Vec2::new(self.screen.logical_width, self.screen.logical_height);
        let local_rect = Rect::from_min_size(Pos2::ZERO, local_size);
        let virtual_size = Vec2::new(
            shared.virtual_screen.logical_width,
            shared.virtual_screen.logical_height,
        );
        let virtual_rect = Rect::from_min_size(Pos2::ZERO, virtual_size);
        let offset = -self.mon_offset;
        let view_scale = self.view_scale();
        let mon_offset = self.mon_offset;
        let mon_scale = self.mon_scale;

        if let Some((_, ts)) = &shared.status_error
            && ts.elapsed() > Duration::from_secs(5)
        {
            shared.status_error = None;
        }

        let blur_patch_ids = &self.blur_tex_ids;
        let egui_ctx = self.gl_win.egui_ctx.clone();
        let window_ref = &self.gl_win.window;

        let full_output = egui_ctx.run(raw_input, |ctx| {
            let editing_text = shared.input.pending_text.is_some();
            let (esc, del, enter_commit, shift_held, cmd_held) = ctx.input(|i| {
                let cmd = primary_mod_held(shared.input.modifiers, Some(i.modifiers));
                (
                    i.key_pressed(Key::Escape),
                    !editing_text && (i.key_pressed(Key::Backspace) || i.key_pressed(Key::Delete)),
                    i.key_pressed(Key::Enter) && !i.modifiers.shift,
                    i.modifiers.shift,
                    cmd,
                )
            });

            if esc {
                if shared.input.pending_text.is_some() {
                    shared.cancel_pending_text();
                } else {
                    outcome = OverlayOutcome::Close;
                    return;
                }
            }
            if enter_commit {
                if shared.input.pending_text.is_some() {
                    shared.commit_pending_text();
                } else if !shared.live_draw
                    && shared.input.selection.is_some()
                    && shared.input.mode == Mode::Annotating
                    && shared.export_and_copy()
                {
                    outcome = OverlayOutcome::Close;
                    return;
                }
            }
            if del
                && let Some(idx) = shared.input.selected_annotation.take()
                && idx < shared.annotations.len()
            {
                shared.push_undo_snapshot();
                shared.annotations.remove(idx);
                shared.input.last_edited_idx = None;
                shared.rebake_blurs_from(idx);
            }

            // 1. Canvas + selection + annotations + pointer handling
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

                    if !shared.live_draw {
                        if let Some(sel_global) = shared.input.selection {
                            let sel_local = Rect::from_min_max(
                                self.global_to_local(sel_global.min),
                                self.global_to_local(sel_global.max),
                            );
                            if let Some(tex) = sharp_tex {
                                draw_m3_selection_frame(
                                    painter,
                                    tex,
                                    local_size,
                                    sel_local,
                                    shared.input.mode == Mode::Annotating
                                        && shared.tools.active_tool == Tool::Move,
                                    &shared.theme,
                                );
                            }

                            if draw_toolbar_here {
                                let px_w = (sel_global.width() * shared.virtual_screen.scale_factor)
                                    .round() as i32;
                                let px_h = (sel_global.height() * shared.virtual_screen.scale_factor)
                                    .round() as i32;
                                let badge_text = if shared.input.mode == Mode::WindowSelect {
                                    if let Some(lbl) = &shared.hovered_window_label {
                                        format!("{lbl}  •  {px_w} × {px_h}")
                                    } else {
                                        format!("{px_w} × {px_h}")
                                    }
                                } else {
                                    format!("{px_w} × {px_h}")
                                };
                                let badge_w = (badge_text.chars().count() as f32 * 7.2 + 24.0)
                                    .clamp(100.0, 380.0);
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
                                let hint_rect = Rect::from_center_size(
                                    local_rect.center(),
                                    Vec2::new(390.0, 44.0),
                                );
                                draw_m3_panel(painter, hint_rect, 10.0, &shared.theme);
                                let t = Strings::for_lang(shared.lang);
                                let hint_text = if shared.input.mode == Mode::WindowSelect {
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

                            if shared.input.mode == Mode::Selecting
                                && let Some(hover_global) = shared.input.hover_pos
                            {
                                let hover = self.global_to_local(hover_global);
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
                                    [
                                        Pos2::new(0.0, hover.y),
                                        Pos2::new(local_rect.max.x, hover.y),
                                    ],
                                    Stroke::new(1.0_f32, light),
                                );
                                painter.line_segment(
                                    [
                                        Pos2::new(hover.x, 0.0),
                                        Pos2::new(hover.x, local_rect.max.y),
                                    ],
                                    Stroke::new(1.0_f32, light),
                                );
                            }
                        }
                    }

                    paint_annotations(
                        painter,
                        &shared.annotations,
                        if shared.tools.active_tool == Tool::Move {
                            shared.input.selected_annotation
                        } else {
                            None
                        },
                        shared.blur.defer_blur_idx,
                        offset,
                        view_scale,
                        blur_patch_ids,
                        text_blur_tex,
                        virtual_size,
                        shared.theme.accent,
                    );
                    if let Some(ann) = &shared.input.in_progress {
                        let empty_blurs = HashMap::new();
                        paint_annotations(
                            painter,
                            std::slice::from_ref(ann),
                            None,
                            None,
                            offset,
                            view_scale,
                            &empty_blurs,
                            text_blur_tex,
                            virtual_size,
                            shared.theme.accent,
                        );
                    }

                    toolbar::draw_text_callout_preview(
                        ctx,
                        painter,
                        shared,
                        offset,
                        view_scale,
                        text_blur_tex,
                        virtual_size,
                    );

                    input::handle_canvas_pointer(
                        ctx,
                        ui,
                        window_ref,
                        shared,
                        local_rect,
                        virtual_rect,
                        mon_offset,
                        mon_scale,
                        shift_held,
                        cmd_held,
                    );
                });

            // 2. Inline Text Editor
            toolbar::draw_inline_text_editor(ctx, shared, local_rect, offset, view_scale);

            // 3. Floating Toolbar
            if draw_toolbar_here
                && shared.input.mode == Mode::Annotating
                && let Some(sel_global) = shared.input.selection
            {
                let sel_local = Rect::from_min_max(
                    self.global_to_local(sel_global.min),
                    self.global_to_local(sel_global.max),
                );
                if let Some(action) =
                    toolbar::draw_floating_toolbar(ctx, shared, sel_local, local_rect, cmd_held)
                {
                    outcome = action;
                }
            }

            // 4. Visible Error Banner (when save/copy fails)
            if draw_toolbar_here {
                toolbar::draw_error_banner(ctx, shared, local_rect);
            }
        });

        let needs_repaint = self
            .gl_win
            .paint_frame([0.0, 0.0, 0.0, if self.live_draw { 0.0 } else { 1.0 }], full_output);

        if shared.blur.need_live_blur_capture {
            shared.blur.need_live_blur_capture = false;
            return OverlayOutcome::CaptureLiveBlurBackground;
        }

        if needs_repaint {
            self.gl_win.window.request_redraw();
        }

        outcome
    }
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

    let fam = egui::FontFamily::Name(active_text_font.egui_family().into());
    match active_text_font {
        TextFont::Roboto => {
            fonts
                .families
                .entry(fam)
                .or_default()
                .push("roboto_m3".to_owned());
        }
        tf => {
            if let Some(bytes) = tf.resolve_font_bytes() {
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

#[cfg(target_os = "windows")]
fn win_ctrl_down() -> bool {
    // SAFETY: `GetAsyncKeyState` is a stateless Win32 user32 import taking a virtual-key code (`VK_CONTROL`) and returning a bitmask with no pointer arguments.
    unsafe extern "system" {
        fn GetAsyncKeyState(vkey: i32) -> i16;
    }
    const VK_CONTROL: i32 = 0x11;
    // SAFETY: Calling `GetAsyncKeyState` with `VK_CONTROL` has no preconditions or aliasing constraints.
    (unsafe { GetAsyncKeyState(VK_CONTROL) } as u16 & 0x8000) != 0
}

pub(super) fn primary_mod_held(
    winit_mods: ModifiersState,
    egui_mods: Option<egui::Modifiers>,
) -> bool {
    #[cfg(target_os = "macos")]
    {
        winit_mods.super_key() || egui_mods.is_some_and(|m| m.mac_cmd || m.command)
    }
    #[cfg(target_os = "windows")]
    {
        winit_mods.control_key()
            || egui_mods.is_some_and(|m| m.ctrl)
            || win_ctrl_down()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        winit_mods.control_key() || egui_mods.is_some_and(|m| m.ctrl)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_shared() -> OverlayShared {
        OverlayShared {
            virtual_screen: CapturedScreen {
                image: RgbaImage::from_pixel(200, 200, image::Rgba([0, 0, 0, 255])),
                x: 0,
                y: 0,
                logical_width: 200.0,
                logical_height: 200.0,
                scale_factor: 1.0,
                physical_width: 200,
                physical_height: 200,
                physical_coords: false,
            },
            live_draw: false,
            windows: Vec::new(),
            hovered_window_label: None,
            save_dir: PathBuf::from("."),
            lang: Language::En,
            theme: ThemeColors::from_positions(0.28, 0.56),
            snap_angle_deg: 90,
            text_font: TextFont::Roboto,
            toolbar_layout: ToolbarLayout::Horizontal,
            annotations: Vec::new(),
            status_error: None,
            tools: ToolState {
                active_tool: Tool::Arrow,
                color_pos: 0.56,
                active_color: Color32::WHITE,
                thickness: 5.0,
                step_counter: 1,
            },
            history: UndoHistory::new(),
            blur: BlurCache::new(),
            input: InteractionState {
                modifiers: ModifiersState::empty(),
                mode: Mode::Annotating,
                hover_pos: None,
                drag_start: None,
                drag_curr: None,
                drag_action: None,
                selection: None,
                toolbar_pos: None,
                in_progress: None,
                selected_annotation: None,
                last_edited_idx: None,
                pending_text: None,
            },
        }
    }

    #[test]
    fn undo_redo_restores_annotations_and_step_counter() {
        let mut s = test_shared();

        s.push_undo_snapshot();
        s.annotations.push(Annotation::Step {
            pos: Pos2::new(10.0, 10.0),
            number: 1,
            color: Color32::WHITE,
            radius: 14.0,
        });
        s.tools.step_counter = 2;

        s.push_undo_snapshot();
        s.annotations.push(Annotation::Line {
            start: Pos2::ZERO,
            end: Pos2::new(50.0, 50.0),
            color: Color32::RED,
            thickness: 3.0,
            dashed: false,
        });

        assert_eq!(s.annotations.len(), 2);
        s.undo();
        assert_eq!(s.annotations.len(), 1);
        assert_eq!(s.tools.step_counter, 2);

        s.undo();
        assert_eq!(s.annotations.len(), 0);
        assert_eq!(s.tools.step_counter, 1);

        s.redo();
        assert_eq!(s.annotations.len(), 1);
        assert_eq!(s.tools.step_counter, 2);

        // New mutation clears redo stack
        s.push_undo_snapshot();
        assert!(s.history.is_redo_empty());
    }

    #[test]
    fn undo_history_enforces_max_capacity_and_empty_noop() {
        let mut s = test_shared();
        // Empty undo / redo are no-ops
        s.undo();
        s.redo();
        assert!(s.annotations.is_empty());

        for i in 1..=(MAX_UNDO_STEPS + 10) {
            s.push_undo_snapshot();
            s.tools.step_counter = i as u32 + 1;
        }
        assert_eq!(s.history.len(), MAX_UNDO_STEPS);
    }

    #[test]
    fn commit_pending_text_creates_edits_and_deletes_on_empty() {
        let mut s = test_shared();

        // Create new text annotation
        s.input.pending_text = Some(PendingText {
            pos: Pos2::new(25.0, 25.0),
            arrow_to: None,
            placement: TextPlacement::LeftTop,
            buf: "  Hello  ".into(),
            just_opened: false,
            original: None,
        });
        s.commit_pending_text();
        assert_eq!(s.annotations.len(), 1);
        assert_eq!(s.history.len(), 1);
        match &s.annotations[0] {
            Annotation::Text { text, .. } => assert_eq!(text, "Hello"),
            _ => panic!("expected Text"),
        }

        // Edit and clear -> deletes annotation and records undo snapshot
        s.open_text_for_edit(0);
        if let Some(p) = &mut s.input.pending_text {
            p.buf.clear();
        }
        s.commit_pending_text();
        assert!(s.annotations.is_empty());
        assert_eq!(s.history.len(), 2);

        // Undo restores "Hello"
        s.undo();
        assert_eq!(s.annotations.len(), 1);
    }

    #[test]
    fn undo_and_redo_preserve_text_and_cancel_open_editor() {
        let mut s = test_shared();
        s.push_undo_snapshot();
        s.annotations.push(Annotation::Step {
            pos: Pos2::new(20.0, 20.0),
            number: 1,
            color: Color32::WHITE,
            radius: 14.0,
        });
        s.push_undo_snapshot();
        s.annotations.push(Annotation::Text {
            pos: Pos2::new(40.0, 40.0),
            text: "Orig".into(),
            color: Color32::WHITE,
            size: 20.0,
            arrow_to: None,
            thickness: 3.0,
            placement: TextPlacement::LeftTop,
            font: TextFont::Roboto,
            box_size: None,
        });

        // 1) Re-editing an existing text annotation and hitting toolbar Undo restores Orig safely
        s.open_text_for_edit(1);
        assert_eq!(s.annotations.len(), 1);
        assert!(s.input.pending_text.is_some());
        s.undo();
        assert!(s.input.pending_text.is_none());
        assert_eq!(s.annotations.len(), 2);

        // 2) Next Undo pops the annotation creation snapshot cleanly
        s.undo();
        assert_eq!(s.annotations.len(), 1);

        // 3) Redo while a fresh draft is open cancels the draft cleanly and restores redo snapshot
        s.input.pending_text = Some(PendingText {
            pos: Pos2::new(30.0, 30.0),
            arrow_to: None,
            placement: TextPlacement::LeftTop,
            buf: "draft".into(),
            just_opened: false,
            original: None,
        });
        s.redo();
        assert!(s.input.pending_text.is_none());
        assert_eq!(s.annotations.len(), 2);
    }

    #[test]
    fn status_error_schedules_five_second_expiry() {
        let mut s = test_shared();
        assert!(s.status_error.is_none());
        let ts = Instant::now();
        s.status_error = Some(("err".into(), ts));
        let expiry = s.status_error.as_ref().map(|(_, t)| *t + Duration::from_secs(5));
        assert_eq!(expiry, Some(ts + Duration::from_secs(5)));
    }
}
