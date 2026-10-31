//! A selection on the screen and its history, in absolute rows, so it
//! survives new output. Moved from Volt Terminal's UI to share it.
use crate::grid::Grid;

#[derive(Debug, Clone, Copy)]
pub enum SelectionMode {
    Linear,
    Block,
}

#[derive(Debug, Clone, Copy)]
pub struct Selection {
    pub pane_id: usize,
    /// Prevent a main-screen selection from highlighting unrelated alt-screen
    /// cells (and vice versa) when a TUI switches screens.
    pub alternate_screen: bool,
    pub mode: SelectionMode,
    pub start_col: usize,
    /// Absolute history + live row number, not a viewport coordinate.
    pub start_row: usize,
    pub end_col: usize,
    pub end_row: usize,
}

impl Selection {
    pub fn at_viewport(
        pane_id: usize,
        alternate_screen: bool,
        mode: SelectionMode,
        grid: &Grid,
        offset: usize,
        col: usize,
        row: usize,
    ) -> Option<Self> {
        let row = grid.viewport_absolute_row(offset, row)?;
        Some(Self {
            pane_id,
            alternate_screen,
            mode,
            start_col: col,
            start_row: row,
            end_col: col,
            end_row: row,
        })
    }

    /// Shift-click and drag keep the original anchor, even after scrolling.
    pub fn extend_to(
        &mut self,
        pane_id: usize,
        alternate_screen: bool,
        grid: &Grid,
        offset: usize,
        col: usize,
        row: usize,
    ) -> bool {
        let origin = grid.scrollback_origin();
        let retained_end = origin.saturating_add(grid.scrollback_len() + grid.rows);
        if self.pane_id != pane_id
            || self.alternate_screen != alternate_screen
            || self.start_row < origin
            || self.start_row >= retained_end
        {
            return false;
        }
        let Some(end_row) = grid.viewport_absolute_row(offset, row) else {
            return false;
        };
        self.end_col = col;
        self.end_row = end_row;
        true
    }

    /// Clip absolute selection endpoints to the visible pane. For a linear
    /// selection, a clipped first/last row occupies the entire visible row.
    pub fn visible_range(
        self,
        grid: &Grid,
        offset: usize,
        pane_rows: usize,
    ) -> Option<((usize, usize), (usize, usize))> {
        let sel = self.normalized();
        let origin = grid.scrollback_origin();
        let last = origin.checked_add(grid.scrollback_len() + grid.rows - 1)?;
        if sel.start_row < origin || sel.end_row > last || pane_rows == 0 {
            return None;
        }
        let first_visible = grid.viewport_absolute_row(offset, 0)?;
        let last_visible = first_visible.checked_add(pane_rows - 1)?;
        let first = sel.start_row.max(first_visible);
        let last = sel.end_row.min(last_visible);
        if first > last {
            return None;
        }
        let (start_col, end_col) = match sel.mode {
            SelectionMode::Linear => (
                if first > sel.start_row {
                    0
                } else {
                    sel.start_col
                },
                if last < sel.end_row {
                    grid.cols - 1
                } else {
                    sel.end_col
                },
            ),
            SelectionMode::Block => (sel.start_col, sel.end_col),
        };
        Some((
            (start_col, first - first_visible),
            (end_col, last - first_visible),
        ))
    }

    /// Copy the selected rows from retained history, not just the viewport.
    pub fn text(self, grid: &Grid) -> Option<String> {
        let sel = self.normalized();
        let origin = grid.scrollback_origin();
        let history = grid.scrollback_len();
        let total = history + grid.rows;
        let start = sel.start_row.checked_sub(origin)?;
        let end = sel.end_row.checked_sub(origin)?;
        if start >= total || end >= total {
            return None;
        }
        let mut out = String::new();
        for row in start..=end {
            let (start_col, end_col) = match sel.mode {
                SelectionMode::Block => (
                    sel.start_col.min(grid.cols - 1),
                    sel.end_col.min(grid.cols - 1),
                ),
                SelectionMode::Linear => (
                    if row == start { sel.start_col } else { 0 },
                    if row == end {
                        sel.end_col.min(grid.cols - 1)
                    } else {
                        grid.cols - 1
                    },
                ),
            };
            let cells = if row < history {
                grid.scrollback_row(row)
            } else {
                grid.row_cells(row - history)
            };
            let mut line = String::new();
            if start_col <= end_col && start_col < grid.cols {
                for cell in &cells[start_col..=end_col] {
                    grid.push_cell_text(&mut line, cell);
                }
            }
            let soft_wrapped = if row < history {
                grid.scrollback_row_soft_wrapped(row)
            } else {
                grid.row_soft_wrapped(row - history)
            };
            if matches!(sel.mode, SelectionMode::Block) {
                out.push_str(&line);
            } else {
                // Spaces at a soft-wrap boundary are text, not padding.
                if soft_wrapped && row != end {
                    out.push_str(&line);
                } else {
                    out.push_str(line.trim_end_matches(' '));
                }
            }
            if row != end && (matches!(sel.mode, SelectionMode::Block) || !soft_wrapped) {
                out.push('\n');
            }
        }
        Some(out)
    }

