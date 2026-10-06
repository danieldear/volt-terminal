//! Shared inspector geometry: rendering and pointer hit testing use the same rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardIcon {
    Folder,
    Branch,
    File,
    Play,
    Agent,
    Link,
    Server,
    Close,
    Refresh,
    Minimize,
    Pin,
    Unpin,
    Expand,
    Search,
    Plus,
    Edit,
    /// Compact, non-interactive status banner. Deliberately has no item glyph.
    Notice,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CardTone {
    #[default]
    Muted,
    Green,
    Red,
    Amber,
    Blue,
    Purple,
    Cyan,
}
impl CardTone {
    pub fn color(self, dark: bool) -> [f32; 4] {
        let rgb = match (self, dark) {
            (Self::Green, true) => [0.48, 0.79, 0.59],
            (Self::Green, false) => [0.10, 0.43, 0.24],
            (Self::Red, true) => [0.94, 0.47, 0.49],
            (Self::Red, false) => [0.72, 0.19, 0.23],
            (Self::Amber, true) => [0.87, 0.70, 0.40],
            (Self::Amber, false) => [0.52, 0.34, 0.08],
            (Self::Blue, true) => [0.48, 0.69, 0.92],
            (Self::Blue, false) => [0.18, 0.39, 0.67],
            (Self::Purple, true) => [0.73, 0.59, 0.92],
            (Self::Purple, false) => [0.48, 0.30, 0.69],
            (Self::Cyan, true) => [0.43, 0.76, 0.78],
            (Self::Cyan, false) => [0.09, 0.43, 0.46],
            (Self::Muted, true) => [0.57, 0.59, 0.61],
            (Self::Muted, false) => [0.40, 0.42, 0.44],
        };
        [rgb[0], rgb[1], rgb[2], 1.]
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct CardRow {
    pub label: String,
    pub detail: String,
    pub icon: CardIcon,
    pub action: Option<usize>,
    pub expanded: bool,
    pub section: bool,
    pub tone: CardTone,
    pub diff: Option<(u64, u64)>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkspaceCard {
    pub title: String,
    pub subtitle: String,
    pub status: String,
    pub tone: CardTone,
    pub minimized: bool,
    pub floating: bool,
    /// Logical-pixel origin, relative to terminal chrome (not the screen).
    pub position: Option<[f32; 2]>,
    pub rows: Vec<CardRow>,
    pub hover: Option<usize>,
    pub focused: bool,
    pub scroll: usize,
}
impl WorkspaceCard {
    pub fn actions(&self) -> Vec<usize> {
        let mut actions = vec![103, 102, 100, 101];
        if !self.minimized {
            actions.extend(self.rows.iter().filter_map(|r| r.action));
        }
        actions
    }
}
#[derive(Clone, Copy, Debug)]
pub struct CardLayout {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub scale: f32,
    pub count: usize,
}
impl CardLayout {
    pub fn for_card(
        width: f32,
        height: f32,
        top: f32,
        scale: f32,
        card: &WorkspaceCard,
    ) -> Option<Self> {
        let mut layout = if card.floating {
            if width / scale < 340. || (height - top) / scale < 160. {
                return None;
            }
            // Reuse normal row sizing without imposing docked terminal-width requirements.
            let mut l = Self::new(
                width.max(660. * scale),
                height.max(top + 220. * scale),
                top,
                scale,
                card.rows.len(),
            )?;
            l.x = width - l.w - 18. * scale;
            l.h = l.h.min(height - top - 20. * scale);
            l.count = (((l.h / scale - 106.) / 40.).floor().max(0.) as usize).min(card.rows.len());
            l
        } else {
            Self::new(width, height, top, scale, card.rows.len())?
        };
        if card.minimized {
            layout.h = 32.0 * scale;
            layout.count = 0;
        }
        if card.floating {
            if let Some([x, y]) = card.position.filter(|p| p.iter().all(|v| v.is_finite())) {
                layout.x = x * scale;
                layout.y = top + y * scale;
            }
            layout.x = layout.x.clamp(10. * scale, width - layout.w - 10. * scale);
            layout.y = layout
                .y
                .clamp(top + 10. * scale, height - layout.h - 10. * scale);
        }
        Some(layout)
    }

    pub fn new(width: f32, height: f32, top: f32, scale: f32, rows: usize) -> Option<Self> {
        if width / scale < 660.0 || (height - top) / scale < 220.0 {
            return None;
        }
        let w = 304.0 * scale;
        let y = top + 18.0 * scale;
        let count = (((height - y - 28.0 * scale) / scale - 106.0) / 40.0)
            .floor()
            .max(1.0) as usize;
        let count = count.min(rows);
        Some(Self {
            x: width - w - 18.0 * scale,
            y,
            w,
            h: (106.0 + count as f32 * 40.0) * scale,
            scale,
            count,
        })
    }
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
    /// Keep the draggable title region separate from every header control.
    pub fn header_drag_contains(&self, x: f32, y: f32, minimized: bool) -> bool {
        self.contains(x, y)
            && x < self.x + 158. * self.scale
            && y < self.y + if minimized { 32. } else { 44. } * self.scale
    }
    pub fn hit(&self, x: f32, y: f32, card: &WorkspaceCard) -> Option<usize> {
        if !self.contains(x, y) {
            return None;
        }
        let x = (x - self.x) / self.scale;
        let y = (y - self.y) / self.scale;
        if (if card.minimized {
            4.0..30.0
        } else {
            12.0..44.0
        })
        .contains(&y)
        {
            if (264.0..296.0).contains(&x) {
                return Some(101);
            }
            if (230.0..262.0).contains(&x) {
                return Some(100);
            }
            if (162.0..194.0).contains(&x) {
                return Some(103);
            }
            if (196.0..228.0).contains(&x) || (card.minimized && x < 162.0) {
                return Some(102);
            }
        }
        if !card.minimized && y >= 82.0 {
            let i = ((y - 82.0) / 40.0) as usize;
            if i < self.count {
                return card.rows.get(card.scroll + i).and_then(|r| r.action);
            }
        }
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pin_hit_and_keyboard_action_work_in_both_modes_and_sizes() {
        for scale in [1., 1.5, 2.] {
            for floating in [false, true] {
                for minimized in [false, true] {
                    let card = WorkspaceCard {
                        floating,
                        minimized,
                        ..Default::default()
                    };
                    let l = CardLayout::for_card(
                        1000. * scale,
                        700. * scale,
                        38. * scale,
                        scale,
                        &card,
                    )
                    .unwrap();
                    let y = if minimized { 17. } else { 27. };
                    assert_eq!(l.hit(l.x + 178. * scale, l.y + y * scale, &card), Some(103));
                    assert!(!l.header_drag_contains(
                        l.x + 178. * scale,
                        l.y + y * scale,
                        minimized
                    ));
                    assert!(l.header_drag_contains(l.x + 80. * scale, l.y + y * scale, minimized));
                    assert_eq!(card.actions(), vec![103, 102, 100, 101]);
                    for (x, id) in [(212., 102), (246., 100), (280., 101)] {
                        assert_eq!(l.hit(l.x + x * scale, l.y + y * scale, &card), Some(id));
                    }
                }
            }
        }
    }
    #[test]
    fn floating_position_is_clamped_and_hits_follow_it() {
        for scale in [1., 1.5, 2.] {
            for position in [[-100., -100.], [4000., 4000.], [100., 80.], [f32::NAN, 0.]] {
                let card = WorkspaceCard {
                    floating: true,
                    position: Some(position),
                    ..Default::default()
                };
                let l = CardLayout::for_card(700. * scale, 400. * scale, 38. * scale, scale, &card)
                    .unwrap();
                assert!(l.x >= 10. * scale && l.y >= 48. * scale);
                assert!(l.x + l.w <= 690. * scale && l.y + l.h <= 390. * scale);
                assert_eq!(
                    l.hit(l.x + 280. * scale, l.y + 25. * scale, &card),
                    Some(101)
                );
            }
            let card = WorkspaceCard {
                floating: true,
                ..Default::default()
            };
            assert!(
                CardLayout::for_card(500. * scale, 400. * scale, 38. * scale, scale, &card)
                    .is_some()
            );
            assert!(
                CardLayout::for_card(300. * scale, 400. * scale, 38. * scale, scale, &card)
                    .is_none()
            );
        }
    }
    #[test]
    fn small_windows_never_lose_terminal_space() {
        assert!(CardLayout::new(600., 600., 38., 1., 5).is_none());
    }
    #[test]
    fn scaled_card_and_hits_agree() {
        for s in [1., 1.5, 2.] {
            let l = CardLayout::new(1000. * s, 700. * s, 38. * s, s, 5).unwrap();
            assert!(l.x + l.w < 1000. * s);
            assert_eq!(
                l.hit(l.x + 280. * s, l.y + 25. * s, &WorkspaceCard::default()),
                Some(101)
            );
            assert!(!l.contains(l.x - 1., l.y));
        }
    }

    #[test]
    fn collapse_preserves_anchor_and_visibility_thresholds() {
        for scale in [1., 1.5, 2.] {
            for (width, height) in [(1000., 700.), (600., 700.), (1000., 200.)] {
                let mut card = WorkspaceCard::default();
                let expanded =
                    CardLayout::for_card(width * scale, height * scale, 38. * scale, scale, &card);
                card.minimized = true;
                let compact =
                    CardLayout::for_card(width * scale, height * scale, 38. * scale, scale, &card);
                assert_eq!(expanded.is_some(), compact.is_some());
                if let (Some(a), Some(b)) = (expanded, compact) {
                    assert_eq!((a.x, a.y, a.w), (b.x, b.y, b.w));
                }
            }
        }
    }

    #[test]
    fn minimized_header_is_compact_and_has_no_hidden_row_hits() {
        for scale in [1.0, 1.5, 2.0] {
            let card = WorkspaceCard {
                minimized: true,
                ..Default::default()
            };
            let l = CardLayout::for_card(1000. * scale, 700. * scale, 38. * scale, scale, &card)
                .unwrap();
            assert_eq!(l.h, 32. * scale);
            assert_eq!(l.count, 0);
            for (x, id) in [(210., 102), (245., 100), (280., 101)] {
                assert_eq!(l.hit(l.x + x * scale, l.y + 25. * scale, &card), Some(id));
            }
            assert_eq!(l.hit(l.x + 80. * scale, l.y + 85. * scale, &card), None);
        }
    }
}
