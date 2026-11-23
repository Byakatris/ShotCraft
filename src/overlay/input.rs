use egui::{Pos2, Rect, Vec2};
use winit::window::{CursorIcon, Window};

use crate::annotations::{Annotation, TextPlacement, Tool};

use super::{DragAction, Mode, OverlayShared, PendingText, ResizeHandle};

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_canvas_pointer(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    window: &Window,
    shared: &mut OverlayShared,
    local_rect: Rect,
    virtual_rect: Rect,
    mon_offset: Vec2,
    mon_scale: f32,
    shift_held: bool,
    cmd_held: bool,
) {
    let resp = ui.allocate_rect(local_rect, egui::Sense::click_and_drag());
    let to_global = |p: Pos2| -> Pos2 { (mon_offset + p.to_vec2() * mon_scale).to_pos2() };

    if shared.mode == Mode::WindowSelect {
        window.set_cursor(CursorIcon::Pointer);
        if let Some(hover_global) = shared.hover_pos {
            if let Some(win) = shared.windows.iter().find(|w| w.rect.contains(hover_global)) {
                shared.selection = Some(win.rect);
                shared.hovered_window_label = Some(win.label.clone());
            } else {
                shared.selection = None;
                shared.hovered_window_label = None;
            }
        }
        if resp.clicked() && shared.selection.is_some() {
            shared.mode = Mode::Annotating;
        }
        return;
    }

    if shared.mode == Mode::Selecting {
        window.set_cursor(CursorIcon::Crosshair);
        if resp.drag_started() {
            shared.drag_start = ctx
                .input(|i| i.pointer.press_origin())
                .or_else(|| resp.interact_pointer_pos())
                .map(to_global);
        }
        if resp.dragged() {
            if let (Some(a), Some(b)) =
                (shared.drag_start, resp.interact_pointer_pos().map(to_global))
            {
                shared.selection = Some(Rect::from_two_pos(a, b).intersect(virtual_rect));
            }
        }
        if resp.drag_stopped() {
            if let Some(sel) = shared.selection {
                if sel.width() >= 12.0 && sel.height() >= 12.0 {
                    shared.mode = Mode::Annotating;
                } else {
                    shared.selection = None;
                }
            }
            shared.drag_start = None;
        }
        return;
    }

    // Mode::Annotating
    if shared.active_tool == Tool::Text && resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos().map(to_global) {
            if shared.pending_text.is_some() {
                shared.commit_pending_text();
            } else if let Some(idx) = shared
                .annotations
                .iter()
                .rposition(|a| matches!(a, Annotation::Text { .. }) && a.hit_test(pos))
            {
                shared.open_text_for_edit(idx);
            } else {
                shared.pending_text = Some(PendingText {
                    pos,
                    arrow_to: None,
                    placement: TextPlacement::LeftTop,
                    buf: String::new(),
                    just_opened: true,
                    original: None,
                });
            }
        }
    }

    if (shared.active_tool == Tool::Pen || shared.active_tool == Tool::Marker) && resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos().map(to_global) {
            shared.push_undo_snapshot();
            let ann = match shared.active_tool {
                Tool::Pen => Annotation::Pen {
                    points: vec![pos],
                    color: shared.active_color,
                    thickness: shared.thickness,
                },
                _ => Annotation::Marker {
                    points: vec![pos],
                    color: shared.active_color,
                    thickness: shared.thickness,
                },
            };
            shared.annotations.push(ann);
            shared.last_edited_idx = Some(shared.annotations.len() - 1);
            shared.selected_annotation = None;
        }
    }

    if shared.active_tool == Tool::Step && resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos().map(to_global) {
            shared.push_undo_snapshot();
            shared.annotations.push(Annotation::Step {
                pos,
                number: shared.step_counter,
                color: shared.active_color,
                radius: shared.step_radius(),
            });
            shared.last_edited_idx = Some(shared.annotations.len() - 1);
            shared.selected_annotation = None;
            shared.step_counter += 1;
        }
    }

    if shared.active_tool == Tool::Move && resp.double_clicked() {
        if let Some(pos) = resp.interact_pointer_pos().map(to_global) {
            if let Some(idx) = shared
                .annotations
                .iter()
                .rposition(|a| matches!(a, Annotation::Text { .. }) && a.hit_test(pos))
            {
                shared.open_text_for_edit(idx);
            }
        }
    } else if shared.active_tool == Tool::Move && resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos().map(to_global) {
            shared.selected_annotation = shared.annotations.iter().rposition(|a| a.hit_test(pos));
            if shared.selected_annotation.is_some() {
                shared.last_edited_idx = shared.selected_annotation;
            }
        }
    }

    let cursor = match shared.active_tool {
        Tool::Move => {
            if shared.drag_action.is_some() {
                CursorIcon::Grabbing
            } else {
                CursorIcon::Default
            }
        }
        Tool::Text => CursorIcon::Text,
        _ => CursorIcon::Crosshair,
    };
    window.set_cursor(cursor);

    if resp.drag_started() {
        let start_pos = ctx
            .input(|i| i.pointer.press_origin())
            .or_else(|| resp.interact_pointer_pos())
            .map(to_global);
        if let Some(pos) = start_pos {
            shared.commit_pending_text();
            shared.drag_start = Some(pos);
            shared.drag_curr = Some(pos);

            match shared.active_tool {
                Tool::Move => {
                    let hit_ctrl = shared.selected_annotation.and_then(|idx| {
                        shared.annotations.get(idx).and_then(|ann| {
                            ann.control_points()
                                .into_iter()
                                .enumerate()
                                .find(|(_, cp)| cp.distance(pos) <= 12.0 * mon_scale)
                                .map(|(h, _)| (idx, h))
                        })
                    });
                    if let Some((idx, handle_idx)) = hit_ctrl {
                        shared.push_undo_snapshot();
                        shared.last_edited_idx = Some(idx);
                        shared.drag_action = Some(DragAction::EditAnnotationHandle { idx, handle_idx });
                    } else if let Some(handle) = (!shared.live_draw)
                        .then(|| {
                            shared
                                .selection
                                .and_then(|s| hit_resize_handle(s, pos, 12.0 * mon_scale))
                        })
                        .flatten()
                    {
                        shared.drag_action = Some(DragAction::ResizeSelection { handle });
                    } else if !shared.live_draw
                        && shared
                            .selection
                            .is_some_and(|s| hit_selection_border(s, pos))
                    {
                        shared.push_undo_snapshot();
                        shared.drag_action = Some(DragAction::MoveSelectionAndAll { last_pos: pos });
                    } else if let Some(idx) = shared.annotations.iter().rposition(|a| a.hit_test(pos))
                    {
                        shared.push_undo_snapshot();
                        shared.selected_annotation = Some(idx);
                        shared.last_edited_idx = Some(idx);
                        shared.drag_action = Some(DragAction::MoveAnnotation { idx, last_pos: pos });
                    } else {
                        shared.selected_annotation = None;
                    }
                }
                Tool::Pen => {
                    shared.drag_action = Some(DragAction::DrawAnnotation);
                    shared.in_progress = Some(Annotation::Pen {
                        points: vec![pos],
                        color: shared.active_color,
                        thickness: shared.thickness,
                    });
                }
                Tool::Marker => {
                    shared.drag_action = Some(DragAction::DrawAnnotation);
                    shared.in_progress = Some(Annotation::Marker {
                        points: vec![pos],
                        color: shared.active_color,
                        thickness: shared.thickness,
                    });
                }
                Tool::Text => {
                    shared.drag_action = Some(DragAction::TextCallout);
                }
                Tool::Step => {
                    shared.push_undo_snapshot();
                    shared.annotations.push(Annotation::Step {
                        pos,
                        number: shared.step_counter,
                        color: shared.active_color,
                        radius: shared.step_radius(),
                    });
                    shared.last_edited_idx = Some(shared.annotations.len() - 1);
                    shared.selected_annotation = None;
                    shared.step_counter += 1;
                    shared.drag_start = None;
                }
                Tool::Blur => {
                    let hit_ctrl = shared.selected_annotation.and_then(|idx| {
                        shared.annotations.get(idx).and_then(|ann| {
                            if matches!(ann, Annotation::Blur { .. }) {
                                ann.control_points()
                                    .into_iter()
                                    .enumerate()
                                    .find(|(_, cp)| cp.distance(pos) <= 12.0 * mon_scale)
                                    .map(|(h, _)| (idx, h))
                            } else {
                                None
                            }
                        })
                    });
                    if let Some((idx, handle_idx)) = hit_ctrl {
                        shared.push_undo_snapshot();
                        shared.last_edited_idx = Some(idx);
                        shared.drag_action = Some(DragAction::EditAnnotationHandle { idx, handle_idx });
                    } else {
                        shared.drag_action = Some(DragAction::DrawAnnotation);
                    }
                }
                Tool::Line | Tool::Arrow | Tool::Ellipse | Tool::Rect => {
                    shared.drag_action = Some(DragAction::DrawAnnotation);
                }
            }
        }
    }

    if resp.dragged() {
        if let Some(curr) = resp.interact_pointer_pos().map(to_global) {
            shared.drag_curr = Some(curr);
            match shared.drag_action {
                Some(DragAction::EditAnnotationHandle { idx, handle_idx }) => {
                    if let Some(ann) = shared.annotations.get_mut(idx) {
                        ann.move_control_point(handle_idx, curr);
                    }
                    if matches!(
                        shared.annotations.get(idx),
                        Some(Annotation::Blur { .. } | Annotation::Text { .. })
                    ) {
                        shared.defer_blur_idx = Some(idx);
                        shared.pending_blur_rebake = Some(idx);
                    }
                }
                Some(DragAction::ResizeSelection { handle }) => {
                    if let Some(sel) = &mut shared.selection {
                        *sel = apply_resize(*sel, handle, curr, virtual_rect);
                    }
                }
                Some(DragAction::MoveSelectionAndAll { last_pos }) => {
                    if let Some(sel) = &mut shared.selection {
                        let raw_delta = curr - last_pos;
                        let dx = raw_delta.x.clamp(
                            virtual_rect.left() - sel.left(),
                            virtual_rect.right() - sel.right(),
                        );
                        let dy = raw_delta.y.clamp(
                            virtual_rect.top() - sel.top(),
                            virtual_rect.bottom() - sel.bottom(),
                        );
                        let delta = Vec2::new(dx, dy);
                        *sel = sel.translate(delta);
                        for ann in &mut shared.annotations {
                            ann.translate(delta);
                        }
                    }
                    shared.drag_action = Some(DragAction::MoveSelectionAndAll { last_pos: curr });
                }
                Some(DragAction::MoveAnnotation { idx, last_pos }) => {
                    let delta = curr - last_pos;
                    if let Some(ann) = shared.annotations.get_mut(idx) {
                        ann.translate(delta);
                    }
                    shared.drag_action = Some(DragAction::MoveAnnotation { idx, last_pos: curr });
                    if matches!(
                        shared.annotations.get(idx),
                        Some(Annotation::Blur { .. } | Annotation::Text { .. })
                    ) {
                        shared.defer_blur_idx = Some(idx);
                        shared.pending_blur_rebake = Some(idx);
                    }
                }
                Some(DragAction::TextCallout) => {}
                Some(DragAction::DrawAnnotation) => {
                    if let Some(start) = shared.drag_start {
                        match shared.active_tool {
                            Tool::Pen => {
                                if let Some(Annotation::Pen { points, .. }) = &mut shared.in_progress
                                {
                                    if points.last().is_none_or(|p| p.distance(curr) > 3.5) {
                                        points.push(curr);
                                    }
                                }
                            }
                            Tool::Marker => {
                                if let Some(Annotation::Marker { points, .. }) =
                                    &mut shared.in_progress
                                {
                                    if points.last().is_none_or(|p| p.distance(curr) > 3.5) {
                                        points.push(curr);
                                    }
                                }
                            }
                            Tool::Line => {
                                let end = if shift_held {
                                    snap_angle(start, curr, shared.snap_angle_deg as f32)
                                } else {
                                    curr
                                };
                                shared.in_progress = Some(Annotation::Line {
                                    start,
                                    end,
                                    color: shared.active_color,
                                    thickness: shared.thickness,
                                });
                            }
                            Tool::Arrow => {
                                let end = if shift_held {
                                    snap_angle(start, curr, shared.snap_angle_deg as f32)
                                } else {
                                    curr
                                };
                                shared.in_progress = Some(Annotation::Arrow {
                                    start,
                                    end,
                                    color: shared.active_color,
                                    thickness: shared.thickness,
                                });
                            }
                            Tool::Ellipse => {
                                let end = if shift_held {
                                    snap_square(start, curr)
                                } else {
                                    curr
                                };
                                shared.in_progress = Some(Annotation::Ellipse {
                                    rect: Rect::from_two_pos(start, end),
                                    color: shared.active_color,
                                    thickness: shared.thickness,
                                    filled: cmd_held,
                                });
                            }
                            Tool::Rect => {
                                let end = if shift_held {
                                    snap_square(start, curr)
                                } else {
                                    curr
                                };
                                shared.in_progress = Some(Annotation::Rect {
                                    rect: Rect::from_two_pos(start, end),
                                    color: shared.active_color,
                                    thickness: shared.thickness,
                                    filled: cmd_held,
                                });
                            }
                            Tool::Blur => {
                                shared.in_progress = Some(Annotation::Blur {
                                    rect: Rect::from_two_pos(start, curr),
                                    radius: shared.blur_radius(),
                                    pixelate: cmd_held,
                                });
                            }
                            _ => {}
                        }
                    }
                }
                None => {}
            }
        }
    }

    if resp.drag_stopped() {
        if matches!(shared.drag_action, Some(DragAction::TextCallout)) {
            if let (Some(start), Some(curr)) = (shared.drag_start, shared.drag_curr) {
                let (text_pos, arrow_to, placement) = if start.distance(curr) >= 10.0 {
                    (curr, Some(start), TextPlacement::from_drag(start, curr))
                } else {
                    (start, None, TextPlacement::LeftTop)
                };
                shared.pending_text = Some(PendingText {
                    pos: text_pos,
                    arrow_to,
                    placement,
                    buf: String::new(),
                    just_opened: true,
                    original: None,
                });
            }
        }
        if let Some(ann) = shared.in_progress.take() {
            shared.push_undo_snapshot();
            let is_blur = matches!(ann, Annotation::Blur { .. });
            let new_idx = shared.annotations.len();
            shared.annotations.push(ann);
            shared.last_edited_idx = Some(new_idx);
            shared.selected_annotation = if is_blur { Some(new_idx) } else { None };
            if is_blur && shared.live_draw && shared.virtual_screen.image.width() <= 2 {
                shared.need_live_blur_capture = true;
            } else {
                shared.rebake_blurs_from(new_idx);
            }
        } else if let Some(action) = shared.drag_action {
            match action {
                DragAction::MoveAnnotation { idx, .. }
                | DragAction::EditAnnotationHandle { idx, .. } => {
                    shared.defer_blur_idx = None;
                    shared.pending_blur_rebake = None;
                    shared.rebake_blurs_from(idx);
                }
                DragAction::MoveSelectionAndAll { .. } => {
                    shared.defer_blur_idx = None;
                    shared.pending_blur_rebake = None;
                    shared.rebake_blurs_from(0);
                }
                _ => {}
            }
        }
        shared.drag_start = None;
        shared.drag_curr = None;
        shared.drag_action = None;
    }
}

