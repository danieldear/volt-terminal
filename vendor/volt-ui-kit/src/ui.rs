//! Rust-first UI: ordinary Rust functions, state, conditionals and typed closures.
//!
//! The native host calls `view(&state)` initially and after dispatched actions.
//! This is whole-tree composition, not dependency-tracked signals. Do not perform
//! I/O or mutate state in view bodies. Callback captures must be owned (`'static`).
//!
//! ```
//! use volt_ui_kit::ui::{column, prelude::*};
//! #[derive(Default)]
//! struct App { count: usize }
//! fn view(app: &App) -> Element<App> {
//!     column![
//!         text(format!("Count: {}", app.count)).font_size(24.),
//!         button("Add").key("add").on_click(|app: &mut App| app.count += 1),
//!     ].padding(24., Edge::All).gap(12., Axis::Both).into()
//! }
//! let (document, handlers) = view(&App::default()).build()?.into_parts();
//! # Ok::<(), volt_ui_kit::declarative::DocumentError>(())
//! ```
//! Directional spacing uses logical pixels; these are property setters (last
//! write to an edge/axis wins), not accumulating wrapper layers:
//! ```
//! use volt_ui_kit::ui::{column, prelude::*};
//! let screen: Element<()> = column![
//!     text("Title"),
//!     row![text("Left"), text("Right")].gap(12., Axis::Horizontal),
//! ].padding(24., Edge::Horizontal).padding(16., Edge::Vertical).padding(8., Edge::Top)
//!  .gap(10., Axis::Vertical).into();
//! screen.build()?;
//! # Ok::<(), volt_ui_kit::declarative::DocumentError>(())
//! ```
//! Adaptive direction belongs to `stack!`, not the fixed-direction aliases:
//! ```compile_fail
//! use volt_ui_kit::ui::prelude::*;
//! vstack![text::<()>("Fixed vertical")].direction(Direction::Horizontal);
//! ```
//! Responsive properties resolve during layout without composing new callbacks:
//! ```
//! use volt_ui_kit::ui::prelude::*;
//! let screen: Element<()> = stack![
//!     text("Navigation"),
//!     zstack![text("Back"), badge("Front")].height(80.),
//! ].direction(responsive(Direction::Vertical).at(Breakpoint::Md, Direction::Horizontal))
//!  .padding(responsive(12.).at(Breakpoint::Md, 24.), Edge::Horizontal)
//!  .gap(responsive(8.).at(Breakpoint::Md, 16.), Axis::Both).into();
//! screen.build()?;
//! # Ok::<(), volt_ui_kit::declarative::DocumentError>(())
//! ```
//!
//! Button callbacks cannot accept a boolean change value:
//! ```compile_fail
//! use volt_ui_kit::ui::{column, prelude::*};
//! button("Add").on_click(|_: &mut (), _: bool| {});
//! ```
//! Checkable callbacks require the proposed boolean value:
//! ```compile_fail
//! use volt_ui_kit::ui::{column, prelude::*};
//! toggle("Preview", false).on_change(|_: &mut ()| {});
//! ```
//! Component-only modifiers remain typed:
//! ```compile_fail
//! use volt_ui_kit::ui::{column, prelude::*};
//! text::<()>("Title").on_click(|_: &mut ()| {});
//! ```
//! Gaps select an axis, not an outer edge:
//! ```compile_fail
//! use volt_ui_kit::ui::prelude::*;
//! vstack![text::<()>("Title")].gap(12., Edge::Top);
//! ```
//! Familiar stack aliases and content placement work inside any container:
//! ```
//! use volt_ui_kit::ui::prelude::*;
//! let screen: Element<()> = panel![
//!     vstack![
//!         text("Centered content").font_weight(700),
//!         hstack![text("Left"), spacer(), badge("Right").rounded(Rounding::Full)]
//!             .width(220.).gap(12., Axis::Horizontal),
//!         divider(),
//!     ].gap(12., Axis::Vertical).flow(Flow::Center),
//! ].height(240.).padding(20.5, Edge::Horizontal).flow(Flow::Center).rounded(4).into();
//! screen.build()?;
//! # Ok::<(), volt_ui_kit::declarative::DocumentError>(())
//! ```
//! Select multiple edges/corners and opt into paint-only effects:
//! ```
//! use volt_ui_kit::ui::prelude::*;
//! let ui: Element<usize> = vstack![
//!     button("Save").key("save").rounded_on(12., [Corner::TopLeft, Corner::BottomRight])
//!         .shadow(Shadow::new().blur(8.).offset(0., 2.))
//!         .hover_style(|s| s.glow(Glow::new().color("#60a5fa80")).opacity(0.8))
//!         .animate_on(Interaction::Hover, Animation::ease_out().duration_ms(120))
//!         .on_click(|count: &mut usize| *count += 1),
//!     text("Notice").key("notice").transition(Transition::fade().duration_ms(180)),
//! ].padding(20.5, [Edge::Top, Edge::Left]).gap(12., [Axis::Horizontal, Axis::Vertical]).into();
//! ui.build()?;
//! # Ok::<(), volt_ui_kit::declarative::DocumentError>(())
//! ```
//!
//! The guides in `docs/` (also the `guide` module with the `native` feature)
//! cover every component, styling and application services in depth.
use crate::declarative::{
    document::{FontSlant, MAX_DEPTH, MAX_NODES},
    AccessRole, Bindings, Document, DocumentError, Handlers, Kind, Node,
};

// Generated composite names must not reject otherwise-valid Unicode cell text.
fn bounded_semantic_name(text: &str, fallback: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        let ch = if ch.is_control() { ' ' } else { ch };
        if out.len() + ch.len_utf8() > 8192 {
            break;
        }
        out.push(ch);
    }
    if out.trim().is_empty() {
        fallback.into()
    } else {
        out
    }
}
pub use crate::declarative::{
    responsive, Align, Ambient, Animation, Breakpoint, Direction, Easing, Flow, Glow, Interaction,
    Responsive, Rounding, Scroll, Scrollbars, Shadow, Tone, Transition, VisualStyle,
};
pub use crate::declarative::{
    BorderStyle, Cursor, Gradient, GridColumns, Justify, Sides, SplitHandle, TextAlign,
};

pub use crate::graphics::{
    BackgroundImage, Canvas, Clip, Image, ImageFit, ImagePosition, VectorPath,
};
pub use crate::{ColorToken, Palette, ThemeMode, UiTheme};

