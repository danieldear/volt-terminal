//! Rust-only layout, text and interaction extensions (Tailwind-style
//! utilities). Kept out of the JSON schema: schema 1 is unchanged.
use super::responsive::Responsive;

/// Main-axis distribution of free space in a row or column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    /// First and last items at the edges, equal space between.
    SpaceBetween,
    /// Equal space around each item (half at the edges).
    SpaceAround,
    /// Equal space between items and at both edges.
    SpaceEvenly,
}

/// Horizontal alignment of text lines inside their box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TextAlign {
    #[default]
    Start,
    Center,
    End,
    Justified,
}

/// Mouse cursor shown while the pointer is over an element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cursor {
    Default,
    Pointer,
    Text,
    Move,
    Grab,
    Grabbing,
    NotAllowed,
    Wait,
    Crosshair,
    ResizeColumn,
    ResizeRow,
    ZoomIn,
    Help,
}

/// How a split pane's divider is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SplitHandle {
    /// A visible bar with a grip (default).
    #[default]
    Bar,
    /// Invisible at rest; a rounded pill appears on hover, drag or keyboard
    /// focus, as in desktop sidebars.
    Reveal,
}

/// How a border is stroked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BorderStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

/// Which edges draw a border.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sides(pub(crate) u8);
impl Sides {
    pub const TOP: Sides = Sides(1);
    pub const RIGHT: Sides = Sides(2);
    pub const BOTTOM: Sides = Sides(4);
    pub const LEFT: Sides = Sides(8);
    pub const ALL: Sides = Sides(15);
    pub const HORIZONTAL: Sides = Sides(2 | 8);
    pub const VERTICAL: Sides = Sides(1 | 4);
    pub fn contains(self, other: Sides) -> bool {
        self.0 & other.0 == other.0
    }
}
impl std::ops::BitOr for Sides {
    type Output = Sides;
    fn bitor(self, rhs: Self) -> Self {
        Sides(self.0 | rhs.0)
    }
}

/// Grid columns: a fixed count, or as many as fit at a minimum width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridColumns {
    Count(u16),
    /// Responsive auto-fit: as many equal columns as fit at this logical width.
    MinWidth(f32),
}

/// A gradient background, resolved from theme tokens at paint time.
#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    pub(crate) kind: GradientKind,
    pub(crate) stops: Vec<(f32, String)>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum GradientKind {
    /// Angle in degrees: 0 = to top, 90 = to right, 180 = to bottom.
    Linear(f32),
    /// Centre as fractions of the box; radius reaches the farthest corner.
    Radial([f32; 2]),
}
impl Gradient {
    /// `angle` in degrees, CSS-style: 0° points up, 90° right, 180° down.
    pub fn linear(angle: f32, stops: impl IntoIterator<Item = (f32, impl Into<String>)>) -> Self {
        Self {
            kind: GradientKind::Linear(angle),
            stops: stops.into_iter().map(|(at, c)| (at, c.into())).collect(),
        }
    }
    /// Radial from a centre given as fractions of the box (0.5, 0.5 = middle).
    pub fn radial(
        centre: [f32; 2],
        stops: impl IntoIterator<Item = (f32, impl Into<String>)>,
    ) -> Self {
        Self {
            kind: GradientKind::Radial(centre),
            stops: stops.into_iter().map(|(at, c)| (at, c.into())).collect(),
        }
    }
    pub(crate) fn valid(&self) -> bool {
        (2..=16).contains(&self.stops.len())
            && self
                .stops
                .iter()
                .all(|(at, c)| at.is_finite() && (0. ..=1.).contains(at) && !c.is_empty())
            && self.stops.windows(2).all(|w| w[0].0 <= w[1].0)
            && match self.kind {
                GradientKind::Linear(a) => a.is_finite(),
                GradientKind::Radial(c) => c.iter().all(|v| v.is_finite()),
            }
    }
}

/// Rust-only extensions carried by every node. Defaults change nothing.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NodeExtras {
    /// Logical min/max width and height.
    pub min: [Option<f32>; 2],
    pub max: [Option<f32>; 2],
    /// Share of the parent's available space, 0..=1, per axis.
    pub fraction: [Option<f32>; 2],
    /// Width / height.
    pub aspect: Option<f32>,
    pub justify: Option<Justify>,
    pub wrap: bool,
    pub grid: Option<GridColumns>,
    pub span: u16,
    pub cursor: Option<Cursor>,
    /// Paint-time translation in logical pixels (hit targets unchanged).
    pub offset: [f32; 2],
    /// Responsive visibility; hidden nodes take no space and are not painted.
    pub visible: Option<Responsive<bool>>,
    pub responsive_width: Option<Responsive<f32>>,
    pub responsive_font_size: Option<Responsive<f32>>,
    /// Scroll views: reveal this key whenever the request number changes.
    pub scroll_to: Option<(String, u32)>,
    pub border_sides: Option<Sides>,
    pub border_style: BorderStyle,
    pub gradient: Option<Gradient>,
    /// Buttons: logical inset of the label from the leading edge.
    pub label_inset: Option<f32>,
    /// Split dividers: drawn only while hovered, dragged or focused.
    pub reveal_handle: bool,
}
impl NodeExtras {
    pub(crate) fn valid(&self) -> bool {
        let size = |v: &Option<f32>| v.is_none_or(|v| v.is_finite() && (0. ..=16384.).contains(&v));
        self.min.iter().chain(&self.max).all(size)
            && self
                .fraction
                .iter()
                .all(|f| f.is_none_or(|f| f.is_finite() && (0. ..=1.).contains(&f)))
            && self
                .aspect
                .is_none_or(|a| a.is_finite() && (0.01..=100.).contains(&a))
            && self
                .offset
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 4096.)
            && self.span <= 64
            && match self.grid {
                Some(GridColumns::Count(n)) => (1..=64).contains(&n),
                Some(GridColumns::MinWidth(w)) => w.is_finite() && (1. ..=4096.).contains(&w),
                None => true,
            }
            && self.gradient.as_ref().is_none_or(Gradient::valid)
            && self
                .label_inset
                .is_none_or(|v| v.is_finite() && (0. ..=512.).contains(&v))
            && self
                .visible
                .as_ref()
                .is_none_or(|r| r.validate("visible", |_| true).is_ok())
            && self.responsive_width.as_ref().is_none_or(|r| {
                r.validate("responsive_width", |v| {
                    v.is_finite() && (0. ..=16384.).contains(v)
                })
                .is_ok()
            })
            && self.responsive_font_size.as_ref().is_none_or(|r| {
                r.validate("responsive_font_size", |v| {
                    v.is_finite() && (8. ..=96.).contains(v)
                })
                .is_ok()
            })
    }
}