pub(super) fn hit_selection_border(sel: Rect, pos: Pos2) -> bool {
    let outer = sel.expand(10.0);
    let inner = sel.shrink(10.0);
    outer.contains(pos) && !inner.contains(pos)
}

pub(super) fn hit_resize_handle(sel: Rect, pos: Pos2, tol: f32) -> Option<ResizeHandle> {
    let handles = [
        (ResizeHandle::TopLeft, sel.left_top()),
        (ResizeHandle::Top, Pos2::new(sel.center().x, sel.top())),
        (ResizeHandle::TopRight, sel.right_top()),
        (ResizeHandle::Right, Pos2::new(sel.right(), sel.center().y)),
        (ResizeHandle::BottomRight, sel.right_bottom()),
        (ResizeHandle::Bottom, Pos2::new(sel.center().x, sel.bottom())),
        (ResizeHandle::BottomLeft, sel.left_bottom()),
        (ResizeHandle::Left, Pos2::new(sel.left(), sel.center().y)),
    ];
    handles
        .into_iter()
        .find(|(_, p)| p.distance(pos) <= tol)
        .map(|(h, _)| h)
}

pub(super) fn apply_resize(sel: Rect, handle: ResizeHandle, pos: Pos2, bounds: Rect) -> Rect {
    let p = pos.clamp(bounds.min, bounds.max);
    let min_size = 24.0;
    let mut left = sel.left();
    let mut right = sel.right();
    let mut top = sel.top();
    let mut bottom = sel.bottom();

    match handle {
        ResizeHandle::TopLeft => {
            left = p.x.min(right - min_size);
            top = p.y.min(bottom - min_size);
        }
        ResizeHandle::Top => {
            top = p.y.min(bottom - min_size);
        }
        ResizeHandle::TopRight => {
            right = p.x.max(left + min_size);
            top = p.y.min(bottom - min_size);
        }
        ResizeHandle::Right => {
            right = p.x.max(left + min_size);
        }
        ResizeHandle::BottomRight => {
            right = p.x.max(left + min_size);
            bottom = p.y.max(top + min_size);
        }
        ResizeHandle::Bottom => {
            bottom = p.y.max(top + min_size);
        }
        ResizeHandle::BottomLeft => {
            left = p.x.min(right - min_size);
            bottom = p.y.max(top + min_size);
        }
        ResizeHandle::Left => {
            left = p.x.min(right - min_size);
        }
    }

    Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, bottom))
}

