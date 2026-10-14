use super::{BgVertex, GlyphVertex, Renderer, Theme};
use crate::workspace_card::{BlockKind, CardIcon, CardLayout, CardRow, CardTone, WorkspaceCard};
use std::sync::Arc;
use volt_ui_kit::Painter;

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
pub(super) struct LayoutCache {
    rows: Vec<CardRow>,
    size: (u32, u32),
    scale: f32,
    top: f32,
    floating: bool,
    minimized: bool,
    position: Option<[f32; 2]>,
    scroll: usize,
    layout: Arc<CardLayout>,
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
        if !self.workspace_card_cache.as_ref().is_some_and(|c| {
            c.card == *card
                && c.size == size
                && c.scale == self.scale_factor
                && c.top == top
                && c.colors == colors
                && c.font == self.font_family
        }) {
            let mut b = vec![];
            let mut g = vec![];
            if let Some(l) = self.card_layout(card, top) {
                self.paint_workspace(&mut b, &mut g, card, &super::ui_kit_theme(theme), &l);
            }
            self.workspace_card_cache = Some(CardGeometry {
                card: card.clone(),
                size,
                scale: self.scale_factor,
                top,
                colors,
                font: self.font_family.clone(),
                bg: b,
                glyphs: g,
            });
        }
        if let Some(c) = &self.workspace_card_cache {
            bg.extend_from_slice(&c.bg);
            glyphs.extend_from_slice(&c.glyphs);
        }
    }
    fn paint_workspace(
        &mut self,
        bg: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        card: &WorkspaceCard,
        theme: &volt_ui_kit::Theme,
        l: &CardLayout,
    ) {
        let s = l.scale;
        let base = theme.background;
        let fg = theme.foreground;
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
        volt_ui_kit::Painter::card_round(
            self,
            bg,
            [l.x, l.y + 3. * s, l.w, l.h],
            19. * s,
            [0., 0., 0., 0.12],
        );
        volt_ui_kit::Painter::card_round(
            self,
            bg,
            [l.x, l.y, l.w, l.h],
            18. * s,
            [fg[0], fg[1], fg[2], 0.13],
        );
        volt_ui_kit::Painter::card_round(
            self,
            bg,
            [l.x + s, l.y + s, l.w - 2. * s, l.h - 2. * s],
            17. * s,
            surface,
        );
        let header_shift = if card.minimized { -10.0 } else { 0.0 };
        volt_ui_kit::Painter::card_round(
            self,
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
        volt_ui_kit::Painter::card_text(
            self,
            glyphs,
            &card.title,
            l.x + 36. * s,
            l.y + (17. + header_shift) * s,
            12. * s,
            muted,
        );
        // Collapsed header keeps the colored status dot; reserve room for controls.
        if !card.minimized {
            let status_width = Self::approx_text_width(&card.status, 11. * s);
            let budget = ((l.w - 44. * s - status_width - 12. * s) / (11. * s * 0.72))
                .floor()
                .max(1.) as usize;
            let subtitle = crate::prompt_viewport::window(&card.subtitle, 0, budget).0;
            volt_ui_kit::Painter::card_text(
                self,
                glyphs,
                &subtitle,
                l.x + 22. * s,
                l.y + 48. * s,
                11. * s,
                muted,
            );
            let width = <Self as volt_ui_kit::Painter>::approx_text_width(&card.status, 11. * s);
            volt_ui_kit::Painter::card_text(
                self,
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
                volt_ui_kit::Painter::card_round(
                    self,
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
            volt_ui_kit::Painter::card_icon(
                self,
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

        let mut body_bg = vec![];
        let mut body_glyphs = vec![];
        for b in &l.blocks {
            let r = l.block_rect(b);
            if r[1] + r[3] <= l.body[1] || r[1] >= l.body[1] + l.body[3] {
                continue;
            }
            let row = &card.rows[b.index];
            let selected = row.action.is_some() && row.action == card.hover;
            if row.section {
                self.draw_rect(
                    &mut body_bg,
                    l.x + 22. * s,
                    r[1],
                    l.w - 44. * s,
                    s,
                    [fg[0], fg[1], fg[2], 0.08],
                );
            }
            if selected || matches!(row.kind, BlockKind::Notice | BlockKind::Code) {
                self.card_round(
                    &mut body_bg,
                    [r[0], r[1] + 2. * s, r[2], r[3] - 4. * s],
                    7. * s,
                    if row.kind == BlockKind::Notice {
                        let c = row.tone.color(dark);
                        [c[0], c[1], c[2], 0.08]
                    } else {
                        [
                            fg[0],
                            fg[1],
                            fg[2],
                            if selected && card.focused {
                                0.13
                            } else {
                                0.055
                            },
                        ]
                    },
                );
            }
            if row.has_icon() {
                self.card_icon(
                    &mut body_bg,
                    row.icon,
                    l.x + 22. * s,
                    r[1] + 10. * s,
                    s,
                    row.tone.color(dark),
                );
            }
            let ink = if row.kind == BlockKind::Notice {
                row.tone.color(dark)
            } else if row.section || row.kind == BlockKind::Paragraph {
                muted
            } else {
                text
            };
            for (i, line) in b.label.iter().enumerate() {
                let y = r[1] + (7. + i as f32 * b.line_height) * s;
                if y + b.line_height * s > l.body[1] && y < l.body[1] + l.body[3] {
                    self.card_text(
                        &mut body_glyphs,
                        line,
                        l.x + b.text_left * s,
                        y,
                        b.font_size * s,
                        ink,
                    );
                }
            }
            if b.detail_below {
                for (i, line) in b.detail.iter().enumerate() {
                    let y = r[1] + (7. + b.label.len() as f32 * b.line_height + i as f32 * 16.) * s;
                    if y + 16. * s > l.body[1] && y < l.body[1] + l.body[3] {
                        self.card_text(
                            &mut body_glyphs,
                            line,
                            l.x + b.text_left * s,
                            y,
                            11. * s,
                            row.tone.color(dark),
                        );
                    }
                }
            } else if let Some((added, removed)) = row.diff {
                let compact = |n: u64| {
                    if n > 9999 {
                        "9999+".to_string()
                    } else {
                        n.to_string()
                    }
                };
                let plus = format!("+{}", compact(added));
                let minus = format!("−{}", compact(removed));
                let mw = Self::approx_text_width(&minus, 11. * s);
                let pw = Self::approx_text_width(&plus, 11. * s);
                let right = l.x + l.w - 38. * s;
                self.card_text(
                    &mut body_glyphs,
                    &plus,
                    right - mw - 10. * s - pw,
                    r[1] + 10. * s,
                    11. * s,
                    CardTone::Green.color(dark),
                );
                self.card_text(
                    &mut body_glyphs,
                    &minus,
                    right - mw,
                    r[1] + 10. * s,
                    11. * s,
                    CardTone::Red.color(dark),
                );
            } else if !row.detail.is_empty() {
                let width = Self::approx_text_width(&row.detail, 11. * s);
                self.card_text(
                    &mut body_glyphs,
                    &row.detail,
                    l.x + l.w - 38. * s - width,
                    r[1] + 10. * s,
                    11. * s,
                    row.tone.color(dark),
                );
            }
            if row.action.is_some() {
                let x = l.x + l.w - 25. * s;
                let y = r[1] + 18. * s;
                let points = if row.expanded {
                    [
                        [x - 3. * s, y - 1. * s],
                        [x, y + 2. * s],
                        [x + 3. * s, y - 1. * s],
                    ]
                } else {
                    [
                        [x - 2. * s, y - 4. * s],
                        [x + 1. * s, y - 1. * s],
                        [x - 2. * s, y + 2. * s],
                    ]
                };
                self.card_line(&mut body_bg, points[0], points[1], s, muted);
                self.card_line(&mut body_bg, points[1], points[2], s, muted);
            }
        }
        let bounds = [
            l.body[0] / self.config.width as f32 * 2. - 1.,
            1. - (l.body[1] + l.body[3]) / self.config.height as f32 * 2.,
            (l.body[0] + l.body[2]) / self.config.width as f32 * 2. - 1.,
            1. - l.body[1] / self.config.height as f32 * 2.,
        ];
        clip_triangles(&body_bg, bg, bounds);
        clip_triangles(&body_glyphs, glyphs, bounds);
        if l.max_scroll > 0 {
            let track = [
                l.x + l.w - 7. * s,
                l.body[1] + 2. * s,
                2. * s,
                l.body[3] - 4. * s,
            ];
            self.card_round(bg, track, s, [fg[0], fg[1], fg[2], 0.08]);
            let total = l.body[3] / s + l.max_scroll as f32;
            let h = (track[3] * (l.body[3] / s) / total)
                .max(14. * s)
                .min(track[3]);
            let y = track[1] + (track[3] - h) * l.scroll as f32 / l.max_scroll as f32;
            self.card_round(bg, [track[0], y, track[2], h], s, muted);
        }
        let footer = match card.hover {
            Some(103) => {
                if card.floating {
                    "Pin / dock beside terminal"
                } else {
                    "Unpin / float over terminal"
                }
            }
            Some(102) => "Collapse workspace",
            Some(100) => "Refresh workspace",
            Some(101) => "Close workspace",
            _ if l.max_scroll > 0 => "Scroll for more · Esc returns",
            _ => "Workspace · Cmd Shift A",
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
    fn card_layout(&self, card: &WorkspaceCard, top: f32) -> Option<Arc<CardLayout>> {
        let size = (self.config.width, self.config.height);
        let mut cache = self.workspace_layout_cache.borrow_mut();
        if let Some(c) = cache.as_ref().filter(|c| {
            c.rows == card.rows
                && c.size == size
                && c.scale == self.scale_factor
                && c.top == top
                && c.floating == card.floating
                && c.minimized == card.minimized
                && c.position == card.position
                && c.scroll == card.scroll
        }) {
            return Some(Arc::clone(&c.layout));
        }
        let layout = Arc::new(CardLayout::for_card(
            size.0 as f32,
            size.1 as f32,
            top,
            self.scale_factor,
            card,
        )?);
        *cache = Some(LayoutCache {
            rows: card.rows.clone(),
            size,
            scale: self.scale_factor,
            top,
            floating: card.floating,
            minimized: card.minimized,
            position: card.position,
            scroll: card.scroll,
            layout: Arc::clone(&layout),
        });
        Some(layout)
    }
    pub(super) fn card_text(
        &mut self,
        g: &mut Vec<GlyphVertex>,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        color: [f32; 4],
    ) {
        self.draw_text_with_line_height(g, text, x, y, size, 1.2, color);
    }
    pub fn workspace_card_layout(&self) -> Option<Arc<CardLayout>> {
        self.card_layout(
            self.workspace_card.as_ref()?,
            self.workspace_card_top_offset(),
        )
    }
    pub fn workspace_card_reserved_width(&self) -> f32 {
        if self.workspace_card.as_ref().is_some_and(|c| !c.floating)
            && self.workspace_card_layout().is_some()
        {
            340. * self.scale_factor
        } else {
            0.
        }
    }
    pub fn workspace_card_top_offset(&self) -> f32 {
        self.top_offset_for_tab_count(1) + self.alert_bar_height()
    }
    pub(super) fn card_round(&self, b: &mut Vec<BgVertex>, r: [f32; 4], radius: f32, c: [f32; 4]) {
        Painter::card_round(self, b, r, radius, c);
    }
}

// Clip only dirty body geometry, including glyph UVs, to the shared content
// rectangle. Header/footer never become clickable or covered by scrolled text.
trait ClipVertex: Copy {
    fn pos(self) -> [f32; 2];
    fn interpolate(self, other: Self, t: f32) -> Self;
}
fn mix<const N: usize>(a: [f32; N], b: [f32; N], t: f32) -> [f32; N] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}
impl ClipVertex for BgVertex {
    fn pos(self) -> [f32; 2] {
        self.pos
    }
    fn interpolate(self, o: Self, t: f32) -> Self {
        Self {
            pos: mix(self.pos, o.pos, t),
            color: mix(self.color, o.color, t),
        }
    }
}
impl ClipVertex for GlyphVertex {
    fn pos(self) -> [f32; 2] {
        self.pos
    }
    fn interpolate(self, o: Self, t: f32) -> Self {
        Self {
            pos: mix(self.pos, o.pos, t),
            uv: mix(self.uv, o.uv, t),
            color: mix(self.color, o.color, t),
        }
    }
}
fn clip_triangles<V: ClipVertex>(input: &[V], output: &mut Vec<V>, bounds: [f32; 4]) {
    for triangle in input.as_chunks::<3>().0 {
        let mut poly = triangle.to_vec();
        for (axis, edge, lower) in [
            (0, bounds[0], true),
            (0, bounds[2], false),
            (1, bounds[1], true),
            (1, bounds[3], false),
        ] {
            let old = std::mem::take(&mut poly);
            if old.is_empty() {
                break;
            }
            let mut prev = *old.last().unwrap();
            let inside = |v: V| {
                if lower {
                    v.pos()[axis] >= edge
                } else {
                    v.pos()[axis] <= edge
                }
            };
            for cur in old {
                if inside(prev) != inside(cur) {
                    let t = (edge - prev.pos()[axis]) / (cur.pos()[axis] - prev.pos()[axis]);
                    poly.push(prev.interpolate(cur, t));
                }
                if inside(cur) {
                    poly.push(cur);
                }
                prev = cur;
            }
        }
        for i in 1..poly.len().saturating_sub(1) {
            output.extend_from_slice(&[poly[0], poly[i], poly[i + 1]]);
        }
    }
}
