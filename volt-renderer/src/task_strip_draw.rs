use super::search_palette_draw::Colors;
use super::{BgVertex, GlyphVertex, Renderer, Theme};
use crate::task_strip::{label, StripState, TaskStripLayout, TaskStripView, H, TEXT};

impl Renderer {
    pub fn task_strip_layout(&self) -> Option<TaskStripLayout> {
        let view = self.task_strip.as_ref()?;
        TaskStripLayout::new(
            view,
            self.config.width as f32,
            self.workspace_card_top_offset(),
            self.scale_factor,
        )
    }

    pub(super) fn draw_task_strip(
        &mut self,
        bg: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        view: &TaskStripView,
        theme: &Theme,
    ) {
        let Some(l) = self.task_strip_layout() else {
            return;
        };
        let c = Colors::from_theme(theme);
        let (red, green) = (theme.ansi[1].to_f32(), theme.ansi[2].to_f32());
        let s = l.scale;
        let pill = |r: [f32; 4]| [r[0] + s, r[1] + s, r[2] - 2. * s, r[3] - 2. * s];
        let size = TEXT * s;
        let ty = |r: [f32; 4]| r[1] + (H * s - size * 1.2) / 2.;
        for (task, r) in view.tasks.iter().zip(&l.buttons) {
            let r = *r;
            let edge = match task.state {
                StripState::Failed => red,
                StripState::Running => c.accent,
                _ => c.border,
            };
            self.card_round(bg, r, 8. * s, edge);
            self.card_round(bg, pill(r), 7. * s, c.panel);
            // Leading mark: ▶ idle, a dot while running, ✓ / ✗ when done.
            let (mx, my) = (r[0] + 13. * s, r[1] + r[3] / 2.);
            match task.state {
                StripState::Idle => {
                    let p = |dx: f32, dy: f32| [mx + dx * s, my + dy * s];
                    self.card_line(bg, p(-3., -4.5), p(-3., 4.5), 1.6 * s, c.accent);
                    self.card_line(bg, p(-3., -4.5), p(4., 0.), 1.6 * s, c.accent);
                    self.card_line(bg, p(-3., 4.5), p(4., 0.), 1.6 * s, c.accent);
                }
                StripState::Running => {
                    let d = 7. * s;
                    self.card_round(bg, [mx - d / 2., my - d / 2., d, d], d / 2., c.accent);
                }
                StripState::Succeeded => {
                    let p = |dx: f32, dy: f32| [mx + dx * s, my + dy * s];
                    self.card_line(bg, p(-4., 0.), p(-1., 3.5), 1.8 * s, green);
                    self.card_line(bg, p(-1., 3.5), p(4.5, -3.5), 1.8 * s, green);
                }
                StripState::Failed => {
                    let p = |dx: f32, dy: f32| [mx + dx * s, my + dy * s];
                    self.card_line(bg, p(-3.5, -3.5), p(3.5, 3.5), 1.8 * s, red);
                    self.card_line(bg, p(-3.5, 3.5), p(3.5, -3.5), 1.8 * s, red);
                }
            }
            self.card_text(
                glyphs,
                &label(&task.name),
                r[0] + 26. * s,
                ty(r),
                size,
                c.text,
            );
        }
        for (r, glyph) in l.more.iter().map(|r| (*r, "⋯")).chain([(l.add, "+")]) {
            self.card_round(bg, r, 8. * s, c.border);
            self.card_round(bg, pill(r), 7. * s, c.panel);
            let gx = r[0] + (r[2] - Self::approx_text_width(glyph, 15. * s)) / 2.;
            self.card_text(
                glyphs,
                glyph,
                gx,
                r[1] + (r[3] - 15. * s * 1.2) / 2.,
                15. * s,
                c.muted,
            );
        }
        if let Some(message) = &view.message {
            let size = 11.5 * s;
            let text: String = message.chars().take(90).collect();
            let w = Self::approx_text_width(&text, size);
            let x = l.message_right - w;
            self.card_round(
                bg,
                [
                    x - 8. * s,
                    l.message_y - 3. * s,
                    w + 16. * s,
                    size * 1.2 + 6. * s,
                ],
                6. * s,
                c.panel,
            );
            self.card_text(glyphs, &text, x, l.message_y, size, c.muted);
        }
    }
}
