use ab_glyph::{Font, PxScale, ScaleFont};
use egui::{Pos2, Rect};
use image::{Rgba, RgbaImage};

use crate::settings::TextFont;

use super::{
    arrow_head_len, callout_anchor, contrasting_fg, dash_params, ellipse_points, font_for,
    measure_line_width, rect_corner_radius, sample_dashed_polyline, smooth_chaikin,
    text_box_and_content_rects, to_rgba, Annotation,
};

/// Crop the captured screenshot to `selection` and rasterize all annotations in exact Z-order.
pub fn render_export(
    source: &RgbaImage,
    selection: Rect,
    scale_factor: f32,
    annotations: &[Annotation],
) -> RgbaImage {
    if source.width() == 0 || source.height() == 0 {
        return RgbaImage::new(1, 1);
    }
    let img_w = source.width() as i32;
    let img_h = source.height() as i32;

    let (x0, x1) = if img_w <= 1 {
        (0, 1)
    } else {
        let s = ((selection.min.x * scale_factor).round() as i32).clamp(0, img_w - 1);
        let e = ((selection.max.x * scale_factor).round() as i32).clamp(s + 1, img_w);
        (s, e)
    };
    let (y0, y1) = if img_h <= 1 {
        (0, 1)
    } else {
        let s = ((selection.min.y * scale_factor).round() as i32).clamp(0, img_h - 1);
        let e = ((selection.max.y * scale_factor).round() as i32).clamp(s + 1, img_h);
        (s, e)
    };

    let out_w = (x1 - x0) as u32;
    let out_h = (y1 - y0) as u32;

    let mut out = image::imageops::crop_imm(source, x0 as u32, y0 as u32, out_w, out_h).to_image();

    let to_px = |p: Pos2| -> (f32, f32) {
        (
            p.x * scale_factor - x0 as f32,
            p.y * scale_factor - y0 as f32,
        )
    };

    for ann in annotations {
        match ann {
            Annotation::Blur {
                rect,
                radius,
                pixelate,
            } => {
                let (bx0, by0) = to_px(rect.min);
                let (bx1, by1) = to_px(rect.max);
                if *pixelate {
                    let block = ((*radius * 1.4).clamp(6.0, 28.0) * scale_factor).round() as i32;
                    pixelate_region(&mut out, bx0 as i32, by0 as i32, bx1 as i32, by1 as i32, block);
                } else {
                    smooth_blur_region(
                        &mut out,
                        bx0 as i32,
                        by0 as i32,
                        bx1 as i32,
                        by1 as i32,
                        (*radius * scale_factor).round().clamp(3.0, 22.0) as i32,
                    );
                }
            }
            Annotation::Marker {
                points,
                color,
                thickness,
                dashed,
            } => {
                let r = (thickness * 3.8 * scale_factor * 0.5).max(3.0);
                let smoothed = smooth_chaikin(points);
                if *dashed {
                    let (dash_len, gap_len) = dash_params(*thickness * 3.8, true);
                    let dashes = sample_dashed_polyline(&smoothed, dash_len, gap_len);
                    raster_polylines_masked(&mut out, &dashes, &to_px, r, to_rgba(*color, 105), true);
                } else {
                    raster_polylines_masked(
                        &mut out,
                        &[smoothed],
                        &to_px,
                        r,
                        to_rgba(*color, 105),
                        false,
                    );
                }
            }
            Annotation::Pen {
                points,
                color,
                thickness,
                dashed,
            } => {
                let r = (thickness * scale_factor * 0.5).max(0.5);
                let rgba = to_rgba(*color, 255);
                let smoothed = smooth_chaikin(points);
                let polylines = if *dashed {
                    let (dash_len, gap_len) = dash_params(*thickness, false);
                    sample_dashed_polyline(&smoothed, dash_len, gap_len)
                } else {
                    vec![smoothed]
                };
                raster_opaque_polylines(&mut out, &polylines, &to_px, r, rgba);
            }
            Annotation::Line {
                start,
                end,
                color,
                thickness,
                dashed,
            } => {
                let r = (thickness * scale_factor * 0.5).max(0.5);
                let rgba = to_rgba(*color, 255);
                let polylines = if *dashed {
                    let (dash_len, gap_len) = dash_params(*thickness, false);
                    sample_dashed_polyline(&[*start, *end], dash_len, gap_len)
                } else {
                    vec![vec![*start, *end]]
                };
                raster_opaque_polylines(&mut out, &polylines, &to_px, r, rgba);
            }
            Annotation::Arrow {
                start,
                end,
                color,
                thickness,
                dashed,
            } => {
                raster_arrow(
                    &mut out,
                    *start,
                    *end,
                    &to_px,
                    thickness * scale_factor,
                    to_rgba(*color, 255),
                    *dashed,
                    *thickness,
                    scale_factor,
                );
            }
            Annotation::Text {
                pos,
                text,
                color,
                size,
                arrow_to,
                thickness,
                placement,
                font,
                box_size,
            } => {
                let (box_r, text_only_r, wrapped_lines, content_sz) =
                    text_box_and_content_rects(*pos, text, *size, *placement, *font, *box_size);
                let bg = text_only_r.expand(6.0);
                let (bx0, by0) = to_px(bg.min);
                let (bx1, by1) = to_px(bg.max);
                smooth_blur_rounded_region(
                    &mut out,
                    bx0.floor() as i32,
                    by0.floor() as i32,
                    bx1.ceil() as i32,
                    by1.ceil() as i32,
                    ((*thickness * 1.1 + 10.0).clamp(12.0, 28.0) * scale_factor).round() as i32,
                    8.0 * scale_factor,
                );

                if let Some(target) = arrow_to {
                    let start = callout_anchor(text_only_r, *target);
                    raster_arrow(
                        &mut out,
                        start,
                        *target,
                        &to_px,
                        (*thickness * 0.85).max(2.0) * scale_factor,
                        to_rgba(*color, 255),
                        false,
                        (*thickness * 0.85).max(2.0),
                        scale_factor,
                    );
                }
                let h_frac = placement.h_align_frac();
                let v_frac = placement.v_align_frac();
                let text_top_logical =
                    box_r.top() + (box_r.height() - content_sz.y).max(0.0) * v_frac;
                let line_step_logical = content_sz.y / wrapped_lines.len().max(1) as f32;
                let shadow_px = scale_factor.max(1.0);
                for (li, line) in wrapped_lines.iter().enumerate() {
                    let lw = measure_line_width(line, *size, *font);
                    let lx = box_r.left() + (box_r.width() - lw).max(0.0) * h_frac;
                    let ly = text_top_logical + li as f32 * line_step_logical;
                    let (tx, ty) = to_px(Pos2::new(lx, ly));
                    raster_text(
                        &mut out,
                        tx + shadow_px,
                        ty + shadow_px,
                        line,
                        size * scale_factor,
                        Rgba([0, 0, 0, 110]),
                        *font,
                    );
                    raster_text(
                        &mut out,
                        tx,
                        ty,
                        line,
                        size * scale_factor,
                        to_rgba(*color, 255),
                        *font,
                    );
                }
            }
            Annotation::Ellipse {
                rect,
                color,
                thickness,
                filled,
            } => {
                if *filled {
                    let (rx0, ry0) = to_px(rect.min);
                    let (rx1, ry1) = to_px(rect.max);
                    raster_filled_ellipse(&mut out, rx0, ry0, rx1, ry1, to_rgba(*color, 255));
                } else {
                    let mut pts = ellipse_points(*rect, 96);
                    if let Some(&first) = pts.first() {
                        pts.push(first);
                    }
                    let r = (thickness * scale_factor * 0.5).max(0.5);
                    raster_opaque_polylines(&mut out, &[pts], &to_px, r, to_rgba(*color, 255));
                }
            }
            Annotation::Rect {
                rect,
                color,
                thickness,
                filled,
            } => {
                let (rx0, ry0) = to_px(rect.min);
                let (rx1, ry1) = to_px(rect.max);
                let corner_px =
                    rect_corner_radius(*thickness, rect.width(), rect.height()) * scale_factor;
                if *filled {
                    raster_filled_rounded_rect(
                        &mut out,
                        rx0,
                        ry0,
                        rx1,
                        ry1,
                        corner_px,
                        to_rgba(*color, 255),
                    );
                } else {
                    raster_rect(
                        &mut out,
                        rx0,
                        ry0,
                        rx1,
                        ry1,
                        thickness * scale_factor,
                        corner_px,
                        to_rgba(*color, 255),
                    );
                }
            }
            Annotation::Step {
                pos,
                number,
                color,
                radius,
            } => {
                let (cx, cy) = to_px(*pos);
                let rad = *radius * scale_factor;
                raster_disc(
                    &mut out,
                    cx,
                    cy,
                    rad + 0.75 * scale_factor,
                    Rgba([255, 255, 255, 255]),
                );
                raster_disc(&mut out, cx, cy, rad, to_rgba(*color, 255));
                let fg = to_rgba(contrasting_fg(*color), 255);
                let font_px = (*radius * 1.05).max(11.0) * scale_factor;
                raster_number(&mut out, cx, cy, *number, font_px, fg);
            }
        }
    }

    out
}

