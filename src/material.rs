use std::f32::consts::FRAC_PI_2;

use egui::epaint::{Mesh, Vertex};
use egui::{Color32, ColorImage, Pos2, Rect, Response, Sense, Stroke, TextureId, Ui, Vec2};
use image::RgbaImage;

#[derive(Debug, Clone, Copy)]
pub struct ThemeColors {
    pub surface: Color32,
    pub surface_container: Color32,
    pub surface_container_high: Color32,
    pub outline: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub on_accent: Color32,
    pub on_surface: Color32,
}

const BG_STOPS: [(f32, Color32); 8] = [
    (0.00, Color32::from_rgb(18, 20, 26)),
    (0.16, Color32::from_rgb(14, 20, 36)),
    (0.28, Color32::from_rgb(15, 22, 38)), // default deep navy
    (0.42, Color32::from_rgb(24, 20, 44)),
    (0.56, Color32::from_rgb(12, 32, 38)),
    (0.70, Color32::from_rgb(16, 34, 24)),
    (0.85, Color32::from_rgb(36, 18, 30)),
    (1.00, Color32::from_rgb(10, 11, 14)),
];

const COLOR_STOPS: [(f32, Color32); 10] = [
    (0.00, Color32::from_rgb(255, 255, 255)),
    (0.11, Color32::from_rgb(255, 59, 48)),
    (0.22, Color32::from_rgb(255, 149, 0)),
    (0.34, Color32::from_rgb(255, 214, 10)),
    (0.46, Color32::from_rgb(52, 199, 89)),
    (0.56, Color32::from_rgb(94, 234, 212)), // default light teal
    (0.70, Color32::from_rgb(10, 132, 255)),
    (0.81, Color32::from_rgb(175, 82, 222)),
    (0.91, Color32::from_rgb(255, 45, 85)),
    (1.00, Color32::from_rgb(18, 20, 26)),
];

const COLOR_PRESETS: [f32; 10] = [0.00, 0.11, 0.22, 0.34, 0.46, 0.56, 0.70, 0.81, 0.91, 1.00];
const SIZE_PRESETS: [f32; 5] = [2.0, 5.0, 10.0, 15.0, 20.0];
const SIZE_PRESET_LABELS: [&str; 5] = ["2 px", "5 px", "10 px", "15 px", "20 px"];

impl ThemeColors {
    pub fn from_positions(bg_pos: f32, accent_pos: f32) -> Self {
        let surface = sample_stops(&BG_STOPS, bg_pos);
        let surface_container = lighten(surface, 10);
        let surface_container_high = lighten(surface, 24);
        let outline = lighten(surface, 48);
        let accent = sample_stops(&COLOR_STOPS, accent_pos);
        let lum = 0.299 * accent.r() as f32 + 0.587 * accent.g() as f32 + 0.114 * accent.b() as f32;
        let accent_hover = if lum > 235.0 {
            darken(accent, 22)
        } else {
            lighten(accent, 24)
        };
        let on_accent = if lum > 140.0 {
            Color32::from_rgb(8, 20, 30)
        } else {
            Color32::WHITE
        };
        Self {
            surface,
            surface_container,
            surface_container_high,
            outline,
            accent,
            accent_hover,
            on_accent,
            on_surface: Color32::from_rgb(232, 239, 250),
        }
    }
}

