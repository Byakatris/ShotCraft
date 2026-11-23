use egui::{Color32, ColorImage, Pos2, Rect, Stroke, Vec2};
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{WindowAttributes, WindowId, WindowLevel};

use crate::capture::CapturedScreen;
use crate::gl_window::{GlEguiWindow, WindowChrome};
use crate::material::draw_rounded_texture_uv;

pub struct CaptureFlashWindow {
    gl_win: GlEguiWindow,
    tex: egui::TextureHandle,
    logical_size: Vec2,
    accent: Color32,
    started_at: Option<std::time::Instant>,
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
            true,
            |_| {},
        )?;
        let _ = gl_win.window.set_cursor_hittest(false);

        let img_size = [screen.image.width() as usize, screen.image.height() as usize];
        let color_img = ColorImage::from_rgba_unmultiplied(img_size, screen.image.as_raw());
        let tex = gl_win
            .egui_ctx
            .load_texture("flash_shot", color_img, egui::TextureOptions::LINEAR);

        gl_win.window.request_redraw();

        Ok(Self {
            gl_win,
            tex,
            logical_size: Vec2::new(screen.logical_width, screen.logical_height),
            accent,
            started_at: None,
        })
    }

    pub fn window_id(&self) -> WindowId {
        self.gl_win.window_id()
    }

    pub fn request_redraw(&self) {
        self.gl_win.window.request_redraw();
    }

    /// Returns `true` when the shrink & fade-out animation is finished.
    pub fn handle_window_event(&mut self, event: &WindowEvent) -> bool {
        if let WindowEvent::Resized(PhysicalSize { width, height }) = event {
            self.gl_win.handle_resize(*width, *height);
            return false;
        }
        if !matches!(event, WindowEvent::RedrawRequested) {
            return false;
        }

        let start = *self.started_at.get_or_insert_with(std::time::Instant::now);
        let duration = 0.48_f32;
        let t = (start.elapsed().as_secs_f32() / duration).clamp(0.0, 1.0);
        if t >= 1.0 {
            return true;
        }

        self.gl_win.make_current();
        let local_rect = Rect::from_min_size(Pos2::ZERO, self.logical_size);

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

        let raw_input = self.gl_win.take_egui_input();
        let frame_rect = Rect::from_center_size(center, self.logical_size * scale);
        let tint = Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
        let border_col = Color32::from_rgba_unmultiplied(
            self.accent.r(),
            self.accent.g(),
            self.accent.b(),
            alpha,
        );

        let tex_id = self.tex.id();
        let full_output = self.gl_win.egui_ctx.run(raw_input, |ctx| {
            egui::Area::new(egui::Id::new("capture_flash_area"))
                .fixed_pos(Pos2::ZERO)
                .show(ctx, |ui| {
                    let painter = ui.painter();
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

        self.gl_win
            .paint_frame([0.0, 0.0, 0.0, 0.0], full_output);
        self.gl_win.window.request_redraw();
        false
    }
}
