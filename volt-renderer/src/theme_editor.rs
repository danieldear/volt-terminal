//! Theme editor presentation and pointer geometry. The editing state,
//! validation and saving live in volt-ui; this is only what gets drawn and
//! where clicks land. Never reserves terminal cells: the panel floats over
//! the top-right corner so the live terminal stays visible as the preview.
use volt_config::Color;

/// Number of editable color slots: six base colors, then ANSI 0–15.
pub const FIELD_COUNT: usize = 22;
pub const BASE_FIELDS: usize = 6;

const PANEL_W: f32 = 620.;
const LEFT_W: f32 = 300.;
const PANEL_H: f32 = 420.;
const MARGIN: f32 = 12.;
const PAD: f32 = 14.;
const ROW_Y0: f32 = 66.;
const ROW_H: f32 = 26.;
const GRID_Y0: f32 = 248.;
const CELL: f32 = 28.;
const GAP: f32 = 6.;
const DETAIL_Y: f32 = 318.;
const FOOTER_Y: f32 = 360.;
const BUTTON_H: f32 = 28.;
const SAVE_W: f32 = 110.;
const REVERT_W: f32 = 84.;

#[derive(Clone, Debug, PartialEq)]
pub struct EditorRow {
    pub label: String,
    /// The color currently applied to the live theme.
    pub color: Color,
    /// What the field shows — may be a partial hex while the user types.
    pub text: String,
    pub valid: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThemeEditorView {
    pub subtitle: String,
    pub picked: Color,
    pub picker_hue: f32,
    pub picker_open: bool,
    pub picked_label: String,
    /// The six base-color rows.
    pub rows: Vec<EditorRow>,
    pub ansi: [Color; 16],
    /// Row for the selected ANSI color, shown under the grid.
    pub detail: EditorRow,
    /// Focused field, `0..FIELD_COUNT`.
    pub focus: usize,
    /// Name being typed for "Save as", with the cursor's char index.
    pub naming: Option<(String, usize)>,
    pub save_label: String,
    /// Message under the footer and whether it reports an error.
    pub status: Option<(String, bool)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorHit {
    Field(usize),
    SaturationValue,
    Hue,
    Channel(usize),
    Preset(usize),
    Back,
    Close,
    Revert,
    Save,
    Body,
    Outside,
}

/// Panel placement in physical pixels. `None` when the window is too small
/// to show the whole panel; the caller then closes the editor rather than
/// leaving an invisible modal capturing the keyboard.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeEditorLayout {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub scale: f32,
    pub compact: bool,
    pub picker_open: bool,
}

impl ThemeEditorLayout {
    pub fn new(width: f32, height: f32, top: f32, scale: f32) -> Option<Self> {
        if ![width, height, top, scale].into_iter().all(f32::is_finite) || scale <= 0. {
            return None;
        }
        let s = scale;
        let compact = width < (PANEL_W + 2. * MARGIN) * s;
        let (w, h) = (if compact { LEFT_W } else { PANEL_W } * s, PANEL_H * s);
        if width < w + 2. * MARGIN * s || height - top < h + 2. * MARGIN * s {
            return None;
        }
        Some(Self {
            x: width - w - MARGIN * s,
            y: top + MARGIN * s,
            w,
            h,
            scale: s,
            compact,
            picker_open: false,
        })
    }

    /// Physical `[x, y, w, h]` from panel-relative logical units.
    fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
        let s = self.scale;
        [self.x + x * s, self.y + y * s, w * s, h * s]
    }

    pub fn base_row(&self, i: usize) -> [f32; 4] {
        self.rect(8., ROW_Y0 + i as f32 * ROW_H, LEFT_W - 16., ROW_H - 2.)
    }

    pub fn detail_row(&self) -> [f32; 4] {
        self.rect(8., DETAIL_Y, LEFT_W - 16., ROW_H - 2.)
    }

    /// Hex field inside a row rect.
    pub fn field_in(&self, row: [f32; 4]) -> [f32; 4] {
        let s = self.scale;
        [
            row[0] + row[2] - 86. * s,
            row[1] + 2. * s,
            80. * s,
            row[3] - 4. * s,
        ]
    }

    pub fn swatch_in(&self, row: [f32; 4]) -> [f32; 4] {
        let s = self.scale;
        [
            row[0] + 6. * s,
            row[1] + (row[3] - 14. * s) / 2.,
            14. * s,
            14. * s,
        ]
    }

    pub fn ansi_cell(&self, i: usize) -> [f32; 4] {
        let x0 = (LEFT_W - (8. * CELL + 7. * GAP)) / 2.;
        let (col, row) = ((i % 8) as f32, (i / 8) as f32);
        self.rect(
            x0 + col * (CELL + GAP),
            GRID_Y0 + row * (CELL + GAP),
            CELL,
            CELL,
        )
    }