fn raster_opaque_polylines(
    img: &mut RgbaImage,
    polylines: &[Vec<Pos2>],
    to_px: &impl Fn(Pos2) -> (f32, f32),
    radius: f32,
    rgba: Rgba<u8>,
) {
    raster_polylines_masked(img, polylines, to_px, radius, rgba, false);
}

fn raster_text(
    img: &mut RgbaImage,
    x: f32,
    y: f32,
    text: &str,
    px_size: f32,
    color: Rgba<u8>,
    tf: TextFont,
) {
    let Some(font) = font_for(tf) else { return };
    let scale = PxScale::from(px_size * 1.15);
    let scaled = font.as_scaled(scale);
    let line_height = scaled.height() + scaled.line_gap();
    let ascent = scaled.ascent();

    let w = img.width() as i32;
    let h = img.height() as i32;

    for (line_idx, line) in text.lines().enumerate() {
        let mut cursor_x = x;
        let baseline_y = y + ascent + line_idx as f32 * line_height;
        let mut prev_id = None;
        for ch in line.chars() {
            let gid = font.glyph_id(ch);
            if let Some(prev) = prev_id {
                cursor_x += scaled.kern(prev, gid);
            }
            let glyph = gid.with_scale_and_position(scale, ab_glyph::point(cursor_x, baseline_y));
            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|gx, gy, cov| {
                    let px = bounds.min.x as i32 + gx as i32;
                    let py = bounds.min.y as i32 + gy as i32;
                    if px >= 0 && px < w && py >= 0 && py < h && cov > 0.02 {
                        blend_pixel(img, px as u32, py as u32, color, cov);
                    }
                });
            }
            cursor_x += scaled.h_advance(gid);
            prev_id = Some(gid);
        }
    }
}

