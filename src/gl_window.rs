use std::ffi::CString;
use std::num::NonZeroU32;
use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;

use glutin::config::{ConfigTemplateBuilder, GlConfig};
use glutin::context::{
    ContextApi, ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext,
    PossiblyCurrentGlContext,
};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::surface::{GlSurface, Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface};
use glutin_winit::DisplayBuilder;
use raw_window_handle::HasWindowHandle;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes, WindowId};

#[derive(Clone, Copy)]
pub enum WindowChrome {
    Standard,
    Overlay { transparent: bool },
}

/// Shared OpenGL + egui window wrapper handling context creation, resizing, painting, and cleanup.
pub struct GlEguiWindow {
    pub window: Window,
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,
    gl: Arc<glow::Context>,
    pub egui_ctx: egui::Context,
    pub egui_state: egui_winit::State,
    egui_painter: egui_glow::Painter,
}

impl GlEguiWindow {
    pub fn open(
        event_loop: &ActiveEventLoop,
        win_attrs: WindowAttributes,
        chrome: WindowChrome,
        disable_vsync: bool,
        configure_ctx: impl FnOnce(&egui::Context),
    ) -> Result<Self, String> {
        let transparent = matches!(chrome, WindowChrome::Overlay { transparent: true });
        let template = if transparent {
            ConfigTemplateBuilder::new().with_alpha_size(8)
        } else {
            ConfigTemplateBuilder::new()
        };

        let built = panic::catch_unwind(AssertUnwindSafe(|| {
            DisplayBuilder::new()
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
        }));

        let (window, gl_config) = match built {
            Ok(Ok(pair)) => pair,
            Ok(Err(e)) => return Err(format!("Failed to create window: {e}")),
            Err(_) => return Err("No compatible OpenGL configuration found".to_string()),
        };

        let window = window.ok_or_else(|| "Window was not created".to_string())?;
        let raw_handle = window
            .window_handle()
            .map_err(|e| format!("Window handle error: {e}"))?
            .as_raw();

        #[cfg(target_os = "macos")]
        if let WindowChrome::Overlay { transparent } = chrome {
            configure_macos_ns_window(raw_handle, transparent);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = chrome;

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
        let w_nz = NonZeroU32::new(size.width.max(1)).ok_or("Invalid window width")?;
        let h_nz = NonZeroU32::new(size.height.max(1)).ok_or("Invalid window height")?;
        let attrs =
            SurfaceAttributesBuilder::<WindowSurface>::new().build(raw_handle, w_nz, h_nz);
        let gl_surface = unsafe {
            gl_display
                .create_window_surface(&gl_config, &attrs)
                .map_err(|e| format!("GL surface error: {e}"))?
        };
        let gl_context = not_current
            .make_current(&gl_surface)
            .map_err(|e| format!("Make current error: {e}"))?;

        if disable_vsync {
            let _ = gl_surface.set_swap_interval(&gl_context, SwapInterval::DontWait);
        }

        #[cfg(target_os = "macos")]
        if transparent {
            configure_macos_transparent_gl_context();
        }

        let gl = Arc::new(unsafe {
            glow::Context::from_loader_function(|s| {
                let Ok(c) = CString::new(s) else {
                    return std::ptr::null();
                };
                gl_display.get_proc_address(&c) as *const _
            })
        });

        let egui_ctx = egui::Context::default();
        configure_ctx(&egui_ctx);

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

        Ok(Self {
            window,
            gl_context,
            gl_surface,
            gl,
            egui_ctx,
            egui_state,
            egui_painter,
        })
    }

    pub fn window_id(&self) -> WindowId {
        self.window.id()
    }

    pub fn make_current(&self) {
        let _ = self.gl_context.make_current(&self.gl_surface);
    }

    pub fn handle_resize(&mut self, width: u32, height: u32) {
        if let (Some(w), Some(h)) = (NonZeroU32::new(width), NonZeroU32::new(height)) {
            self.make_current();
            self.gl_surface.resize(&self.gl_context, w, h);
        }
    }

    pub fn on_window_event(&mut self, event: &WindowEvent) -> bool {
        if let WindowEvent::Resized(PhysicalSize { width, height }) = event {
            self.handle_resize(*width, *height);
        }
        self.egui_state
            .on_window_event(&self.window, event)
            .repaint
    }

    pub fn take_egui_input(&mut self) -> egui::RawInput {
        self.egui_state.take_egui_input(&self.window)
    }

    pub fn paint_frame(&mut self, clear_rgba: [f32; 4], full_output: egui::FullOutput) -> bool {
        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);

        let clipped = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        let size = self.window.inner_size();
        unsafe {
            use glow::HasContext;
            self.gl.viewport(0, 0, size.width as i32, size.height as i32);
            self.gl
                .clear_color(clear_rgba[0], clear_rgba[1], clear_rgba[2], clear_rgba[3]);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }

        self.egui_painter.paint_and_update_textures(
            [size.width, size.height],
            full_output.pixels_per_point,
            &clipped,
            &full_output.textures_delta,
        );
        let _ = self.gl_surface.swap_buffers(&self.gl_context);

        full_output
            .viewport_output
            .values()
            .any(|v| v.repaint_delay.is_zero())
    }
}

impl Drop for GlEguiWindow {
    fn drop(&mut self) {
        self.make_current();
        self.egui_painter.destroy();
    }
}

#[cfg(target_os = "macos")]
fn configure_macos_ns_window(raw_handle: raw_window_handle::RawWindowHandle, transparent: bool) {
    if let raw_window_handle::RawWindowHandle::AppKit(appkit) = raw_handle {
        unsafe {
            let ns_view = appkit.ns_view.as_ptr() as *mut objc2::runtime::AnyObject;
            let ns_win: *mut objc2::runtime::AnyObject = objc2::msg_send![ns_view, window];
            if !ns_win.is_null() {
                let _: () = objc2::msg_send![ns_win, setLevel: 1000_isize];
                if transparent {
                    let _: () = objc2::msg_send![ns_win, setHasShadow: false];
                    let _: () = objc2::msg_send![ns_win, setOpaque: false];
                    if let Some(ns_color_cls) = objc2::runtime::AnyClass::get(c"NSColor") {
                        let clear: *mut objc2::runtime::AnyObject =
                            objc2::msg_send![ns_color_cls, clearColor];
                        let _: () = objc2::msg_send![ns_win, setBackgroundColor: clear];
                    }
                    let layer: *mut objc2::runtime::AnyObject = objc2::msg_send![ns_view, layer];
                    if !layer.is_null() {
                        let _: () = objc2::msg_send![layer, setOpaque: false];
                    }
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn configure_macos_transparent_gl_context() {
    unsafe {
        if let Some(cls) = objc2::runtime::AnyClass::get(c"NSOpenGLContext") {
            let ctx: *mut objc2::runtime::AnyObject = objc2::msg_send![cls, currentContext];
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
