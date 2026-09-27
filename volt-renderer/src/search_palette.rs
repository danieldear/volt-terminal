//! Search presentation and shared pointer geometry; never reserves terminal cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchRow {
    pub kind: String,
    pub title: String,
    pub detail: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchView {
    pub query: String,
    pub query_selected: bool,
    pub cursor: usize,
    pub scope: usize,
    pub selected: usize,
    pub rows: Vec<SearchRow>,
    pub preview: Vec<String>,
    pub status: String,
    pub root: String,
    pub action: String,
}
#[derive(Clone, Copy, Debug)]
pub struct SearchLayout {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub scale: f32,
    pub count: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchHit {
    Close,
    Scope(usize),
    Row(usize),
    Body,
    Outside,
}
impl SearchLayout {
    pub fn new(width: f32, height: f32, top: f32, scale: f32) -> Option<Self> {
        let s = scale.max(0.1);
        let w = (width - 24. * s).min(760. * s);
        let h = (height - top - 24. * s).min(600. * s);
        if w < 260. * s || h < 290. * s {
            return None;
        }
        let count = (((h / s - 210.) / 46.).floor() as usize).clamp(1, 7);
        Some(Self {
            x: (width - w) / 2.,
            y: top + 12. * s,
            w,
            h,
            scale: s,
            count,
        })
    }
    pub fn first(&self, selected: usize) -> usize {
        selected.saturating_sub(self.count - 1)
    }
    pub fn hit(&self, x: f32, y: f32, selected: usize, len: usize) -> SearchHit {
        let x = (x - self.x) / self.scale;
        let y = (y - self.y) / self.scale;
        let w = self.w / self.scale;
        if x < 0. || y < 0. || x > w || y > self.h / self.scale {
            return SearchHit::Outside;
        }
        if x > w - 38. && y < 40. {
            return SearchHit::Close;
        }
        if (76. ..106.).contains(&y) {
            return SearchHit::Scope(
                ((x - 12.).max(0.) / ((w - 24.) / 6.)).floor().min(5.) as usize
            );
        }
        if (116. ..116. + self.count as f32 * 46.).contains(&y) {
            let i = self.first(selected) + ((y - 116.) / 46.) as usize;
            if i < len {
                return SearchHit::Row(i);
            }
        }
        SearchHit::Body
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn geometry_scales_and_limits() {
        for scale in [1., 1.5, 2.] {
            let l = SearchLayout::new(1000. * scale, 720. * scale, 20. * scale, scale).unwrap();
            assert!(l.x >= 0. && l.y + l.h <= 720. * scale);
            assert_eq!(
                l.hit(l.x + 10. * scale, l.y + 80. * scale, 0, 20),
                SearchHit::Scope(0)
            );
            assert_eq!(
                l.hit(l.x + 20. * scale, l.y + 120. * scale, 10, 20),
                SearchHit::Row(l.first(10))
            );
            assert_eq!(l.hit(0., 0., 0, 0), SearchHit::Outside);
        }
    }
    #[test]
    fn tiny_window_is_safe() {
        assert!(SearchLayout::new(100., 100., 0., 2.).is_none());
    }
}
