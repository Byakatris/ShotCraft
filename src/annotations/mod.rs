pub mod paint;
pub mod raster;
#[cfg(test)]
mod tests;

use std::f32::consts::TAU;
use std::sync::OnceLock;

use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use egui::{Color32, Pos2, Rect, Vec2};
use image::Rgba;

use crate::settings::TextFont;

pub use paint::{draw_egui_arrow, paint_annotations};
pub use raster::{bake_blur_patch, render_export, separable_box_blur_rgb};

pub const ROBOTO_FONT_BYTES: &[u8] = include_bytes!("../../assets/Roboto-Medium.ttf");

/// Shared visual constants used by both live GPU preview (`paint.rs`) and CPU export (`raster.rs`).
pub(crate) const MARKER_ALPHA: u8 = 105;
pub(crate) const MARKER_WIDTH_MULT: f32 = 3.8;
pub(crate) const TEXT_BG_EXPAND: f32 = 6.0;
pub(crate) const TEXT_BG_RADIUS: f32 = 8.0;
pub(crate) const ARROW_HEAD_HALF_WIDTH_RATIO: f32 = 0.62;

pub struct TextLayout {
    pub box_rect: Rect,
    pub content_rect: Rect,
    pub wrapped_lines: Vec<String>,
    pub measured_size: Vec2,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ArrowGeometry {
    pub wing_l: Pos2,
    pub wing_r: Pos2,
}

/// Shared arrowhead geometry computation used identically by GPU preview and CPU export.
pub(crate) fn compute_arrow_geometry(
    start: Pos2,
    end: Pos2,
    stroke_width_px: f32,
    scale: f32,
) -> Option<ArrowGeometry> {
    let dir = end - start;
    let len = dir.length();
    if len < 2.0 {
        return None;
    }
    let unit = dir / len;
    let perp = Vec2::new(-unit.y, unit.x);
    let head_len = arrow_head_len(stroke_width_px, scale, len);
    let head_half_w = head_len * ARROW_HEAD_HALF_WIDTH_RATIO;
    Some(ArrowGeometry {
        wing_l: end - unit * head_len + perp * head_half_w,
        wing_r: end - unit * head_len - perp * head_half_w,
    })
}

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
        dashed: bool,
    },
    Marker {
        points: Vec<Pos2>,
        color: Color32,
        thickness: f32,
        dashed: bool,
    },
    Line {
        start: Pos2,
        end: Pos2,
        color: Color32,
        thickness: f32,
        dashed: bool,
    },
    Arrow {
        start: Pos2,
        end: Pos2,
        color: Color32,
        thickness: f32,
        dashed: bool,
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
                let layout =
                    text_box_and_content_rects(*pos, text, *size, *placement, *font, *box_size);
                let mut r = layout.box_rect.union(layout.content_rect);
                if let Some(target) = arrow_to {
                    r.extend_with(text_drag_handle(layout.content_rect));
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
                let layout =
                    text_box_and_content_rects(*pos, text, *size, *placement, *font, *box_size);
                if layout.box_rect.expand(6.0).contains(p)
                    || layout.content_rect.expand(8.0).contains(p)
                {
                    return true;
                }
                if let Some(target) = arrow_to {
                    let start = callout_anchor(layout.content_rect, *target);
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

    /// Control handles: 2 endpoints for Line/Arrow, 4 corners for Rect/Ellipse/Blur, and 2 independent points (text card + arrow tip) for Text callouts.
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
                let Some(target) = arrow_to else {
                    return Vec::new();
                };
                let layout =
                    text_box_and_content_rects(*pos, text, *size, *placement, *font, *box_size);
                vec![text_drag_handle(layout.content_rect), *target]
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
                if handle_idx == 0 {
                    let layout =
                        text_box_and_content_rects(*pos, text, *size, *placement, *font, *box_size);
                    let old_handle = text_drag_handle(layout.content_rect);
                    *pos += new_pos - old_handle;
                } else if handle_idx == 1
                    && let Some(t) = arrow_to
                {
                    *t = new_pos;
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

/// 2-pass Chaikin corner-cutting to turn mouse polyline points into smooth curves.
pub(super) fn smooth_chaikin(pts: &[Pos2]) -> Vec<Pos2> {
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

pub(super) fn font_for(tf: TextFont) -> Option<&'static FontArc> {
    static CACHE: [OnceLock<Option<FontArc>>; TextFont::COUNT] =
        [const { OnceLock::new() }; TextFont::COUNT];
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

pub(super) fn measure_line_width(line: &str, size: f32, tf: TextFont) -> f32 {
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
) -> TextLayout {
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
    TextLayout {
        box_rect: box_r,
        content_rect: content_r,
        wrapped_lines: lines,
        measured_size: Vec2::new(content_w, content_h),
    }
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
    text_box_and_content_rects(pos, text, size, placement, tf, box_size).content_rect
}

pub fn text_drag_handle(content_r: Rect) -> Pos2 {
    Pos2::new(content_r.center().x, content_r.top() - 6.0)
}

pub fn dash_params(stroke_width: f32, is_marker: bool) -> (f32, f32) {
    let w = stroke_width.max(1.0);
    if is_marker {
        ((w * 1.15).max(8.0), (w * 1.75).max(11.0))
    } else {
        ((w * 1.4).max(4.5), (w * 2.0).max(6.5))
    }
}

/// Split a polyline into tight alternating dash sub-polylines by arc length.
pub fn sample_dashed_polyline(pts: &[Pos2], dash_len: f32, gap_len: f32) -> Vec<Vec<Pos2>> {
    if pts.is_empty() {
        return Vec::new();
    }
    if pts.len() == 1 || dash_len <= 0.0 || gap_len <= 0.0 {
        return vec![pts.to_vec()];
    }

    let mut dashes: Vec<Vec<Pos2>> = Vec::new();
    let mut in_dash = true;
    let mut remaining = dash_len;
    let mut current: Vec<Pos2> = vec![pts[0]];

    for w in pts.windows(2) {
        let mut a = w[0];
        let b = w[1];
        let mut seg_rem = a.distance(b);
        if seg_rem < 1e-4 {
            continue;
        }
        let dir = (b - a) / seg_rem;
        while seg_rem > 1e-4 {
            if seg_rem <= remaining {
                if in_dash {
                    current.push(b);
                }
                remaining -= seg_rem;
                if remaining <= 1e-4 {
                    if in_dash {
                        if current.len() >= 2 {
                            dashes.push(std::mem::take(&mut current));
                        } else {
                            current.clear();
                        }
                        in_dash = false;
                        remaining = gap_len;
                    } else {
                        in_dash = true;
                        remaining = dash_len;
                        current = vec![b];
                    }
                }
                break;
            } else {
                let split = a + dir * remaining;
                if in_dash {
                    current.push(split);
                    if current.len() >= 2 {
                        dashes.push(std::mem::take(&mut current));
                    } else {
                        current.clear();
                    }
                    in_dash = false;
                    seg_rem -= remaining;
                    a = split;
                    remaining = gap_len;
                } else {
                    in_dash = true;
                    seg_rem -= remaining;
                    a = split;
                    remaining = dash_len;
                    current = vec![split];
                }
            }
        }
    }

    if in_dash && current.len() >= 2 {
        dashes.push(current);
    } else if dashes.is_empty() {
        dashes.push(vec![pts[0]]);
    }
    dashes
}

/// Shared arrowhead length formula used identically by GPU preview and CPU export.
pub fn arrow_head_len(stroke_width_px: f32, scale: f32, line_len_px: f32) -> f32 {
    (stroke_width_px * 3.0 + 10.0 * scale.max(0.5)).min(line_len_px * 0.55)
}

/// Shared rounded-rectangle corner radius formula (logical pixels) used identically by GPU preview and CPU export.
pub fn rect_corner_radius(thickness_logical: f32, width_logical: f32, height_logical: f32) -> f32 {
    (thickness_logical * 1.2)
        .clamp(4.0, 14.0)
        .min(width_logical.abs() * 0.45)
        .min(height_logical.abs() * 0.45)
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

pub(super) fn contrasting_fg(c: Color32) -> Color32 {
    let lum = 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32;
    if lum > 165.0 {
        Color32::from_rgb(8, 20, 30)
    } else {
        Color32::WHITE
    }
}

pub(super) fn ellipse_points(rect: Rect, segments: usize) -> Vec<Pos2> {
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

pub(super) fn to_rgba(c: Color32, alpha: u8) -> Rgba<u8> {
    Rgba([c.r(), c.g(), c.b(), alpha])
}