pub(crate) fn safe_clamp01(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn lighten(c: Color32, delta: u8) -> Color32 {
    Color32::from_rgb(
        c.r().saturating_add(delta),
        c.g().saturating_add(delta),
        c.b().saturating_add(delta),
    )
}

fn darken(c: Color32, delta: u8) -> Color32 {
    Color32::from_rgb(
        c.r().saturating_sub(delta),
        c.g().saturating_sub(delta),
        c.b().saturating_sub(delta),
    )
}

fn sample_stops(stops: &[(f32, Color32)], t: f32) -> Color32 {
    let t = safe_clamp01(t);
    for win in stops.windows(2) {
        let (t0, c0) = win[0];
        let (t1, c1) = win[1];
        if t <= t1 {
            let f = safe_clamp01((t - t0) / (t1 - t0).max(1e-5));
            return lerp_color(c0, c1, f);
        }
    }
    stops.last().map_or(Color32::WHITE, |s| s.1)
}

pub fn color_from_slider(t: f32) -> Color32 {
    sample_stops(&COLOR_STOPS, t)
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = safe_clamp01(t);
    let lerp = |x: u8, y: u8| -> u8 { (x as f32 + (y as f32 - x as f32) * t).round() as u8 };
    Color32::from_rgba_unmultiplied(
        lerp(a.r(), b.r()),
        lerp(a.g(), b.g()),
        lerp(a.b(), b.b()),
        lerp(a.a(), b.a()),
    )
}

/// Draw a rounded-rectangle textured mesh with explicit global UV rect mapping onto `local_rect`.
pub fn draw_rounded_texture_uv(
    painter: &egui::Painter,
    tex_id: TextureId,
    virtual_size: Vec2,
    global_rect: Rect,
    local_rect: Rect,
    radius: f32,
    tint: Color32,
) {
    if local_rect.width() <= 0.5 || local_rect.height() <= 0.5 {
        return;
    }
    let vw = virtual_size.x.max(1.0);
    let vh = virtual_size.y.max(1.0);
    let r = radius
        .min(local_rect.width() * 0.5)
        .min(local_rect.height() * 0.5)
        .max(0.0);
    let mut perimeter = Vec::with_capacity(32);
    if r < 0.5 {
        perimeter.push(local_rect.left_top());
        perimeter.push(local_rect.right_top());
        perimeter.push(local_rect.right_bottom());
        perimeter.push(local_rect.left_bottom());
    } else {
        let segs = 7;
        let corners = [
            (Pos2::new(local_rect.right() - r, local_rect.top() + r), -FRAC_PI_2, 0.0),
            (Pos2::new(local_rect.right() - r, local_rect.bottom() - r), 0.0, FRAC_PI_2),
            (Pos2::new(local_rect.left() + r, local_rect.bottom() - r), FRAC_PI_2, std::f32::consts::PI),
            (Pos2::new(local_rect.left() + r, local_rect.top() + r), std::f32::consts::PI, std::f32::consts::PI + FRAC_PI_2),
        ];
        for (center, a0, a1) in corners {
            for i in 0..=segs {
                let t = i as f32 / segs as f32;
                let a = a0 + (a1 - a0) * t;
                perimeter.push(Pos2::new(center.x + r * a.cos(), center.y + r * a.sin()));
            }
        }
    }

    let mut mesh = Mesh::with_texture(tex_id);
    mesh.vertices.reserve(perimeter.len() + 1);
    mesh.indices.reserve(perimeter.len() * 3);
    let offset = global_rect.min - local_rect.min;
    let uv_of = |lp: Pos2| -> Pos2 {
        let gp = lp + offset;
        Pos2::new(safe_clamp01(gp.x / vw), safe_clamp01(gp.y / vh))
    };

    let center = local_rect.center();
    mesh.vertices.push(Vertex {
        pos: center,
        uv: uv_of(center),
        color: tint,
    });
    for &p in &perimeter {
        mesh.vertices.push(Vertex {
            pos: p,
            uv: uv_of(p),
            color: tint,
        });
    }
    let n = perimeter.len() as u32;
    for i in 0..n {
        let next = (i + 1) % n;
        mesh.indices.push(0);
        mesh.indices.push(1 + i);
        mesh.indices.push(1 + next);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// 2x stronger blur texture for Text readability backdrop using 4x4 box averaging.
pub fn make_strong_text_blur_image(src: &RgbaImage) -> ColorImage {
    let sw = src.width();
    let sh = src.height();
    if sw == 0 || sh == 0 {
        return ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 255]);
    }
    let factor = 4u32;
    let w = (sw / factor).max(1) as usize;
    let h = (sh / factor).max(1) as usize;
    let mut buf = vec![[0f32; 3]; w * h];
    for y in 0..h {
        let sy0 = (y as u32) * factor;
        let sy1 = (sy0 + factor).min(sh).max(sy0 + 1);
        for x in 0..w {
            let sx0 = (x as u32) * factor;
            let sx1 = (sx0 + factor).min(sw).max(sx0 + 1);
            let mut rs = 0.0_f32;
            let mut gs = 0.0_f32;
            let mut bs = 0.0_f32;
            let mut cnt = 0.0_f32;
            for py in sy0..sy1 {
                for px in sx0..sx1 {
                    let p = src.get_pixel(px, py);
                    rs += p[0] as f32;
                    gs += p[1] as f32;
                    bs += p[2] as f32;
                    cnt += 1.0;
                }
            }
            buf[y * w + x] = [rs / cnt, gs / cnt, bs / cnt];
        }
    }

    crate::annotations::separable_box_blur_rgb(&mut buf, w, h, 6, 2);

    let pixels = buf
        .into_iter()
        .map(|c| {
            Color32::from_rgba_unmultiplied(
                c[0].round().clamp(0.0, 255.0) as u8,
                c[1].round().clamp(0.0, 255.0) as u8,
                c[2].round().clamp(0.0, 255.0) as u8,
                255,
            )
        })
        .collect();
    ColorImage {
        size: [w, h],
        pixels,
    }
}

fn draw_rounded_texture(
    painter: &egui::Painter,
    tex_id: TextureId,
    screen_size: Vec2,
    rect: Rect,
    radius: f32,
    tint: Color32,
) {
    draw_rounded_texture_uv(painter, tex_id, screen_size, rect, rect, radius, tint);
}

pub fn draw_m3_panel(painter: &egui::Painter, rect: Rect, radius: f32, theme: &ThemeColors) {
    painter.rect_filled(
        rect.expand(1.5).translate(Vec2::new(0.0, 2.0)),
        radius + 1.5,
        Color32::from_rgba_unmultiplied(0, 0, 0, 72),
    );
    painter.rect_filled(rect, radius, theme.surface);
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.1_f32, theme.outline),
        egui::StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(1.0),
        (radius - 1.0).max(0.0),
        Stroke::new(0.8_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 22)),
        egui::StrokeKind::Inside,
    );
}

