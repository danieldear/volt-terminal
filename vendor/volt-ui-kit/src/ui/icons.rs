//! A built-in vector icon set: original line drawings on a 24-unit grid,
//! stroked at 1.75 units with round geometry. Theme-aware and crisp at any
//! size; drawn through `VectorPath::from_svg`.
//!
//! ```ignore
//! icon(Icon::Search).size(16.).color(ColorToken::Muted)
//! ```
use super::*;

/// Glyphs in the built-in set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    Search,
    Plus,
    Minus,
    Close,
    Check,
    ChevronUp,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    ChevronsLeft,
    ChevronsRight,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Menu,
    MoreHorizontal,
    MoreVertical,
    Home,
    Settings,
    User,
    Users,
    Bell,
    Mail,
    Calendar,
    Clock,
    Star,
    StarFilled,
    Heart,
    Bookmark,
    Trash,
    Edit,
    Copy,
    Download,
    Upload,
    Link,
    ExternalLink,
    Eye,
    EyeOff,
    Lock,
    Unlock,
    Info,
    Warning,
    Error,
    Success,
    Help,
    Sun,
    Moon,
    Image,
    File,
    Folder,
    Filter,
    Refresh,
    Play,
    Pause,
    Grid,
    List,
    Sliders,
    Globe,
    Bolt,
    Command,
    Inbox,
    Send,
    Palette,
    Upload2,
}

