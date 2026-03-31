use vte::Perform;
use crate::cell::{Cell, CellColor};
use crate::grid::Grid;
use volt_config::Color;

pub struct Performer {
    pub grid: Grid,
    current_fg: CellColor,
    current_bg: CellColor,
    current_bold: bool,
    current_italic: bool,
    current_underline: bool,
}

impl Performer {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            grid: Grid::new(cols, rows),
            current_fg: CellColor::Default,
            current_bg: CellColor::Default,
            current_bold: false,
            current_italic: false,
            current_underline: false,
        }
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
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
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
                2 | 3 => self.grid.clear_screen(),
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
            'm' => {
                // SGR
                let params_vec: Vec<u16> = params
                    .iter()
                    .filter_map(|s| s.first().copied())
                    .collect();
                if params_vec.is_empty() {
                    self.reset_attrs();
                    return;
                }
                let mut i = 0;
                while i < params_vec.len() {
                    match params_vec[i] {
                        0 => self.reset_attrs(),
                        1 => self.current_bold = true,
                        3 => self.current_italic = true,
                        4 => self.current_underline = true,
                        22 => self.current_bold = false,
                        23 => self.current_italic = false,
                        24 => self.current_underline = false,
                        30..=37 => {
                            self.current_fg = CellColor::Indexed(params_vec[i] as u8 - 30)
                        }
                        39 => self.current_fg = CellColor::Default,
                        40..=47 => {
                            self.current_bg = CellColor::Indexed(params_vec[i] as u8 - 40)
                        }
                        49 => self.current_bg = CellColor::Default,
                        90..=97 => {
                            self.current_fg = CellColor::Indexed(params_vec[i] as u8 - 90 + 8)
                        }
                        100..=107 => {
                            self.current_bg = CellColor::Indexed(params_vec[i] as u8 - 100 + 8)
                        }
                        38 | 48 => {
                            let is_fg = params_vec[i] == 38;
                            if i + 2 < params_vec.len() && params_vec[i + 1] == 5 {
                                let idx = params_vec[i + 2] as u8;
                                if is_fg {
                                    self.current_fg = CellColor::Indexed(idx);
                                } else {
                                    self.current_bg = CellColor::Indexed(idx);
                                }
                                i += 2;
                            } else if i + 4 < params_vec.len() && params_vec[i + 1] == 2 {
                                let c = Color::rgb(
                                    params_vec[i + 2] as u8,
                                    params_vec[i + 3] as u8,
                                    params_vec[i + 4] as u8,
                                );
                                if is_fg {
                                    self.current_fg = CellColor::Rgb(c);
                                } else {
                                    self.current_bg = CellColor::Rgb(c);
                                }
                                i += 4;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
            _ => {}
        }
    }

    fn hook(&mut self, _: &vte::Params, _: &[u8], _: bool, _: char) {}
    fn put(&mut self, _: u8) {}
    fn unhook(&mut self) {}

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        // OSC 0 / OSC 2: set window title
        if params.len() >= 2 && (params[0] == b"0" || params[0] == b"2") {
            let _title = String::from_utf8_lossy(params[1]).to_string();
            // title event will be wired up in Task 5 when we have a channel
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        match byte {
            b'M' => {
                // reverse index: scroll region down by 1 if at scroll_top, else move cursor up
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
        assert_eq!(p.grid.cursor_row, 4); // 1-based -> 0-based
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
        feed(&mut p, b"\x1b[31mA"); // red fg = index 1
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
}
