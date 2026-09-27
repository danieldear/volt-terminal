use super::{BgVertex, GlyphVertex, Renderer, Theme};
use crate::search_palette::{SearchLayout, SearchView};
pub(super) struct SearchGeometry {
    view: SearchView,
    size: (u32, u32),
    scale: f32,
    top: f32,
    colors: ([f32; 4], [f32; 4]),
    bg: Vec<BgVertex>,
    glyphs: Vec<GlyphVertex>,
}
fn fit(text: &str, width: f32, size: f32) -> String {
    let cap = (width / (size * 0.65)).max(0.) as usize;
    let mut out = String::new();
    let mut n = 0;
    for c in text.chars().filter(|c| !c.is_control()) {
        let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if n + w > cap.saturating_sub(1) {
            out.push('…');
            break;
        }
        out.push(c);
        n += w;
    }
    out
}
impl Renderer {
    pub fn search_layout(&self) -> Option<SearchLayout> {
        self.search_palette.as_ref()?;
        SearchLayout::new(
            self.config.width as f32,
            self.config.height as f32,
            self.workspace_card_top_offset(),
            self.scale_factor,
        )
    }
    pub(super) fn draw_search_palette(
        &mut self,
        bg: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        view: &SearchView,
        theme: &Theme,
        top: f32,
    ) {
        let colors = (theme.background.to_f32(), theme.foreground.to_f32());
        let size = (self.config.width, self.config.height);
        let hit = self.search_palette_cache.as_ref().is_some_and(|c| {
            c.view == *view
                && c.size == size
                && c.scale == self.scale_factor
                && c.top == top
                && c.colors == colors
        });
        if !hit {
            let mut b = vec![];
            let mut g = vec![];
            self.build_search_palette(&mut b, &mut g, view, theme, top);
            self.search_palette_cache = Some(SearchGeometry {
                view: view.clone(),
                size,
                scale: self.scale_factor,
                top,
                colors,
                bg: b,
                glyphs: g,
            });
        }
        if let Some(c) = &self.search_palette_cache {
            bg.extend_from_slice(&c.bg);
            glyphs.extend_from_slice(&c.glyphs);
        }
    }
    fn build_search_palette(
        &mut self,
        bg: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        v: &SearchView,
        theme: &Theme,
        top: f32,
    ) {
        let Some(l) = SearchLayout::new(
            self.config.width as f32,
            self.config.height as f32,
            top,
            self.scale_factor,
        ) else {
            return;
        };
        let s = l.scale;
        let [x, y, w, h] = [l.x, l.y, l.w, l.h];
        let fg = theme.foreground.to_f32();
        let base = theme.background.to_f32();
        let panel = [base[0] * 0.83, base[1] * 0.83, base[2] * 0.83, 1.];
        let line = [fg[0] * 0.24, fg[1] * 0.24, fg[2] * 0.24, 1.];
        let muted = [fg[0] * 0.65, fg[1] * 0.65, fg[2] * 0.65, 1.];
        let accent = [0.48, 0.72, 0.98, 1.];
        self.card_round(
            bg,
            [x + 2. * s, y + 5. * s, w, h],
            18. * s,
            [0., 0., 0., 0.35],
        );
        self.card_round(bg, [x, y, w, h], 18. * s, line);
        self.card_round(bg, [x + s, y + s, w - 2. * s, h - 2. * s], 17. * s, panel);
        self.card_text(
            glyphs,
            "SEARCH WORKSPACE",
            x + 20. * s,
            y + 13. * s,
            11. * s,
            accent,
        );
        self.card_text(
            glyphs,
            &fit(&v.root, w - 224. * s, 9. * s),
            x + 180. * s,
            y + 15. * s,
            9. * s,
            muted,
        );
        self.card_text(glyphs, "×", x + w - 29. * s, y + 11. * s, 18. * s, muted);
        if v.query_selected && !v.query.is_empty() {
            let width = (unicode_width::UnicodeWidthStr::width(v.query.as_str()) as f32 * 9.6 * s)
                .min(w - 48. * s);
            self.card_round(
                bg,
                [x + 18. * s, y + 40. * s, width + 4. * s, 24. * s],
                3. * s,
                [0.18, 0.30, 0.43, 1.],
            );
        }
        let query = if v.query.is_empty() {
            "Files, output, text, branches, tasks…".into()
        } else {
            v.query.clone()
        };
        self.card_text(
            glyphs,
            &fit(&query, w - 48. * s, 16. * s),
            x + 20. * s,
            y + 42. * s,
            16. * s,
            if v.query.is_empty() { muted } else { fg },
        );
        // Query caret is presentation only; input remains owned by the modal controller.
        let before: String = v.query.chars().take(v.cursor).collect();
        let caret = unicode_width::UnicodeWidthStr::width(before.as_str()) as f32 * 9.6 * s;
        if !v.query_selected && caret < w - 50. * s {
            self.draw_rect(
                bg,
                x + 20. * s + caret,
                y + 43. * s,
                1. * s,
                19. * s,
                accent,
            );
        }
        let tw = (w - 24. * s) / 6.;
        for (i, label) in ["All", "Output", "Files", "Text", "Git", "Tasks"]
            .iter()
            .enumerate()
        {
            let tx = x + 12. * s + i as f32 * tw;
            if i == v.scope {
                self.card_round(
                    bg,
                    [tx, y + 76. * s, tw - 4. * s, 28. * s],
                    7. * s,
                    [0.18, 0.25, 0.32, 1.],
                );
            }
            self.card_text(
                glyphs,
                label,
                tx + 9. * s,
                y + 83. * s,
                11. * s,
                if i == v.scope { accent } else { muted },
            );
        }
        self.draw_rect(bg, x + 16. * s, y + 110. * s, w - 32. * s, s, line);
        let first = l.first(v.selected);
        for (slot, row) in v.rows.iter().skip(first).take(l.count).enumerate() {
            let ry = y + (116. + slot as f32 * 46.) * s;
            let selected = first + slot == v.selected;
            if selected {
                self.card_round(
                    bg,
                    [x + 8. * s, ry, w - 16. * s, 44. * s],
                    8. * s,
                    [0.16, 0.21, 0.26, 1.],
                );
                self.card_round(
                    bg,
                    [x + 9. * s, ry + 9. * s, 3. * s, 26. * s],
                    1.5 * s,
                    accent,
                );
            }
            let color = match row.kind.as_str() {
                "FILE" | "TEXT" => [0.45, 0.8, 0.78, 1.],
                "BRANCH" | "WORKTREE" => [0.76, 0.61, 0.95, 1.],
                "TASK" => [0.94, 0.76, 0.4, 1.],
                _ => accent,
            };
            self.card_text(glyphs, &row.kind, x + 20. * s, ry + 9. * s, 9. * s, color);
            self.card_text(
                glyphs,
                &fit(&row.title, w - 122. * s, 13. * s),
                x + 94. * s,
                ry + 5. * s,
                13. * s,
                fg,
            );
            self.card_text(
                glyphs,
                &fit(&row.detail, w - 122. * s, 10. * s),
                x + 94. * s,
                ry + 25. * s,
                10. * s,
                muted,
            );
        }
        if v.rows.is_empty() {
            self.card_text(
                glyphs,
                "No results yet — type or choose a scope",
                x + 20. * s,
                y + 130. * s,
                12. * s,
                muted,
            );
        }
        let py = y + (122. + l.count as f32 * 46.) * s;
        let bottom = y + h - 62. * s;
        if py + 18. * s < bottom {
            self.draw_rect(bg, x + 16. * s, py - 4. * s, w - 32. * s, s, line);
            for (i, text) in v.preview.iter().enumerate() {
                let yy = py + i as f32 * 16. * s;
                if yy + 15. * s > bottom {
                    break;
                }
                self.card_text(
                    glyphs,
                    &fit(text, w - 40. * s, 11. * s),
                    x + 20. * s,
                    yy,
                    11. * s,
                    muted,
                );
            }
        }
        self.card_text(
            glyphs,
            &fit(&v.status, w - 40. * s, 10. * s),
            x + 20. * s,
            y + h - 54. * s,
            10. * s,
            muted,
        );
        self.card_text(
            glyphs,
            &fit(&v.action, w - 40. * s, 10. * s),
            x + 20. * s,
            y + h - 36. * s,
            10. * s,
            accent,
        );
        self.card_text(
            glyphs,
            &fit(
                "↑ ↓ select   Tab scope   Enter preview   Esc close",
                w - 40. * s,
                9. * s,
            ),
            x + 20. * s,
            y + h - 19. * s,
            9. * s,
            muted,
        );
    }
}
