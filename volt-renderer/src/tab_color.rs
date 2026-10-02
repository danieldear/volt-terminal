//! User-selected tab accents. These are session-local visual labels, not
//! terminal status: the running/idle indicator remains independent.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabColor {
    Red,
    Orange,
    Amber,
    Green,
    Teal,
    Blue,
    Purple,
    Pink,
}

impl TabColor {
    pub const ALL: [Self; 8] = [
        Self::Red,
        Self::Orange,
        Self::Amber,
        Self::Green,
        Self::Teal,
        Self::Blue,
        Self::Purple,
        Self::Pink,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Orange => "orange",
            Self::Amber => "amber",
            Self::Green => "green",
            Self::Teal => "teal",
            Self::Blue => "blue",
            Self::Purple => "purple",
            Self::Pink => "pink",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Red => "Red",
            Self::Orange => "Orange",
            Self::Amber => "Amber",
            Self::Green => "Green",
            Self::Teal => "Teal",
            Self::Blue => "Blue",
            Self::Purple => "Purple",
            Self::Pink => "Pink",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|color| color.id() == id)
    }

    /// Normalized UI accent used only in the tab chrome, not terminal cells.
    pub const fn rgb(self) -> [f32; 3] {
        match self {
            Self::Red => [0.93, 0.38, 0.42],
            Self::Orange => [0.96, 0.57, 0.31],
            Self::Amber => [0.96, 0.73, 0.34],
            Self::Green => [0.43, 0.78, 0.49],
            Self::Teal => [0.34, 0.77, 0.74],
            Self::Blue => [0.40, 0.65, 0.95],
            Self::Purple => [0.66, 0.51, 0.92],
            Self::Pink => [0.93, 0.49, 0.70],
        }
    }
}