fn blend_pixel(img: &mut RgbaImage, x: u32, y: u32, src: Rgba<u8>, coverage: f32) {
    let a = ((src[3] as f32 / 255.0) * coverage).clamp(0.0, 1.0);
    let inv = 1.0 - a;
    let dst = img.get_pixel_mut(x, y);
    dst[0] = (src[0] as f32 * a + dst[0] as f32 * inv).round() as u8;
    dst[1] = (src[1] as f32 * a + dst[1] as f32 * inv).round() as u8;
    dst[2] = (src[2] as f32 * a + dst[2] as f32 * inv).round() as u8;
    dst[3] = 255;
}

fn raster_polylines_masked(
    img: &mut RgbaImage,
    polylines: &[Vec<Pos2>],
    to_px: &impl Fn(Pos2) -> (f32, f32),
    radius: f32,
    color: Rgba<u8>,
    flat_caps: bool,
) {
    let px_lines: Vec<Vec<(f32, f32)>> = polylines
        .iter()
        .filter(|pts| !pts.is_empty())
        .map(|pts| pts.iter().map(|&p| to_px(p)).collect())
        .collect();
    raster_px_polylines_masked(img, &px_lines, radius, color, flat_caps);
}

/// Unified single-pass anti-aliased coverage-mask polyline rasterizer (`O(Segments * Length)`).
/// Guarantees uniform alpha without double-darkened seams and supports flat butt caps for dashed Marker.
fn raster_px_polylines_masked(
    img: &mut RgbaImage,
    px_polylines: &[Vec<(f32, f32)>],
    radius: f32,
    color: Rgba<u8>,
    flat_caps: bool,
) {
    if px_polylines.is_empty() || img.width() == 0 || img.height() == 0 {
        return;
    }
    let w = img.width() as i32;
    let h = img.height() as i32;
    let mut min_x = w;
    let mut max_x = -1;
    let mut min_y = h;
    let mut max_y = -1;

    let pad_f = radius + 2.0;
    let mut all_px: Vec<Vec<(f32, f32)>> = Vec::with_capacity(px_polylines.len());
    for pts in px_polylines {
        if pts.is_empty() {
            continue;
        }
        let mut clean: Vec<(f32, f32)> = Vec::with_capacity(pts.len());
        for &p in pts {
            if clean
                .last()
                .is_none_or(|&(lx, ly)| (p.0 - lx).hypot(p.1 - ly) >= 1e-3)
            {
                clean.push(p);
            }
        }
        for &(px, py) in &clean {
            min_x = min_x.min((px - pad_f).floor() as i32);
            max_x = max_x.max((px + pad_f).ceil() as i32);
            min_y = min_y.min((py - pad_f).floor() as i32);
            max_y = max_y.max((py + pad_f).ceil() as i32);
        }
        all_px.push(clean);
    }

    min_x = min_x.clamp(0, w - 1);
    max_x = max_x.clamp(0, w - 1);
    min_y = min_y.clamp(0, h - 1);
    max_y = max_y.clamp(0, h - 1);
    if max_x < min_x || max_y < min_y || all_px.is_empty() {
        return;
    }

    let mask_w = (max_x - min_x + 1) as usize;
    let mask_h = (max_y - min_y + 1) as usize;
    let mut mask = vec![0u8; mask_w * mask_h];
    let outer = radius + 0.5;
    let outer2 = outer * outer;
    let pad = (radius + 1.5).ceil() as i32;

    let stamp_seg = |mask: &mut [u8],
                     ax: f32,
                     ay: f32,
                     bx: f32,
                     by: f32,
                     cut_start: bool,
                     cut_end: bool| {
        let sx0 = ((ax.min(bx) as i32) - pad).clamp(min_x, max_x);
        let sx1 = ((ax.max(bx) as i32) + pad).clamp(min_x, max_x);
        let sy0 = ((ay.min(by) as i32) - pad).clamp(min_y, max_y);
        let sy1 = ((ay.max(by) as i32) + pad).clamp(min_y, max_y);
        let abx = bx - ax;
        let aby = by - ay;
        let len_sq = abx * abx + aby * aby;

        for y in sy0..=sy1 {
            let row_off = (y - min_y) as usize * mask_w;
            let fy = y as f32 + 0.5;
            for x in sx0..=sx1 {
                let fx = x as f32 + 0.5;
                let t = if len_sq < 1e-4 {
                    0.0
                } else {
                    let raw_t = ((fx - ax) * abx + (fy - ay) * aby) / len_sq;
                    if (cut_start && raw_t < 0.0) || (cut_end && raw_t > 1.0) {
                        continue;
                    }
                    raw_t.clamp(0.0, 1.0)
                };
                let dx = fx - (ax + abx * t);
                let dy = fy - (ay + aby * t);
                let d2 = dx * dx + dy * dy;
                if d2 <= outer2 {
                    let cov = ((outer - d2.sqrt()).clamp(0.0, 1.0) * 255.0).round() as u8;
                    let idx = row_off + (x - min_x) as usize;
                    if cov > mask[idx] {
                        mask[idx] = cov;
                    }
                }
            }
        }
    };

    for px_pts in &all_px {
        if px_pts.len() == 1 {
            stamp_seg(
                &mut mask,
                px_pts[0].0,
                px_pts[0].1,
                px_pts[0].0,
                px_pts[0].1,
                false,
                false,
            );
        } else {
            let last_seg = px_pts.len() - 2;
            for (seg_idx, seg) in px_pts.windows(2).enumerate() {
                stamp_seg(
                    &mut mask,
                    seg[0].0,
                    seg[0].1,
                    seg[1].0,
                    seg[1].1,
                    flat_caps && seg_idx == 0,
                    flat_caps && seg_idx == last_seg,
                );
            }
        }
    }

    for my in 0..mask_h {
        let row_off = my * mask_w;
        let iy = (min_y + my as i32) as u32;
        for mx in 0..mask_w {
            let cov = mask[row_off + mx];
            if cov > 0 {
                blend_pixel(img, (min_x + mx as i32) as u32, iy, color, f32::from(cov) / 255.0);
            }
        }
    }
}

