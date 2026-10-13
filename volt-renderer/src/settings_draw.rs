use super::{BgVertex, GlyphVertex, Renderer, Theme};
use crate::settings::{
    control_rect, dropdown_rect, slider_track, SettingsControl, SettingsDropdown, SettingsLayout,
    SettingsView, DROPDOWN_ROWS,
};
use volt_ui_kit::{
    button::{Button, ButtonColors},
    style::Colors,
    CardIcon, Painter,
};
pub(super) struct SettingsGeometry {
    view: SettingsView,
    layout: SettingsLayout,
    theme: Theme,
    toggles: Vec<Option<bool>>,
    controls: Vec<SettingsControl>,
    dropdown: Option<SettingsDropdown>,
    bg: Vec<BgVertex>,
    glyphs: Vec<GlyphVertex>,
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
        if !self.settings_cache.as_ref().is_some_and(|c| {
            c.view == *view
                && c.layout == l
                && c.theme == *theme
                && c.toggles == self.settings_toggles
                && c.controls == self.settings_controls
                && c.dropdown == self.settings_dropdown
        }) {
            let mut b = vec![];
            let mut g = vec![];
            let toggles = self.settings_toggles.clone();
            let controls = self.settings_controls.clone();
            let dropdown = self.settings_dropdown.clone();
            let mut presentation = view.clone();
            for (i, row) in presentation.rows.iter_mut().enumerate() {
                let typed = controls
                    .get(i)
                    .is_some_and(|c| !matches!(c, SettingsControl::Info));
                let toggle = toggles.get(i).is_some_and(Option::is_some);
                if typed || toggle {
                    row.label.clear();
                    row.value.clear();
                    row.adjustable = false;
                    row.actionable = false;
                    row.caret = None;
                    row.selected = false;
                }
            }
            volt_ui_kit::settings_paint::SettingsPaint::build_settings(
                self,
                &mut b,
                &mut g,
                &presentation,
                &l,
                &super::ui_kit_theme(theme),
            );
            let colors = Colors::from_theme(&super::ui_kit_theme(theme));
            for (i, row) in view.rows.iter().take(l.rows).enumerate() {
                let fallback = toggles
                    .get(i)
                    .copied()
                    .flatten()
                    .map(SettingsControl::Switch);
                let Some(control) = controls.get(i).or(fallback.as_ref()) else {
                    continue;
                };
                if matches!(control, SettingsControl::Info) {
                    continue;
                }
                let r = l.row(i);
                let field = control_rect(&l, i);
                let room = if matches!(control, SettingsControl::Switch(_)) {
                    r[2] - 110. * l.scale
                } else {
                    field[0] - r[0] - 24. * l.scale
                };
                let label = fit(&row.label, room, 12. * l.scale);
                self.card_text(
                    &mut g,
                    &label,
                    r[0] + 10. * l.scale,
                    r[1] + (r[3] - 14.4 * l.scale) * 0.5,
                    12. * l.scale,
                    colors.muted,
                );
                if matches!(control, SettingsControl::Choice) && !row.editing {
                    let value = fit(&row.value, field[2] - 40. * l.scale, 12. * l.scale);
                    if self.draw_toolkit_select(
                        &mut b,
                        &mut g,
                        field,
                        l.scale,
                        &value,
                        (i == view.focus, theme),
                    ) {
                        continue;
                    }
                }
                match control {
                    SettingsControl::Switch(checked) => {
                        let x = r[0] + r[2] - 54. * l.scale;
                        self.card_text(
                            &mut g,
                            if *checked { "On" } else { "Off" },
                            x - 38. * l.scale,
                            r[1] + (r[3] - 15.6 * l.scale) * 0.5,
                            13. * l.scale,
                            colors.text,
                        );
                        self.draw_toolkit_toggle(
                            &mut b,
                            &mut g,
                            [x, r[1] + (r[3] - 22. * l.scale) * 0.5],
                            l.scale,
                            *checked,
                            theme,
                        );
                    }
                    SettingsControl::Slider { value, min, max } if !row.editing => {
                        let t = slider_track(&l, i);
                        let fraction = ((*value - min) / (max - min)).clamp(0., 1.);
                        let widget = [field[0], field[1], field[2] - 49. * l.scale, field[3]];
                        if !self.draw_toolkit_slider(
                            &mut b,
                            &mut g,
                            widget,
                            l.scale,
                            [*value, *min, *max],
                            theme,
                        ) {
                            self.card_round(&mut b, t, 2. * l.scale, colors.border);
                            self.card_round(
                                &mut b,
                                [t[0], t[1], t[2] * fraction, t[3]],
                                2. * l.scale,
                                colors.accent,
                            );
                            self.card_round(
                                &mut b,
                                [
                                    t[0] + t[2] * fraction - 7. * l.scale,
                                    t[1] - 5. * l.scale,
                                    14. * l.scale,
                                    14. * l.scale,
                                ],
                                7. * l.scale,
                                colors.text,
                            );
                        }
                        self.card_text(
                            &mut g,
                            &format!("{:.0}%", value * 100.),
                            field[0] + field[2] - 47. * l.scale,
                            field[1] + 8. * l.scale,
                            12. * l.scale,
                            colors.text,
                        );
                    }
                    SettingsControl::Action(label) => {
                        Button {
                            label: &fit(label, field[2] - 36. * l.scale, 12. * l.scale),
                            icon: Some(CardIcon::ArrowRight),
                            enabled: true,
                        }
                        .paint(
                            self,
                            &mut b,
                            &mut g,
                            field,
                            l.scale,
                            ButtonColors {
                                surface: colors.selected,
                                text: colors.text,
                            },
                        );
                    }
                    _ => {
                        self.card_round(
                            &mut b,
                            field,
                            6. * l.scale,
                            if i == view.focus {
                                colors.accent
                            } else {
                                colors.border
                            },
                        );
                        let inner = [
                            field[0] + l.scale,
                            field[1] + l.scale,
                            field[2] - 2. * l.scale,
                            field[3] - 2. * l.scale,
                        ];
                        self.card_round(&mut b, inner, 5. * l.scale, colors.panel);
                        let icon_room = if matches!(
                            control,
                            SettingsControl::Choice | SettingsControl::Shortcut
                        ) {
                            26. * l.scale
                        } else {
                            0.
                        };
                        let room = field[2] - 20. * l.scale - icon_room;
                        let mut value = row.value.clone();
                        if matches!(control, SettingsControl::Shortcut) && row.editing {
                            value = "Press shortcut…".into();
                        } else if let Some(cursor) = row.caret {
                            let capacity = (room / (12. * l.scale * 1.3)).floor().max(4.) as usize;
                            let start = cursor.saturating_sub(capacity / 2);
                            let mut chars: Vec<char> =
                                value.chars().skip(start).take(capacity).collect();
                            chars.insert((cursor - start).min(chars.len()), '│');
                            value = chars.into_iter().collect();
                        }
                        if row.selected && !matches!(control, SettingsControl::Shortcut) {
                            self.card_round(
                                &mut b,
                                [
                                    field[0] + 6. * l.scale,
                                    field[1] + 5. * l.scale,
                                    room,
                                    22. * l.scale,
                                ],
                                3. * l.scale,
                                colors.selected,
                            );
                        }
                        self.card_text(
                            &mut g,
                            &fit(&value, room, 12. * l.scale),
                            field[0] + 10. * l.scale,
                            field[1] + 8. * l.scale,
                            12. * l.scale,
                            colors.text,
                        );
                        if matches!(control, SettingsControl::Choice) {
                            let x = field[0] + field[2] - 20. * l.scale;
                            let y = field[1] + 14. * l.scale;
                            self.card_triangle(
                                &mut b,
                                [
                                    [x, y],
                                    [x + 8. * l.scale, y],
                                    [x + 4. * l.scale, y + 5. * l.scale],
                                ],
                                colors.muted,
                            );
                        } else if matches!(control, SettingsControl::Shortcut) {
                            self.card_icon(
                                &mut b,
                                CardIcon::Edit,
                                field[0] + field[2] - 23. * l.scale,
                                field[1] + 8. * l.scale,
                                l.scale,
                                colors.muted,
                            );
                        }
                    }
                }
            }
            if let Some(d) = &dropdown {
                let r = dropdown_rect(&l, d);
                // UI buffers draw all shapes before text. Remove obscured glyph
                // quads before adding the popup text, so underlying rows cannot bleed.
                let mut visible = Vec::with_capacity(g.len());
                for quad in g.as_chunks::<6>().0.iter() {
                    let mut bounds = [
                        f32::INFINITY,
                        f32::INFINITY,
                        f32::NEG_INFINITY,
                        f32::NEG_INFINITY,
                    ];
                    for v in quad {
                        let x = (v.pos[0] + 1.) * self.config.width as f32 * 0.5;
                        let y = (1. - v.pos[1]) * self.config.height as f32 * 0.5;
                        bounds[0] = bounds[0].min(x);
                        bounds[1] = bounds[1].min(y);
                        bounds[2] = bounds[2].max(x);
                        bounds[3] = bounds[3].max(y);
                    }
                    if bounds[2] <= r[0]
                        || bounds[0] >= r[0] + r[2]
                        || bounds[3] <= r[1]
                        || bounds[1] >= r[1] + r[3]
                    {
                        visible.extend_from_slice(quad);
                    }
                }
                g = visible;
                self.card_round(&mut b, r, 6. * l.scale, colors.border);
                self.card_round(
                    &mut b,
                    [
                        r[0] + l.scale,
                        r[1] + l.scale,
                        r[2] - 2. * l.scale,
                        r[3] - 2. * l.scale,
                    ],
                    5. * l.scale,
                    colors.panel,
                );
                for (slot, label) in d
                    .items
                    .iter()
                    .skip(d.offset)
                    .take(DROPDOWN_ROWS)
                    .enumerate()
                {
                    let item = [
                        r[0] + 3. * l.scale,
                        r[1] + (4. + slot as f32 * 28.) * l.scale,
                        r[2] - 6. * l.scale,
                        28. * l.scale,
                    ];
                    if slot + d.offset == d.selected {
                        self.card_round(&mut b, item, 4. * l.scale, colors.selected);
                    }
                    if slot + d.offset == d.current {
                        self.card_icon(
                            &mut b,
                            CardIcon::Check,
                            item[0] + 4. * l.scale,
                            item[1] + 6. * l.scale,
                            l.scale,
                            colors.accent,
                        );
                    }
                    self.card_text(
                        &mut g,
                        &fit(label, item[2] - 33. * l.scale, 12. * l.scale),
                        item[0] + 26. * l.scale,
                        item[1] + 7. * l.scale,
                        12. * l.scale,
                        colors.text,
                    );
                }
            }
            self.settings_cache = Some(SettingsGeometry {
                view: view.clone(),
                layout: l,
                theme: theme.clone(),
                toggles,
                controls,
                dropdown,
                bg: b,
                glyphs: g,
            });
        }
        if let Some(c) = &self.settings_cache {
            bg.extend_from_slice(&c.bg);
            glyphs.extend_from_slice(&c.glyphs);
        }
    }
}

fn fit(text: &str, width: f32, size: f32) -> String {
    let capacity = (width / (size * 0.6)).floor().max(1.) as usize;
    if text.chars().count() <= capacity {
        text.into()
    } else {
        format!(
            "{}…",
            text.chars()
                .take(capacity.saturating_sub(1))
                .collect::<String>()
        )
    }
}
