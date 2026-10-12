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

/// An icon element; configure with `size`, `color` and `stroke`.
pub struct IconElement {
    glyph: Icon,
    size: f32,
    color: String,
    stroke: f32,
    key: Option<String>,
    label: Option<String>,
}
/// A built-in icon at 20 px in the inherited text colour role.
pub fn icon(glyph: Icon) -> IconElement {
    IconElement {
        glyph,
        size: 20.,
        color: ColorToken::BaseContent.into(),
        stroke: 1.75,
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
    /// Stroke width on the 24-unit grid (default 1.75).
    pub fn stroke(mut self, width: f32) -> Self {
        self.stroke = width;
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
        let (strokes, fills) = paths(i.glyph);
        let s = i.size / 24.;
        let m = [s, 0., 0., s, 0., 0.];
        let (color, stroke) = (i.color, i.stroke * s);
        let mut e = canvas(move |c| {
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