pub(super) fn snap_square(start: Pos2, curr: Pos2) -> Pos2 {
    let dx = curr.x - start.x;
    let dy = curr.y - start.y;
    let side = dx.abs().max(dy.abs());
    Pos2::new(start.x + side * dx.signum(), start.y + side * dy.signum())
}

pub(super) fn snap_angle(start: Pos2, curr: Pos2, snap_deg: f32) -> Pos2 {
    let d = curr - start;
    let len = d.length();
    if len < 1.0 {
        return curr;
    }
    let step = snap_deg.clamp(1.0, 90.0).to_radians();
    let angle = (d.y.atan2(d.x) / step).round() * step;
    start + Vec2::new(len * angle.cos(), len * angle.sin())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_resize_all_handles_and_clamping() {
        let bounds = Rect::from_min_max(Pos2::ZERO, Pos2::new(1000.0, 800.0));
        let sel = Rect::from_min_max(Pos2::new(100.0, 100.0), Pos2::new(300.0, 300.0));

        let tl = apply_resize(sel, ResizeHandle::TopLeft, Pos2::new(50.0, 60.0), bounds);
        assert_eq!(tl, Rect::from_min_max(Pos2::new(50.0, 60.0), Pos2::new(300.0, 300.0)));

        // Cannot invert past min_size (24)
        let tl_clamp = apply_resize(sel, ResizeHandle::TopLeft, Pos2::new(500.0, 500.0), bounds);
        assert_eq!(tl_clamp.width(), 24.0);
        assert_eq!(tl_clamp.height(), 24.0);

        // Out-of-bounds right/bottom clamped to screen bounds
        let br = apply_resize(sel, ResizeHandle::BottomRight, Pos2::new(1500.0, 1200.0), bounds);
        assert_eq!(br.max, Pos2::new(1000.0, 800.0));

        let t = apply_resize(sel, ResizeHandle::Top, Pos2::new(200.0, 40.0), bounds);
        assert_eq!(t.top(), 40.0);
        assert_eq!(t.left(), 100.0);

        let b = apply_resize(sel, ResizeHandle::Bottom, Pos2::new(200.0, 450.0), bounds);
        assert_eq!(b.bottom(), 450.0);

        let l = apply_resize(sel, ResizeHandle::Left, Pos2::new(20.0, 200.0), bounds);
        assert_eq!(l.left(), 20.0);

        let r = apply_resize(sel, ResizeHandle::Right, Pos2::new(420.0, 200.0), bounds);
        assert_eq!(r.right(), 420.0);
    }

    #[test]
    fn snap_angle_and_square_geometry() {
        let s = Pos2::new(100.0, 100.0);
        let snapped_90 = snap_angle(s, Pos2::new(200.0, 115.0), 90.0);
        assert!((snapped_90.y - 100.0).abs() < 0.01);
        assert!(snapped_90.x > 199.0);

        let snapped_45 = snap_angle(s, Pos2::new(200.0, 195.0), 45.0);
        assert!(((snapped_45.x - 100.0) - (snapped_45.y - 100.0)).abs() < 0.5);

        let sq = snap_square(s, Pos2::new(150.0, 200.0));
        assert_eq!(sq, Pos2::new(200.0, 200.0));
    }

    #[test]
    fn hit_test_selection_handles_and_border() {
        let sel = Rect::from_min_max(Pos2::new(100.0, 100.0), Pos2::new(300.0, 200.0));
        assert_eq!(
            hit_resize_handle(sel, Pos2::new(102.0, 101.0), 12.0),
            Some(ResizeHandle::TopLeft)
        );
        assert_eq!(
            hit_resize_handle(sel, Pos2::new(200.0, 200.0), 12.0),
            Some(ResizeHandle::Bottom)
        );
        assert_eq!(hit_resize_handle(sel, Pos2::new(200.0, 150.0), 12.0), None);

        assert!(hit_selection_border(sel, Pos2::new(150.0, 100.0)));
        assert!(!hit_selection_border(sel, Pos2::new(200.0, 150.0)));
    }
}
