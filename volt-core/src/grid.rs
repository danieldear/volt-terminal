use crate::cell::{Cell, CellColor};
use std::{collections::HashMap, sync::Arc};

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
    scrollback_wrapped: Vec<bool>,
    history_enabled: bool,
    scrollback_start: usize, // index of the oldest row in the ring
    scrollback_count: usize, // number of rows currently stored
    /// Maximum number of scrollback lines to retain.
    pub scrollback_limit: usize,
    /// Blank-row template used for fast row clears (memcpy instead of
    /// per-element fill). Rebuilt lazily when `cols` changes.
    blank_row: Vec<Cell>,
    pub(crate) erase_cell: Cell,
    // Only extended graphemes allocate. IDs travel with Copy cells through
    // row rotation/reflow; cross-grid blits remap them explicitly.
    has_wide: bool,
    graphemes: Vec<Arc<str>>,
    grapheme_ids: HashMap<Arc<str>, usize>,
}

struct ReflowSource<'a> {
    cells: &'a [Cell],
    row_map: &'a [usize],
    soft_wrapped: &'a [bool],
    cols: usize,
    rows: usize,
}

struct ReflowResult {
    cells: Vec<Cell>,
    wrapped: Vec<bool>,
    cursor_col: usize,
    cursor_row: usize,
    pending_wrap: bool,
    history: Vec<Cell>,
    history_wrapped: Vec<bool>,
}

struct ReflowTarget {
    cols: usize,
    rows: usize,
}

