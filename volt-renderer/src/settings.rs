//! Settings/onboarding presentation; no file access or execution.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsRow {
    pub label: String,
    pub value: String,
    pub hint: String,
    pub editing: bool,
    pub adjustable: bool,
    pub actionable: bool,
    pub caret: Option<usize>,
    pub selected: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsView {
    pub title: String,
    pub subtitle: String,
    pub sections: Vec<String>,
    pub section: usize,
    pub rows: Vec<SettingsRow>,
    pub focus: usize,
    pub status: String,
    pub onboarding: bool,
    pub preview: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsHit {
    Section(usize),
    Row(usize),
    Minus(usize),
    Plus(usize),
    Apply,
    Cancel,
    Back,
    Config,
    Body,
    Outside,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SettingsLayout {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub scale: f32,
    pub rows: usize,
}
impl SettingsLayout {
    pub fn new(width: f32, height: f32, top: f32, scale: f32) -> Option<Self> {
        let scale = scale.max(0.1);
        let w = (width - 24. * scale).min(680. * scale);
        let h = (height - top - 24. * scale).min(590. * scale);
        if w < 460. * scale || h < 370. * scale {
            return None;
        }
        let rows = ((h / scale - 185.) / 54.).floor().clamp(1., 7.) as usize;
        Some(Self {
            x: (width - w) / 2.,
            y: top + (height - top - h) / 2.,
            w,
            h,
            scale,
            rows,
        })
    }
    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
        [
            self.x + x * self.scale,
            self.y + y * self.scale,
            w * self.scale,
            h * self.scale,
        ]
    }
    pub fn section(&self, i: usize) -> [f32; 4] {
        self.rect(14., 78. + i as f32 * 26., 122., 24.)
    }
    pub fn row(&self, i: usize) -> [f32; 4] {
        self.rect(151., 78. + i as f32 * 54., self.w / self.scale - 166., 49.)
    }
    pub fn footer(&self, i: usize) -> [f32; 4] {
        self.rect(
            self.w / self.scale - 112. - i as f32 * 100.,
            self.h / self.scale - 48.,
            92.,
            32.,
        )
    }
    pub fn config(&self) -> [f32; 4] {
        self.rect(14., self.h / self.scale - 48., 124., 32.)
    }
    pub fn hit(&self, x: f32, y: f32, sections: usize) -> SettingsHit {
        let inside = |r: [f32; 4]| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
        if !inside([self.x, self.y, self.w, self.h]) {
            return SettingsHit::Outside;
        }
        for (i, hit) in [
            (0, SettingsHit::Apply),
            (1, SettingsHit::Cancel),
            (2, SettingsHit::Back),
        ] {
            if inside(self.footer(i)) {
                return hit;
            }
        }
        if inside(self.config()) {
            return SettingsHit::Config;
        }
        for i in 0..sections {
            if inside(self.section(i)) {
                return SettingsHit::Section(i);
            }
        }
        for i in 0..self.rows {
            let r = self.row(i);
            if inside(r) {
                let v = if x > r[0] + r[2] - 26. * self.scale {
                    SettingsHit::Plus(i)
                } else if x > r[0] + r[2] - 52. * self.scale {
                    SettingsHit::Minus(i)
                } else {
                    SettingsHit::Row(i)
                };
                return v;
            }
        }
        SettingsHit::Body
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fits_default_retina_and_hits_at_multiple_scales() {
        for s in [1., 1.5, 2.] {
            let l = SettingsLayout::new(700. * s, 450. * s, 56. * s, s).unwrap();
            assert!(l.y >= 56. * s && l.y + l.h <= 450. * s);
            for i in 0..l.rows {
                let r = l.row(i);
                assert!(r[1] + r[3] < l.footer(0)[1]);
                assert_eq!(l.hit(r[0] + 5. * s, r[1] + 5. * s, 4), SettingsHit::Row(i));
            }
            let r = l.footer(0);
            assert_eq!(l.hit(r[0] + 2. * s, r[1] + 2. * s, 4), SettingsHit::Apply);
        }
    }
}