/// Bake a real live blur or pixelate patch for `blur_rect` including the screen AND all earlier annotations.
pub fn bake_blur_patch(
    virtual_source: &RgbaImage,
    blur_rect: Rect,
    scale_factor: f32,
    blur_radius: f32,
    pixelate: bool,
    earlier: &[Annotation],
) -> egui::ColorImage {
    let mut img = render_export(virtual_source, blur_rect, scale_factor, earlier);
    let w = img.width() as i32;
    let h = img.height() as i32;
    if pixelate {
        let block = ((blur_radius * 1.4).clamp(6.0, 28.0) * scale_factor).round() as i32;
        pixelate_region(&mut img, 0, 0, w, h, block);
    } else {
        smooth_blur_region(
            &mut img,
            0,
            0,
            w,
            h,
            (blur_radius * scale_factor).round().clamp(3.0, 22.0) as i32,
        );
    }
    egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw())
}

/// Irreversible mosaic pixelation over `[x0..x1, y0..y1]`.
pub fn pixelate_region(
    img: &mut RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    block_size: i32,
) {
    let w = img.width() as i32;
    let h = img.height() as i32;
    if w == 0 || h == 0 {
        return;
    }
    let x0 = x0.clamp(0, w);
    let y0 = y0.clamp(0, h);
    let x1 = x1.clamp(0, w);
    let y1 = y1.clamp(0, h);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let bs = block_size.clamp(4, 64);

    let mut by = y0;
    while by < y1 {
        let ey = (by + bs).min(y1);
        let mut bx = x0;
        while bx < x1 {
            let ex = (bx + bs).min(x1);
            let mut rs = 0u64;
            let mut gs = 0u64;
            let mut bs_sum = 0u64;
            let mut cnt = 0u64;
            for py in by..ey {
                for px in bx..ex {
                    let p = img.get_pixel(px as u32, py as u32);
                    rs += p[0] as u64;
                    gs += p[1] as u64;
                    bs_sum += p[2] as u64;
                    cnt += 1;
                }
            }
            if let (Some(r), Some(g), Some(b)) =
                (rs.checked_div(cnt), gs.checked_div(cnt), bs_sum.checked_div(cnt))
            {
                let col = Rgba([r as u8, g as u8, b as u8, 255]);
                for py in by..ey {
                    for px in bx..ex {
                        img.put_pixel(px as u32, py as u32, col);
                    }
                }
            }
            bx = ex;
        }
        by = ey;
    }
}