/// Stroked SVG path data and filled path data for each glyph.
fn paths(icon: Icon) -> (&'static [&'static str], &'static [&'static str]) {
    use Icon::*;
    match icon {
        Search => (&["M10.5 4a6.5 6.5 0 1 0 0 13a6.5 6.5 0 1 0 0-13z", "M15.5 15.5L20 20"], &[]),
        Plus => (&["M12 5v14", "M5 12h14"], &[]),
        Minus => (&["M5 12h14"], &[]),
        Close => (&["M6 6l12 12", "M18 6L6 18"], &[]),
        Check => (&["M5 12.5l4.5 4.5L19 7.5"], &[]),
        ChevronUp => (&["M6 15l6-6l6 6"], &[]),
        ChevronDown => (&["M6 9l6 6l6-6"], &[]),
        ChevronLeft => (&["M15 6l-6 6l6 6"], &[]),
        ChevronRight => (&["M9 6l6 6l-6 6"], &[]),
        ChevronsLeft => (&["M11 6l-6 6l6 6", "M18 6l-6 6l6 6"], &[]),
        ChevronsRight => (&["M6 6l6 6l-6 6", "M13 6l6 6l-6 6"], &[]),
        ArrowUp => (&["M12 19V5", "M6 11l6-6l6 6"], &[]),
        ArrowDown => (&["M12 5v14", "M6 13l6 6l6-6"], &[]),
        ArrowLeft => (&["M19 12H5", "M11 6l-6 6l6 6"], &[]),
        ArrowRight => (&["M5 12h14", "M13 6l6 6l-6 6"], &[]),
        Menu => (&["M4 7h16", "M4 12h16", "M4 17h16"], &[]),
        MoreHorizontal => (
            &[],
            &[
                "M6 10.5a1.5 1.5 0 1 1 0 3a1.5 1.5 0 1 1 0-3z",
                "M12 10.5a1.5 1.5 0 1 1 0 3a1.5 1.5 0 1 1 0-3z",
                "M18 10.5a1.5 1.5 0 1 1 0 3a1.5 1.5 0 1 1 0-3z",
            ],
        ),
        MoreVertical => (
            &[],
            &[
                "M12 4.5a1.5 1.5 0 1 1 0 3a1.5 1.5 0 1 1 0-3z",
                "M12 10.5a1.5 1.5 0 1 1 0 3a1.5 1.5 0 1 1 0-3z",
                "M12 16.5a1.5 1.5 0 1 1 0 3a1.5 1.5 0 1 1 0-3z",
            ],
        ),
        Home => (&["M4 10.5L12 4l8 6.5V19a1 1 0 0 1-1 1h-4.5v-6h-5v6H5a1 1 0 0 1-1-1z"], &[]),
        Settings => (
            &[
                "M12 9a3 3 0 1 0 0 6a3 3 0 1 0 0-6z",
                "M12 3v2.5M12 18.5V21M3 12h2.5M18.5 12H21M5.6 5.6l1.8 1.8M16.6 16.6l1.8 1.8M5.6 18.4l1.8-1.8M16.6 7.4l1.8-1.8",
            ],
            &[],
        ),
        User => (&["M12 4a4 4 0 1 0 0 8a4 4 0 1 0 0-8z", "M4.5 20a7.5 7.5 0 0 1 15 0"], &[]),
        Users => (
            &[
                "M9 5a3.5 3.5 0 1 0 0 7a3.5 3.5 0 1 0 0-7z",
                "M2.5 19.5a6.5 6.5 0 0 1 13 0",
                "M15.5 5.3a3.5 3.5 0 0 1 0 6.4",
                "M17.5 14.2a6.5 6.5 0 0 1 4 5.3",
            ],
            &[],
        ),
        Bell => (&["M6 16.5V11a6 6 0 0 1 12 0v5.5l1.5 1.5h-15z", "M10 20.5h4"], &[]),
        Mail => (&["M4 6h16v12H4z", "M4 7l8 6l8-6"], &[]),
        Calendar => (&["M4 6h16v14H4z", "M4 10h16", "M8 3.5V7", "M16 3.5V7"], &[]),
        Clock => (&["M12 3.5a8.5 8.5 0 1 0 0 17a8.5 8.5 0 1 0 0-17z", "M12 7.5V12l3 2"], &[]),
        Star => (&["M12 3.8l2.5 5.2l5.7.8l-4.1 4l1 5.6L12 16.7l-5.1 2.7l1-5.6l-4.1-4l5.7-.8z"], &[]),
        StarFilled => (&[], &["M12 3.8l2.5 5.2l5.7.8l-4.1 4l1 5.6L12 16.7l-5.1 2.7l1-5.6l-4.1-4l5.7-.8z"]),
        Heart => (&["M12 19.5C5 15 3.5 11.5 3.5 9a4.5 4.5 0 0 1 8.5-2a4.5 4.5 0 0 1 8.5 2c0 2.5-1.5 6-8.5 10.5z"], &[]),
        Bookmark => (&["M6.5 4h11v16l-5.5-4l-5.5 4z"], &[]),
        Trash => (&["M4.5 6.5h15", "M9.5 6.5V4.5h5v2", "M6.5 6.5l1 13h9l1-13", "M10 10.5v6", "M14 10.5v6"], &[]),
        Edit => (&["M15.5 4.5l4 4L9 19H5v-4z", "M13.5 6.5l4 4"], &[]),
        Copy => (&["M9 9h11v11H9z", "M15 9V4H4v11h5"], &[]),
        Download => (&["M12 4v11", "M7 10l5 5l5-5", "M4.5 19.5h15"], &[]),
        Upload => (&["M12 15V4", "M7 9l5-5l5 5", "M4.5 19.5h15"], &[]),
        Upload2 => (&["M12 16V8", "M8 11.5l4-4l4 4", "M7 19.5a5 5 0 0 1-1-9.9a6.5 6.5 0 0 1 12.6 1.4A4.3 4.3 0 0 1 17.5 19.5z"], &[]),
        Link => (
            &[
                "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1",
                "M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1",
            ],
            &[],
        ),
        ExternalLink => (&["M13.5 4.5H19.5v6", "M19.5 4.5L11 13", "M17 14v5H5V7h5"], &[]),
        Eye => (
            &[
                "M2.5 12s3.5-6.5 9.5-6.5s9.5 6.5 9.5 6.5s-3.5 6.5-9.5 6.5S2.5 12 2.5 12z",
                "M12 9a3 3 0 1 0 0 6a3 3 0 1 0 0-6z",
            ],
            &[],
        ),
        EyeOff => (
            &[
                "M4 4l16 16",
                "M9.9 6a9.6 9.6 0 0 1 2.1-.5c6 0 9.5 6.5 9.5 6.5a16 16 0 0 1-2.6 3.4",
                "M6.6 7.4A15.5 15.5 0 0 0 2.5 12s3.5 6.5 9.5 6.5a9 9 0 0 0 4.6-1.3",
                "M10 10a3 3 0 0 0 4 4",
            ],
            &[],
        ),
        Lock => (&["M5.5 11h13v9h-13z", "M8.5 11V8a3.5 3.5 0 0 1 7 0v3"], &[]),
        Unlock => (&["M5.5 11h13v9h-13z", "M8.5 11V8a3.5 3.5 0 0 1 6.8-1.2"], &[]),
        Info => (
            &["M12 3.5a8.5 8.5 0 1 0 0 17a8.5 8.5 0 1 0 0-17z", "M12 11v5.5"],
            &["M12 6.8a1.2 1.2 0 1 1 0 2.4a1.2 1.2 0 1 1 0-2.4z"],
        ),
        Warning => (
            &["M12 4L21 19.5H3z", "M12 10v4.5"],
            &["M12 16.3a1.1 1.1 0 1 1 0 2.2a1.1 1.1 0 1 1 0-2.2z"],
        ),
        Error => (
            &["M12 3.5a8.5 8.5 0 1 0 0 17a8.5 8.5 0 1 0 0-17z", "M9 9l6 6", "M15 9l-6 6"],
            &[],
        ),
        Success => (
            &["M12 3.5a8.5 8.5 0 1 0 0 17a8.5 8.5 0 1 0 0-17z", "M8 12.3l2.7 2.7L16 9.5"],
            &[],
        ),
        Help => (
            &[
                "M12 3.5a8.5 8.5 0 1 0 0 17a8.5 8.5 0 1 0 0-17z",
                "M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6v.6",
            ],
            &["M12 16.2a1.1 1.1 0 1 1 0 2.2a1.1 1.1 0 1 1 0-2.2z"],
        ),
        Sun => (
            &[
                "M12 8a4 4 0 1 0 0 8a4 4 0 1 0 0-8z",
                "M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.3 5.3l1.4 1.4M17.3 17.3l1.4 1.4M5.3 18.7l1.4-1.4M17.3 6.7l1.4-1.4",
            ],
            &[],
        ),
        Moon => (&["M19.5 14.5A8 8 0 0 1 9.5 4.5a8 8 0 1 0 10 10z"], &[]),
        Image => (&["M4 5h16v14H4z", "M4 16l5-5l4 4l2.5-2.5L20 17"], &["M15.5 7.5a1.5 1.5 0 1 1 0 3a1.5 1.5 0 1 1 0-3z"]),
        File => (&["M13.5 3.5H6.5v17h11V7.5z", "M13.5 3.5v4h4"], &[]),
        Folder => (&["M3.5 6.5h6l2 2.5h9v10h-17z"], &[]),
        Filter => (&["M4 5h16l-6.5 7.5V19l-3-1.5v-5z"], &[]),
        Refresh => (&["M19.5 12a7.5 7.5 0 1 1-2.2-5.3", "M19.5 4.5v4h-4"], &[]),
        Play => (&[], &["M7.5 5v14l11.5-7z"]),
        Pause => (&[], &["M7 5h3.5v14H7z", "M13.5 5H17v14h-3.5z"]),
        Grid => (&["M4 4h7v7H4z", "M13 4h7v7h-7z", "M4 13h7v7H4z", "M13 13h7v7h-7z"], &[]),
        List => (&["M9 6.5h11", "M9 12h11", "M9 17.5h11"], &[
            "M5 5.3a1.2 1.2 0 1 1 0 2.4a1.2 1.2 0 1 1 0-2.4z",
            "M5 10.8a1.2 1.2 0 1 1 0 2.4a1.2 1.2 0 1 1 0-2.4z",
            "M5 16.3a1.2 1.2 0 1 1 0 2.4a1.2 1.2 0 1 1 0-2.4z",
        ]),
        Sliders => (&["M4 7h9", "M17 7h3", "M4 17h3", "M11 17h9", "M15 5v4", "M9 15v4"], &[]),
        Globe => (
            &[
                "M12 3.5a8.5 8.5 0 1 0 0 17a8.5 8.5 0 1 0 0-17z",
                "M3.5 12h17",
                "M12 3.5c2.5 2.5 3.5 5.5 3.5 8.5s-1 6-3.5 8.5c-2.5-2.5-3.5-5.5-3.5-8.5s1-6 3.5-8.5z",
            ],
            &[],
        ),
        Bolt => (&["M13.5 3L5.5 13.5h6l-1 7.5l8-10.5h-6z"], &[]),
        Command => (&["M9 9h6v6H9z", "M9 9V7a2 2 0 1 0-2 2zM15 9h2a2 2 0 1 0-2-2zM15 15v2a2 2 0 1 0 2-2zM9 15H7a2 2 0 1 0 2 2z"], &[]),
        Inbox => (&["M4 13l2.5-7.5h11L20 13v6H4z", "M4 13h4.5l1 2h5l1-2H20"], &[]),
        Send => (&["M20.5 3.5L10 14", "M20.5 3.5L14 20.5l-4-6.5l-6.5-4z"], &[]),
        Palette => (
            &["M12 3.5a8.5 8.5 0 0 0 0 17c1.4 0 2-1 1.6-2c-.6-1.4.2-2.5 1.6-2.5h1.8a3.5 3.5 0 0 0 3.5-3.5A9 9 0 0 0 12 3.5z"],
            &[
                "M7.5 10.2a1.2 1.2 0 1 1 0 2.4a1.2 1.2 0 1 1 0-2.4z",
                "M10.5 6.5a1.2 1.2 0 1 1 0 2.4a1.2 1.2 0 1 1 0-2.4z",
                "M15 6.8a1.2 1.2 0 1 1 0 2.4a1.2 1.2 0 1 1 0-2.4z",
            ],
        ),
    }
}