    pub fn save_button(&self) -> [f32; 4] {
        self.rect(
            self.w / self.scale - PAD - SAVE_W,
            FOOTER_Y,
            SAVE_W,
            BUTTON_H,
        )
    }

    pub fn revert_button(&self) -> [f32; 4] {
        self.rect(
            self.w / self.scale - PAD - SAVE_W - 8. - REVERT_W,
            FOOTER_Y,
            REVERT_W,
            BUTTON_H,
        )
    }

    pub fn name_field(&self) -> [f32; 4] {
        self.rect(
            PAD,
            FOOTER_Y,
            self.w / self.scale - 2. * PAD - SAVE_W - 8.,
            BUTTON_H,
        )
    }

    pub fn picker_rect(&self) -> [f32; 4] {
        self.rect(
            if self.compact { 14. } else { 316. },
            84.,
            if self.compact { 272. } else { 286. },
            126.,
        )
    }
    pub fn hue_rect(&self) -> [f32; 4] {
        let r = self.picker_rect();
        [r[0], self.y + 224. * self.scale, r[2], 14. * self.scale]
    }
    pub fn channel_rect(&self, i: usize) -> [f32; 4] {
        let r = self.picker_rect();
        [
            r[0] + 24. * self.scale,
            self.y + (246. + i as f32 * 26.) * self.scale,
            r[2] - 60. * self.scale,
            16. * self.scale,
        ]
    }
    pub fn preset_rect(&self, i: usize) -> [f32; 4] {
        let r = self.picker_rect();
        [
            r[0] + i as f32 * (r[2] / 12.),
            self.y + 326. * self.scale,
            r[2] / 12. - 3. * self.scale,
            20. * self.scale,
        ]
    }
    pub fn back_button(&self) -> [f32; 4] {
        self.rect(8., 6., 36., 28.)
    }
    pub fn picker_visible(&self) -> bool {
        !self.compact || self.picker_open
    }

    pub fn hit(&self, x: f32, y: f32, naming: bool) -> EditorHit {
        let inside = |r: [f32; 4]| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
        if !inside([self.x, self.y, self.w, self.h]) {
            return EditorHit::Outside;
        }
        if inside(self.rect(self.w / self.scale - 34., 0., 34., 34.)) {
            return EditorHit::Close;
        }
        if inside(self.save_button()) {
            return EditorHit::Save;
        }
        if !naming && inside(self.revert_button()) {
            return EditorHit::Revert;
        }
        if self.compact && self.picker_open && !naming && inside(self.back_button()) {
            return EditorHit::Back;
        }
        if self.picker_visible() && !naming {
            if inside(self.picker_rect()) {
                return EditorHit::SaturationValue;
            }
            if inside(self.hue_rect()) {
                return EditorHit::Hue;
            }
            if let Some(i) = (0..3).find(|&i| inside(self.channel_rect(i))) {
                return EditorHit::Channel(i);
            }
            if let Some(i) = (0..12).find(|&i| inside(self.preset_rect(i))) {
                return EditorHit::Preset(i);
            }
        }
        if self.compact && self.picker_open {
            return EditorHit::Body;
        }
        if let Some(i) = (0..BASE_FIELDS).find(|&i| inside(self.base_row(i))) {
            return EditorHit::Field(i);
        }
        if let Some(i) = (0..16).find(|&i| inside(self.ansi_cell(i))) {
            return EditorHit::Field(BASE_FIELDS + i);
        }
        EditorHit::Body
    }
}

