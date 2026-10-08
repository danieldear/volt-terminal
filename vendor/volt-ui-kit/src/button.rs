//! Small reusable button: shared painting and hit testing, no application action.
use crate::{CardIcon, Painter};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ButtonColors {
    pub surface: [f32; 4],
    pub text: [f32; 4],
}
#[derive(Clone, Copy, Debug)]
pub struct Button<'a> {
    pub label: &'a str,
    pub icon: Option<CardIcon>,
    pub enabled: bool,
}
impl Button<'_> {
    pub fn hit(&self, rect: [f32; 4], x: f32, y: f32) -> bool {
        self.enabled
            && rect.into_iter().all(f32::is_finite)
            && x.is_finite()
            && y.is_finite()
            && x >= rect[0]
            && y >= rect[1]
            && x < rect[0] + rect[2]
            && y < rect[1] + rect[3]
    }
    #[allow(clippy::too_many_arguments)]
    pub fn paint<P: Painter>(
        &self,
        p: &mut P,
        b: &mut Vec<P::Shape>,
        g: &mut Vec<P::Glyph>,
        r: [f32; 4],
        scale: f32,
        colors: ButtonColors,
    ) {
        p.card_round(b, r, 7. * scale, colors.surface);
        let mut color = colors.text;
        if !self.enabled {
            color[3] *= 0.45;
        }
        let size = 12. * scale;
        let icon_room = if self.icon.is_some() { 22. * scale } else { 0. };
        let width = icon_room + P::approx_text_width(self.label, size);
        let x = r[0] + (r[2] - width) * 0.5;
        if let Some(icon) = self.icon {
            p.card_icon(b, icon, x, r[1] + (r[3] - 16. * scale) * 0.5, scale, color);
        }
        p.card_text(
            g,
            self.label,
            x + icon_room,
            r[1] + (r[3] - size * 1.2) * 0.5,
            size,
            color,
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hits_are_half_open_and_disabled_buttons_cannot_activate() {
        let mut b = Button {
            label: "Apply",
            icon: Some(CardIcon::Check),
            enabled: true,
        };
        for scale in [1., 1.5, 2., 3.] {
            let r = [20. * scale, 40. * scale, 92. * scale, 32. * scale];
            assert!(b.hit(r, r[0], r[1]));
            assert!(b.hit(r, r[0] + r[2] / 2., r[1] + r[3] / 2.));
            assert!(!b.hit(r, r[0] + r[2], r[1]));
            assert!(!b.hit(r, f32::NAN, r[1]));
        }
        b.enabled = false;
        assert!(!b.hit([20., 40., 92., 32.], 40., 50.));
    }
}
