use egui::{Color32, CornerRadius, Pos2, Rect, Stroke, TextureId, Vec2};

use crate::annotations::{callout_anchor, draw_egui_arrow, Annotation, TextPlacement, Tool};
use crate::icons;
use crate::material::{
    draw_m3_panel, draw_rounded_texture_uv, m3_action_button, m3_color_slider,
    m3_color_slider_vertical, m3_size_slider, m3_size_slider_vertical, m3_tool_button,
    spring_animate, ThemeColors,
};
use crate::settings::{Strings, TextFont, ToolbarLayout};

use super::{DragAction, OverlayOutcome, OverlayShared, PendingText};

pub(super) fn draw_text_callout_preview(
    ctx: &egui::Context,
    painter: &egui::Painter,
    shared: &OverlayShared,
    offset: Vec2,
    view_scale: f32,
    text_blur_tex: Option<TextureId>,
    virtual_size: Vec2,
) {
    let map_pt = |p: Pos2| -> Pos2 {
        let s = p + offset;
        Pos2::new(s.x * view_scale, s.y * view_scale)
    };
    let map_rect = |r: Rect| -> Rect { Rect::from_min_max(map_pt(r.min), map_pt(r.max)) };

    if matches!(shared.drag_action, Some(DragAction::TextCallout)) {
        if let (Some(start), Some(curr)) = (shared.drag_start, shared.drag_curr) {
            let box_w = 104.0;
            let box_h = shared.text_font_size() + 10.0;
            let placement = TextPlacement::from_drag(start, curr);
            let preview_box = placement.box_rect(curr, box_w, box_h);
            painter.rect_stroke(
                map_rect(preview_box),
                8.0 * view_scale,
                Stroke::new(1.4_f32, shared.active_color),
                egui::StrokeKind::Inside,
            );
            if start.distance(curr) >= 8.0 {
                let anchor = callout_anchor(preview_box, start);
                draw_egui_arrow(
                    painter,
                    map_pt(anchor),
                    map_pt(start),
                    Stroke::new(
                        (shared.thickness * 0.85).max(2.0) * view_scale,
                        shared.active_color,
                    ),
                );
            }
        }
    } else if let Some(pending) = &shared.pending_text {
        let (inner_w, inner_h, _, _, _) =
            pending_editor_metrics(ctx, pending, shared.thickness, shared.text_font);
        let frame_w = inner_w + 12.0;
        let frame_h = inner_h + 8.0;
        let live_box = pending.placement.box_rect(pending.pos, frame_w, frame_h);
        let bg_local = map_rect(live_box);
        if let Some(tex) = text_blur_tex {
            draw_rounded_texture_uv(
                painter,
                tex,
                virtual_size,
                live_box,
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
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 38)),
                egui::StrokeKind::Middle,
            );
        }
        if let Some(target) = pending.arrow_to {
            let anchor = callout_anchor(live_box, target);
            draw_egui_arrow(
                painter,
                map_pt(anchor),
                map_pt(target),
                Stroke::new(
                    (shared.thickness * 0.85).max(2.0) * view_scale,
                    shared.active_color,
                ),
            );
        }
    }
}

