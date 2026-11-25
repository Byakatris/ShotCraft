use egui::{Color32, Pos2, Rect, Stroke, Vec2};

pub fn draw_move_icon(p: &egui::Painter, r: Rect, c: Color32) {
    let pt = |x: f32, y: f32| Pos2::new(r.left() + r.width() * x, r.top() + r.height() * y);
    let top = pt(0.24, 0.14);
    let bot = pt(0.24, 0.80);
    let notch = pt(0.42, 0.62);
    let right = pt(0.76, 0.54);
    let tail_l = pt(0.54, 0.86);
    let tail_r = pt(0.65, 0.81);
    let stem = pt(0.50, 0.56);

    p.add(egui::Shape::convex_polygon(vec![top, bot, notch, right], c, Stroke::NONE));
    p.add(egui::Shape::convex_polygon(
        vec![notch, tail_l, tail_r, stem],
        c,
        Stroke::NONE,
    ));
}

pub fn draw_pen_icon(p: &egui::Painter, r: Rect, c: Color32, dashed: bool) {
    let pt = |x: f32, y: f32| Pos2::new(r.left() + r.width() * x, r.top() + r.height() * y);

    // Sharp pencil tip at bottom-left
    let tip = pt(0.15, 0.85);
    let cone_a = pt(0.21, 0.65);
    let cone_b = pt(0.35, 0.79);
    p.add(egui::Shape::closed_line(
        vec![tip, cone_a, cone_b],
        Stroke::new(1.5_f32, c),
    ));
    // Dark lead dot at the very tip
    p.add(egui::Shape::convex_polygon(
        vec![tip, pt(0.18, 0.75), pt(0.25, 0.82)],
        c,
        Stroke::NONE,
    ));

    // Main angled pencil barrel (2 crisp parallel lines + collar)
    let body_a = pt(0.22, 0.64);
    let body_b = pt(0.36, 0.78);
    let top_a = pt(0.67, 0.19);
    let top_b = pt(0.81, 0.33);
    p.add(egui::Shape::closed_line(
        vec![body_a, top_a, top_b, body_b],
        Stroke::new(1.7_f32, c),
    ));
    // Ferrule / eraser band near top-right
    p.line_segment([pt(0.60, 0.26), pt(0.74, 0.40)], Stroke::new(1.5_f32, c));

    if dashed {
        let stroke = Stroke::new(1.7_f32, c);
        p.line_segment([pt(0.29, 0.87), pt(0.41, 0.87)], stroke);
        p.line_segment([pt(0.51, 0.87), pt(0.63, 0.87)], stroke);
        p.line_segment([pt(0.73, 0.87), pt(0.85, 0.87)], stroke);
    }
}

/// Wide angled chisel-tip highlighter marker (without underline).
pub fn draw_marker_icon(p: &egui::Painter, r: Rect, c: Color32, dashed: bool) {
    let pt = |x: f32, y: f32| Pos2::new(r.left() + r.width() * x, r.top() + r.height() * y);

    // Wide angled rectangular barrel (45 deg)
    let barrel = vec![
        pt(0.30, 0.52),
        pt(0.67, 0.15),
        pt(0.87, 0.35),
        pt(0.50, 0.72),
    ];
    p.add(egui::Shape::convex_polygon(
        barrel,
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 205),
        Stroke::NONE,
    ));

    // Slanted wide chisel tip at bottom-left
    let tip = vec![
        pt(0.27, 0.55),
        pt(0.47, 0.75),
        pt(0.25, 0.86),
        pt(0.14, 0.79),
    ];
    p.add(egui::Shape::convex_polygon(tip, c, Stroke::NONE));

    if dashed {
        let mark_col = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 190);
        let stroke = Stroke::new(2.8_f32, mark_col);
        p.line_segment([pt(0.36, 0.85), pt(0.54, 0.85)], stroke);
        p.line_segment([pt(0.68, 0.85), pt(0.86, 0.85)], stroke);
    }
}

pub fn draw_line_icon(p: &egui::Painter, r: Rect, c: Color32, dashed: bool) {
    let a = Pos2::new(r.left() + r.width() * 0.20, r.bottom() - r.height() * 0.20);
    let b = Pos2::new(r.right() - r.width() * 0.20, r.top() + r.height() * 0.20);
    if dashed {
        let d = b - a;
        let stroke = Stroke::new(2.1_f32, c);
        p.line_segment([a, a + d * 0.22], stroke);
        p.line_segment([a + d * 0.39, a + d * 0.61], stroke);
        p.line_segment([a + d * 0.78, b], stroke);
    } else {
        p.line_segment([a, b], Stroke::new(2.2_f32, c));
    }
    p.circle_filled(a, 2.3, c);
    p.circle_filled(b, 2.3, c);
}