#[derive(Clone)]
enum Glyph {
    Builtin(Icon),
    Svg(SvgIcon),
}
/// An icon element; configure with `size`, `color` and `stroke`.
pub struct IconElement {
    glyph: Glyph,
    size: f32,
    color: String,
    stroke: f32,
    stroke_set: bool,
    keep_colors: bool,
    key: Option<String>,
    label: Option<String>,
}
/// A built-in icon at 20 px in the inherited text colour role.
pub fn icon(glyph: Icon) -> IconElement {
    IconElement {
        glyph: Glyph::Builtin(glyph),
        size: 20.,
        color: ColorToken::BaseContent.into(),
        stroke: 1.75,
        stroke_set: false,
        keep_colors: false,
        key: None,
        label: None,
    }
}
impl IconElement {
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
    pub fn color(mut self, color: impl Into<String>) -> Self {
        self.color = color.into();
        self
    }
    /// Stroke width on the 24-unit grid (default 1.75). For SVG icons this
    /// replaces the file's own stroke widths (in its own units).
    pub fn stroke(mut self, width: f32) -> Self {
        self.stroke = width;
        self.stroke_set = true;
        self
    }
    /// SVG icons: keep colours written in the file instead of tinting every
    /// shape with `.color(..)`; `currentColor` still uses the icon colour.
    pub fn keep_colors(mut self, value: bool) -> Self {
        self.keep_colors = value;
        self
    }
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    /// Give a meaningful (non-decorative) icon a screen-reader name.
    pub fn accessible_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}
