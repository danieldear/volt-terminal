use crate::{paint::Painter, style::Theme, workspace_card::*};
/// Notices occupy one shared layout row, not one selectable item per line.
/// Wrap at word boundaries and bound long diagnostics to two lines.
fn notice_lines(text: &str) -> Vec<String> {
    use unicode_width::UnicodeWidthStr;
    const WIDTH: usize = 36;
    let mut lines = vec![String::new()];
    for word in text.split_whitespace() {
        let line = lines.last_mut().unwrap();
        let width = line.width() + usize::from(!line.is_empty()) + word.width();
        if width > WIDTH && !line.is_empty() {
            if lines.len() == 2 {
                let last = lines.last_mut().unwrap();
                while last.width() >= WIDTH {
                    last.pop();
                }
                last.push('…');
                break;
            }
            lines.push(String::new());
        }
        let line = lines.last_mut().unwrap();
        if !line.is_empty() {
            line.push(' ');
        }
        if word.width() > WIDTH {
            for c in word.chars() {
                if line.width() + unicode_width::UnicodeWidthChar::width(c).unwrap_or(0) >= WIDTH {
                    break;
                }
                line.push(c);
            }
            line.push('…');
        } else {
            line.push_str(word);
        }
    }
    lines
}

#[allow(clippy::too_many_arguments)]
pub trait CardPaint: Painter {
    fn build_workspace_card(
        &mut self,
        bg: &mut Vec<Self::Shape>,
        glyphs: &mut Vec<Self::Glyph>,
        card: &WorkspaceCard,
        theme: &Theme,
        top: f32,
    ) {
        self.build_workspace_card_with_footer(bg, glyphs, card, theme, top, None);
    }
    fn build_workspace_card_with_footer(
        &mut self,
        bg: &mut Vec<Self::Shape>,
        glyphs: &mut Vec<Self::Glyph>,
        card: &WorkspaceCard,
        theme: &Theme,
        top: f32,
        footer: Option<&str>,
    ) {
        let Some(l) = CardLayout::for_card(
            self.viewport().width,
            self.viewport().height,
            top,
            self.viewport().scale,
            card,
        ) else {
            return;
        };
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
            if row.icon == CardIcon::Notice {
                let color = row.tone.color(dark);
                self.card_round(
                    bg,
                    [l.x + 22. * s, y + 2. * s, l.w - 44. * s, 36. * s],
                    6. * s,
                    [color[0], color[1], color[2], 0.08],
                );
                let lines = notice_lines(&row.label);
                let start = y + (40. - lines.len() as f32 * 12.6) * 0.5 * s;
                for (i, line) in lines.iter().enumerate() {
                    self.card_text(
                        glyphs,
                        line,
                        l.x + 30. * s,
                        start + i as f32 * 12.6 * s,
                        10.5 * s,
                        color,
                    );
                }
                continue;
            }
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
        let footer = if let Some(footer) = footer {
            footer
        } else if card.hover == Some(103) {
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
impl<P: Painter> CardPaint for P {}
#[cfg(test)]
mod notice_tests {
    use super::notice_lines;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn status_wraps_between_words_not_inside_command() {
        let lines = notice_lines("Wait for the current command to finish before running a task.");
        assert_eq!(
            lines,
            [
                "Wait for the current command to",
                "finish before running a task."
            ]
        );
        assert_eq!(
            notice_lines("Command running. Wait before starting a task."),
            ["Command running. Wait before", "starting a task."]
        );
    }

    #[test]
    fn long_diagnostics_remain_bounded_and_signal_truncation() {
        for text in [
            "longword".repeat(100),
            "many words ".repeat(100),
            "界".repeat(100),
        ] {
            let lines = notice_lines(&text);
            assert!(lines.len() <= 2);
            assert!(lines.iter().all(|l| l.width() <= 36));
            assert!(lines.last().unwrap().ends_with('…'));
        }
    }
}
