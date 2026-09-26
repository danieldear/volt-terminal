use std::path::PathBuf;

use crate::cell::{Cell, CellColor};
use crate::events::CoreEvent;
use crate::grid::Grid;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use volt_config::Color;
use vte::Perform;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseTrackingMode {
    Off,
    X10,
    ButtonEvent,
    AnyMotion,
}

pub struct Performer {
    pub grid: Grid,
    use_alt_screen: bool,
    alt_grid: Grid,
    saved_cursor_col: usize,
    saved_cursor_row: usize,
    current_fg: CellColor,
    current_bg: CellColor,
    current_bold: bool,
    current_italic: bool,
    current_underline: bool,
    current_reverse: bool,
    pub cursor_visible: bool,
    /// Bytes to write back to the PTY (e.g. cursor position reports).
    pub pending_writes: Vec<Vec<u8>>,
    /// Parsed terminal events (OSC title/cwd, command status, etc.) for UI.
    pub pending_events: Vec<CoreEvent>,
    pub display_dirty: bool,
    damage_rows: Option<(usize, usize)>,
    mouse_tracking: MouseTrackingMode,
    mouse_sgr: bool,
    application_cursor_keys: bool,
    bracketed_paste: bool,
}

impl Performer {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            grid: Grid::new(cols, rows),
            use_alt_screen: false,
            alt_grid: Grid::new_alt(cols, rows),
            saved_cursor_col: 0,
            saved_cursor_row: 0,
            current_fg: CellColor::Default,
            current_bg: CellColor::Default,
            current_bold: false,
            current_italic: false,
            current_underline: false,
            current_reverse: false,
            cursor_visible: true,
            pending_writes: Vec::new(),
            pending_events: Vec::new(),
            display_dirty: false,
            damage_rows: None,
            mouse_tracking: MouseTrackingMode::Off,
            mouse_sgr: false,
            application_cursor_keys: false,
            bracketed_paste: false,
        }
    }

    /// Resize both the main and alt screen grids.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.grid.resize(cols, rows);
        self.alt_grid.resize(cols, rows);
        if self.use_alt_screen {
            self.saved_cursor_col = self.alt_grid.cursor_col;
            self.saved_cursor_row = self.alt_grid.cursor_row;
        }
    }

    /// Set the configured scrollback limit on both grid buffers. The inactive
    /// buffer may become the active one after an alt-screen swap or RIS.
    pub fn set_scrollback_limit(&mut self, limit: usize) {
        // Configured limits are at least one; resize populated rings safely.
        self.grid.set_scrollback_limit(limit);
        self.alt_grid.set_scrollback_limit(limit);
    }

    pub fn mouse_tracking_mode(&self) -> MouseTrackingMode {
        self.mouse_tracking
    }

    pub fn mouse_sgr_mode(&self) -> bool {
        self.mouse_sgr
    }

    pub fn mouse_reporting_enabled(&self) -> bool {
        self.mouse_tracking != MouseTrackingMode::Off
    }

    pub fn application_cursor_keys_mode(&self) -> bool {
        self.application_cursor_keys
    }

    /// Whether the application requested bracketed paste (DECSET 2004).
    /// When enabled, pasted text must be wrapped in ESC[200~ … ESC[201~.
    pub fn bracketed_paste_mode(&self) -> bool {
        self.bracketed_paste
    }

    pub fn take_damage_rows(&mut self) -> Option<(usize, usize)> {
        self.damage_rows.take()
    }

    /// Extend the preceding grapheme without allocating anything in ASCII
    /// print(). Handles combining marks, variation selectors, emoji modifiers,
    /// ZWJ sequences and regional-indicator pairs across PTY chunk boundaries.
    #[inline(never)]
    fn extend_grapheme(&mut self, c: char) -> bool {
        if self.grid.cols == 0 || self.grid.rows == 0 {
            return false;
        }
        let row = self.grid.cursor_row;
        let mut col = if self.grid.pending_wrap {
            self.grid.cursor_col
        } else if self.grid.cursor_col > 0 {
            self.grid.cursor_col - 1
        } else {
            return false;
        };
        if self.grid.cell(col, row).is_continuation() && col > 0 {
            col -= 1;
        }
        let previous = *self.grid.cell(col, row);
        let mut text = String::new();
        self.grid.push_cell_text(&mut text, &previous);
        // Bounded per-cell extension protects against unbounded combining-mark
        // streams. The limit is bytes, not the number of stored grid cells.
        if text.len() + c.len_utf8() > 4096 {
            return char_display_width(c) == 0;
        }
        text.push(c);
        if text.graphemes(true).count() != 1 {
            return false;
        }
        let intrinsic_width = UnicodeWidthStr::width(text.as_str()).clamp(1, 2);
        let width = intrinsic_width.min(self.grid.cols);
        let old_width = previous.width();
        let mut target_col = col;
        let mut target_row = row;
        if width == 2 && col + 1 >= self.grid.cols {
            self.grid.put_char(col, row, Cell::wrap_spacer());
            self.grid.set_row_soft_wrapped(row, true);
            self.grid.cursor_col = 0;
            self.grid.pending_wrap = false;
            self.grid.newline();
            target_col = 0;
            target_row = self.grid.cursor_row;
            self.mark_dirty_all();
        }
        let mut cell = previous;
        cell.set_wide(width == 2);
        cell.set_clipped_wide(intrinsic_width == 2 && width == 1);
        self.grid.put_char(target_col, target_row, cell);
        // Retain the existing precomposed fast representation when possible.
        let composed = if previous.cluster_id().is_none() {
            unicode_normalization::char::compose(previous.c(), c)
        } else {
            None
        };
        if let Some(composed) = composed {
            let cell = self.grid.cell_mut(target_col, target_row);
            cell.set_char(composed);
            cell.set_wide(width == 2);
            cell.set_clipped_wide(intrinsic_width == 2 && width == 1);
        } else {
            self.grid.set_grapheme(target_col, target_row, &text, width);
        }
        if width != old_width || target_row != row || target_col != col {
            self.grid.cursor_row = target_row;
            self.grid.cursor_col = (target_col + width).min(self.grid.cols - 1);
            self.grid.pending_wrap = target_col + width >= self.grid.cols;
        }
        self.mark_dirty_row(target_row);
        true
    }

    #[inline(always)]
    fn make_cell(&self, c: char) -> Cell {
        let mut cell = Cell::default();
        cell.set_char(c);
        Cell {
            fg: self.current_fg,
            bg: self.current_bg,
            bold: self.current_bold,
            italic: self.current_italic,
            underline: self.current_underline,
            reverse: self.current_reverse,
            ..cell
        }
    }

    fn mark_dirty_range(&mut self, start_row: usize, end_row: usize) {
        if self.grid.rows == 0 {
            self.display_dirty = true;
            return;
        }
        let start = start_row.min(self.grid.rows - 1);
        let end = end_row.min(self.grid.rows - 1);
        if start > end {
            return;
        }
        self.damage_rows = Some(match self.damage_rows {
            Some((cur_start, cur_end)) => (cur_start.min(start), cur_end.max(end)),
            None => (start, end),
        });
        self.display_dirty = true;
    }

    fn mark_dirty_row(&mut self, row: usize) {
        self.mark_dirty_range(row, row);
    }

    fn mark_dirty_all(&mut self) {
        if self.grid.rows > 0 {
            self.mark_dirty_range(0, self.grid.rows - 1);
        } else {
            self.display_dirty = true;
        }
    }

    fn mark_cursor_row_change(&mut self, old_row: usize) {
        let new_row = self.grid.cursor_row;
        if old_row == new_row {
            self.mark_dirty_row(new_row);
        } else {
            self.mark_dirty_row(old_row);
            self.mark_dirty_row(new_row);
        }
    }

    fn reset_attrs(&mut self) {
        self.current_fg = CellColor::Default;
        self.current_bg = CellColor::Default;
        self.grid.set_erase_background(self.current_bg);
        self.current_bold = false;
        self.current_italic = false;
        self.current_underline = false;
        self.current_reverse = false;
    }

    fn param(params: &vte::Params, idx: usize) -> u16 {
        params
            .iter()
            .nth(idx)
            .and_then(|s| s.first().copied())
            .unwrap_or(0)
    }

    /// Full terminal reset (RIS — Reset to Initial State, `ESC c`): drops the
    /// alt screen entirely (not a restore — RIS discards it), clears the grid
    /// and scrollback, resets the cursor and all SGR attributes, and turns
    /// off every optional mode (mouse tracking, bracketed paste, application
    /// cursor keys). `scrollback_limit` is a config value, not terminal
    /// state, so it's preserved across the reset.
    pub fn reset(&mut self) {
        self.use_alt_screen = false;
        let cols = self.grid.cols;
        let rows = self.grid.rows;
        let scrollback_limit = self.grid.scrollback_limit;
        self.grid = Grid::new(cols, rows);
        self.grid.scrollback_limit = scrollback_limit;
        self.alt_grid = Grid::new_alt(cols, rows);
        self.alt_grid.scrollback_limit = scrollback_limit;
        self.saved_cursor_col = 0;
        self.saved_cursor_row = 0;
        self.reset_attrs();
        self.cursor_visible = true;
        self.pending_writes.clear();
        self.mouse_tracking = MouseTrackingMode::Off;
        self.mouse_sgr = false;
        self.application_cursor_keys = false;
        self.bracketed_paste = false;
        self.mark_dirty_all();
    }

    fn enter_alt_screen(&mut self) {
        if !self.use_alt_screen {
            self.saved_cursor_col = self.grid.cursor_col;
            self.saved_cursor_row = self.grid.cursor_row;
            let cols = self.grid.cols;
            let rows = self.grid.rows;
            self.alt_grid.resize(cols, rows);
            std::mem::swap(&mut self.grid, &mut self.alt_grid);
            self.grid.set_erase_background(self.current_bg);
            self.grid.erase_all();
            self.grid.cursor_col = 0;
            self.grid.cursor_row = 0;
            self.use_alt_screen = true;
        }
    }

    fn exit_alt_screen(&mut self) {
        if self.use_alt_screen {
            std::mem::swap(&mut self.grid, &mut self.alt_grid);
            self.grid.cursor_col = self.saved_cursor_col.min(self.grid.cols.saturating_sub(1));
            self.grid.cursor_row = self.saved_cursor_row.min(self.grid.rows.saturating_sub(1));
            self.grid.set_erase_background(self.current_bg);
            self.use_alt_screen = false;
        }
    }
}