impl<C> From<IconElement> for Element<C> {
    fn from(i: IconElement) -> Self {
        let color = i.color;
        let mut e = match i.glyph {
            Glyph::Builtin(glyph) => {
                let (strokes, fills) = paths(glyph);
                let s = i.size / 24.;
                let m = [s, 0., 0., s, 0., 0.];
                let stroke = i.stroke * s;
                canvas(move |c| {
                    for d in strokes {
                        if let Ok(p) = VectorPath::from_svg(d) {
                            c.stroke(p.transformed(m), color.clone(), stroke);
                        }
                    }
                    for d in fills {
                        if let Ok(p) = VectorPath::from_svg(d) {
                            c.fill(p.transformed(m), color.clone());
                        }
                    }
                })
            }
            Glyph::Svg(svg) => {
                // Fit the viewBox into the square, centred, keeping aspect.
                let [vx, vy, vw, vh] = svg.0.view_box;
                let s = i.size / vw.max(vh);
                let (ox, oy) = ((i.size - vw * s) / 2., (i.size - vh * s) / 2.);
                let m = [s, 0., 0., s, ox - vx * s, oy - vy * s];
                let (keep, stroke_override) = (i.keep_colors, i.stroke_set.then_some(i.stroke));
                let ink = move |p: Paint| match p {
                    Paint::Color(c) if keep => crate::theme::hex(c),
                    _ => color.clone(),
                };
                canvas(move |c| {
                    for shape in &svg.0.shapes {
                        let path = shape.path.clone().transformed(m);
                        if let Some(fill) = shape.fill {
                            c.fill(path.clone(), ink(fill));
                        }
                        if let Some((paint, width)) = shape.stroke {
                            c.stroke(path, ink(paint), stroke_override.unwrap_or(width) * s);
                        }
                    }
                })
            }
        }
        .width(i.size)
        .height(i.size);
        if let Some(key) = i.key {
            e = e.key(key);
        }
        if let Some(label) = i.label {
            e = e.accessible_label(label);
        }
        e
    }
}
/// Every glyph, for galleries and tests.
pub const ALL_ICONS: [Icon; 65] = {
    use Icon::*;
    [
        Search,
        Plus,
        Minus,
        Close,
        Check,
        ChevronUp,
        ChevronDown,
        ChevronLeft,
        ChevronRight,
        ChevronsLeft,
        ChevronsRight,
        ArrowUp,
        ArrowDown,
        ArrowLeft,
        ArrowRight,
        Menu,
        MoreHorizontal,
        MoreVertical,
        Home,
        Settings,
        User,
        Users,
        Bell,
        Mail,
        Calendar,
        Clock,
        Star,
        StarFilled,
        Heart,
        Bookmark,
        Trash,
        Edit,
        Copy,
        Download,
        Upload,
        Link,
        ExternalLink,
        Eye,
        EyeOff,
        Lock,
        Unlock,
        Info,
        Warning,
        Error,
        Success,
        Help,
        Sun,
        Moon,
        Image,
        File,
        Folder,
        Filter,
        Refresh,
        Play,
        Pause,
        Grid,
        List,
        Sliders,
        Globe,
        Bolt,
        Command,
        Inbox,
        Send,
        Palette,
        Upload2,
    ]
};
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_icon_parses() {
        for glyph in ALL_ICONS {
            let (strokes, fills) = paths(glyph);
            assert!(!strokes.is_empty() || !fills.is_empty(), "{glyph:?} empty");
            for d in strokes.iter().chain(fills) {
                VectorPath::from_svg(d).unwrap_or_else(|e| panic!("{glyph:?}: {e}: {d}"));
            }
        }
    }
}

// ── Custom SVG icons ─────────────────────────────────────────────────────