/// Inner edges selected by `.padding(value, edge)`; left/right are absolute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    All,
    Horizontal,
    Vertical,
    Top,
    Right,
    Bottom,
    Left,
}
/// Inter-child gap axes; edges are deliberately not accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Both,
    Horizontal,
    Vertical,
}

/// Individual corners, clockwise from top-left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}
/// Convenient corner groups selected by side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corners {
    All,
    Top,
    Right,
    Bottom,
    Left,
}
/// Union of edge selections. Empty arrays are a no-op.
#[derive(Clone, Copy, Debug)]
pub struct Edges(u8);
/// Union of gap axes. Empty arrays are a no-op.
#[derive(Clone, Copy, Debug)]
pub struct Axes(u8);
/// Union of corners. Unselected corners are square on the first rounded_on call.
#[derive(Clone, Copy, Debug)]
pub struct CornerSet(u8);
impl From<Edge> for Edges {
    fn from(v: Edge) -> Self {
        Self(match v {
            Edge::All => 15,
            Edge::Horizontal => 10,
            Edge::Vertical => 5,
            Edge::Top => 1,
            Edge::Right => 2,
            Edge::Bottom => 4,
            Edge::Left => 8,
        })
    }
}
impl From<Axis> for Axes {
    fn from(v: Axis) -> Self {
        Self(match v {
            Axis::Both => 3,
            Axis::Horizontal => 1,
            Axis::Vertical => 2,
        })
    }
}
impl From<Corner> for CornerSet {
    fn from(v: Corner) -> Self {
        Self(1 << v as u8)
    }
}
impl From<Corners> for CornerSet {
    fn from(v: Corners) -> Self {
        Self(match v {
            Corners::All => 15,
            Corners::Top => 3,
            Corners::Right => 6,
            Corners::Bottom => 12,
            Corners::Left => 9,
        })
    }
}
macro_rules! selection_arrays {
    ($selector:ty, $item:ty) => {
        impl<const N: usize> From<[$item; N]> for $selector {
            fn from(v: [$item; N]) -> Self {
                Self(v.into_iter().fold(0, |m, x| m | Self::from(x).0))
            }
        }
        impl From<&[$item]> for $selector {
            fn from(v: &[$item]) -> Self {
                Self(v.iter().fold(0, |m, x| m | Self::from(*x).0))
            }
        }
        impl<const N: usize> From<&[$item; N]> for $selector {
            fn from(v: &[$item; N]) -> Self {
                Self::from(v.as_slice())
            }
        }
        impl From<Vec<$item>> for $selector {
            fn from(v: Vec<$item>) -> Self {
                Self::from(v.as_slice())
            }
        }
    };
}
selection_arrays!(Edges, Edge);
selection_arrays!(Axes, Axis);
selection_arrays!(CornerSet, Corner);
selection_arrays!(CornerSet, Corners);

/// Explicit normal/italic font selection. The host resolves available families.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontStyle {
    Normal,
    Italic,
}