    pub fn normalized(self) -> Self {
        match self.mode {
            SelectionMode::Linear => {
                if (self.start_row, self.start_col) <= (self.end_row, self.end_col) {
                    self
                } else {
                    Self {
                        pane_id: self.pane_id,
                        alternate_screen: self.alternate_screen,
                        mode: self.mode,
                        start_col: self.end_col,
                        start_row: self.end_row,
                        end_col: self.start_col,
                        end_row: self.start_row,
                    }
                }
            }
            SelectionMode::Block => Self {
                pane_id: self.pane_id,
                alternate_screen: self.alternate_screen,
                mode: self.mode,
                start_col: self.start_col.min(self.end_col),
                start_row: self.start_row.min(self.end_row),
                end_col: self.start_col.max(self.end_col),
                end_row: self.start_row.max(self.end_row),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill_row(grid: &mut Grid, row: usize, ch: char) {
        for col in 0..grid.cols {
            let mut cell = crate::cell::Cell::default();
            cell.set_char(ch);
            grid.put_char(col, row, cell);
        }
    }

    #[test]
    fn multiline_selection_survives_repeated_scroll_and_copies_offscreen_rows() {
        let mut grid = Grid::new(4, 3);
        grid.set_scrollback_limit(3);
        for (row, ch) in ['a', 'b', 'c'].into_iter().enumerate() {
            fill_row(&mut grid, row, ch);
        }
        for ch in ['d', 'e', 'f'] {
            grid.scroll_up(0, 2, 1);
            fill_row(&mut grid, 2, ch);
        }
        let mut sel =
            Selection::at_viewport(7, false, SelectionMode::Linear, &grid, 3, 1, 0).unwrap();
        assert!(sel.extend_to(7, false, &grid, 0, 1, 1));
        assert_eq!(
            sel.text(&grid).as_deref(),
            Some("aaa\nbbbb\ncccc\ndddd\nee")
        );
        assert_eq!(sel.visible_range(&grid, 3, 3), Some(((1, 0), (3, 2))));
        assert_eq!(sel.visible_range(&grid, 2, 3), Some(((0, 0), (3, 2))));
        assert_eq!(sel.visible_range(&grid, 0, 3), Some(((0, 0), (1, 1))));
        assert_eq!(
            sel.text(&grid).as_deref(),
            Some("aaa\nbbbb\ncccc\ndddd\nee")
        );

        grid.scroll_up(0, 2, 1); // Evicts the selected anchor, never copy wrong text.
        assert!(sel.text(&grid).is_none());
        assert!(sel.visible_range(&grid, 3, 3).is_none());
    }

    #[test]
    fn shift_extension_keeps_anchor_and_rejects_other_panes() {
        let mut grid = Grid::new(4, 3);
        grid.scroll_up(0, 2, 1);
        grid.scroll_up(0, 2, 1);
        let mut sel =
            Selection::at_viewport(1, false, SelectionMode::Linear, &grid, 0, 2, 1).unwrap();
        let anchor = (sel.start_col, sel.start_row);
        assert!(!sel.extend_to(2, false, &grid, 2, 0, 0));
        assert!(!sel.extend_to(1, true, &grid, 2, 0, 0));
        assert_eq!((sel.start_col, sel.start_row), anchor);
        assert!(sel.extend_to(1, false, &grid, 2, 0, 0));
        assert_eq!((sel.start_col, sel.start_row), anchor);
        assert_eq!(sel.normalized().start_row, 0);
    }

    #[test]
    fn block_selection_keeps_columns_and_survives_new_output() {
        let mut grid = Grid::new(4, 2);
        grid.set_scrollback_limit(3);
        fill_row(&mut grid, 0, 'a');
        fill_row(&mut grid, 1, 'b');
        grid.scroll_up(0, 1, 1);
        fill_row(&mut grid, 1, 'c');
        let mut sel =
            Selection::at_viewport(1, false, SelectionMode::Block, &grid, 1, 1, 0).unwrap();
        assert!(sel.extend_to(1, false, &grid, 0, 2, 1));
        assert_eq!(sel.text(&grid).as_deref(), Some("aa\nbb\ncc"));
        assert_eq!(sel.visible_range(&grid, 1, 2), Some(((1, 0), (2, 1))));
        grid.scroll_up(0, 1, 1);
        fill_row(&mut grid, 1, 'd');
        assert_eq!(sel.text(&grid).as_deref(), Some("aa\nbb\ncc"));
        assert_eq!(sel.visible_range(&grid, 2, 2), Some(((1, 0), (2, 1))));
    }
}