/// How a shape is painted: the icon colour (`currentColor`, or any colour
/// when tinting) or a colour kept from the file.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Paint {
    Current,
    Color([f32; 4]),
}
#[derive(Clone, Debug, PartialEq)]
struct SvgShape {
    path: VectorPath,
    fill: Option<Paint>,
    stroke: Option<(Paint, f32)>,
}
#[derive(Debug, PartialEq)]
struct SvgData {
    view_box: [f32; 4],
    shapes: Vec<SvgShape>,
}
/// An icon loaded from SVG markup: any icon set (Lucide, Heroicons, Tabler,
/// Material Symbols…) or your own drawings. Cheap to clone.
///
/// Supports `<path>`, `<circle>`, `<ellipse>`, `<rect>` (with `rx`/`ry`),
/// `<line>`, `<polyline>`, `<polygon>` and nested `<g>` with inherited
/// `fill`, `stroke`, `stroke-width`, `style` and `transform`
/// (`translate`, `scale`, `matrix`). Gradients, text, masks and filters are
/// ignored.
#[derive(Clone, Debug, PartialEq)]
pub struct SvgIcon(std::sync::Arc<SvgData>);
impl SvgIcon {
    /// Parse SVG markup. Errors name the first problem found.
    pub fn parse(markup: &str) -> Result<SvgIcon, String> {
        svg_doc::parse(markup).map(|data| SvgIcon(std::sync::Arc::new(data)))
    }
    /// Read and parse an `.svg` file (at most 1 MiB).
    pub fn load(path: impl AsRef<std::path::Path>) -> Result<SvgIcon, String> {
        let path = path.as_ref();
        let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if meta.len() > 1 << 20 {
            return Err(format!("{}: larger than 1 MiB", path.display()));
        }
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }
    /// The drawing area in SVG units.
    pub fn view_box(&self) -> [f32; 4] {
        self.0.view_box
    }
    /// Number of drawable shapes found.
    pub fn shape_count(&self) -> usize {
        self.0.shapes.len()
    }
}

/// A named collection of SVG icons, e.g. a whole icon-set folder.
#[derive(Clone, Debug, Default)]
pub struct IconSet {
    icons: std::collections::BTreeMap<String, SvgIcon>,
}
impl IconSet {
    pub fn new() -> Self {
        Self::default()
    }
    /// Load every `*.svg` in a folder; names are file stems ("arrow-left").
    /// Files that fail to parse are skipped and reported in the error list.
    pub fn from_dir(dir: impl AsRef<std::path::Path>) -> std::io::Result<(IconSet, Vec<String>)> {
        let mut set = IconSet::new();
        let mut errors = Vec::new();
        let mut entries: Vec<_> = std::fs::read_dir(dir)?.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries.into_iter().take(20_000) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("svg") {
                continue;
            }
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            match SvgIcon::load(&path) {
                Ok(svg) => set.insert(name, svg),
                Err(e) => errors.push(e),
            }
        }
        Ok((set, errors))
    }
    pub fn insert(&mut self, name: impl Into<String>, icon: SvgIcon) {
        self.icons.insert(name.into(), icon);
    }
    pub fn get(&self, name: &str) -> Option<&SvgIcon> {
        self.icons.get(name)
    }
    /// An element for a named icon, or `None` when the set lacks it.
    pub fn icon(&self, name: &str) -> Option<IconElement> {
        self.get(name).map(svg_icon)
    }
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.icons.keys().map(String::as_str)
    }
    pub fn len(&self) -> usize {
        self.icons.len()
    }
    pub fn is_empty(&self) -> bool {
        self.icons.is_empty()
    }
}

/// An element for an SVG icon, configured like built-in icons: `.size`,
/// `.color`, `.stroke` (overrides the file's stroke widths) and
/// `.keep_colors(true)` for multi-colour artwork.
pub fn svg_icon(svg: &SvgIcon) -> IconElement {
    IconElement {
        glyph: Glyph::Svg(svg.clone()),
        size: 20.,
        color: ColorToken::BaseContent.into(),
        stroke: 1.75,
        stroke_set: false,
        keep_colors: false,
        key: None,
        label: None,
    }
}

mod svg_doc {
    use super::{Paint, SvgData, SvgShape};
    use crate::graphics::VectorPath;

