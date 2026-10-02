//! Theme editor presentation and pointer geometry. The editing state,
//! validation and saving live in volt-ui; this is only what gets drawn and
//! where clicks land. Never reserves terminal cells: the panel floats over
//! the top-right corner so the live terminal stays visible as the preview.
use volt_config::Color;

/// Number of editable color slots: six base colors, then ANSI 0–15.
pub const FIELD_COUNT: usize = 22;
pub const BASE_FIELDS: usize = 6;

const PANEL_W: f32 = 300.;
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
const BUTTON_H: f32 = 26.;
const SAVE_W: f32 = 84.;
const REVERT_W: f32 = 66.;

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
}

impl ThemeEditorLayout {
    pub fn new(width: f32, height: f32, top: f32, scale: f32) -> Option<Self> {
        let s = scale.max(0.1);
        let (w, h) = (PANEL_W * s, PANEL_H * s);
        if width < w + 2. * MARGIN * s || height - top < h + 2. * MARGIN * s {
            return None;
        }
        Some(Self {
            x: width - w - MARGIN * s,
            y: top + MARGIN * s,
            w,
            h,
            scale: s,
        })
    }

    /// Physical `[x, y, w, h]` from panel-relative logical units.
    fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
        let s = self.scale;
        [self.x + x * s, self.y + y * s, w * s, h * s]
    }

    pub fn base_row(&self, i: usize) -> [f32; 4] {
        self.rect(8., ROW_Y0 + i as f32 * ROW_H, PANEL_W - 16., ROW_H - 2.)
    }

    pub fn detail_row(&self) -> [f32; 4] {
        self.rect(8., DETAIL_Y, PANEL_W - 16., ROW_H - 2.)
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
        let x0 = (PANEL_W - (8. * CELL + 7. * GAP)) / 2.;
        let (col, row) = ((i % 8) as f32, (i / 8) as f32);
        self.rect(
            x0 + col * (CELL + GAP),
            GRID_Y0 + row * (CELL + GAP),
            CELL,
            CELL,
        )
    }

    pub fn save_button(&self) -> [f32; 4] {
        self.rect(PANEL_W - PAD - SAVE_W, FOOTER_Y, SAVE_W, BUTTON_H)
    }

    pub fn revert_button(&self) -> [f32; 4] {
        self.rect(
            PANEL_W - PAD - SAVE_W - 8. - REVERT_W,
            FOOTER_Y,
            REVERT_W,
            BUTTON_H,
        )
    }

    pub fn name_field(&self) -> [f32; 4] {
        self.rect(PAD, FOOTER_Y, PANEL_W - 2. * PAD - SAVE_W - 8., BUTTON_H)
    }

    pub fn hit(&self, x: f32, y: f32, naming: bool) -> EditorHit {
        let inside = |r: [f32; 4]| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
        if !inside([self.x, self.y, self.w, self.h]) {
            return EditorHit::Outside;
        }
        if inside(self.rect(PANEL_W - 34., 0., 34., 34.)) {
            return EditorHit::Close;
        }
        if inside(self.save_button()) {
            return EditorHit::Save;
        }
        if !naming && inside(self.revert_button()) {
            return EditorHit::Revert;
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

#[cfg(test)]
mod tests {
    use super::*;

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
