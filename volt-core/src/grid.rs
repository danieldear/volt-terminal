use crate::cell::Cell;

#[derive(Clone)]
pub struct Grid {
    pub cols: usize,
    pub rows: usize,
    cells: Vec<Cell>,
    row_map: Vec<usize>,
    /// Whether a visual row soft-wraps into the next row.
    /// Stored by physical row index so it naturally follows row_map rotations.
    soft_wrapped: Vec<bool>,
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
    scrollback_start: usize, // index of the oldest row in the ring
    scrollback_count: usize, // number of rows currently stored
    /// Maximum number of scrollback lines to retain.
    pub scrollback_limit: usize,
    /// Blank-row template used for fast row clears (memcpy instead of
    /// per-element fill). Rebuilt lazily when `cols` changes.
    blank_row: Vec<Cell>,
}

impl Grid {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            cols,
            rows,
            cells: vec![Cell::default(); cols * rows],
            row_map: (0..rows).collect(),
            soft_wrapped: vec![false; rows],
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
            blank_row: Vec::new(),
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

    /// Hot-path single-cell write: one `row_map` lookup covers the cell store,
    /// the dirty flag, and the soft-wrap clear that `print()` needs per char.
    #[inline]
    pub fn put_char(&mut self, col: usize, row: usize, cell: Cell) {
        let physical = self.row_map[row];
        self.soft_wrapped[physical] = false;
        self.dirty[row] = true;
        self.cells[physical * self.cols + col] = cell;
    }

    pub fn row_soft_wrapped(&self, row: usize) -> bool {
        if row >= self.rows {
            return false;
        }
        self.soft_wrapped[self.row_map[row]]
    }

