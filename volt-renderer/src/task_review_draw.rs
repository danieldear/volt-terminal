use super::{BgVertex, GlyphVertex, Renderer, Theme};
use crate::task_review::{ReviewLayout, TaskReview};
use volt_ui_kit::{
    button::{Button, ButtonColors},
    CardIcon, Colors, Painter,
};
pub(super) struct ReviewGeometry {
    view: TaskReview,
    size: [f32; 3],
    colors: Colors,
    bg: Vec<BgVertex>,
    glyphs: Vec<GlyphVertex>,
}
impl Renderer {
    pub(super) fn draw_task_review(
        &mut self,
        b: &mut Vec<BgVertex>,
        g: &mut Vec<GlyphVertex>,
        v: &TaskReview,
        theme: &Theme,
    ) {
        let size = [
            self.config.width as f32,
            self.config.height as f32,
            self.scale_factor,
        ];
        let colors = Colors::from_theme(&super::ui_kit_theme(theme));
        if !self
            .task_review_cache
            .as_ref()
            .is_some_and(|c| c.view == *v && c.size == size && c.colors == colors)
        {
            let mut bg = vec![];
            let mut glyphs = vec![];
            self.build_task_review(&mut bg, &mut glyphs, v, theme);
            self.task_review_cache = Some(ReviewGeometry {
                view: v.clone(),
                size,
                colors,
                bg,
                glyphs,
            });
        }
        if let Some(c) = &self.task_review_cache {
            b.extend_from_slice(&c.bg);
            g.extend_from_slice(&c.glyphs);
        }
    }
    fn build_task_review(
        &mut self,
        b: &mut Vec<BgVertex>,
        g: &mut Vec<GlyphVertex>,
        v: &TaskReview,
        theme: &Theme,
    ) {
        let Some(l) = ReviewLayout::new(
            self.config.width as f32,
            self.config.height as f32,
            self.scale_factor,
            v,
        ) else {
            self.card_text(
                g,
                "Enlarge the window to review this task. Esc cancels.",
                12.,
                40.,
                12.,
                theme.foreground.to_f32(),
            );
            return;
        };
        let c = Colors::from_theme(&super::ui_kit_theme(theme));
        let s = l.scale;
        self.draw_rect(
            b,
            0.,
            0.,
            self.config.width as f32,
            self.config.height as f32,
            [0., 0., 0., 0.28],
        );
        self.card_round(b, l.panel, 16. * s, c.border);
        self.card_round(
            b,
            [
                l.panel[0] + s,
                l.panel[1] + s,
                l.panel[2] - 2. * s,
                l.panel[3] - 2. * s,
            ],
            15. * s,
            c.panel,
        );
        self.card_icon(
            b,
            if v.mode == crate::task_review::ReviewMode::TrustFile {
                CardIcon::File
            } else {
                CardIcon::Play
            },
            l.panel[0] + 20. * s,
            l.panel[1] + 22. * s,
            s,
            c.accent,
        );
        self.card_text(
            g,
            v.mode.title(),
            l.panel[0] + 44. * s,
            l.panel[1] + 19. * s,
            17. * s,
            c.text,
        );
        let cols = ((l.panel[2] - 40. * s) / (12. * s * 0.7)) as usize;
        let safe_name = crate::task_review::wrap(&v.name, cols).join(" ");
        let name = crate::prompt_viewport::window(&safe_name, 0, cols).0;
        self.card_text(
            g,
            &name,
            l.panel[0] + 20. * s,
            l.panel[1] + 50. * s,
            13. * s,
            c.accent,
        );
        let reason_cols = ((l.panel[2] - 40. * s) / (11. * s * 0.72)).floor().max(1.) as usize;
        for (i, line) in crate::task_review::wrap(v.mode.reason(), reason_cols)
            .iter()
            .take(2)
            .enumerate()
        {
            self.card_text(
                g,
                line,
                l.panel[0] + 20. * s,
                l.panel[1] + (76. + i as f32 * 15.) * s,
                11. * s,
                c.muted,
            );
        }
        self.card_round(b, l.body, 8. * s, c.selected);
        for (i, line) in l.lines.iter().skip(l.first).take(l.count).enumerate() {
            self.card_text(
                g,
                line,
                l.body[0] + 12. * s,
                l.body[1] + 10. * s + i as f32 * 19. * s,
                13. * s,
                c.text,
            );
        }
        if l.lines.len() > l.count {
            let track = [
                l.body[0] + l.body[2] - 6. * s,
                l.body[1] + 6. * s,
                2. * s,
                l.body[3] - 12. * s,
            ];
            self.card_round(b, track, s, c.border);
            let thumb_h = (track[3] * l.count as f32 / l.lines.len() as f32)
                .max(12. * s)
                .min(track[3]);
            let travel = (track[3] - thumb_h) * l.first as f32
                / l.lines.len().saturating_sub(l.count).max(1) as f32;
            self.card_round(
                b,
                [track[0], track[1] + travel, track[2], thumb_h],
                s,
                c.accent,
            );
        }
        let counter = if l.lines.len() > l.count {
            format!(
                "{}–{} / {} · ↑↓ scroll",
                l.first + 1,
                (l.first + l.count).min(l.lines.len()),
                l.lines.len()
            )
        } else {
            "Esc cancel · Tab focus · Enter".into()
        };
        // On narrow windows the footer hint would collide with the buttons.
        if l.panel[2] >= 480. * s {
            self.card_text(
                g,
                &counter,
                l.panel[0] + 20. * s,
                l.cancel[1] + 8. * s,
                10. * s,
                c.muted,
            );
        }
        for (rect, label, icon, primary) in [
            (l.cancel, "Cancel", CardIcon::Close, false),
            (l.primary, v.mode.action(), CardIcon::Check, true),
        ] {
            if primary == v.primary_focus {
                self.card_round(
                    b,
                    [
                        rect[0] - 2. * s,
                        rect[1] - 2. * s,
                        rect[2] + 4. * s,
                        rect[3] + 4. * s,
                    ],
                    9. * s,
                    c.accent,
                );
            }
            Button {
                label,
                icon: Some(icon),
                enabled: true,
            }
            .paint(
                self,
                b,
                g,
                rect,
                s,
                ButtonColors {
                    surface: if primary { c.accent } else { c.selected },
                    text: if primary { c.panel } else { c.text },
                },
            );
        }
    }
}
