use crate::mccs::DisplayMode;
use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Stroke, Vec2};

pub fn draw_gameview_icon(painter: &egui::Painter, center: Pos2, mode: DisplayMode, color: Color32) {
    match mode {
        DisplayMode::Action | DisplayMode::Racing | DisplayMode::Sports => {
            // Gamepad silhouette
            let body = Rect::from_center_size(center, Vec2::new(34.0, 20.0));
            painter.rect_filled(body, Rounding::same(9.0), color);
            // Left & right grips
            painter.circle_filled(center + Vec2::new(-12.0, 7.0), 6.5, color);
            painter.circle_filled(center + Vec2::new(12.0, 7.0), 6.5, color);
            // Cutout dark D-pad & buttons
            let dark = Color32::from_rgb(40, 44, 52);
            let dpad_c = center + Vec2::new(-8.5, -1.0);
            painter.rect_filled(
                Rect::from_center_size(dpad_c, Vec2::new(8.0, 2.6)),
                Rounding::same(1.0),
                dark,
            );
            painter.rect_filled(
                Rect::from_center_size(dpad_c, Vec2::new(2.6, 8.0)),
                Rounding::same(1.0),
                dark,
            );
            painter.circle_filled(center + Vec2::new(7.0, -2.5), 1.8, dark);
            painter.circle_filled(center + Vec2::new(11.0, 1.0), 1.8, dark);
        }
        DisplayMode::Hdr => {
            let box_rect = Rect::from_center_size(center, Vec2::new(36.0, 22.0));
            painter.rect_filled(box_rect, Rounding::same(4.0), color);
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                "HDR",
                egui::FontId::proportional(12.5),
                Color32::from_rgb(40, 44, 52),
            );
        }
        DisplayMode::User => {
            // Head + shoulders
            painter.circle_filled(center + Vec2::new(0.0, -7.0), 6.5, color);
            let body = Rect::from_center_size(center + Vec2::new(0.0, 7.5), Vec2::new(24.0, 13.0));
            painter.rect_filled(
                body,
                Rounding {
                    nw: 10.0,
                    ne: 10.0,
                    sw: 3.0,
                    se: 3.0,
                },
                color,
            );
        }
        DisplayMode::Standard => {
            // 3 horizontal sliders
            let stroke = Stroke::new(2.2_f32, color);
            for (dy, kx) in [(-8.0, -5.0), (0.0, 6.0), (8.0, -2.0)] {
                let y = center.y + dy;
                painter.line_segment(
                    [Pos2::new(center.x - 14.0, y), Pos2::new(center.x + 14.0, y)],
                    stroke,
                );
                painter.circle_filled(Pos2::new(center.x + kx, y), 3.3, color);
            }
        }
        DisplayMode::Eco => {
            // Globe outline
            painter.circle_stroke(center + Vec2::new(-2.0, 1.0), 12.0, Stroke::new(2.2_f32, color));
            // Simple continent curves inside the globe
            let c = center + Vec2::new(-2.0, 1.0);
            painter.add(egui::Shape::convex_polygon(
                vec![
                    c + Vec2::new(-8.0, -4.0),
                    c + Vec2::new(-2.0, -6.0),
                    c + Vec2::new(-3.0, 1.0),
                    c + Vec2::new(-8.0, 3.0),
                ],
                color,
                Stroke::NONE,
            ));
            painter.add(egui::Shape::convex_polygon(
                vec![
                    c + Vec2::new(-3.0, 3.0),
                    c + Vec2::new(3.0, 2.0),
                    c + Vec2::new(1.0, 9.0),
                    c + Vec2::new(-5.0, 8.0),
                ],
                color,
                Stroke::NONE,
            ));
            // Pointed leaf on top-right
            let leaf_pts = vec![
                center + Vec2::new(2.0, -1.0),
                center + Vec2::new(4.0, -10.0),
                center + Vec2::new(14.0, -12.0),
                center + Vec2::new(11.0, -3.0),
            ];
            painter.add(egui::Shape::convex_polygon(leaf_pts, color, Stroke::NONE));
            // Leaf vein
            painter.line_segment(
                [center + Vec2::new(3.0, -2.0), center + Vec2::new(11.0, -9.0)],
                Stroke::new(1.6_f32, Color32::from_rgb(40, 44, 52)),
            );
        }
        DisplayMode::Graphic | _ => {
            // Picture frame with mountain & sun
            let frame = Rect::from_center_size(center, Vec2::new(30.0, 24.0));
            painter.rect_stroke(frame, Rounding::same(4.0), Stroke::new(2.2_f32, color));
            painter.circle_filled(center + Vec2::new(-7.0, -5.0), 2.5, color);
            // Mountain triangles
            let p1 = center + Vec2::new(-12.0, 9.0);
            let p2 = center + Vec2::new(-2.0, -1.0);
            let p3 = center + Vec2::new(6.0, 9.0);
            painter.add(egui::Shape::convex_polygon(vec![p1, p2, p3], color, Stroke::NONE));
            let q1 = center + Vec2::new(0.0, 9.0);
            let q2 = center + Vec2::new(7.0, 1.0);
            let q3 = center + Vec2::new(13.0, 9.0);
            painter.add(egui::Shape::convex_polygon(vec![q1, q2, q3], color, Stroke::NONE));
        }
    }
}