    pub fn set_row_soft_wrapped(&mut self, row: usize, wrapped: bool) {
        if row >= self.rows {
            return;
        }
        let idx = self.row_map[row];
        self.soft_wrapped[idx] = wrapped;
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

    fn trim_trailing_spaces_len(cells: &[Cell]) -> usize {
        let mut end = cells.len();
        while end > 0 && cells[end - 1].c == ' ' {
            end -= 1;
        }
        end
    }

    fn reflow_visible_rows(
        old_cells: &[Cell],
        old_row_map: &[usize],
        old_soft_wrapped: &[bool],
        old_cols: usize,
        old_rows: usize,
        new_cols: usize,
        new_rows: usize,
        old_cursor_row: usize,
        old_cursor_col: usize,
        had_pending_wrap: bool,
    ) -> (Vec<Cell>, Vec<bool>, usize, usize, bool) {
        let mut new_cells = vec![Cell::default(); new_cols * new_rows];
        let mut new_soft_wrapped = vec![false; new_rows];
        if old_cols == 0 || old_rows == 0 || new_cols == 0 || new_rows == 0 {
            return (new_cells, new_soft_wrapped, 0, 0, false);
        }

        let mut visual_rows = Vec::with_capacity(old_rows);
        let mut visual_wraps = Vec::with_capacity(old_rows);
        for row in 0..old_rows {
            let physical = old_row_map[row];
            let src = physical * old_cols;
            visual_rows.push(old_cells[src..src + old_cols].to_vec());
            visual_wraps.push(old_soft_wrapped[physical]);
        }

        let mut logical_lines: Vec<Vec<Cell>> = Vec::new();
        let mut current: Vec<Cell> = Vec::new();
        let mut line_idx = 0usize;
        let mut cursor_line_idx = 0usize;
        let mut cursor_offset = 0usize;
        for row in 0..old_rows {
            if row == old_cursor_row {
                cursor_line_idx = line_idx;
                cursor_offset = current.len() + old_cursor_col + usize::from(had_pending_wrap);
            }
            current.extend_from_slice(&visual_rows[row]);
            let has_next = row + 1 < old_rows;
            let is_wrapped = has_next && visual_wraps[row];
            if !is_wrapped {
                logical_lines.push(std::mem::take(&mut current));
                line_idx += 1;
            }
        }
        if !current.is_empty() {
            logical_lines.push(current);
        }
        if logical_lines.is_empty() {
            logical_lines.push(Vec::new());
        }

        let mut virtual_rows: Vec<Vec<Cell>> = Vec::new();
        let mut virtual_wraps: Vec<bool> = Vec::new();
        let mut cursor_virtual_row = 0usize;
        let mut cursor_virtual_col = 0usize;
        let mut cursor_pending_wrap = false;

        for (idx, line) in logical_lines.iter().enumerate() {
            let line_start = virtual_rows.len();
            let mut line_len = Grid::trim_trailing_spaces_len(line);
            if idx == cursor_line_idx {
                line_len = line_len.max(cursor_offset.min(line.len()));
            }
            if line_len == 0 {
                virtual_rows.push(vec![Cell::default(); new_cols]);
                virtual_wraps.push(false);
            } else {
                let mut offset = 0usize;
                while offset < line_len {
                    let take = (line_len - offset).min(new_cols);
                    let mut row_cells = vec![Cell::default(); new_cols];
                    row_cells[..take].copy_from_slice(&line[offset..offset + take]);
                    virtual_rows.push(row_cells);
                    virtual_wraps.push(offset + take < line_len);
                    offset += take;
                }
            }

            if idx != cursor_line_idx {
                continue;
            }
            let cursor_pos = cursor_offset.min(line_len);
            let mut row_in_line = cursor_pos / new_cols;
            let mut col_in_line = cursor_pos % new_cols;
            cursor_pending_wrap = false;
            if had_pending_wrap && cursor_pos > 0 && col_in_line == 0 {
                row_in_line = row_in_line.saturating_sub(1);
                col_in_line = new_cols - 1;
                cursor_pending_wrap = true;
            }
            cursor_virtual_row = line_start + row_in_line;
            cursor_virtual_col = col_in_line.min(new_cols - 1);
        }

        if virtual_rows.is_empty() {
            virtual_rows.push(vec![Cell::default(); new_cols]);
            virtual_wraps.push(false);
        }
        if cursor_virtual_row >= virtual_rows.len() {
            cursor_virtual_row = virtual_rows.len() - 1;
            cursor_virtual_col = cursor_virtual_col.min(new_cols - 1);
            cursor_pending_wrap = false;
        }

        // Keep as much history as possible while ensuring the cursor stays visible.
        let mut window_start = cursor_virtual_row
            .saturating_add(1)
            .saturating_sub(new_rows);
        if window_start + new_rows > virtual_rows.len() {
            window_start = virtual_rows.len().saturating_sub(new_rows);
        }

        for dst_row in 0..new_rows {
            let src_row = window_start + dst_row;
            if src_row >= virtual_rows.len() {
                break;
            }
            let dst = dst_row * new_cols;
            new_cells[dst..dst + new_cols].copy_from_slice(&virtual_rows[src_row]);
            new_soft_wrapped[dst_row] = virtual_wraps[src_row];
        }

        let mut new_cursor_row = cursor_virtual_row.saturating_sub(window_start);
        if new_cursor_row >= new_rows {
            new_cursor_row = new_rows - 1;
            cursor_pending_wrap = false;
        }
        let new_cursor_col = cursor_virtual_col.min(new_cols - 1);
        (
            new_cells,
            new_soft_wrapped,
            new_cursor_col,
            new_cursor_row,
            cursor_pending_wrap,
        )
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        let old_cols = self.cols;
        let old_rows = self.rows;
        let old_cursor_col = self.cursor_col;
        let old_cursor_row = self.cursor_row;
        let had_pending_wrap = self.pending_wrap;
        let old_cells = self.cells.clone();
        let old_row_map = self.row_map.clone();
        let old_soft_wrapped = self.soft_wrapped.clone();
        let (new_cells, new_soft_wrapped, new_cursor_col, new_cursor_row, new_pending_wrap) =
            if cols != old_cols {
                Grid::reflow_visible_rows(
                    &old_cells,
                    &old_row_map,
                    &old_soft_wrapped,
                    old_cols,
                    old_rows,
                    cols,
                    rows,
                    old_cursor_row,
                    old_cursor_col,
                    had_pending_wrap,
                )
            } else {
                let mut cells = vec![Cell::default(); cols * rows];
                let mut soft_wrapped = vec![false; rows];
                let copy_cols = old_cols.min(cols);
                let copy_rows = rows.min(old_rows);
                let mut window_start = old_cursor_row.saturating_add(1).saturating_sub(copy_rows);
                if window_start + copy_rows > old_rows {
                    window_start = old_rows.saturating_sub(copy_rows);
                }
                for row in 0..copy_rows {
                    let src_row = window_start + row;
                    let old_physical = old_row_map[src_row];
                    let src = old_physical * old_cols;
                    let dst = row * cols;
                    cells[dst..dst + copy_cols].copy_from_slice(&old_cells[src..src + copy_cols]);
                    soft_wrapped[row] = old_soft_wrapped[old_physical];
                }
                let mut cursor_col = old_cursor_col.min(cols.saturating_sub(1));
                let cursor_row = old_cursor_row
                    .saturating_sub(window_start)
                    .min(rows.saturating_sub(1));
                let mut pending_wrap = had_pending_wrap;
                if had_pending_wrap {
                    let logical_next_col = old_cursor_col.saturating_add(1);
                    if logical_next_col < cols {
                        cursor_col = logical_next_col;
                        pending_wrap = false;
                    }
                }
                (cells, soft_wrapped, cursor_col, cursor_row, pending_wrap)
            };

        self.cols = cols;
        self.rows = rows;
        self.scroll_bottom = rows.saturating_sub(1);
        self.cells = new_cells;
        self.row_map = (0..rows).collect();
        self.soft_wrapped = new_soft_wrapped;
        self.cursor_col = new_cursor_col;
        self.cursor_row = new_cursor_row;
        self.pending_wrap = new_pending_wrap;
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
            let physical = self.row_map[row];
            self.clear_physical_row(physical);
            self.soft_wrapped[physical] = false;
        }
        // Mark affected region dirty.
        self.dirty[top..=bottom].fill(true);
    }