fn rounded_box_sdf(
    fx: f32,
    fy: f32,
    lx: f32,
    ty: f32,
    rx: f32,
    by: f32,
    corner_radius: f32,
) -> f32 {
    let hx = ((rx - lx) * 0.5).max(0.5);
    let hy = ((by - ty) * 0.5).max(0.5);
    let cr = corner_radius.clamp(0.0, hx.min(hy));
    let qx = (fx - (lx + rx) * 0.5).abs() - (hx - cr);
    let qy = (fy - (ty + by) * 0.5).abs() - (hy - cr);
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - cr
}

/// Shared O(W * H) separable box blur over a linearized `[f32; 3]` RGB buffer.
pub fn separable_box_blur_rgb(
    buf: &mut [[f32; 3]],
    w: usize,
    h: usize,
    radius: usize,
    passes: usize,
) {
    if w == 0 || h == 0 || radius == 0 || passes == 0 || buf.len() < w * h {
        return;
    }
    let mut tmp = vec![[0f32; 3]; w * h];
    for _ in 0..passes {
        for y in 0..h {
            let row = y * w;
            let mut rs = 0.0;
            let mut gs = 0.0;
            let mut bs = 0.0;
            let mut cnt = 0.0;
            for nx in 0..=radius.min(w - 1) {
                let c = buf[row + nx];
                rs += c[0];
                gs += c[1];
                bs += c[2];
                cnt += 1.0;
            }
            for x in 0..w {
                tmp[row + x] = [rs / cnt, gs / cnt, bs / cnt];
                let add_x = x + radius + 1;
                if add_x < w {
                    let c = buf[row + add_x];
                    rs += c[0];
                    gs += c[1];
                    bs += c[2];
                    cnt += 1.0;
                }
                if x >= radius {
                    let c = buf[row + x - radius];
                    rs -= c[0];
                    gs -= c[1];
                    bs -= c[2];
                    cnt -= 1.0;
                }
            }
        }

        for x in 0..w {
            let mut rs = 0.0;
            let mut gs = 0.0;
            let mut bs = 0.0;
            let mut cnt = 0.0;
            for ny in 0..=radius.min(h - 1) {
                let c = tmp[ny * w + x];
                rs += c[0];
                gs += c[1];
                bs += c[2];
                cnt += 1.0;
            }
            for y in 0..h {
                buf[y * w + x] = [rs / cnt, gs / cnt, bs / cnt];
                let add_y = y + radius + 1;
                if add_y < h {
                    let c = tmp[add_y * w + x];
                    rs += c[0];
                    gs += c[1];
                    bs += c[2];
                    cnt += 1.0;
                }
                if y >= radius {
                    let c = tmp[(y - radius) * w + x];
                    rs -= c[0];
                    gs -= c[1];
                    bs -= c[2];
                    cnt -= 1.0;
                }
            }
        }
    }
}

/// 2-pass separable Gaussian-approximating blur over `[x0..x1, y0..y1]` in `O(W * H)`.
pub fn smooth_blur_region(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, radius: i32) {
    smooth_blur_rounded_region(img, x0, y0, x1, y1, radius, 0.0);
}

