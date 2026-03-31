use vte::Perform;
use crate::cell::{Cell, CellColor};
use crate::grid::Grid;
use volt_config::Color;

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
    pub cursor_visible: bool,
    /// Bytes to write back to the PTY (e.g. cursor position reports).
    pub pending_writes: Vec<Vec<u8>>,
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
            cursor_visible: true,
            pending_writes: Vec::new(),
        }
    }

    /// Resize both the main and alt screen grids.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.grid.resize(cols, rows);
        self.alt_grid.resize(cols, rows);
    }

    fn make_cell(&self, c: char) -> Cell {
        Cell {
            c,
            fg: self.current_fg,
            bg: self.current_bg,
            bold: self.current_bold,
            italic: self.current_italic,
            underline: self.current_underline,
            dirty: true,
        }
    }

    fn reset_attrs(&mut self) {
        self.current_fg = CellColor::Default;
        self.current_bg = CellColor::Default;
        self.current_bold = false;
        self.current_italic = false;
        self.current_underline = false;
    }

    fn param(params: &vte::Params, idx: usize) -> u16 {
        params.iter().nth(idx).and_then(|s| s.first().copied()).unwrap_or(0)
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

impl Perform for Performer {
    fn print(&mut self, c: char) {
        let col = self.grid.cursor_col;
        let row = self.grid.cursor_row;
        if col < self.grid.cols && row < self.grid.rows {
            *self.grid.cell_mut(col, row) = self.make_cell(c);
        }
        self.grid.advance_cursor();
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x08 => {
                // BS
                if self.grid.cursor_col > 0 {
                    self.grid.cursor_col -= 1;
                }
            }
            0x09 => {
                // HT
                let next = (self.grid.cursor_col / 8 + 1) * 8;
                self.grid.cursor_col = next.min(self.grid.cols.saturating_sub(1));
            }
            0x0a | 0x0b | 0x0c => self.grid.newline(), // LF/VT/FF
            0x0d => self.grid.cursor_col = 0,           // CR
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
        match action {
            'A' => {
                // cursor up
                let n = Self::param(params, 0).max(1) as usize;
                self.grid.cursor_row = self.grid.cursor_row.saturating_sub(n);
            }
            'B' => {
                // cursor down
                let n = Self::param(params, 0).max(1) as usize;
                self.grid.cursor_row = (self.grid.cursor_row + n)
                    .min(self.grid.rows.saturating_sub(1));
            }
            'C' => {
                // cursor forward
                let n = Self::param(params, 0).max(1) as usize;
                self.grid.cursor_col = (self.grid.cursor_col + n)
                    .min(self.grid.cols.saturating_sub(1));
            }
            'D' => {
                // cursor back
                let n = Self::param(params, 0).max(1) as usize;
                self.grid.cursor_col = self.grid.cursor_col.saturating_sub(n);
            }
            'E' => {
                // cursor next line
                let n = Self::param(params, 0).max(1) as usize;
                self.grid.cursor_row = (self.grid.cursor_row + n)
                    .min(self.grid.rows.saturating_sub(1));
                self.grid.cursor_col = 0;
            }
            'F' => {
                // cursor prev line
                let n = Self::param(params, 0).max(1) as usize;
                self.grid.cursor_row = self.grid.cursor_row.saturating_sub(n);
                self.grid.cursor_col = 0;
            }
            'G' => {
                // cursor horizontal absolute
                let col = Self::param(params, 0).saturating_sub(1) as usize;
                self.grid.cursor_col = col.min(self.grid.cols.saturating_sub(1));
            }
            'H' | 'f' => {
                // cursor position (1-based)
                let row = Self::param(params, 0).saturating_sub(1) as usize;
                let col = Self::param(params, 1).saturating_sub(1) as usize;
                self.grid.cursor_row = row.min(self.grid.rows.saturating_sub(1));
                self.grid.cursor_col = col.min(self.grid.cols.saturating_sub(1));
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
                }
                1 => {
                    let (col, row) = (self.grid.cursor_col, self.grid.cursor_row);
                    let last_col = self.grid.cols.saturating_sub(1);
                    for r in 0..row {
                        self.grid.clear_line(r, 0, last_col);
                    }
                    self.grid.clear_line(row, 0, col);
                }
                2 | 3 => self.grid.erase_all(),
                _ => {}
            },
            'K' => match Self::param(params, 0) {
                // erase in line
                0 => {
                    let (col, row) = (self.grid.cursor_col, self.grid.cursor_row);
                    let last_col = self.grid.cols.saturating_sub(1);
                    self.grid.clear_line(row, col, last_col);
                }
                1 => {
                    let (col, row) = (self.grid.cursor_col, self.grid.cursor_row);
                    self.grid.clear_line(row, 0, col);
                }
                2 => {
                    let row = self.grid.cursor_row;
                    let last_col = self.grid.cols.saturating_sub(1);
                    self.grid.clear_line(row, 0, last_col);
                }
                _ => {}
            },
            'L' => {
                // insert lines
                let n = Self::param(params, 0).max(1) as usize;
                let row = self.grid.cursor_row;
                let bottom = self.grid.scroll_bottom;
                self.grid.scroll_down(row, bottom, n);
            }
            'M' => {
                // delete lines
                let n = Self::param(params, 0).max(1) as usize;
                let row = self.grid.cursor_row;
                let bottom = self.grid.scroll_bottom;
                self.grid.scroll_up(row, bottom, n);
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
            }
            'S' => {
                // scroll up (pan up): content moves up, blank lines appear at bottom
                let n = Self::param(params, 0).max(1) as usize;
                let top = self.grid.scroll_top;
                let bot = self.grid.scroll_bottom;
                self.grid.scroll_up(top, bot, n);
            }
            'T' => {
                // scroll down (pan down): content moves down, blank lines appear at top
                let n = Self::param(params, 0).max(1) as usize;
                let top = self.grid.scroll_top;
                let bot = self.grid.scroll_bottom;
                self.grid.scroll_down(top, bot, n);
            }
            'n' => {
                // DSR — device status report
                if Self::param(params, 0) == 6 {
                    // CPR: respond with ESC[row;colR (1-based)
                    let row = self.grid.cursor_row + 1;
                    let col = self.grid.cursor_col + 1;
                    self.pending_writes.push(format!("\x1b[{};{}R", row, col).into_bytes());
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
            }
            'h' if private => {
                match Self::param(params, 0) {
                    1049 => self.enter_alt_screen(),
                    25   => self.cursor_visible = true,
                    _    => {}
                }
            }
            'l' if private => {
                match Self::param(params, 0) {
                    1049 => self.exit_alt_screen(),
                    25   => self.cursor_visible = false,
                    _    => {}
                }
            }
            'm' => {
                // SGR
                let mut iter = params.iter().peekable();
                if iter.peek().is_none() { self.reset_attrs(); return; }
                while let Some(subparams) = iter.next() {
                    let p = subparams.first().copied().unwrap_or(0);
                    match p {
                        0 => self.reset_attrs(),
                        1 => self.current_bold = true,
                        3 => self.current_italic = true,
                        4 => self.current_underline = true,
                        22 => self.current_bold = false,
                        23 => self.current_italic = false,
                        24 => self.current_underline = false,
                        30..=37 => self.current_fg = CellColor::Indexed(p as u8 - 30),
                        39 => self.current_fg = CellColor::Default,
                        40..=47 => self.current_bg = CellColor::Indexed(p as u8 - 40),
                        49 => self.current_bg = CellColor::Default,
                        90..=97  => self.current_fg = CellColor::Indexed(p as u8 - 90 + 8),
                        100..=107 => self.current_bg = CellColor::Indexed(p as u8 - 100 + 8),
                        38 | 48 => {
                            let is_fg = p == 38;
                            if subparams.len() >= 3 && subparams[1] == 5 {
                                let idx = subparams[2] as u8;
                                if is_fg { self.current_fg = CellColor::Indexed(idx); }
                                else      { self.current_bg = CellColor::Indexed(idx); }
                            } else if subparams.len() >= 5 && subparams[1] == 2 {
                                let c = Color::rgb(subparams[2] as u8, subparams[3] as u8, subparams[4] as u8);
                                if is_fg { self.current_fg = CellColor::Rgb(c); }
                                else      { self.current_bg = CellColor::Rgb(c); }
                            } else if let Some(next) = iter.next() {
                                match next.first().copied().unwrap_or(0) {
                                    5 => {
                                        if let Some(idx_param) = iter.next() {
                                            let idx = idx_param.first().copied().unwrap_or(0) as u8;
                                            if is_fg { self.current_fg = CellColor::Indexed(idx); }
                                            else      { self.current_bg = CellColor::Indexed(idx); }
                                        }
                                    }
                                    2 => {
                                        let r = iter.next().and_then(|s| s.first().copied()).unwrap_or(0) as u8;
                                        let g = iter.next().and_then(|s| s.first().copied()).unwrap_or(0) as u8;
                                        let b = iter.next().and_then(|s| s.first().copied()).unwrap_or(0) as u8;
                                        let c = Color::rgb(r, g, b);
                                        if is_fg { self.current_fg = CellColor::Rgb(c); }
                                        else      { self.current_bg = CellColor::Rgb(c); }
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
        if params.len() >= 2 && (params[0] == b"0" || params[0] == b"2") {
            let _title = String::from_utf8_lossy(params[1]).to_string();
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        match byte {
            b'7' => {
                // DEC save cursor
                self.saved_cursor_col = self.grid.cursor_col;
                self.saved_cursor_row = self.grid.cursor_row;
            }
            b'8' => {
                // DEC restore cursor
                self.grid.cursor_col = self.saved_cursor_col.min(self.grid.cols.saturating_sub(1));
                self.grid.cursor_row = self.saved_cursor_row.min(self.grid.rows.saturating_sub(1));
            }
            b'M' => {
                // reverse index
                if self.grid.cursor_row == self.grid.scroll_top {
                    let top = self.grid.scroll_top;
                    let bottom = self.grid.scroll_bottom;
                    self.grid.scroll_down(top, bottom, 1);
                } else if self.grid.cursor_row > 0 {
                    self.grid.cursor_row -= 1;
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
    fn test_cursor_position_report() {
        let mut p = Performer::new(80, 24);
        p.grid.cursor_row = 3;
        p.grid.cursor_col = 7;
        feed(&mut p, b"\x1b[6n");
        assert_eq!(p.pending_writes, vec![b"\x1b[4;8R".to_vec()]);
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
}