pub fn draw_arrow_icon(p: &egui::Painter, r: Rect, c: Color32, dashed: bool) {
    let a = Pos2::new(r.left() + r.width() * 0.20, r.bottom() - r.height() * 0.20);
    let b = Pos2::new(r.right() - r.width() * 0.18, r.top() + r.height() * 0.18);
    if dashed {
        let d = b - a;
        let stroke = Stroke::new(2.1_f32, c);
        p.line_segment([a, a + d * 0.24], stroke);
        p.line_segment([a + d * 0.44, a + d * 0.68], stroke);
        p.line_segment([a + d * 0.84, b], stroke);
    } else {
        p.line_segment([a, b], Stroke::new(2.2_f32, c));
    }
    p.line_segment(
        [b, Pos2::new(b.x - r.width() * 0.34, b.y)],
        Stroke::new(2.2_f32, c),
    );
    p.line_segment(
        [b, Pos2::new(b.x, b.y + r.height() * 0.34)],
        Stroke::new(2.2_f32, c),
    );
}

pub fn draw_text_icon(p: &egui::Painter, r: Rect, c: Color32) {
    let top_y = r.top() + r.height() * 0.22;
    let bot_y = r.bottom() - r.height() * 0.18;
    let cx = r.center().x;
    let left_x = r.left() + r.width() * 0.20;
    let right_x = r.right() - r.width() * 0.20;
    p.line_segment([Pos2::new(left_x, top_y), Pos2::new(right_x, top_y)], Stroke::new(2.4_f32, c));
    p.line_segment([Pos2::new(left_x, top_y), Pos2::new(left_x, top_y + 3.0)], Stroke::new(1.8_f32, c));
    p.line_segment([Pos2::new(right_x, top_y), Pos2::new(right_x, top_y + 3.0)], Stroke::new(1.8_f32, c));
    p.line_segment([Pos2::new(cx, top_y), Pos2::new(cx, bot_y)], Stroke::new(2.4_f32, c));
    p.line_segment(
        [Pos2::new(cx - r.width() * 0.16, bot_y), Pos2::new(cx + r.width() * 0.16, bot_y)],
        Stroke::new(2.0_f32, c),
    );
}

pub fn draw_ellipse_icon(p: &egui::Painter, r: Rect, c: Color32, filled: bool) {
    let radius = r.width().min(r.height()) * 0.36;
    if filled {
        p.circle_filled(r.center(), radius, c);
    } else {
        p.circle_stroke(r.center(), radius, Stroke::new(2.1_f32, c));
    }
}

pub fn draw_rect_icon(p: &egui::Painter, r: Rect, c: Color32, filled: bool) {
    let box_rect = Rect::from_center_size(r.center(), Vec2::splat(r.width().min(r.height()) * 0.70));
    if filled {
        p.rect_filled(box_rect, 3.0, c);
    } else {
        p.rect_stroke(box_rect, 3.0, Stroke::new(2.1_f32, c), egui::StrokeKind::Middle);
    }
}

pub fn draw_step_icon(p: &egui::Painter, r: Rect, c: Color32) {
    let rad = r.width().min(r.height()) * 0.39;
    p.circle_stroke(r.center(), rad, Stroke::new(1.9_f32, c));
    p.text(
        r.center(),
        egui::Align2::CENTER_CENTER,
        "1",
        egui::FontId::proportional(11.5),
        c,
    );
}

/// Google Material `blur_on` style tapered dot grid, or mosaic grid when `pixelate` is true.
pub fn draw_blur_icon(p: &egui::Painter, r: Rect, c: Color32, pixelate: bool) {
    if pixelate {
        let cell = r.width().min(r.height()) * 0.25;
        let gap = cell * 0.16;
        let cx = r.center().x;
        let cy = r.center().y;
        let sz = Vec2::splat((cell - gap).max(2.0));
        for dy in [-1.0_f32, 0.0, 1.0] {
            for dx in [-1.0_f32, 0.0, 1.0] {
                let alpha = if ((dx + dy).abs() as i32) % 2 == 0 {
                    240
                } else {
                    135
                };
                let center = Pos2::new(cx + dx * cell, cy + dy * cell);
                p.rect_filled(
                    Rect::from_center_size(center, sz),
                    1.0,
                    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha),
                );
            }
        }
        return;
    }

    let step = r.width().min(r.height()) * 0.27;
    let cx = r.center().x;
    let cy = r.center().y;
    for dy in [-1.0_f32, 0.0, 1.0] {
        for dx in [-1.0_f32, 0.0, 1.0] {
            let dist = dx.abs() + dy.abs();
            let rad = if dist == 0.0 {
                2.5
            } else if dist == 1.0 {
                2.0
            } else {
                1.4
            };
            let alpha = if dist == 0.0 {
                255
            } else if dist == 1.0 {
                205
            } else {
                135
            };
            p.circle_filled(
                Pos2::new(cx + dx * step, cy + dy * step),
                rad,
                Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha),
            );
        }
    }
}

