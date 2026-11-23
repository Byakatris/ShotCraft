use std::collections::HashMap;
use std::f32::consts::TAU;
use std::sync::OnceLock;

use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use egui::{Color32, Pos2, Rect, Stroke, TextureId, Vec2};
use image::{Rgba, RgbaImage};

use crate::material::draw_rounded_texture_uv;
use crate::settings::TextFont;

pub const ROBOTO_FONT_BYTES: &[u8] = include_bytes!("../assets/Roboto-Medium.ttf");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Move,
    Pen,
    Marker,
    Line,
    Arrow,
    Text,
    Ellipse,
    Rect,
    Step,
    Blur,
}

impl Tool {
    pub fn code(self) -> &'static str {
        match self {
            Self::Move => "move",
            Self::Pen => "pen",
            Self::Marker => "marker",
            Self::Line => "line",
            Self::Arrow => "arrow",
            Self::Text => "text",
            Self::Ellipse => "ellipse",
            Self::Rect => "rect",
            Self::Step => "step",
            Self::Blur => "blur",
        }
    }

    pub fn from_code(s: &str) -> Self {
        match s.trim() {
            "move" => Self::Move,
            "pen" => Self::Pen,
            "marker" => Self::Marker,
            "line" => Self::Line,
            "arrow" => Self::Arrow,
            "text" => Self::Text,
            "ellipse" => Self::Ellipse,
            "rect" => Self::Rect,
            "step" => Self::Step,
            "blur" => Self::Blur,
            _ => Self::Arrow,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextPlacement {
    LeftTop,
    CenterTop,
    RightTop,
    RightCenter,
    RightBottom,
    CenterBottom,
    LeftBottom,
    LeftCenter,
}

impl TextPlacement {
    /// Determine which of the 8 box points should attach to `curr` when dragging from `start` to `curr`.
    pub fn from_drag(start: Pos2, curr: Pos2) -> Self {
        let d = curr - start;
        if d.length_sq() < 16.0 {
            return Self::LeftTop;
        }
        let sector = (d.y.atan2(d.x) / std::f32::consts::FRAC_PI_4)
            .round()
            .rem_euclid(8.0) as i32;
        match sector {
            0 => Self::LeftCenter,
            1 => Self::LeftTop,
            2 => Self::CenterTop,
            3 => Self::RightTop,
            4 => Self::RightCenter,
            5 => Self::RightBottom,
            6 => Self::CenterBottom,
            _ => Self::LeftBottom,
        }
    }

    pub fn box_rect(self, anchor: Pos2, w: f32, h: f32) -> Rect {
        let min_x = match self {
            Self::LeftTop | Self::LeftCenter | Self::LeftBottom => anchor.x,
            Self::CenterTop | Self::CenterBottom => anchor.x - w * 0.5,
            Self::RightTop | Self::RightCenter | Self::RightBottom => anchor.x - w,
        };
        let min_y = match self {
            Self::LeftTop | Self::CenterTop | Self::RightTop => anchor.y,
            Self::LeftCenter | Self::RightCenter => anchor.y - h * 0.5,
            Self::LeftBottom | Self::CenterBottom | Self::RightBottom => anchor.y - h,
        };
        Rect::from_min_size(Pos2::new(min_x, min_y), Vec2::new(w, h))
    }

    pub fn h_align(self) -> egui::Align {
        match self {
            Self::LeftTop | Self::LeftCenter | Self::LeftBottom => egui::Align::Min,
            Self::CenterTop | Self::CenterBottom => egui::Align::Center,
            Self::RightTop | Self::RightCenter | Self::RightBottom => egui::Align::Max,
        }
    }

    pub fn h_align_frac(self) -> f32 {
        match self {
            Self::LeftTop | Self::LeftCenter | Self::LeftBottom => 0.0,
            Self::CenterTop | Self::CenterBottom => 0.5,
            Self::RightTop | Self::RightCenter | Self::RightBottom => 1.0,
        }
    }

    pub fn v_align_frac(self) -> f32 {
        match self {
            Self::LeftTop | Self::CenterTop | Self::RightTop => 0.0,
            Self::LeftCenter | Self::RightCenter => 0.5,
            Self::LeftBottom | Self::CenterBottom | Self::RightBottom => 1.0,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Annotation {
    Pen {
        points: Vec<Pos2>,
        color: Color32,
        thickness: f32,
    },
    Marker {
        points: Vec<Pos2>,
        color: Color32,
        thickness: f32,
    },
    Line {
        start: Pos2,
        end: Pos2,
        color: Color32,
        thickness: f32,
    },
    Arrow {
        start: Pos2,
        end: Pos2,
        color: Color32,
        thickness: f32,
    },
    Text {
        pos: Pos2,
        text: String,
        color: Color32,
        size: f32,
        arrow_to: Option<Pos2>,
        thickness: f32,
        placement: TextPlacement,
        font: TextFont,
        box_size: Option<Vec2>,
    },
    Ellipse {
        rect: Rect,
        color: Color32,
        thickness: f32,
        filled: bool,
    },
    Rect {
        rect: Rect,
        color: Color32,
        thickness: f32,
        filled: bool,
    },
    Step {
        pos: Pos2,
        number: u32,
        color: Color32,
        radius: f32,
    },
    Blur {
        rect: Rect,
        radius: f32,
        pixelate: bool,
    },
}

impl Annotation {
    pub fn bounding_rect(&self) -> Rect {
        match self {
            Self::Pen {
                points, thickness, ..
            }
            | Self::Marker {
                points, thickness, ..
            } => {
                if points.is_empty() {
                    return Rect::NOTHING;
                }
                let mut r = Rect::from_min_max(points[0], points[0]);
                for &p in points {
                    r.extend_with(p);
                }
                r.expand(*thickness * 0.5 + 6.0)
            }
            Self::Line {
                start,
                end,
                thickness,
                ..
            }
            | Self::Arrow {
                start,
                end,
                thickness,
                ..
            } => Rect::from_two_pos(*start, *end).expand(*thickness + 8.0),
            Self::Text {
                pos,
                text,
                size,
                arrow_to,
                placement,
                font,
                box_size,
                ..
            } => {
                let mut r = text_box_rect(*pos, text, *size, *placement, *font, *box_size);
                if let Some(target) = arrow_to {
                    r.extend_with(*target);
                }
                r.expand(6.0)
            }
            Self::Ellipse {
                rect, thickness, ..
            }
            | Self::Rect {
                rect, thickness, ..
            } => rect.expand(*thickness * 0.5 + 4.0),
            Self::Step { pos, radius, .. } => {
                Rect::from_center_size(*pos, Vec2::splat(*radius * 2.0 + 4.0))
            }
            Self::Blur { rect, .. } => *rect,
        }
    }

    pub fn hit_test(&self, p: Pos2) -> bool {
        match self {
            Self::Line { start, end, .. } | Self::Arrow { start, end, .. } => {
                dist_to_segment(p, *start, *end) <= 12.0
            }
            Self::Pen {
                points, thickness, ..
            }
            | Self::Marker {
                points, thickness, ..
            } => {
                let tol = (*thickness * 0.5 + 10.0).max(12.0);
                if points.len() == 1 {
                    return points[0].distance(p) <= tol;
                }
                points.windows(2).any(|w| dist_to_segment(p, w[0], w[1]) <= tol)
            }
            Self::Text {
                pos,
                text,
                size,
                arrow_to,
                placement,
                font,
                box_size,
                ..
            } => {
                let box_r = text_box_rect(*pos, text, *size, *placement, *font, *box_size);
                if box_r.expand(6.0).contains(p) {
                    return true;
                }
                if let Some(target) = arrow_to {
                    let start = callout_anchor(box_r, *target);
                    return dist_to_segment(p, start, *target) <= 12.0;
                }
                false
            }
            Self::Step { pos, radius, .. } => pos.distance(p) <= *radius + 4.0,
            _ => self.bounding_rect().expand(4.0).contains(p),
        }
    }

    pub fn translate(&mut self, delta: Vec2) {
        match self {
            Self::Pen { points, .. } | Self::Marker { points, .. } => {
                for pt in points {
                    *pt += delta;
                }
            }
            Self::Line { start, end, .. } | Self::Arrow { start, end, .. } => {
                *start += delta;
                *end += delta;
            }
            Self::Text { pos, arrow_to, .. } => {
                *pos += delta;
                if let Some(t) = arrow_to {
                    *t += delta;
                }
            }
            Self::Step { pos, .. } => {
                *pos += delta;
            }
            Self::Ellipse { rect, .. }
            | Self::Rect { rect, .. }
            | Self::Blur { rect, .. } => {
                *rect = rect.translate(delta);
            }
        }
    }

    pub fn scale_coords(&mut self, factor: f32) {
        let sc = |p: &mut Pos2| {
            p.x *= factor;
            p.y *= factor;
        };
        match self {
            Self::Pen {
                points, thickness, ..
            }
            | Self::Marker {
                points, thickness, ..
            } => {
                for pt in points {
                    sc(pt);
                }
                *thickness *= factor;
            }
            Self::Line {
                start,
                end,
                thickness,
                ..
            }
            | Self::Arrow {
                start,
                end,
                thickness,
                ..
            } => {
                sc(start);
                sc(end);
                *thickness *= factor;
            }
            Self::Text {
                pos,
                size,
                arrow_to,
                thickness,
                box_size,
                ..
            } => {
                sc(pos);
                *size *= factor;
                *thickness *= factor;
                if let Some(t) = arrow_to {
                    sc(t);
                }
                if let Some(bs) = box_size {
                    *bs *= factor;
                }
            }
            Self::Ellipse {
                rect, thickness, ..
            }
            | Self::Rect {
                rect, thickness, ..
            } => {
                sc(&mut rect.min);
                sc(&mut rect.max);
                *thickness *= factor;
            }
            Self::Step { pos, radius, .. } => {
                sc(pos);
                *radius *= factor;
            }
            Self::Blur { rect, radius, .. } => {
                sc(&mut rect.min);
                sc(&mut rect.max);
                *radius *= factor;
            }
        }
    }

    /// Control handles: 2 endpoints for Line/Arrow, 4 corners for Rect/Ellipse/Blur/Text (+ arrow tip for Text callout).
    pub fn control_points(&self) -> Vec<Pos2> {
        match self {
            Self::Line { start, end, .. } | Self::Arrow { start, end, .. } => vec![*start, *end],
            Self::Rect { rect, .. } | Self::Ellipse { rect, .. } | Self::Blur { rect, .. } => {
                vec![
                    rect.left_top(),
                    rect.right_top(),
                    rect.right_bottom(),
                    rect.left_bottom(),
                ]
            }
            Self::Text {
                pos,
                text,
                size,
                arrow_to,
                placement,
                font,
                box_size,
                ..
            } => {
                let r = text_box_rect(*pos, text, *size, *placement, *font, *box_size).expand(4.0);
                let mut pts = vec![
                    r.left_top(),
                    r.right_top(),
                    r.right_bottom(),
                    r.left_bottom(),
                ];
                if let Some(t) = arrow_to {
                    pts.push(*t);
                }
                pts
            }
            _ => Vec::new(),
        }
    }

    pub fn move_control_point(&mut self, handle_idx: usize, new_pos: Pos2) {
        match self {
            Self::Line { start, end, .. } | Self::Arrow { start, end, .. } => {
                if handle_idx == 0 {
                    *start = new_pos;
                } else if handle_idx == 1 {
                    *end = new_pos;
                }
            }
            Self::Rect { rect, .. } | Self::Ellipse { rect, .. } | Self::Blur { rect, .. } => {
                let anchor = match handle_idx {
                    0 => rect.right_bottom(),
                    1 => rect.left_bottom(),
                    2 => rect.left_top(),
                    3 => rect.right_top(),
                    _ => return,
                };
                let r = Rect::from_two_pos(anchor, new_pos);
                if r.width() >= 6.0 && r.height() >= 6.0 {
                    *rect = r;
                }
            }
            Self::Text {
                pos,
                text,
                size,
                arrow_to,
                placement,
                font,
                box_size,
                ..
            } => {
                if handle_idx == 4 {
                    if let Some(t) = arrow_to {
                        *t = new_pos;
                    }
                    return;
                }
                let old_box =
                    text_box_rect(*pos, text, *size, *placement, *font, *box_size).expand(4.0);
                let corner = |r: Rect, idx: usize| -> Option<Pos2> {
                    match idx {
                        0 => Some(r.right_bottom()),
                        1 => Some(r.left_bottom()),
                        2 => Some(r.left_top()),
                        3 => Some(r.right_top()),
                        _ => None,
                    }
                };
                let Some(anchor) = corner(old_box, handle_idx) else {
                    return;
                };
                let new_w = ((new_pos.x - anchor.x).abs() - 8.0).max(36.0);
                let new_h = ((new_pos.y - anchor.y).abs() - 8.0).max(*size * 1.25);
                *box_size = Some(Vec2::new(new_w, new_h));
                let new_box =
                    text_box_rect(*pos, text, *size, *placement, *font, *box_size).expand(4.0);
                if let Some(new_anchor) = corner(new_box, handle_idx) {
                    *pos += anchor - new_anchor;
                }
            }
            _ => {}
        }
    }

    pub fn set_color(&mut self, new_color: Color32) {
        match self {
            Self::Pen { color, .. }
            | Self::Marker { color, .. }
            | Self::Line { color, .. }
            | Self::Arrow { color, .. }
            | Self::Text { color, .. }
            | Self::Ellipse { color, .. }
            | Self::Rect { color, .. }
            | Self::Step { color, .. } => *color = new_color,
            Self::Blur { .. } => {}
        }
    }

    pub fn apply_thickness(&mut self, new_t: f32) {
        match self {
            Self::Pen { thickness, .. }
            | Self::Marker { thickness, .. }
            | Self::Line { thickness, .. }
            | Self::Arrow { thickness, .. }
            | Self::Ellipse { thickness, .. }
            | Self::Rect { thickness, .. } => *thickness = new_t,
            Self::Text {
                size, thickness, ..
            } => {
                *thickness = new_t;
                *size = (13.0 + new_t * 2.2).clamp(16.0, 57.0);
            }
            Self::Step { radius, .. } => {
                *radius = (9.5 + new_t * 0.95).clamp(11.0, 28.5);
            }
            Self::Blur { radius, .. } => {
                *radius = (2.5 + new_t * 0.55).clamp(3.0, 14.0);
            }
        }
    }
}

/// 2-pass Chaikin corner-cutting to turn mouse polyline points into silky-smooth curves.
fn smooth_chaikin(pts: &[Pos2]) -> Vec<Pos2> {
    if pts.len() <= 2 {
        return pts.to_vec();
    }
    let mut cur = pts.to_vec();
    for _ in 0..2 {
        let mut next = Vec::with_capacity(cur.len() * 2);
        next.push(cur[0]);
        for w in cur.windows(2) {
            let p0 = w[0];
            let p1 = w[1];
            next.push(Pos2::new(p0.x * 0.75 + p1.x * 0.25, p0.y * 0.75 + p1.y * 0.25));
            next.push(Pos2::new(p0.x * 0.25 + p1.x * 0.75, p0.y * 0.25 + p1.y * 0.75));
        }
        if let Some(&last) = cur.last() {
            next.push(last);
        }
        cur = next;
    }
    cur
}

fn measure_line_width(line: &str, size: f32, tf: TextFont) -> f32 {
    if let Some(font) = font_for(tf) {
        let scale = PxScale::from(size * 1.15);
        let scaled = font.as_scaled(scale);
        let mut w = 0.0_f32;
        let mut prev_id = None;
        for ch in line.chars() {
            let gid = font.glyph_id(ch);
            if let Some(prev) = prev_id {
                w += scaled.kern(prev, gid);
            }
            w += scaled.h_advance(gid);
            prev_id = Some(gid);
        }
        return w;
    }
    line.chars().count() as f32 * size * 0.72
}

pub fn wrap_text_lines(
    text: &str,
    size: f32,
    tf: TextFont,
    wrap_width: Option<f32>,
) -> (Vec<String>, Vec2) {
    let line_height = if let Some(font) = font_for(tf) {
        let scaled = font.as_scaled(PxScale::from(size * 1.15));
        (scaled.height() + scaled.line_gap()).max(size * 1.25)
    } else {
        size * 1.25
    };

    let mut out_lines: Vec<String> = Vec::new();
    let max_w = wrap_width.map(|w| w.max(24.0));

    for raw_line in text.split('\n') {
        let Some(limit) = max_w else {
            out_lines.push(raw_line.to_string());
            continue;
        };
        if raw_line.is_empty() {
            out_lines.push(String::new());
            continue;
        }
        let mut cur = String::new();
        for word in raw_line.split_inclusive(' ') {
            let candidate = format!("{cur}{word}");
            if cur.is_empty() || measure_line_width(candidate.trim_end(), size, tf) <= limit {
                if measure_line_width(word.trim_end(), size, tf) > limit && cur.is_empty() {
                    let mut chunk = String::new();
                    for ch in word.chars() {
                        let next = format!("{chunk}{ch}");
                        if !chunk.is_empty() && measure_line_width(&next, size, tf) > limit {
                            out_lines.push(chunk);
                            chunk = ch.to_string();
                        } else {
                            chunk = next;
                        }
                    }
                    cur = chunk;
                } else {
                    cur = candidate;
                }
            } else {
                out_lines.push(cur.trim_end().to_string());
                cur = word.to_string();
                if measure_line_width(cur.trim_end(), size, tf) > limit {
                    let mut chunk = String::new();
                    for ch in cur.chars() {
                        let next = format!("{chunk}{ch}");
                        if !chunk.is_empty() && measure_line_width(&next, size, tf) > limit {
                            out_lines.push(chunk);
                            chunk = ch.to_string();
                        } else {
                            chunk = next;
                        }
                    }
                    cur = chunk;
                }
            }
        }
        if !cur.is_empty() {
            out_lines.push(cur.trim_end().to_string());
        }
    }

    if out_lines.is_empty() {
        out_lines.push(String::new());
    }

    let content_max_w = out_lines
        .iter()
        .map(|l| measure_line_width(l, size, tf))
        .fold(0.0_f32, f32::max);
    let box_w = match max_w {
        Some(w) => w.max(24.0),
        None => content_max_w.max(24.0),
    };
    let box_h = out_lines.len() as f32 * line_height + 4.0;
    (out_lines, Vec2::new(box_w, box_h))
}

/// Compute box rect, tight text-content rect, wrapped lines, and content size in a single pass.
pub fn text_box_and_content_rects(
    pos: Pos2,
    text: &str,
    size: f32,
    placement: TextPlacement,
    tf: TextFont,
    box_size: Option<Vec2>,
) -> (Rect, Rect, Vec<String>, Vec2) {
    let wrap_w = box_size.map(|s| s.x);
    let (lines, measured_sz) = wrap_text_lines(text, size, tf, wrap_w);
    let final_w = box_size.map_or(measured_sz.x, |s| s.x.max(24.0));
    let final_h = box_size.map_or(measured_sz.y, |s| s.y.max(measured_sz.y));
    let box_r = placement.box_rect(pos, final_w, final_h);

    let content_w = lines
        .iter()
        .map(|l| measure_line_width(l, size, tf))
        .fold(0.0_f32, f32::max)
        .max(20.0);
    let content_h = measured_sz.y.max(size * 1.25);
    let left = box_r.left() + (box_r.width() - content_w).max(0.0) * placement.h_align_frac();
    let top = box_r.top() + (box_r.height() - content_h).max(0.0) * placement.v_align_frac();
    let content_r = Rect::from_min_size(Pos2::new(left, top), Vec2::new(content_w, content_h));
    (box_r, content_r, lines, Vec2::new(content_w, content_h))
}

pub fn text_box_rect(
    pos: Pos2,
    text: &str,
    size: f32,
    placement: TextPlacement,
    tf: TextFont,
    box_size: Option<Vec2>,
) -> Rect {
    text_box_and_content_rects(pos, text, size, placement, tf, box_size).0
}

/// Tight bounding rect around the actual wrapped text glyphs (not the stretched virtual box).
pub fn text_content_rect(
    pos: Pos2,
    text: &str,
    size: f32,
    placement: TextPlacement,
    tf: TextFont,
    box_size: Option<Vec2>,
) -> Rect {
    text_box_and_content_rects(pos, text, size, placement, tf, box_size).1
}

/// Snap the callout arrow anchor to the closest of the 8 magnetic points on `box_rect`.
pub fn callout_anchor(box_rect: Rect, target: Pos2) -> Pos2 {
    let p = box_rect.expand(6.0);
    let pts = [
        p.left_top(),
        Pos2::new(p.center().x, p.top()),
        p.right_top(),
        Pos2::new(p.right(), p.center().y),
        p.right_bottom(),
        Pos2::new(p.center().x, p.bottom()),
        p.left_bottom(),
        Pos2::new(p.left(), p.center().y),
    ];
    pts.into_iter()
        .min_by(|a, b| a.distance_sq(target).total_cmp(&b.distance_sq(target)))
        .unwrap_or_else(|| p.center())
}

fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_sq();
    if len_sq < 1e-4 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    p.distance(a + ab * t)
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
                if !skip_baked {
                    if let Some(&patch_tex) = blur_patches.get(&idx) {
                        painter.image(
                            patch_tex,
                            local_rect,
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                        continue;
                    }
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
            } => {
                let marker_col =
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 105);
                let width = *thickness * 3.8 * view_scale;
                let smoothed: Vec<Pos2> = smooth_chaikin(points).into_iter().map(map_pt).collect();
                if smoothed.len() >= 2 {
                    painter.add(egui::Shape::line(
                        smoothed.clone(),
                        Stroke::new(width, marker_col),
                    ));
                    if let Some(&first) = smoothed.first() {
                        painter.circle_filled(first, width * 0.5, marker_col);
                    }
                    if let Some(&last) = smoothed.last() {
                        painter.circle_filled(last, width * 0.5, marker_col);
                    }
                } else if let Some(&p) = smoothed.first() {
                    painter.circle_filled(p, width * 0.5, marker_col);
                }
            }
            Annotation::Pen {
                points,
                color,
                thickness,
            } => {
                let lw = *thickness * view_scale;
                let smoothed: Vec<Pos2> = smooth_chaikin(points).into_iter().map(map_pt).collect();
                if smoothed.len() >= 2 {
                    painter.add(egui::Shape::line(smoothed.clone(), Stroke::new(lw, *color)));
                    if let Some(&first) = smoothed.first() {
                        painter.circle_filled(first, lw * 0.5, *color);
                    }
                    if let Some(&last) = smoothed.last() {
                        painter.circle_filled(last, lw * 0.5, *color);
                    }
                } else if let Some(&p) = smoothed.first() {
                    painter.circle_filled(p, lw * 0.5, *color);
                }
            }
            Annotation::Line {
                start,
                end,
                color,
                thickness,
            } => {
                let s = map_pt(*start);
                let e = map_pt(*end);
                let lw = *thickness * view_scale;
                painter.line_segment([s, e], Stroke::new(lw, *color));
                painter.circle_filled(s, lw * 0.5, *color);
                painter.circle_filled(e, lw * 0.5, *color);
            }
            Annotation::Arrow {
                start,
                end,
                color,
                thickness,
            } => {
                draw_egui_arrow(
                    painter,
                    map_pt(*start),
                    map_pt(*end),
                    Stroke::new(*thickness * view_scale, *color),
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
                let font_id = egui::FontId::new(
                    *size * view_scale,
                    egui::FontFamily::Name(font.egui_family().into()),
                );
                let (box_r, _, wrapped_lines, measured_sz) =
                    text_box_and_content_rects(*pos, text, *size, *placement, *font, *box_size);
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
                let text_only_r = Rect::from_min_size(
                    Pos2::new(text_left, text_top),
                    Vec2::new(actual_w, content_h),
                );
                let bg_global = text_only_r.expand(6.0);
                let bg_local = map_rect(bg_global);

                if !skip_baked {
                    if let Some(&patch_tex) = blur_patches.get(&idx) {
                        draw_rounded_texture_uv(
                            painter,
                            patch_tex,
                            bg_global.size(),
                            Rect::from_min_size(Pos2::ZERO, bg_global.size()),
                            bg_local,
                            8.0 * view_scale,
                            Color32::WHITE,
                        );
                    } else if let Some(tex) = text_blur_tex {
                        draw_rounded_texture_uv(
                            painter,
                            tex,
                            virtual_size,
                            bg_global,
                            bg_local,
                            8.0 * view_scale,
                            Color32::WHITE,
                        );
                    } else {
                        painter.rect_filled(
                            bg_local,
                            8.0 * view_scale,
                            Color32::from_rgba_unmultiplied(14, 18, 28, 210),
                        );
                        painter.rect_stroke(
                            bg_local,
                            8.0 * view_scale,
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
                        8.0 * view_scale,
                        Color32::WHITE,
                    );
                } else {
                    painter.rect_filled(
                        bg_local,
                        8.0 * view_scale,
                        Color32::from_rgba_unmultiplied(14, 18, 28, 210),
                    );
                }

                if let Some(target) = arrow_to {
                    let start = map_pt(callout_anchor(box_r, *target));
                    draw_egui_arrow(
                        painter,
                        start,
                        map_pt(*target),
                        Stroke::new((*thickness * 0.85).max(2.0) * view_scale, *color),
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
                for (li, line) in wrapped_lines.iter().enumerate() {
                    let tp = Pos2::new(anchor_x, anchor_y + li as f32 * line_step);
                    painter.text(
                        Pos2::new(tp.x + 1.0, tp.y + 1.0),
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
                let cr = (*thickness * 1.2 * view_scale).clamp(4.0, 14.0);
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
                painter.circle_stroke(lpos, rad, Stroke::new(1.5_f32, Color32::WHITE));
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

fn contrasting_fg(c: Color32) -> Color32 {
    let lum = 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32;
    if lum > 165.0 {
        Color32::from_rgb(8, 20, 30)
    } else {
        Color32::WHITE
    }
}

fn ellipse_points(rect: Rect, segments: usize) -> Vec<Pos2> {
    let center = rect.center();
    let rx = rect.width() * 0.5;
    let ry = rect.height() * 0.5;
    (0..segments)
        .map(|i| {
            let a = (i as f32 / segments as f32) * TAU;
            Pos2::new(center.x + rx * a.cos(), center.y + ry * a.sin())
        })
        .collect()
}

pub fn draw_egui_arrow(painter: &egui::Painter, start: Pos2, end: Pos2, stroke: Stroke) {
    let dir = end - start;
    let len = dir.length();
    if len < 2.0 {
        return;
    }
    let unit = dir / len;
    let perp = egui::vec2(-unit.y, unit.x);
    let head_len = (stroke.width * 3.0 + 10.0).min(len * 0.55);
    let head_half_w = head_len * 0.62;

    let wing_l = end - unit * head_len + perp * head_half_w;
    let wing_r = end - unit * head_len - perp * head_half_w;

    painter.line_segment([start, end], stroke);
    painter.line_segment([end, wing_l], stroke);
    painter.line_segment([end, wing_r], stroke);

    let r = stroke.width * 0.5;
    painter.circle_filled(start, r, stroke.color);
    painter.circle_filled(end, r, stroke.color);
    painter.circle_filled(wing_l, r, stroke.color);
    painter.circle_filled(wing_r, r, stroke.color);
}

fn to_rgba(c: Color32, alpha: u8) -> Rgba<u8> {
    Rgba([c.r(), c.g(), c.b(), alpha])
}

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
            } => {
                let r = (thickness * 3.8 * scale_factor * 0.5).max(3.0);
                let smoothed = smooth_chaikin(points);
                raster_polyline_blended(&mut out, &smoothed, &to_px, r, to_rgba(*color, 105));
            }
            Annotation::Pen {
                points,
                color,
                thickness,
            } => {
                let r = (thickness * scale_factor * 0.5).max(1.0);
                let rgba = to_rgba(*color, 255);
                let smoothed = smooth_chaikin(points);
                for win in smoothed.windows(2) {
                    raster_thick_line(&mut out, to_px(win[0]), to_px(win[1]), r, rgba);
                }
                if let Some(&p) = smoothed.first() {
                    let (px, py) = to_px(p);
                    raster_disc(&mut out, px, py, r, rgba);
                }
            }
            Annotation::Line {
                start,
                end,
                color,
                thickness,
            } => {
                raster_thick_line(
                    &mut out,
                    to_px(*start),
                    to_px(*end),
                    (thickness * scale_factor * 0.5).max(1.0),
                    to_rgba(*color, 255),
                );
            }
            Annotation::Arrow {
                start,
                end,
                color,
                thickness,
            } => {
                raster_arrow(
                    &mut out,
                    to_px(*start),
                    to_px(*end),
                    thickness * scale_factor,
                    to_rgba(*color, 255),
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
                smooth_blur_region(
                    &mut out,
                    bx0 as i32,
                    by0 as i32,
                    bx1 as i32,
                    by1 as i32,
                    ((*thickness * 1.1 + 10.0).clamp(12.0, 28.0) * scale_factor).round() as i32,
                );

                if let Some(target) = arrow_to {
                    let start = callout_anchor(box_r, *target);
                    raster_arrow(
                        &mut out,
                        to_px(start),
                        to_px(*target),
                        (*thickness * 0.85).max(2.0) * scale_factor,
                        to_rgba(*color, 255),
                    );
                }
                let h_frac = placement.h_align_frac();
                let v_frac = placement.v_align_frac();
                let text_top_logical =
                    box_r.top() + (box_r.height() - content_sz.y).max(0.0) * v_frac;
                let line_step_logical = content_sz.y / wrapped_lines.len().max(1) as f32;
                for (li, line) in wrapped_lines.iter().enumerate() {
                    let lw = measure_line_width(line, *size, *font);
                    let lx = box_r.left() + (box_r.width() - lw).max(0.0) * h_frac;
                    let ly = text_top_logical + li as f32 * line_step_logical;
                    let (tx, ty) = to_px(Pos2::new(lx, ly));
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
                    let pts = ellipse_points(*rect, 96);
                    let r = (thickness * scale_factor * 0.5).max(1.0);
                    let rgba = to_rgba(*color, 255);
                    for i in 0..pts.len() {
                        let a = to_px(pts[i]);
                        let b = to_px(pts[(i + 1) % pts.len()]);
                        raster_thick_line(&mut out, a, b, r, rgba);
                    }
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
                if *filled {
                    raster_filled_rounded_rect(
                        &mut out,
                        rx0,
                        ry0,
                        rx1,
                        ry1,
                        thickness * scale_factor,
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
                    rad + 1.5 * scale_factor,
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

fn font_for(tf: TextFont) -> Option<&'static FontArc> {
    static CACHE: [OnceLock<Option<FontArc>>; 15] = [const { OnceLock::new() }; 15];
    let slot = CACHE.get(tf as usize)?;
    slot.get_or_init(|| match tf {
        TextFont::Roboto => FontArc::try_from_slice(ROBOTO_FONT_BYTES).ok(),
        _ => tf
            .resolve_font_bytes()
            .and_then(|b| FontArc::try_from_vec(b).ok())
            .or_else(|| FontArc::try_from_slice(ROBOTO_FONT_BYTES).ok()),
    })
    .as_ref()
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

/// Fast bounding-strip mask rasterizer for semi-transparent Marker polylines (uniform alpha, O(Segments * Length)).
fn raster_polyline_blended(
    img: &mut RgbaImage,
    points: &[Pos2],
    to_px: &impl Fn(Pos2) -> (f32, f32),
    radius: f32,
    color: Rgba<u8>,
) {
    if points.is_empty() || img.width() == 0 || img.height() == 0 {
        return;
    }
    let w = img.width() as i32;
    let h = img.height() as i32;
    let mut min_x = w;
    let mut max_x = -1;
    let mut min_y = h;
    let mut max_y = -1;
    let px_pts: Vec<(f32, f32)> = points
        .iter()
        .map(|&p| {
            let (px, py) = to_px(p);
            min_x = min_x.min((px - radius - 2.0).floor() as i32);
            max_x = max_x.max((px + radius + 2.0).ceil() as i32);
            min_y = min_y.min((py - radius - 2.0).floor() as i32);
            max_y = max_y.max((py + radius + 2.0).ceil() as i32);
            (px, py)
        })
        .collect();

    min_x = min_x.clamp(0, w - 1);
    max_x = max_x.clamp(0, w - 1);
    min_y = min_y.clamp(0, h - 1);
    max_y = max_y.clamp(0, h - 1);
    if max_x < min_x || max_y < min_y {
        return;
    }

    let mask_w = (max_x - min_x + 1) as usize;
    let mask_h = (max_y - min_y + 1) as usize;
    let mut mask = vec![0u8; mask_w * mask_h];
    let r2 = radius * radius;
    let pad = (radius + 1.0).ceil() as i32;

    let stamp_seg = |mask: &mut [u8], ax: f32, ay: f32, bx: f32, by: f32| {
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
                    (((fx - ax) * abx + (fy - ay) * aby) / len_sq).clamp(0.0, 1.0)
                };
                let dx = fx - (ax + abx * t);
                let dy = fy - (ay + aby * t);
                if dx * dx + dy * dy <= r2 {
                    mask[row_off + (x - min_x) as usize] = 1;
                }
            }
        }
    };

    if px_pts.len() == 1 {
        stamp_seg(&mut mask, px_pts[0].0, px_pts[0].1, px_pts[0].0, px_pts[0].1);
    } else {
        for seg in px_pts.windows(2) {
            stamp_seg(&mut mask, seg[0].0, seg[0].1, seg[1].0, seg[1].1);
        }
    }

    for my in 0..mask_h {
        let row_off = my * mask_w;
        let iy = (min_y + my as i32) as u32;
        for mx in 0..mask_w {
            if mask[row_off + mx] != 0 {
                blend_pixel(img, (min_x + mx as i32) as u32, iy, color, 1.0);
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
            if cnt > 0 {
                let col = Rgba([
                    (rs / cnt) as u8,
                    (gs / cnt) as u8,
                    (bs_sum / cnt) as u8,
                    255,
                ]);
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

/// 2-pass separable Gaussian-approximating blur over `[x0..x1, y0..y1]` in `O(W * H)`.
pub fn smooth_blur_region(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, radius: i32) {
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

    let mut tmp = vec![[0f32; 3]; rw * rh];
    let r = radius.clamp(2, 22) as usize;

    for _ in 0..2 {
        for y in 0..rh {
            let row = y * rw;
            let mut rs = 0.0;
            let mut gs = 0.0;
            let mut bs = 0.0;
            let mut cnt = 0.0;
            for nx in 0..=r.min(rw - 1) {
                let c = buf[row + nx];
                rs += c[0];
                gs += c[1];
                bs += c[2];
                cnt += 1.0;
            }
            for x in 0..rw {
                tmp[row + x] = [rs / cnt, gs / cnt, bs / cnt];
                let add_x = x + r + 1;
                if add_x < rw {
                    let c = buf[row + add_x];
                    rs += c[0];
                    gs += c[1];
                    bs += c[2];
                    cnt += 1.0;
                }
                if x >= r {
                    let c = buf[row + x - r];
                    rs -= c[0];
                    gs -= c[1];
                    bs -= c[2];
                    cnt -= 1.0;
                }
            }
        }

        for x in 0..rw {
            let mut rs = 0.0;
            let mut gs = 0.0;
            let mut bs = 0.0;
            let mut cnt = 0.0;
            for ny in 0..=r.min(rh - 1) {
                let c = tmp[ny * rw + x];
                rs += c[0];
                gs += c[1];
                bs += c[2];
                cnt += 1.0;
            }
            for y in 0..rh {
                buf[y * rw + x] = [rs / cnt, gs / cnt, bs / cnt];
                let add_y = y + r + 1;
                if add_y < rh {
                    let c = tmp[add_y * rw + x];
                    rs += c[0];
                    gs += c[1];
                    bs += c[2];
                    cnt += 1.0;
                }
                if y >= r {
                    let c = tmp[(y - r) * rw + x];
                    rs -= c[0];
                    gs -= c[1];
                    bs -= c[2];
                    cnt -= 1.0;
                }
            }
        }
    }

    for y in 0..rh {
        for x in 0..rw {
            let c = buf[y * rw + x];
            img.put_pixel(
                (x0 + x as i32) as u32,
                (y0 + y as i32) as u32,
                Rgba([
                    c[0].round().clamp(0.0, 255.0) as u8,
                    c[1].round().clamp(0.0, 255.0) as u8,
                    c[2].round().clamp(0.0, 255.0) as u8,
                    255,
                ]),
            );
        }
    }
}

fn raster_disc(img: &mut RgbaImage, cx: f32, cy: f32, radius: f32, color: Rgba<u8>) {
    let w = img.width() as i32;
    let h = img.height() as i32;
    if w == 0 || h == 0 {
        return;
    }
    let r2 = radius * radius;
    let min_x = ((cx - radius - 1.0).floor() as i32).clamp(0, w - 1);
    let max_x = ((cx + radius + 1.0).ceil() as i32).clamp(0, w - 1);
    let min_y = ((cy - radius - 1.0).floor() as i32).clamp(0, h - 1);
    let max_y = ((cy + radius + 1.0).ceil() as i32).clamp(0, h - 1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            if dx * dx + dy * dy <= r2 {
                img.put_pixel(x as u32, y as u32, color);
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
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let dist = (dx * dx + dy * dy).sqrt();
    let steps = (dist / (radius * 0.4).max(0.8)).ceil().max(1.0) as i32;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        raster_disc(img, a.0 + dx * t, a.1 + dy * t, radius, color);
    }
}

fn raster_rect(
    img: &mut RgbaImage,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    thickness: f32,
    color: Rgba<u8>,
) {
    let r = (thickness * 0.5).max(1.0);
    let w = (x1 - x0).abs();
    let h = (y1 - y0).abs();
    let cr = (thickness * 1.2).clamp(4.0, 28.0).min(w * 0.45).min(h * 0.45);
    let lx = x0.min(x1);
    let rx = x0.max(x1);
    let ty = y0.min(y1);
    let by = y0.max(y1);

    raster_thick_line(img, (lx + cr, ty), (rx - cr, ty), r, color);
    raster_thick_line(img, (rx, ty + cr), (rx, by - cr), r, color);
    raster_thick_line(img, (rx - cr, by), (lx + cr, by), r, color);
    raster_thick_line(img, (lx, by - cr), (lx, ty + cr), r, color);

    let corners = [
        (lx + cr, ty + cr, std::f32::consts::PI, std::f32::consts::PI * 1.5),
        (rx - cr, ty + cr, std::f32::consts::PI * 1.5, TAU),
        (rx - cr, by - cr, 0.0, std::f32::consts::FRAC_PI_2),
        (lx + cr, by - cr, std::f32::consts::FRAC_PI_2, std::f32::consts::PI),
    ];
    for (cx, cy, a0, a1) in corners {
        let steps = 10;
        for i in 0..steps {
            let t0 = a0 + (a1 - a0) * (i as f32 / steps as f32);
            let t1 = a0 + (a1 - a0) * ((i + 1) as f32 / steps as f32);
            raster_thick_line(
                img,
                (cx + cr * t0.cos(), cy + cr * t0.sin()),
                (cx + cr * t1.cos(), cy + cr * t1.sin()),
                r,
                color,
            );
        }
    }
}

fn raster_filled_rounded_rect(
    img: &mut RgbaImage,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    thickness: f32,
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
    let w = rx - lx;
    let h = by - ty;
    let cr = (thickness * 1.2).clamp(4.0, 28.0).min(w * 0.45).min(h * 0.45);
    let cx_min = lx + cr;
    let cx_max = (rx - cr).max(cx_min);
    let cy_min = ty + cr;
    let cy_max = (by - cr).max(cy_min);

    let min_x = (lx.floor() as i32).clamp(0, iw - 1);
    let max_x = (rx.ceil() as i32).clamp(0, iw - 1);
    let min_y = (ty.floor() as i32).clamp(0, ih - 1);
    let max_y = (by.ceil() as i32).clamp(0, ih - 1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let cx = fx.clamp(cx_min, cx_max);
            let cy = fy.clamp(cy_min, cy_max);
            let dx = fx - cx;
            let dy = fy - cy;
            if dx * dx + dy * dy <= cr * cr {
                img.put_pixel(x as u32, y as u32, color);
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

    let min_x = (lx.floor() as i32).clamp(0, iw - 1);
    let max_x = (rx.ceil() as i32).clamp(0, iw - 1);
    let min_y = (ty.floor() as i32).clamp(0, ih - 1);
    let max_y = (by.ceil() as i32).clamp(0, ih - 1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = (x as f32 + 0.5 - cx) / rx_rad;
            let dy = (y as f32 + 0.5 - cy) / ry_rad;
            if dx * dx + dy * dy <= 1.0 {
                img.put_pixel(x as u32, y as u32, color);
            }
        }
    }
}

fn raster_arrow(
    img: &mut RgbaImage,
    start: (f32, f32),
    end: (f32, f32),
    thickness: f32,
    color: Rgba<u8>,
) {
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

    let head_len = (thickness * 3.0 + 10.0).min(len * 0.55);
    let head_half_w = head_len * 0.62;
    let r = (thickness * 0.5).max(1.0);

    let wing_l = (
        end.0 - ux * head_len + px * head_half_w,
        end.1 - uy * head_len + py * head_half_w,
    );
    let wing_r = (
        end.0 - ux * head_len - px * head_half_w,
        end.1 - uy * head_len - py * head_half_w,
    );

    raster_thick_line(img, start, end, r, color);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{AppSettings, TextFont};

    #[test]
    fn embedded_fonts_parse_and_roundtrip() {
        assert!(FontArc::try_from_slice(ROBOTO_FONT_BYTES).is_ok());
        assert_eq!(AppSettings::default().text_font, TextFont::Roboto);
        assert!(font_for(TextFont::GostTypeAU).is_some());
        for tf in TextFont::ALL {
            assert_eq!(TextFont::from_code(tf.code()), tf);
        }
    }

    #[test]
    fn tool_code_roundtrip() {
        let all = [
            Tool::Move,
            Tool::Pen,
            Tool::Marker,
            Tool::Line,
            Tool::Arrow,
            Tool::Text,
            Tool::Ellipse,
            Tool::Rect,
            Tool::Step,
            Tool::Blur,
        ];
        for t in all {
            assert_eq!(Tool::from_code(t.code()), t);
        }
    }

    #[test]
    fn wrap_text_edge_cases_never_panic() {
        let (l0, s0) = wrap_text_lines("", 24.0, TextFont::Roboto, Some(40.0));
        assert!(!l0.is_empty() && s0.x >= 24.0 && s0.y > 0.0);

        let (l1, _) = wrap_text_lines(
            "СуперДовгеСловоБезПробілівЯкеМаєПеренестись",
            24.0,
            TextFont::Roboto,
            Some(60.0),
        );
        assert!(l1.len() >= 2);

        let (l2, _) = wrap_text_lines("Привіт світ тест", 24.0, TextFont::Roboto, Some(1.0));
        assert!(l2.len() >= 2);
    }

    #[test]
    fn blur_and_pixelate_edge_cases_never_panic() {
        let mut img = RgbaImage::new(10, 10);
        smooth_blur_region(&mut img, 8, 8, 2, 2, 10);
        smooth_blur_region(&mut img, -50, -50, -10, -10, 10);
        smooth_blur_region(&mut img, 50, 50, 100, 100, 10);
        smooth_blur_region(&mut img, 0, 0, 10, 10, 0);

        pixelate_region(&mut img, 8, 8, 2, 2, 8);
        pixelate_region(&mut img, -20, -20, 50, 50, 8);

        let mut checkerboard = RgbaImage::new(8, 8);
        for y in 0..8u32 {
            for x in 0..8u32 {
                let c = if (x + y) % 2 == 0 { 0 } else { 200 };
                checkerboard.put_pixel(x, y, Rgba([c, c, c, 255]));
            }
        }
        pixelate_region(&mut checkerboard, 0, 0, 8, 8, 8);
        assert_eq!(*checkerboard.get_pixel(0, 0), Rgba([100, 100, 100, 255]));
        assert_eq!(*checkerboard.get_pixel(7, 7), Rgba([100, 100, 100, 255]));
    }

    #[test]
    fn render_export_tiny_and_empty_images_never_panic() {
        let empty = RgbaImage::new(0, 0);
        let out_empty = render_export(
            &empty,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(10.0, 10.0)),
            1.0,
            &[],
        );
        assert_eq!((out_empty.width(), out_empty.height()), (1, 1));

        let tiny = RgbaImage::from_pixel(1, 1, Rgba([10, 20, 30, 255]));
        let anns = vec![
            Annotation::Blur {
                rect: Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                radius: 5.0,
                pixelate: false,
            },
            Annotation::Rect {
                rect: Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                color: Color32::RED,
                thickness: 4.0,
                filled: true,
            },
        ];
        let out_tiny = render_export(
            &tiny,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(50.0, 50.0)),
            2.0,
            &anns,
        );
        assert_eq!((out_tiny.width(), out_tiny.height()), (1, 1));
    }

    #[test]
    fn render_export_all_annotations_and_vector_step() {
        let src = RgbaImage::from_pixel(120, 120, Rgba([30, 40, 60, 255]));
        let anns = vec![
            Annotation::Pen {
                points: vec![Pos2::new(10.0, 10.0)],
                color: Color32::WHITE,
                thickness: 5.0,
            },
            Annotation::Marker {
                points: vec![Pos2::new(15.0, 15.0), Pos2::new(110.0, 100.0)],
                color: Color32::YELLOW,
                thickness: 5.0,
            },
            Annotation::Line {
                start: Pos2::new(5.0, 5.0),
                end: Pos2::new(80.0, 80.0),
                color: Color32::GREEN,
                thickness: 4.0,
            },
            Annotation::Arrow {
                start: Pos2::new(20.0, 90.0),
                end: Pos2::new(90.0, 20.0),
                color: Color32::RED,
                thickness: 5.0,
            },
            Annotation::Rect {
                rect: Rect::from_min_max(Pos2::new(10.0, 10.0), Pos2::new(50.0, 50.0)),
                color: Color32::BLUE,
                thickness: 4.0,
                filled: true,
            },
            Annotation::Ellipse {
                rect: Rect::from_min_max(Pos2::new(40.0, 40.0), Pos2::new(90.0, 80.0)),
                color: Color32::LIGHT_BLUE,
                thickness: 3.0,
                filled: false,
            },
            Annotation::Blur {
                rect: Rect::from_min_max(Pos2::new(20.0, 20.0), Pos2::new(70.0, 70.0)),
                radius: 8.0,
                pixelate: true,
            },
            Annotation::Text {
                pos: Pos2::new(60.0, 60.0),
                text: "Тест кирилиці".to_string(),
                color: Color32::WHITE,
                size: 24.0,
                arrow_to: Some(Pos2::new(20.0, 20.0)),
                thickness: 5.0,
                placement: TextPlacement::CenterTop,
                font: TextFont::Roboto,
                box_size: Some(Vec2::new(80.0, 90.0)),
            },
            Annotation::Step {
                pos: Pos2::new(100.0, 100.0),
                number: 42,
                color: Color32::from_rgb(10, 20, 30),
                radius: 14.0,
            },
        ];
        let out = render_export(
            &src,
            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(120.0, 120.0)),
            1.0,
            &anns,
        );
        assert_eq!(out.width(), 120);
        assert_eq!(out.height(), 120);

        // Verify vector step glyph pixels were actually drawn in contrasting white inside the circle
        let mut white_pixels = 0usize;
        for y in 92..=108u32 {
            for x in 92..=108u32 {
                let p = out.get_pixel(x, y);
                if p[0] > 180 && p[1] > 180 && p[2] > 180 {
                    white_pixels += 1;
                }
            }
        }
        assert!(white_pixels >= 8);
    }

    #[test]
    fn scale_coords_scales_positions_and_thickness() {
        let mut arrow = Annotation::Arrow {
            start: Pos2::new(10.0, 20.0),
            end: Pos2::new(30.0, 40.0),
            color: Color32::RED,
            thickness: 4.0,
        };
        arrow.scale_coords(2.0);
        match arrow {
            Annotation::Arrow {
                start,
                end,
                thickness,
                ..
            } => {
                assert_eq!(start, Pos2::new(20.0, 40.0));
                assert_eq!(end, Pos2::new(60.0, 80.0));
                assert_eq!(thickness, 8.0);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn golden_arrow_renders_shaft_and_arrowhead() {
        let bg = Rgba([20, 20, 30, 255]);
        let src = RgbaImage::from_pixel(80, 80, bg);
        let anns = vec![Annotation::Arrow {
            start: Pos2::new(10.0, 40.0),
            end: Pos2::new(70.0, 40.0),
            color: Color32::from_rgb(255, 0, 0),
            thickness: 4.0,
        }];
        let out = render_export(
            &src,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(80.0, 80.0)),
            1.0,
            &anns,
        );

        let mid = out.get_pixel(40, 40);
        assert!(mid[0] > 230 && mid[1] < 30 && mid[2] < 30, "mid={mid:?}");

        let tip = out.get_pixel(68, 40);
        assert!(tip[0] > 200, "tip={tip:?}");

        let wing_hits = (30..39)
            .flat_map(|y| (50..70).map(move |x| (x, y)))
            .filter(|&(x, y)| out.get_pixel(x, y)[0] > 180)
            .count();
        assert!(wing_hits >= 5, "wing_hits={wing_hits}");
        assert_eq!(*out.get_pixel(5, 5), bg);
    }

    #[test]
    fn golden_gaussian_blur_smooths_sharp_edge() {
        let mut src = RgbaImage::new(100, 60);
        for y in 0..60 {
            for x in 0..100 {
                let c = if x < 50 {
                    Rgba([0, 0, 0, 255])
                } else {
                    Rgba([255, 255, 255, 255])
                };
                src.put_pixel(x, y, c);
            }
        }
        let anns = vec![Annotation::Blur {
            rect: Rect::from_min_max(Pos2::new(20.0, 10.0), Pos2::new(80.0, 50.0)),
            radius: 8.0,
            pixelate: false,
        }];
        let out = render_export(
            &src,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 60.0)),
            1.0,
            &anns,
        );
        let seam_left = out.get_pixel(47, 30)[0];
        let seam_right = out.get_pixel(52, 30)[0];
        assert!(seam_left > 15 && seam_left < 200, "seam_left={seam_left}");
        assert!(
            seam_right > 55 && seam_right < 245,
            "seam_right={seam_right}"
        );
        // Outside blur rect remains pure black/white
        assert_eq!(*out.get_pixel(5, 30), Rgba([0, 0, 0, 255]));
        assert_eq!(*out.get_pixel(95, 30), Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn golden_marker_uniform_alpha_without_joint_seams() {
        let src = RgbaImage::from_pixel(100, 100, Rgba([255, 255, 255, 255]));
        let anns = vec![Annotation::Marker {
            points: vec![
                Pos2::new(20.0, 50.0),
                Pos2::new(50.0, 50.0),
                Pos2::new(50.0, 80.0),
            ],
            color: Color32::from_rgb(0, 0, 255),
            thickness: 5.0,
        }];
        let out = render_export(
            &src,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 100.0)),
            1.0,
            &anns,
        );
        let seg_mid = *out.get_pixel(35, 50);
        let joint = *out.get_pixel(50, 50);
        assert_eq!(seg_mid, joint, "marker joint must not double-darken");
        assert!(joint[0] < 200 && joint[2] == 255, "joint={joint:?}");
    }

    #[test]
    fn golden_z_order_blur_then_stroke_stays_crisp() {
        let src = RgbaImage::from_pixel(100, 100, Rgba([30, 30, 30, 255]));
        let anns = vec![
            Annotation::Line {
                start: Pos2::new(10.0, 40.0),
                end: Pos2::new(90.0, 40.0),
                color: Color32::from_rgb(255, 0, 0),
                thickness: 6.0,
            },
            Annotation::Blur {
                rect: Rect::from_min_max(Pos2::new(20.0, 20.0), Pos2::new(80.0, 80.0)),
                radius: 8.0,
                pixelate: false,
            },
            Annotation::Line {
                start: Pos2::new(10.0, 60.0),
                end: Pos2::new(90.0, 60.0),
                color: Color32::from_rgb(0, 255, 0),
                thickness: 4.0,
            },
        ];
        let out = render_export(
            &src,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 100.0)),
            1.0,
            &anns,
        );
        let red_blurred = out.get_pixel(50, 40);
        let green_crisp = out.get_pixel(50, 60);
        assert!(
            red_blurred[0] < 200,
            "red line before blur must be softened: {red_blurred:?}"
        );
        assert!(
            green_crisp[1] > 230 && green_crisp[0] < 30,
            "green line after blur must remain crisp: {green_crisp:?}"
        );
    }

    #[test]
    fn golden_text_and_step_render_visible_glyphs() {
        let bg = Rgba([15, 20, 30, 255]);
        let src = RgbaImage::from_pixel(160, 100, bg);
        let anns = vec![
            Annotation::Text {
                pos: Pos2::new(70.0, 45.0),
                text: "ABC".to_string(),
                color: Color32::WHITE,
                size: 20.0,
                arrow_to: Some(Pos2::new(20.0, 80.0)),
                thickness: 3.0,
                placement: TextPlacement::LeftTop,
                font: TextFont::Roboto,
                box_size: Some(Vec2::new(120.0, 60.0)),
            },
            Annotation::Step {
                pos: Pos2::new(135.0, 75.0),
                number: 7,
                color: Color32::from_rgb(20, 180, 170),
                radius: 13.0,
            },
        ];
        let out = render_export(
            &src,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(160.0, 100.0)),
            1.0,
            &anns,
        );
        let lit_pixels = (0..100)
            .flat_map(|y| (0..160).map(move |x| (x, y)))
            .filter(|&(x, y)| *out.get_pixel(x, y) != bg)
            .count();
        assert!(lit_pixels > 150, "lit_pixels={lit_pixels}");
    }
}
