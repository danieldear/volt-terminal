use super::{BgVertex, GlyphVertex, Renderer, Theme};
use crate::workspace_card::{CardIcon, CardLayout, CardTone, WorkspaceCard};
pub(super) struct CardGeometry {
    card: WorkspaceCard,
    size: (u32, u32),
    scale: f32,
    top: f32,
    colors: ([f32; 4], [f32; 4]),
    font: String,
    bg: Vec<BgVertex>,
    glyphs: Vec<GlyphVertex>,
}
impl Renderer {
    pub(super) fn draw_workspace_card(
        &mut self,
        bg: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        card: &WorkspaceCard,
        theme: &Theme,
        top: f32,
    ) {
        let colors = (theme.background.to_f32(), theme.foreground.to_f32());
        let size = (self.config.width, self.config.height);
        let hit = self.workspace_card_cache.as_ref().is_some_and(|cache| {
            cache.card == *card
                && cache.size == size
                && cache.scale == self.scale_factor
                && cache.top == top
                && cache.colors == colors
                && cache.font == self.font_family
        });
        if !hit {
            let mut card_bg = Vec::new();
            let mut card_glyphs = Vec::new();
            self.build_workspace_card(&mut card_bg, &mut card_glyphs, card, theme, top);
            self.workspace_card_cache = Some(CardGeometry {
                card: card.clone(),
                size,
                scale: self.scale_factor,
                top,
                colors,
                font: self.font_family.clone(),
                bg: card_bg,
                glyphs: card_glyphs,
            });
        }
        if let Some(cache) = &self.workspace_card_cache {
            bg.extend_from_slice(&cache.bg);
            glyphs.extend_from_slice(&cache.glyphs);
        }
    }

    pub(super) fn card_text(
        &mut self,
        glyphs: &mut Vec<GlyphVertex>,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        color: [f32; 4],
    ) {
        self.draw_text_with_line_height(glyphs, text, x, y, size, 1.2, color);
    }