type Click<C> = Box<dyn FnMut(&mut C)>;
type Change<C> = Box<dyn FnMut(&mut C, bool)>;
type ChangeText<C> = Box<dyn FnMut(&mut C, String)>;
type ChangeNumber<C> = Box<dyn FnMut(&mut C, f32)>;
type ContextCallback<C> = Box<dyn FnMut(&mut C, [f32; 2])>;
type WindowCallback<C> = Box<dyn FnMut(&mut C, std::ops::Range<usize>)>;
type FilesCallback<C> = Box<dyn FnMut(&mut C, Vec<std::path::PathBuf>)>;
enum Callback<C> {
    Click(Click<C>),
    Change(Change<C>),
    Text {
        change: Option<ChangeText<C>>,
        submit: Option<Click<C>>,
    },
    Number(ChangeNumber<C>),
    Range {
        lower: ChangeNumber<C>,
        upper: ChangeNumber<C>,
    },
    Dismiss(Click<C>),
    Context(ContextCallback<C>),
    Window(WindowCallback<C>),
    Files(FilesCallback<C>),
}
mod bars;
mod charts;
mod data;
mod dates;
mod dialogs;
mod disclosure;
mod display;
pub use charts::{
    area_chart, candlestick_chart, donut_chart, nice_ticks, short_number, stacked_bar_chart,
    AreaChart, Candle, CandlestickChart, DonutChart, StackedBarChart,
};
mod forms;
pub use disclosure::{
    accordion, collapsible, command, command_palette, drawer, toaster, Accordion, Collapsible,
    Command, CommandPalette, CommandPaletteState, Drawer, DrawerSide, Toast, ToastPosition,
    Toaster,
};
pub use display::{
    alert, avatar, avatar_group, bar_chart, breadcrumb, carousel, empty_state, image_view, kbd,
    line_chart, pagination, skeleton, skeleton_text, sparkline, stat, steps, timeline, Alert,
    Avatar, Breadcrumb, Carousel, Pagination, Presence, Severity, Stat, TimelineItem,
};
mod icons;
mod inputs;
mod menus;
mod navigation;
mod popovers;
mod progress;
mod split;
pub mod tokens;
pub use dates::{
    calendar, date_picker, date_range_picker, days_in_month, time_picker, Calendar, CalendarDate,
    DatePicker, DatePickerState, DateRange, DateRangePicker, TimeOfDay, TimePicker,
};
pub use forms::{
    color_picker, combobox, drop_zone, form_field, number_input, otp_input, radio_group, rating,
    tag_input, ColorPicker, Combobox, ComboboxState, DropZone, FormField, Hsl, NumberInput,
    OtpInput, RadioGroup, Rating, TagInput, TagInputState,
};
pub use icons::{icon, svg_icon, Icon, IconElement, IconSet, SvgIcon, ALL_ICONS};
mod value_controls;
pub use bars::{
    button_bar, segmented_control, tab, tab_bar, tab_item, tabs, toolbar, ButtonBar,
    SelectionControl, Tab, TabItem, TabPlacement, Tabs, Toolbar,
};
pub use data::{
    data_item, table, table_column, table_row, tree_node, tree_view, virtual_list, DataItem,
    SortDirection, Table, TableColumn, TableRow, TreeNode, TreeView, VirtualList,
};
pub use dialogs::{dialog, Dialog};
pub use inputs::{text_area, text_field, TextInput};
pub use menus::{
    context_menu, menu, menu_item, menu_separator, menu_submenu, ContextMenu, Menu, MenuItem,
};
pub use navigation::{
    navigation, navigation_item, Navigation, NavigationItem, NavigationPlacement,
};
pub use popovers::{hover_card, popover, tooltip, Popover, Tooltip};
pub use progress::{loading, progress, progress_indeterminate, spinner, Progress};
pub use split::{split, SplitPane};
pub use value_controls::{
    range_slider, select, select_item, slider, RangeSlider, Select, SelectItem, Slider,
};
/// A composed, owned UI description. No callback is run while building it.
pub struct Element<C> {
    node: Node,
    auto_id: bool,
    children: Vec<Element<C>>,
    callback: Option<Callback<C>>,
    validation_error: Option<String>,
}
impl<C> Element<C> {
    fn new(kind: Kind) -> Self {
        Self {
            node: {
                let mut node = Node::new(kind);
                if kind == Kind::Button {
                    node.style.radius = Some(Rounding::Radius(6.));
                }
                node
            },
            auto_id: true,
            children: vec![],
            callback: None,
            validation_error: None,
        }
    }
}
/// Validated layout tree and its directly connected Rust closures.
pub struct BuiltUi<C> {
    document: Document,
    handlers: Handlers<'static, C>,
}
impl<C> BuiltUi<C> {
    pub fn into_parts(self) -> (Document, Handlers<'static, C>) {
        (self.document, self.handlers)
    }
}
impl<C: 'static> Element<C> {
    /// Lower to the shared runtime directly; no JSON serialization or parsing.
    /// Rejects missing callbacks, duplicate keys and invalid geometry/styles.
    pub fn build(self) -> Result<BuiltUi<C>, DocumentError> {
        fn lower<C: 'static>(
            element: Element<C>,
            path: String,
            depth: usize,
            count: &mut usize,
            handlers: &mut Handlers<'static, C>,
        ) -> Result<Node, DocumentError> {
            *count += 1;
            if depth > MAX_DEPTH || *count > MAX_NODES {
                return Err(DocumentError(format!(
                    "{path}: node count or depth limit exceeded"
                )));
            }
            let Element {
                mut node,
                auto_id,
                children,
                callback,
                validation_error,
            } = element;
            if let Some(message) = validation_error {
                return Err(DocumentError(format!("{path}: {message}")));
            }
            if auto_id {
                node.id = path.clone();
            }
            if let Some(callback) = callback {
                let name = format!("rust-action-{}", *count);
                match callback {
                    Callback::Click(f) => {
                        handlers
                            .on_activate(&name, f)
                            .map_err(|e| DocumentError(e.to_string()))?;
                        node.on_activate = Some(name);
                    }
                    Callback::Change(f) => {
                        handlers
                            .on_toggle_changed(&name, f)
                            .map_err(|e| DocumentError(e.to_string()))?;
                        node.on_toggle_changed = Some(name);
                    }
                    Callback::Text { change, submit } => {
                        if let Some(f) = change {
                            let action = format!("{name}-change");
                            handlers
                                .on_text_changed(&action, f)
                                .map_err(|e| DocumentError(e.to_string()))?;
                            node.on_text_changed = Some(action);
                        }
                        if let Some(f) = submit {
                            let action = format!("{name}-submit");
                            handlers
                                .on_activate(&action, f)
                                .map_err(|e| DocumentError(e.to_string()))?;
                            node.on_submit = Some(action);
                        }
                    }
                    Callback::Number(f) => {
                        handlers
                            .on_value_changed(&name, f)
                            .map_err(|e| DocumentError(e.to_string()))?;
                        if let Some(slider) = node.slider.as_mut() {
                            slider.on_change = Some(name.clone());
                        }
                        if let Some(split) = node.split.as_mut() {
                            split.on_change = Some(name);
                        }
                    }
                    Callback::Range { lower, upper } => {
                        let low = format!("{name}-lower");
                        handlers
                            .on_value_changed(&name, upper)
                            .map_err(|e| DocumentError(e.to_string()))?;
                        handlers
                            .on_value_changed(&low, lower)
                            .map_err(|e| DocumentError(e.to_string()))?;
                        if let Some(slider) = node.slider.as_mut() {
                            slider.on_change = Some(name);
                            slider.on_lower_change = Some(low);
                        }
                    }
                    Callback::Context(f) => {
                        handlers
                            .on_context_requested(&name, f)
                            .map_err(|e| DocumentError(e.to_string()))?;
                        node.on_context = Some(name);
                    }
                    Callback::Window(f) => {
                        handlers
                            .on_window_changed(&name, f)
                            .map_err(|e| DocumentError(e.to_string()))?;
                        if let Some(list) = node.virtual_list.as_mut() {
                            list.on_window_change = Some(name);
                        }
                    }
                    Callback::Files(f) => {
                        handlers
                            .on_files_dropped(&name, f)
                            .map_err(|e| DocumentError(e.to_string()))?;
                        node.on_files_dropped = Some(name);
                    }
                    Callback::Dismiss(f) => {
                        handlers
                            .on_activate(&name, f)
                            .map_err(|e| DocumentError(e.to_string()))?;
                        if let Some(overlay) = node.overlay.as_mut() {
                            overlay.dismiss_action = Some(name);
                        }
                    }
                }
            }
            node.children = children
                .into_iter()
                .enumerate()
                .map(|(i, child)| lower(child, format!("{path}-{i}"), depth + 1, count, handlers))
                .collect::<Result<_, _>>()?;
            Ok(node)
        }
        let mut handlers = Handlers::default();
        let root = lower(self, "auto-root".into(), 0, &mut 0, &mut handlers)?;
        let document = Document::from_node(root)?;
        document.validate_connections(&handlers, &Bindings::default())?;
        Ok(BuiltUi { document, handlers })
    }
}

pub struct Text<C>(Element<C>);
pub struct Badge<C>(Element<C>);
pub struct Separator<C>(Element<C>);
pub struct Spacer<C>(Element<C>);
pub struct Button<C>(Element<C>);
pub struct Checkable<C>(Element<C>);
pub struct Container<C>(Element<C>);
/// A stack whose horizontal/vertical direction can change at viewport breakpoints.
pub struct Stack<C>(Element<C>);
macro_rules! element_from {
    ($($name:ident),+ $(,)?) => { $(
        impl<C> From<$name<C>> for Element<C> {
            fn from(value: $name<C>) -> Self { value.0 }
        }
    )+ };
}
element_from!(Text, Badge, Separator, Spacer, Button, Checkable, Container, Stack);

