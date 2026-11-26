use ab_glyph::FontArc;
use egui::{Color32, Pos2, Rect, Vec2};
use image::{Rgba, RgbaImage};

use crate::settings::{AppSettings, TextFont};

use super::raster::{
    pixelate_region, render_export, smooth_blur_region, smooth_blur_rounded_region,
};
use super::*;

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
            dashed: false,
        },
        Annotation::Marker {
            points: vec![Pos2::new(15.0, 15.0), Pos2::new(110.0, 100.0)],
            color: Color32::YELLOW,
            thickness: 5.0,
            dashed: false,
        },
        Annotation::Line {
            start: Pos2::new(5.0, 5.0),
            end: Pos2::new(80.0, 80.0),
            color: Color32::GREEN,
            thickness: 4.0,
            dashed: false,
        },
        Annotation::Arrow {
            start: Pos2::new(20.0, 90.0),
            end: Pos2::new(90.0, 20.0),
            color: Color32::RED,
            thickness: 5.0,
            dashed: false,
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
        dashed: false,
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
        dashed: false,
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
        dashed: false,
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
            dashed: false,
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
            dashed: false,
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

#[test]
fn dashed_polyline_splitting_and_tight_stride() {
    let (dash_len, gap_len) = dash_params(4.0, false);
    assert!(dash_len >= 4.5 && gap_len > dash_len);

    let dashes = sample_dashed_polyline(
        &[Pos2::new(0.0, 0.0), Pos2::new(50.0, 0.0)],
        10.0,
        10.0,
    );
    assert_eq!(dashes.len(), 3);
    assert_eq!(dashes[0], vec![Pos2::new(0.0, 0.0), Pos2::new(10.0, 0.0)]);
    assert_eq!(dashes[1], vec![Pos2::new(20.0, 0.0), Pos2::new(30.0, 0.0)]);
    assert_eq!(dashes[2], vec![Pos2::new(40.0, 0.0), Pos2::new(50.0, 0.0)]);

    let single = sample_dashed_polyline(&[Pos2::new(5.0, 7.0)], 10.0, 10.0);
    assert_eq!(single, vec![vec![Pos2::new(5.0, 7.0)]]);
}

#[test]
fn golden_dashed_strokes_leave_visible_gaps_and_uniform_marker_alpha() {
    let bg = Rgba([255, 255, 255, 255]);
    let src = RgbaImage::from_pixel(140, 140, bg);
    let anns = vec![
        Annotation::Line {
            start: Pos2::new(10.0, 20.0),
            end: Pos2::new(130.0, 20.0),
            color: Color32::from_rgb(255, 0, 0),
            thickness: 4.0,
            dashed: true,
        },
        Annotation::Arrow {
            start: Pos2::new(10.0, 50.0),
            end: Pos2::new(130.0, 50.0),
            color: Color32::from_rgb(0, 200, 0),
            thickness: 4.0,
            dashed: true,
        },
        Annotation::Pen {
            points: vec![
                Pos2::new(10.0, 80.0),
                Pos2::new(70.0, 80.0),
                Pos2::new(130.0, 80.0),
            ],
            color: Color32::from_rgb(0, 140, 255),
            thickness: 4.0,
            dashed: true,
        },
        Annotation::Marker {
            points: vec![Pos2::new(10.0, 115.0), Pos2::new(130.0, 115.0)],
            color: Color32::from_rgb(0, 0, 255),
            thickness: 3.0,
            dashed: true,
        },
    ];
    let out = render_export(
        &src,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(140.0, 140.0)),
        1.0,
        &anns,
    );
    for (label, y) in [("line", 20u32), ("arrow", 50), ("pen", 80), ("marker", 115)] {
        let hit = (15..120).filter(|&x| *out.get_pixel(x, y) != bg).count();
        let gap = (15..120).filter(|&x| *out.get_pixel(x, y) == bg).count();
        assert!(hit > 10 && gap > 10, "{label}: hit={hit}, gap={gap}");
    }
    // Arrowhead tip remains solid green
    assert!(out.get_pixel(128, 50)[1] > 180 && out.get_pixel(128, 50)[0] < 30);

    // Every lit marker pixel has identical alpha-blended color (no joint double-darkening)
    let marker_colors: Vec<Rgba<u8>> = (15..120)
        .map(|x| *out.get_pixel(x, 115))
        .filter(|p| *p != bg)
        .collect();
    assert!(!marker_colors.is_empty());
    let first = marker_colors[0];
    assert!(marker_colors.iter().all(|&p| p == first));
}

