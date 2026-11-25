use std::time::{Duration, Instant};

use egui::{Color32, ColorImage, Pos2, Rect, Stroke, Vec2};
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{WindowAttributes, WindowId, WindowLevel};

use crate::capture::CapturedScreen;
use crate::gl_window::{GlEguiWindow, WindowChrome};
use crate::material::draw_rounded_texture_uv;

const FLASH_DURATION_SECS: f32 = 0.48;
const FLASH_FRAME_INTERVAL: Duration = Duration::from_millis(16);

#[derive(Debug, Clone, Copy)]
pub struct FlashFrame {
    pub scale: f32,
    pub center: Pos2,
    pub alpha: u8,
    pub dim_alpha: u8,
    pub radius: f32,
}

/// Compute one animation frame for `elapsed_secs` inside `logical_size`. Returns `None` once finished.
pub fn compute_flash_frame(elapsed_secs: f32, logical_size: Vec2) -> Option<FlashFrame> {
    if !elapsed_secs.is_finite() || elapsed_secs >= FLASH_DURATION_SECS {
        return None;
    }
    let t = (elapsed_secs.max(0.0) / FLASH_DURATION_SECS).clamp(0.0, 1.0);
    let local_rect = Rect::from_min_size(Pos2::ZERO, logical_size);
    let center_mid = local_rect.center();

    if t < 0.35 {
        let p1 = (t / 0.35).clamp(0.0, 1.0);
        let e1 = 1.0 - (1.0 - p1).powi(3);
        let scale = 1.0 - 0.30 * e1;
        let dim_alpha = (96.0 - 36.0 * e1).round().clamp(0.0, 255.0) as u8;
        Some(FlashFrame {
            scale,
            center: center_mid,
            alpha: 255,
            dim_alpha,
            radius: 14.0 * e1,
        })
    } else {
        let p2 = ((t - 0.35) / 0.65).clamp(0.0, 1.0);
        let e2 = p2 * p2 * (3.0 - 2.0 * p2);
        let scale = (0.70 * (1.0 - e2)).max(0.002);
        let cy = center_mid.y + (local_rect.bottom() - center_mid.y) * e2;
        let alpha = ((1.0 - p2.powi(2)) * 255.0).round().clamp(0.0, 255.0) as u8;
        let dim_alpha = ((1.0 - e2) * 60.0).round().clamp(0.0, 60.0) as u8;
        Some(FlashFrame {
            scale,
            center: Pos2::new(center_mid.x, cy),
            alpha,
            dim_alpha,
            radius: 14.0 + 10.0 * e2,
        })
    }
}

pub struct CaptureFlashWindow {
    gl_win: GlEguiWindow,
    tex: egui::TextureHandle,
    logical_size: Vec2,
    accent: Color32,
    started_at: Option<Instant>,
    next_frame_at: Instant,
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
            WindowChrome::Overlay { transparent: true },
            false,
            |_| {},
        )?;
        let _ = gl_win.window.set_cursor_hittest(false);

        let color_img = if screen.image.width() == 0 || screen.image.height() == 0 {
            ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 0])
        } else {
            let img_size = [screen.image.width() as usize, screen.image.height() as usize];
            ColorImage::from_rgba_unmultiplied(img_size, screen.image.as_raw())
        };
        let tex = gl_win
            .egui_ctx
            .load_texture("flash_shot", color_img, egui::TextureOptions::LINEAR);

        gl_win.window.request_redraw();

        Ok(Self {
            gl_win,
            tex,
            logical_size: Vec2::new(screen.logical_width.max(1.0), screen.logical_height.max(1.0)),
            accent,
            started_at: None,
            next_frame_at: Instant::now(),
        })
    }

    pub fn window_id(&self) -> WindowId {
        self.gl_win.window_id()
    }

    pub fn next_frame_at(&self) -> Instant {
        self.next_frame_at
    }

    pub fn request_redraw(&self) {
        self.gl_win.window.request_redraw();
    }

    /// Returns `true` when the shrink & fade-out animation is finished or closed.
    pub fn handle_window_event(&mut self, event: &WindowEvent) -> bool {
        if matches!(event, WindowEvent::CloseRequested) {
            return true;
        }
        if let WindowEvent::Resized(PhysicalSize { width, height }) = event
            && *width > 0
            && *height > 0
        {
            let sf = (self.gl_win.window.scale_factor() as f32).max(1.0);
            self.logical_size = Vec2::new(*width as f32 / sf, *height as f32 / sf);
        }
        let _ = self.gl_win.on_window_event(event);
        if !matches!(event, WindowEvent::RedrawRequested) {
            return false;
        }

        let start = *self.started_at.get_or_insert_with(Instant::now);
        let Some(frame) = compute_flash_frame(start.elapsed().as_secs_f32(), self.logical_size)
        else {
            return true;
        };

        self.gl_win.make_current();
        let local_rect = Rect::from_min_size(Pos2::ZERO, self.logical_size);
        let raw_input = self.gl_win.take_egui_input();
        let frame_rect = Rect::from_center_size(frame.center, self.logical_size * frame.scale);
        let tint = Color32::from_rgba_unmultiplied(255, 255, 255, frame.alpha);
        let border_col = Color32::from_rgba_unmultiplied(
            self.accent.r(),
            self.accent.g(),
            self.accent.b(),
            frame.alpha,
        );

        let tex_id = self.tex.id();
        let full_output = self.gl_win.egui_ctx.run(raw_input, |ctx| {
            let painter = ctx.layer_painter(egui::LayerId::background());
            painter.rect_filled(
                local_rect,
                0.0,
                Color32::from_rgba_unmultiplied(6, 10, 20, frame.dim_alpha),
            );
            draw_rounded_texture_uv(
                &painter,
                tex_id,
                frame_rect.size(),
                Rect::from_min_size(Pos2::ZERO, frame_rect.size()),
                frame_rect,
                frame.radius,
                tint,
            );
            painter.rect_stroke(
                frame_rect,
                frame.radius,
                Stroke::new(2.8_f32, border_col),
                egui::StrokeKind::Outside,
            );
        });

        self.gl_win
            .paint_frame([0.0, 0.0, 0.0, 0.0], full_output);
        self.next_frame_at = Instant::now() + FLASH_FRAME_INTERVAL;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flash_frame_curve_continuity_and_bounds() {
        let size = Vec2::new(1000.0, 800.0);

        let f0 = compute_flash_frame(0.0, size).unwrap();
        assert!((f0.scale - 1.0).abs() < 1e-4);
        assert_eq!(f0.alpha, 255);
        assert_eq!(f0.center, Pos2::new(500.0, 400.0));

        let f_before = compute_flash_frame(FLASH_DURATION_SECS * 0.3499, size).unwrap();
        let f_after = compute_flash_frame(FLASH_DURATION_SECS * 0.3501, size).unwrap();
        assert!((f_before.scale - f_after.scale).abs() < 0.01);
        assert!((f_before.radius - f_after.radius).abs() < 0.1);
        assert!(f_before.alpha == 255 && f_after.alpha == 255);

        let f_late = compute_flash_frame(FLASH_DURATION_SECS * 0.85, size).unwrap();
        assert!(f_late.scale < 0.70 && f_late.scale >= 0.002);
        assert!(f_late.center.y > 400.0 && f_late.center.y <= 800.0);
        assert!(f_late.alpha < 255);

        assert!(compute_flash_frame(FLASH_DURATION_SECS, size).is_none());
        assert!(compute_flash_frame(FLASH_DURATION_SECS + 1.0, size).is_none());
        assert!(compute_flash_frame(f32::NAN, size).is_none());
    }
}