pub(super) fn draw_inline_text_editor(
    ctx: &egui::Context,
    shared: &mut OverlayShared,
    local_rect: Rect,
    offset: Vec2,
    view_scale: f32,
) {
    let Some(pending) = &mut shared.pending_text else {
        return;
    };
    let shifted = pending.pos + offset;
    let local_pt = Pos2::new(shifted.x * view_scale, shifted.y * view_scale);
    if !local_rect.expand(40.0).contains(local_pt) {
        return;
    }
    let default_thick = shared.thickness;
    let default_font = shared.text_font;
    let (box_w, box_h, font_size, edit_font, orig_wrap) =
        pending_editor_metrics(ctx, pending, default_thick, default_font);
    let scaled_w = box_w * view_scale;
    let scaled_h = box_h * view_scale;
    let frame_w = scaled_w + 12.0;
    let frame_h = scaled_h + 8.0;
    let editor_rect = pending.placement.box_rect(local_pt, frame_w, frame_h);
    let editor_pos = Pos2::new(
        editor_rect
            .min
            .x
            .clamp(8.0, (local_rect.max.x - frame_w - 8.0).max(8.0)),
        editor_rect
            .min
            .y
            .clamp(8.0, (local_rect.max.y - frame_h - 8.0).max(8.0)),
    );
    let h_align = pending.placement.h_align();
    let just_opened = pending.just_opened;
    pending.just_opened = false;

    egui::Area::new(egui::Id::new("inline_text_editor"))
        .fixed_pos(editor_pos)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(Color32::from_rgba_unmultiplied(
                    shared.theme.surface_container.r(),
                    shared.theme.surface_container.g(),
                    shared.theme.surface_container.b(),
                    110,
                ))
                .stroke(Stroke::new(1.4_f32, shared.theme.accent))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(6, 4))
                .show(ui, |ui| {
                    ui.set_min_width(scaled_w);
                    ui.set_max_width(scaled_w);
                    let edit = egui::TextEdit::multiline(&mut pending.buf)
                        .desired_width(orig_wrap.map_or(f32::INFINITY, |w| w * view_scale))
                        .desired_rows(1)
                        .font(egui::FontId::new(
                            font_size * view_scale,
                            egui::FontFamily::Name(edit_font.egui_family().into()),
                        ))
                        .horizontal_align(h_align)
                        .text_color(shared.active_color)
                        .frame(false);
                    let resp = ui.add(edit);
                    if just_opened {
                        resp.request_focus();
                    }
                });
        });
}