fn percent_decode(input: &str) -> String {
    fn hex_value(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }

    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_value(bytes[i + 1]), hex_value(bytes[i + 2])) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn parse_osc7_path(raw: &[u8]) -> Option<PathBuf> {
    let s = std::str::from_utf8(raw).ok()?;
    let rest = s.strip_prefix("file://")?;
    let slash = rest.find('/')?;
    let path = &rest[slash..];
    Some(PathBuf::from(percent_decode(path)))
}

fn char_display_width(c: char) -> usize {
    UnicodeWidthChar::width(c).unwrap_or(1)
}

impl Perform for Performer {
    #[inline(always)]
    fn print(&mut self, c: char) {
        // Fast path for the overwhelmingly common case: printable ASCII with
        // no pending wrap, landing strictly inside the row. One row_map lookup
        // and one damage mark per character.
        if !self.grid.pending_wrap && (c as u32) >= 0x20 && (c as u32) < 0x7f {
            let col = self.grid.cursor_col;
            let row = self.grid.cursor_row;
            if col + 1 < self.grid.cols && row < self.grid.rows {
                let cell = self.make_cell(c);
                self.grid.put_ascii(col, row, cell);
                self.grid.cursor_col = col + 1;
                self.mark_dirty_row(row);
                return;
            }
        }

        if self.grid.cols == 0 || self.grid.rows == 0 {
            return;
        }
        let old_cursor_row = self.grid.cursor_row;
        if !c.is_ascii() && self.extend_grapheme(c) {
            return;
        }
        let intrinsic_width = char_display_width(c);
        let width = intrinsic_width.min(self.grid.cols);
        if width == 0 {
            return;
        }

        // Deferred wrap: fire the pending wrap before placing the new character.
        if self.grid.pending_wrap {
            self.grid.pending_wrap = false;
            self.grid.set_row_soft_wrapped(self.grid.cursor_row, true);
            self.grid.cursor_col = 0;
            let will_scroll = self.grid.cursor_row == self.grid.scroll_bottom;
            self.grid.newline();
            if will_scroll {
                self.mark_dirty_range(self.grid.scroll_top, self.grid.scroll_bottom);
            }
        }

        if width == 2 && self.grid.cursor_col + 1 >= self.grid.cols {
            let col = self.grid.cursor_col;
            let row = self.grid.cursor_row;
            self.grid.put_char(col, row, Cell::wrap_spacer());
            self.grid.set_row_soft_wrapped(self.grid.cursor_row, true);
            self.grid.cursor_col = 0;
            let will_scroll = self.grid.cursor_row == self.grid.scroll_bottom;
            self.grid.newline();
            if will_scroll {
                self.mark_dirty_range(self.grid.scroll_top, self.grid.scroll_bottom);
            }
        }

        let col = self.grid.cursor_col;
        let row = self.grid.cursor_row;
        if col < self.grid.cols && row < self.grid.rows {
            let mut cell = self.make_cell(c);
            cell.set_wide(width == 2);
            cell.set_clipped_wide(intrinsic_width == 2 && width == 1);
            self.grid.put_char(col, row, cell);
            self.mark_dirty_row(row);
        }

        if width == 1 {
            self.grid.advance_cursor();
        } else if col + 2 >= self.grid.cols {
            self.grid.pending_wrap = true;
            self.grid.cursor_col = self.grid.cols.saturating_sub(1);
        } else {
            self.grid.cursor_col += 2;
        }
        // The written row is already marked; only mark extra rows if the
        // cursor moved (wrap/scroll), avoiding a redundant per-char update.
        if self.grid.cursor_row != old_cursor_row {
            self.mark_cursor_row_change(old_cursor_row);
        }
    }