    pub fn workspace_card_layout(&self) -> Option<CardLayout> {
        let card = self.workspace_card.as_ref()?;
        CardLayout::for_card(
            self.config.width as f32,
            self.config.height as f32,
            self.workspace_card_top_offset(),
            self.scale_factor,
            card,
        )
    }
    pub fn workspace_card_reserved_width(&self) -> f32 {
        if self.workspace_card.as_ref().is_some_and(|c| !c.floating)
            && self.workspace_card_layout().is_some()
        {
            340.0 * self.scale_factor
        } else {
            0.0
        }
    }
    pub fn workspace_card_top_offset(&self) -> f32 {
        self.top_offset_for_tab_count(1) + self.alert_bar_height()
    }
    fn card_triangle(&self, bg: &mut Vec<BgVertex>, pts: [[f32; 2]; 3], color: [f32; 4]) {
        for p in pts {
            bg.push(BgVertex {
                pos: [
                    p[0] / self.config.width as f32 * 2. - 1.,
                    1. - p[1] / self.config.height as f32 * 2.,
                ],
                color,
            });
        }
    }
    fn card_line(
        &self,
        bg: &mut Vec<BgVertex>,
        a: [f32; 2],
        b: [f32; 2],
        width: f32,
        color: [f32; 4],
    ) {
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let len = dx.hypot(dy).max(0.001);
        let n = [-dy / len * width / 2., dx / len * width / 2.];
        let p = [a[0] + n[0], a[1] + n[1]];
        let q = [a[0] - n[0], a[1] - n[1]];
        let r = [b[0] + n[0], b[1] + n[1]];
        let t = [b[0] - n[0], b[1] - n[1]];
        self.card_triangle(bg, [p, q, r], color);
        self.card_triangle(bg, [q, t, r], color);
    }
    pub(super) fn card_round(
        &self,
        bg: &mut Vec<BgVertex>,
        rect: [f32; 4],
        radius: f32,
        color: [f32; 4],
    ) {
        let [x, y, w, h] = rect;
        let r = radius.min(w / 2.).min(h / 2.);
        self.draw_rect(bg, x + r, y, w - 2. * r, h, color);
        self.draw_rect(bg, x, y + r, r, h - 2. * r, color);
        self.draw_rect(bg, x + w - r, y + r, r, h - 2. * r, color);
        for (cx, cy, start) in [
            (x + r, y + r, 180_f32),
            (x + w - r, y + r, 270.),
            (x + w - r, y + h - r, 0.),
            (x + r, y + h - r, 90.),
        ] {
            for i in 0..16 {
                let a = (start + i as f32 * 90. / 16.).to_radians();
                let b = (start + (i + 1) as f32 * 90. / 16.).to_radians();
                self.card_triangle(
                    bg,
                    [
                        [cx, cy],
                        [cx + r * a.cos(), cy + r * a.sin()],
                        [cx + r * b.cos(), cy + r * b.sin()],
                    ],
                    color,
                );
            }
        }
    }
    fn card_icon(
        &self,
        bg: &mut Vec<BgVertex>,
        icon: CardIcon,
        x: f32,
        y: f32,
        s: f32,
        c: [f32; 4],
    ) {
        // A coherent 16-unit outline family. No dependency on terminal font glyphs.
        let paths: &[&[[f32; 2]]] = match icon {
            CardIcon::Folder => &[&[
                [1., 4.],
                [6., 4.],
                [8., 6.],
                [15., 6.],
                [15., 13.],
                [1., 13.],
                [1., 4.],
            ]],
            CardIcon::Branch => &[
                &[[4., 2.], [4., 14.]],
                &[[4., 10.], [12., 6.], [12., 2.]],
                &[[2., 2.], [6., 2.]],
                &[[10., 2.], [14., 2.]],
            ],
            CardIcon::File => &[
                &[
                    [3., 1.],
                    [10., 1.],
                    [14., 5.],
                    [14., 15.],
                    [3., 15.],
                    [3., 1.],
                ],
                &[[10., 1.], [10., 5.], [14., 5.]],
            ],
            CardIcon::Play => &[&[[4., 2.], [13., 8.], [4., 14.], [4., 2.]]],
            CardIcon::Agent => &[
                &[[8., 0.], [8., 3.]],
                &[[2., 4.], [14., 4.], [14., 13.], [2., 13.], [2., 4.]],
                &[[5., 7.], [5., 9.]],
                &[[11., 7.], [11., 9.]],
            ],
            CardIcon::Link => &[
                &[
                    [6., 11.],
                    [3., 14.],
                    [0., 11.],
                    [0., 8.],
                    [5., 3.],
                    [8., 3.],
                ],
                &[
                    [8., 13.],
                    [11., 13.],
                    [16., 8.],
                    [16., 5.],
                    [13., 2.],
                    [10., 5.],
                ],
                &[[5., 11.], [11., 5.]],
            ],
            CardIcon::Server => &[
                &[[1., 2.], [15., 2.], [15., 14.], [1., 14.], [1., 2.]],
                &[[1., 8.], [15., 8.]],
                &[[4., 5.], [6., 5.]],
                &[[4., 11.], [6., 11.]],
            ],
            CardIcon::Pin | CardIcon::Unpin => &[
                &[
                    [5., 2.],
                    [11., 2.],
                    [10., 7.],
                    [13., 10.],
                    [3., 10.],
                    [6., 7.],
                    [5., 2.],
                ],
                &[[8., 10.], [8., 15.]],
            ],
            CardIcon::Minimize => &[&[[3., 8.], [13., 8.]]],
            CardIcon::Expand => &[&[[3., 8.], [13., 8.]], &[[8., 3.], [8., 13.]]],
            CardIcon::Close => &[&[[4., 4.], [12., 12.]], &[[12., 4.], [4., 12.]]],
            CardIcon::Refresh => &[
                &[
                    [13., 6.],
                    [11., 2.],
                    [5., 2.],
                    [2., 5.],
                    [2., 11.],
                    [5., 14.],
                    [11., 14.],
                    [14., 11.],
                ],
                &[[9., 6.], [14., 6.], [14., 1.]],
            ],
        };
        let transform = |p: [f32; 2]| {
            let [px, py] = if icon == CardIcon::Unpin {
                let a = std::f32::consts::FRAC_1_SQRT_2;
                [
                    (p[0] - 8.) * a - (p[1] - 8.) * a + 8.,
                    (p[0] - 8.) * a + (p[1] - 8.) * a + 8.,
                ]
            } else {
                p
            };
            [x + px * s, y + py * s]
        };
        for path in paths {
            for p in path.windows(2) {
                self.card_line(bg, transform(p[0]), transform(p[1]), 1.35 * s, c);
            }
        }
    }
    fn build_workspace_card(
        &mut self,
        bg: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        card: &WorkspaceCard,
        theme: &Theme,
        top: f32,
    ) {
        let Some(l) = CardLayout::for_card(
            self.config.width as f32,
            self.config.height as f32,
            top,
            self.scale_factor,
            card,
        ) else {
            return;
        };
        let s = l.scale;
        let base = theme.background.to_f32();
        let fg = theme.foreground.to_f32();
        let dark = base[0] + base[1] + base[2] < 1.5;
        let surface = if dark {
            [
                base[0] * 0.78 + 0.018,
                base[1] * 0.78 + 0.018,
                base[2] * 0.78 + 0.018,
                1.,
            ]
        } else {
            [base[0] * 0.94, base[1] * 0.94, base[2] * 0.94, 1.]
        };
        let text = if dark {
            [0.82, 0.83, 0.84, 1.]
        } else {
            [0.16, 0.17, 0.19, 1.]
        };
        let muted = if dark {
            [0.57, 0.59, 0.61, 1.]
        } else {
            [0.40, 0.42, 0.44, 1.]
        };
        self.card_round(
            bg,
            [l.x, l.y + 3. * s, l.w, l.h],
            19. * s,
            [0., 0., 0., 0.12],
        );
        self.card_round(
            bg,
            [l.x, l.y, l.w, l.h],
            18. * s,
            [fg[0], fg[1], fg[2], 0.13],
        );
        self.card_round(
            bg,
            [l.x + s, l.y + s, l.w - 2. * s, l.h - 2. * s],
            17. * s,
            surface,
        );
        let header_shift = if card.minimized { -10.0 } else { 0.0 };
        self.card_round(
            bg,
            [
                l.x + 22. * s,
                l.y + (24. + header_shift) * s,
                6. * s,
                6. * s,
            ],
            3. * s,
            card.tone.color(dark),
        );
        self.card_text(
            glyphs,
            &card.title,
            l.x + 36. * s,
            l.y + (17. + header_shift) * s,
            12. * s,
            muted,
        );
        // Collapsed header keeps the colored status dot; reserve room for controls.
        if !card.minimized {
            self.card_text(
                glyphs,
                &card.subtitle,
                l.x + 22. * s,
                l.y + 48. * s,
                11. * s,
                muted,
            );
            let width = Self::approx_text_width(&card.status, 11. * s);
            self.card_text(
                glyphs,
                &card.status,
                l.x + l.w - 22. * s - width,
                l.y + 48. * s,
                11. * s,
                card.tone.color(dark),
            );
        }
        for (id, icon, dx) in [
            (
                103,
                if card.floating {
                    CardIcon::Unpin
                } else {
                    CardIcon::Pin
                },
                167.,
            ),
            (
                102,
                if card.minimized {
                    CardIcon::Expand
                } else {
                    CardIcon::Minimize
                },
                201.,
            ),
            (100, CardIcon::Refresh, 235.),
            (101, CardIcon::Close, 270.),
        ] {
            let pinned = id == 103 && !card.floating;
            if card.hover == Some(id) || pinned {
                self.card_round(
                    bg,
                    [
                        l.x + (dx - 3.) * s,
                        l.y + (14. + header_shift) * s,
                        22. * s,
                        22. * s,
                    ],
                    7. * s,
                    if pinned {
                        [0.48, 0.69, 0.92, 0.16]
                    } else {
                        [fg[0], fg[1], fg[2], 0.08]
                    },
                );
            }
            self.card_icon(
                bg,
                icon,
                l.x + (dx + 2.4) * s,
                l.y + (19.4 + header_shift) * s,
                0.7 * s,
                if pinned {
                    CardTone::Blue.color(dark)
                } else {
                    muted
                },
            );
        }
        if card.minimized {
            return;
        }
        for (i, row) in card.rows.iter().skip(card.scroll).take(l.count).enumerate() {
            let y = l.y + (82. + i as f32 * 40.) * s;
            let selected = row.action.is_some() && row.action == card.hover;
            if row.section {
                self.draw_rect(
                    bg,
                    l.x + 22. * s,
                    y,
                    l.w - 44. * s,
                    s,
                    [fg[0], fg[1], fg[2], 0.08],
                );
            }
            if selected {
                self.card_round(
                    bg,
                    [l.x + 10. * s, y + 3. * s, l.w - 20. * s, 35. * s],
                    7. * s,
                    [fg[0], fg[1], fg[2], if card.focused { 0.13 } else { 0.07 }],
                );
            }
            let ink = if row.section { muted } else { text };
            self.card_icon(
                bg,
                row.icon,
                l.x + 22. * s,
                y + 12. * s,
                s,
                row.tone.color(dark),
            );
            self.card_text(glyphs, &row.label, l.x + 50. * s, y + 10. * s, 12. * s, ink);
            if let Some((added, removed)) = row.diff {
                let compact = |n: u64| {
                    if n > 9999 {
                        "9999+".to_string()
                    } else {
                        n.to_string()
                    }
                };
                let plus = format!("+{}", compact(added));
                let minus = format!("-{}", compact(removed));
                let minus_w = Self::approx_text_width(&minus, 11. * s);
                let plus_w = Self::approx_text_width(&plus, 11. * s);
                let right = l.x + l.w - 38. * s;
                self.card_text(
                    glyphs,
                    &plus,
                    right - minus_w - 10. * s - plus_w,
                    y + 11. * s,
                    11. * s,
                    CardTone::Green.color(dark),
                );
                self.card_text(
                    glyphs,
                    &minus,
                    right - minus_w,
                    y + 11. * s,
                    11. * s,
                    CardTone::Red.color(dark),
                );
            } else if !row.detail.is_empty() {
                let width = Self::approx_text_width(&row.detail, 11. * s);
                self.card_text(
                    glyphs,
                    &row.detail,
                    l.x + l.w - 38. * s - width,
                    y + 11. * s,
                    11. * s,
                    row.tone.color(dark),
                );
            }
            if row.action.is_some() {
                let x = l.x + l.w - 25. * s;
                let points = if row.expanded {
                    [
                        [x - 3. * s, y + 17. * s],
                        [x, y + 20. * s],
                        [x + 3. * s, y + 17. * s],
                    ]
                } else {
                    [
                        [x - 2. * s, y + 14. * s],
                        [x + 1. * s, y + 17. * s],
                        [x - 2. * s, y + 20. * s],
                    ]
                };
                self.card_line(bg, points[0], points[1], s, muted);
                self.card_line(bg, points[1], points[2], s, muted);
            }
        }
        let footer = if card.hover == Some(103) {
            if card.floating {
                "Pin / dock beside terminal"
            } else {
                "Unpin / float over terminal"
            }
        } else if card.hover == Some(102) {
            "Collapse inspector"
        } else if card.hover == Some(100) {
            "Refresh workspace and model status"
        } else if card.hover == Some(101) {
            "Close inspector / restore terminal"
        } else if card.hover == Some(0) && card.rows.iter().any(|r| r.diff.is_some()) {
            "Tracked lines / untracked excluded"
        } else if card.rows.len() > l.count {
            "Scroll for more  /  Esc to return"
        } else {
            "Workspace  /  Cmd Shift A"
        };
        self.card_text(
            glyphs,
            footer,
            l.x + 22. * s,
            l.y + l.h - 19. * s,
            9. * s,
            muted,
        );
    }
}
