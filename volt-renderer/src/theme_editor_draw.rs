use super::{BgVertex, GlyphVertex, Renderer};
use crate::theme_editor::{EditorRow, ThemeEditorLayout, ThemeEditorView, BASE_FIELDS};

pub(super) struct EditorGeometry {
    view: ThemeEditorView,
    layout: ThemeEditorLayout,
    bg: Vec<BgVertex>,
    glyphs: Vec<GlyphVertex>,
}

// Fixed neutral colors, independent of the theme being edited: the editor
// must stay readable even mid-edit, e.g. with foreground == background.
const SHADOW: [f32; 4] = [0., 0., 0., 0.35];
const BORDER: [f32; 4] = [0.42, 0.42, 0.47, 1.];
const PANEL: [f32; 4] = [0.12, 0.12, 0.14, 1.];
const FOCUS_ROW: [f32; 4] = [0.19, 0.19, 0.23, 1.];
const TEXT: [f32; 4] = [0.95, 0.95, 0.97, 1.];
const SECONDARY: [f32; 4] = [0.70, 0.70, 0.75, 1.];
const MUTED: [f32; 4] = [0.55, 0.55, 0.60, 1.];
const FIELD: [f32; 4] = [0.08, 0.08, 0.10, 1.];
const FIELD_BORDER: [f32; 4] = [0.26, 0.26, 0.30, 1.];
const ACCENT: [f32; 4] = [0.48, 0.72, 0.98, 1.];
const INVALID: [f32; 4] = [0.88, 0.42, 0.46, 1.];
const SWATCH_EDGE: [f32; 4] = [1., 1., 1., 0.22];
const BUTTON: [f32; 4] = [0.32, 0.32, 0.37, 1.];
const BUTTON_FILL: [f32; 4] = [0.24, 0.24, 0.29, 1.];
const OK: [f32; 4] = [0.55, 0.82, 0.58, 1.];
const ERROR: [f32; 4] = [0.95, 0.52, 0.52, 1.];

/// Truncate to the columns that fit `width` at `size`, ending in `…`.
fn fit(text: &str, width: f32, size: f32) -> String {
    let cap = (width / (size * 0.6)).max(1.) as usize;
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars().filter(|c| !c.is_control()) {
        let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if used + w > cap {
            out.pop();
            out.push('…');
            break;
        }
        out.push(c);
        used += w;
    }
    out
}

impl Renderer {
    pub fn theme_editor_layout(&self) -> Option<ThemeEditorLayout> {
        self.theme_editor.as_ref()?;
        ThemeEditorLayout::new(
            self.config.width as f32,
            self.config.height as f32,
            self.workspace_card_top_offset(),
            self.scale_factor,
        )
    }

    pub(super) fn draw_theme_editor(
        &mut self,
        bg: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        view: &ThemeEditorView,
    ) {
        // Same layout function the click hit-test uses, so they can't drift.
        let Some(layout) = self.theme_editor_layout() else {
            return;
        };
        let cached = self
            .theme_editor_cache
            .as_ref()
            .is_some_and(|c| c.view == *view && c.layout == layout);
        if !cached {
            let mut b = vec![];
            let mut g = vec![];
            self.build_theme_editor(&mut b, &mut g, view, &layout);
            self.theme_editor_cache = Some(EditorGeometry {
                view: view.clone(),
                layout,
                bg: b,
                glyphs: g,
            });
        }
        if let Some(c) = &self.theme_editor_cache {
            bg.extend_from_slice(&c.bg);
            glyphs.extend_from_slice(&c.glyphs);
        }
    }

    /// Text vertically centred in a box of height `h` at `y`.
    fn editor_text_y(y: f32, h: f32, size: f32) -> f32 {
        y + (h - size * 1.2) / 2.
    }

