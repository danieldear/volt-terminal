use crate::cell::Cell;

#[derive(Clone)]
pub struct Grid {
    pub cols: usize,
    pub rows: usize,
    cells: Vec<Cell>,
    row_map: Vec<usize>,
    pub cursor_col: usize,
    pub cursor_row: usize,
    pub scroll_top: usize,
    pub scroll_bottom: usize,
    /// Pending-wrap (xenl): set when the last char was printed in the rightmost
    /// column. The actual line-wrap is deferred until the next `print()` call.
    /// This matches xterm / ghostty behaviour and prevents zsh PROMPT_SP from
    /// leaving a visible `%` on its own line.
    pub pending_wrap: bool,
    /// Per-row dirty flag: set when a row's content changes, cleared after render.
    /// Used by the renderer for damage tracking to skip unchanged rows.
    dirty: Vec<bool>,
}

impl Grid {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            cols,
            rows,
            cells: vec![Cell::default(); cols * rows],
            row_map: (0..rows).collect(),
            cursor_col: 0,
            cursor_row: 0,
            scroll_top: 0,
            scroll_bottom: rows.saturating_sub(1),
            pending_wrap: false,
            dirty: vec![true; rows],
        }
    }

    pub fn cell(&self, col: usize, row: usize) -> &Cell {
        let idx = self.row_map[row] * self.cols + col;
        &self.cells[idx]
    }

    pub fn cell_mut(&mut self, col: usize, row: usize) -> &mut Cell {
        self.dirty[row] = true;
        let idx = self.row_map[row] * self.cols + col;
        &mut self.cells[idx]
    }

    /// Returns a slice of per-row dirty flags. `true` means the row changed since last `clear_dirty()`.
    pub fn dirty_rows(&self) -> &[bool] {
        &self.dirty
    }

    /// Clears all dirty flags after the renderer has consumed them.
    pub fn clear_dirty(&mut self) {
        self.dirty.fill(false);
    }

    /// Marks every row dirty (e.g. after resize or full clear).
    fn mark_all_dirty(&mut self) {
        self.dirty.fill(true);
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        let mut new_cells = vec![Cell::default(); cols * rows];
        for row in 0..rows.min(self.rows) {
            for col in 0..cols.min(self.cols) {
                new_cells[row * cols + col] = *self.cell(col, row);
            }
        }
        self.cols = cols;
        self.rows = rows;
        self.scroll_bottom = rows.saturating_sub(1);
        self.cells = new_cells;
        self.row_map = (0..rows).collect();
        self.cursor_col = self.cursor_col.min(cols.saturating_sub(1));
        self.cursor_row = self.cursor_row.min(rows.saturating_sub(1));
        self.dirty = vec![true; rows];
    }

    pub fn scroll_down(&mut self, top: usize, bottom: usize, count: usize) {
        if self.cols == 0
            || self.rows == 0
            || top >= self.rows
            || bottom >= self.rows
            || top > bottom
        {
            return;
        }
        let region_rows = bottom - top + 1;
        let count = count.min(region_rows);
        if count == 0 {
            return;
        }

        self.row_map[top..=bottom].rotate_right(count);
        for row in top..(top + count) {
            let start = self.row_map[row] * self.cols;
            let end = start + self.cols;
            self.cells[start..end].fill(Cell::default());
        }
        // Mark affected region dirty.
        for row in top..=bottom {
            self.dirty[row] = true;
        }
    }

    pub fn scroll_up(&mut self, top: usize, bottom: usize, count: usize) {
        if self.cols == 0
            || self.rows == 0
            || top >= self.rows
            || bottom >= self.rows
            || top > bottom
        {
            return;
        }
        let region_rows = bottom - top + 1;
        let count = count.min(region_rows);
        if count == 0 {
            return;
        }

        self.row_map[top..=bottom].rotate_left(count);
        let clear_start_row = bottom + 1 - count;
        for row in clear_start_row..=bottom {
            let start = self.row_map[row] * self.cols;
            let end = start + self.cols;
            self.cells[start..end].fill(Cell::default());
        }
        // Mark affected region dirty.
        for row in top..=bottom {
            self.dirty[row] = true;
        }
    }

    pub fn clear_line(&mut self, row: usize, from_col: usize, to_col: usize) {
        if row >= self.rows || self.cols == 0 {
            return;
        }
        let end = to_col.min(self.cols.saturating_sub(1));
        if from_col > end {
            return;
        }
        let row_start = self.row_map[row] * self.cols;
        for col in from_col..=end {
            self.cells[row_start + col] = Cell::default();
        }
        self.dirty[row] = true;
    }

    pub fn clear_screen(&mut self) {
        self.cells.fill(Cell::default());
        self.cursor_col = 0;
        self.cursor_row = 0;
        self.mark_all_dirty();
    }

    pub fn erase_all(&mut self) {
        self.cells.fill(Cell::default());
        self.mark_all_dirty();
    }

    pub fn advance_cursor(&mut self) {
        if self.cursor_col + 1 >= self.cols {
            // Stay at the last column; the wrap fires on the next print() call.
            self.pending_wrap = true;
        } else {
            self.cursor_col += 1;
        }
    }

    pub fn newline(&mut self) {
        if self.cursor_row == self.scroll_bottom {
            self.scroll_up(self.scroll_top, self.scroll_bottom, 1);
        } else if self.cursor_row < self.rows.saturating_sub(1) {
            self.cursor_row += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_grid_is_blank() {
        let g = Grid::new(80, 24);
        assert_eq!(g.cols, 80);
        assert_eq!(g.rows, 24);
        assert_eq!(g.cell(0, 0).c, ' ');
        assert_eq!(g.cursor_col, 0);
        assert_eq!(g.cursor_row, 0);
    }

    #[test]
    fn test_write_and_read_cell() {
        let mut g = Grid::new(80, 24);
        g.cell_mut(5, 3).c = 'A';
        assert_eq!(g.cell(5, 3).c, 'A');
    }

    #[test]
    fn test_resize_preserves_content() {
        let mut g = Grid::new(80, 24);
        g.cell_mut(2, 1).c = 'X';
        g.resize(100, 30);
        assert_eq!(g.cell(2, 1).c, 'X');
        assert_eq!(g.cols, 100);
        assert_eq!(g.rows, 30);
    }

    #[test]
    fn test_scroll_up_moves_content() {
        let mut g = Grid::new(80, 24);
        g.cell_mut(0, 1).c = 'A';
        g.scroll_up(0, 23, 1);
        assert_eq!(g.cell(0, 0).c, 'A');
        assert_eq!(g.cell(0, 23).c, ' ');
    }

    #[test]
    fn test_advance_cursor_sets_pending_wrap_at_last_col() {
        // xenl (pending wrap): advance at last col sets the flag but doesn't move yet.
        let mut g = Grid::new(4, 4);
        g.cursor_col = 3;
        g.cursor_row = 0;
        g.advance_cursor();
        assert!(g.pending_wrap);
        assert_eq!(g.cursor_col, 3); // stays at last column
        assert_eq!(g.cursor_row, 0); // row unchanged until next print
    }

    #[test]
    fn test_newline_scrolls_at_bottom() {
        let mut g = Grid::new(80, 4);
        g.cell_mut(0, 0).c = 'A';
        g.cursor_row = 3; // scroll_bottom
        g.newline();
        assert_eq!(g.cell(0, 0).c, ' ');
        assert_eq!(g.cursor_row, 3); // stays at bottom
    }

    #[test]
    fn test_scroll_down_moves_content() {
        let mut g = Grid::new(80, 24);
        g.cell_mut(0, 0).c = 'A';
        g.scroll_down(0, 23, 1);
        assert_eq!(g.cell(0, 1).c, 'A');
        assert_eq!(g.cell(0, 0).c, ' ');
    }

    #[test]
    fn test_clear_line() {
        let mut g = Grid::new(80, 24);
        for col in 0..5 {
            g.cell_mut(col, 0).c = 'X';
        }
        g.clear_line(0, 0, 79);
        assert_eq!(g.cell(0, 0).c, ' ');
        assert_eq!(g.cell(4, 0).c, ' ');
    }
}