#[test]
fn text_callout_has_two_independent_line_handles() {
    let mut callout = Annotation::Text {
        pos: Pos2::new(100.0, 80.0),
        text: "Note".to_string(),
        color: Color32::WHITE,
        size: 20.0,
        arrow_to: Some(Pos2::new(20.0, 30.0)),
        thickness: 4.0,
        placement: TextPlacement::LeftTop,
        font: TextFont::Roboto,
        box_size: None,
    };
    let cps = callout.control_points();
    assert_eq!(cps.len(), 2);
    assert_eq!(cps[1], Pos2::new(20.0, 30.0));
    assert!(callout.bounding_rect().contains(cps[0]));
    assert!(callout.bounding_rect().contains(cps[1]));

    let target_cp0 = Pos2::new(200.0, 140.0);
    callout.move_control_point(0, target_cp0);
    let after_move_text = callout.control_points();
    assert_eq!(after_move_text.len(), 2);
    assert!((after_move_text[0].x - target_cp0.x).abs() < 0.01);
    assert!((after_move_text[0].y - target_cp0.y).abs() < 0.01);
    assert_eq!(after_move_text[1], Pos2::new(20.0, 30.0));

    let pos_before = match &callout {
        Annotation::Text { pos, .. } => *pos,
        _ => unreachable!(),
    };
    callout.move_control_point(1, Pos2::new(45.0, 65.0));
    match &callout {
        Annotation::Text { pos, arrow_to, .. } => {
            assert_eq!(*pos, pos_before);
            assert_eq!(*arrow_to, Some(Pos2::new(45.0, 65.0)));
        }
        _ => unreachable!(),
    }

    let plain = Annotation::Text {
        pos: Pos2::new(10.0, 10.0),
        text: "Plain".to_string(),
        color: Color32::WHITE,
        size: 20.0,
        arrow_to: None,
        thickness: 4.0,
        placement: TextPlacement::LeftTop,
        font: TextFont::Roboto,
        box_size: None,
    };
    assert!(plain.control_points().is_empty());
}

#[test]
fn rect_corner_radius_matches_gpu_and_cpu_bounds() {
    assert_eq!(rect_corner_radius(4.0, 100.0, 80.0), 4.8);
    assert_eq!(rect_corner_radius(2.0, 100.0, 80.0), 4.0);
    // Clamped to 14.0 upper bound on large rects (shared by GPU & CPU)
    assert_eq!(rect_corner_radius(20.0, 200.0, 200.0), 14.0);
    // Shrinks safely on narrow rects without self-intersection
    assert!((rect_corner_radius(10.0, 12.0, 100.0) - 5.4).abs() < 1e-4);
}

#[test]
fn rounded_text_blur_backdrop_preserves_crisp_outer_corners() {
    let mut src = RgbaImage::new(64, 64);
    for y in 0..64u32 {
        for x in 0..64u32 {
            let v = if (x + y) % 2 == 0 { 10u8 } else { 245u8 };
            src.put_pixel(x, y, Rgba([v, v, v, 255]));
        }
    }
    let mut out = src.clone();
    smooth_blur_rounded_region(&mut out, 4, 4, 60, 60, 10, 12.0);

    // Center pixel inside box is smoothly averaged by 2-pass blur
    let center = out.get_pixel(32, 32)[0];
    assert!(center > 90 && center < 165, "center={center}");

    // Pixel at corner (4, 4) lies inside rect [4..60] but outside 12px rounded corner -> stays exact original
    assert_eq!(*out.get_pixel(4, 4), *src.get_pixel(4, 4));
    assert_eq!(*out.get_pixel(59, 4), *src.get_pixel(59, 4));
}

