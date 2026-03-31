use volt_config::{Color, Theme};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CellColor {
    Default,
    Indexed(u8),
    Rgb(Color),
}

impl CellColor {
    pub fn resolve_fg(&self, theme: &Theme) -> Color {
        match self {
            CellColor::Default => theme.foreground,
            CellColor::Indexed(i) => theme.ansi_color(*i),
            CellColor::Rgb(c) => *c,
        }
    }

    pub fn resolve_bg(&self, theme: &Theme) -> Color {
        match self {
            CellColor::Default => theme.background,
            CellColor::Indexed(i) => theme.ansi_color(*i),
            CellColor::Rgb(c) => *c,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub c: char,
    pub fg: CellColor,
    pub bg: CellColor,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub dirty: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: CellColor::Default,
            bg: CellColor::Default,
            bold: false,
            italic: false,
            underline: false,
            dirty: true,
        }
    }
}