pub(super) fn draw_floating_toolbar(
    ctx: &egui::Context,
    shared: &mut OverlayShared,
    sel: Rect,
    local_rect: Rect,
    cmd_held: bool,
) -> Option<OverlayOutcome> {
    let is_vert = shared.toolbar_layout == ToolbarLayout::Vertical;
    let grip_pad = 10.0;
    let (bar_w, bar_h) = if is_vert {
        (96.0, 560.0 + grip_pad)
    } else {
        (592.0 + grip_pad, 84.0)
    };

    let (default_x, default_y) = if is_vert {
        let right_fits = sel.max.x + bar_w + 18.0 <= local_rect.max.x;
        let x = if right_fits {
            sel.max.x + 12.0
        } else if sel.min.x - bar_w - 18.0 >= 0.0 {
            sel.min.x - bar_w - 12.0
        } else {
            (local_rect.max.x - bar_w - 14.0).max(12.0)
        };
        let y = (sel.center().y - bar_h * 0.5)
            .clamp(12.0, (local_rect.max.y - bar_h - 12.0).max(12.0));
        (x, y)
    } else {
        let x = (sel.center().x - bar_w * 0.5)
            .clamp(12.0, (local_rect.max.x - bar_w - 12.0).max(12.0));
        let below = sel.max.y + bar_h + 26.0 <= local_rect.max.y;
        let y = if below {
            sel.max.y + 12.0
        } else if sel.min.y - bar_h - 18.0 >= 0.0 {
            sel.min.y - bar_h - 12.0
        } else {
            (local_rect.max.y - bar_h - 28.0).max(16.0)
        };
        (x, y)
    };

    let (tx, ty) = if let Some(pos) = shared.toolbar_pos {
        (
            pos.x.clamp(8.0, (local_rect.max.x - bar_w - 8.0).max(8.0)),
            pos.y.clamp(8.0, (local_rect.max.y - bar_h - 8.0).max(8.0)),
        )
    } else {
        (default_x, default_y)
    };

    let pop_id = egui::Id::new("toolbar_pop_in");
    if ctx.data(|d| d.get_temp::<(f32, f32)>(pop_id)).is_none() {
        ctx.data_mut(|d| d.insert_temp(pop_id, (0.0_f32, 0.0_f32)));
    }
    let pop_t = spring_animate(ctx, pop_id, 1.0, 0.28, 0.72);
    let pop_offset_y = (1.0 - pop_t) * 10.0;
    let bar_rect = Rect::from_min_size(Pos2::new(tx, ty + pop_offset_y), Vec2::new(bar_w, bar_h));
    let bar_radius = 14.0;
    let mut outcome: Option<OverlayOutcome> = None;

    egui::Area::new(egui::Id::new("m3_toolbar"))
        .fixed_pos(bar_rect.min)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            draw_m3_panel(ui.painter(), bar_rect, bar_radius, &shared.theme);

            let grip_rect = if is_vert {
                Rect::from_min_size(bar_rect.min, Vec2::new(bar_w, grip_pad + 4.0))
            } else {
                Rect::from_min_size(bar_rect.min, Vec2::new(grip_pad + 4.0, bar_h))
            };
            let grip_resp = ui.interact(grip_rect, egui::Id::new("toolbar_grip"), egui::Sense::drag());
            if grip_resp.dragged() {
                ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
                let d = grip_resp.drag_delta();
                shared.toolbar_pos = Some(Pos2::new(
                    (bar_rect.min.x + d.x)
                        .clamp(8.0, (local_rect.max.x - bar_w - 8.0).max(8.0)),
                    (bar_rect.min.y + d.y)
                        .clamp(8.0, (local_rect.max.y - bar_h - 8.0).max(8.0)),
                ));
            } else if grip_resp.hovered() {
                ctx.set_cursor_icon(egui::CursorIcon::Grab);
            }

            let grip_hov = spring_animate(
                ctx,
                egui::Id::new("toolbar_grip_hov"),
                if grip_resp.hovered() || grip_resp.dragged() {
                    1.0
                } else {
                    0.0
                },
                0.22,
                0.65,
            )
            .clamp(0.0, 1.2);
            let grip_alpha = (205.0 + 50.0 * grip_hov).round() as u8;
            let ac = shared.theme.accent;
            let grip_col = Color32::from_rgba_unmultiplied(ac.r(), ac.g(), ac.b(), grip_alpha);
            let half_w = 1.0 + 0.4 * grip_hov;
            let end_pad = 12.0 - 3.0 * grip_hov;
            let grip_pill = if is_vert {
                Rect::from_min_max(
                    Pos2::new(grip_rect.left() + end_pad, grip_rect.top() + 7.0 - half_w),
                    Pos2::new(grip_rect.right() - end_pad, grip_rect.top() + 7.0 + half_w),
                )
            } else {
                Rect::from_min_max(
                    Pos2::new(grip_rect.left() + 8.0 - half_w, grip_rect.top() + end_pad),
                    Pos2::new(grip_rect.left() + 8.0 + half_w, grip_rect.bottom() - end_pad),
                )
            };
            ui.painter()
                .rect_filled(grip_pill, CornerRadius::same(2), grip_col);

            let t = Strings::for_lang(shared.lang);
            let tip_ellipse = Strings::with_mod(t.tool_ellipse);
            let tip_rect = Strings::with_mod(t.tool_rect);
            let tip_blur = Strings::with_mod(t.tool_blur);

            let tools: [(Tool, &str, &str, fn(&egui::Painter, Rect, Color32)); 10] = [
                (Tool::Move, "t_move", t.tool_move, icons::draw_move_icon),
                (Tool::Pen, "t_pen", t.tool_pen, icons::draw_pen_icon),
                (Tool::Marker, "t_marker", t.tool_marker, icons::draw_marker_icon),
                (Tool::Line, "t_line", t.tool_line, icons::draw_line_icon),
                (Tool::Arrow, "t_arrow", t.tool_arrow, icons::draw_arrow_icon),
                (Tool::Text, "t_text", t.tool_text, icons::draw_text_icon),
                (
                    Tool::Ellipse,
                    "t_ellipse",
                    &tip_ellipse,
                    if cmd_held {
                        |p, r, c| icons::draw_ellipse_icon(p, r, c, true)
                    } else {
                        |p, r, c| icons::draw_ellipse_icon(p, r, c, false)
                    },
                ),
                (
                    Tool::Rect,
                    "t_rect",
                    &tip_rect,
                    if cmd_held {
                        |p, r, c| icons::draw_rect_icon(p, r, c, true)
                    } else {
                        |p, r, c| icons::draw_rect_icon(p, r, c, false)
                    },
                ),
                (Tool::Step, "t_step", t.tool_step, icons::draw_step_icon),
                (
                    Tool::Blur,
                    "t_blur",
                    &tip_blur,
                    if cmd_held {
                        |p, r, c| icons::draw_blur_icon(p, r, c, true)
                    } else {
                        |p, r, c| icons::draw_blur_icon(p, r, c, false)
                    },
                ),
            ];

            let prev_color_pos = shared.color_pos;
            let prev_thick = shared.thickness;
            let mut slider_dragging = false;

            if is_vert {
                ui.add_space(grip_pad);
                ui.allocate_ui_with_layout(
                    Vec2::new(bar_w, bar_h - grip_pad),
                    egui::Layout::left_to_right(egui::Align::Min),
                    |ui| {
                        ui.add_space(6.0);
                        ui.allocate_ui_with_layout(
                            Vec2::new(40.0, bar_h),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                ui.add_space(8.0);
                                ui.spacing_mut().item_spacing.y = 3.5;
                                let btn_sz = Vec2::splat(32.0);
                                render_tool_and_action_buttons(
                                    ui,
                                    btn_sz,
                                    true,
                                    &tools,
                                    &t,
                                    shared,
                                    &mut outcome,
                                );
                            },
                        );

                        let sep_x = bar_rect.left() + 51.0;
                        ui.painter().line_segment(
                            [
                                Pos2::new(sep_x, bar_rect.top() + grip_pad + 10.0),
                                Pos2::new(sep_x, bar_rect.bottom() - 10.0),
                            ],
                            Stroke::new(1.0_f32, shared.theme.outline),
                        );

                        ui.add_space(6.0);
                        ui.allocate_ui_with_layout(
                            Vec2::new(36.0, bar_h),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                ui.add_space(10.0);
                                ui.spacing_mut().item_spacing.y = 10.0;
                                let (col, c_drag) = m3_color_slider_vertical(
                                    ui,
                                    376.0,
                                    &mut shared.color_pos,
                                    t.slider_color,
                                    &shared.theme,
                                );
                                shared.active_color = col;
                                let s_drag = m3_size_slider_vertical(
                                    ui,
                                    144.0,
                                    &mut shared.thickness,
                                    1.5,
                                    20.0,
                                    t.slider_size,
                                    &shared.theme,
                                );
                                slider_dragging = c_drag || s_drag;
                            },
                        );
                    },
                );
            } else {
                let content_rect =
                    Rect::from_min_max(bar_rect.min + Vec2::new(grip_pad, 0.0), bar_rect.max);
                ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
                    ui.allocate_ui_with_layout(
                        content_rect.size(),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.add_space(8.0);
                            let pad_x = 10.0;
                            let btn_sz = Vec2::splat(32.0);

                            ui.horizontal(|ui| {
                                ui.add_space(pad_x);
                                ui.spacing_mut().item_spacing.x = 4.5;
                                render_tool_and_action_buttons(
                                    ui,
                                    btn_sz,
                                    false,
                                    &tools,
                                    &t,
                                    shared,
                                    &mut outcome,
                                );
                            });

                            ui.add_space(4.0);
                            let sep_y = ui.cursor().top();
                            ui.painter().line_segment(
                                [
                                    Pos2::new(bar_rect.left() + grip_pad + pad_x, sep_y),
                                    Pos2::new(bar_rect.right() - pad_x, sep_y),
                                ],
                                Stroke::new(1.0_f32, shared.theme.outline),
                            );
                            ui.add_space(5.0);

                            ui.horizontal(|ui| {
                                ui.add_space(pad_x);
                                ui.spacing_mut().item_spacing.x = 12.0;
                                let color_w =
                                    (content_rect.width() - pad_x * 2.0 - 138.0 - 12.0).max(200.0);
                                let (col, c_drag) = m3_color_slider(
                                    ui,
                                    color_w,
                                    &mut shared.color_pos,
                                    t.slider_color,
                                    &shared.theme,
                                );
                                shared.active_color = col;
                                let s_drag = m3_size_slider(
                                    ui,
                                    &mut shared.thickness,
                                    1.5,
                                    20.0,
                                    t.slider_size,
                                    &shared.theme,
                                );
                                slider_dragging = c_drag || s_drag;
                            });
                        },
                    );
                });
            }

            let target_idx = shared.selected_annotation.or(shared.last_edited_idx);
            let color_changed = (shared.color_pos - prev_color_pos).abs() > 1e-4;
            let thick_changed = (shared.thickness - prev_thick).abs() > 1e-4;

            if (color_changed || thick_changed) && target_idx.is_some() {
                if !shared.slider_undo_pushed {
                    shared.push_undo_snapshot();
                    shared.slider_undo_pushed = true;
                }
                if let Some(idx) = target_idx {
                    if let Some(ann) = shared.annotations.get_mut(idx) {
                        if color_changed {
                            ann.set_color(shared.active_color);
                        }
                        if thick_changed {
                            ann.apply_thickness(shared.thickness);
                        }
                    }
                    let affects_blur = shared
                        .annotations
                        .iter()
                        .skip(idx)
                        .any(|a| matches!(a, Annotation::Blur { .. } | Annotation::Text { .. }));
                    if affects_blur {
                        if slider_dragging {
                            shared.defer_blur_idx = Some(idx);
                            shared.pending_blur_rebake = Some(idx);
                        } else {
                            shared.defer_blur_idx = None;
                            shared.pending_blur_rebake = None;
                            shared.rebake_blurs_from(idx);
                        }
                    }
                }
            }

            if !slider_dragging && ctx.input(|i| !i.pointer.any_down()) {
                shared.slider_undo_pushed = false;
                shared.flush_pending_blur_rebake();
            }
        });

    outcome
}

