use crate::{paint::Painter, style::Colors, task_form::*};
fn fit(text: &str, width: f32, size: f32) -> String {
    let cap = (width / (size * 0.6)).max(0.) as usize;
    let count = text.chars().count();
    if count <= cap {
        return text.to_string();
    }
    let mut out: String = text.chars().take(cap.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[allow(clippy::too_many_arguments)]
pub trait FormPaint: Painter {
    fn form_text_y(y: f32, h: f32, size: f32) -> f32 {
        y + (h - size * 1.2) / 2.
    }

    fn build_task_form(
        &mut self,
        bg: &mut Vec<Self::Shape>,
        glyphs: &mut Vec<Self::Glyph>,
        v: &TaskFormView,
        l: &TaskFormLayout,
        c: &Colors,
        (error, ok): ([f32; 4], [f32; 4]),
    ) {
        let s = l.scale;
        let (x, y, w, h) = (l.x, l.y, l.w, l.h);
        self.card_round(bg, [x + 2. * s, y + 8. * s, w, h], 12. * s, c.shadow);
        self.card_round(bg, [x, y, w, h], 12. * s, c.border);
        self.card_round(bg, [x + s, y + s, w - 2. * s, h - 2. * s], 11. * s, c.panel);

        let pad = PAD * s;
        self.card_text(glyphs, &v.title, x + pad, y + 20. * s, 16. * s, c.text);
        let project = fit(&format!("Saved to {}", v.project), w - 2. * pad, 11.5 * s);
        self.card_text(glyphs, &project, x + pad, y + 44. * s, 11.5 * s, c.quiet);

        for (i, field) in v.fields.iter().enumerate().take(FIELDS) {
            self.draw_form_field(bg, glyphs, l, c, i, field, v.focus == i);
        }

        // "Ask before running" checkbox.
        let r = l.confirm();
        let box_size = 16. * s;
        let bx = r[0];
        let by = r[1] + (r[3] - box_size) / 2.;
        let focused = v.focus == FIELDS;
        self.card_round(
            bg,
            [bx, by, box_size, box_size],
            4. * s,
            if focused || v.confirm {
                c.accent
            } else {
                c.border
            },
        );
        if !v.confirm {
            self.card_round(
                bg,
                [
                    bx + 1.5 * s,
                    by + 1.5 * s,
                    box_size - 3. * s,
                    box_size - 3. * s,
                ],
                3. * s,
                c.panel,
            );
        } else {
            let tick = 1.8 * s;
            self.card_line(
                bg,
                [bx + 4. * s, by + 8.5 * s],
                [bx + 7. * s, by + 11.5 * s],
                tick,
                c.panel,
            );
            self.card_line(
                bg,
                [bx + 7. * s, by + 11.5 * s],
                [bx + 12.5 * s, by + 5. * s],
                tick,
                c.panel,
            );
        }
        let size = 13. * s;
        let ty = Self::form_text_y(r[1], r[3], size);
        let label = "Ask before running";
        self.card_text(glyphs, label, bx + box_size + 10. * s, ty, size, c.text);
        let hint_x = bx + box_size + 10. * s + Self::approx_text_width(label, size) + 10. * s;
        self.card_text(
            glyphs,
            "shows the command first",
            hint_x,
            ty + 1. * s,
            11.5 * s,
            c.quiet,
        );

        // Footer: status on the left, Cancel and Save on the right.
        let cancel = l.cancel();
        let save = l.save();
        if let Some((message, is_error)) = &v.status {
            let text = fit(message, cancel[0] - x - pad - 12. * s, 12. * s);
            let ty = Self::form_text_y(save[1], BUTTON_H * s, 12. * s);
            self.card_text(
                glyphs,
                &text,
                x + pad,
                ty,
                12. * s,
                if *is_error { error } else { ok },
            );
        }
        self.card_round(bg, cancel, 7. * s, c.border);
        self.card_round(
            bg,
            [
                cancel[0] + s,
                cancel[1] + s,
                cancel[2] - 2. * s,
                cancel[3] - 2. * s,
            ],
            6. * s,
            c.panel,
        );
        self.card_round(bg, save, 7. * s, c.accent);
        for (rect, label, color) in [(cancel, "Cancel", c.text), (save, "Save task", c.panel)] {
            let size = 13. * s;
            let tx = rect[0] + (rect[2] - Self::approx_text_width(label, size)) / 2.;
            let ty = Self::form_text_y(rect[1], rect[3], size);
            self.card_text(glyphs, label, tx, ty, size, color);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_form_field(
        &mut self,
        bg: &mut Vec<Self::Shape>,
        glyphs: &mut Vec<Self::Glyph>,
        l: &TaskFormLayout,
        c: &Colors,
        i: usize,
        field: &FormField,
        focused: bool,
    ) {
        let s = l.scale;
        let label = l.label(i);
        self.card_text(
            glyphs,
            &field.label,
            label[0],
            label[1] + 2. * s,
            12. * s,
            c.muted,
        );
        let input = l.input(i);
        self.card_round(bg, input, 7. * s, if focused { c.accent } else { c.line });
        self.card_round(
            bg,
            [
                input[0] + s,
                input[1] + s,
                input[2] - 2. * s,
                input[3] - 2. * s,
            ],
            6. * s,
            c.selected,
        );
        let size = 13.5 * s;
        let tx = input[0] + 12. * s;
        let ty = Self::form_text_y(input[1], INPUT_H * s, size);
        let room = input[2] - 24. * s;
        if field.value.is_empty() {
            self.card_text(
                glyphs,
                &fit(&field.placeholder, room, size),
                tx,
                ty,
                size,
                c.quiet,
            );
        }
        // Keep the caret in view for long commands.
        let cols = (room / (size * 0.6)).max(1.) as usize;
        let cursor = field.cursor.unwrap_or(0);
        let skip = cursor.saturating_sub(cols.saturating_sub(1));
        if !field.value.is_empty() {
            let shown: String = field.value.chars().skip(skip).take(cols).collect();
            self.card_text(glyphs, &shown, tx, ty, size, c.text);
        }
        if focused {
            if let Some(cursor) = field.cursor {
                let before: String = field.value.chars().skip(skip).take(cursor - skip).collect();
                let cx = tx + Self::approx_text_width(&before, size);
                self.draw_rect(
                    bg,
                    cx,
                    input[1] + 8. * s,
                    (1.6 * s).max(1.),
                    input[3] - 16. * s,
                    c.accent,
                );
            }
        }
    }
}
impl<P: Painter> FormPaint for P {}
