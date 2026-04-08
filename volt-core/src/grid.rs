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
    /// Flat ring buffer for scrollback history.
    /// Stored as contiguous cells: row i starts at physical_row(i) * cols.
    /// Growing phase: buf grows by `cols` cells per line until capacity is reached.
    /// Steady state: oldest row slot is overwritten in place — zero allocations.
    scrollback_buf: Vec<Cell>,
    scrollback_start: usize,   // index of the oldest row in the ring
    scrollback_count: usize,   // number of rows currently stored
    /// Maximum number of scrollback lines to retain.
    pub scrollback_limit: usize,
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
            scrollback_buf: Vec::new(),
            scrollback_start: 0,
            scrollback_count: 0,
            scrollback_limit: 10_000,
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
        let old_cols = self.cols;
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
        // Migrate scrollback to the new column width.
        // Row-only resizes leave the buffer intact; col changes rewrite it.
        if old_cols != cols {
            self.reflow_scrollback(old_cols, cols);
        }
    }

    /// Rewrite the flat scrollback ring buffer to use `new_cols` as the stride.
    /// Rows are truncated (narrowing) or right-padded with blanks (widening).
    /// After this call the buffer is sequential (scrollback_start = 0).
    fn reflow_scrollback(&mut self, old_cols: usize, new_cols: usize) {
        if self.scrollback_count == 0 {
            return;
        }
        let count = self.scrollback_count;
        let copy_cols = old_cols.min(new_cols);
        let mut new_buf = vec![Cell::default(); count * new_cols];
        for i in 0..count {
            let physical = (self.scrollback_start + i) % self.scrollback_limit;
            let src = physical * old_cols;
            let dst = i * new_cols;
            new_buf[dst..dst + copy_cols]
                .copy_from_slice(&self.scrollback_buf[src..src + copy_cols]);
        }
        self.scrollback_buf = new_buf;
        self.scrollback_start = 0;
        // scrollback_count is unchanged; limit stays the same
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

        // Save rows scrolling off the top into the scrollback buffer.
        // Only do this for full-screen scrolls (top == 0) so that partial
        // scroll regions used by apps like vim don't pollute history.
        if top == 0 && self.scrollback_limit > 0 && self.cols > 0 {
            for i in 0..count {
                let row_start = self.row_map[i] * self.cols;
                let row_end = row_start + self.cols;
                if self.scrollback_count < self.scrollback_limit {
                    // Growing phase: extend the flat buffer by one row.
                    // Vec doubling means amortised O(1); once full, no more allocs.
                    self.scrollback_buf.extend_from_slice(&self.cells[row_start..row_end]);
                    self.scrollback_count += 1;
                } else {
                    // Steady state: overwrite the oldest row slot in place.
                    let dst = self.scrollback_start * self.cols;
                    self.scrollback_buf[dst..dst + self.cols]
                        .copy_from_slice(&self.cells[row_start..row_end]);
                    self.scrollback_start =
                        (self.scrollback_start + 1) % self.scrollback_limit;
                }
            }
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

    /// Number of lines stored in the scrollback buffer.
    pub fn scrollback_len(&self) -> usize {
        self.scrollback_count
    }

    /// Returns the cell at `col` in a scrollback row.
    /// `sb_row` is 0-indexed from the oldest line.
    pub fn scrollback_cell(&self, sb_row: usize, col: usize) -> &Cell {
        let physical = (self.scrollback_start + sb_row) % self.scrollback_limit;
        &self.scrollback_buf[physical * self.cols + col]
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