    #[derive(Clone, Copy)]
    struct Style {
        fill: Option<Paint>,
        stroke: Option<Paint>,
        width: f32,
        matrix: [f32; 6],
    }
    fn multiply(a: [f32; 6], b: [f32; 6]) -> [f32; 6] {
        [
            a[0] * b[0] + a[2] * b[1],
            a[1] * b[0] + a[3] * b[1],
            a[0] * b[2] + a[2] * b[3],
            a[1] * b[2] + a[3] * b[3],
            a[0] * b[4] + a[2] * b[5] + a[4],
            a[1] * b[4] + a[3] * b[5] + a[5],
        ]
    }
    fn numbers(s: &str) -> Vec<f32> {
        s.split(|c: char| c == ',' || c.is_whitespace())
            .filter(|t| !t.is_empty())
            .filter_map(|t| t.trim_end_matches("px").parse().ok())
            .collect()
    }
    fn transform(value: &str) -> [f32; 6] {
        let mut m = [1., 0., 0., 1., 0., 0.];
        for part in value.split(')') {
            let Some((name, args)) = part.split_once('(') else {
                continue;
            };
            let a = numbers(args);
            let t = match (name.trim(), a.as_slice()) {
                ("translate", [x]) => [1., 0., 0., 1., *x, 0.],
                ("translate", [x, y, ..]) => [1., 0., 0., 1., *x, *y],
                ("scale", [s]) => [*s, 0., 0., *s, 0., 0.],
                ("scale", [x, y, ..]) => [*x, 0., 0., *y, 0., 0.],
                ("matrix", [a, b, c, d, e, f]) => [*a, *b, *c, *d, *e, *f],
                _ => continue,
            };
            m = multiply(m, t);
        }
        m
    }
    fn paint(value: &str) -> Option<Option<Paint>> {
        let v = value.trim();
        Some(match v {
            "none" | "transparent" => None,
            "currentColor" | "currentcolor" => Some(Paint::Current),
            "black" => Some(Paint::Color([0., 0., 0., 1.])),
            "white" => Some(Paint::Color([1., 1., 1., 1.])),
            _ if v.starts_with('#') => Some(Paint::Color(crate::theme::parse_hex(v)?)),
            // Gradients (url(#…)) and other forms: draw in the icon colour.
            _ => Some(Paint::Current),
        })
    }
    /// Attributes of one tag, with `style="a: b; …"` merged in.
    fn attributes(tag: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut rest = tag;
        while let Some(eq) = rest.find('=') {
            let name = rest[..eq]
                .split_whitespace()
                .last()
                .unwrap_or("")
                .to_owned();
            let after = rest[eq + 1..].trim_start();
            let Some(quote) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else {
                break;
            };
            let Some(end) = after[1..].find(quote) else {
                break;
            };
            out.push((name, after[1..1 + end].to_owned()));
            rest = &after[end + 2..];
        }
        if let Some((_, style)) = out.iter().find(|(n, _)| n == "style").cloned() {
            for decl in style.split(';') {
                if let Some((k, v)) = decl.split_once(':') {
                    out.push((k.trim().to_owned(), v.trim().to_owned()));
                }
            }
        }
        out
    }
    fn get<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
        attrs
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
    fn num(attrs: &[(String, String)], name: &str) -> f32 {
        get(attrs, name)
            .and_then(|v| numbers(v).first().copied())
            .unwrap_or(0.)
    }
    const K: f32 = 0.552_284_8;
    fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> VectorPath {
        let (kx, ky) = (rx * K, ry * K);
        VectorPath::new()
            .move_to(cx + rx, cy)
            .cubic_to([cx + rx, cy + ky], [cx + kx, cy + ry], [cx, cy + ry])
            .cubic_to([cx - kx, cy + ry], [cx - rx, cy + ky], [cx - rx, cy])
            .cubic_to([cx - rx, cy - ky], [cx - kx, cy - ry], [cx, cy - ry])
            .cubic_to([cx + kx, cy - ry], [cx + rx, cy - ky], [cx + rx, cy])
            .close()
    }
    fn rounded_rect(x: f32, y: f32, w: f32, h: f32, rx: f32, ry: f32) -> VectorPath {
        if rx <= 0. || ry <= 0. {
            return VectorPath::rectangle(x, y, w, h);
        }
        let (rx, ry) = (rx.min(w / 2.), ry.min(h / 2.));
        let (kx, ky) = (rx * K, ry * K);
        let (r, b) = (x + w, y + h);
        VectorPath::new()
            .move_to(x + rx, y)
            .line_to(r - rx, y)
            .cubic_to([r - rx + kx, y], [r, y + ry - ky], [r, y + ry])
            .line_to(r, b - ry)
            .cubic_to([r, b - ry + ky], [r - rx + kx, b], [r - rx, b])
            .line_to(x + rx, b)
            .cubic_to([x + rx - kx, b], [x, b - ry + ky], [x, b - ry])
            .line_to(x, y + ry)
            .cubic_to([x, y + ry - ky], [x + rx - kx, y], [x + rx, y])
            .close()
    }
    fn points(value: &str, close: bool) -> Option<VectorPath> {
        let n = numbers(value);
        let mut pairs = n.as_chunks::<2>().0.iter();
        let first = pairs.next()?;
        let mut p = VectorPath::new().move_to(first[0], first[1]);
        for xy in pairs {
            p = p.line_to(xy[0], xy[1]);
        }
        Some(if close { p.close() } else { p })
    }
    pub(super) fn parse(markup: &str) -> Result<SvgData, String> {
        if markup.len() > 1 << 20 {
            return Err("SVG larger than 1 MiB".into());
        }
        let mut stack: Vec<Style> = vec![Style {
            fill: Some(Paint::Current),
            stroke: None,
            width: 1.,
            matrix: [1., 0., 0., 1., 0., 0.],
        }];
        let mut view_box = None;
        let mut shapes = Vec::new();
        let mut skip_depth = 0usize;
        let mut rest = markup;
        let mut seen_svg = false;
        while let Some(open) = rest.find('<') {
            rest = &rest[open + 1..];
            if let Some(after) = rest.strip_prefix("!--") {
                rest = after.find("-->").map_or("", |end| &after[end + 3..]);
                continue;
            }
            let end = rest.find('>').ok_or("unterminated tag")?;
            let tag = &rest[..end];
            rest = &rest[end + 1..];
            if tag.starts_with('?') || tag.starts_with('!') {
                continue;
            }
            if let Some(name) = tag.strip_prefix('/') {
                let name = name.trim();
                if skip_depth > 0 {
                    if matches!(
                        name,
                        "defs"
                            | "clipPath"
                            | "mask"
                            | "title"
                            | "desc"
                            | "style"
                            | "linearGradient"
                            | "radialGradient"
                            | "pattern"
                            | "symbol"
                    ) {
                        skip_depth -= 1;
                    }
                } else if matches!(name, "g" | "svg" | "a") && stack.len() > 1 {
                    stack.pop();
                }
                continue;
            }
            let self_closing = tag.ends_with('/');
            let tag = tag.trim_end_matches('/');
            let name = tag.split_whitespace().next().unwrap_or("");
            if skip_depth > 0 {
                if !self_closing
                    && matches!(
                        name,
                        "defs"
                            | "clipPath"
                            | "mask"
                            | "title"
                            | "desc"
                            | "style"
                            | "linearGradient"
                            | "radialGradient"
                            | "pattern"
                            | "symbol"
                    )
                {
                    skip_depth += 1;
                }
                continue;
            }
            if matches!(
                name,
                "defs"
                    | "clipPath"
                    | "mask"
                    | "title"
                    | "desc"
                    | "style"
                    | "linearGradient"
                    | "radialGradient"
                    | "pattern"
                    | "symbol"
            ) {
                if !self_closing {
                    skip_depth = 1;
                }
                continue;
            }
            let attrs = attributes(tag);
            let parent = *stack.last().unwrap();
            let mut style = parent;
            if let Some(v) = get(&attrs, "fill") {
                if let Some(p) = paint(v) {
                    style.fill = p;
                }
            }
            if let Some(v) = get(&attrs, "stroke") {
                if let Some(p) = paint(v) {
                    style.stroke = p;
                }
            }
            if let Some(w) = get(&attrs, "stroke-width").and_then(|v| numbers(v).first().copied()) {
                if w.is_finite() && (0. ..=1000.).contains(&w) {
                    style.width = w;
                }
            }
            if let Some(t) = get(&attrs, "transform") {
                style.matrix = multiply(parent.matrix, transform(t));
            }
            let path = match name {
                "svg" => {
                    seen_svg = true;
                    if view_box.is_none() {
                        let vb = get(&attrs, "viewBox").map(numbers).unwrap_or_default();
                        view_box = if vb.len() == 4 && vb[2] > 0. && vb[3] > 0. {
                            Some([vb[0], vb[1], vb[2], vb[3]])
                        } else {
                            let (w, h) = (num(&attrs, "width"), num(&attrs, "height"));
                            (w > 0. && h > 0.).then_some([0., 0., w, h])
                        };
                    }
                    if !self_closing {
                        stack.push(style);
                    }
                    None
                }
                "g" | "a" => {
                    if !self_closing {
                        stack.push(style);
                    }
                    None
                }
                "path" => get(&attrs, "d")
                    .map(|d| VectorPath::from_svg(d).map_err(|e| format!("<path d>: {e}")))
                    .transpose()?,
                "circle" => {
                    let r = num(&attrs, "r");
                    (r > 0.).then(|| ellipse(num(&attrs, "cx"), num(&attrs, "cy"), r, r))
                }
                "ellipse" => {
                    let (rx, ry) = (num(&attrs, "rx"), num(&attrs, "ry"));
                    (rx > 0. && ry > 0.)
                        .then(|| ellipse(num(&attrs, "cx"), num(&attrs, "cy"), rx, ry))
                }
                "rect" => {
                    let (w, h) = (num(&attrs, "width"), num(&attrs, "height"));
                    let rx = get(&attrs, "rx").map(|_| num(&attrs, "rx"));
                    let ry = get(&attrs, "ry").map(|_| num(&attrs, "ry"));
                    let (rx, ry) = (rx.or(ry).unwrap_or(0.), ry.or(rx).unwrap_or(0.));
                    (w > 0. && h > 0.)
                        .then(|| rounded_rect(num(&attrs, "x"), num(&attrs, "y"), w, h, rx, ry))
                }
                "line" => Some(
                    VectorPath::new()
                        .move_to(num(&attrs, "x1"), num(&attrs, "y1"))
                        .line_to(num(&attrs, "x2"), num(&attrs, "y2")),
                ),
                "polyline" => get(&attrs, "points").and_then(|v| points(v, false)),
                "polygon" => get(&attrs, "points").and_then(|v| points(v, true)),
                _ => None,
            };
            if let Some(path) = path {
                let path = path.transformed(style.matrix);
                // Lines and open polylines never fill.
                let fill = if matches!(name, "line" | "polyline") {
                    None
                } else {
                    style.fill
                };
                let stroke = style.stroke.map(|p| {
                    let m = style.matrix;
                    let scale = ((m[0] * m[3] - m[1] * m[2]).abs()).sqrt();
                    (p, style.width * scale)
                });
                if fill.is_some() || stroke.is_some() {
                    shapes.push(SvgShape { path, fill, stroke });
                }
                if shapes.len() > 4096 {
                    return Err("more than 4096 shapes".into());
                }
            }
        }
        if !seen_svg {
            return Err("no <svg> element".into());
        }
        Ok(SvgData {
            view_box: view_box.unwrap_or([0., 0., 24., 24.]),
            shapes,
        })
    }
}
#[cfg(test)]
mod svg_icon_tests {
    use super::*;
    // Lucide "house": stroked, attributes inherited from the root.
    const LUCIDE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-house"><path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8"/><path d="M3 10a2 2 0 0 1 .709-1.528l7-5.999a2 2 0 0 1 2.582 0l7 5.999A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>"#;
    // Heroicons solid style: filled, 20-unit grid.
    const HERO: &str = r#"<?xml version="1.0"?><!-- heroicons --><svg viewBox="0 0 20 20" fill="currentColor" aria-hidden="true"><path fill-rule="evenodd" d="M10 18a8 8 0 1 0 0-16 8 8 0 0 0 0 16Z" clip-rule="evenodd"/></svg>"#;
    // Multi-colour with groups, transforms, styles and primitives.
    const ART: &str = r##"<svg viewBox="0 0 100 50"><defs><linearGradient id="g"><stop offset="0"/></linearGradient></defs><title>Badge</title>
        <g transform="translate(10 5)" style="fill:#ff0000;stroke:none">
            <rect x="0" y="0" width="40" height="40" rx="8"/>
            <circle cx="20" cy="20" r="6" fill="#ffffff"/>
        </g>
        <ellipse cx="75" cy="25" rx="20" ry="10" fill="currentColor"/>
        <line x1="0" y1="49" x2="100" y2="49" stroke="#000" stroke-width="2"/>
        <polygon points="60,5 70,5 65,12"/>
    </svg>"##;
    #[test]
    fn real_icon_sets_parse_with_inherited_paint() {
        let house = SvgIcon::parse(LUCIDE).unwrap();
        assert_eq!(house.view_box(), [0., 0., 24., 24.]);
        assert_eq!(house.shape_count(), 2);
        assert!(house
            .0
            .shapes
            .iter()
            .all(|s| s.fill.is_none() && s.stroke == Some((Paint::Current, 2.))));
        let circle = SvgIcon::parse(HERO).unwrap();
        assert_eq!(circle.view_box(), [0., 0., 20., 20.]);
        assert_eq!(circle.0.shapes[0].fill, Some(Paint::Current));
    }
    #[test]
    fn groups_transforms_styles_and_primitives() {
        let art = SvgIcon::parse(ART).unwrap();
        // rect, circle, ellipse, line, polygon; defs/title skipped.
        assert_eq!(art.shape_count(), 5);
        let red = crate::theme::parse_hex("#ff0000").unwrap();
        assert_eq!(art.0.shapes[0].fill, Some(Paint::Color(red)));
        assert_eq!(art.0.shapes[0].stroke, None, "style overrides attributes");
        // The circle is translated by the group: centre (30, 25).
        assert!(art.0.shapes[1].path.contains(30., 25.));
        assert!(!art.0.shapes[1].path.contains(20., 20.));
        assert_eq!(art.0.shapes[3].fill, None, "lines never fill");
        assert!(art.0.shapes[3].stroke.is_some());
    }
    #[test]
    fn malformed_markup_is_rejected_clearly() {
        assert!(SvgIcon::parse("not svg").unwrap_err().contains("<svg>"));
        assert!(SvgIcon::parse("<svg><path d=\"M0 0 L\"/></svg>")
            .unwrap_err()
            .contains("path"));
        assert!(SvgIcon::parse("<svg").is_err());
    }
    #[test]
    fn icon_sets_load_folders_and_report_bad_files() {
        let dir = std::env::temp_dir().join(format!("volt-icons-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("house.svg"), LUCIDE).unwrap();
        std::fs::write(dir.join("dot.svg"), HERO).unwrap();
        std::fs::write(dir.join("broken.svg"), "<nope/>").unwrap();
        std::fs::write(dir.join("notes.txt"), "ignored").unwrap();
        let (set, errors) = IconSet::from_dir(&dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(set.names().collect::<Vec<_>>(), ["dot", "house"]);
        assert_eq!(errors.len(), 1);
        assert!(set.icon("house").is_some() && set.icon("missing").is_none());
    }
    #[test]
    fn svg_icons_build_and_fit_their_box() {
        let e: Element<()> = vstack![
            svg_icon(&SvgIcon::parse(LUCIDE).unwrap())
                .size(32.)
                .key("a"),
            svg_icon(&SvgIcon::parse(ART).unwrap())
                .size(40.)
                .keep_colors(true)
                .key("b"),
        ]
        .into();
        let b = e.build().unwrap();
        let (doc, _) = b.into_parts();
        let a = &doc.root().children[0];
        assert_eq!((a.width, a.height), (Some(32.), Some(32.)));
        // Every path lands inside the 32×32 box (2-unit stroke margin aside).
        for p in &a.surface.canvas.paths {
            let ok = p.path.commands.iter().all(|c| match c {
                crate::graphics::PathCommand::Move(p) | crate::graphics::PathCommand::Line(p) => {
                    p[0] >= -0.01 && p[0] <= 32.01 && p[1] >= -0.01 && p[1] <= 32.01
                }
                _ => true,
            });
            assert!(ok);
        }
        let b_paths = &doc.root().children[1].surface.canvas.paths;
        assert!(b_paths.iter().any(|p| p.color == "#FF0000"), "colours kept");
    }
}