fn render_tool_and_action_buttons(
    ui: &mut egui::Ui,
    btn_sz: Vec2,
    vertical_bar: bool,
    tools: &[(Tool, &str, &str, fn(&egui::Painter, Rect, Color32)); 10],
    t: &Strings,
    shared: &mut OverlayShared,
    outcome: &mut Option<OverlayOutcome>,
) {
    for &(tool, id_salt, tip, draw_fn) in tools {
        if m3_tool_button(
            ui,
            id_salt,
            btn_sz,
            shared.active_tool == tool,
            tip,
            &shared.theme,
            draw_fn,
        )
        .clicked()
        {
            shared.commit_pending_text();
            shared.active_tool = tool;
        }
    }

    m3_divider(ui, &shared.theme, vertical_bar);

    let tip_undo = Strings::with_mod(t.act_undo);
    let tip_redo = Strings::with_mod(t.act_redo);
    let tip_copy = Strings::with_mod(t.act_copy);
    let tip_save = Strings::with_mod(t.act_save);

    if m3_action_button(ui, btn_sz, false, &tip_undo, &shared.theme, icons::draw_undo_icon)
        .clicked()
    {
        shared.undo();
    }
    if m3_action_button(ui, btn_sz, false, &tip_redo, &shared.theme, icons::draw_redo_icon)
        .clicked()
    {
        shared.redo();
    }
    if m3_action_button(ui, btn_sz, true, &tip_copy, &shared.theme, icons::draw_copy_icon).clicked()
    {
        if shared.live_draw {
            *outcome = Some(OverlayOutcome::LiveCopy);
        } else if shared.export_and_copy() {
            *outcome = Some(OverlayOutcome::Close);
        }
    }
    if m3_action_button(ui, btn_sz, false, &tip_save, &shared.theme, icons::draw_save_icon)
        .clicked()
    {
        if shared.live_draw {
            *outcome = Some(OverlayOutcome::LiveSave);
        } else if shared.export_and_save() {
            *outcome = Some(OverlayOutcome::Close);
        }
    }
    if m3_action_button(
        ui,
        btn_sz,
        false,
        t.act_close,
        &shared.theme,
        icons::draw_close_icon,
    )
    .clicked()
    {
        *outcome = Some(OverlayOutcome::Close);
    }
}