pub fn draw_m3_selection_frame(
    painter: &egui::Painter,
    sharp_tex: TextureId,
    screen_size: Vec2,
    sel: Rect,
    show_handles: bool,
    theme: &ThemeColors,
) {
    let radius = 8.0_f32.min(sel.width() * 0.2).min(sel.height() * 0.2);
    draw_rounded_texture(painter, sharp_tex, screen_size, sel, radius, Color32::WHITE);

    painter.rect_stroke(
        sel,
        radius,
        Stroke::new(1.8_f32, theme.accent),
        egui::StrokeKind::Outside,
    );

    if show_handles {
        let handle_pts = [
            sel.left_top(),
            Pos2::new(sel.center().x, sel.top()),
            sel.right_top(),
            Pos2::new(sel.right(), sel.center().y),
            sel.right_bottom(),
            Pos2::new(sel.center().x, sel.bottom()),
            sel.left_bottom(),
            Pos2::new(sel.left(), sel.center().y),
        ];
        for p in handle_pts {
            painter.circle_filled(p, 5.5, theme.accent);
            painter.circle_stroke(p, 5.5, Stroke::new(1.3_f32, theme.surface));
        }
    }
}

/// Pure semi-implicit Euler damped harmonic spring step with adaptive substepping and NaN guard.
pub fn spring_step(
    mut x: f32,
    mut v: f32,
    target: f32,
    dt: f32,
    response: f32,
    damping_fraction: f32,
) -> (f32, f32, bool) {
    if !target.is_finite() {
        return (0.0, 0.0, true);
    }
    if !x.is_finite() || !v.is_finite() {
        return (target, 0.0, true);
    }
    let dt = if dt.is_finite() {
        dt.clamp(0.001, 0.05)
    } else {
        0.016
    };
    let omega = std::f32::consts::TAU / response.max(0.12);
    let stiffness = omega * omega;
    let damping = 2.0 * damping_fraction.clamp(0.2, 1.5) * omega;
    let substeps = ((omega * dt / 0.8).ceil() as usize).clamp(2, 8);
    let sub_dt = dt / substeps as f32;

    for _ in 0..substeps {
        let accel = -stiffness * (x - target) - damping * v;
        v += accel * sub_dt;
        x += v * sub_dt;
    }

    if !x.is_finite() || !v.is_finite() {
        return (target, 0.0, true);
    }
    if (x - target).abs() < 0.0008 && v.abs() < 0.0008 {
        (target, 0.0, true)
    } else {
        (x, v, false)
    }
}

/// Apple SwiftUI / CASpringAnimation-style damped harmonic spring physics.
pub fn spring_animate(
    ctx: &egui::Context,
    id: egui::Id,
    target: f32,
    response: f32,
    damping_fraction: f32,
) -> f32 {
    let dt = ctx.input(|i| i.stable_dt);
    let (x0, v0) = ctx
        .data(|d| d.get_temp::<(f32, f32)>(id))
        .unwrap_or((target, 0.0));
    let (x, v, settled) = spring_step(x0, v0, target, dt, response, damping_fraction);
    if !settled {
        ctx.request_repaint();
    }
    ctx.data_mut(|d| d.insert_temp(id, (x, v)));
    x
}

pub fn spring_scale(ctx: &egui::Context, id: egui::Id, hovered: bool, pressed: bool) -> f32 {
    let target = if pressed {
        0.91
    } else if hovered {
        1.05
    } else {
        1.0
    };
    spring_animate(ctx, id.with("apple_sp_s"), target, 0.22, 0.58).clamp(0.86, 1.12)
}

pub fn morph_radius(
    ctx: &egui::Context,
    id: egui::Id,
    height: f32,
    squared: bool,
) -> f32 {
    let t = spring_animate(
        ctx,
        id.with("apple_sp_r"),
        if squared { 1.0 } else { 0.0 },
        0.25,
        0.70,
    )
    .clamp(0.0, 1.0);
    let full_round = height * 0.5;
    let rounded_sq = 7.0_f32.min(full_round);
    full_round + (rounded_sq - full_round) * t
}

pub fn m3_tool_button(
    ui: &mut Ui,
    id_salt: &str,
    size: Vec2,
    active: bool,
    tooltip: &str,
    theme: &ThemeColors,
    draw_icon: impl FnOnce(&egui::Painter, Rect, Color32),
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hovered = resp.hovered();
    let pressed = resp.is_pointer_button_down_on();
    let id = ui.make_persistent_id(id_salt);
    let scale = spring_scale(ui.ctx(), id, hovered, pressed);
    let draw_rect = Rect::from_center_size(rect.center(), rect.size() * scale);

    let t = spring_animate(
        ui.ctx(),
        id.with("active_t"),
        if active { 1.0 } else { 0.0 },
        0.24,
        0.74,
    )
    .clamp(0.0, 1.0);
    let radius = morph_radius(ui.ctx(), id, draw_rect.height(), active || pressed);

    let base_fill = if hovered && !active {
        lighten(theme.surface_container_high, 14)
    } else {
        theme.surface_container_high
    };
    let active_fill = if hovered && active {
        theme.accent_hover
    } else {
        theme.accent
    };
    let fill = lerp_color(base_fill, active_fill, t);
    let icon_color = lerp_color(theme.on_surface, theme.on_accent, t);

    let painter = ui.painter();
    painter.rect_filled(draw_rect, radius, fill);
    if t < 0.5 && hovered {
        painter.rect_stroke(
            draw_rect,
            radius,
            Stroke::new(
                1.0_f32,
                Color32::from_rgba_unmultiplied(theme.accent.r(), theme.accent.g(), theme.accent.b(), 140),
            ),
            egui::StrokeKind::Inside,
        );
    }

    draw_icon(painter, draw_rect.shrink(6.0 * scale), icon_color);
    resp.on_hover_text(tooltip)
}

