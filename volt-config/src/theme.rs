#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self { Self { r, g, b } }
    pub fn to_f32(self) -> [f32; 4] {
        [self.r as f32 / 255.0, self.g as f32 / 255.0, self.b as f32 / 255.0, 1.0]
    }
}

impl Default for Color {
    fn default() -> Self { Self::rgb(0, 0, 0) }
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub cursor:     Color,
    pub ansi: [Color; 16],  // indices 0-7 normal, 8-15 bright
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            background: Color::rgb(24,  24,  37),
            foreground: Color::rgb(202, 211, 245),
            cursor:     Color::rgb(202, 211, 245),
            ansi: [
                // normal
                Color::rgb(30,  30,  46),   // black
                Color::rgb(243, 139, 168),  // red
                Color::rgb(166, 227, 161),  // green
                Color::rgb(249, 226, 175),  // yellow
                Color::rgb(137, 180, 250),  // blue
                Color::rgb(203, 166, 247),  // magenta
                Color::rgb(137, 220, 235),  // cyan
                Color::rgb(166, 173, 200),  // white
                // bright
                Color::rgb(88,  91,  112),  // bright black
                Color::rgb(243, 139, 168),  // bright red
                Color::rgb(166, 227, 161),  // bright green
                Color::rgb(249, 226, 175),  // bright yellow
                Color::rgb(137, 180, 250),  // bright blue
                Color::rgb(203, 166, 247),  // bright magenta
                Color::rgb(137, 220, 235),  // bright cyan
                Color::rgb(186, 194, 222),  // bright white
            ],
        }
    }

    pub fn ansi_color(&self, index: u8) -> Color {
        match index {
            0..=15 => self.ansi[index as usize],
            16..=231 => {
                let i = index - 16;
                let level = |v: u8| if v == 0 { 0u8 } else { 55 + v * 40 };
                let b = level(i % 6);
                let g = level((i / 6) % 6);
                let r = level(i / 36);
                Color::rgb(r, g, b)
            }
            232..=255 => {
                let v = 8 + (index - 232) * 10;
                Color::rgb(v, v, v)
            }
        }
    }
}
