//! "Add task" form presentation and pointer geometry. Editing and saving
//! live in volt-ui; this is only what gets drawn and where clicks land.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormField {
    pub label: String,
    pub value: String,
    /// Shown dimmed while `value` is empty.
    pub placeholder: String,
    /// Caret position (chars) when this field has focus.
    pub cursor: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskFormView {
    pub title: String,
    /// The project the task is saved to.
    pub project: String,
    /// Name, Command, Folder.
    pub fields: Vec<FormField>,
    pub confirm: bool,
    /// Focused control: 0..fields.len() are fields, then the checkbox.
    pub focus: usize,
    /// Message under the form and whether it reports an error.
    pub status: Option<(String, bool)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormHit {
    Field(usize),
    Confirm,
    Save,
    Cancel,
    Body,
    Outside,
}

pub const FIELDS: usize = 3;
// Logical units; multiplied by the scale factor.
const W: f32 = 520.;
const H: f32 = 400.;
pub(crate) const PAD: f32 = 22.;
pub(crate) const FIELD_Y0: f32 = 76.;
pub(crate) const FIELD_STEP: f32 = 66.;
pub(crate) const LABEL_H: f32 = 20.;
pub(crate) const INPUT_H: f32 = 34.;
pub(crate) const CONFIRM_Y: f32 = FIELD_Y0 + FIELDS as f32 * FIELD_STEP + 4.;
pub(crate) const FOOTER_Y: f32 = H - 64.;
pub(crate) const BUTTON_H: f32 = 34.;
const SAVE_W: f32 = 104.;
const CANCEL_W: f32 = 92.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TaskFormLayout {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub scale: f32,
}

impl TaskFormLayout {
    /// Centered below the top chrome; `None` when the window is too small.
    pub fn new(width: f32, height: f32, top: f32, scale: f32) -> Option<Self> {
        let s = scale.max(0.1);
        let (w, h) = (W * s, H * s);
        if width < w + 24. * s || height - top < h + 24. * s {
            return None;
        }
        let gap = ((height - top - h) / 2.).min(96. * s).max(12. * s);
        Some(Self {
            x: (width - w) / 2.,
            y: top + gap,
            w,
            h,
            scale: s,
        })
    }

    fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
        let s = self.scale;
        [self.x + x * s, self.y + y * s, w * s, h * s]
    }

    pub fn label(&self, i: usize) -> [f32; 4] {
        self.rect(PAD, FIELD_Y0 + i as f32 * FIELD_STEP, W - 2. * PAD, LABEL_H)
    }

    pub fn input(&self, i: usize) -> [f32; 4] {
        self.rect(
            PAD,
            FIELD_Y0 + i as f32 * FIELD_STEP + LABEL_H + 2.,
            W - 2. * PAD,
            INPUT_H,
        )
    }

    /// The checkbox row (box plus its label), clickable as a whole.
    pub fn confirm(&self) -> [f32; 4] {
        self.rect(PAD, CONFIRM_Y, W - 2. * PAD, 28.)
    }

    pub fn save(&self) -> [f32; 4] {
        self.rect(W - PAD - SAVE_W, FOOTER_Y, SAVE_W, BUTTON_H)
    }

    pub fn cancel(&self) -> [f32; 4] {
        self.rect(
            W - PAD - SAVE_W - 10. - CANCEL_W,
            FOOTER_Y,
            CANCEL_W,
            BUTTON_H,
        )
    }

    pub fn hit(&self, x: f32, y: f32) -> FormHit {
        let inside = |r: [f32; 4]| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
        if !inside([self.x, self.y, self.w, self.h]) {
            return FormHit::Outside;
        }
        if inside(self.save()) {
            return FormHit::Save;
        }
        if inside(self.cancel()) {
            return FormHit::Cancel;
        }
        if inside(self.confirm()) {
            return FormHit::Confirm;
        }
        // The label above a field focuses it too.
        if let Some(i) = (0..FIELDS).find(|&i| inside(self.input(i)) || inside(self.label(i))) {
            return FormHit::Field(i);
        }
        FormHit::Body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn center(r: [f32; 4]) -> (f32, f32) {
        (r[0] + r[2] / 2., r[1] + r[3] / 2.)
    }

    #[test]
    fn hits_line_up_with_drawn_geometry_at_every_scale() {
        for scale in [1., 1.5, 2.] {
            let l = TaskFormLayout::new(1200. * scale, 800. * scale, 30. * scale, scale).unwrap();
            for i in 0..FIELDS {
                let (x, y) = center(l.input(i));
                assert_eq!(l.hit(x, y), FormHit::Field(i));
            }
            let (x, y) = center(l.confirm());
            assert_eq!(l.hit(x, y), FormHit::Confirm);
            let (x, y) = center(l.save());
            assert_eq!(l.hit(x, y), FormHit::Save);
            let (x, y) = center(l.cancel());
            assert_eq!(l.hit(x, y), FormHit::Cancel);
            assert_eq!(l.hit(l.x - 1., l.y + 5.), FormHit::Outside);
        }
    }

    #[test]
    fn controls_stack_inside_the_panel_and_small_windows_get_none() {
        let l = TaskFormLayout::new(1200., 800., 0., 1.).unwrap();
        let last = l.input(FIELDS - 1);
        assert!(last[1] + last[3] < l.confirm()[1]);
        assert!(l.confirm()[1] + l.confirm()[3] < l.save()[1]);
        assert!(l.save()[1] + l.save()[3] < l.y + l.h);
        assert!(l.cancel()[0] + l.cancel()[2] < l.save()[0]);
        assert!(TaskFormLayout::new(500., 800., 0., 1.).is_none());
        assert!(TaskFormLayout::new(1200., 400., 0., 1.).is_none());
    }
}
