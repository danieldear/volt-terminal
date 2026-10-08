use crate::button::{Button, ButtonColors};
use crate::{
    paint::Painter,
    settings::*,
    style::{Colors, Theme},
    workspace_card::CardIcon,
};
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

#[allow(clippy::too_many_arguments)]
pub trait SettingsPaint: Painter {
    fn build_settings(
        &mut self,
        b: &mut Vec<Self::Shape>,
        g: &mut Vec<Self::Glyph>,
        v: &SettingsView,
        l: &SettingsLayout,
        t: &Theme,
    ) {
        let c = Colors::from_theme(t);
        let s = l.scale;
        self.draw_rect(
            b,
            0.,
            self.viewport().top,
            self.viewport().width,
            self.viewport().height,
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
                t.foreground,
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
            let color = if i == 0 { c.panel } else { c.text };
            let icon = match (i, v.onboarding, label) {
                (0, true, "Next") => Some(CardIcon::ArrowRight),
                (0, _, _) => Some(CardIcon::Check),
                (1, _, _) => Some(CardIcon::Close),
                (2, true, _) => Some(CardIcon::ArrowLeft),
                _ => None,
            };
            Button {
                label,
                icon,
                enabled: true,
            }
            .paint(
                self,
                b,
                g,
                r,
                s,
                ButtonColors {
                    surface: if i == 0 { c.accent } else { c.selected },
                    text: color,
                },
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
impl<P: Painter> SettingsPaint for P {}