// Deliberately common visual/layout modifiers, not a stringly property map.
macro_rules! modifiers {
    ($target:ident, [$($field:tt)*], [$($auto:tt)*]) => {
        modifiers!($target, [], [$($field)*], [$($auto)*]);
    };
    ($target:ident, [$($generic:ident),*], [$($field:tt)*], [$($auto:tt)*]) => {
        impl<C $(, $generic)*> $target<C $(, $generic)*> {
            /// Stable, globally unique identity. Required on dynamic interactive items.
            /// Must be 1..=128 ASCII letters/digits/underscore/hyphen; checked at build.
            pub fn key(mut self, key: impl Into<String>) -> Self {
                self.$($field)*.id = key.into(); self.$($auto)* = false; self
            }
            /// A screen-reader name independent of the painted label or placeholder.
            /// Required for meaningful icon-only/custom hit targets. Does not alter layout.
            pub fn accessible_label(mut self, label: impl Into<String>) -> Self {
                self.$($field)*.accessible_label = Some(label.into()); self
            }
            pub fn width(mut self, value: f32) -> Self { self.$($field)*.width = Some(value); self }
            /// Logical size limits; they apply to fixed, flexible and natural sizes.
            pub fn min_width(mut self, value: f32) -> Self { self.$($field)*.ext.min[0] = Some(value); self }
            pub fn max_width(mut self, value: f32) -> Self { self.$($field)*.ext.max[0] = Some(value); self }
            pub fn min_height(mut self, value: f32) -> Self { self.$($field)*.ext.min[1] = Some(value); self }
            pub fn max_height(mut self, value: f32) -> Self { self.$($field)*.ext.max[1] = Some(value); self }
            /// A share (0..=1) of the parent's available width, like `w-1/2`.
            pub fn width_fraction(mut self, value: f32) -> Self { self.$($field)*.ext.fraction[0] = Some(value); self }
            pub fn height_fraction(mut self, value: f32) -> Self { self.$($field)*.ext.fraction[1] = Some(value); self }
            /// Width over height; the unset axis follows the other.
            pub fn aspect_ratio(mut self, value: f32) -> Self { self.$($field)*.ext.aspect = Some(value); self }
            /// A width that changes at breakpoints.
            pub fn responsive_width(mut self, value: impl Into<Responsive<f32>>) -> Self { self.$($field)*.ext.responsive_width = Some(value.into()); self }
            /// Show or hide at breakpoints; hidden content takes no space.
            pub fn visible(mut self, value: impl Into<Responsive<bool>>) -> Self { self.$($field)*.ext.visible = Some(value.into()); self }
            /// Columns this item spans inside a `grid`.
            pub fn column_span(mut self, value: u16) -> Self { self.$($field)*.ext.span = value; self }
            /// The mouse cursor while the pointer is over this element.
            pub fn cursor(mut self, value: Cursor) -> Self { self.$($field)*.ext.cursor = Some(value); self }
            /// Paint-time translation in logical pixels; layout and hit targets stay put.
            pub fn offset(mut self, x: f32, y: f32) -> Self { self.$($field)*.ext.offset = [x, y]; self }
            /// Draw the border only on these edges (straight edges; corners stay square).
            pub fn border_sides(mut self, sides: Sides) -> Self { self.$($field)*.ext.border_sides = Some(sides); self }
            pub fn border_style(mut self, style: BorderStyle) -> Self { self.$($field)*.ext.border_style = style; self }
            /// A linear or radial gradient fill, resolved from theme tokens.
            pub fn background_gradient(mut self, gradient: Gradient) -> Self { self.$($field)*.ext.gradient = Some(gradient); self }
            /// Remove a component's fixed-width default; use the parent's cross-axis allocation.
            /// In a vertical stack this fills the available width. Horizontal stacks still use flex.
            pub fn fill_width(mut self) -> Self { self.$($field)*.width = None; self }
            /// Drop an explicit height so the parent decides (stretch or flex).
            pub fn fill_height(mut self) -> Self { self.$($field)*.height = None; self }
            pub fn height(mut self, value: f32) -> Self { self.$($field)*.height = Some(value); self }
            pub fn flex(mut self, value: f32) -> Self { self.$($field)*.flex = value; self }
            pub fn color(mut self, value: impl Into<String>) -> Self { self.$($field)*.style.color = Some(value.into()); self }
            pub fn background(mut self, value: impl Into<String>) -> Self { self.$($field)*.style.background = Some(value.into()); self }
            pub fn border_color(mut self, value: impl Into<String>) -> Self { self.$($field)*.style.border_color = Some(value.into()); self }
            pub fn accent_color(mut self, value: impl Into<String>) -> Self { self.$($field)*.style.accent_color = Some(value.into()); self }
            pub fn border_width(mut self, value: f32) -> Self { self.$($field)*.style.border_width = Some(value); self }
            /// Numeric radius compatibility alias; prefer rounded(number/preset).
            pub fn radius(self, value: f32) -> Self { self.rounded(value) }
            /// Numeric logical radius or Rounding::Full (resolved after layout).
            pub fn rounded(mut self, value: impl Into<Rounding>) -> Self {
                self.$($field)*.style.radius = Some(value.into()); self.$($field)*.style.corners = None; self
            }
            /// Set selected corner radii. First call makes unselected corners square;
            /// later calls update selected corners only. rounded resets all corners.
            pub fn rounded_on(mut self, value: impl Into<Rounding>, corners: impl Into<CornerSet>) -> Self {
                let mask = corners.into().0;
                if mask == 0 { return self; }
                let radii = self.$($field)*.style.corners.get_or_insert([Rounding::Radius(0.);4]);
                let value = value.into();
                for (i,radius) in radii.iter_mut().enumerate() { if mask & (1 << i) != 0 { *radius = value; } }
                self
            }
            /// Background color/image opacity, without fading children.
            pub fn background_opacity(mut self, value:f32)->Self { self.$($field)*.style.background_opacity=Some(value);self }
            pub fn background_image(mut self, image:impl Into<BackgroundImage>)->Self { self.$($field)*.surface.image=Some(image.into());self }
            /// Explicit clipping of children; own outer effects remain visible.
            pub fn clip_contents(mut self, value:bool)->Self { self.$($field)*.surface.clip=value.then_some(Clip::Rounded);self }
            pub fn clip(mut self, clip:Clip)->Self { self.$($field)*.surface.clip=Some(clip);self }
            /// Skip this box as a wheel-scroll starting target. Children remain
            /// eligible; painting and button/keyboard interaction are unchanged.
            /// Useful for empty decorative layers above another scroll view.
            pub fn scroll_passthrough(mut self, value: bool) -> Self { self.$($field)*.effects.scroll_passthrough = value; self }
            /// Subtree group opacity. Does not disable pointer/keyboard interaction.
            pub fn opacity(mut self, value:f32) -> Self { self.$($field)*.effects.base.opacity=Some(value); self }
            pub fn shadow(mut self, value:Shadow) -> Self { self.$($field)*.effects.base.shadow=Some(value); self }
            pub fn glow(mut self, value:Glow) -> Self { self.$($field)*.effects.base.glow=Some(value); self }
            pub fn hover_style(mut self, patch:impl FnOnce(VisualStyle)->VisualStyle) -> Self { self.$($field)*.effects.states[0]=patch(VisualStyle::default()); self }
            /// Appearance while disabled (`disabled:` variant).
            pub fn disabled_style(mut self, patch:impl FnOnce(VisualStyle)->VisualStyle) -> Self { self.$($field)*.effects.disabled=patch(VisualStyle::default()); self }
            pub fn pressed_style(mut self, patch:impl FnOnce(VisualStyle)->VisualStyle) -> Self { self.$($field)*.effects.states[1]=patch(VisualStyle::default()); self }
            pub fn focus_style(mut self, patch:impl FnOnce(VisualStyle)->VisualStyle) -> Self { self.$($field)*.effects.states[2]=patch(VisualStyle::default()); self }
            pub fn animate_on(mut self, interaction:Interaction, animation:Animation) -> Self { self.$($field)*.effects.animations[interaction.index()]=Some(animation); self }
            /// Animate explicit appearance changes for the same key after recomposition.
            pub fn animation(mut self, animation:Animation) -> Self { self.$($field)*.effects.animation=Some(animation); self }
            pub fn transition(mut self, transition:Transition) -> Self { self.$($field)*.effects.transition=Some(transition); self }
            /// Continuous decorative motion; see [`Ambient`].
            pub fn ambient(mut self, ambient:Ambient) -> Self { self.$($field)*.effects.ambient=Some(ambient); self }
            pub fn font_family(mut self, value: impl Into<String>) -> Self { self.$($field)*.style.font_family = Some(value.into()); self }
            pub fn font_size(mut self, value: f32) -> Self { self.$($field)*.style.font_size = Some(value); self }
            pub fn font_weight(mut self, value: u16) -> Self { self.$($field)*.style.font_weight = Some(value); self }
            /// Horizontal alignment of text lines within the element (inherited).
            pub fn text_align(mut self, value: TextAlign) -> Self { self.$($field)*.style.text_align = Some(value); self }
            /// Line height as a multiple of the font size, e.g. `1.5` (inherited).
            pub fn line_height(mut self, value: f32) -> Self { self.$($field)*.style.line_height = Some(value); self }
            /// Cut text to this many lines, ending with an ellipsis (`line-clamp`).
            pub fn max_lines(mut self, value: u16) -> Self { self.$($field)*.style.max_lines = Some(value); self }
            /// One line with an ellipsis when it does not fit (`truncate`).
            pub fn truncate(mut self) -> Self { self.$($field)*.style.max_lines = Some(1); self }
            /// Keep text on one line; overflow is clipped (`whitespace-nowrap`).
            pub fn no_wrap(mut self, value: bool) -> Self { self.$($field)*.style.no_wrap = Some(value); self }
            pub fn underline(mut self, value: bool) -> Self { self.$($field)*.style.underline = Some(value); self }
            pub fn strikethrough(mut self, value: bool) -> Self { self.$($field)*.style.strikethrough = Some(value); self }
            /// Upper-case the text and label (`uppercase`).
            pub fn uppercase(mut self) -> Self {
                if let Some(t) = self.$($field)*.text.as_mut() { *t = t.to_uppercase(); }
                if let Some(l) = self.$($field)*.label.as_mut() { *l = l.to_uppercase(); }
                self
            }
            /// A font size that changes at breakpoints.
            pub fn responsive_font_size(mut self, value: impl Into<Responsive<f32>>) -> Self { self.$($field)*.ext.responsive_font_size = Some(value.into()); self }
            /// Tracking in em: `0.12` adds 12% of the font size after each character.
            pub fn letter_spacing(mut self, em: f32) -> Self { self.$($field)*.style.letter_spacing = Some(em); self }
            pub fn font_style(mut self, value: FontStyle) -> Self {
                self.$($field)*.style.font_style = Some(match value { FontStyle::Normal => FontSlant::Normal, FontStyle::Italic => FontSlant::Italic }); self
            }
            pub fn italic(self, value: bool) -> Self { self.font_style(if value { FontStyle::Italic } else { FontStyle::Normal }) }
        }
    };
}
modifiers!(Element, [node], [auto_id]);
modifiers!(Text, [0.node], [0.auto_id]);
modifiers!(Badge, [0.node], [0.auto_id]);
modifiers!(Separator, [0.node], [0.auto_id]);
modifiers!(Spacer, [0.node], [0.auto_id]);
modifiers!(Button, [0.node], [0.auto_id]);
modifiers!(Checkable, [0.node], [0.auto_id]);
modifiers!(Container, [0.node], [0.auto_id]);
modifiers!(Stack, [0.node], [0.auto_id]);