pub fn draw_custom_modes_icon(painter: &egui::Painter, center: Pos2, color: Color32) {
    // 2x2 rounded squares (preset/submenu grid icon) with a "+" in bottom-right
    let s = 10.0;
    let gap = 3.0;
    for (dx, dy, is_plus) in [
        (-gap - s * 0.5, -gap - s * 0.5, false),
        (gap + s * 0.5, -gap - s * 0.5, false),
        (-gap - s * 0.5, gap + s * 0.5, false),
        (gap + s * 0.5, gap + s * 0.5, true),
    ] {
        let c = center + Vec2::new(dx, dy);
        if is_plus {
            let stroke = Stroke::new(2.2_f32, color);
            painter.line_segment([c + Vec2::new(-4.5, 0.0), c + Vec2::new(4.5, 0.0)], stroke);
            painter.line_segment([c + Vec2::new(0.0, -4.5), c + Vec2::new(0.0, 4.5)], stroke);
        } else {
            painter.rect_filled(
                Rect::from_center_size(c, Vec2::splat(s)),
                Rounding::same(2.5),
                color,
            );
        }
    }
}

pub fn draw_oled_card_icon(painter: &egui::Painter, center: Pos2, kind: u8, color: Color32) {
    let stroke = Stroke::new(2.0_f32, color);
    match kind {
        0 => {
            // Pixel refresh: circular arrow + grid dots
            painter.circle_stroke(center, 12.0, stroke);
            for dx in [-4.0, 0.0, 4.0] {
                for dy in [-4.0, 0.0, 4.0] {
                    painter.rect_filled(
                        Rect::from_center_size(center + Vec2::new(dx, dy), Vec2::splat(2.2)),
                        Rounding::ZERO,
                        color,
                    );
                }
            }
        }
        1 => {
            // Burn-in clear: monitor with vertical bars
            let scr = Rect::from_center_size(center + Vec2::new(0.0, -2.0), Vec2::new(28.0, 18.0));
            painter.rect_stroke(scr, Rounding::same(3.0), stroke);
            painter.rect_filled(
                Rect::from_center_size(center + Vec2::new(0.0, -2.0), Vec2::new(8.0, 12.0)),
                Rounding::ZERO,
                color,
            );
            painter.line_segment(
                [center + Vec2::new(-7.0, 10.0), center + Vec2::new(7.0, 10.0)],
                stroke,
            );
        }
        _ => {
            // Screen saver: monitor outline
            let scr = Rect::from_center_size(center + Vec2::new(0.0, -2.0), Vec2::new(28.0, 18.0));
            painter.rect_stroke(scr, Rounding::same(3.0), stroke);
            painter.line_segment(
                [center + Vec2::new(-7.0, 10.0), center + Vec2::new(7.0, 10.0)],
                stroke,
            );
        }
    }
}
