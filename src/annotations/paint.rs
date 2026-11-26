use std::collections::HashMap;

use egui::{Color32, Pos2, Rect, Stroke, TextureId, Vec2};

use crate::material::draw_rounded_texture_uv;

use super::{
    callout_anchor, compute_arrow_geometry, contrasting_fg, dash_params, ellipse_points,
    rect_corner_radius, sample_dashed_polyline, smooth_chaikin, text_box_and_content_rects,
    Annotation, TextPlacement, MARKER_ALPHA, MARKER_WIDTH_MULT, TEXT_BG_EXPAND, TEXT_BG_RADIUS,
};

fn paint_polyline_group(
    painter: &egui::Painter,
    polylines: Vec<Vec<Pos2>>,
    map_pt: &impl Fn(Pos2) -> Pos2,
    width: f32,
    color: Color32,
    round_caps: bool,
) {
    let stroke = Stroke::new(width, color);
    let r = width * 0.5;
    for sub in polylines {
        let mapped: Vec<Pos2> = sub.into_iter().map(map_pt).collect();
        if mapped.len() >= 2 {
            let first = mapped[0];
            let last = mapped[mapped.len() - 1];
            painter.add(egui::Shape::line(mapped, stroke));
            if round_caps {
                painter.circle_filled(first, r, color);
                painter.circle_filled(last, r, color);
            }
        } else if let Some(&p) = mapped.first() {
            painter.circle_filled(p, r, color);
        }
    }
}