#[test]
fn antialiased_cpu_rasterization_blends_edges_smoothly() {
    let bg = Rgba([0, 0, 0, 255]);
    let src = RgbaImage::from_pixel(100, 100, bg);
    let anns = vec![Annotation::Ellipse {
        rect: Rect::from_min_max(Pos2::new(20.0, 20.0), Pos2::new(80.0, 80.0)),
        color: Color32::from_rgb(200, 100, 50),
        thickness: 4.0,
        filled: true,
    }];
    let out = render_export(
        &src,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 100.0)),
        1.0,
        &anns,
    );

    assert_eq!(*out.get_pixel(50, 50), Rgba([200, 100, 50, 255]));
    let soft_edges = (18..=82)
        .flat_map(|y| (18..=82).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let r = out.get_pixel(x, y)[0];
            r > 5 && r < 195
        })
        .count();
    assert!(soft_edges >= 12, "expected anti-aliased edge pixels, got {soft_edges}");
}

#[test]
fn dashed_marker_flat_caps_preserve_gap_and_uniform_alpha() {
    let bg = Rgba([255, 255, 255, 255]);
    let src = RgbaImage::from_pixel(180, 60, bg);
    let anns = vec![Annotation::Marker {
        points: vec![Pos2::new(10.0, 30.0), Pos2::new(170.0, 30.0)],
        color: Color32::from_rgb(0, 120, 255),
        thickness: 6.0,
        dashed: true,
    }];
    let (dash_len, gap_len) = dash_params(6.0 * 3.8, true);
    let out = render_export(
        &src,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(180.0, 60.0)),
        1.0,
        &anns,
    );

    // Center of first gap remains untouched background (no protruding round caps)
    let gap_mid_x = (10.0 + dash_len + gap_len * 0.5).round() as u32;
    assert_eq!(*out.get_pixel(gap_mid_x, 30), bg);

    // Centerline pixels across all dashes keep identical alpha blend
    let lit: Vec<Rgba<u8>> = (12..165)
        .map(|x| *out.get_pixel(x, 30))
        .filter(|p| *p != bg)
        .collect();
    assert!(!lit.is_empty());
    assert!(lit.iter().all(|&p| p == lit[0]));
}

#[test]
fn text_callout_anchor_consistent_across_hit_test_and_handles() {
    let pos = Pos2::new(120.0, 100.0);
    let target = Pos2::new(30.0, 40.0);
    let ann = Annotation::Text {
        pos,
        text: "Callout".to_string(),
        color: Color32::WHITE,
        size: 18.0,
        arrow_to: Some(target),
        thickness: 3.0,
        placement: TextPlacement::LeftTop,
        font: TextFont::Roboto,
        box_size: None,
    };
    let layout =
        text_box_and_content_rects(pos, "Callout", 18.0, TextPlacement::LeftTop, TextFont::Roboto, None);
    let anchor = callout_anchor(layout.content_rect, target);
    let midpoint = Pos2::new((anchor.x + target.x) * 0.5, (anchor.y + target.y) * 0.5);

    assert_eq!(ann.control_points().len(), 2);
    assert!(ann.hit_test(anchor));
    assert!(ann.hit_test(midpoint));
}

#[test]
fn arrow_head_len_scales_proportionally_for_hidpi() {
    let base = arrow_head_len(4.0, 1.0, 200.0);
    let hidpi = arrow_head_len(4.0 * 2.0, 2.0, 400.0);
    assert!((hidpi - base * 2.0).abs() < 1e-4, "base={base}, hidpi={hidpi}");
}