/// Retained vector commands; the closure executes during composition, not per frame.
pub fn canvas<C>(draw: impl FnOnce(&mut Canvas)) -> Element<C> {
    let mut element = Element::new(Kind::Canvas);
    draw(&mut element.node.surface.canvas);
    element
}
pub fn text<C>(value: impl Into<String>) -> Text<C> {
    let mut element = Element::new(Kind::Text);
    element.node.text = Some(value.into());
    Text(element)
}
pub fn badge<C>(value: impl Into<String>) -> Badge<C> {
    let mut element = Element::new(Kind::Badge);
    element.node.text = Some(value.into());
    Badge(element)
}
pub fn separator<C>() -> Separator<C> {
    Separator(Element::new(Kind::Separator))
}
/// Horizontal divider; separator() remains an equivalent compatibility name.
pub fn divider<C>() -> Separator<C> {
    separator()
}
/// A noninteractive flexible space along its parent's main layout axis.
/// For fixed space use `.flex(0.).width(...)` / `.height(...)`.
pub fn spacer<C>() -> Spacer<C> {
    let mut element = Element::new(Kind::Spacer);
    element.node.flex = 1.;
    Spacer(element)
}
pub fn button<C>(label: impl Into<String>) -> Button<C> {
    let mut element = Element::new(Kind::Button);
    element.node.label = Some(label.into());
    Button(element)
}
fn checkable<C>(kind: Kind, label: impl Into<String>, checked: bool) -> Checkable<C> {
    let mut element = Element::new(kind);
    element.node.label = Some(label.into());
    element.node.checked = checked;
    Checkable(element)
}
pub fn checkbox<C>(label: impl Into<String>, checked: bool) -> Checkable<C> {
    checkable(Kind::Checkbox, label, checked)
}
pub fn toggle<C>(label: impl Into<String>, checked: bool) -> Checkable<C> {
    checkable(Kind::Toggle, label, checked)
}
impl<C> Text<C> {
    pub fn tone(mut self, value: Tone) -> Self {
        self.0.node.tone = value;
        self
    }
}
impl<C> Badge<C> {
    pub fn tone(mut self, value: Tone) -> Self {
        self.0.node.tone = value;
        self
    }
}
/// Visual emphasis for a button. Every variant resolves through theme tokens,
/// so it follows light/dark mode and accent changes. Later style modifiers
/// (`.background`, `.hover_style`, ...) still override the variant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    /// The theme's standard control surface.
    #[default]
    Neutral,
    /// Filled with the primary colour; the label picks a contrasting colour.
    Primary,
    /// A light primary tint with primary-coloured text: secondary emphasis.
    Soft,
    /// Transparent with a border.
    Outline,
    /// Transparent until hovered: toolbars and inline actions.
    Ghost,
    /// Destructive actions.
    Danger,
}
/// Standard control heights and label sizes, shared by buttons and bars.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ControlSize {
    /// 28px tall, 12px labels.
    Small,
    /// 36px tall, 14px labels.
    #[default]
    Medium,
    /// 44px tall, 16px labels.
    Large,
}
impl ControlSize {
    pub fn height(self) -> f32 {
        match self {
            Self::Small => 28.,
            Self::Medium => 36.,
            Self::Large => 44.,
        }
    }
    pub fn font_size(self) -> f32 {
        match self {
            Self::Small => 12.,
            Self::Medium => 14.,
            Self::Large => 16.,
        }
    }
}
impl<C> Button<C> {
    pub fn enabled(mut self, value: bool) -> Self {
        self.0.node.enabled = value;
        self
    }
    /// Apply a themed emphasis preset (see [`ButtonVariant`]).
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        let style = &mut self.0.node.style;
        style.background = None;
        style.color = None;
        style.border_color = None;
        style.border_width = None;
        self.0.node.primary = variant == ButtonVariant::Primary;
        self.0.node.effects.states = Default::default();
        let tint = |token: ColorToken, percent: u8| token.alpha(percent);
        match variant {
            ButtonVariant::Neutral => self,
            ButtonVariant::Primary => self.primary(true),
            ButtonVariant::Soft => self
                .background(tint(ColorToken::Primary, 16))
                .color(ColorToken::Primary)
                .border_width(0.)
                .hover_style(|s| s.background(tint(ColorToken::Primary, 26)))
                .pressed_style(|s| s.background(tint(ColorToken::Primary, 36))),
            ButtonVariant::Outline => self
                .background(tint(ColorToken::BaseContent, 0))
                .color(ColorToken::BaseContent)
                .border_color(ColorToken::Border)
                .border_width(1.)
                .hover_style(|s| s.background(tint(ColorToken::BaseContent, 6)))
                .pressed_style(|s| s.background(tint(ColorToken::BaseContent, 12))),
            ButtonVariant::Ghost => self
                .background(tint(ColorToken::BaseContent, 0))
                .color(ColorToken::BaseContent)
                .border_width(0.)
                .hover_style(|s| s.background(tint(ColorToken::BaseContent, 8)))
                .pressed_style(|s| s.background(tint(ColorToken::BaseContent, 14))),
            ButtonVariant::Danger => self
                .background(ColorToken::Error)
                .color(ColorToken::ErrorContent)
                .border_width(0.)
                .hover_style(|s| s.opacity(0.9))
                .pressed_style(|s| s.opacity(0.78)),
        }
    }
    /// Standard height and label size (see [`ControlSize`]).
    pub fn size(self, size: ControlSize) -> Self {
        self.height(size.height()).font_size(size.font_size())
    }
    /// Mark a button that opens a menu, popup or section: a chevron is drawn
    /// at the trailing edge (pointing up while `open`) and the label starts
    /// at the leading edge.
    pub fn disclosure(mut self, open: bool) -> Self {
        self.0.node.disclosure = Some(open);
        self
    }
    /// Space before the label at the leading edge (default 10), e.g. to make
    /// room for an icon layered over the button.
    pub fn label_inset(mut self, value: f32) -> Self {
        self.0.node.ext.label_inset = Some(value);
        self
    }
    /// Place the label at the start, centre (default) or end of the button.
    pub fn label_align(mut self, align: Align) -> Self {
        self.0.node.flow = Some(match align {
            Align::Start => Flow::Left,
            Align::End => Flow::Right,
            Align::Center | Align::Stretch => Flow::Center,
        });
        self
    }
    /// Fill with the primary colour. The label uses the paired
    /// `PrimaryContent` colour so an inherited text colour (set on a parent)
    /// cannot make it unreadable; a later `.color(...)` still wins.
    pub fn primary(mut self, value: bool) -> Self {
        const LABEL: &str = "@primary-content";
        self.0.node.primary = value;
        let color = &mut self.0.node.style.color;
        if value && color.is_none() {
            *color = Some(LABEL.into());
        } else if !value && color.as_deref() == Some(LABEL) {
            *color = None;
        }
        self
    }
    pub fn on_click(mut self, callback: impl FnMut(&mut C) + 'static) -> Self {
        self.0.callback = Some(Callback::Click(Box::new(callback)));
        self
    }
}
impl<C> Checkable<C> {
    pub fn enabled(mut self, value: bool) -> Self {
        self.0.node.enabled = value;
        self
    }
    pub fn on_change(mut self, callback: impl FnMut(&mut C, bool) + 'static) -> Self {
        self.0.callback = Some(Callback::Change(Box::new(callback)));
        self
    }
}
fn container<C>(kind: Kind, children: impl IntoIterator<Item = Element<C>>) -> Container<C> {
    let mut element = Element::new(kind);
    element.children = children.into_iter().collect();
    Container(element)
}
pub fn column<C>(children: impl IntoIterator<Item = Element<C>>) -> Container<C> {
    container(Kind::Column, children)
}
pub fn row<C>(children: impl IntoIterator<Item = Element<C>>) -> Container<C> {
    container(Kind::Row, children)
}
/// A padded surface (12px on every edge unless `.padding(...)` overrides it).
pub fn panel<C>(children: impl IntoIterator<Item = Element<C>>) -> Container<C> {
    let mut panel = container(Kind::Panel, children);
    panel.0.node.padding = 12.;
    panel
}
/// Alias for [`column()`]: a vertically arranged stack of children.
pub fn vstack<C>(children: impl IntoIterator<Item = Element<C>>) -> Container<C> {
    column(children)
}
/// Alias for [`row()`]: a horizontally arranged stack of children.
pub fn hstack<C>(children: impl IntoIterator<Item = Element<C>>) -> Container<C> {
    row(children)
}
/// An adaptive stack; defaults to vertical. Fixed stacks do not expose direction.
/// Equal-width columns (two by default); see `columns` and `min_column_width`.
/// Items fill rows left to right, span with `column_span` and share row heights.
pub fn grid<C>(children: impl IntoIterator<Item = Element<C>>) -> Container<C> {
    let mut container = column(children);
    container.0.node.ext.grid = Some(GridColumns::Count(2));
    container
}
pub fn stack<C>(children: impl IntoIterator<Item = Element<C>>) -> Stack<C> {
    Stack(container(Kind::Stack, children).0)
}
/// Overlapping layers, centered by default; later children paint/hit in front.
pub fn zstack<C>(children: impl IntoIterator<Item = Element<C>>) -> Container<C> {
    container(Kind::ZStack, children)
}
impl<C> Stack<C> {
    pub fn direction(mut self, value: impl Into<Responsive<Direction>>) -> Self {
        self.0.node.responsive.direction = Some(value.into());
        self
    }
}
macro_rules! container_modifiers {
    ($target:ident) => {
        container_modifiers!($target, []);
    };
    ($target:ident, [$($generic:ident),*]) => {
        impl<C $(, $generic)*> $target<C $(, $generic)*> {
            /// Logical inner spacing. The last setter affecting an edge wins,
            /// replacing that edge's entire responsive rule set, not merging it.
            pub fn padding(
                mut self,
                value: impl Into<Responsive<f32>>,
                edge: impl Into<Edges>,
            ) -> Self {
                let value = value.into();
                let mask = edge.into().0;
                for edge in (0..4).filter(|i| mask & (1 << i) != 0) {
                    self.0.node.responsive.padding[edge] = Some(value.clone());
                }
                self
            }
            /// Inter-child spacing on the resolved main axis. ZStack ignores gaps.
            pub fn gap(mut self, value: impl Into<Responsive<f32>>, axis: impl Into<Axes>) -> Self {
                let value = value.into();
                let mask = axis.into().0;
                for axis in (0..2).filter(|i| mask & (1 << i) != 0) {
                    self.0.node.responsive.gap[axis] = Some(value.clone());
                }
                self
            }
            /// Opt-in overflow scrolling. Auto enables each axis only if needed.
            pub fn scroll(mut self, mode: Scroll) -> Self {
                self.0.node.surface.scroll = Some(mode);
                self
            }
            /// Keep a vertical scroll view at its end while the reader is there,
            /// as chat transcripts and logs do. Scrolling up releases it;
            /// returning to the end re-engages it. New views start at the end.
            pub fn anchor_end(mut self, value: bool) -> Self {
                self.0.node.surface.anchor_end = value;
                self
            }
            /// Hide/show position indicators without changing overflow or input.
            pub fn scrollbars(mut self, policy: Scrollbars) -> Self {
                self.0.node.surface.scrollbars = policy;
                self
            }
            /// A draggable window region; interactive controls, including disabled controls, take precedence.
            pub fn drag_region(mut self, value: bool) -> Self {
                self.0.node.effects.drag_region = value;
                self
            }
            /// Place content in both axes; for ZStack, place each layer independently.
            pub fn flow(mut self, value: Flow) -> Self {
                self.0.node.flow = Some(value);
                self
            }
            pub fn align(mut self, value: Align) -> Self {
                self.0.node.align = value;
                self
            }
            /// Distribute free main-axis space (`justify-content`).
            pub fn justify(mut self, value: Justify) -> Self {
                self.0.node.ext.justify = Some(value);
                self
            }
            /// Rows only: break onto new lines instead of shrinking children.
            /// The cross-axis gap separates lines.
            pub fn wrap(mut self, value: bool) -> Self {
                self.0.node.ext.wrap = value;
                self
            }
            /// Scroll views: bring the element with `key` to the top whenever
            /// `request` changes (bump it from a callback to scroll again; an
            /// initial 0 does nothing). The reader can scroll freely afterwards.
            pub fn scroll_to(mut self, key: impl Into<String>, request: u32) -> Self {
                self.0.node.ext.scroll_to = Some((key.into(), request));
                self
            }
            /// Grid containers: a fixed number of equal columns.
            pub fn columns(mut self, count: u16) -> Self {
                self.0.node.ext.grid = Some(GridColumns::Count(count));
                self
            }
            /// Grid containers: as many equal columns as fit at this width.
            pub fn min_column_width(mut self, width: f32) -> Self {
                self.0.node.ext.grid = Some(GridColumns::MinWidth(width));
                self
            }
        }
    };
}
container_modifiers!(Container);
container_modifiers!(Stack);
/// Compose heterogeneous Rust builders into a vertical layout.
#[macro_export]
macro_rules! column { ($($child:expr),* $(,)?) => { $crate::ui::column(vec![$($crate::ui::Element::from($child)),*]) }; }
/// Compose heterogeneous Rust builders into a horizontal layout.
#[macro_export]
macro_rules! row { ($($child:expr),* $(,)?) => { $crate::ui::row(vec![$($crate::ui::Element::from($child)),*]) }; }
/// Compose heterogeneous Rust builders into a padded surface/vertical layout.
#[macro_export]
macro_rules! panel { ($($child:expr),* $(,)?) => { $crate::ui::panel(vec![$($crate::ui::Element::from($child)),*]) }; }
pub use crate::{column, panel, row};
/// SwiftUI-familiar alias for [`column!`].
#[macro_export]
macro_rules! vstack { ($($child:expr),* $(,)?) => { $crate::column![$($child),*] }; }
/// SwiftUI-familiar alias for [`row!`].
#[macro_export]
macro_rules! hstack { ($($child:expr),* $(,)?) => { $crate::row![$($child),*] }; }
/// Compose heterogeneous children into an adaptive horizontal/vertical stack.
#[macro_export]
macro_rules! stack { ($($child:expr),* $(,)?) => { $crate::ui::stack(vec![$($crate::ui::Element::from($child)),*]) }; }
/// Compose overlapping layers; declaration order is back to front.
#[macro_export]
macro_rules! zstack { ($($child:expr),* $(,)?) => { $crate::ui::zstack(vec![$($crate::ui::Element::from($child)),*]) }; }
/// Compose heterogeneous children into a grid.
#[macro_export]
macro_rules! grid { ($($child:expr),* $(,)?) => { $crate::ui::grid(vec![$($crate::ui::Element::from($child)),*]) }; }
pub use crate::{grid, hstack, stack, vstack, zstack};
modifiers!(ButtonBar, [0.node], [0.auto_id]);
modifiers!(Toolbar, [0.node], [0.auto_id]);
modifiers!(SelectionControl, [T], [0.node], [0.auto_id]);
modifiers!(Tabs, [T], [0.node], [0.auto_id]);
modifiers!(TextInput, [0.node], [0.auto_id]);
modifiers!(Slider, [0.node], [0.auto_id]);
modifiers!(Select, [T], [0.node], [0.auto_id]);
modifiers!(Dialog, [0.node], [0.auto_id]);
modifiers!(Menu, [T], [0.node], [0.auto_id]);
modifiers!(ContextMenu, [T], [0.node], [0.auto_id]);
modifiers!(Popover, [0.node], [0.auto_id]);
modifiers!(Tooltip, [0.node], [0.auto_id]);
modifiers!(VirtualList, [T, F], [0.node], [0.auto_id]);
modifiers!(Table, [T, K], [0.node], [0.auto_id]);
modifiers!(TreeView, [T], [0.node], [0.auto_id]);
modifiers!(Progress, [0.node], [0.auto_id]);
modifiers!(SplitPane, [0.node], [0.auto_id]);
container_modifiers!(ButtonBar);
container_modifiers!(Toolbar);
modifiers!(Navigation, [T], [0.node], [0.auto_id]);
container_modifiers!(Navigation, [T]);

