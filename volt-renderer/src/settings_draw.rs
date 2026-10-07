use super::{search_palette_draw::Colors, BgVertex, GlyphVertex, Renderer, Theme};
use crate::settings::{SettingsLayout, SettingsView};
use crate::workspace_card::CardIcon;
pub(super) struct SettingsGeometry {
    view: SettingsView,
    layout: SettingsLayout,
    theme: Theme,
    bg: Vec<BgVertex>,
    glyphs: Vec<GlyphVertex>,
}
fn fit(s: &str, w: f32, size: f32) -> String {
    let n = (w / (size * 0.6)).max(1.) as usize;
    if s.chars().count() <= n {
        s.into()
    } else {
        format!(
            "{}…",
            s.chars().take(n.saturating_sub(1)).collect::<String>()
        )
    }
}
impl Renderer {
    pub fn installed_font_families(&self) -> Vec<String> {
        let mut names = vec!["monospace".to_string()];
        for f in self.font_system.db().faces() {
            if !f.monospaced {
                continue;
            }
            for (n, _) in &f.families {
                if !n.chars().any(char::is_control) {
                    names.push(n.clone());
                }
            }
        }
        names.sort();
        names.dedup();
        names
    }
    pub fn settings_layout(&self) -> Option<SettingsLayout> {
        self.settings.as_ref()?;
        SettingsLayout::new(
            self.config.width as f32,
            self.config.height as f32,
            self.workspace_card_top_offset(),
            self.scale_factor,
        )
    }
    pub(super) fn draw_settings(
        &mut self,
        bg: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        view: &SettingsView,
        theme: &Theme,
    ) {
        let Some(l) = self.settings_layout() else {
            return;
        };
        if !self
            .settings_cache
            .as_ref()
            .is_some_and(|c| c.view == *view && c.layout == l && c.theme == *theme)
        {
            let mut b = vec![];
            let mut g = vec![];
            self.build_settings(&mut b, &mut g, view, &l, theme);
            self.settings_cache = Some(SettingsGeometry {
                view: view.clone(),
                layout: l,
                theme: theme.clone(),
                bg: b,
                glyphs: g,
            });
        }
        if let Some(c) = &self.settings_cache {
            bg.extend_from_slice(&c.bg);
            glyphs.extend_from_slice(&c.glyphs);
        }
    }
    fn build_settings(
        &mut self,
        b: &mut Vec<BgVertex>,
        g: &mut Vec<GlyphVertex>,
        v: &SettingsView,
        l: &SettingsLayout,
        t: &Theme,
    ) {
        let c = Colors::from_theme(t);
        let s = l.scale;
        self.draw_rect(
            b,
            0.,
            self.workspace_card_top_offset(),
            self.config.width as f32,
            self.config.height as f32,
            [0., 0., 0., 0.38],
        );
        self.card_round(b, [l.x + 2. * s, l.y + 7. * s, l.w, l.h], 15. * s, c.shadow);
        self.card_round(b, [l.x, l.y, l.w, l.h], 14. * s, c.border);
        self.card_round(
            b,
            [l.x + s, l.y + s, l.w - 2. * s, l.h - 2. * s],
            13. * s,
            c.panel,
        );
        self.card_text(g, &v.title, l.x + 22. * s, l.y + 18. * s, 19. * s, c.text);
        self.card_text(
            g,
            &fit(&v.subtitle, l.w - 44. * s, 11. * s),
            l.x + 22. * s,
            l.y + 47. * s,
            11. * s,
            c.muted,
        );
        for (i, name) in v.sections.iter().enumerate() {
            let r = l.section(i);
            if r[1] + r[3] > l.config()[1] - 8. * s {
                break;
            }
            if i == v.section {
                self.card_round(b, r, 7. * s, c.selected);
                self.draw_rect(b, r[0], r[1] + 7. * s, 3. * s, r[3] - 14. * s, c.accent);
            }
            self.card_text(
                g,
                name,
                r[0] + 10. * s,
                r[1] + 6. * s,
                12. * s,
                if i == v.section { c.accent } else { c.muted },
            );
        }
        for (i, row) in v.rows.iter().take(l.rows).enumerate() {
            let r = l.row(i);
            if i == v.focus {
                self.card_round(b, r, 7. * s, c.selected);
            }
            let room = r[2] - 64. * s;
            self.card_text(
                g,
                &fit(&row.label, room, 12. * s),
                r[0] + 10. * s,
                r[1] + 3. * s,
                12. * s,
                c.muted,
            );
            let mut value = row.value.clone();
            if let Some(cursor) = row.caret {
                // Keep the caret visible while editing long paths or arguments.
                let capacity = (room / (13. * s * 1.3)).floor().max(4.) as usize;
                let start = cursor.saturating_sub(capacity / 2);
                let mut chars: Vec<char> = value.chars().skip(start).take(capacity).collect();
                chars.insert((cursor - start).min(chars.len()), '│');
                value = chars.into_iter().collect();
                if start > 0 {
                    value.insert(0, '…');
                }
            }
            if row.selected {
                self.card_round(
                    b,
                    [r[0] + 8. * s, r[1] + 21. * s, room - 4. * s, 22. * s],
                    3. * s,
                    c.selected,
                );
            }
            self.card_text(
                g,
                &fit(&value, room, 13. * s),
                r[0] + 10. * s,
                r[1] + 23. * s,
                13. * s,
                if i == v.focus { c.accent } else { c.text },
            );
            let buttons: &[(f32, &str)] = if row.adjustable {
                &[(49., "−"), (24., "+")]
            } else if row.actionable {
                &[(24., "›")]
            } else {
                &[]
            };
            for &(dx, label) in buttons {
                self.card_text(
                    g,
                    label,
                    r[0] + r[2] - dx * s,
                    r[1] + 20. * s,
                    14. * s,
                    c.quiet,
                );
            }
        }
        let bottom = l.y + l.h - 90. * s;
        let hint = v.rows.get(v.focus).map(|r| r.hint.as_str()).unwrap_or("");
        self.card_text(
            g,
            &fit(
                if v.status.is_empty() { hint } else { &v.status },
                l.w - 38. * s,
                11. * s,
            ),
            l.x + 18. * s,
            bottom,
            11. * s,
            if v.status.is_empty() {
                c.quiet
            } else {
                c.accent
            },
        );
        if v.preview {
            self.card_text(
                g,
                "❯ project  main ✓  echo 'Hello 🌍'",
                l.x + 155. * s,
                bottom - 24. * s,
                12. * s,
                t.foreground.to_f32(),
            );
        }
        for (i, label) in [
            (
                0,
                if v.onboarding {
                    if v.section + 1 == v.sections.len() {
                        "Finish"
                    } else {
                        "Next"
                    }
                } else {
                    "Apply"
                },
            ),
            (1, if v.onboarding { "Skip" } else { "Cancel" }),
            (2, if v.onboarding { "Back" } else { "Reset page" }),
        ] {
            let r = l.footer(i);
            self.card_round(b, r, 7. * s, if i == 0 { c.accent } else { c.selected });
            let color = if i == 0 { c.panel } else { c.text };
            let icon = match (i, v.onboarding, label) {
                (0, true, "Next") => Some(CardIcon::ArrowRight),
                (0, _, _) => Some(CardIcon::Check),
                (1, _, _) => Some(CardIcon::Close),
                (2, true, _) => Some(CardIcon::ArrowLeft),
                _ => None,
            };
            let size = 12. * s;
            let icon_room = if icon.is_some() { 22. * s } else { 0. };
            let width = icon_room + Self::approx_text_width(label, size);
            let x = r[0] + (r[2] - width) * 0.5;
            if let Some(icon) = icon {
                self.card_icon(b, icon, x, r[1] + (r[3] - 16. * s) * 0.5, s, color);
            }
            self.card_text(
                g,
                label,
                x + icon_room,
                r[1] + (r[3] - size * 1.2) * 0.5,
                size,
                color,
            );
        }
        let r = l.config();
        self.card_text(
            g,
            "Open config ↗",
            r[0] + 4. * s,
            r[1] + 8. * s,
            11. * s,
            c.muted,
        );
    }
}
