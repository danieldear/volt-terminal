use super::{BgVertex, GlyphVertex, Renderer, Theme};
use crate::search_palette::{
    SearchLayout, SearchRow, SearchView, FOOTER_H, PILL_H, PILL_TEXT, QUERY_H, ROW_H, SCOPES,
};

pub(super) struct SearchGeometry {
    view: SearchView,
    layout: SearchLayout,
    colors: Colors,
    bg: Vec<BgVertex>,
    glyphs: Vec<GlyphVertex>,
}

/// Panel colors derived from the active terminal theme: surfaces are the
/// background nudged toward the foreground, and the theme's yellow is the
/// accent (amber in Gruvbox, soft gold in Nord, and so on).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Colors {
    pub(super) shadow: [f32; 4],
    pub(super) border: [f32; 4],
    pub(super) panel: [f32; 4],
    pub(super) line: [f32; 4],
    pub(super) selected: [f32; 4],
    pub(super) text: [f32; 4],
    pub(super) muted: [f32; 4],
    pub(super) quiet: [f32; 4],
    pub(super) accent: [f32; 4],
    pub(super) accent_bright: [f32; 4],
    pub(super) accent_dim: [f32; 4],
}

impl Colors {
    pub(super) fn from_theme(theme: &Theme) -> Self {
        let bg = theme.background.to_f32();
        let fg = theme.foreground.to_f32();
        let mix = |t: f32| {
            [
                bg[0] + (fg[0] - bg[0]) * t,
                bg[1] + (fg[1] - bg[1]) * t,
                bg[2] + (fg[2] - bg[2]) * t,
                1.,
            ]
        };
        let accent = theme.ansi[3].to_f32();
        Self {
            shadow: [0., 0., 0., 0.45],
            border: mix(0.22),
            panel: mix(0.04),
            line: mix(0.11),
            selected: mix(0.10),
            text: fg,
            muted: mix(0.66),
            quiet: mix(0.48),
            accent,
            accent_bright: theme.ansi[11].to_f32(),
            accent_dim: [accent[0], accent[1], accent[2], 0.18],
        }
    }
}

/// Truncate to the columns that fit `width` at `size`, ending in `…`.
fn fit(text: &str, width: f32, size: f32) -> String {
    let cap = (width / (size * 0.6)).max(0.) as usize;
    let cw = |c: char| unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
    let clean = text.chars().filter(|c| !c.is_control());
    if clean.clone().map(cw).sum::<usize>() <= cap {
        return clean.collect();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in clean {
        if used + cw(c) > cap.saturating_sub(1) {
            break;
        }
        out.push(c);
        used += cw(c);
    }
    out.push('…');
    out
}

/// Char ranges → byte ranges of `text`, for per-glyph coloring.
fn byte_ranges(text: &str, hits: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let offsets: Vec<usize> = text
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(text.len()))
        .collect();
    let at = |c: usize| offsets[c.min(offsets.len() - 1)];
    hits.iter()
        .filter(|(a, b)| a < b)
        .map(|&(a, b)| (at(a), at(b)))
        .collect()
}

impl Renderer {
    pub fn search_layout(&self) -> Option<SearchLayout> {
        let view = self.search_palette.as_ref()?;
        SearchLayout::new(
            self.config.width as f32,
            self.config.height as f32,
            self.workspace_card_top_offset(),
            self.scale_factor,
            view.rows.len(),
        )
    }