container_modifiers!(SelectionControl, [T]);
container_modifiers!(Tabs, [T]);
#[macro_export]
macro_rules! button_bar { ($($child:expr),* $(,)?) => { $crate::ui::button_bar(vec![$($crate::ui::Element::from($child)),*]) }; }
#[macro_export]
macro_rules! toolbar { ($($child:expr),* $(,)?) => { $crate::ui::toolbar(vec![$($crate::ui::Element::from($child)),*]) }; }
#[macro_export]
macro_rules! tab_bar { ($($item:expr),* $(,)?) => { $crate::ui::tab_bar(vec![$($item),*]) }; }
#[macro_export]
macro_rules! tabs { ($($item:expr),* $(,)?) => { $crate::ui::tabs(vec![$($item),*]) }; }
pub use crate::{button_bar, tab_bar, tabs, toolbar};
/// Import component functions, types and layout macros together.
pub mod prelude {
    pub use super::{
        accordion, alert, avatar, avatar_group, bar_chart, breadcrumb, carousel, collapsible,
        command, command_palette, drawer, empty_state, hover_card, image_view, kbd, line_chart,
        pagination, skeleton, skeleton_text, sparkline, stat, steps, timeline, toaster,
        CommandPaletteState, DrawerSide, Presence, Severity, TimelineItem, Toast, ToastPosition,
    };
    pub use super::{area_chart, candlestick_chart, donut_chart, stacked_bar_chart, Candle};
    pub use super::{
        badge, button, button_bar, canvas, checkbox, column, context_menu, data_item, dialog,
        divider, grid, hstack, loading, menu, menu_item, menu_separator, menu_submenu, navigation,
        navigation_item, panel, popover, progress, progress_indeterminate, range_slider,
        responsive, row, segmented_control, select, select_item, separator, slider, spacer,
        spinner, split, stack, tab, tab_bar, tab_item, table, table_column, table_row, tabs, text,
        text_area, text_field, toggle, toolbar, tooltip, tree_node, tree_view, virtual_list,
        vstack, zstack, Align, Ambient, Animation, Axes, Axis, BackgroundImage, BorderStyle,
        Breakpoint, ButtonBar, ButtonVariant, Canvas, Clip, ColorToken, ContextMenu, ControlSize,
        Corner, CornerSet, Corners, Cursor, DataItem, Dialog, Direction, Easing, Edge, Edges,
        Element, Flow, FontStyle, Glow, Gradient, GridColumns, Image, ImageFit, ImagePosition,
        Interaction, Justify, Menu, MenuItem, Navigation, NavigationItem, NavigationPlacement,
        Palette, Popover, Progress, Responsive, Rounding, Scroll, Scrollbars, Select, SelectItem,
        SelectionControl, Shadow, Sides, Slider, SortDirection, SplitHandle, SplitPane, Tab,
        TabItem, TabPlacement, Table, TableColumn, TableRow, Tabs, TextAlign, TextInput, ThemeMode,
        Tone, Toolbar, Tooltip, Transition, TreeNode, TreeView, UiTheme, VectorPath, VirtualList,
        VisualStyle,
    };
    pub use super::{
        calendar, date_picker, date_range_picker, time_picker, CalendarDate, DatePickerState,
        DateRange, TimeOfDay,
    };
    pub use super::{
        color_picker, combobox, drop_zone, form_field, number_input, otp_input, radio_group,
        rating, tag_input, ComboboxState, Hsl, TagInputState,
    };
    pub use super::{icon, svg_icon, Icon, IconElement, IconSet, SvgIcon};
}