pub(super) fn draw_error_banner(
    ctx: &egui::Context,
    shared: &OverlayShared,
    local_rect: Rect,
) {
    let Some((msg, _)) = &shared.status_error else {
        return;
    };
    let w = (msg.chars().count() as f32 * 7.4 + 32.0)
        .clamp(220.0, (local_rect.width() - 40.0).max(220.0));
    let pos = Pos2::new((local_rect.center().x - w * 0.5).max(16.0), 18.0);
    egui::Area::new(egui::Id::new("overlay_error_toast"))
        .fixed_pos(pos)
        .order(egui::Order::Tooltip)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(Color32::from_rgb(138, 28, 36))
                .stroke(Stroke::new(1.2_f32, Color32::from_rgb(255, 130, 130)))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(egui::Margin::symmetric(14, 8))
                .show(ui, |ui| {
                    ui.set_max_width(w);
                    ui.label(
                        egui::RichText::new(msg)
                            .size(13.0)
                            .color(Color32::WHITE)
                            .strong(),
                    );
                });
        });
}

fn m3_divider(ui: &mut egui::Ui, theme: &ThemeColors, vertical_bar: bool) {
    if vertical_bar {
        let (div_rect, _) = ui.allocate_exact_size(Vec2::new(24.0, 6.0), egui::Sense::hover());
        ui.painter().line_segment(
            [
                Pos2::new(div_rect.left(), div_rect.center().y),
                Pos2::new(div_rect.right(), div_rect.center().y),
            ],
            Stroke::new(1.0_f32, theme.outline),
        );
    } else {
        let (div_rect, _) = ui.allocate_exact_size(Vec2::new(6.0, 22.0), egui::Sense::hover());
        ui.painter().line_segment(
            [
                Pos2::new(div_rect.center().x, div_rect.top()),
                Pos2::new(div_rect.center().x, div_rect.bottom()),
            ],
            Stroke::new(1.0_f32, theme.outline),
        );
    }
}

