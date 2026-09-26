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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cell {
    // Scalar or grid-local grapheme ID, plus width flags. Keep Cell 16 bytes
    // and Copy: ASCII scrolling/blitting stays a row-slice memcpy.
    pub(crate) text: u32,
    pub fg: CellColor,
    pub bg: CellColor,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub reverse: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            text: ' ' as u32,
            fg: CellColor::Default,
            bg: CellColor::Default,
            bold: false,
            italic: false,
            underline: false,
            reverse: false,
        }
    }
}

impl Cell {
    const CLUSTER: u32 = 1 << 31;
    const WIDE: u32 = 1 << 30;
    const CONTINUATION: u32 = 1 << 29;
    const WRAP_SPACER: u32 = 1 << 28;
    const CLIPPED_WIDE: u32 = 1 << 27;
    const VALUE: u32 = Self::CLIPPED_WIDE - 1;

    pub fn c(&self) -> char {
        if self.is_continuation() || self.is_wrap_spacer() {
            return ' ';
        }
        if self.cluster_id().is_some() {
            return '\u{fffd}';
        }
        char::from_u32(self.text & Self::VALUE).unwrap_or('\u{fffd}')
    }
    pub fn set_char(&mut self, c: char) {
        self.text = c as u32;
    }
    pub fn is_wrap_spacer(&self) -> bool {
        self.text & Self::WRAP_SPACER != 0
    }
    pub fn wrap_spacer() -> Self {
        Self {
            text: Self::WRAP_SPACER,
            ..Self::default()
        }
    }
    pub(crate) fn is_clipped_wide(&self) -> bool {
        self.text & Self::CLIPPED_WIDE != 0
    }
    pub(crate) fn set_clipped_wide(&mut self, clipped: bool) {
        self.text =
            (self.text & !Self::CLIPPED_WIDE) | if clipped { Self::CLIPPED_WIDE } else { 0 };
    }
    pub fn is_wide(&self) -> bool {
        self.text & Self::WIDE != 0
    }
    pub fn is_continuation(&self) -> bool {
        self.text & Self::CONTINUATION != 0
    }
    pub fn width(&self) -> usize {
        if self.is_continuation() {
            0
        } else if self.is_wide() {
            2
        } else {
            1
        }
    }
    pub fn set_wide(&mut self, wide: bool) {
        self.text = (self.text & !Self::WIDE) | if wide { Self::WIDE } else { 0 };
    }
    pub fn continuation(mut self) -> Self {
        self.text = Self::CONTINUATION;
        self
    }
    pub(crate) fn cluster_id(&self) -> Option<usize> {
        (self.text & Self::CLUSTER != 0).then_some((self.text & Self::VALUE) as usize)
    }
    pub(crate) fn set_cluster(&mut self, id: usize) {
        assert!(id <= Self::VALUE as usize);
        self.text = (self.text & (Self::WIDE | Self::CLIPPED_WIDE)) | Self::CLUSTER | id as u32;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn compact_copyable_cell_is_preserved() {
        assert_eq!(std::mem::size_of::<super::Cell>(), 16);
        fn is_copy<T: Copy>() {}
        is_copy::<super::Cell>();
    }
}