/// Draw all annotations in exact creation Z-order onto a monitor pane.
/// `offset` shifts global virtual coordinates by `-mon_offset`; `view_scale` adapts mixed-DPI panes.
#[allow(clippy::too_many_arguments)]
pub fn paint_annotations(
    painter: &egui::Painter,
    annotations: &[Annotation],
    selected_idx: Option<usize>,
    defer_blur_idx: Option<usize>,
    offset: Vec2,
    view_scale: f32,
    blur_patches: &HashMap<usize, TextureId>,
    text_blur_tex: Option<TextureId>,
    virtual_size: Vec2,
    accent: Color32,
) {
    let map_pt = |p: Pos2| -> Pos2 {
        let s = p + offset;
        Pos2::new(s.x * view_scale, s.y * view_scale)
    };
    let map_rect = |r: Rect| -> Rect { Rect::from_min_max(map_pt(r.min), map_pt(r.max)) };

    for (idx, ann) in annotations.iter().enumerate() {
        let skip_baked = defer_blur_idx == Some(idx);
        match ann {
            Annotation::Blur { rect, pixelate, .. } => {
                let local_rect = map_rect(*rect);
                if !skip_baked
                    && let Some(&patch_tex) = blur_patches.get(&idx)
                {
                    painter.image(
                        patch_tex,
                        local_rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    continue;
                }
                if let Some(tex) = text_blur_tex {
                    let uv = Rect::from_min_max(
                        Pos2::new(
                            (rect.min.x / virtual_size.x.max(1.0)).clamp(0.0, 1.0),
                            (rect.min.y / virtual_size.y.max(1.0)).clamp(0.0, 1.0),
                        ),
                        Pos2::new(
                            (rect.max.x / virtual_size.x.max(1.0)).clamp(0.0, 1.0),
                            (rect.max.y / virtual_size.y.max(1.0)).clamp(0.0, 1.0),
                        ),
                    );
                    painter.image(
                        tex,
                        local_rect,
                        uv,
                        Color32::from_rgba_unmultiplied(255, 255, 255, 220),
                    );
                } else {
                    if *pixelate {
                        painter.rect_filled(
                            local_rect,
                            4.0,
                            Color32::from_rgba_unmultiplied(16, 22, 36, 130),
                        );
                    }
                    painter.rect_stroke(
                        local_rect,
                        4.0,
                        Stroke::new(1.2_f32, Color32::from_rgba_unmultiplied(94, 234, 212, 185)),
                        egui::StrokeKind::Outside,
                    );
                }
            }
            Annotation::Marker {
                points,
                color,
                thickness,
                dashed,
            } => {
                let marker_col = Color32::from_rgba_unmultiplied(
                    color.r(),
                    color.g(),
                    color.b(),
                    MARKER_ALPHA,
                );
                let width = *thickness * MARKER_WIDTH_MULT * view_scale;
                let smoothed = smooth_chaikin(points);
                let polylines = if *dashed {
                    let (dash_len, gap_len) = dash_params(*thickness * MARKER_WIDTH_MULT, true);
                    sample_dashed_polyline(&smoothed, dash_len, gap_len)
                } else {
                    vec![smoothed]
                };
                // Omit round caps on semi-transparent markers so caps never double-darken
                paint_polyline_group(painter, polylines, &map_pt, width, marker_col, false);
            }
            Annotation::Pen {
                points,
                color,
                thickness,
                dashed,
            } => {
                let lw = *thickness * view_scale;
                let smoothed = smooth_chaikin(points);
                let polylines = if *dashed {
                    let (dash_len, gap_len) = dash_params(*thickness, false);
                    sample_dashed_polyline(&smoothed, dash_len, gap_len)
                } else {
                    vec![smoothed]
                };
                paint_polyline_group(painter, polylines, &map_pt, lw, *color, true);
            }
            Annotation::Line {
                start,
                end,
                color,
                thickness,
                dashed,
            } => {
                let lw = *thickness * view_scale;
                let polylines = if *dashed {
                    let (dash_len, gap_len) = dash_params(*thickness, false);
                    sample_dashed_polyline(&[*start, *end], dash_len, gap_len)
                } else {
                    vec![vec![*start, *end]]
                };
                paint_polyline_group(painter, polylines, &map_pt, lw, *color, true);
            }
            Annotation::Arrow {
                start,
                end,
                color,
                thickness,
                dashed,
            } => {
                if *dashed {
                    let (dash_len, gap_len) = dash_params(*thickness, false);
                    draw_egui_arrow_dashed(
                        painter,
                        *start,
                        *end,
                        &map_pt,
                        Stroke::new(*thickness * view_scale, *color),
                        view_scale,
                        (dash_len, gap_len),
                    );
                } else {
                    draw_egui_arrow(
                        painter,
                        map_pt(*start),
                        map_pt(*end),
                        Stroke::new(*thickness * view_scale, *color),
                        view_scale,
                    );
                }
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
                let font_id = egui::FontId::new(
                    *size * view_scale,
                    egui::FontFamily::Name(font.egui_family().into()),
                );
                let layout =
                    text_box_and_content_rects(*pos, text, *size, *placement, *font, *box_size);
                let box_r = layout.box_rect;
                let content_r = layout.content_rect;
                let wrapped_lines = layout.wrapped_lines;
                let measured_sz = layout.measured_size;
                let display_text = wrapped_lines.join("\n");
                let unscaled_font =
                    egui::FontId::new(*size, egui::FontFamily::Name(font.egui_family().into()));
                let galley = painter.layout_no_wrap(display_text, unscaled_font, *color);
                let actual_w = galley.size().x.max(measured_sz.x).max(20.0);
                let content_h = galley.size().y.max(measured_sz.y).max(*size * 1.25);

                let h_frac = placement.h_align_frac();
                let v_frac = placement.v_align_frac();
                let text_left = box_r.left() + (box_r.width() - actual_w).max(0.0) * h_frac;
                let text_top = box_r.top() + (box_r.height() - content_h).max(0.0) * v_frac;
                let galley_r = Rect::from_min_size(
                    Pos2::new(text_left, text_top),
                    Vec2::new(actual_w, content_h),
                );
                let baked_bg = content_r.expand(TEXT_BG_EXPAND);
                let bg_global = content_r.union(galley_r).expand(TEXT_BG_EXPAND);
                let bg_local = map_rect(bg_global);
                let patch_matches_size = (bg_global.width() - baked_bg.width()).abs() < 1.5
                    && (bg_global.height() - baked_bg.height()).abs() < 1.5;

                if !skip_baked {
                    if patch_matches_size
                        && let Some(&patch_tex) = blur_patches.get(&idx)
                    {
                        draw_rounded_texture_uv(
                            painter,
                            patch_tex,
                            bg_global.size(),
                            Rect::from_min_size(Pos2::ZERO, bg_global.size()),
                            bg_local,
                            TEXT_BG_RADIUS * view_scale,
                            Color32::WHITE,
                        );
                    } else if let Some(tex) = text_blur_tex {
                        draw_rounded_texture_uv(
                            painter,
                            tex,
                            virtual_size,
                            bg_global,
                            bg_local,
                            TEXT_BG_RADIUS * view_scale,
                            Color32::WHITE,
                        );
                    } else {
                        painter.rect_filled(
                            bg_local,
                            TEXT_BG_RADIUS * view_scale,
                            Color32::from_rgba_unmultiplied(14, 18, 28, 210),
                        );
                        painter.rect_stroke(
                            bg_local,
                            TEXT_BG_RADIUS * view_scale,
                            Stroke::new(
                                1.0_f32,
                                Color32::from_rgba_unmultiplied(255, 255, 255, 38),
                            ),
                            egui::StrokeKind::Middle,
                        );
                    }
                } else if let Some(tex) = text_blur_tex {
                    draw_rounded_texture_uv(
                        painter,
                        tex,
                        virtual_size,
                        bg_global,
                        bg_local,
                        TEXT_BG_RADIUS * view_scale,
                        Color32::WHITE,
                    );
                } else {
                    painter.rect_filled(
                        bg_local,
                        TEXT_BG_RADIUS * view_scale,
                        Color32::from_rgba_unmultiplied(14, 18, 28, 210),
                    );
                }

                if let Some(target) = arrow_to {
                    let start = map_pt(callout_anchor(content_r, *target));
                    draw_egui_arrow(
                        painter,
                        start,
                        map_pt(*target),
                        Stroke::new((*thickness * 0.85).max(2.0) * view_scale, *color),
                        view_scale,
                    );
                }
                let local_box = map_rect(box_r);
                let align = match placement {
                    TextPlacement::LeftTop
                    | TextPlacement::LeftCenter
                    | TextPlacement::LeftBottom => egui::Align2::LEFT_TOP,
                    TextPlacement::CenterTop | TextPlacement::CenterBottom => {
                        egui::Align2::CENTER_TOP
                    }
                    TextPlacement::RightTop
                    | TextPlacement::RightCenter
                    | TextPlacement::RightBottom => egui::Align2::RIGHT_TOP,
                };
                let anchor_x = local_box.left() + local_box.width() * h_frac;
                let local_content_h = content_h * view_scale;
                let anchor_y =
                    local_box.top() + (local_box.height() - local_content_h).max(0.0) * v_frac;
                let line_step = local_content_h / wrapped_lines.len().max(1) as f32;
                let shadow_px = view_scale.max(1.0);
                for (li, line) in wrapped_lines.iter().enumerate() {
                    let tp = Pos2::new(anchor_x, anchor_y + li as f32 * line_step);
                    painter.text(
                        Pos2::new(tp.x + shadow_px, tp.y + shadow_px),
                        align,
                        line,
                        font_id.clone(),
                        Color32::from_rgba_unmultiplied(0, 0, 0, 110),
                    );
                    painter.text(tp, align, line, font_id.clone(), *color);
                }
            }
            Annotation::Ellipse {
                rect,
                color,
                thickness,
                filled,
            } => {
                let pts = ellipse_points(map_rect(*rect), 48);
                if *filled {
                    painter.add(egui::Shape::convex_polygon(pts, *color, Stroke::NONE));
                } else {
                    painter.add(egui::Shape::closed_line(
                        pts,
                        Stroke::new(*thickness * view_scale, *color),
                    ));
                }
            }
            Annotation::Rect {
                rect,
                color,
                thickness,
                filled,
            } => {
                let cr = rect_corner_radius(*thickness, rect.width(), rect.height()) * view_scale;
                let lrect = map_rect(*rect);
                if *filled {
                    painter.rect_filled(lrect, cr, *color);
                } else {
                    painter.rect_stroke(
                        lrect,
                        cr,
                        Stroke::new(*thickness * view_scale, *color),
                        egui::StrokeKind::Middle,
                    );
                }
            }
            Annotation::Step {
                pos,
                number,
                color,
                radius,
            } => {
                let lpos = map_pt(*pos);
                let rad = *radius * view_scale;
                painter.circle_filled(lpos, rad, *color);
                painter.circle_stroke(lpos, rad, Stroke::new(1.5 * view_scale, Color32::WHITE));
                let text_color = contrasting_fg(*color);
                painter.text(
                    lpos,
                    egui::Align2::CENTER_CENTER,
                    number.to_string(),
                    egui::FontId::proportional((rad * 1.05).max(11.0)),
                    text_color,
                );
            }
        }

        if selected_idx == Some(idx) {
            let b = map_rect(ann.bounding_rect()).expand(4.0);
            painter.rect_stroke(
                b,
                6.0,
                Stroke::new(1.4_f32, accent),
                egui::StrokeKind::Outside,
            );
            for cp in ann.control_points() {
                let lcp = map_pt(cp);
                painter.circle_filled(lcp, 6.0, accent);
                painter.circle_stroke(lcp, 6.0, Stroke::new(1.5_f32, Color32::WHITE));
            }
        }
    }
}

pub fn draw_egui_arrow(
    painter: &egui::Painter,
    start: Pos2,
    end: Pos2,
    stroke: Stroke,
    scale: f32,
) {
    let Some(geom) = compute_arrow_geometry(start, end, stroke.width, scale) else {
        return;
    };
    painter.line_segment([start, end], stroke);
    painter.circle_filled(start, stroke.width * 0.5, stroke.color);
    draw_egui_arrowhead(painter, geom, end, stroke);
}

fn draw_egui_arrow_dashed(
    painter: &egui::Painter,
    start_logical: Pos2,
    end_logical: Pos2,
    map_pt: &impl Fn(Pos2) -> Pos2,
    stroke: Stroke,
    scale: f32,
    (dash_len, gap_len): (f32, f32),
) {
    let start = map_pt(start_logical);
    let end = map_pt(end_logical);
    let Some(geom) = compute_arrow_geometry(start, end, stroke.width, scale) else {
        return;
    };
    let dashes = sample_dashed_polyline(&[start_logical, end_logical], dash_len, gap_len);
    paint_polyline_group(painter, dashes, map_pt, stroke.width, stroke.color, true);
    draw_egui_arrowhead(painter, geom, end, stroke);
}

fn draw_egui_arrowhead(
    painter: &egui::Painter,
    geom: super::ArrowGeometry,
    end: Pos2,
    stroke: Stroke,
) {
    painter.line_segment([end, geom.wing_l], stroke);
    painter.line_segment([end, geom.wing_r], stroke);

    let r = stroke.width * 0.5;
    painter.circle_filled(end, r, stroke.color);
    painter.circle_filled(geom.wing_l, r, stroke.color);
    painter.circle_filled(geom.wing_r, r, stroke.color);
}