pub fn m3_action_button(
    ui: &mut Ui,
    size: Vec2,
    primary: bool,
    tooltip: &str,
    theme: &ThemeColors,
    draw_icon: impl FnOnce(&egui::Painter, Rect, Color32),
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hovered = resp.hovered();
    let pressed = resp.is_pointer_button_down_on();
    let scale = spring_scale(ui.ctx(), resp.id, hovered, pressed);
    let draw_rect = Rect::from_center_size(rect.center(), rect.size() * scale);
    let radius = morph_radius(ui.ctx(), resp.id, draw_rect.height(), pressed);
    let painter = ui.painter();

    let (fill, fg) = if primary {
        (
            if hovered { theme.accent_hover } else { theme.accent },
            theme.on_accent,
        )
    } else {
        (
            if hovered {
                lighten(theme.surface_container_high, 14)
            } else {
                theme.surface_container_high
            },
            theme.on_surface,
        )
    };

    painter.rect_filled(draw_rect, radius, fill);
    if !primary {
        painter.rect_stroke(
            draw_rect,
            radius,
            Stroke::new(
                1.0_f32,
                if hovered { theme.accent } else { theme.outline },
            ),
            egui::StrokeKind::Inside,
        );
    }

    draw_icon(painter, draw_rect.shrink(6.0 * scale), fg);
    resp.on_hover_text(tooltip)
}

fn draw_teardrop(
    painter: &egui::Painter,
    center: Pos2,
    r: f32,
    tip: Pos2,
    fill: Color32,
    halo: Color32,
) {
    let to_tip = tip - center;
    let d = to_tip.length();
    if d <= r + 1.5 {
        painter.circle_filled(center, r + 1.6, halo);
        painter.circle_filled(center, r, fill);
        painter.circle_stroke(center, r + 0.8, Stroke::new(1.6_f32, Color32::WHITE));
        return;
    }

    let dir_angle = to_tip.y.atan2(to_tip.x);
    let alpha = (r / d).clamp(-0.99, 0.99).acos();
    let a_start = dir_angle + alpha;
    let a_end = dir_angle + std::f32::consts::TAU - alpha;

    let arc_segs = 20usize;
    let mut pts = Vec::with_capacity(arc_segs + 3);
    for i in 0..=arc_segs {
        let f = i as f32 / arc_segs as f32;
        let a = a_start + (a_end - a_start) * f;
        pts.push(Pos2::new(center.x + r * a.cos(), center.y + r * a.sin()));
    }
    let perp = Vec2::new(-to_tip.y / d, to_tip.x / d) * 1.4;
    pts.push(tip - perp);
    pts.push(tip + perp);

    let mut halo_pts = Vec::with_capacity(pts.len());
    halo_pts.extend_from_slice(&pts);
    painter.add(egui::epaint::PathShape::convex_polygon(
        halo_pts,
        halo,
        Stroke::new(2.8_f32, halo),
    ));
    painter.add(egui::epaint::PathShape::convex_polygon(
        pts,
        fill,
        Stroke::new(1.6_f32, Color32::WHITE),
    ));
}

/// Draw a horizontal or vertical color-stops gradient track and return the inner `[0..1]` mapping rect.
fn draw_stops_track(
    painter: &egui::Painter,
    track_rect: Rect,
    stops: &[(f32, Color32)],
    outline: Color32,
    vertical: bool,
) -> Rect {
    let r = if vertical {
        track_rect.width() * 0.5
    } else {
        track_rect.height() * 0.5
    };
    let first_c = stops.first().map_or(Color32::WHITE, |s| s.1);
    let last_c = stops.last().map_or(Color32::WHITE, |s| s.1);

    let inner = if vertical {
        painter.circle_filled(Pos2::new(track_rect.center().x, track_rect.top() + r), r, first_c);
        painter.circle_filled(Pos2::new(track_rect.center().x, track_rect.bottom() - r), r, last_c);
        Rect::from_min_max(
            Pos2::new(track_rect.left(), track_rect.top() + r),
            Pos2::new(track_rect.right(), track_rect.bottom() - r),
        )
    } else {
        painter.circle_filled(Pos2::new(track_rect.left() + r, track_rect.center().y), r, first_c);
        painter.circle_filled(Pos2::new(track_rect.right() - r, track_rect.center().y), r, last_c);
        Rect::from_min_max(
            Pos2::new(track_rect.left() + r, track_rect.top()),
            Pos2::new(track_rect.right() - r, track_rect.bottom()),
        )
    };

    let n = stops.len();
    let mut mesh = Mesh::default();
    mesh.vertices.reserve(n * 2);
    mesh.indices.reserve(n.saturating_sub(1) * 6);
    for (idx, &(stop_t, col)) in stops.iter().enumerate() {
        let (p0, p1) = if vertical {
            let y = inner.top() + inner.height() * stop_t;
            (Pos2::new(inner.left(), y), Pos2::new(inner.right(), y))
        } else {
            let x = inner.left() + inner.width() * stop_t;
            (Pos2::new(x, inner.top()), Pos2::new(x, inner.bottom()))
        };
        mesh.vertices.push(Vertex { pos: p0, uv: Pos2::ZERO, color: col });
        mesh.vertices.push(Vertex { pos: p1, uv: Pos2::ZERO, color: col });
        if idx + 1 < n {
            let base = (idx * 2) as u32;
            if vertical {
                mesh.indices
                    .extend_from_slice(&[base, base + 1, base + 2, base + 1, base + 3, base + 2]);
            } else {
                mesh.indices
                    .extend_from_slice(&[base, base + 2, base + 1, base + 1, base + 2, base + 3]);
            }
        }
    }
    painter.add(egui::Shape::mesh(mesh));
    painter.rect_stroke(
        track_rect,
        r,
        Stroke::new(1.0_f32, outline),
        egui::StrokeKind::Outside,
    );
    inner
}

