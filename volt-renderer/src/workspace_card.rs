//! Volt's workspace presentation model. Discovery/actions live in volt-ui;
//! reusable shapes, colors, icons and buttons still come from volt-ui-kit.
//! Blocks are semantic items, not pre-wrapped display lines.
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
pub use volt_ui_kit::workspace_card::{CardIcon, CardTone};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BlockKind {
    Section,
    Action,
    #[default]
    Metadata,
    Paragraph,
    Code,
    Notice,
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
    pub kind: BlockKind,
}
impl CardRow {
    pub fn new(
        label: &str,
        detail: &str,
        icon: CardIcon,
        action: Option<usize>,
        section: bool,
        expanded: bool,
    ) -> Self {
        Self {
            label: label.into(),
            detail: detail.into(),
            icon,
            action,
            section,
            expanded,
            tone: CardTone::Muted,
            diff: None,
            kind: if section {
                BlockKind::Section
            } else if action.is_some() {
                BlockKind::Action
            } else if icon == CardIcon::Notice {
                BlockKind::Notice
            } else {
                BlockKind::Metadata
            },
        }
    }
    pub fn has_icon(&self) -> bool {
        !matches!(
            self.kind,
            BlockKind::Paragraph | BlockKind::Code | BlockKind::Notice
        )
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkspaceCard {
    pub title: String,
    pub subtitle: String,
    pub status: String,
    pub tone: CardTone,
    pub minimized: bool,
    pub floating: bool,
    pub position: Option<[f32; 2]>,
    /// One entry per item, even when the item occupies multiple display lines.
    pub rows: Vec<CardRow>,
    pub hover: Option<usize>,
    pub focused: bool,
    /// Logical-pixel content offset, not an item or wrapped-line index.
    pub scroll: usize,
}
impl WorkspaceCard {
    pub fn actions(&self) -> Vec<usize> {
        let mut out = vec![103, 102, 100, 101];
        if !self.minimized {
            out.extend(self.rows.iter().filter_map(|r| r.action));
        }
        out
    }
}
#[derive(Clone, Debug)]
pub struct BlockLayout {
    pub index: usize,
    /// Logical-pixel offset and height within the unscrolled content.
    pub offset: f32,
    pub height: f32,
    pub label: Vec<String>,
    pub detail: Vec<String>,
    pub detail_below: bool,
    pub text_left: f32,
    pub font_size: f32,
    pub line_height: f32,
}
#[derive(Clone, Debug)]
pub struct CardLayout {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub scale: f32,
    pub body: [f32; 4],
    pub blocks: Vec<BlockLayout>,
    pub max_scroll: usize,
    pub scroll: usize,
}
impl CardLayout {
    pub fn for_card(
        width: f32,
        height: f32,
        top: f32,
        scale: f32,
        card: &WorkspaceCard,
    ) -> Option<Self> {
        if ![width, height, top, scale].into_iter().all(f32::is_finite) || scale <= 0. || top < 0. {
            return None;
        }
        let min_width = if card.floating { 340. } else { 660. };
        if width / scale < min_width || (height - top) / scale < 160. {
            return None;
        }
        let mut offset = 0.;
        let mut blocks = vec![];
        for (index, row) in card.rows.iter().enumerate() {
            let font_size = if matches!(
                row.kind,
                BlockKind::Metadata | BlockKind::Paragraph | BlockKind::Code | BlockKind::Notice
            ) {
                11.
            } else {
                12.
            };
            let line_height = font_size * 1.45;
            let text_left = if row.has_icon() { 50. } else { 30. };
            let right = if row.action.is_some() { 38. } else { 24. };
            let full_width = 304. - text_left - right;
            let detail_below = row.detail.width() > 14;
            let detail_width = if row.detail.is_empty() || detail_below {
                0.
            } else {
                row.detail.width() as f32 * 11. * 0.68 + 12.
            };
            let diff_width = if row.diff.is_some() { 106. } else { 0. };
            let cols = ((full_width - detail_width.max(diff_width)).max(60.) / (font_size * 0.72))
                .floor() as usize;
            let label = wrap_block(&row.label, cols.max(1));
            let detail = if detail_below {
                wrap_block(
                    &row.detail,
                    (full_width / (11. * 0.72)).floor().max(1.) as usize,
                )
            } else {
                vec![]
            };
            let text_height = label.len() as f32 * line_height + detail.len() as f32 * 16.;
            let min_height = if row.section {
                40.
            } else if row.action.is_some() {
                36.
            } else {
                26.
            };
            let block_height = (text_height + 14.).max(min_height);
            blocks.push(BlockLayout {
                index,
                offset,
                height: block_height,
                label,
                detail,
                detail_below,
                text_left,
                font_size,
                line_height,
            });
            offset += block_height;
        }
        let max_h = (height - top - 36. * scale).max(0.);
        let h = if card.minimized {
            32. * scale
        } else {
            (106. + offset) * scale
        }
        .min(max_h);
        let w = 304. * scale;
        let mut x = width - w - 18. * scale;
        let mut y = top + 18. * scale;
        if card.floating {
            if let Some([px, py]) = card.position.filter(|p| p.iter().all(|v| v.is_finite())) {
                x = px * scale;
                y = top + py * scale;
            }
            x = x.clamp(10. * scale, (width - w - 10. * scale).max(10. * scale));
            y = y.clamp(
                top + 10. * scale,
                (height - h - 10. * scale).max(top + 10. * scale),
            );
        }
        let body = [
            x + 10. * scale,
            y + 82. * scale,
            w - 20. * scale,
            if card.minimized {
                0.
            } else {
                (h - 106. * scale).max(0.)
            },
        ];
        let max_scroll = if card.minimized {
            0
        } else {
            (offset - body[3] / scale).max(0.).ceil() as usize
        };
        Some(Self {
            x,
            y,
            w,
            h,
            scale,
            body,
            blocks,
            max_scroll,
            scroll: card.scroll.min(max_scroll),
        })
    }
    pub fn contains(&self, x: f32, y: f32) -> bool {
        contains([self.x, self.y, self.w, self.h], x, y)
    }
    pub fn header_drag_contains(&self, x: f32, y: f32, minimized: bool) -> bool {
        self.contains(x, y)
            && x < self.x + 158. * self.scale
            && y < self.y + if minimized { 32. } else { 44. } * self.scale
    }
    pub fn hit(&self, x: f32, y: f32, card: &WorkspaceCard) -> Option<usize> {
        if !self.contains(x, y) {
            return None;
        }
        let lx = (x - self.x) / self.scale;
        let ly = (y - self.y) / self.scale;
        if (if card.minimized { 4.0..30. } else { 12.0..44. }).contains(&ly) {
            for (start, id) in [(264., 101), (230., 100), (162., 103), (196., 102)] {
                if (start..start + 32.).contains(&lx) {
                    return Some(id);
                }
            }
            if card.minimized && lx < 162. {
                return Some(102);
            }
        }
        if !card.minimized && contains(self.body, x, y) {
            let at = (y - self.body[1]) / self.scale + self.scroll as f32;
            return self
                .blocks
                .iter()
                .find(|b| at >= b.offset && at < b.offset + b.height)
                .and_then(|b| card.rows[b.index].action);
        }
        None
    }
    pub fn scroll_for_action(&self, action: usize, card: &WorkspaceCard) -> usize {
        let Some(b) = self
            .blocks
            .iter()
            .find(|b| card.rows[b.index].action == Some(action))
        else {
            return self.scroll;
        };
        let height = self.body[3] / self.scale;
        let next = if b.offset < self.scroll as f32 || b.height > height {
            b.offset
        } else if b.offset + b.height > self.scroll as f32 + height {
            b.offset + b.height - height
        } else {
            self.scroll as f32
        };
        (next.max(0.).ceil() as usize).min(self.max_scroll)
    }
    pub fn block_rect(&self, b: &BlockLayout) -> [f32; 4] {
        [
            self.body[0],
            self.body[1] + (b.offset - self.scroll as f32) * self.scale,
            self.body[2],
            b.height * self.scale,
        ]
    }
}
pub fn contains(r: [f32; 4], x: f32, y: f32) -> bool {
    x.is_finite() && y.is_finite() && x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}
/// Preserve every character; wrapping is presentation only. Controls and bidi
/// formatting are visible literals, never interpreted as terminal sequences.
pub fn wrap_block(text: &str, columns: usize) -> Vec<String> {
    let mut lines = vec![];
    let mut line = String::new();
    let mut width = 0;
    for ch in text.chars() {
        if ch == '\n' {
            lines.push(std::mem::take(&mut line));
            width = 0;
            continue;
        }
        let visible = if ch.is_control()
            || matches!(ch,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}'|'\u{200e}'|'\u{200f}')
        {
            ch.escape_default().to_string()
        } else {
            ch.to_string()
        };
        for c in visible.chars() {
            let cw = c.width().unwrap_or(0);
            if width + cw > columns.max(1) && !line.is_empty() {
                let split = line
                    .rfind(|c: char| c.is_whitespace() || c == '/')
                    .map(|i| i + line[i..].chars().next().unwrap().len_utf8())
                    .filter(|&i| line[..i].width() >= columns.max(1) / 3);
                if let Some(at) = split {
                    let tail = line.split_off(at);
                    lines.push(std::mem::replace(&mut line, tail));
                    width = line.width();
                } else {
                    lines.push(std::mem::take(&mut line));
                    width = 0;
                }
            }
            line.push(c);
            width += cw;
        }
    }
    lines.push(line);
    lines
}
#[cfg(test)]
mod tests {
    use super::*;
    fn card() -> WorkspaceCard {
        WorkspaceCard {
            rows: vec![
                CardRow::new("Project", "", CardIcon::Folder, Some(5), true, true),
                CardRow::new(
                    "/Users/neo/workspace/rust/terminal-public",
                    "",
                    CardIcon::Folder,
                    None,
                    false,
                    false,
                ),
                CardRow::new("Build", "Review", CardIcon::Play, Some(1100), false, false),
            ],
            ..Default::default()
        }
    }
    #[test]
    fn wrapping_preserves_one_item_and_action_geometry_at_every_scale() {
        for scale in [1., 1.5, 2., 3.] {
            let c = card();
            let l =
                CardLayout::for_card(1000. * scale, 700. * scale, 38. * scale, scale, &c).unwrap();
            assert_eq!(l.blocks.len(), 3);
            assert!(l.blocks[1].label.len() > 1);
            assert_eq!(l.blocks[1].label.join(""), c.rows[1].label);
            for b in &l.blocks {
                let r = l.block_rect(b);
                assert_eq!(
                    l.hit(r[0] + 20. * scale, r[1] + r[3] * 0.5, &c),
                    c.rows[b.index].action
                );
            }
            for (x, id) in [(178., 103), (212., 102), (246., 100), (280., 101)] {
                assert_eq!(l.hit(l.x + x * scale, l.y + 27. * scale, &c), Some(id));
                assert!(!l.header_drag_contains(l.x + x * scale, l.y + 27. * scale, false));
            }
        }
    }
    #[test]
    fn huge_blocks_scroll_to_the_end_without_becoming_more_items() {
        let mut c = card();
        c.rows[1].label = format!("{}SAFE_END", "日本語/e\u{301}/".repeat(300));
        c.scroll = usize::MAX;
        let l = CardLayout::for_card(1000., 300., 38., 1., &c).unwrap();
        assert_eq!(l.blocks.len(), 3);
        assert!(l.max_scroll > 1000);
        assert_eq!(l.scroll, l.max_scroll);
        assert!(l.blocks[1].label.join("").ends_with("SAFE_END"));
        let at = l.scroll_for_action(1100, &c);
        assert!(at <= l.max_scroll);
        let r = l.block_rect(&l.blocks[2]);
        assert_eq!(l.hit(r[0] + 20., r[1] + r[3] * 0.5, &c), Some(1100));
        assert_eq!(l.hit(f32::NAN, 0., &c), None);
    }
    #[test]
    fn modes_and_collapse_keep_anchor_and_reservation_contract() {
        for floating in [false, true] {
            let mut c = card();
            c.floating = floating;
            let a = CardLayout::for_card(1000., 700., 38., 1., &c).unwrap();
            c.minimized = true;
            let b = CardLayout::for_card(1000., 700., 38., 1., &c).unwrap();
            assert_eq!((a.x, a.y), (b.x, b.y));
            assert_eq!(b.h, 32.);
            assert_eq!(b.hit(b.x + 80., b.y + 17., &c), Some(102));
        }
        assert!(CardLayout::for_card(1000., 700., 0., f32::NAN, &card()).is_none());
    }
}