    fn build_theme_editor(
        &mut self,
        bg: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        v: &ThemeEditorView,
        l: &ThemeEditorLayout,
    ) {
        let s = l.scale;
        let (x, y, w, h) = (l.x, l.y, l.w, l.h);
        self.card_round(bg, [x + 2. * s, y + 5. * s, w, h], 12. * s, SHADOW);
        self.card_round(bg, [x, y, w, h], 12. * s, BORDER);
        self.card_round(bg, [x + s, y + s, w - 2. * s, h - 2. * s], 11. * s, PANEL);

        let pad = 14. * s;
        self.card_text(
            glyphs,
            "Customize theme",
            x + pad,
            y + 12. * s,
            13. * s,
            TEXT,
        );
        let subtitle = fit(&v.subtitle, w - 2. * pad - 20. * s, 11. * s);
        self.card_text(glyphs, &subtitle, x + pad, y + 31. * s, 11. * s, SECONDARY);
        self.card_text(glyphs, "×", x + w - 26. * s, y + 7. * s, 17. * s, SECONDARY);

        // The name field owns the keyboard while naming; no color is focused.
        let focus = v.naming.is_none().then_some(v.focus);
        self.card_text(glyphs, "Base colors", x + pad, y + 49. * s, 11. * s, MUTED);
        for (i, row) in v.rows.iter().enumerate() {
            let rect = l.base_row(i);
            self.draw_editor_row(bg, glyphs, l, rect, row, focus == Some(i));
        }

        self.card_text(
            glyphs,
            "ANSI palette · normal, then bright",
            x + pad,
            y + 229. * s,
            11. * s,
            MUTED,
        );
        for (i, color) in v.ansi.iter().enumerate() {
            let [cx, cy, cw, ch] = l.ansi_cell(i);
            if focus == Some(BASE_FIELDS + i) {
                self.card_round(
                    bg,
                    [cx - 3. * s, cy - 3. * s, cw + 6. * s, ch + 6. * s],
                    6. * s,
                    TEXT,
                );
                self.card_round(
                    bg,
                    [cx - 1.5 * s, cy - 1.5 * s, cw + 3. * s, ch + 3. * s],
                    5. * s,
                    PANEL,
                );
            }
            self.card_round(bg, [cx, cy, cw, ch], 4. * s, SWATCH_EDGE);
            self.card_round(
                bg,
                [cx + s, cy + s, cw - 2. * s, ch - 2. * s],
                3. * s,
                color.to_f32(),
            );
        }
        let detail_focused = focus.is_some_and(|f| f >= BASE_FIELDS);
        self.draw_editor_row(bg, glyphs, l, l.detail_row(), &v.detail, detail_focused);

        self.draw_rect(
            bg,
            x + pad,
            y + 352. * s,
            w - 2. * pad,
            s.max(1.),
            FIELD_BORDER,
        );
        let save = l.save_button();
        match &v.naming {
            Some((name, cursor)) => {
                let f = l.name_field();
                self.card_round(bg, f, 5. * s, ACCENT);
                self.card_round(
                    bg,
                    [f[0] + s, f[1] + s, f[2] - 2. * s, f[3] - 2. * s],
                    4. * s,
                    FIELD,
                );
                let size = 12. * s;
                let inner = f[2] - 16. * s;
                // Keep the cursor in view for names longer than the field.
                let cols = (inner / (size * 0.6)).max(1.) as usize;
                let skip = cursor.saturating_sub(cols.saturating_sub(1));
                let shown: String = name.chars().skip(skip).take(cols).collect();
                let ty = Self::editor_text_y(f[1], f[3], size);
                if name.is_empty() {
                    self.card_text(glyphs, "Theme name", f[0] + 8. * s, ty, size, MUTED);
                } else {
                    self.card_text(glyphs, &shown, f[0] + 8. * s, ty, size, TEXT);
                }
                let before: String = shown.chars().take(cursor - skip).collect();
                let cx = f[0] + 8. * s + Self::approx_text_width(&before, size);
                self.draw_rect(
                    bg,
                    cx,
                    f[1] + 5. * s,
                    (1.5 * s).max(1.),
                    f[3] - 10. * s,
                    TEXT,
                );
            }
            None => {
                let r = l.revert_button();
                self.card_round(bg, r, 5. * s, BUTTON);
                self.card_round(
                    bg,
                    [r[0] + s, r[1] + s, r[2] - 2. * s, r[3] - 2. * s],
                    4. * s,
                    PANEL,
                );
                let label_x = r[0] + (r[2] - Self::approx_text_width("Revert", 12. * s)) / 2.;
                let ly = Self::editor_text_y(r[1], r[3], 12. * s);
                self.card_text(glyphs, "Revert", label_x, ly, 12. * s, TEXT);
            }
        }
        self.card_round(bg, save, 5. * s, BUTTON);
        self.card_round(
            bg,
            [save[0] + s, save[1] + s, save[2] - 2. * s, save[3] - 2. * s],
            4. * s,
            BUTTON_FILL,
        );
        let label = if v.naming.is_some() {
            "Save"
        } else {
            v.save_label.as_str()
        };
        let label_x = save[0] + (save[2] - Self::approx_text_width(label, 12. * s)) / 2.;
        let ly = Self::editor_text_y(save[1], save[3], 12. * s);
        self.card_text(glyphs, label, label_x, ly, 12. * s, TEXT);

        let (message, color) = match &v.status {
            Some((message, true)) => (message.as_str(), ERROR),
            Some((message, false)) => (message.as_str(), OK),
            None if v.naming.is_some() => ("Enter save · Esc back", MUTED),
            None => ("Tab next field · ⌘S save · Esc cancel", MUTED),
        };
        let text = fit(message, w - 2. * pad, 11. * s);
        self.card_text(glyphs, &text, x + pad, y + 394. * s, 11. * s, color);
    }

    fn draw_editor_row(
        &mut self,
        bg: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        l: &ThemeEditorLayout,
        rect: [f32; 4],
        row: &EditorRow,
        focused: bool,
    ) {
        let s = l.scale;
        if focused {
            self.card_round(bg, rect, 5. * s, FOCUS_ROW);
        }
        let sw = l.swatch_in(rect);
        self.card_round(bg, sw, 3. * s, SWATCH_EDGE);
        self.card_round(
            bg,
            [sw[0] + s, sw[1] + s, sw[2] - 2. * s, sw[3] - 2. * s],
            2. * s,
            row.color.to_f32(),
        );

        let size = 12. * s;
        let ty = Self::editor_text_y(rect[1], rect[3], size);
        let f = l.field_in(rect);
        let label = fit(&row.label, f[0] - (sw[0] + sw[2] + 8. * s) - 6. * s, size);
        self.card_text(
            glyphs,
            &label,
            sw[0] + sw[2] + 8. * s,
            ty,
            size,
            if focused { TEXT } else { SECONDARY },
        );

        let edge = if !row.valid {
            INVALID
        } else if focused {
            ACCENT
        } else {
            FIELD_BORDER
        };
        self.card_round(bg, f, 4. * s, edge);
        self.card_round(
            bg,
            [f[0] + s, f[1] + s, f[2] - 2. * s, f[3] - 2. * s],
            3. * s,
            FIELD,
        );
        let text_x = f[0] + 7. * s;
        let fty = Self::editor_text_y(f[1], f[3], size);
        self.card_text(glyphs, &row.text, text_x, fty, size, TEXT);
        if focused {
            let cx = text_x + Self::approx_text_width(&row.text, size);
            self.draw_rect(
                bg,
                cx,
                f[1] + 4. * s,
                (1.5 * s).max(1.),
                f[3] - 8. * s,
                TEXT,
            );
        }
    }
}