pub fn m3_color_slider(
    ui: &mut Ui,
    width: f32,
    color_pos: &mut f32,
    tooltip: &str,
    theme: &ThemeColors,
) -> (Color32, bool) {
    color_slider_impl(ui, width.max(120.0), color_pos, tooltip, theme, false)
}

pub fn m3_color_slider_vertical(
    ui: &mut Ui,
    height: f32,
    color_pos: &mut f32,
    tooltip: &str,
    theme: &ThemeColors,
) -> (Color32, bool) {
    color_slider_impl(ui, height.max(120.0), color_pos, tooltip, theme, true)
}

fn color_slider_impl(
    ui: &mut Ui,
    extent: f32,
    color_pos: &mut f32,
    tooltip: &str,
    theme: &ThemeColors,
    vertical: bool,
) -> (Color32, bool) {
    *color_pos = safe_clamp01(*color_pos);
    let total_size = if vertical {
        Vec2::new(32.0, extent)
    } else {
        Vec2::new(extent, 28.0)
    };
    let (rect, _) = ui.allocate_exact_size(total_size, Sense::hover());
    let track_thick = 9.0;
    let track_rect = if vertical {
        Rect::from_center_size(
            Pos2::new(rect.right() - 8.5, rect.center().y),
            Vec2::new(track_thick, total_size.y - 14.0),
        )
    } else {
        Rect::from_center_size(
            Pos2::new(rect.center().x, rect.bottom() - 7.0),
            Vec2::new((total_size.x - 14.0).max(40.0), track_thick),
        )
    };
    let r_cap = track_thick * 0.5;
    let inner_span = if vertical {
        (track_rect.height() - r_cap * 2.0).max(1.0)
    } else {
        (track_rect.width() - r_cap * 2.0).max(1.0)
    };
    let inner_start = if vertical {
        track_rect.top() + r_cap
    } else {
        track_rect.left() + r_cap
    };

    for (idx, &preset_t) in COLOR_PRESETS.iter().enumerate() {
        let dot_center = if vertical {
            Pos2::new(rect.left() + 6.5, inner_start + inner_span * preset_t)
        } else {
            Pos2::new(inner_start + inner_span * preset_t, rect.top() + 5.5)
        };
        let hit_rect = Rect::from_center_size(dot_center, Vec2::splat(13.0));
        let preset_resp = ui.interact(
            hit_rect,
            ui.make_persistent_id(("m3_color_preset", vertical, idx)),
            Sense::click(),
        );
        if preset_resp.clicked() {
            *color_pos = preset_t;
        }
        let col = sample_stops(&COLOR_STOPS, preset_t);
        let selected = (*color_pos - preset_t).abs() < 0.03;
        let dot_scale = spring_scale(
            ui.ctx(),
            preset_resp.id,
            preset_resp.hovered(),
            preset_resp.is_pointer_button_down_on(),
        );
        let r = (if selected { 4.7 } else { 3.9 }) * dot_scale;
        ui.painter().circle_filled(dot_center, r, col);
        ui.painter().circle_stroke(
            dot_center,
            r,
            Stroke::new(
                if selected { 1.4_f32 } else { 0.9_f32 },
                if selected { Color32::WHITE } else { theme.outline },
            ),
        );
    }

    let slider_hit = if vertical {
        Rect::from_min_max(Pos2::new(rect.left() + 12.0, rect.top()), rect.max)
    } else {
        Rect::from_min_max(Pos2::new(rect.left(), rect.top() + 11.0), rect.max)
    };
    let resp = ui.interact(
        slider_hit,
        ui.make_persistent_id(("m3_color_track_interact", vertical)),
        Sense::click_and_drag(),
    );

    if (resp.dragged() || resp.clicked())
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let coord = if vertical { pos.y } else { pos.x };
        *color_pos = safe_clamp01((coord - inner_start) / inner_span);
    }

    let current = sample_stops(&COLOR_STOPS, *color_pos);
    let active = resp.dragged() || resp.is_pointer_button_down_on();
    let vis_id = ui.make_persistent_id(("m3_color_vis_pos", vertical));
    let vis_pos = if active {
        ui.ctx().data_mut(|d| d.insert_temp(vis_id, (*color_pos, 0.0_f32)));
        *color_pos
    } else {
        safe_clamp01(spring_animate(ui.ctx(), vis_id, *color_pos, 0.24, 0.72))
    };

    let screen_rect = ui.ctx().screen_rect();
    let painter = ui.painter().with_clip_rect(screen_rect);
    let inner = draw_stops_track(&painter, track_rect, &COLOR_STOPS, theme.outline, vertical);

    let pop_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(("m3_color_droplet", vertical)),
        active,
        0.14,
    );
    let drop_r = 6.0 + 6.5 * pop_t;

    if vertical {
        let thumb_y = inner.top() + inner.height() * vis_pos;
        let base_x = track_rect.center().x;
        let dir = if base_x + 34.0 <= screen_rect.max.x { 1.0 } else { -1.0 };
        let drop_center = Pos2::new(base_x + 17.5 * pop_t * dir, thumb_y);
        let tip = Pos2::new(base_x + 1.0 * dir, thumb_y);
        draw_teardrop(&painter, drop_center, drop_r, tip, current, theme.surface);
        if pop_t > 0.05 {
            let half_w = (track_thick * 0.5 + 2.5) * pop_t;
            let bar = Rect::from_center_size(Pos2::new(base_x, thumb_y), Vec2::new(half_w * 2.0, 2.4));
            painter.rect_filled(bar, 1.2, Color32::WHITE);
        }
    } else {
        let thumb_x = inner.left() + inner.width() * vis_pos;
        let base_y = track_rect.center().y;
        let dir = if base_y + 34.0 <= screen_rect.max.y { 1.0 } else { -1.0 };
        let drop_center = Pos2::new(thumb_x, base_y + 17.5 * pop_t * dir);
        let tip = Pos2::new(thumb_x, base_y + 1.0 * dir);
        draw_teardrop(&painter, drop_center, drop_r, tip, current, theme.surface);
        if pop_t > 0.05 {
            let half_h = (track_thick * 0.5 + 2.5) * pop_t;
            let bar = Rect::from_center_size(Pos2::new(thumb_x, base_y), Vec2::new(2.4, half_h * 2.0));
            painter.rect_filled(bar, 1.2, Color32::WHITE);
        }
    }

    let dragging = resp.dragged();
    resp.on_hover_text(tooltip);
    (current, dragging)
}