/// 2-pass separable blur with optional rounded-corner masking so backdrop corners match GPU pills.
pub fn smooth_blur_rounded_region(
    img: &mut RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    radius: i32,
    corner_radius: f32,
) {
    let w = img.width() as i32;
    let h = img.height() as i32;
    if w == 0 || h == 0 {
        return;
    }
    let x0 = x0.clamp(0, w);
    let y0 = y0.clamp(0, h);
    let x1 = x1.clamp(0, w);
    let y1 = y1.clamp(0, h);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let rw = (x1 - x0) as usize;
    let rh = (y1 - y0) as usize;
    if rw < 2 || rh < 2 {
        return;
    }

    let mut buf = vec![[0f32; 3]; rw * rh];
    for y in 0..rh {
        for x in 0..rw {
            let p = img.get_pixel((x0 + x as i32) as u32, (y0 + y as i32) as u32);
            buf[y * rw + x] = [p[0] as f32, p[1] as f32, p[2] as f32];
        }
    }

    let r = radius.clamp(2, 22) as usize;
    separable_box_blur_rgb(&mut buf, rw, rh, r, 2);

    let cr = corner_radius.max(0.0);
    for y in 0..rh {
        for x in 0..rw {
            let c = buf[y * rw + x];
            let ix = (x0 + x as i32) as u32;
            let iy = (y0 + y as i32) as u32;
            if cr <= 0.0 {
                img.put_pixel(
                    ix,
                    iy,
                    Rgba([
                        c[0].round().clamp(0.0, 255.0) as u8,
                        c[1].round().clamp(0.0, 255.0) as u8,
                        c[2].round().clamp(0.0, 255.0) as u8,
                        255,
                    ]),
                );
            } else {
                let sdf = rounded_box_sdf(
                    x as f32 + 0.5,
                    y as f32 + 0.5,
                    0.0,
                    0.0,
                    rw as f32,
                    rh as f32,
                    cr,
                );
                let cov = (0.5 - sdf).clamp(0.0, 1.0);
                if cov > 0.0 {
                    let orig = *img.get_pixel(ix, iy);
                    let inv = 1.0 - cov;
                    img.put_pixel(
                        ix,
                        iy,
                        Rgba([
                            (c[0] * cov + orig[0] as f32 * inv).round().clamp(0.0, 255.0) as u8,
                            (c[1] * cov + orig[1] as f32 * inv).round().clamp(0.0, 255.0) as u8,
                            (c[2] * cov + orig[2] as f32 * inv).round().clamp(0.0, 255.0) as u8,
                            orig[3],
                        ]),
                    );
                }
            }
        }
    }
}

fn raster_disc(img: &mut RgbaImage, cx: f32, cy: f32, radius: f32, color: Rgba<u8>) {
    let w = img.width() as i32;
    let h = img.height() as i32;
    if w == 0 || h == 0 {
        return;
    }
    let outer = radius + 0.5;
    let outer2 = outer * outer;
    let min_x = ((cx - radius - 1.5).floor() as i32).clamp(0, w - 1);
    let max_x = ((cx + radius + 1.5).ceil() as i32).clamp(0, w - 1);
    let min_y = ((cy - radius - 1.5).floor() as i32).clamp(0, h - 1);
    let max_y = ((cy + radius + 1.5).ceil() as i32).clamp(0, h - 1);

    for y in min_y..=max_y {
        let dy = y as f32 + 0.5 - cy;
        for x in min_x..=max_x {
            let dx = x as f32 + 0.5 - cx;
            let d2 = dx * dx + dy * dy;
            if d2 <= outer2 {
                let cov = (outer - d2.sqrt()).clamp(0.0, 1.0);
                if cov > 0.0 {
                    blend_pixel(img, x as u32, y as u32, color, cov);
                }
            }
        }
    }
}

