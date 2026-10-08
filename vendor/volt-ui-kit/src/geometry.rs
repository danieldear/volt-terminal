//! Validated physical-pixel bounds and topmost-first pointer routing.
//! Hosts rebuild the hit map with their layout; this module owns no event loop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect([f32; 4]);
impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Option<Self> {
        ([x, y, width, height, x + width, y + height]
            .into_iter()
            .all(f32::is_finite)
            && width > 0.
            && height > 0.)
            .then_some(Self([x, y, width, height]))
    }
    pub fn bounds(self) -> [f32; 4] {
        self.0
    }
    pub fn contains(self, x: f32, y: f32) -> bool {
        let [left, top, w, h] = self.0;
        x.is_finite() && y.is_finite() && x >= left && y >= top && x < left + w && y < top + h
    }
    pub fn contains_rounded(self, radius: f32, x: f32, y: f32) -> bool {
        if !self.contains(x, y) || !radius.is_finite() || radius < 0. {
            return false;
        }
        let [left, top, w, h] = self.0;
        let r = radius.min(w * 0.5).min(h * 0.5);
        if r == 0. {
            return true;
        }
        let cx = x.clamp(left + r, left + w - r);
        let cy = y.clamp(top + r, top + h - r);
        (x - cx).hypot(y - cy) <= r
    }
}
/// A background can consume clicks without pretending to be an action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HitTarget<T> {
    Action(T),
    Block,
}
#[derive(Clone, Debug)]
struct Region<T> {
    rect: Rect,
    radius: f32,
    clip: Option<Rect>,
    target: HitTarget<T>,
}
#[derive(Clone, Debug)]
pub struct HitMap<T> {
    regions: Vec<Region<T>>,
}
impl<T> Default for HitMap<T> {
    fn default() -> Self {
        Self {
            regions: Vec::new(),
        }
    }
}
impl<T> HitMap<T> {
    pub fn clear(&mut self) {
        self.regions.clear();
    }
    /// Register in paint order (bottom to top). Invalid radii are rejected.
    pub fn push(
        &mut self,
        rect: Rect,
        radius: f32,
        clip: Option<Rect>,
        target: HitTarget<T>,
    ) -> bool {
        if !radius.is_finite() || radius < 0. {
            return false;
        }
        self.regions.push(Region {
            rect,
            radius,
            clip,
            target,
        });
        true
    }
    pub fn hit(&self, x: f32, y: f32) -> Option<&HitTarget<T>> {
        self.regions
            .iter()
            .rev()
            .find(|r| {
                r.rect.contains_rounded(r.radius, x, y) && r.clip.is_none_or(|c| c.contains(x, y))
            })
            .map(|r| &r.target)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_bounds_and_edges() {
        for b in [
            [0., 0., 0., 1.],
            [0., 0., -1., 1.],
            [f32::NAN, 0., 1., 1.],
            [f32::MAX, 0., f32::MAX, 1.],
        ] {
            assert!(Rect::new(b[0], b[1], b[2], b[3]).is_none());
        }
        let r = Rect::new(10., 20., 100., 80.).unwrap();
        assert!(r.contains(10., 20.));
        assert!(!r.contains(110., 20.));
        assert!(!r.contains(f32::NAN, 30.));
        assert!(!r.contains_rounded(14., 10., 20.));
        assert!(r.contains_rounded(14., 60., 20.));
        assert!(!r.contains_rounded(f32::NAN, 60., 40.));
    }
    #[test]
    fn topmost_blocker_clip_and_clear() {
        let r = Rect::new(0., 0., 100., 100.).unwrap();
        let mut map = HitMap::default();
        map.push(r, 0., None, HitTarget::Action(1));
        map.push(r, 14., None, HitTarget::Block);
        assert_eq!(map.hit(50., 50.), Some(&HitTarget::Block));
        assert_eq!(map.hit(0., 0.), Some(&HitTarget::Action(1)));
        map.push(
            r,
            0.,
            Some(Rect::new(20., 20., 20., 20.).unwrap()),
            HitTarget::Action(2),
        );
        assert_eq!(map.hit(25., 25.), Some(&HitTarget::Action(2)));
        assert_eq!(map.hit(50., 50.), Some(&HitTarget::Block));
        assert!(!map.push(r, -1., None, HitTarget::Block));
        map.clear();
        assert_eq!(map.hit(25., 25.), None);
    }
}