/// Smooth Material `undo` curved arrow.
pub fn draw_undo_icon(p: &egui::Painter, r: Rect, c: Color32) {
    let cx = r.center().x + 1.0;
    let top_y = r.top() + r.height() * 0.32;
    let bot_y = r.bottom() - r.height() * 0.26;
    let left_x = r.left() + r.width() * 0.18;
    let right_x = r.right() - r.width() * 0.18;

    let arc = vec![
        Pos2::new(left_x, top_y),
        Pos2::new(cx + 2.0, top_y),
        Pos2::new(right_x, top_y + 3.0),
        Pos2::new(right_x, (top_y + bot_y) * 0.5 + 1.0),
        Pos2::new(cx + 2.0, bot_y),
        Pos2::new(cx - 3.0, bot_y),
    ];
    p.add(egui::Shape::line(arc, Stroke::new(2.0_f32, c)));
    p.line_segment(
        [Pos2::new(left_x, top_y), Pos2::new(left_x + 4.5, top_y - 4.0)],
        Stroke::new(2.0_f32, c),
    );
    p.line_segment(
        [Pos2::new(left_x, top_y), Pos2::new(left_x + 4.5, top_y + 4.0)],
        Stroke::new(2.0_f32, c),
    );
}

/// Smooth Material `redo` curved arrow (mirrored undo).
pub fn draw_redo_icon(p: &egui::Painter, r: Rect, c: Color32) {
    let cx = r.center().x - 1.0;
    let top_y = r.top() + r.height() * 0.32;
    let bot_y = r.bottom() - r.height() * 0.26;
    let left_x = r.left() + r.width() * 0.18;
    let right_x = r.right() - r.width() * 0.18;

    let arc = vec![
        Pos2::new(right_x, top_y),
        Pos2::new(cx - 2.0, top_y),
        Pos2::new(left_x, top_y + 3.0),
        Pos2::new(left_x, (top_y + bot_y) * 0.5 + 1.0),
        Pos2::new(cx - 2.0, bot_y),
        Pos2::new(cx + 3.0, bot_y),
    ];
    p.add(egui::Shape::line(arc, Stroke::new(2.0_f32, c)));
    p.line_segment(
        [Pos2::new(right_x, top_y), Pos2::new(right_x - 4.5, top_y - 4.0)],
        Stroke::new(2.0_f32, c),
    );
    p.line_segment(
        [Pos2::new(right_x, top_y), Pos2::new(right_x - 4.5, top_y + 4.0)],
        Stroke::new(2.0_f32, c),
    );
}

/// Material `content_copy` icon.
pub fn draw_copy_icon(p: &egui::Painter, r: Rect, c: Color32) {
    let w = r.width() * 0.50;
    let h = r.height() * 0.58;
    let back = Rect::from_min_size(
        Pos2::new(r.left() + r.width() * 0.16, r.top() + r.height() * 0.12),
        Vec2::new(w, h),
    );
    let front = Rect::from_min_size(
        Pos2::new(r.left() + r.width() * 0.34, r.top() + r.height() * 0.30),
        Vec2::new(w, h),
    );
    p.rect_stroke(
        back,
        2.5,
        Stroke::new(1.6_f32, Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 185)),
        egui::StrokeKind::Middle,
    );
    p.rect_filled(front, 2.5, Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 50));
    p.rect_stroke(front, 2.5, Stroke::new(1.7_f32, c), egui::StrokeKind::Middle);
}

/// Material `download` / save-to-desktop icon.
pub fn draw_save_icon(p: &egui::Painter, r: Rect, c: Color32) {
    let cx = r.center().x;
    let top = Pos2::new(cx, r.top() + r.height() * 0.16);
    let mid = Pos2::new(cx, r.bottom() - r.height() * 0.34);
    p.line_segment([top, mid], Stroke::new(2.0_f32, c));
    p.line_segment(
        [mid, Pos2::new(cx - 4.5, mid.y - 4.5)],
        Stroke::new(2.0_f32, c),
    );
    p.line_segment(
        [mid, Pos2::new(cx + 4.5, mid.y - 4.5)],
        Stroke::new(2.0_f32, c),
    );
    let y = r.bottom() - r.height() * 0.18;
    p.add(egui::Shape::line(
        vec![
            Pos2::new(r.left() + r.width() * 0.18, y - 3.5),
            Pos2::new(r.left() + r.width() * 0.18, y),
            Pos2::new(r.right() - r.width() * 0.18, y),
            Pos2::new(r.right() - r.width() * 0.18, y - 3.5),
        ],
        Stroke::new(1.9_f32, c),
    ));
}

pub fn draw_close_icon(p: &egui::Painter, r: Rect, c: Color32) {
    let s = r.shrink(r.width() * 0.22);
    p.line_segment([s.left_top(), s.right_bottom()], Stroke::new(2.0_f32, c));
    p.line_segment([s.right_top(), s.left_bottom()], Stroke::new(2.0_f32, c));
}