    /// Clear one physical row via memcpy from a blank-row template — measurably
    /// faster than `fill(Cell::default())`, which stores per element. Hot on
    /// every scrolled line.
    #[inline]
    fn clear_physical_row(&mut self, physical: usize) {
        if self.blank_row.len() != self.cols {
            self.blank_row = vec![Cell::default(); self.cols];
        }
        let start = physical * self.cols;
        self.cells[start..start + self.cols].copy_from_slice(&self.blank_row);
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
                    self.scrollback_buf
                        .extend_from_slice(&self.cells[row_start..row_end]);
                    self.scrollback_count += 1;
                } else {
                    // Steady state: overwrite the oldest row slot in place.
                    let dst = self.scrollback_start * self.cols;
                    self.scrollback_buf[dst..dst + self.cols]
                        .copy_from_slice(&self.cells[row_start..row_end]);
                    self.scrollback_start = (self.scrollback_start + 1) % self.scrollback_limit;
                }
            }
        }

        self.row_map[top..=bottom].rotate_left(count);
        let clear_start_row = bottom + 1 - count;
        for row in clear_start_row..=bottom {
            let physical = self.row_map[row];
            self.clear_physical_row(physical);
            self.soft_wrapped[physical] = false;
        }
        // Mark affected region dirty.
        self.dirty[top..=bottom].fill(true);
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
        if end + 1 == self.cols {
            self.soft_wrapped[self.row_map[row]] = false;
        }
        self.dirty[row] = true;
    }

    pub fn clear_screen(&mut self) {
        self.cells.fill(Cell::default());
        self.soft_wrapped.fill(false);
        self.cursor_col = 0;
        self.cursor_row = 0;
        self.mark_all_dirty();
    }

    pub fn erase_all(&mut self) {
        self.cells.fill(Cell::default());
        self.soft_wrapped.fill(false);
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

    /// Contiguous cell slice of a visual row (physical rows are contiguous).
    /// Lets renderers copy whole rows via memcpy instead of per-cell access —
    /// important because the UI blits under the performer lock.
    pub fn row_cells(&self, row: usize) -> &[Cell] {
        let start = self.row_map[row] * self.cols;
        &self.cells[start..start + self.cols]
    }

    /// Contiguous cell slice of a scrollback row (0-indexed from oldest).
    pub fn scrollback_row(&self, sb_row: usize) -> &[Cell] {
        let physical = (self.scrollback_start + sb_row) % self.scrollback_limit;
        let start = physical * self.cols;
        &self.scrollback_buf[start..start + self.cols]
    }

    /// Copy `src` into visual `row` starting at `dst_col`, clipped to the grid.
    pub fn copy_into_row(&mut self, row: usize, dst_col: usize, src: &[Cell]) {
        if row >= self.rows || dst_col >= self.cols {
            return;
        }
        let n = src.len().min(self.cols - dst_col);
        let start = self.row_map[row] * self.cols + dst_col;
        self.cells[start..start + n].copy_from_slice(&src[..n]);
        self.dirty[row] = true;
    }

    /// Discard all scrollback history (e.g. Cmd+K clear).
    pub fn clear_scrollback(&mut self) {
        self.scrollback_buf.clear();
        self.scrollback_start = 0;
        self.scrollback_count = 0;
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