#[test]
fn thin_stroke_and_step_border_export_accurately() {
    let bg = Rgba([0, 0, 0, 255]);
    let src = RgbaImage::from_pixel(100, 100, bg);
    let anns = vec![
        Annotation::Line {
            start: Pos2::new(10.0, 20.0),
            end: Pos2::new(90.0, 20.0),
            color: Color32::WHITE,
            thickness: 1.5,
            dashed: false,
        },
        Annotation::Step {
            pos: Pos2::new(50.0, 65.0),
            number: 1,
            color: Color32::from_rgb(10, 40, 200),
            radius: 10.0,
        },
    ];
    let out = render_export(
        &src,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 100.0)),
        1.0,
        &anns,
    );

    // 1.5px line centered on y=20.0 straddles pixels y=19 and y=20 equally (0.75 AA coverage each -> ~191)
    assert_eq!(out.get_pixel(50, 19)[0], 191);
    assert_eq!(out.get_pixel(50, 20)[0], 191);
    assert_eq!(out.get_pixel(50, 22)[0], 0);

    // Step outer white rim matches 0.75 * scale centered 1.5px border (at dx=12px it is background)
    assert_eq!(*out.get_pixel(62, 65), bg);
}

#[test]
fn paint_annotations_headless_emits_dashed_segments_and_callout_handles() {
    use std::collections::HashMap;
    use std::sync::Arc;

    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "roboto_test".to_owned(),
        Arc::new(egui::FontData::from_static(ROBOTO_FONT_BYTES)),
    );
    fonts.families.insert(
        egui::FontFamily::Name(TextFont::Roboto.egui_family().into()),
        vec!["roboto_test".to_owned()],
    );
    ctx.set_fonts(fonts);

    let anns = vec![
        Annotation::Line {
            start: Pos2::new(10.0, 20.0),
            end: Pos2::new(130.0, 20.0),
            color: Color32::RED,
            thickness: 4.0,
            dashed: true,
        },
        Annotation::Text {
            pos: Pos2::new(160.0, 90.0),
            text: "Callout".to_string(),
            color: Color32::WHITE,
            size: 18.0,
            arrow_to: Some(Pos2::new(60.0, 60.0)),
            thickness: 3.0,
            placement: TextPlacement::LeftTop,
            font: TextFont::Roboto,
            box_size: None,
        },
    ];

    let empty_blurs = HashMap::new();
    let full_output = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            paint_annotations(
                ui.painter(),
                &anns,
                Some(1),
                None,
                Vec2::ZERO,
                2.0,
                &empty_blurs,
                None,
                Vec2::new(400.0, 300.0),
                Color32::from_rgb(94, 234, 212),
            );
        });
    });

    // At least: dashed segments (>2), text pill + arrow + shadow + text, selection rect + 2x2 handle circles
    assert!(
        full_output.shapes.len() >= 12,
        "expected >= 12 GPU shapes, got {}",
        full_output.shapes.len()
    );
}