    fn execute(&mut self, byte: u8) {
        let old_cursor_row = self.grid.cursor_row;
        // Any C0 control character clears the pending-wrap state.
        self.grid.pending_wrap = false;
        match byte {
            0x08 => {
                // BS
                if self.grid.cursor_col > 0 {
                    self.grid.cursor_col -= 1;
                }
                self.mark_cursor_row_change(old_cursor_row);
            }
            0x09 => {
                // HT
                let next = (self.grid.cursor_col / 8 + 1) * 8;
                self.grid.cursor_col = next.min(self.grid.cols.saturating_sub(1));
                self.mark_cursor_row_change(old_cursor_row);
            }
            0x0a..=0x0c => {
                // LF/VT/FF
                self.grid.set_row_soft_wrapped(self.grid.cursor_row, false);
                let will_scroll = self.grid.cursor_row == self.grid.scroll_bottom;
                self.grid.newline();
                if will_scroll {
                    self.mark_dirty_range(self.grid.scroll_top, self.grid.scroll_bottom);
                } else {
                    self.mark_cursor_row_change(old_cursor_row);
                }
            }
            0x0d => {
                self.grid.cursor_col = 0; // CR
                self.mark_cursor_row_change(old_cursor_row);
            }
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        let private = intermediates.contains(&b'?');
        let old_cursor_row = self.grid.cursor_row;
        // Cursor-moving sequences clear pending wrap (same as xterm behaviour).
        match action {
            'A' | 'B' | 'C' | 'D' | 'E' | 'F' | 'G' | 'H' | 'f' | 'J' | 'K' | 'r' | 's' | 'u' => {
                self.grid.pending_wrap = false
            }
            _ => {}
        }
        match action {
            'A' => {
                // cursor up
                let n = Self::param(params, 0).max(1) as usize;
                self.grid.cursor_row = self.grid.cursor_row.saturating_sub(n);
                self.mark_cursor_row_change(old_cursor_row);
            }
            'B' => {
                // cursor down
                let n = Self::param(params, 0).max(1) as usize;
                self.grid.cursor_row =
                    (self.grid.cursor_row + n).min(self.grid.rows.saturating_sub(1));
                self.mark_cursor_row_change(old_cursor_row);
            }
            'C' => {
                // cursor forward
                let n = Self::param(params, 0).max(1) as usize;
                self.grid.cursor_col =
                    (self.grid.cursor_col + n).min(self.grid.cols.saturating_sub(1));
                self.mark_cursor_row_change(old_cursor_row);
            }
            'D' => {
                // cursor back
                let n = Self::param(params, 0).max(1) as usize;
                self.grid.cursor_col = self.grid.cursor_col.saturating_sub(n);
                self.mark_cursor_row_change(old_cursor_row);
            }
            'E' => {
                // cursor next line
                let n = Self::param(params, 0).max(1) as usize;
                self.grid.cursor_row =
                    (self.grid.cursor_row + n).min(self.grid.rows.saturating_sub(1));
                self.grid.cursor_col = 0;
                self.mark_cursor_row_change(old_cursor_row);
            }
            'F' => {
                // cursor prev line
                let n = Self::param(params, 0).max(1) as usize;
                self.grid.cursor_row = self.grid.cursor_row.saturating_sub(n);
                self.grid.cursor_col = 0;
                self.mark_cursor_row_change(old_cursor_row);
            }
            'G' => {
                // cursor horizontal absolute
                let col = Self::param(params, 0).saturating_sub(1) as usize;
                self.grid.cursor_col = col.min(self.grid.cols.saturating_sub(1));
                self.mark_cursor_row_change(old_cursor_row);
            }
            'H' | 'f' => {
                // cursor position (1-based)
                let row = Self::param(params, 0).saturating_sub(1) as usize;
                let col = Self::param(params, 1).saturating_sub(1) as usize;
                self.grid.cursor_row = row.min(self.grid.rows.saturating_sub(1));
                self.grid.cursor_col = col.min(self.grid.cols.saturating_sub(1));
                self.mark_cursor_row_change(old_cursor_row);
            }
            'J' => match Self::param(params, 0) {
                // erase in display
                0 => {
                    let (col, row) = (self.grid.cursor_col, self.grid.cursor_row);
                    let last_col = self.grid.cols.saturating_sub(1);
                    let last_row = self.grid.rows.saturating_sub(1);
                    self.grid.clear_line(row, col, last_col);
                    for r in (row + 1)..=last_row {
                        self.grid.clear_line(r, 0, last_col);
                    }
                    self.mark_dirty_range(row, last_row);
                }
                1 => {
                    let (col, row) = (self.grid.cursor_col, self.grid.cursor_row);
                    let last_col = self.grid.cols.saturating_sub(1);
                    for r in 0..row {
                        self.grid.clear_line(r, 0, last_col);
                    }
                    self.grid.clear_line(row, 0, col);
                    self.mark_dirty_range(0, row);
                }
                2 | 3 => {
                    self.grid.erase_all();
                    self.mark_dirty_all();
                }
                _ => {}
            },
            'K' => match Self::param(params, 0) {
                // erase in line
                0 => {
                    let (col, row) = (self.grid.cursor_col, self.grid.cursor_row);
                    let last_col = self.grid.cols.saturating_sub(1);
                    self.grid.clear_line(row, col, last_col);
                    self.mark_dirty_row(row);
                }
                1 => {
                    let (col, row) = (self.grid.cursor_col, self.grid.cursor_row);
                    self.grid.clear_line(row, 0, col);
                    self.mark_dirty_row(row);
                }
                2 => {
                    let row = self.grid.cursor_row;
                    let last_col = self.grid.cols.saturating_sub(1);
                    self.grid.clear_line(row, 0, last_col);
                    self.mark_dirty_row(row);
                }
                _ => {}
            },
            'L' => {
                // insert lines
                let n = Self::param(params, 0).max(1) as usize;
                let row = self.grid.cursor_row;
                let bottom = self.grid.scroll_bottom;
                self.grid.scroll_down(row, bottom, n);
                self.mark_dirty_range(row, bottom);
            }
            'M' => {
                // delete lines
                let n = Self::param(params, 0).max(1) as usize;
                let row = self.grid.cursor_row;
                let bottom = self.grid.scroll_bottom;
                self.grid.scroll_up(row, bottom, n);
                self.mark_dirty_range(row, bottom);
            }
            'P' => {
                // delete characters
                let n = Self::param(params, 0).max(1) as usize;
                let row = self.grid.cursor_row;
                let col = self.grid.cursor_col;
                let cols = self.grid.cols;
                for c in col..cols {
                    let src = if c + n < cols {
                        *self.grid.cell(c + n, row)
                    } else {
                        self.grid.erase_cell
                    };
                    *self.grid.cell_mut(c, row) = src;
                }
                self.grid.repair_row(row);
                self.mark_dirty_row(row);
            }
            '@' => {
                // insert blank characters — shift existing chars right
                let n = Self::param(params, 0).max(1) as usize;
                let row = self.grid.cursor_row;
                let col = self.grid.cursor_col;
                let cols = self.grid.cols;
                for c in (col..cols).rev() {
                    let src = if c >= col + n {
                        *self.grid.cell(c - n, row)
                    } else {
                        self.grid.erase_cell
                    };
                    *self.grid.cell_mut(c, row) = src;
                }
                self.grid.repair_row(row);
                self.mark_dirty_row(row);
            }
            'X' => {
                // erase N characters at cursor position (no cursor movement)
                let n = Self::param(params, 0).max(1) as usize;
                let row = self.grid.cursor_row;
                let col = self.grid.cursor_col;
                let cols = self.grid.cols;
                self.grid
                    .clear_line(row, col, (col + n).min(cols).saturating_sub(1));
                self.grid.repair_row(row);
                self.mark_dirty_row(row);
            }
            'S' => {
                // scroll up (pan up): content moves up, blank lines appear at bottom
                let n = Self::param(params, 0).max(1) as usize;
                let top = self.grid.scroll_top;
                let bot = self.grid.scroll_bottom;
                self.grid.scroll_up(top, bot, n);
                self.mark_dirty_range(top, bot);
            }
            'T' => {
                // scroll down (pan down): content moves down, blank lines appear at top
                let n = Self::param(params, 0).max(1) as usize;
                let top = self.grid.scroll_top;
                let bot = self.grid.scroll_bottom;
                self.grid.scroll_down(top, bot, n);
                self.mark_dirty_range(top, bot);
            }
            'c' => {
                // DA — device attributes. Programs (vim, tmux, …) send this and
                // wait for a reply; staying silent stalls them on a timeout.
                if intermediates.contains(&b'>') {
                    // Secondary DA: terminal type ; firmware version ; ROM cartridge.
                    self.pending_writes.push(b"\x1b[>1;10;0c".to_vec());
                } else if !private && Self::param(params, 0) == 0 {
                    // Primary DA: identify as a VT102-compatible terminal.
                    self.pending_writes.push(b"\x1b[?6c".to_vec());
                }
            }
            'n' => {
                // DSR — device status report
                if !private && Self::param(params, 0) == 5 {
                    // Report ready. Neovim uses this after OSC 11 to avoid
                    // waiting for an unsupported background-color query.
                    self.pending_writes.push(b"\x1b[0n".to_vec());
                } else if !private && Self::param(params, 0) == 6 {
                    // CPR: respond with ESC[row;colR (1-based)
                    let row = self.grid.cursor_row + 1;
                    let col = self.grid.cursor_col + 1;
                    self.pending_writes
                        .push(format!("\x1b[{};{}R", row, col).into_bytes());
                }
            }
            'r' => {
                // set scroll region (1-based)
                let top = Self::param(params, 0).saturating_sub(1) as usize;
                let bot = (Self::param(params, 1) as usize)
                    .saturating_sub(1)
                    .min(self.grid.rows.saturating_sub(1));
                if top < bot {
                    self.grid.scroll_top = top;
                    self.grid.scroll_bottom = bot;
                }
                self.grid.cursor_col = 0;
                self.grid.cursor_row = 0;
                self.mark_cursor_row_change(old_cursor_row);
            }
            's' => {
                // save cursor position (ANSI)
                self.saved_cursor_col = self.grid.cursor_col;
                self.saved_cursor_row = self.grid.cursor_row;
            }
            'u' => {
                // restore cursor position (ANSI)
                self.grid.cursor_col = self.saved_cursor_col.min(self.grid.cols.saturating_sub(1));
                self.grid.cursor_row = self.saved_cursor_row.min(self.grid.rows.saturating_sub(1));
                self.mark_cursor_row_change(old_cursor_row);
            }
            'h' if private => match Self::param(params, 0) {
                1 => self.application_cursor_keys = true,
                1049 => {
                    self.enter_alt_screen();
                    self.mark_dirty_all();
                }
                25 => {
                    self.cursor_visible = true;
                    self.mark_dirty_row(self.grid.cursor_row);
                }
                1000 => self.mouse_tracking = MouseTrackingMode::X10,
                1002 => self.mouse_tracking = MouseTrackingMode::ButtonEvent,
                1003 => self.mouse_tracking = MouseTrackingMode::AnyMotion,
                1006 => self.mouse_sgr = true,
                2004 => self.bracketed_paste = true,
                _ => {}
            },
            'l' if private => match Self::param(params, 0) {
                1 => self.application_cursor_keys = false,
                1049 => {
                    self.exit_alt_screen();
                    self.mark_dirty_all();
                }
                25 => {
                    self.cursor_visible = false;
                    self.mark_dirty_row(self.grid.cursor_row);
                }
                1000 | 1002 | 1003 => self.mouse_tracking = MouseTrackingMode::Off,
                1006 => self.mouse_sgr = false,
                2004 => self.bracketed_paste = false,
                _ => {}
            },
            'm' => {
                // SGR
                let mut iter = params.iter().peekable();
                if iter.peek().is_none() {
                    self.reset_attrs();
                    return;
                }
                while let Some(subparams) = iter.next() {
                    let p = subparams.first().copied().unwrap_or(0);
                    match p {
                        0 => self.reset_attrs(),
                        1 => self.current_bold = true,
                        3 => self.current_italic = true,
                        4 => self.current_underline = true,
                        7 => self.current_reverse = true,
                        22 => self.current_bold = false,
                        23 => self.current_italic = false,
                        24 => self.current_underline = false,
                        27 => self.current_reverse = false,
                        30..=37 => self.current_fg = CellColor::Indexed(p as u8 - 30),
                        39 => self.current_fg = CellColor::Default,
                        40..=47 => self.current_bg = CellColor::Indexed(p as u8 - 40),
                        49 => self.current_bg = CellColor::Default,
                        90..=97 => self.current_fg = CellColor::Indexed(p as u8 - 90 + 8),
                        100..=107 => self.current_bg = CellColor::Indexed(p as u8 - 100 + 8),
                        38 | 48 => {
                            let is_fg = p == 38;
                            if subparams.len() >= 3 && subparams[1] == 5 {
                                let idx = subparams[2] as u8;
                                if is_fg {
                                    self.current_fg = CellColor::Indexed(idx);
                                } else {
                                    self.current_bg = CellColor::Indexed(idx);
                                }
                            } else if subparams.len() >= 5 && subparams[1] == 2 {
                                let c = Color::rgb(
                                    subparams[2] as u8,
                                    subparams[3] as u8,
                                    subparams[4] as u8,
                                );
                                if is_fg {
                                    self.current_fg = CellColor::Rgb(c);
                                } else {
                                    self.current_bg = CellColor::Rgb(c);
                                }
                            } else if let Some(next) = iter.next() {
                                match next.first().copied().unwrap_or(0) {
                                    5 => {
                                        if let Some(idx_param) = iter.next() {
                                            let idx = idx_param.first().copied().unwrap_or(0) as u8;
                                            if is_fg {
                                                self.current_fg = CellColor::Indexed(idx);
                                            } else {
                                                self.current_bg = CellColor::Indexed(idx);
                                            }
                                        }
                                    }
                                    2 => {
                                        let r = iter
                                            .next()
                                            .and_then(|s| s.first().copied())
                                            .unwrap_or(0)
                                            as u8;
                                        let g = iter
                                            .next()
                                            .and_then(|s| s.first().copied())
                                            .unwrap_or(0)
                                            as u8;
                                        let b = iter
                                            .next()
                                            .and_then(|s| s.first().copied())
                                            .unwrap_or(0)
                                            as u8;
                                        let c = Color::rgb(r, g, b);
                                        if is_fg {
                                            self.current_fg = CellColor::Rgb(c);
                                        } else {
                                            self.current_bg = CellColor::Rgb(c);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
                self.grid.set_erase_background(self.current_bg);
            }
            _ => {}
        }
    }

    fn hook(&mut self, _: &vte::Params, _: &[u8], _: bool, _: char) {}
    fn put(&mut self, _: u8) {}
    fn unhook(&mut self) {}

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.is_empty() {
            return;
        }

        if params.len() >= 2 && (params[0] == b"0" || params[0] == b"2") {
            let title = String::from_utf8_lossy(params[1]).to_string();
            self.pending_events.push(CoreEvent::TitleChanged(title));
            return;
        }

        if params.len() >= 2 && params[0] == b"7" {
            if let Some(path) = parse_osc7_path(params[1]) {
                self.pending_events.push(CoreEvent::CwdChanged(path));
            }
            return;
        }

        if params.len() >= 2 && params[0] == b"133" && params[1] == b"D" {
            let exit_code = params
                .get(2)
                .and_then(|code| std::str::from_utf8(code).ok())
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(0);
            self.pending_events.push(CoreEvent::CommandFinished {
                exit_code,
                duration_ms: 0,
            });
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        let old_cursor_row = self.grid.cursor_row;
        match byte {
            b'7' => {
                // DEC save cursor
                self.saved_cursor_col = self.grid.cursor_col;
                self.saved_cursor_row = self.grid.cursor_row;
            }
            b'8' => {
                // DEC restore cursor
                self.grid.pending_wrap = false;
                self.grid.cursor_col = self.saved_cursor_col.min(self.grid.cols.saturating_sub(1));
                self.grid.cursor_row = self.saved_cursor_row.min(self.grid.rows.saturating_sub(1));
                self.mark_cursor_row_change(old_cursor_row);
            }
            b'M' => {
                // reverse index
                if self.grid.cursor_row == self.grid.scroll_top {
                    let top = self.grid.scroll_top;
                    let bottom = self.grid.scroll_bottom;
                    self.grid.scroll_down(top, bottom, 1);
                    self.mark_dirty_range(top, bottom);
                } else if self.grid.cursor_row > 0 {
                    self.grid.cursor_row -= 1;
                    self.mark_cursor_row_change(old_cursor_row);
                }
            }
            b'c' => {
                // RIS — full terminal reset.
                self.reset();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(p: &mut Performer, bytes: &[u8]) {
        let mut parser = vte::Parser::new();
        parser.advance(p, bytes);
    }

    fn collect_logical_lines(p: &Performer) -> Vec<String> {
        let mut logical_lines = Vec::new();
        let mut current = String::new();
        for row in 0..p.grid.rows {
            let mut row_text = String::new();
            for col in 0..p.grid.cols {
                row_text.push(p.grid.cell(col, row).c());
            }
            current.push_str(row_text.trim_end());
            if !p.grid.row_soft_wrapped(row) {
                logical_lines.push(current.clone());
                current.clear();
            }
        }
        if !current.is_empty() {
            logical_lines.push(current);
        }
        logical_lines
    }

    #[test]
    fn extended_graphemes_survive_chunks_scroll_and_blit() {
        let mut p = Performer::new(8, 2);
        let mut parser = vte::Parser::new();
        for byte in "x\u{0301}\u{0308} 👩\u{200d}💻".as_bytes() {
            parser.advance(&mut p, &[*byte]);
        }
        assert_eq!(
            p.grid.row_text(p.grid.row_cells(0)).trim_end(),
            "x\u{0301}\u{0308} 👩\u{200d}💻"
        );
        assert_eq!(p.grid.cursor_col, 4);
        assert!(p.grid.cell(3, 0).is_continuation());
        let mut copy = Grid::new(8, 2);
        copy.copy_from_grid(0, 0, &p.grid, p.grid.row_cells(0));
        assert_eq!(
            copy.row_text(copy.row_cells(0)),
            p.grid.row_text(p.grid.row_cells(0))
        );
        feed(&mut p, b"\r\n\r\n");
        assert!(p
            .grid
            .row_text(p.grid.scrollback_row(0))
            .starts_with("x\u{0301}\u{0308}"));
    }

    #[test]
    fn wide_overwrite_erase_insert_delete_keep_pairs_valid() {
        for edit in [
            "\x1b[1;2HX",
            "\x1b[1;2H\x1b[X",
            "\x1b[1;2H\x1b[P",
            "\x1b[1;2H\x1b[@",
        ] {
            let mut p = Performer::new(8, 2);
            feed(&mut p, format!("你好吗{edit}").as_bytes());
            for row in 0..p.grid.rows {
                for col in 0..p.grid.cols {
                    let cell = p.grid.cell(col, row);
                    if cell.is_wide() {
                        assert!(
                            col + 1 < p.grid.cols && p.grid.cell(col + 1, row).is_continuation()
                        );
                    }
                    if cell.is_continuation() {
                        assert!(col > 0 && p.grid.cell(col - 1, row).is_wide());
                    }
                }
            }
        }
    }

    #[test]
    fn wide_grapheme_expands_at_right_margin() {
        let mut p = Performer::new(3, 3);
        feed(&mut p, "ab❤\u{fe0f}".as_bytes());
        assert_eq!(p.grid.row_text(p.grid.row_cells(0)), "ab");
        assert_eq!(p.grid.row_text(p.grid.row_cells(1)).trim_end(), "❤\u{fe0f}");
        assert!(p.grid.cell(0, 1).is_wide());
        assert_eq!(p.grid.cursor_col, 2);
    }

    #[test]
    fn emoji_modifiers_and_flags_are_single_wide_clusters() {
        let mut p = Performer::new(20, 2);
        feed(&mut p, "👍🏽🇺🇸Z".as_bytes());
        assert_eq!(p.grid.cursor_col, 5);
        assert_eq!(p.grid.row_text(p.grid.row_cells(0)).trim_end(), "👍🏽🇺🇸Z");
        assert!(p.grid.cell(1, 0).is_continuation());
        assert!(p.grid.cell(3, 0).is_continuation());
    }

    #[test]
    fn reflow_keeps_wide_graphemes_together() {
        let mut p = Performer::new(8, 5);
        feed(&mut p, "ab你cd好".as_bytes());
        p.resize(3, 5);
        for row in 0..p.grid.rows {
            for col in 0..p.grid.cols {
                if p.grid.cell(col, row).is_wide() {
                    assert!(col + 1 < p.grid.cols && p.grid.cell(col + 1, row).is_continuation());
                }
            }
        }
        let text: String = (0..p.grid.rows)
            .map(|r| p.grid.row_text(p.grid.row_cells(r)).trim_end().to_string())
            .collect();
        assert_eq!(text, "ab你cd好");
        p.resize(12, 5);
        assert_eq!(p.grid.row_text(p.grid.row_cells(0)).trim_end(), "ab你cd好");
    }

    fn all_logical_lines(p: &Performer) -> Vec<String> {
        let g = &p.grid;
        let mut lines = Vec::new();
        let mut text = String::new();
        for row in 0..g.scrollback_len() + g.rows {
            let (cells, wrapped) = if row < g.scrollback_len() {
                (g.scrollback_row(row), g.scrollback_row_soft_wrapped(row))
            } else {
                let r = row - g.scrollback_len();
                (g.row_cells(r), g.row_soft_wrapped(r))
            };
            let part = g.row_text(cells);
            text.push_str(if wrapped { &part } else { part.trim_end() });
            if !wrapped {
                lines.push(std::mem::take(&mut text));
            }
        }
        if !text.is_empty() {
            lines.push(text);
        }
        while lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }
        lines
    }

    #[test]
    fn history_and_visible_boundary_reflow_without_text_loss() {
        let mut p = Performer::new(8, 3);
        let text = "first line long enough to cross history boundary\r\nsecond 日本語 x\u{0308} line\r\n$ ";
        feed(&mut p, text.as_bytes());
        let expected = all_logical_lines(&p);
        assert!(p.grid.scrollback_len() > 0);
        for cols in [3, 17, 1, 8, 40] {
            p.resize(cols, 3);
            assert_eq!(all_logical_lines(&p), expected, "cols={cols}");
        }
    }

    #[test]
    fn row_shrink_pushes_history_and_growth_restores_it() {
        let mut p = Performer::new(6, 4);
        feed(&mut p, b"A\r\nB\r\nC\r\nD");
        p.resize(6, 2);
        assert_eq!(p.grid.scrollback_len(), 2);
        assert_eq!(all_logical_lines(&p), ["A", "B", "C", "D"]);
        p.resize(6, 4);
        assert_eq!(p.grid.scrollback_len(), 0);
        assert_eq!(p.grid.row_text(p.grid.row_cells(0)).trim_end(), "A");
    }

    #[test]
    fn history_ring_wrap_preserves_hard_newlines_and_limits() {
        let mut p = Performer::new(5, 2);
        p.set_scrollback_limit(3);
        feed(
            &mut p,
            b"11111\r\n22222\r\n33333\r\n44444\r\n55555\r\n66666",
        );
        assert_eq!(p.grid.scrollback_len(), 3);
        assert_eq!(
            all_logical_lines(&p),
            ["22222", "33333", "44444", "55555", "66666"]
        );
        p.resize(10, 2);
        assert_eq!(
            all_logical_lines(&p),
            ["22222", "33333", "44444", "55555", "66666"]
        );
        p.resize(2, 2);
        assert!(p.grid.scrollback_len() <= 3);
        assert!(all_logical_lines(&p).last().unwrap().ends_with("66666"));
    }

    #[test]
    fn one_column_restores_wide_cells_and_cursor_after_growth() {
        for initial_cols in [1, 8] {
            let mut p = Performer::new(initial_cols, 8);
            feed(&mut p, "你👩\u{200d}💻".as_bytes());
            p.resize(1, 8);
            p.resize(8, 8);
            assert!(p.grid.cell(0, 0).is_wide());
            assert!(p.grid.cell(1, 0).is_continuation());
            assert!(p.grid.cell(2, 0).is_wide());
            assert!(p.grid.cell(3, 0).is_continuation());
            assert_eq!(p.grid.cursor_col, 4);
            feed(&mut p, b"Z");
            assert_eq!(
                p.grid.row_text(p.grid.row_cells(0)).trim_end(),
                "你👩\u{200d}💻Z"
            );
        }
    }

    #[test]
    fn alternate_screen_does_not_accumulate_history_or_restore_stale_cursor() {
        let mut p = Performer::new(8, 3);
        feed(&mut p, b"abcdefghijkl");
        feed(&mut p, b"\x1b[?1049h");
        feed(&mut p, b"1\r\n2\r\n3\r\n4\r\n5");
        assert_eq!(p.grid.scrollback_len(), 0);
        p.resize(16, 3);
        feed(&mut p, b"\x1b[?1049lZ");
        assert_eq!(
            p.grid.row_text(p.grid.row_cells(0)).trim_end(),
            "abcdefghijklZ"
        );
    }

    #[test]
    fn partial_scroll_region_does_not_enter_history() {
        let mut p = Performer::new(5, 4);
        feed(&mut p, b"\x1b[1;2r1\r\n2\r\n3\r\n4");
        assert_eq!(p.grid.scrollback_len(), 0);
    }

    #[test]
    fn repeated_history_reflow_preserves_unicode_and_hard_lines() {
        let mut p = Performer::new(13, 4);
        let input = (0..30)
            .map(|i| format!("{i:02}:a你b👩\u{200d}💻x\u{0308} z\r\n"))
            .collect::<String>();
        feed(&mut p, input.as_bytes());
        let expected = all_logical_lines(&p);
        for width in [1, 2, 3, 7, 19, 80, 4, 13] {
            for height in [1, 3, 8] {
                p.resize(width, height);
                assert_eq!(all_logical_lines(&p), expected, "{width}x{height}");
                for row in 0..p.grid.rows {
                    for col in 0..p.grid.cols {
                        if p.grid.cell(col, row).is_wide() {
                            assert!(
                                col + 1 < p.grid.cols
                                    && p.grid.cell(col + 1, row).is_continuation()
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn reduced_history_limit_keeps_newest_rows_after_ring_wrap() {
        let mut p = Performer::new(6, 2);
        p.set_scrollback_limit(4);
        feed(&mut p, b"A\r\nB\r\nC\r\nD\r\nE\r\nF\r\nG");
        p.set_scrollback_limit(2);
        assert_eq!(all_logical_lines(&p), ["D", "E", "F", "G"]);
        feed(&mut p, b"\r\nH");
        assert_eq!(all_logical_lines(&p), ["E", "F", "G", "H"]);
        p.resize(12, 3);
        assert_eq!(all_logical_lines(&p), ["E", "F", "G", "H"]);
    }

    #[test]
    fn widened_single_column_glyph_repairs_ascii_overwrite() {
        let mut p = Performer::new(1, 3);
        feed(&mut p, "你".as_bytes());
        p.resize(8, 3);
        feed(&mut p, b"\x1b[1;2HX");
        assert_eq!(p.grid.cell(0, 0).c(), ' ');
        assert_eq!(p.grid.cell(1, 0).c(), 'X');
    }

    #[test]
    fn erase_and_scroll_use_current_background_without_text_attributes() {
        for erase in ["\x1b[2J", "\x1b[2K", "\x1b[8X", "\x1b[8P", "\x1b[8@"] {
            let mut p = Performer::new(8, 2);
            feed(
                &mut p,
                format!("text\x1b[1;1H\x1b[1;4;44m{erase}").as_bytes(),
            );
            for col in 0..8 {
                let cell = p.grid.cell(col, 0);
                assert_eq!(cell.bg, CellColor::Indexed(4), "{erase:?}");
                assert_eq!(cell.c(), ' ');
                assert!(!cell.bold && !cell.underline);
            }
        }
        let mut p = Performer::new(4, 2);
        feed(&mut p, b"\x1b[44m\r\n\r\n");
        assert_eq!(p.grid.cell(0, 1).bg, CellColor::Indexed(4));
        feed(&mut p, b"\x1b[0m\r\n");
        assert_eq!(p.grid.cell(0, 1).bg, CellColor::Default);
    }

    #[test]
    fn test_print_places_char() {
        let mut p = Performer::new(80, 24);
        feed(&mut p, b"AB");
        assert_eq!(p.grid.cell(0, 0).c(), 'A');
        assert_eq!(p.grid.cell(1, 0).c(), 'B');
        assert_eq!(p.grid.cursor_col, 2);
    }

    #[test]
    fn device_status_query_reports_ready_without_changing_display() {
        let mut p = Performer::new(8, 3);
        feed(&mut p, b"\x1b]11;?\x07\x1b[5n");
        assert_eq!(p.pending_writes, vec![b"\x1b[0n".to_vec()]);
        assert!(!p.display_dirty);
        p.pending_writes.clear();
        feed(&mut p, b"\x1b[?5n\x1b[?6n");
        assert!(p.pending_writes.is_empty());
    }

    #[test]
    fn test_cr_lf_moves_cursor() {
        let mut p = Performer::new(80, 24);
        feed(&mut p, b"A\r\nB");
        assert_eq!(p.grid.cell(0, 0).c(), 'A');
        assert_eq!(p.grid.cell(0, 1).c(), 'B');
        assert_eq!(p.grid.cursor_row, 1);
        assert_eq!(p.grid.cursor_col, 1);
    }

    #[test]
    fn test_csi_cursor_up() {
        let mut p = Performer::new(80, 24);
        p.grid.cursor_row = 5;
        feed(&mut p, b"\x1b[2A");
        assert_eq!(p.grid.cursor_row, 3);
    }

    #[test]
    fn test_csi_cursor_position() {
        let mut p = Performer::new(80, 24);
        feed(&mut p, b"\x1b[5;10H");
        assert_eq!(p.grid.cursor_row, 4);
        assert_eq!(p.grid.cursor_col, 9);
    }

    #[test]
    fn test_csi_erase_line_entire() {
        let mut p = Performer::new(80, 24);
        feed(&mut p, b"Hello");
        feed(&mut p, b"\x1b[2K");
        for col in 0..5 {
            assert_eq!(p.grid.cell(col, 0).c(), ' ');
        }
    }

    #[test]
    fn test_sgr_bold_and_reset() {
        let mut p = Performer::new(80, 24);
        feed(&mut p, b"\x1b[1mA");
        assert!(p.grid.cell(0, 0).bold);
        feed(&mut p, b"\x1b[0mB");
        assert!(!p.grid.cell(1, 0).bold);
    }

    #[test]
    fn test_sgr_indexed_color() {
        let mut p = Performer::new(80, 24);
        feed(&mut p, b"\x1b[31mA");
        match p.grid.cell(0, 0).fg {
            crate::cell::CellColor::Indexed(1) => {}
            other => panic!("expected Indexed(1), got {:?}", other),
        }
    }

    #[test]
    fn test_backspace() {
        let mut p = Performer::new(80, 24);
        feed(&mut p, b"AB\x08");
        assert_eq!(p.grid.cursor_col, 1);
    }

    #[test]
    fn test_sgr_colon_form_rgb() {
        let mut p = Performer::new(80, 24);
        feed(&mut p, b"\x1b[38:2:255:0:128mA");
        match p.grid.cell(0, 0).fg {
            crate::cell::CellColor::Rgb(c) => {
                assert_eq!(c.r, 255);
                assert_eq!(c.g, 0);
                assert_eq!(c.b, 128);
            }
            other => panic!("expected Rgb, got {:?}", other),
        }
    }

    #[test]
    fn test_sgr_reverse_video_toggle() {
        let mut p = Performer::new(80, 24);
        feed(&mut p, b"\x1b[7mA");
        assert!(p.grid.cell(0, 0).reverse);
        feed(&mut p, b"\x1b[27mB");
        assert!(!p.grid.cell(1, 0).reverse);
    }

    #[test]
    fn test_cursor_position_report() {
        let mut p = Performer::new(80, 24);
        p.grid.cursor_row = 3;
        p.grid.cursor_col = 7;
        feed(&mut p, b"\x1b[6n");
        assert_eq!(p.pending_writes, vec![b"\x1b[4;8R".to_vec()]);
    }

    #[test]
    fn test_osc_title_event() {
        let mut p = Performer::new(80, 24);
        feed(&mut p, b"\x1b]2;hello world\x07");
        match p.pending_events.pop() {
            Some(CoreEvent::TitleChanged(t)) => assert_eq!(t, "hello world"),
            other => panic!("expected title event, got {:?}", other),
        }
    }

    #[test]
    fn test_osc7_cwd_event() {
        let mut p = Performer::new(80, 24);
        feed(&mut p, b"\x1b]7;file:///tmp/my%20dir\x07");
        match p.pending_events.pop() {
            Some(CoreEvent::CwdChanged(path)) => assert_eq!(path, PathBuf::from("/tmp/my dir")),
            other => panic!("expected cwd event, got {:?}", other),
        }
    }

    #[test]
    fn test_osc133_command_finished_event() {
        let mut p = Performer::new(80, 24);
        feed(&mut p, b"\x1b]133;D;42\x07");
        match p.pending_events.pop() {
            Some(CoreEvent::CommandFinished {
                exit_code,
                duration_ms,
            }) => {
                assert_eq!(exit_code, 42);
                assert_eq!(duration_ms, 0);
            }
            other => panic!("expected command finished event, got {:?}", other),
        }
    }

    #[test]
    fn test_alt_screen_switch() {
        let mut p = Performer::new(80, 24);
        feed(&mut p, b"Hello");
        assert_eq!(p.grid.cell(0, 0).c(), 'H');

        // Enter alt screen — grid should be cleared
        feed(&mut p, b"\x1b[?1049h");
        assert!(p.use_alt_screen);
        assert_eq!(p.grid.cell(0, 0).c(), ' ');

        // Write something on alt screen
        feed(&mut p, b"Alt");
        assert_eq!(p.grid.cell(0, 0).c(), 'A');

        // Leave alt screen — main screen restored
        feed(&mut p, b"\x1b[?1049l");
        assert!(!p.use_alt_screen);
        assert_eq!(p.grid.cell(0, 0).c(), 'H');
    }

    #[test]
    fn test_cursor_hide_show() {
        let mut p = Performer::new(80, 24);
        assert!(p.cursor_visible);
        feed(&mut p, b"\x1b[?25l");
        assert!(!p.cursor_visible);
        feed(&mut p, b"\x1b[?25h");
        assert!(p.cursor_visible);
    }

    #[test]
    fn test_application_cursor_keys_mode_toggle() {
        let mut p = Performer::new(80, 24);
        assert!(!p.application_cursor_keys_mode());
        feed(&mut p, b"\x1b[?1h");
        assert!(p.application_cursor_keys_mode());
        feed(&mut p, b"\x1b[?1l");
        assert!(!p.application_cursor_keys_mode());
    }

    #[test]
    fn test_dec_save_restore_cursor() {
        let mut p = Performer::new(80, 24);
        p.grid.cursor_col = 10;
        p.grid.cursor_row = 5;
        feed(&mut p, b"\x1b7"); // save
        p.grid.cursor_col = 0;
        p.grid.cursor_row = 0;
        feed(&mut p, b"\x1b8"); // restore
        assert_eq!(p.grid.cursor_col, 10);
        assert_eq!(p.grid.cursor_row, 5);
    }

    #[test]
    fn test_resize_wider_clears_pending_wrap_without_newline() {
        let mut p = Performer::new(5, 3);
        feed(&mut p, b"ABCDE");
        assert!(p.grid.pending_wrap);
        assert_eq!(p.grid.cursor_col, 4);

        p.resize(10, 3);
        assert!(!p.grid.pending_wrap);
        assert_eq!(p.grid.cursor_col, 5);

        feed(&mut p, b"F");
        assert_eq!(p.grid.cell(5, 0).c(), 'F');
        assert_eq!(p.grid.cell(0, 1).c(), ' ');
        assert_eq!(p.grid.cursor_row, 0);
        assert_eq!(p.grid.cursor_col, 6);
    }

    #[test]
    fn test_resize_narrow_reflows_pending_wrap_position() {
        let mut p = Performer::new(5, 3);
        feed(&mut p, b"ABCDE");
        assert!(p.grid.pending_wrap);

        p.resize(4, 3);
        assert!(!p.grid.pending_wrap);
        assert_eq!(p.grid.cursor_row, 1);
        assert_eq!(p.grid.cursor_col, 1);

        feed(&mut p, b"F");
        assert_eq!(p.grid.cell(0, 1).c(), 'E');
        assert_eq!(p.grid.cell(1, 1).c(), 'F');
        assert_eq!(p.grid.cursor_row, 1);
        assert_eq!(p.grid.cursor_col, 2);
    }

    #[test]
    fn test_resize_wider_reflows_soft_wrapped_history() {
        let mut p = Performer::new(12, 6);
        feed(&mut p, b"Federated Learning.key\r\n$ ");

        p.resize(24, 6);

        let logical_lines = collect_logical_lines(&p);
        assert!(logical_lines
            .iter()
            .any(|line| line.contains("Federated Learning.key")));
        assert!(logical_lines.iter().any(|line| line.starts_with("$")));
    }

    #[test]
    fn test_repeated_resize_keeps_wrapped_text() {
        let mut p = Performer::new(12, 8);
        feed(&mut p, b"Federated Learning.key\r\n$ ");

        p.resize(8, 8);
        p.resize(20, 8);
        p.resize(10, 8);
        p.resize(24, 8);

        let logical_lines = collect_logical_lines(&p);

        assert!(logical_lines
            .iter()
            .any(|line| line.contains("Federated Learning.key")));
    }

    #[test]
    fn test_resize_does_not_merge_hard_newlines_at_margin() {
        let mut p = Performer::new(5, 4);
        feed(&mut p, b"ABCDE\r\nFGHIJ");

        p.resize(10, 4);

        let logical_lines = collect_logical_lines(&p);
        assert!(logical_lines.iter().any(|line| line == "ABCDE"));
        assert!(logical_lines.iter().any(|line| line.starts_with("FGHIJ")));
        assert!(!logical_lines.iter().any(|line| line.contains("ABCDEFGHIJ")));
    }

    #[test]
    fn test_resize_after_el0_clears_stale_soft_wrap() {
        let mut p = Performer::new(6, 4);
        feed(&mut p, b"ABCDEFG");
        feed(&mut p, b"\x1b[1;4H");
        feed(&mut p, b"\x1b[K");

        p.resize(12, 4);

        let logical_lines = collect_logical_lines(&p);
        assert!(logical_lines.iter().any(|line| line == "ABC"));
        assert!(logical_lines.iter().any(|line| line.starts_with("G")));
        assert!(!logical_lines.iter().any(|line| line.contains("ABCG")));
    }

    #[test]
    fn test_row_only_resize_keeps_cursor_context_not_top_rows() {
        let mut p = Performer::new(6, 4);
        feed(&mut p, b"A\r\nB\r\nC\r\nD");

        p.resize(6, 2);

        assert_eq!(p.grid.cursor_row, 1);
        assert_eq!(p.grid.cell(0, 0).c(), 'C');
        assert_eq!(p.grid.cell(0, 1).c(), 'D');
    }

    #[test]
    fn test_wide_char_advances_two_columns() {
        let mut p = Performer::new(10, 2);
        feed(&mut p, "你".as_bytes());
        assert_eq!(p.grid.cell(0, 0).c(), '你');
        assert_eq!(p.grid.cursor_col, 2);
    }

    #[test]
    fn test_zero_width_char_does_not_advance_cursor() {
        let mut p = Performer::new(10, 2);
        feed(&mut p, "e\u{0301}".as_bytes());
        assert_eq!(p.grid.cursor_col, 1);
    }

    #[test]
    fn test_combining_mark_composes_with_preceding_cell() {
        let mut p = Performer::new(10, 2);
        feed(&mut p, "e\u{0301}".as_bytes());
        assert_eq!(p.grid.cell(0, 0).c(), 'é');
    }

    #[test]
    fn test_combining_mark_composes_at_pending_wrap_column() {
        let mut p = Performer::new(3, 2);
        feed(&mut p, "abe".as_bytes());
        assert!(p.grid.pending_wrap);
        feed(&mut p, "\u{0301}".as_bytes());
        assert_eq!(p.grid.cell(2, 0).c(), 'é');
        assert!(p.grid.pending_wrap);
    }

    #[test]
    fn test_bracketed_paste_mode_toggle() {
        let mut p = Performer::new(80, 24);
        assert!(!p.bracketed_paste_mode());
        feed(&mut p, b"\x1b[?2004h");
        assert!(p.bracketed_paste_mode());
        feed(&mut p, b"\x1b[?2004l");
        assert!(!p.bracketed_paste_mode());
    }

    #[test]
    fn test_primary_device_attributes_reply() {
        let mut p = Performer::new(80, 24);
        feed(&mut p, b"\x1b[c");
        assert_eq!(p.pending_writes, vec![b"\x1b[?6c".to_vec()]);
        p.pending_writes.clear();
        feed(&mut p, b"\x1b[0c");
        assert_eq!(p.pending_writes, vec![b"\x1b[?6c".to_vec()]);
    }

    #[test]
    fn test_secondary_device_attributes_reply() {
        let mut p = Performer::new(80, 24);
        feed(&mut p, b"\x1b[>c");
        assert_eq!(p.pending_writes, vec![b"\x1b[>1;10;0c".to_vec()]);
    }

    #[test]
    fn test_ris_clears_screen_scrollback_and_attrs() {
        let mut p = Performer::new(80, 4);
        // Build up state a reset must clear: scrolled-off scrollback,
        // bold+colored text, a custom scroll region, alt screen, and hidden
        // cursor with mouse tracking on.
        feed(&mut p, b"line1\r\nline2\r\nline3\r\nline4\r\nline5"); // pushes "line1" into scrollback
        assert!(
            p.grid.scrollback_len() > 0,
            "precondition: scrollback non-empty"
        );
        feed(&mut p, b"\x1b[1;31mBOLD");
        feed(&mut p, b"\x1b[2;3r"); // scroll region rows 2-3
        feed(&mut p, b"\x1b[?1049h"); // alt screen — swaps in a blank grid
        feed(&mut p, b"\x1b[?25l"); // hide cursor
        feed(&mut p, b"\x1b[?1000h"); // mouse tracking on
        assert!(p.use_alt_screen);

        feed(&mut p, b"\x1bc"); // RIS

        assert_eq!(p.grid.scrollback_len(), 0);
        assert_eq!(p.grid.cell(0, 0).c(), ' ');
        assert_eq!(p.grid.cursor_col, 0);
        assert_eq!(p.grid.cursor_row, 0);
        assert_eq!(p.grid.scroll_top, 0);
        assert_eq!(p.grid.scroll_bottom, 3);
        assert!(!p.use_alt_screen);
        assert!(p.cursor_visible);
        assert_eq!(p.mouse_tracking_mode(), MouseTrackingMode::Off);
        assert!(!p.bracketed_paste_mode());

        // Attributes reset too: next printed char should carry no SGR state.
        feed(&mut p, b"A");
        let cell = p.grid.cell(0, 0);
        assert!(!cell.bold);
        assert_eq!(cell.fg, CellColor::Default);
    }

    #[test]
    fn test_ris_preserves_scrollback_limit() {
        let mut p = Performer::new(80, 24);
        p.set_scrollback_limit(500);
        p.reset();
        assert_eq!(p.grid.scrollback_limit, 500);
    }

    #[test]
    fn test_ris_preserves_scrollback_limit_from_alt_screen() {
        let mut p = Performer::new(80, 24);
        p.set_scrollback_limit(500);
        feed(&mut p, b"\x1b[?1049h");
        p.reset();
        assert_eq!(p.grid.scrollback_limit, 500);
        assert_eq!(p.alt_grid.scrollback_limit, 500);
    }
}