fn pending_editor_metrics(
    ctx: &egui::Context,
    pending: &PendingText,
    thickness: f32,
    text_font: TextFont,
) -> (f32, f32, f32, TextFont, Option<f32>) {
    let (font_size, edit_font, orig_wrap) = match &pending.original {
        Some((_, Annotation::Text { size, font, box_size, .. })) => {
            (*size, *font, box_size.map(|s| s.x))
        }
        _ => (
            (13.0 + thickness * 2.2).clamp(16.0, 57.0),
            text_font,
            None,
        ),
    };
    let font_id = egui::FontId::new(font_size, egui::FontFamily::Name(edit_font.egui_family().into()));
    let galley_sz = ctx.fonts(|f| {
        if let Some(w) = orig_wrap {
            f.layout(pending.buf.clone(), font_id, Color32::WHITE, w.max(24.0))
                .size()
        } else {
            f.layout_no_wrap(pending.buf.clone(), font_id, Color32::WHITE)
                .size()
        }
    });
    let box_w = if let Some(w) = orig_wrap {
        w.max(24.0)
    } else if pending.buf.is_empty() {
        10.0
    } else {
        (galley_sz.x + 14.0).clamp(10.0, 640.0)
    };
    let box_h = galley_sz.y.max(font_size * 1.25);
    (box_w, box_h, font_size, edit_font, orig_wrap)
}