pub const PRESETS: [Color; 12] = [
    Color::rgb(239, 68, 68),
    Color::rgb(249, 115, 22),
    Color::rgb(245, 158, 11),
    Color::rgb(234, 179, 8),
    Color::rgb(34, 197, 94),
    Color::rgb(20, 184, 166),
    Color::rgb(59, 130, 246),
    Color::rgb(99, 102, 241),
    Color::rgb(168, 85, 247),
    Color::rgb(236, 72, 153),
    Color::rgb(255, 255, 255),
    Color::rgb(0, 0, 0),
];
pub fn rgb_to_hsv(c: Color) -> [f32; 3] {
    let [r, g, b] = [c.r as f32 / 255., c.g as f32 / 255., c.b as f32 / 255.];
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0. {
        0.
    } else if max == r {
        60. * ((g - b) / d).rem_euclid(6.)
    } else if max == g {
        60. * ((b - r) / d + 2.)
    } else {
        60. * ((r - g) / d + 4.)
    };
    [h, if max == 0. { 0. } else { d / max }, max]
}
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color {
    if ![h, s, v].into_iter().all(f32::is_finite) {
        return Color::rgb(0, 0, 0);
    }
    let h = h.rem_euclid(360.) / 60.;
    let s = s.clamp(0., 1.);
    let v = v.clamp(0., 1.);
    let c = v * s;
    let x = c * (1. - (h % 2. - 1.).abs());
    let m = v - c;
    let a = match h as u8 {
        0 => [c, x, 0.],
        1 => [x, c, 0.],
        2 => [0., c, x],
        3 => [0., x, c],
        4 => [x, 0., c],
        _ => [c, 0., x],
    };
    let [r, g, b] = a.map(|a| ((a + m) * 255.).round().clamp(0., 255.) as u8);
    Color::rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picker_targets_roundtrip_and_compact_navigation() {
        for scale in [1., 1.5, 2.] {
            for width in [400., 1000.] {
                let mut l = ThemeEditorLayout::new(width * scale, 800. * scale, 30. * scale, scale)
                    .unwrap();
                l.picker_open = true;
                let center = |r: [f32; 4]| (r[0] + r[2] * 0.5, r[1] + r[3] * 0.5);
                for (r, target) in [
                    (l.picker_rect(), EditorHit::SaturationValue),
                    (l.hue_rect(), EditorHit::Hue),
                ] {
                    let (x, y) = center(r);
                    assert_eq!(l.hit(x, y, false), target);
                    assert_eq!(l.hit(x, y, true), EditorHit::Body);
                }
                for i in 0..3 {
                    let (x, y) = center(l.channel_rect(i));
                    assert_eq!(l.hit(x, y, false), EditorHit::Channel(i));
                }
                for i in 0..12 {
                    let (x, y) = center(l.preset_rect(i));
                    assert_eq!(l.hit(x, y, false), EditorHit::Preset(i));
                }
            }
        }
        for c in PRESETS.into_iter().chain([
            Color::rgb(128, 128, 128),
            Color::rgb(1, 2, 3),
            Color::rgb(251, 121, 19),
        ]) {
            let [h, s, v] = rgb_to_hsv(c);
            assert_eq!(hsv_to_rgb(h, s, v), c);
        }
    }

    #[test]
    fn hits_line_up_with_drawn_geometry_at_every_scale() {
        for scale in [1., 1.5, 2.] {
            let l =
                ThemeEditorLayout::new(1200. * scale, 800. * scale, 30. * scale, scale).unwrap();
            assert!(l.x + l.w <= 1200. * scale && l.y + l.h <= 800. * scale);
            let center = |r: [f32; 4]| (r[0] + r[2] / 2., r[1] + r[3] / 2.);
            for i in 0..BASE_FIELDS {
                let (x, y) = center(l.base_row(i));
                assert_eq!(l.hit(x, y, false), EditorHit::Field(i));
            }
            for i in 0..16 {
                let (x, y) = center(l.ansi_cell(i));
                assert_eq!(l.hit(x, y, false), EditorHit::Field(BASE_FIELDS + i));
            }
            let (x, y) = center(l.save_button());
            assert_eq!(l.hit(x, y, false), EditorHit::Save);
            assert_eq!(l.hit(x, y, true), EditorHit::Save);
            let (x, y) = center(l.revert_button());
            assert_eq!(l.hit(x, y, false), EditorHit::Revert);
            assert_eq!(l.hit(x, y, true), EditorHit::Body, "no Revert while naming");
            assert_eq!(
                l.hit(l.x + l.w - 5. * scale, l.y + 5. * scale, false),
                EditorHit::Close
            );
            assert_eq!(l.hit(l.x - 1., l.y + 10., false), EditorHit::Outside);
        }
    }

    #[test]
    fn ansi_grid_and_rows_stay_inside_the_panel_without_overlapping() {
        let l = ThemeEditorLayout::new(1000., 800., 0., 1.).unwrap();
        let last_row = l.base_row(BASE_FIELDS - 1);
        let first_cell = l.ansi_cell(0);
        assert!(last_row[1] + last_row[3] < first_cell[1]);
        let last_cell = l.ansi_cell(15);
        assert!(last_cell[0] + last_cell[2] <= l.x + l.w);
        assert!(last_cell[1] + last_cell[3] < l.detail_row()[1]);
        assert!(l.detail_row()[1] + l.detail_row()[3] < l.save_button()[1]);
        assert!(l.save_button()[1] + l.save_button()[3] < l.y + l.h);
    }

    #[test]
    fn too_small_windows_get_no_layout() {
        assert!(ThemeEditorLayout::new(320., 900., 0., 1.).is_none());
        assert!(ThemeEditorLayout::new(1000., 440., 0., 1.).is_none());
        assert!(ThemeEditorLayout::new(1000., 880., 0., 2.).is_none());
    }
}
