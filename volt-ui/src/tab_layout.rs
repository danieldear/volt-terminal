/// Tab bar geometry and hit-testing.
///
/// `TabLayout` is cheap to compute for the current window and tab count;
/// pointer handlers use it for hit-testing and drag targets.
pub struct TabLayout {
    pub sc: f32,
    pub left_pad: f32,
    pub tab_w: f32,
    pub tab_gap: f32,
    pub tab_y: f32,
    pub tab_h: f32,
    pub close_w: f32,
    pub plus_x: f32,
    pub plus_w: f32,
}

impl TabLayout {
    pub fn compute(sw: f32, tab_bar_h: f32, n_tabs: usize, sc: f32) -> Self {
        let left_pad = (78.0 * sc).round();
        let tab_gap = (4.0 * sc).round().max(2.0);
        let plus_w = (24.0 * sc).round().max(20.0);
        let right_pad = (10.0 * sc).round();
        let plus_x = (sw - right_pad - plus_w).max(left_pad + plus_w);
        let tab_area_w = (plus_x - left_pad - 10.0 * sc).max(80.0 * sc);
        let n = n_tabs.max(1);
        let tab_w = ((tab_area_w - tab_gap * (n.saturating_sub(1)) as f32) / n as f32)
            .min(220.0 * sc)
            .max(100.0 * sc);
        let tab_h = (tab_bar_h - 8.0 * sc).max(24.0 * sc);
        let tab_y = ((tab_bar_h - tab_h) * 0.5).round().max(2.0 * sc);
        TabLayout {
            sc,
            left_pad,
            tab_w,
            tab_gap,
            tab_y,
            tab_h,
            close_w: 16.0 * sc,
            plus_x,
            plus_w,
        }
    }

    pub fn tab_x(&self, i: usize) -> f32 {
        self.left_pad + i as f32 * (self.tab_w + self.tab_gap)
    }

    pub fn close_rect(&self, i: usize) -> (f32, f32, f32, f32) {
        let close_box_w = self.close_w + 6.0 * self.sc;
        let close_box_x = self.tab_x(i) + self.tab_w - close_box_w - 5.0 * self.sc;
        let close_box_y = self.tab_y + (1.0 * self.sc).max(1.0);
        let close_box_h = (self.tab_h - 2.0 * self.sc).max(18.0 * self.sc);
        (close_box_x, close_box_y, close_box_w, close_box_h)
    }

    pub fn plus_rect(&self) -> (f32, f32, f32, f32) {
        let plus_box_x = self.plus_x - 2.0 * self.sc;
        let plus_box_y = self.tab_y + (1.0 * self.sc).max(1.0);
        let plus_box_w = self.plus_w + 4.0 * self.sc;
        let plus_box_h = (self.tab_h - 2.0 * self.sc).max(18.0 * self.sc);
        (plus_box_x, plus_box_y, plus_box_w, plus_box_h)
    }

    pub fn hit_tab(&self, mx: f32, my: f32, n: usize) -> Option<usize> {
        if my < self.tab_y || my >= self.tab_y + self.tab_h {
            return None;
        }
        for i in 0..n {
            let tx = self.tab_x(i);
            if mx >= tx && mx < tx + self.tab_w {
                return Some(i);
            }
        }
        None
    }

    pub fn hit_close(&self, mx: f32, my: f32, i: usize) -> bool {
        let (x, y, w, h) = self.close_rect(i);
        mx >= x && mx < x + w && my >= y && my < y + h
    }

    pub fn hit_plus(&self, mx: f32, my: f32) -> bool {
        let (x, y, w, h) = self.plus_rect();
        mx >= x && mx < x + w && my >= y && my < y + h
    }

    /// Move only after the pointer crosses a neighboring tab's center. This
    /// keeps a dragged tab stable in its own slot, including over the gaps,
    /// while allowing a single motion to cross several tabs or window edges.
    pub fn reorder_target(&self, mx: f32, current: usize, count: usize) -> Option<usize> {
        if current >= count {
            return None;
        }
        let mut target = current;
        while target + 1 < count && mx >= self.tab_x(target + 1) + self.tab_w * 0.5 {
            target += 1;
        }
        while target > 0 && mx <= self.tab_x(target - 1) + self.tab_w * 0.5 {
            target -= 1;
        }
        Some(target)
    }
}

#[cfg(test)]
mod tests {
    use super::TabLayout;

    #[test]
    fn drag_crosses_neighbor_centers_and_clamps_at_edges() {
        let layout = TabLayout::compute(900.0, 36.0, 4, 1.0);
        let center = |i| layout.tab_x(i) + layout.tab_w * 0.5;
        assert_eq!(layout.reorder_target(center(1) - 1.0, 0, 4), Some(0));
        assert_eq!(layout.reorder_target(center(1), 0, 4), Some(1));
        assert_eq!(layout.reorder_target(center(3) + 500.0, 0, 4), Some(3));
        assert_eq!(layout.reorder_target(center(2) + 1.0, 3, 4), Some(3));
        assert_eq!(layout.reorder_target(center(2), 3, 4), Some(2));
        assert_eq!(layout.reorder_target(-500.0, 3, 4), Some(0));
        assert_eq!(layout.reorder_target(center(0), 4, 4), None);
    }
}
