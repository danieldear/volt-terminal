use std::path::PathBuf;

use crate::cell::{Cell, CellColor};
use crate::events::CoreEvent;
use crate::grid::Grid;
use unicode_width::UnicodeWidthChar;
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
}

impl Performer {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            grid: Grid::new(cols, rows),
            use_alt_screen: false,
            alt_grid: Grid::new(cols, rows),
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
        }
    }

    /// Resize both the main and alt screen grids.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.grid.resize(cols, rows);
        self.alt_grid.resize(cols, rows);
    }

    /// Set the scrollback line limit on the main grid.
    /// The alt screen never accumulates scrollback, so only the main grid is updated.
    pub fn set_scrollback_limit(&mut self, limit: usize) {
        // Treat 0 as "no scrollback"; clamp to 1 so ring-buffer modulo never panics.
        self.grid.scrollback_limit = limit.max(1);
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

    pub fn take_damage_rows(&mut self) -> Option<(usize, usize)> {
        self.damage_rows.take()
    }

    fn make_cell(&self, c: char) -> Cell {
        Cell {
            c,
            fg: self.current_fg,
            bg: self.current_bg,
            bold: self.current_bold,
            italic: self.current_italic,
            underline: self.current_underline,
            reverse: self.current_reverse,
            dirty: true,
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

    fn enter_alt_screen(&mut self) {
        if !self.use_alt_screen {
            self.saved_cursor_col = self.grid.cursor_col;
            self.saved_cursor_row = self.grid.cursor_row;
            let cols = self.grid.cols;
            let rows = self.grid.rows;
            self.alt_grid.resize(cols, rows);
            std::mem::swap(&mut self.grid, &mut self.alt_grid);
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
    fn print(&mut self, c: char) {
        let old_cursor_row = self.grid.cursor_row;
        let width = char_display_width(c);
        if width == 0 {
            return;
        }

        // Deferred wrap: fire the pending wrap before placing the new character.
        if self.grid.pending_wrap {
            self.grid.pending_wrap = false;
            self.grid.cursor_col = 0;
            let will_scroll = self.grid.cursor_row == self.grid.scroll_bottom;
            self.grid.newline();
            if will_scroll {
                self.mark_dirty_range(self.grid.scroll_top, self.grid.scroll_bottom);
            }
        }

        if width == 2 && self.grid.cursor_col + 1 >= self.grid.cols {
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
            *self.grid.cell_mut(col, row) = self.make_cell(c);
            if width == 2 && col + 1 < self.grid.cols {
                *self.grid.cell_mut(col + 1, row) = self.make_cell(' ');
            }
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
        self.mark_cursor_row_change(old_cursor_row);
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
                        Cell::default()
                    };
                    *self.grid.cell_mut(c, row) = src;
                }
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
                        Cell::default()
                    };
                    *self.grid.cell_mut(c, row) = src;
                }
                self.mark_dirty_row(row);
            }
            'X' => {
                // erase N characters at cursor position (no cursor movement)
                let n = Self::param(params, 0).max(1) as usize;
                let row = self.grid.cursor_row;
                let col = self.grid.cursor_col;
                let cols = self.grid.cols;
                for c in col..(col + n).min(cols) {
                    *self.grid.cell_mut(c, row) = Cell::default();
                }
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
            'n' => {
                // DSR — device status report
                if Self::param(params, 0) == 6 {
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
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(p: &mut Performer, bytes: &[u8]) {
        let mut parser = vte::Parser::new();
        for &b in bytes {
            parser.advance(p, b);
        }
    }

    #[test]
    fn test_print_places_char() {
        let mut p = Performer::new(80, 24);
        feed(&mut p, b"AB");
        assert_eq!(p.grid.cell(0, 0).c, 'A');
        assert_eq!(p.grid.cell(1, 0).c, 'B');
        assert_eq!(p.grid.cursor_col, 2);
    }

    #[test]
    fn test_cr_lf_moves_cursor() {
        let mut p = Performer::new(80, 24);
        feed(&mut p, b"A\r\nB");
        assert_eq!(p.grid.cell(0, 0).c, 'A');
        assert_eq!(p.grid.cell(0, 1).c, 'B');
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
            assert_eq!(p.grid.cell(col, 0).c, ' ');
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
        assert_eq!(p.grid.cell(0, 0).c, 'H');

        // Enter alt screen — grid should be cleared
        feed(&mut p, b"\x1b[?1049h");
        assert!(p.use_alt_screen);
        assert_eq!(p.grid.cell(0, 0).c, ' ');

        // Write something on alt screen
        feed(&mut p, b"Alt");
        assert_eq!(p.grid.cell(0, 0).c, 'A');

        // Leave alt screen — main screen restored
        feed(&mut p, b"\x1b[?1049l");
        assert!(!p.use_alt_screen);
        assert_eq!(p.grid.cell(0, 0).c, 'H');
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
    fn test_wide_char_advances_two_columns() {
        let mut p = Performer::new(10, 2);
        feed(&mut p, "你".as_bytes());
        assert_eq!(p.grid.cell(0, 0).c, '你');
        assert_eq!(p.grid.cursor_col, 2);
    }

    #[test]
    fn test_zero_width_char_does_not_advance_cursor() {
        let mut p = Performer::new(10, 2);
        feed(&mut p, "e\u{0301}".as_bytes());
        assert_eq!(p.grid.cursor_col, 1);
    }
}