fn raster_thick_line(
    img: &mut RgbaImage,
    a: (f32, f32),
    b: (f32, f32),
    radius: f32,
    color: Rgba<u8>,
) {
    let w = img.width() as i32;
    let h = img.height() as i32;
    if w == 0 || h == 0 {
        return;
    }
    let outer = radius + 0.5;
    let outer2 = outer * outer;
    let pad = (radius + 1.5).ceil() as i32;
    let min_x = ((a.0.min(b.0).floor() as i32) - pad).clamp(0, w - 1);
    let max_x = ((a.0.max(b.0).ceil() as i32) + pad).clamp(0, w - 1);
    let min_y = ((a.1.min(b.1).floor() as i32) - pad).clamp(0, h - 1);
    let max_y = ((a.1.max(b.1).ceil() as i32) + pad).clamp(0, h - 1);

    let abx = b.0 - a.0;
    let aby = b.1 - a.1;
    let len_sq = abx * abx + aby * aby;

    for y in min_y..=max_y {
        let fy = y as f32 + 0.5;
        for x in min_x..=max_x {
            let fx = x as f32 + 0.5;
            let t = if len_sq < 1e-4 {
                0.0
            } else {
                (((fx - a.0) * abx + (fy - a.1) * aby) / len_sq).clamp(0.0, 1.0)
            };
            let dx = fx - (a.0 + abx * t);
            let dy = fy - (a.1 + aby * t);
            let d2 = dx * dx + dy * dy;
            if d2 <= outer2 {
                let cov = (outer - d2.sqrt()).clamp(0.0, 1.0);
                if cov > 0.0 {
                    blend_pixel(img, x as u32, y as u32, color, cov);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn raster_rect(
    img: &mut RgbaImage,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    thickness: f32,
    corner_radius_px: f32,
    color: Rgba<u8>,
) {
    let r = (thickness * 0.5).max(0.5);
    let w = (x1 - x0).abs();
    let h = (y1 - y0).abs();
    let cr = corner_radius_px.clamp(0.0, (w * 0.45).min(h * 0.45));
    let lx = x0.min(x1);
    let rx = x0.max(x1);
    let ty = y0.min(y1);
    let by = y0.max(y1);

    let mut pts = Vec::with_capacity(48);
    let corners = [
        (rx - cr, ty + cr, -std::f32::consts::FRAC_PI_2, 0.0),
        (rx - cr, by - cr, 0.0, std::f32::consts::FRAC_PI_2),
        (lx + cr, by - cr, std::f32::consts::FRAC_PI_2, std::f32::consts::PI),
        (lx + cr, ty + cr, std::f32::consts::PI, std::f32::consts::PI * 1.5),
    ];
    let steps = 10usize;
    for (cx, cy, a0, a1) in corners {
        for i in 0..=steps {
            let t = a0 + (a1 - a0) * (i as f32 / steps as f32);
            pts.push((cx + cr * t.cos(), cy + cr * t.sin()));
        }
    }
    if let Some(&first) = pts.first() {
        pts.push(first);
    }
    raster_px_polylines_masked(img, &[pts], r, color, false);
}

fn raster_filled_rounded_rect(
    img: &mut RgbaImage,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    corner_radius_px: f32,
    color: Rgba<u8>,
) {
    let iw = img.width() as i32;
    let ih = img.height() as i32;
    if iw == 0 || ih == 0 {
        return;
    }
    let lx = x0.min(x1);
    let rx = x0.max(x1);
    let ty = y0.min(y1);
    let by = y0.max(y1);

    let min_x = (lx.floor() as i32 - 1).clamp(0, iw - 1);
    let max_x = (rx.ceil() as i32 + 1).clamp(0, iw - 1);
    let min_y = (ty.floor() as i32 - 1).clamp(0, ih - 1);
    let max_y = (by.ceil() as i32 + 1).clamp(0, ih - 1);

    for y in min_y..=max_y {
        let fy = y as f32 + 0.5;
        for x in min_x..=max_x {
            let fx = x as f32 + 0.5;
            let sdf = rounded_box_sdf(fx, fy, lx, ty, rx, by, corner_radius_px);
            let cov = (0.5 - sdf).clamp(0.0, 1.0);
            if cov > 0.0 {
                blend_pixel(img, x as u32, y as u32, color, cov);
            }
        }
    }
}

fn raster_filled_ellipse(
    img: &mut RgbaImage,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    color: Rgba<u8>,
) {
    let iw = img.width() as i32;
    let ih = img.height() as i32;
    if iw == 0 || ih == 0 {
        return;
    }
    let lx = x0.min(x1);
    let rx = x0.max(x1);
    let ty = y0.min(y1);
    let by = y0.max(y1);
    let cx = (lx + rx) * 0.5;
    let cy = (ty + by) * 0.5;
    let rx_rad = ((rx - lx) * 0.5).max(1.0);
    let ry_rad = ((by - ty) * 0.5).max(1.0);
    let inv_rx2 = 1.0 / (rx_rad * rx_rad);
    let inv_ry2 = 1.0 / (ry_rad * ry_rad);

    let min_x = (lx.floor() as i32 - 1).clamp(0, iw - 1);
    let max_x = (rx.ceil() as i32 + 1).clamp(0, iw - 1);
    let min_y = (ty.floor() as i32 - 1).clamp(0, ih - 1);
    let max_y = (by.ceil() as i32 + 1).clamp(0, ih - 1);

    for y in min_y..=max_y {
        let dy = y as f32 + 0.5 - cy;
        for x in min_x..=max_x {
            let dx = x as f32 + 0.5 - cx;
            let f_val = dx * dx * inv_rx2 + dy * dy * inv_ry2 - 1.0;
            let grad = (dx * inv_rx2).hypot(dy * inv_ry2);
            let sdf_px = if grad < 1e-6 {
                -rx_rad.min(ry_rad)
            } else {
                f_val / (2.0 * grad)
            };
            let cov = (0.5 - sdf_px).clamp(0.0, 1.0);
            if cov > 0.0 {
                blend_pixel(img, x as u32, y as u32, color, cov);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn raster_arrow(
    img: &mut RgbaImage,
    start_logical: Pos2,
    end_logical: Pos2,
    to_px: &impl Fn(Pos2) -> (f32, f32),
    px_thickness: f32,
    color: Rgba<u8>,
    dashed: bool,
    logical_thickness: f32,
    scale_factor: f32,
) {
    let start = to_px(start_logical);
    let end = to_px(end_logical);
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 2.0 {
        return;
    }
    let ux = dx / len;
    let uy = dy / len;
    let px = -uy;
    let py = ux;

    let head_len = arrow_head_len(px_thickness, scale_factor, len);
    let head_half_w = head_len * 0.62;
    let r = (px_thickness * 0.5).max(0.5);

    let wing_l = (
        end.0 - ux * head_len + px * head_half_w,
        end.1 - uy * head_len + py * head_half_w,
    );
    let wing_r = (
        end.0 - ux * head_len - px * head_half_w,
        end.1 - uy * head_len - py * head_half_w,
    );

    if dashed {
        let (dash_len, gap_len) = dash_params(logical_thickness, false);
        let dashes = sample_dashed_polyline(&[start_logical, end_logical], dash_len, gap_len);
        raster_opaque_polylines(img, &dashes, to_px, r, color);
    } else {
        raster_thick_line(img, start, end, r, color);
    }
    raster_thick_line(img, end, wing_l, r, color);
    raster_thick_line(img, end, wing_r, r, color);
}

/// Crisp vector glyph rasterizer for step numbers using embedded Roboto via `ab_glyph`.
fn raster_number(
    img: &mut RgbaImage,
    cx: f32,
    cy: f32,
    number: u32,
    font_px: f32,
    color: Rgba<u8>,
) {
    let Some(font) = font_for(TextFont::Roboto) else { return };
    let text = number.to_string();
    let scale = PxScale::from(font_px.max(10.0));
    let scaled = font.as_scaled(scale);

    let mut total_advance = 0.0_f32;
    let mut prev_id = None;
    for ch in text.chars() {
        let gid = font.glyph_id(ch);
        if let Some(prev) = prev_id {
            total_advance += scaled.kern(prev, gid);
        }
        total_advance += scaled.h_advance(gid);
        prev_id = Some(gid);
    }

    let mut glyphs = Vec::with_capacity(text.len());
    let mut cursor_x = cx - total_advance * 0.5;
    let baseline_y = cy + (scaled.ascent() + scaled.descent()) * 0.5;
    let mut prev_id = None;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for ch in text.chars() {
        let gid = font.glyph_id(ch);
        if let Some(prev) = prev_id {
            cursor_x += scaled.kern(prev, gid);
        }
        let glyph = gid.with_scale_and_position(scale, ab_glyph::point(cursor_x, baseline_y));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let b = outlined.px_bounds();
            min_y = min_y.min(b.min.y);
            max_y = max_y.max(b.max.y);
            glyphs.push(outlined);
        }
        cursor_x += scaled.h_advance(gid);
        prev_id = Some(gid);
    }

    let y_shift = if min_y.is_finite() && max_y.is_finite() {
        cy - (min_y + max_y) * 0.5
    } else {
        0.0
    };

    let w = img.width() as i32;
    let h = img.height() as i32;
    for outlined in glyphs {
        let bounds = outlined.px_bounds();
        outlined.draw(|gx, gy, cov| {
            let px = bounds.min.x.round() as i32 + gx as i32;
            let py = (bounds.min.y + y_shift).round() as i32 + gy as i32;
            if px >= 0 && px < w && py >= 0 && py < h && cov > 0.02 {
                blend_pixel(img, px as u32, py as u32, color, cov);
            }
        });
    }
}
