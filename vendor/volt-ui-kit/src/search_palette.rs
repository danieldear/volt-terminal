//! Workspace search presentation and shared pointer geometry. The panel
//! floats over the terminal and never reserves cells. Searching and ranking
//! live in volt-ui; this is only what gets drawn and where clicks land.

/// Scope tabs, in the order the search engine indexes them.
pub const SCOPES: [&str; 6] = ["All", "Terminal", "Files", "Text", "Git", "Tasks"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchRow {
    /// Short label for the result type, e.g. "File" or "Branch".
    pub kind: String,
    pub title: String,
    pub detail: String,
    /// Matched characters in `title`, as half-open char ranges.
    pub hits: Vec<(usize, usize)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchView {
    pub query: String,
    pub query_selected: bool,
    pub cursor: usize,
    /// Index into `SCOPES`.
    pub scope: usize,
    pub selected: usize,
    pub rows: Vec<SearchRow>,
    pub status: String,
    /// What Enter does for the selected result, shown at the right of the footer.
    pub hint: String,
    /// Project the search runs in.
    pub root: String,
}

// Logical units; multiplied by the scale factor.
const MAX_W: f32 = 640.;
const MARGIN: f32 = 12.;
/// Gap above the panel when the window has room, like a command palette.
const TOP_GAP: f32 = 56.;
pub(crate) const QUERY_H: f32 = 56.;
pub(crate) const SCOPES_H: f32 = 44.;
pub(crate) const PILL_H: f32 = 28.;
pub(crate) const PILL_TEXT: f32 = 13.;
pub(crate) const ROW_H: f32 = 54.;
pub(crate) const FOOTER_H: f32 = 38.;
const LIST_PAD: f32 = 6.;
const MAX_ROWS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SearchLayout {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub scale: f32,
    /// Result rows shown at once; the list scrolls to keep the selection in view.
    pub count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchHit {
    Scope(usize),
    Row(usize),
    Body,
    Outside,
}

impl SearchLayout {
    /// The panel grows with its results, up to `MAX_ROWS` or the window
    /// height. `None` when even one row doesn't fit; the caller then closes
    /// search instead of leaving an invisible modal.
    pub fn new(width: f32, height: f32, top: f32, scale: f32, results: usize) -> Option<Self> {
        let s = scale.max(0.1);
        let w = (width - 2. * MARGIN * s).min(MAX_W * s);
        let chrome = Self::list_top() + 2. * LIST_PAD + FOOTER_H;
        let room = (height - top) / s - 2. * MARGIN;
        let fit = ((room - chrome) / ROW_H).floor();
        if w < 400. * s || fit < 1. {
            return None;
        }
        let count = results.clamp(1, MAX_ROWS).min(fit as usize);
        let h = chrome + count as f32 * ROW_H;
        // Sit a little below the top when there's room, never off-screen.
        let gap = TOP_GAP.min(room - h).max(MARGIN);
        Some(Self {
            x: (width - w) / 2.,
            y: top + gap * s,
            w,
            h: h * s,
            scale: s,
            count,
        })
    }

    fn list_top() -> f32 {
        QUERY_H + 1. + SCOPES_H + 1.
    }

    /// Physical `[x, y, w, h]` from panel-relative logical units.
    fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
        let s = self.scale;
        [self.x + x * s, self.y + y * s, w * s, h * s]
    }

    fn logical_w(&self) -> f32 {
        self.w / self.scale
    }

    pub fn query_row(&self) -> [f32; 4] {
        self.rect(0., 0., self.logical_w(), QUERY_H)
    }

    pub fn scope_pill(&self, i: usize) -> [f32; 4] {
        let width = |label: &str| label.chars().count() as f32 * PILL_TEXT * 0.6 + 22.;
        let x = 12. + SCOPES[..i].iter().map(|l| width(l) + 4.).sum::<f32>();
        self.rect(x, QUERY_H + 1. + 8., width(SCOPES[i]), PILL_H)
    }

    pub fn list(&self) -> [f32; 4] {
        let top = Self::list_top();
        self.rect(
            0.,
            top,
            self.logical_w(),
            self.h / self.scale - top - FOOTER_H,
        )
    }

    pub fn row(&self, slot: usize) -> [f32; 4] {
        self.rect(
            0.,
            Self::list_top() + LIST_PAD + slot as f32 * ROW_H,
            self.logical_w(),
            ROW_H,
        )
    }

    pub fn footer(&self) -> [f32; 4] {
        self.rect(
            0.,
            self.h / self.scale - FOOTER_H,
            self.logical_w(),
            FOOTER_H,
        )
    }

    /// First visible row, keeping `selected` in view.
    pub fn first(&self, selected: usize) -> usize {
        selected.saturating_sub(self.count - 1)
    }

    pub fn hit(&self, x: f32, y: f32, selected: usize, len: usize) -> SearchHit {
        let inside = |r: [f32; 4]| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
        if !inside([self.x, self.y, self.w, self.h]) {
            return SearchHit::Outside;
        }
        if let Some(i) = (0..SCOPES.len()).find(|&i| inside(self.scope_pill(i))) {
            return SearchHit::Scope(i);
        }
        let first = self.first(selected);
        if let Some(slot) = (0..self.count).find(|&slot| inside(self.row(slot))) {
            if first + slot < len {
                return SearchHit::Row(first + slot);
            }
        }
        SearchHit::Body
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
            let l = SearchLayout::new(1200. * scale, 800. * scale, 30. * scale, scale, 30).unwrap();
            assert!(l.x >= 0. && l.y + l.h <= 800. * scale);
            for i in 0..SCOPES.len() {
                let (x, y) = center(l.scope_pill(i));
                assert_eq!(l.hit(x, y, 0, 0), SearchHit::Scope(i));
            }
            let (x, y) = center(l.row(0));
            assert_eq!(l.hit(x, y, 0, 3), SearchHit::Row(0));
            // Scrolled: slot 0 shows the first visible row.
            assert_eq!(l.hit(x, y, 20, 30), SearchHit::Row(l.first(20)));
            // Empty slots are panel body, not rows.
            assert_eq!(l.hit(x, y, 0, 0), SearchHit::Body);
            assert_eq!(l.hit(l.x - 1., l.y + 10., 0, 0), SearchHit::Outside);
        }
    }

    #[test]
    fn the_panel_grows_with_its_results_up_to_a_limit() {
        let one = SearchLayout::new(1200., 800., 0., 1., 0).unwrap();
        let four = SearchLayout::new(1200., 800., 0., 1., 4).unwrap();
        let many = SearchLayout::new(1200., 800., 0., 1., 300).unwrap();
        assert_eq!((one.count, four.count, many.count), (1, 4, MAX_ROWS));
        assert!(one.h < four.h && four.h < many.h);
        assert!((four.h - one.h - 3. * ROW_H).abs() < 0.01);
        assert!((four.w - MAX_W).abs() < 0.01, "compact, not window-wide");
        let last = four.row(3);
        assert!(last[1] + last[3] <= four.footer()[1]);
        let pill = four.scope_pill(SCOPES.len() - 1);
        assert!(pill[0] + pill[2] < four.x + four.w);
    }

    #[test]
    fn short_and_narrow_windows_are_handled() {
        // A short window shows fewer rows and keeps the panel on screen.
        let short = SearchLayout::new(1200., 360., 0., 1., 300).unwrap();
        assert!(short.count >= 1 && short.count < MAX_ROWS);
        assert!(short.y + short.h <= 360.);
        assert!(SearchLayout::new(300., 700., 0., 1., 3).is_none());
        assert!(SearchLayout::new(1200., 200., 0., 1., 3).is_none());
    }
}