struct CursorSnapshot {
    row: usize,
    col: usize,
    had_pending_wrap: bool,
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
            scrollback_wrapped: Vec::new(),
            history_enabled: true,
            scrollback_start: 0,
            scrollback_count: 0,
            scrollback_limit: 10_000,
            blank_row: Vec::new(),
            erase_cell: Cell::default(),
            has_wide: false,
            graphemes: Vec::new(),
            grapheme_ids: HashMap::new(),
        }
    }

    pub(crate) fn new_alt(cols: usize, rows: usize) -> Self {
        let mut grid = Self::new(cols, rows);
        grid.history_enabled = false;
        grid
    }

    pub fn scrollback_row_soft_wrapped(&self, row: usize) -> bool {
        self.scrollback_wrapped[(self.scrollback_start + row) % self.scrollback_limit]
    }

    pub(crate) fn set_erase_background(&mut self, bg: CellColor) {
        if self.erase_cell.bg != bg {
            self.erase_cell.bg = bg;
            self.blank_row.clear();
        }
    }

    pub fn set_scrollback_limit(&mut self, limit: usize) {
        let limit = limit.max(1);
        if limit == self.scrollback_limit {
            return;
        }
        let skip = self.scrollback_count.saturating_sub(limit);
        let mut cells = Vec::with_capacity((self.scrollback_count - skip) * self.cols);
        let mut wrapped = Vec::with_capacity(self.scrollback_count - skip);
        for row in skip..self.scrollback_count {
            cells.extend_from_slice(self.scrollback_row(row));
            wrapped.push(self.scrollback_row_soft_wrapped(row));
        }
        self.scrollback_count -= skip;
        self.scrollback_start = 0;
        self.scrollback_buf = cells;
        self.scrollback_wrapped = wrapped;
        self.scrollback_limit = limit;
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

    /// Keep the overwhelmingly common ASCII-only path free of wide-cell
    /// boundary reads. Once a wide cell is present, use the repairing writer.
    #[inline(always)]
    pub fn put_ascii(&mut self, col: usize, row: usize, cell: Cell) {
        if self.has_wide {
            self.put_char(col, row, cell);
            return;
        }
        let physical = self.row_map[row];
        self.soft_wrapped[physical] = false;
        self.dirty[row] = true;
        self.cells[physical * self.cols + col] = cell;
    }

    /// Hot-path single-cell write: one `row_map` lookup covers the cell store,
    /// the dirty flag, and the soft-wrap clear that `print()` needs per char.
    #[inline]
    pub fn put_char(&mut self, col: usize, row: usize, cell: Cell) {
        let physical = self.row_map[row];
        self.soft_wrapped[physical] = false;
        self.dirty[row] = true;
        let index = physical * self.cols + col;
        let old = self.cells[index];
        if old.is_continuation() && col > 0 {
            self.cells[index - 1] = Cell::default();
        } else if old.is_wide() && col + 1 < self.cols {
            self.cells[index + 1] = Cell::default();
        }
        self.cells[index] = cell;
        if cell.is_wide() && col + 1 < self.cols {
            self.has_wide = true;
            if self.cells[index + 1].is_wide() && col + 2 < self.cols {
                self.cells[index + 2] = Cell::default();
            }
            self.cells[index + 1] = cell.continuation();
        }
    }

    pub fn has_extended_text(&self) -> bool {
        !self.graphemes.is_empty()
    }

    pub fn extended_text(&self, cell: &Cell) -> Option<&str> {
        cell.cluster_id()
            .and_then(|id| self.graphemes.get(id))
            .map(|s| s.as_ref())
    }

    pub fn cell_char(&self, cell: &Cell) -> char {
        self.extended_text(cell)
            .and_then(|s| s.chars().next())
            .unwrap_or_else(|| cell.c())
    }

    pub fn push_cell_text(&self, text: &mut String, cell: &Cell) {
        if cell.is_continuation() || cell.is_wrap_spacer() {
            return;
        }
        if let Some(cluster) = self.extended_text(cell) {
            text.push_str(cluster);
        } else {
            text.push(cell.c());
        }
    }

    pub fn row_text(&self, cells: &[Cell]) -> String {
        let mut text = String::new();
        for cell in cells {
            self.push_cell_text(&mut text, cell);
        }
        text
    }

    fn intern_grapheme(&mut self, text: &str) -> usize {
        if let Some(&id) = self.grapheme_ids.get(text) {
            return id;
        }
        let text: Arc<str> = Arc::from(text);
        let id = self.graphemes.len();
        self.graphemes.push(Arc::clone(&text));
        self.grapheme_ids.insert(text, id);
        id
    }

    fn compact_graphemes(&mut self) {
        let mut ids = HashMap::new();
        let mut texts = Vec::new();
        for cell in self.cells.iter_mut().chain(self.scrollback_buf.iter_mut()) {
            if let Some(old) = cell.cluster_id() {
                let text = &self.graphemes[old];
                let id = *ids.entry(Arc::clone(text)).or_insert_with(|| {
                    texts.push(Arc::clone(text));
                    texts.len() - 1
                });
                cell.set_cluster(id);
            }
        }
        self.graphemes = texts;
        self.grapheme_ids = ids;
    }

    pub fn set_grapheme(&mut self, col: usize, row: usize, text: &str, width: usize) {
        // Reclaim historical/intermediate clusters once the pool exceeds a
        // multiple of the number of cells it could possibly serve. No cost on
        // the ASCII path, and old strings cannot grow forever in a long session.
        if self.graphemes.len() > 1024 + 2 * (self.cells.len() + self.scrollback_buf.len()) {
            self.compact_graphemes();
        }
        let id = self.intern_grapheme(text);
        let mut cell = *self.cell(col, row);
        cell.set_cluster(id);
        cell.set_wide(width == 2);
        let wrapped = self.row_soft_wrapped(row);
        self.put_char(col, row, cell);
        self.set_row_soft_wrapped(row, wrapped);
    }

    /// Import a row from another grid without mixing up grid-local text IDs.
    /// The common ASCII path remains exactly one memcpy.
    #[inline(always)]
    pub fn copy_from_grid(&mut self, row: usize, col: usize, source: &Grid, cells: &[Cell]) {
        self.has_wide |= source.has_wide;
        if source.graphemes.is_empty() {
            self.copy_into_row(row, col, cells);
            return;
        }
        self.copy_extended_row(row, col, source, cells);
    }

    #[inline(never)]
    fn copy_extended_row(&mut self, row: usize, col: usize, source: &Grid, cells: &[Cell]) {
        if self.graphemes.len() > 1024 + 2 * (self.cells.len() + self.scrollback_buf.len()) {
            self.compact_graphemes();
        }
        let mut imported = cells.to_vec();
        for cell in &mut imported {
            if let Some(text) = source.extended_text(cell) {
                cell.set_cluster(self.intern_grapheme(text));
            }
        }
        self.copy_into_row(row, col, &imported);
    }

    /// Repair pairs after insert/delete/truncation. Never leave half a glyph
    /// behind: it would overlap unrelated text or produce ghost backgrounds.
    pub fn repair_wide_row(cells: &mut [Cell]) {
        for col in 0..cells.len() {
            if cells[col].is_wide() {
                if col + 1 >= cells.len() || !cells[col + 1].is_continuation() {
                    cells[col] = Cell::default();
                }
            } else if cells[col].is_continuation() && (col == 0 || !cells[col - 1].is_wide()) {
                cells[col] = Cell::default();
            }
        }
    }

    pub fn repair_row(&mut self, row: usize) {
        let start = self.row_map[row] * self.cols;
        Self::repair_wide_row(&mut self.cells[start..start + self.cols]);
        self.dirty[row] = true;
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
        while end > 0 && cells[end - 1] == Cell::default() {
            end -= 1;
        }
        end
    }

    fn reflow_visible_rows(
        source: ReflowSource<'_>,
        target: ReflowTarget,
        cursor: CursorSnapshot,
    ) -> ReflowResult {
        let mut new_cells = vec![Cell::default(); target.cols * target.rows];
        let mut new_soft_wrapped = vec![false; target.rows];
        if source.cols == 0 || source.rows == 0 || target.cols == 0 || target.rows == 0 {
            return ReflowResult {
                cells: new_cells,
                wrapped: new_soft_wrapped,
                cursor_col: 0,
                cursor_row: 0,
                pending_wrap: false,
                history: Vec::new(),
                history_wrapped: Vec::new(),
            };
        }

        let mut visual_rows = Vec::with_capacity(source.rows);
        let mut visual_wraps = Vec::with_capacity(source.rows);
        for &physical in source.row_map.iter().take(source.rows) {
            let src = physical * source.cols;
            // Expand temporarily clipped wide glyphs back to their intrinsic
            // two-cell representation before computing logical offsets.
            let mut row = Vec::new();
            for &cell in &source.cells[src..src + source.cols] {
                if cell.is_clipped_wide() {
                    let mut lead = cell;
                    lead.set_clipped_wide(false);
                    lead.set_wide(true);
                    row.push(lead);
                    row.push(lead.continuation());
                } else {
                    row.push(cell);
                }
            }
            visual_rows.push(row);
            visual_wraps.push(source.soft_wrapped[physical]);
        }

        let mut logical_lines: Vec<Vec<Cell>> = Vec::new();
        let mut current: Vec<Cell> = Vec::new();
        let mut line_idx = 0usize;
        let mut cursor_line_idx = 0usize;
        let mut cursor_offset = 0usize;
        for (row, (row_cells, &row_wraps)) in
            visual_rows.iter().zip(visual_wraps.iter()).enumerate()
        {
            if row == cursor.row {
                cursor_line_idx = line_idx;
                cursor_offset = current.len()
                    + row_cells
                        .iter()
                        .take({
                            let physical = source.row_map[row];
                            source.cells[physical * source.cols
                                ..physical * source.cols
                                    + cursor.col
                                    + usize::from(cursor.had_pending_wrap)]
                                .iter()
                                .map(|c| if c.is_clipped_wide() { 2 } else { 1 })
                                .sum()
                        })
                        .filter(|cell| !cell.is_wrap_spacer())
                        .count();
            }
            current.extend(
                row_cells
                    .iter()
                    .filter(|cell| !cell.is_wrap_spacer())
                    .copied(),
            );
            let has_next = row + 1 < source.rows;
            let is_wrapped = has_next && row_wraps;
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
            let mut positions = vec![(0usize, 0usize); line_len + 1];
            if line_len == 0 {
                virtual_rows.push(vec![Cell::default(); target.cols]);
                virtual_wraps.push(false);
            } else {
                let mut offset = 0usize;
                while offset < line_len {
                    let row_index = virtual_rows.len() - line_start;
                    let mut take = (line_len - offset).min(target.cols);
                    if take > 1 && line[offset + take - 1].is_wide() {
                        take -= 1;
                    }
                    let mut row_cells = vec![Cell::default(); target.cols];
                    row_cells[..take].copy_from_slice(&line[offset..offset + take]);
                    let single_column_wide = target.cols == 1 && line[offset].is_wide();
                    if single_column_wide {
                        row_cells[0].set_wide(false);
                        row_cells[0].set_clipped_wide(true);
                    }
                    let consumed = if single_column_wide {
                        (line_len - offset).min(2)
                    } else {
                        take
                    };
                    for i in 0..consumed {
                        positions[offset + i] = (row_index, i.min(target.cols - 1));
                    }
                    let more = offset + consumed < line_len;
                    positions[offset + consumed] = if more || take == target.cols {
                        (row_index + 1, 0)
                    } else {
                        (row_index, take)
                    };
                    if more && take < target.cols {
                        row_cells[take] = Cell::wrap_spacer();
                    }
                    virtual_rows.push(row_cells);
                    virtual_wraps.push(more);
                    offset += consumed;
                }
            }

            if idx != cursor_line_idx {
                continue;
            }
            let cursor_pos = cursor_offset.min(line_len);
            let (mut row_in_line, mut col_in_line) = positions[cursor_pos];
            cursor_pending_wrap = false;
            if (cursor.had_pending_wrap || cursor_pos == line_len)
                && cursor_pos > 0
                && col_in_line == 0
            {
                let previous = positions[cursor_pos - 1];
                row_in_line = previous.0;
                col_in_line = target.cols - 1;
                cursor_pending_wrap = true;
            }
            cursor_virtual_row = line_start + row_in_line;
            cursor_virtual_col = col_in_line.min(target.cols - 1);
        }

        if virtual_rows.is_empty() {
            virtual_rows.push(vec![Cell::default(); target.cols]);
            virtual_wraps.push(false);
        }
        if cursor_virtual_row >= virtual_rows.len() {
            cursor_virtual_row = virtual_rows.len() - 1;
            cursor_virtual_col = cursor_virtual_col.min(target.cols - 1);
            cursor_pending_wrap = false;
        }

        // Keep as much history as possible while ensuring the cursor stays visible.
        let content_end = virtual_rows
            .iter()
            .rposition(|row| row.iter().any(|cell| *cell != Cell::default()))
            .map_or(0, |r| r + 1)
            .max(cursor_virtual_row + 1);
        let window_start = content_end
            .saturating_sub(target.rows)
            .min(cursor_virtual_row);

        for (dst_row, (row_cells, &wrapped)) in virtual_rows
            .iter()
            .zip(virtual_wraps.iter())
            .skip(window_start)
            .take(target.rows)
            .enumerate()
        {
            let dst = dst_row * target.cols;
            new_cells[dst..dst + target.cols].copy_from_slice(row_cells);
            new_soft_wrapped[dst_row] = wrapped;
        }

        let mut new_cursor_row = cursor_virtual_row.saturating_sub(window_start);
        if new_cursor_row >= target.rows {
            new_cursor_row = target.rows - 1;
            cursor_pending_wrap = false;
        }
        let new_cursor_col = cursor_virtual_col.min(target.cols - 1);
        ReflowResult {
            cells: new_cells,
            wrapped: new_soft_wrapped,
            cursor_col: new_cursor_col,
            cursor_row: new_cursor_row,
            pending_wrap: cursor_pending_wrap,
            history: virtual_rows[..window_start]
                .iter()
                .flatten()
                .copied()
                .collect(),
            history_wrapped: virtual_wraps[..window_start].to_vec(),
        }
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        // A terminal always has at least one addressable cell.
        let cols = cols.max(1);
        let rows = rows.max(1);
        if cols == self.cols && rows == self.rows {
            return;
        }
        if !self.history_enabled {
            // Alternate-screen TUIs own their layout and redraw on SIGWINCH.
            // Resize their rectangle, never reflow it into shell history.
            let mut cells = vec![Cell::default(); cols * rows];
            for row in 0..rows.min(self.rows) {
                let n = cols.min(self.cols);
                cells[row * cols..row * cols + n].copy_from_slice(&self.row_cells(row)[..n]);
                Self::repair_wide_row(&mut cells[row * cols..(row + 1) * cols]);
            }
            self.cells = cells;
            self.soft_wrapped = vec![false; rows];
            self.cursor_col = self.cursor_col.min(cols - 1);
            self.cursor_row = self.cursor_row.min(rows - 1);
            self.pending_wrap = false;
        } else {
            // Reflow one ordered stream so a logical line crossing from
            // scrollback into the visible screen is not broken or truncated.
            let count = self.scrollback_count + self.rows;
            let mut cells = Vec::with_capacity(count * self.cols);
            let mut wrapped = Vec::with_capacity(count);
            for row in 0..self.scrollback_count {
                cells.extend_from_slice(self.scrollback_row(row));
                wrapped.push(self.scrollback_row_soft_wrapped(row));
            }
            for row in 0..self.rows {
                cells.extend_from_slice(self.row_cells(row));
                wrapped.push(self.row_soft_wrapped(row));
            }
            let result = Self::reflow_visible_rows(
                ReflowSource {
                    cells: &cells,
                    row_map: &(0..count).collect::<Vec<_>>(),
                    soft_wrapped: &wrapped,
                    cols: self.cols,
                    rows: count,
                },
                ReflowTarget { cols, rows },
                CursorSnapshot {
                    row: self.scrollback_count + self.cursor_row,
                    col: self.cursor_col,
                    had_pending_wrap: self.pending_wrap,
                },
            );
            let history_count = result.history_wrapped.len();
            let skip = history_count.saturating_sub(self.scrollback_limit);
            self.scrollback_buf = result.history[skip * cols..].to_vec();
            self.scrollback_wrapped = result.history_wrapped[skip..].to_vec();
            self.scrollback_count = history_count - skip;
            self.scrollback_start = 0;
            self.cells = result.cells;
            self.soft_wrapped = result.wrapped;
            self.cursor_col = result.cursor_col;
            self.cursor_row = result.cursor_row;
            self.pending_wrap = result.pending_wrap;
        }
        self.has_wide |= self.cells.iter().any(Cell::is_wide);
        self.cols = cols;
        self.rows = rows;
        self.row_map = (0..rows).collect();
        self.scroll_top = 0;
        self.scroll_bottom = rows - 1;
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
            self.blank_row = vec![self.erase_cell; self.cols];
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
        if self.history_enabled && top == 0 && bottom + 1 == self.rows && self.scrollback_limit > 0
        {
            for i in 0..count {
                let row_start = self.row_map[i] * self.cols;
                let row_end = row_start + self.cols;
                if self.scrollback_count < self.scrollback_limit {
                    // Growing phase: extend the flat buffer by one row.
                    // Vec doubling means amortised O(1); once full, no more allocs.
                    self.scrollback_buf
                        .extend_from_slice(&self.cells[row_start..row_end]);
                    self.scrollback_wrapped
                        .push(self.soft_wrapped[self.row_map[i]]);
                    self.scrollback_count += 1;
                } else {
                    // Steady state: overwrite the oldest row slot in place.
                    let dst = self.scrollback_start * self.cols;
                    self.scrollback_buf[dst..dst + self.cols]
                        .copy_from_slice(&self.cells[row_start..row_end]);
                    self.scrollback_wrapped[self.scrollback_start] =
                        self.soft_wrapped[self.row_map[i]];
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
        if self.cells[row_start + from_col].is_continuation() && from_col > 0 {
            self.cells[row_start + from_col - 1] = self.erase_cell;
        }
        if self.cells[row_start + end].is_wide() && end + 1 < self.cols {
            self.cells[row_start + end + 1] = self.erase_cell;
        }
        for col in from_col..=end {
            self.cells[row_start + col] = self.erase_cell;
        }
        if end + 1 == self.cols {
            self.soft_wrapped[self.row_map[row]] = false;
        }
        self.dirty[row] = true;
    }

    pub fn clear_screen(&mut self) {
        self.cells.fill(self.erase_cell);
        self.soft_wrapped.fill(false);
        self.cursor_col = 0;
        self.cursor_row = 0;
        self.mark_all_dirty();
    }

    pub fn erase_all(&mut self) {
        self.cells.fill(self.erase_cell);
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
    #[inline(always)]
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
    #[inline(always)]
    fn copy_into_row(&mut self, row: usize, dst_col: usize, src: &[Cell]) {
        if row >= self.rows || dst_col >= self.cols {
            return;
        }
        let n = src.len().min(self.cols - dst_col);
        if n == 0 {
            return;
        }
        let start = self.row_map[row] * self.cols + dst_col;
        if self.has_wide {
            // Clear partners outside the overwritten range before the memcpy.
            if dst_col > 0 && self.cells[start].is_continuation() {
                self.cells[start - 1] = Cell::default();
            }
            if dst_col + n < self.cols && self.cells[start + n - 1].is_wide() {
                self.cells[start + n] = Cell::default();
            }
        }
        self.cells[start..start + n].copy_from_slice(&src[..n]);
        if self.has_wide {
            // A pane's clipped edge must not join a glyph from another pane.
            if self.cells[start].is_continuation() {
                self.cells[start] = Cell::default();
            }
            if self.cells[start + n - 1].is_wide() {
                self.cells[start + n - 1] = Cell::default();
            }
        }
        self.dirty[row] = true;
    }

    /// Discard all scrollback history (e.g. Cmd+K clear).
    pub fn clear_scrollback(&mut self) {
        self.scrollback_buf.clear();
        self.scrollback_wrapped.clear();
        self.scrollback_start = 0;
        self.scrollback_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grapheme_width_changes_repair_partners() {
        let mut grid = Grid::new(6, 1);
        grid.set_row_soft_wrapped(0, true);
        grid.set_grapheme(1, 0, "👩‍💻", 2);
        assert!(grid.cell(2, 0).is_continuation());
        assert!(grid.row_soft_wrapped(0));
        grid.set_grapheme(1, 0, "x\u{0301}", 1);
        assert_eq!(grid.cell(2, 0).c(), ' ');
        assert!(!grid.cell(2, 0).is_continuation());
        grid.set_grapheme(1, 0, "👩‍💻", 2);
        let mut ascii = Cell::default();
        ascii.set_char('a');
        grid.put_ascii(2, 0, ascii);
        assert_eq!(grid.cell(1, 0).c(), ' ');
        assert_eq!(grid.cell(2, 0).c(), 'a');
    }

    #[test]
    fn pane_copy_repairs_clipped_wide_boundaries() {
        let mut source = Grid::new(4, 1);
        source.set_grapheme(0, 0, "👩‍💻", 2);
        let mut dst = Grid::new(4, 1);
        dst.copy_from_grid(0, 3, &source, source.row_cells(0));
        assert!(!dst.cell(3, 0).is_wide());
        dst.copy_from_grid(0, 0, &source, &source.row_cells(0)[1..]);
        assert!(!dst.cell(0, 0).is_continuation());
        dst.copy_from_grid(0, 0, &source, source.row_cells(0));
        assert!(dst.cell(1, 0).is_continuation());
        let ascii = Grid::new(1, 1);
        dst.copy_from_grid(0, 1, &ascii, ascii.row_cells(0));
        assert_eq!(dst.row_text(dst.row_cells(0)), "    ");
        dst.copy_from_grid(0, 0, &source, source.row_cells(0));
        dst.copy_from_grid(0, 0, &ascii, ascii.row_cells(0));
        assert_eq!(dst.row_text(dst.row_cells(0)), "    ");
    }

    #[test]
    fn extended_ids_are_remapped_between_panes_and_compacted_safely() {
        let mut a = Grid::new(4, 2);
        let mut b = Grid::new(4, 2);
        a.set_grapheme(0, 0, "x\u{0301}", 1);
        b.set_grapheme(0, 0, "y\u{0308}", 1);
        let mut dst = Grid::new(8, 2);
        dst.copy_from_grid(0, 0, &a, a.row_cells(0));
        dst.copy_from_grid(0, 4, &b, b.row_cells(0));
        assert_eq!(dst.row_text(dst.row_cells(0)), "x\u{0301}   y\u{0308}   ");
        let snapshot = dst.clone();
        dst.set_grapheme(0, 0, "z\u{0308}", 1);
        dst.compact_graphemes();
        assert_eq!(dst.row_text(dst.row_cells(0)), "z\u{0308}   y\u{0308}   ");
        assert_eq!(
            snapshot.row_text(snapshot.row_cells(0)),
            "x\u{0301}   y\u{0308}   "
        );
        assert_eq!(dst.graphemes.len(), 2);
    }

    #[test]
    fn test_new_grid_is_blank() {
        let g = Grid::new(80, 24);
        assert_eq!(g.cols, 80);
        assert_eq!(g.rows, 24);
        assert_eq!(g.cell(0, 0).c(), ' ');
        assert_eq!(g.cursor_col, 0);
        assert_eq!(g.cursor_row, 0);
    }

    #[test]
    fn test_write_and_read_cell() {
        let mut g = Grid::new(80, 24);
        g.cell_mut(5, 3).set_char('A');
        assert_eq!(g.cell(5, 3).c(), 'A');
    }

    #[test]
    fn test_resize_preserves_content() {
        let mut g = Grid::new(80, 24);
        g.cell_mut(2, 1).set_char('X');
        g.resize(100, 30);
        assert_eq!(g.cell(2, 1).c(), 'X');
        assert_eq!(g.cols, 100);
        assert_eq!(g.rows, 30);
    }

    #[test]
    fn test_scroll_up_moves_content() {
        let mut g = Grid::new(80, 24);
        g.cell_mut(0, 1).set_char('A');
        g.scroll_up(0, 23, 1);
        assert_eq!(g.cell(0, 0).c(), 'A');
        assert_eq!(g.cell(0, 23).c(), ' ');
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
        g.cell_mut(0, 0).set_char('A');
        g.cursor_row = 3; // scroll_bottom
        g.newline();
        assert_eq!(g.cell(0, 0).c(), ' ');
        assert_eq!(g.cursor_row, 3); // stays at bottom
    }

    #[test]
    fn test_scroll_down_moves_content() {
        let mut g = Grid::new(80, 24);
        g.cell_mut(0, 0).set_char('A');
        g.scroll_down(0, 23, 1);
        assert_eq!(g.cell(0, 1).c(), 'A');
        assert_eq!(g.cell(0, 0).c(), ' ');
    }

    #[test]
    fn test_clear_line() {
        let mut g = Grid::new(80, 24);
        for col in 0..5 {
            g.cell_mut(col, 0).set_char('X');
        }
        g.clear_line(0, 0, 79);
        assert_eq!(g.cell(0, 0).c(), ' ');
        assert_eq!(g.cell(4, 0).c(), ' ');
    }
}