#[test]
fn paint_vs_raster_parity_all_tools() {
    use std::collections::HashMap;
    use std::sync::Arc;

    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "roboto_parity".to_owned(),
        Arc::new(egui::FontData::from_static(ROBOTO_FONT_BYTES)),
    );
    fonts.families.insert(
        egui::FontFamily::Name(TextFont::Roboto.egui_family().into()),
        vec!["roboto_parity".to_owned()],
    );
    ctx.set_fonts(fonts);

    let mut src = RgbaImage::new(200, 200);
    for y in 0..200u32 {
        for x in 0..200u32 {
            let c = if (x / 4 + y / 4) % 2 == 0 {
                Rgba([20, 25, 35, 255])
            } else {
                Rgba([60, 75, 95, 255])
            };
            src.put_pixel(x, y, c);
        }
    }
    let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 200.0));

    let cases: Vec<(&str, Annotation, f32)> = vec![
        (
            "pen",
            Annotation::Pen {
                points: vec![Pos2::new(30.0, 40.0), Pos2::new(150.0, 120.0)],
                color: Color32::RED,
                thickness: 4.0,
                dashed: false,
            },
            4.0,
        ),
        (
            "marker",
            Annotation::Marker {
                points: vec![Pos2::new(35.0, 60.0), Pos2::new(160.0, 60.0)],
                color: Color32::YELLOW,
                thickness: 4.0,
                dashed: false,
            },
            // Solid marker uses rounded caps on CPU export (radius ~7.6px) vs flat caps in egui
            8.0,
        ),
        (
            "marker_dashed",
            Annotation::Marker {
                points: vec![Pos2::new(35.0, 60.0), Pos2::new(160.0, 60.0)],
                color: Color32::YELLOW,
                thickness: 4.0,
                dashed: true,
            },
            4.0,
        ),
        (
            "line",
            Annotation::Line {
                start: Pos2::new(25.0, 30.0),
                end: Pos2::new(170.0, 140.0),
                color: Color32::GREEN,
                thickness: 4.0,
                dashed: false,
            },
            4.0,
        ),
        (
            "arrow",
            Annotation::Arrow {
                start: Pos2::new(30.0, 100.0),
                end: Pos2::new(170.0, 100.0),
                color: Color32::RED,
                thickness: 4.0,
                dashed: false,
            },
            4.0,
        ),
        (
            "arrow_dashed",
            Annotation::Arrow {
                start: Pos2::new(30.0, 120.0),
                end: Pos2::new(170.0, 120.0),
                color: Color32::LIGHT_BLUE,
                thickness: 4.0,
                dashed: true,
            },
            4.0,
        ),
        (
            "rect",
            Annotation::Rect {
                rect: Rect::from_min_max(Pos2::new(40.0, 40.0), Pos2::new(150.0, 130.0)),
                color: Color32::BLUE,
                thickness: 4.0,
                filled: false,
            },
            4.0,
        ),
        (
            "ellipse",
            Annotation::Ellipse {
                rect: Rect::from_min_max(Pos2::new(40.0, 50.0), Pos2::new(160.0, 150.0)),
                color: Color32::WHITE,
                thickness: 4.0,
                filled: false,
            },
            4.0,
        ),
        (
            "step",
            Annotation::Step {
                pos: Pos2::new(100.0, 100.0),
                number: 3,
                color: Color32::from_rgb(20, 180, 170),
                radius: 14.0,
            },
            4.0,
        ),
        (
            "text_callout",
            Annotation::Text {
                pos: Pos2::new(110.0, 60.0),
                text: "Note".to_string(),
                color: Color32::WHITE,
                size: 18.0,
                arrow_to: Some(Pos2::new(40.0, 140.0)),
                thickness: 3.0,
                placement: TextPlacement::LeftTop,
                font: TextFont::Roboto,
                box_size: None,
            },
            7.0,
        ),
    ];

    let empty_blurs = HashMap::new();
    for (name, ann, tol) in cases {
        let single = [ann];
        let raw_input = egui::RawInput {
            screen_rect: Some(bounds),
            ..Default::default()
        };
        let full_output = ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    paint_annotations(
                        ui.painter(),
                        &single,
                        None,
                        None,
                        Vec2::ZERO,
                        1.0,
                        &empty_blurs,
                        None,
                        Vec2::new(200.0, 200.0),
                        Color32::from_rgb(94, 234, 212),
                    );
                });
        });
        let clipped = ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        let mut gpu_bbox = Rect::NOTHING;
        for prim in clipped {
            if let egui::epaint::Primitive::Mesh(mesh) = prim.primitive {
                for v in &mesh.vertices {
                    gpu_bbox.extend_with(v.pos);
                }
            }
        }
        assert!(gpu_bbox.is_positive(), "{name}: GPU emitted empty mesh");

        let out = render_export(&src, bounds, 1.0, &single);
        let mut cpu_bbox = Rect::NOTHING;
        for y in 0..200u32 {
            for x in 0..200u32 {
                if out.get_pixel(x, y) != src.get_pixel(x, y) {
                    cpu_bbox.extend_with(Pos2::new(x as f32, y as f32));
                }
            }
        }
        assert!(cpu_bbox.is_positive(), "{name}: CPU raster emitted no pixels");

        assert!(
            (gpu_bbox.min.x - cpu_bbox.min.x).abs() <= tol
                && (gpu_bbox.min.y - cpu_bbox.min.y).abs() <= tol
                && (gpu_bbox.max.x - cpu_bbox.max.x).abs() <= tol
                && (gpu_bbox.max.y - cpu_bbox.max.y).abs() <= tol,
            "{name}: GPU bbox {gpu_bbox:?} vs CPU bbox {cpu_bbox:?} exceeded tol {tol}"
        );
    }
}