fn draw_simple_stops_slider(
    ui: &mut Ui,
    pos_val: &mut f32,
    stops: &[(f32, Color32)],
    theme: &ThemeColors,
) -> (bool, bool) {
    *pos_val = safe_clamp01(*pos_val);
    let total_size = Vec2::new(186.0, 30.0);
    let (rect, resp) = ui.allocate_exact_size(total_size, Sense::click_and_drag());
    let track_h = 10.0;
    let track_rect = Rect::from_center_size(rect.center(), Vec2::new(total_size.x - 16.0, track_h));
    let r = track_h * 0.5;
    let inner_left = track_rect.left() + r;
    let inner_w = (track_rect.width() - r * 2.0).max(1.0);

    let mut changed = false;
    if (resp.dragged() || resp.clicked())
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let next = safe_clamp01((pos.x - inner_left) / inner_w);
        if (next - *pos_val).abs() > 1e-4 {
            *pos_val = next;
            changed = true;
        }
    }

    let current = sample_stops(stops, *pos_val);
    let inner = draw_stops_track(ui.painter(), track_rect, stops, theme.outline, false);

    let thumb_x = inner.left() + inner.width() * *pos_val;
    let c = Pos2::new(thumb_x, track_rect.center().y);
    ui.painter().circle_filled(c, 8.0, current);
    ui.painter()
        .circle_stroke(c, 8.0, Stroke::new(1.5_f32, Color32::WHITE));

    let committed = resp.drag_stopped() || resp.clicked();
    (changed, committed)
}

pub fn m3_bg_slider(ui: &mut Ui, bg_pos: &mut f32, theme: &ThemeColors) -> (bool, bool) {
    draw_simple_stops_slider(ui, bg_pos, &BG_STOPS, theme)
}

pub fn m3_accent_slider(ui: &mut Ui, accent_pos: &mut f32, theme: &ThemeColors) -> (bool, bool) {
    draw_simple_stops_slider(ui, accent_pos, &COLOR_STOPS, theme)
}

pub fn m3_size_slider(
    ui: &mut Ui,
    value: &mut f32,
    min: f32,
    max: f32,
    tooltip: &str,
    theme: &ThemeColors,
) -> bool {
    size_slider_impl(ui, 138.0, value, min, max, tooltip, theme, false)
}

pub fn m3_size_slider_vertical(
    ui: &mut Ui,
    height: f32,
    value: &mut f32,
    min: f32,
    max: f32,
    tooltip: &str,
    theme: &ThemeColors,
) -> bool {
    size_slider_impl(ui, height.max(90.0), value, min, max, tooltip, theme, true)
}