    pub(super) fn draw_search_palette(
        &mut self,
        bg: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        view: &SearchView,
        theme: &Theme,
    ) {
        let colors = Colors::from_theme(theme);
        // Same layout function the click hit-test uses, so they can't drift.
        let Some(layout) = self.search_layout() else {
            return;
        };
        let cached = self
            .search_palette_cache
            .as_ref()
            .is_some_and(|c| c.view == *view && c.layout == layout && c.colors == colors);
        if !cached {
            let mut b = vec![];
            let mut g = vec![];
            self.build_search_palette(&mut b, &mut g, view, &layout, &colors);
            self.search_palette_cache = Some(SearchGeometry {
                view: view.clone(),
                layout,
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

    /// Text vertically centred in a box of height `h` at `y`.
    fn search_text_y(y: f32, h: f32, size: f32) -> f32 {
        y + (h - size * 1.2) / 2.
    }

    /// `text` in `color`, with `hits` (char ranges) in the bright accent.
    #[allow(clippy::too_many_arguments)]
    fn search_text(
        &mut self,
        glyphs: &mut Vec<GlyphVertex>,
        c: &Colors,
        text: &str,
        hits: &[(usize, usize)],
        x: f32,
        y: f32,
        size: f32,
        color: [f32; 4],
    ) {
        let ranges = byte_ranges(text, hits);
        self.draw_text_colored(glyphs, text, x, y, size, 1.2, &|i| {
            if ranges.iter().any(|&(a, b)| i >= a && i < b) {
                c.accent_bright
            } else {
                color
            }
        });
    }

    fn build_search_palette(
        &mut self,
        bg: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        v: &SearchView,
        l: &SearchLayout,
        c: &Colors,
    ) {
        let s = l.scale;
        let (x, y, w, h) = (l.x, l.y, l.w, l.h);
        self.card_round(bg, [x + 2. * s, y + 8. * s, w, h], 12. * s, c.shadow);
        self.card_round(bg, [x, y, w, h], 12. * s, c.border);
        self.card_round(bg, [x + s, y + s, w - 2. * s, h - 2. * s], 11. * s, c.panel);

        self.draw_query_row(bg, glyphs, v, l, c);
        self.draw_rect(bg, x + s, y + QUERY_H * s, w - 2. * s, s.max(1.), c.line);

        for (i, label) in SCOPES.iter().enumerate() {
            let p = l.scope_pill(i);
            let on = i == v.scope;
            if on {
                self.card_round(bg, p, 6. * s, c.accent_dim);
            }
            let size = PILL_TEXT * s;
            let tx = p[0] + (p[2] - Self::approx_text_width(label, size)) / 2.;
            let ty = Self::search_text_y(p[1], PILL_H * s, size);
            self.card_text(
                glyphs,
                label,
                tx,
                ty,
                size,
                if on { c.accent_bright } else { c.quiet },
            );
        }
        let list = l.list();
        self.draw_rect(bg, x + s, list[1] - s, w - 2. * s, s.max(1.), c.line);

        let first = l.first(v.selected);
        for (slot, row) in v.rows.iter().skip(first).take(l.count).enumerate() {
            let selected = first + slot == v.selected;
            self.draw_result(bg, glyphs, c, row, l.row(slot), selected, s);
        }
        if v.rows.is_empty() {
            let msg = if v.query.is_empty() {
                "Type to search files, text, terminal output, branches and tasks."
            } else {
                v.status.as_str()
            };
            let size = 13. * s;
            let text = fit(msg, list[2] - 40. * s, size);
            self.card_text(
                glyphs,
                &text,
                list[0] + 20. * s,
                list[1] + 22. * s,
                size,
                c.quiet,
            );
        }

        let f = l.footer();
        self.draw_rect(bg, x + s, f[1], w - 2. * s, s.max(1.), c.line);
        let size = 12. * s;
        let ty = Self::search_text_y(f[1], FOOTER_H * s, size);
        let hint_w = Self::approx_text_width(&v.hint, size);
        let status = fit(&v.status, w - hint_w - 64. * s, size);
        self.card_text(glyphs, &status, x + 18. * s, ty, size, c.quiet);
        self.card_text(glyphs, &v.hint, x + w - 18. * s - hint_w, ty, size, c.muted);
    }

    fn draw_query_row(
        &mut self,
        bg: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        v: &SearchView,
        l: &SearchLayout,
        c: &Colors,
    ) {
        let s = l.scale;
        let q = l.query_row();
        let cy = q[1] + q[3] / 2.;
        // Magnifier: ring plus handle.
        let (mx, my, r) = (q[0] + 26. * s, cy - 2. * s, 6.5 * s);
        self.card_round(bg, [mx - r, my - r, 2. * r, 2. * r], r, c.quiet);
        let ri = r - 1.8 * s;
        self.card_round(bg, [mx - ri, my - ri, 2. * ri, 2. * ri], ri, c.panel);
        self.card_line(
            bg,
            [mx + r * 0.7, my + r * 0.7],
            [mx + r * 1.45, my + r * 1.45],
            1.8 * s,
            c.quiet,
        );

        let size = 16. * s;
        let tx = q[0] + 48. * s;
        let ty = Self::search_text_y(q[1], q[3], size);
        let root_size = 11.5 * s;
        let root = fit(&v.root, q[2] * 0.3, root_size);
        let root_w = Self::approx_text_width(&root, root_size);
        let room = q[2] - 48. * s - root_w - 40. * s;
        if v.query.is_empty() {
            self.card_text(glyphs, "Search this project", tx, ty, size, c.quiet);
        } else {
            let query = fit(&v.query, room, size);
            if v.query_selected {
                let qw = Self::approx_text_width(&query, size);
                self.card_round(
                    bg,
                    [tx - 2. * s, ty - 1. * s, qw + 4. * s, size * 1.25],
                    3. * s,
                    c.accent_dim,
                );
            }
            self.card_text(glyphs, &query, tx, ty, size, c.text);
        }
        if !v.query_selected {
            let before: String = v.query.chars().take(v.cursor).collect();
            let cx = tx + Self::approx_text_width(&before, size);
            if cx < tx + room {
                self.draw_rect(bg, cx, ty + 1. * s, size * 0.6, size * 1.15, c.accent);
            }
        }
        let rty = Self::search_text_y(q[1], q[3], root_size);
        self.card_text(
            glyphs,
            &root,
            q[0] + q[2] - 18. * s - root_w,
            rty,
            root_size,
            c.quiet,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_result(
        &mut self,
        bg: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        c: &Colors,
        row: &SearchRow,
        r: [f32; 4],
        selected: bool,
        s: f32,
    ) {
        if selected {
            let sel = [
                r[0] + 8. * s,
                r[1] + 2. * s,
                r[2] - 16. * s,
                (ROW_H - 4.) * s,
            ];
            self.card_round(bg, sel, 6. * s, c.selected);
            self.card_round(bg, [sel[0], sel[1], 2. * s, sel[3]], 1. * s, c.accent);
        }
        let kind_x = r[0] + 22. * s;
        let text_x = r[0] + 96. * s;
        let room = r[0] + r[2] - text_x - 18. * s;
        self.card_text(
            glyphs,
            &row.kind,
            kind_x,
            r[1] + 11. * s,
            11.5 * s,
            if selected { c.muted } else { c.quiet },
        );
        let size = 13.5 * s;
        let title = fit(&row.title, room, size);
        self.search_text(
            glyphs,
            c,
            &title,
            &row.hits,
            text_x,
            r[1] + 9. * s,
            size,
            c.text,
        );
        let detail = fit(&row.detail, room, 12. * s);
        self.card_text(glyphs, &detail, text_x, r[1] + 29. * s, 12. * s, c.quiet);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_ranges_map_to_bytes_for_multibyte_text() {
        // "é" is 2 bytes; char range 1..3 covers "éx".
        assert_eq!(byte_ranges("aéxb", &[(1, 3)]), vec![(1, 4)]);
        assert_eq!(byte_ranges("abc", &[(2, 9)]), vec![(2, 3)]);
        assert!(byte_ranges("abc", &[(2, 2)]).is_empty());
    }

    #[test]
    fn colors_follow_the_theme() {
        let gruvbox = Colors::from_theme(&Theme::by_name("gruvbox"));
        let nord = Colors::from_theme(&Theme::by_name("nord"));
        assert_ne!(gruvbox, nord);
        let theme = Theme::by_name("gruvbox");
        assert_eq!(gruvbox.accent, theme.ansi[3].to_f32());
        assert_eq!(gruvbox.text, theme.foreground.to_f32());
        // Surfaces sit between background and foreground, closest to the background.
        let (bg, fg) = (theme.background.to_f32(), theme.foreground.to_f32());
        for ch in 0..3 {
            let lo = bg[ch].min(fg[ch]);
            let hi = bg[ch].max(fg[ch]);
            assert!((lo..=hi).contains(&gruvbox.panel[ch]));
            assert!((gruvbox.panel[ch] - bg[ch]).abs() < (gruvbox.muted[ch] - bg[ch]).abs() + 1e-6);
        }
    }

    #[test]
    fn fit_truncates_with_an_ellipsis() {
        assert_eq!(fit("abcdef", 6. * 0.6 * 10., 10.), "abcdef");
        assert_eq!(fit("abcdefgh", 6. * 0.6 * 10., 10.), "abcde…");
    }
}