#[allow(clippy::too_many_arguments)]
fn size_slider_impl(
    ui: &mut Ui,
    extent: f32,
    value: &mut f32,
    min: f32,
    max: f32,
    tooltip: &str,
    theme: &ThemeColors,
    vertical: bool,
) -> bool {
    let span = (max - min).max(1e-4);
    if !value.is_finite() {
        *value = min;
    } else {
        *value = value.clamp(min, min + span);
    }

    let total_size = if vertical {
        Vec2::new(32.0, extent)
    } else {
        Vec2::new(extent, 28.0)
    };
    let (rect, _) = ui.allocate_exact_size(total_size, Sense::hover());
    let track_thick = 8.0;
    let track_rect = if vertical {
        Rect::from_center_size(
            Pos2::new(rect.right() - 8.5, rect.center().y),
            Vec2::new(track_thick, total_size.y - 14.0),
        )
    } else {
        Rect::from_center_size(
            Pos2::new(rect.center().x, rect.bottom() - 7.0),
            Vec2::new((total_size.x - 14.0).max(40.0), track_thick),
        )
    };

    for (idx, &sz) in SIZE_PRESETS.iter().enumerate() {
        let pt = safe_clamp01((sz - min) / span);
        let dot_center = if vertical {
            Pos2::new(rect.left() + 6.5, track_rect.bottom() - track_rect.height() * pt)
        } else {
            Pos2::new(track_rect.left() + track_rect.width() * pt, rect.top() + 5.5)
        };
        let hit_rect = Rect::from_center_size(dot_center, Vec2::splat(13.0));
        let preset_resp = ui.interact(
            hit_rect,
            ui.make_persistent_id(("m3_size_preset", vertical, idx)),
            Sense::click(),
        );
        if preset_resp.clicked() {
            *value = sz.clamp(min, max);
        }
        let selected = (*value - sz).abs() < 0.6;
        let base_r = 2.2 + 2.6 * pt;
        let dot_scale = spring_scale(
            ui.ctx(),
            preset_resp.id,
            preset_resp.hovered(),
            preset_resp.is_pointer_button_down_on(),
        );
        let r = (if selected { base_r + 0.6 } else { base_r }) * dot_scale;
        let fill = if selected {
            theme.accent
        } else if preset_resp.hovered() {
            theme.on_surface
        } else {
            Color32::from_rgba_unmultiplied(
                theme.on_surface.r(),
                theme.on_surface.g(),
                theme.on_surface.b(),
                150,
            )
        };
        ui.painter().circle_filled(dot_center, r, fill);
        ui.painter().circle_stroke(
            dot_center,
            r,
            Stroke::new(
                if selected { 1.3_f32 } else { 0.8_f32 },
                if selected { Color32::WHITE } else { theme.outline },
            ),
        );
        preset_resp.on_hover_text(SIZE_PRESET_LABELS[idx]);
    }

    let slider_hit = if vertical {
        Rect::from_min_max(Pos2::new(rect.left() + 12.0, rect.top()), rect.max)
    } else {
        Rect::from_min_max(Pos2::new(rect.left(), rect.top() + 11.0), rect.max)
    };
    let resp = ui.interact(
        slider_hit,
        ui.make_persistent_id(("m3_size_track_interact", vertical)),
        Sense::click_and_drag(),
    );

    if (resp.dragged() || resp.clicked())
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let t = if vertical {
            safe_clamp01((track_rect.bottom() - pos.y) / track_rect.height().max(1.0))
        } else {
            safe_clamp01((pos.x - track_rect.left()) / track_rect.width().max(1.0))
        };
        *value = min + t * span;
    }

    let t = safe_clamp01((*value - min) / span);
    let active = resp.dragged() || resp.is_pointer_button_down_on();
    let vis_id = ui.make_persistent_id(("m3_size_vis_t", vertical));
    let vis_t = if active {
        ui.ctx().data_mut(|d| d.insert_temp(vis_id, (t, 0.0_f32)));
        t
    } else {
        safe_clamp01(spring_animate(ui.ctx(), vis_id, t, 0.24, 0.72))
    };

    let screen_rect = ui.ctx().screen_rect();
    let painter = ui.painter().with_clip_rect(screen_rect);

    painter.rect_filled(track_rect, track_thick * 0.5, theme.surface_container_high);
    let fill_rect = if vertical {
        let fill_h = (track_rect.height() * vis_t).max(track_thick);
        Rect::from_min_max(Pos2::new(track_rect.left(), track_rect.bottom() - fill_h), track_rect.max)
    } else {
        let fill_w = (track_rect.width() * vis_t).max(track_thick);
        Rect::from_min_size(track_rect.min, Vec2::new(fill_w, track_thick))
    };
    painter.rect_filled(fill_rect, track_thick * 0.5, theme.accent);

    let pop_t = ui.ctx().animate_bool_with_time(
        ui.make_persistent_id(("m3_size_droplet", vertical)),
        active,
        0.14,
    );
    let drop_r = 6.0 + 7.2 * pop_t;

    let drop_center = if vertical {
        let thumb_y = track_rect.bottom() - track_rect.height() * vis_t;
        let base_x = track_rect.center().x;
        let dir = if base_x + 34.0 <= screen_rect.max.x { 1.0 } else { -1.0 };
        let dc = Pos2::new(base_x + 17.5 * pop_t * dir, thumb_y);
        let tip = Pos2::new(base_x + 1.0 * dir, thumb_y);
        draw_teardrop(&painter, dc, drop_r, tip, theme.accent, theme.surface);
        if pop_t > 0.05 {
            let half_w = (track_thick * 0.5 + 2.5) * pop_t;
            let bar = Rect::from_center_size(Pos2::new(base_x, thumb_y), Vec2::new(half_w * 2.0, 2.4));
            painter.rect_filled(bar, 1.2, Color32::WHITE);
        }
        dc
    } else {
        let thumb_x = track_rect.left() + track_rect.width() * vis_t;
        let base_y = track_rect.center().y;
        let dir = if base_y + 34.0 <= screen_rect.max.y { 1.0 } else { -1.0 };
        let dc = Pos2::new(thumb_x, base_y + 17.5 * pop_t * dir);
        let tip = Pos2::new(thumb_x, base_y + 1.0 * dir);
        draw_teardrop(&painter, dc, drop_r, tip, theme.accent, theme.surface);
        if pop_t > 0.05 {
            let half_h = (track_thick * 0.5 + 2.5) * pop_t;
            let bar = Rect::from_center_size(Pos2::new(thumb_x, base_y), Vec2::new(2.4, half_h * 2.0));
            painter.rect_filled(bar, 1.2, Color32::WHITE);
        }
        dc
    };

    if pop_t > 0.3 {
        painter.text(
            drop_center,
            egui::Align2::CENTER_CENTER,
            format!("{:.0}", *value),
            egui::FontId::proportional(11.0),
            theme.on_accent,
        );
    }

    let dragging = resp.dragged();
    let val_now = *value;
    resp.on_hover_ui(|ui| {
        ui.label(format!("{tooltip}: {val_now:.0} px"));
    });
    dragging
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn safe_clamp01_handles_nan_and_inf() {
        assert_eq!(safe_clamp01(f32::NAN), 0.0);
        assert_eq!(safe_clamp01(f32::INFINITY), 0.0);
        assert_eq!(safe_clamp01(f32::NEG_INFINITY), 0.0);
        assert_eq!(safe_clamp01(-0.5), 0.0);
        assert_eq!(safe_clamp01(2.5), 1.0);
        assert_eq!(safe_clamp01(0.42), 0.42);
    }

    #[test]
    fn theme_colors_and_stops_edge_cases() {
        assert_eq!(sample_stops(&COLOR_STOPS, 0.0), Color32::from_rgb(255, 255, 255));
        assert_eq!(sample_stops(&COLOR_STOPS, 1.0), Color32::from_rgb(18, 20, 26));
        assert_eq!(sample_stops(&COLOR_STOPS, f32::NAN), Color32::from_rgb(255, 255, 255));

        let mid = lerp_color(Color32::BLACK, Color32::WHITE, 0.5);
        assert!((mid.r() as i32 - 128).abs() <= 1);
        assert_eq!(lerp_color(Color32::RED, Color32::BLUE, f32::NAN), Color32::RED);

        // White accent hover must differ visibly from accent itself
        let white_theme = ThemeColors::from_positions(0.28, 0.0);
        assert_eq!(white_theme.accent, Color32::WHITE);
        assert_ne!(white_theme.accent_hover, Color32::WHITE);
        assert_eq!(white_theme.on_accent, Color32::from_rgb(8, 20, 30));

        // Dark accent uses white on_accent
        let dark_theme = ThemeColors::from_positions(0.28, 1.0);
        assert_eq!(dark_theme.on_accent, Color32::WHITE);
    }

    #[test]
    fn spring_step_remains_stable_even_with_tiny_response_or_nan() {
        let (x, v, settled) = spring_step(f32::NAN, f32::INFINITY, 1.0, 0.016, 0.2, 0.7);
        assert_eq!((x, v, settled), (1.0, 0.0, true));

        let mut cur_x = 0.0_f32;
        let mut cur_v = 0.0_f32;
        for _ in 0..120 {
            let (nx, nv, done) = spring_step(cur_x, cur_v, 1.0, 0.05, 0.01, 0.5);
            assert!(nx.is_finite() && nv.is_finite());
            assert!(nx.abs() < 2.0, "spring diverged: {nx}");
            cur_x = nx;
            cur_v = nv;
            if done {
                break;
            }
        }
        assert!((cur_x - 1.0).abs() < 0.01);
    }

    #[test]
    fn strong_text_blur_box_averages_and_handles_tiny_images() {
        let empty = RgbaImage::new(0, 0);
        let out_empty = make_strong_text_blur_image(&empty);
        assert_eq!(out_empty.size, [1, 1]);

        let tiny = RgbaImage::from_pixel(2, 2, Rgba([100, 120, 140, 255]));
        let out_tiny = make_strong_text_blur_image(&tiny);
        assert_eq!(out_tiny.size, [1, 1]);
        assert_eq!(out_tiny.pixels[0], Color32::from_rgb(100, 120, 140));

        // Checkerboard 1x1 pixels must average to gray (~100), not sample pure 0 or 200
        let mut checker = RgbaImage::new(16, 16);
        for y in 0..16 {
            for x in 0..16 {
                let c = if (x + y) % 2 == 0 { 0 } else { 200 };
                checker.put_pixel(x, y, Rgba([c, c, c, 255]));
            }
        }
        let blurred = make_strong_text_blur_image(&checker);
        assert_eq!(blurred.size, [4, 4]);
        let px = blurred.pixels[0];
        assert!((px.r() as i32 - 100).abs() <= 2, "expected ~100, got {px:?}");
    }
}
