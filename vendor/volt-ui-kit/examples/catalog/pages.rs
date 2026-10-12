//! Page metadata and the live demos, code and API for every component.
use super::{
    art::{icon, Icon},
    App, THEMES,
};
use volt_ui_kit::ui::{prelude::*, Element as El};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Page {
    Introduction,
    Theming,
    Accessibility,
    Layout,
    Badge,
    Button,
    ButtonGroup,
    Toolbar,
    Segmented,
    Tabs,
    Checkbox,
    Switch,
    TextField,
    TextArea,
    Slider,
    Select,
    Dialog,
    Menu,
    ContextMenu,
    Popover,
    Tooltip,
    Progress,
    Spinner,
    Toast,
    Card,
    ScrollArea,
    Table,
    TreeView,
    VirtualList,
    SplitPane,
    Canvas,
    Image,
    Effects,
    BackgroundTasks,
    ApplicationLifecycle,
    Documents,
    DesktopServices,
    SignIn,
    Settings,
    Navigation,
    Utilities,
    Icons,
    DateTime,
    FormInputs,
    Disclosure,
    DataDisplay,
    Charts,
}
use Page::*;
pub const PAGES: [Page; 47] = [
    Introduction,
    Theming,
    Accessibility,
    Layout,
    Utilities,
    Icons,
    Badge,
    Button,
    ButtonGroup,
    Toolbar,
    Segmented,
    Tabs,
    Navigation,
    Checkbox,
    Switch,
    TextField,
    TextArea,
    Slider,
    Select,
    DateTime,
    FormInputs,
    Dialog,
    Menu,
    ContextMenu,
    Popover,
    Tooltip,
    Disclosure,
    Progress,
    Spinner,
    Toast,
    Card,
    ScrollArea,
    Table,
    TreeView,
    VirtualList,
    SplitPane,
    Canvas,
    Image,
    DataDisplay,
    Charts,
    Effects,
    BackgroundTasks,
    ApplicationLifecycle,
    Documents,
    DesktopServices,
    SignIn,
    Settings,
];
impl Page {
    pub fn title(self) -> &'static str {
        match self {
            Introduction => "Introduction",
            Theming => "Theming",
            Accessibility => "Accessibility",
            Layout => "Layout",
            Badge => "Badge",
            Button => "Button",
            ButtonGroup => "Button group",
            Toolbar => "Toolbar",
            Segmented => "Segmented control",
            Tabs => "Tabs",
            Checkbox => "Checkbox",
            Switch => "Switch",
            TextField => "Text field",
            TextArea => "Text area",
            Slider => "Slider",
            Select => "Select",
            Dialog => "Dialog",
            Menu => "Menu",
            ContextMenu => "Context menu",
            Popover => "Popover",
            Tooltip => "Tooltip",
            Progress => "Progress",
            Spinner => "Spinner",
            Toast => "Toast",
            Card => "Card",
            ScrollArea => "Scroll area",
            Table => "Table",
            TreeView => "Tree view",
            VirtualList => "Virtual list",
            SplitPane => "Split pane",
            Canvas => "Canvas",
            Image => "Image",
            Effects => "Effects & motion",
            BackgroundTasks => "Background tasks",
            ApplicationLifecycle => "Application lifecycle",
            Documents => "Documents",
            DesktopServices => "Desktop services",
            SignIn => "Sign-in form",
            Settings => "Settings panel",
            Navigation => "Navigation",
            Utilities => "Utilities",
            Icons => "Icons",
            DateTime => "Date & time",
            FormInputs => "Form inputs",
            Disclosure => "Overlays",
            DataDisplay => "Data display",
            Charts => "Charts",
        }
    }
    pub fn slug(self) -> String {
        self.title()
            .to_lowercase()
            .replace(" & ", "-")
            .replace([' ', '&'], "-")
    }
    pub fn group(self) -> &'static str {
        match self {
            Introduction | Theming | Accessibility | Layout | Utilities => "Getting started",
            BackgroundTasks | ApplicationLifecycle | Documents | DesktopServices | SignIn
            | Settings => "Patterns",
            _ => "Components",
        }
    }
    /// Recently added components, flagged in the sidebar.
    pub fn is_new(self) -> bool {
        matches!(
            self,
            Accessibility
                | Utilities
                | Icons
                | DateTime
                | FormInputs
                | Disclosure
                | DataDisplay
                | Charts
                | Menu
                | ContextMenu
                | Popover
                | Tooltip
                | Progress
                | Spinner
                | Table
                | TreeView
                | VirtualList
                | SplitPane
                | BackgroundTasks
                | ApplicationLifecycle
                | Documents
                | DesktopServices
        )
    }
    pub fn summary(self) -> &'static str {
        match self {
            Introduction => {
                "Native desktop components for Rust, declared as ordinary functions of your state."
            }
            Theming => {
                "Semantic colour roles, light and dark palettes, accents and translucent tokens."
            }
            Accessibility => "Named controls, semantic selection and mounted scroll/focus reveal.",
            Layout => "Stacks, layers, spacing, alignment and responsive direction.",
            Badge => "A short status or count, set in a compact pill.",
            Button => "Triggers an action. Six variants, three sizes and a disclosure style.",
            ButtonGroup => "Related actions joined into one divided control.",
            Toolbar => "A horizontal row of actions with flexible spacing.",
            Segmented => "Pick one of a few mutually exclusive options.",
            Tabs => "Switch between views; only the active page is mounted.",
            Checkbox => "Toggle an option on or off, with a label.",
            Switch => "An on/off setting that takes effect immediately.",
            TextField => "Single-line text entry with placeholder, password and read-only modes.",
            TextArea => "Multi-line text with soft wrap, selection, undo and IME.",
            Slider => "Choose a value from a continuous or stepped range.",
            Select => "Choose one option from a dropdown list.",
            Dialog => "A modal card that captures focus until dismissed.",
            Menu => "A list of actions opened from a trigger.",
            ContextMenu => "Actions for a surface, opened with a right click.",
            Popover => "Rich content anchored to a trigger.",
            Tooltip => "Short help shown on hover or keyboard focus.",
            Progress => "Shows how far a task has come, or that it is under way.",
            Spinner => "Indicates indeterminate work in a small space.",
            Toast => "A brief notification that confirms an action.",
            Card => "A padded surface that groups related content.",
            ScrollArea => "Opt-in vertical, horizontal or nested scrolling.",
            Table => "Rows and columns with sorting and selection.",
            TreeView => "Hierarchical items that expand and collapse.",
            VirtualList => "Thousands of rows; only the visible window is built.",
            SplitPane => "Two panes divided by a draggable handle.",
            Canvas => "Retained vector paths: fills, strokes and transforms.",
            Image => "Image backgrounds with fit, position, opacity and clipping.",
            Effects => "Shadows, glows, state styles, keyed animation and transitions.",
            BackgroundTasks => "Worker threads report progress through a bounded message queue.",
            ApplicationLifecycle => "State-aware commands and guarded closing for unsaved work.",
            Documents => "Native file panels and asynchronous document saving.",
            DesktopServices => "Explicit local notifications and shared native tray commands.",
            SignIn => "Inputs, validation and actions composed into a form.",
            Settings => "Grouped preferences with switches, selects and sliders.",
            Navigation => {
                "Independent controlled destination navigation: sidebar, rail and bottom placement."
            }
            Utilities => "Grid, wrapping, justify, typography, gradients, borders and motion.",
            Icons => "A built-in, theme-aware vector icon set.",
            DateTime => "Calendars, date and range pickers, and a time picker.",
            FormInputs => {
                "Radio groups, steppers, comboboxes, tags, ranges, colours, codes, ratings and drops."
            }
            Disclosure => "Collapsibles, accordions, drawers, toasts, a command palette and rich menus.",
            DataDisplay => "Avatars, alerts, skeletons, breadcrumbs, pagination, steps and more.",
            Charts => "Sparklines, bar charts and line charts drawn with vector paths.",
        }
    }
}

/// One example on a page: a live demo and the code that produced it.
pub struct Section {
    pub id: &'static str,
    pub title: &'static str,
    pub note: &'static str,
    pub demo: El<App>,
    pub code: &'static str,
}
pub struct Doc {
    pub sections: Vec<Section>,
    pub api: Vec<(&'static str, &'static str)>,
}
fn section(
    id: &'static str,
    title: &'static str,
    note: &'static str,
    demo: impl Into<El<App>>,
    code: &'static str,
) -> Section {
    Section {
        id,
        title,
        note,
        demo: demo.into(),
        code,
    }
}
fn line(items: Vec<El<App>>) -> El<App> {
    hstack(items)
        .gap(12., Axis::Horizontal)
        .align(Align::Center)
        .into()
}
fn noop(_: &mut App) {}
fn muted(value: impl Into<String>) -> volt_ui_kit::ui::Text<App> {
    text(value).font_size(13.).color(ColorToken::Muted)
}
fn label(value: impl Into<String>) -> volt_ui_kit::ui::Text<App> {
    text(value).font_size(14.).font_weight(500)
}

pub fn doc(page: Page, app: &App) -> Doc {
    match page {
        Introduction => Doc {
            sections: vec![],
            api: vec![],
        },
        Theming => theming(app),
        Accessibility => accessibility_doc(app),
        Layout => layout(),
        Badge => badge_doc(),
        Button => button_doc(app),
        ButtonGroup => button_group(),
        Toolbar => toolbar_doc(),
        Segmented => segmented(app),
        Tabs => tabs_doc(app),
        Checkbox => checkbox_doc(app),
        Switch => switch_doc(app),
        TextField => text_field_doc(app),
        TextArea => text_area_doc(app),
        Slider => slider_doc(app),
        Select => select_doc(app),
        Dialog => dialog_doc(app),
        Menu => menu_doc(app),
        ContextMenu => context_doc(app),
        Popover => popover_doc(app),
        Tooltip => tooltip_doc(),
        Progress => progress_doc(app),
        Spinner => spinner_doc(),
        Toast => toast_doc(),
        Card => card_doc(),
        ScrollArea => scroll_doc(app),
        Table => table_doc(app),
        TreeView => tree_doc(app),
        VirtualList => virtual_doc(app),
        SplitPane => split_doc(app),
        Canvas => canvas_doc(),
        Image => image_doc(app),
        Effects => effects_doc(app),
        BackgroundTasks => tasks_doc(app),
        ApplicationLifecycle => super::lifecycle::doc(app),
        Documents => super::documents::doc(app),
        DesktopServices => super::desktop_services::doc(app),
        SignIn => sign_in(app),
        Settings => settings_doc(app),
        Navigation => navigation_doc(app),
        Utilities => super::kit::utilities_doc(),
        Icons => super::kit::icons_doc(),
        DateTime => super::kit::dates_doc(app),
        FormInputs => super::kit::forms_doc(app),
        Disclosure => super::kit::disclosure_doc(app),
        DataDisplay => super::kit::display_doc(app),
        Charts => super::kit::charts_doc(),
    }
}

// ------------------------------------------------------------ Getting started

fn accessibility_doc(app: &App) -> Doc {
    let controls = vstack![
        hstack![
            label("Profile name"),
            text_field(&app.name)
                .key("ax-name")
                .accessible_label("Profile name")
                .placeholder("Your full name")
                .width(240.)
                .on_change(|app: &mut App, value| app.name = value),
            zstack![
                button("")
                    .key("ax-notifications")
                    .accessible_label("Show notifications")
                    .variant(ButtonVariant::Outline)
                    .width(36.)
                    .height(36.)
                    .on_click(|app: &mut App| app.notify("No new notifications")),
                icon(Icon::Bell, 18., ColorToken::BaseContent),
            ]
            .width(36.)
            .height(36.),
        ]
        .gap(12., Axis::Horizontal)
        .align(Align::Center),
        tab_bar([tab_item(0u8, "Overview"), tab_item(1, "Details")])
            .key("ax-tabs")
            .accessible_label("Profile views")
            .selected(app.tab)
            .on_select(|app: &mut App, value| app.tab = value),
        segmented_control([(0u8, "Quiet"), (1, "Balanced"), (2, "All")])
            .key("ax-segment")
            .accessible_label("Notification level")
            .selected(app.segment)
            .on_select(|app: &mut App, value| app.segment = value),
        muted(
            "Names are independent of painted text. Selected tabs and choices have semantic state."
        ),
    ]
    .gap(16., Axis::Vertical);
    let actions = vstack((0..12).map(|i| -> El<App> {
        button(format!("Action {}", i + 1))
            .key(format!("ax-action-{i}"))
            .variant(ButtonVariant::Outline)
            .height(36.)
            .fill_width()
            .on_click(move |app: &mut App| app.notify(format!("Action {} activated", i + 1)))
            .into()
    }))
    .key("ax-scroll")
    .accessible_label("Scrollable actions")
    .width(360.)
    .height(156.)
    .gap(8., Axis::Vertical)
    .padding(12., Edge::All)
    .scroll(Scroll::Vertical)
    .border_width(1.)
    .border_color(ColorToken::Border)
    .rounded(10.);
    Doc {
        sections: vec![
            section("access-names", "Names and selection", "Try Tab, arrow keys and the icon action. Password values remain withheld.", controls,
                r#"text_field(&app.name).accessible_label("Profile name").placeholder("Your full name")
button("").accessible_label("Show notifications").on_click(show_notifications)
tab_bar(items).selected(app.tab).on_select(select_tab)
segmented_control(options).selected(app.level).on_select(select_level)"#),
            section("access-scroll", "Scroll and focus reveal", "Tab or an assistive focus request reveals mounted actions below the viewport. Scroll actions target this container only.", actions,
                r#"vstack(actions).key("actions").accessible_label("Scrollable actions")
    .height(156.).scroll(Scroll::Vertical)
// Runtime::accessibility_focus(key) reveals a mounted off-screen control.
// Native AccessKit routes ScrollUp/Down/Left/Right and ScrollIntoView."#),
        ],
        api: vec![
            (".accessible_label(name)", "Nonempty, control-free name, up to 8 KiB; does not alter painted text."),
            ("Tabs / segmented / select / table / tree", "Automatic roles and selected state; tree items also expose expansion."),
            ("Runtime::accessibility_focus(key)", "Focus and reveal a mounted enabled control in the current modal scope."),
            ("Runtime::accessibility_reveal(key)", "Reveal without changing keyboard focus."),
            ("Runtime::accessibility_scroll(key, [dx, dy], …)", "Page a viewport; each direction is -1, 0 or 1. Virtual windows dispatch normally."),
            ("Platform acceptance", "VoiceOver/Narrator/Orca and unmounted virtual-row navigation still require validation/work."),
        ],
    }
}

fn theming(app: &App) -> Doc {
    let swatch = |token: ColorToken, name: &'static str| -> El<App> {
        vstack![
            panel![]
                .height(44.)
                .padding(0., Edge::All)
                .background(token)
                .border_color(ColorToken::Border)
                .border_width(1.)
                .rounded(8.),
            text(name)
                .font_size(12.)
                .font_family("SF Mono")
                .color(ColorToken::Muted),
        ]
        .gap(6., Axis::Vertical)
        .width(112.)
        .into()
    };
    let roles = [
        (ColorToken::Base100, "base-100"),
        (ColorToken::Base200, "base-200"),
        (ColorToken::Base300, "base-300"),
        (ColorToken::BaseContent, "base-content"),
        (ColorToken::Primary, "primary"),
        (ColorToken::Border, "border"),
        (ColorToken::Muted, "muted"),
        (ColorToken::Success, "success"),
        (ColorToken::Warning, "warning"),
        (ColorToken::Error, "error"),
    ];
    let grid = vstack![
        hstack(
            roles[..5]
                .iter()
                .map(|(t, n)| swatch(*t, n))
                .collect::<Vec<_>>()
        )
        .gap(12., Axis::Horizontal),
        hstack(
            roles[5..]
                .iter()
                .map(|(t, n)| swatch(*t, n))
                .collect::<Vec<_>>()
        )
        .gap(12., Axis::Horizontal),
    ]
    .gap(16., Axis::Vertical);
    let themes = hstack(
        THEMES
            .iter()
            .enumerate()
            .map(|(i, (name, light, _))| {
                button(*name)
                    .key(format!("theme-pick-{i}"))
                    .size(ControlSize::Small)
                    .background(format!("#{light:06X}"))
                    .color("#FFFFFF")
                    .border_color(ColorToken::BaseContent)
                    .border_width(if app.theme == i { 2. } else { 0. })
                    .on_click(move |app: &mut App| app.theme = i)
                    .into()
            })
            .collect::<Vec<_>>(),
    )
    .gap(8., Axis::Horizontal);
    let modes = segmented_control([
        (ThemeMode::Light, "Light"),
        (ThemeMode::Dark, "Dark"),
        (ThemeMode::System, "System"),
    ])
    .key("theme-mode")
    .selected(app.mode)
    .on_select(|app: &mut App, m| app.mode = m);
    let tints = line(
        [100u8, 60, 30, 15, 6]
            .into_iter()
            .map(|a| {
                panel![text(format!("{a}%")).font_size(12.)]
                    .width(72.)
                    .height(44.)
                    .flow(Flow::Center)
                    .background(ColorToken::Primary.alpha(a))
                    .color(if a > 50 {
                        ColorToken::PrimaryContent
                    } else {
                        ColorToken::BaseContent
                    })
                    .rounded(8.)
                    .into()
            })
            .collect(),
    );
    Doc {
        sections: vec![
            section("theme-roles", "Semantic roles", "Components draw with roles, not literal colours, so one palette restyles everything.", grid, r#"text("Saved").color(ColorToken::Success)
panel![...].background(ColorToken::Base200).border_color(ColorToken::Border)"#),
            section("theme-pick", "Accent and mode", "Try a colour and a mode; this whole site re-themes live.", vstack![themes, modes].gap(16., Axis::Vertical), r#"fn theme(app: &App, system: ThemeMode) -> UiTheme {
    let base = if app.dark { Palette::dark() } else { Palette::light() };
    UiTheme::from_palette(base.with_accent([0.15, 0.39, 0.92, 1.]))
}
native::run_ui_with_theme(options, app, view, theme)?;"#),
            section("theme-alpha", "Translucent tokens", "Any role at an opacity, still resolved from the active theme.", tints, r#"panel![...].background(ColorToken::Primary.alpha(15)) // "@primary/15""#),
            section("theme-window", "Window surfaces", "The preview shows surface alpha, not desktop blur. Native GPU windows can request transparency/OS blur; text stays opaque. Software falls back to opaque.",
                zstack![
                    vstack![].background_image(app.image.clone().background().fit(ImageFit::Cover)),
                    panel![
                        text(if app.glass { "Translucent window requested" } else { "Opaque window (default)" }).font_weight(600),
                        muted("Background alpha is separate from content opacity."),
                        badge("SF interface · SF Mono source").font_family("SF Mono").font_size(11.),
                    ].width(340.).background(ColorToken::Base100.alpha(88))
                        .border_color(ColorToken::Border).border_width(1.).rounded(12.)
                        .padding(18., Edge::All).gap(8., Axis::Vertical),
                ].width(460.).height(210.).flow(Flow::Center).rounded(14.).clip(Clip::Bounds),
                r#"cargo run --release --features native-gpu --example catalog -- \
    --page theming --theme dark --background blur --window-opacity 0.86
# --background transparent: alpha without a blur request
# --background opaque: solid window; software fallback
# blur/material: public AppKit material; compositor: opt-in private macOS blur."#),
        ],
        api: vec![
            ("ColorToken::alpha(percent)", "A role at 0..=100% opacity, e.g. \"@primary/15\"."),
            ("Palette::with_accent(rgba)", "Set primary/accent and pick contrasting foregrounds."),
            ("UiTheme::from_palette(palette)", "Build a theme from a fully custom palette."),
            ("run_ui_with_theme(options, state, view, theme)", "Derive the theme from application state."),
            ("Catalog: --background / --window-opacity", "Independent background opacity and tint; foreground stays opaque. Select Material or Compositor in the sidebar."),
        ],
    }
}

fn layout() -> Doc {
    let tile = |label: &str| -> El<App> {
        panel![text(label.to_string()).font_size(12.)]
            .padding(10., Edge::All)
            .background(ColorToken::Base200)
            .border_color(ColorToken::Border)
            .border_width(1.)
            .rounded(8.)
            .into()
    };
    let stacks = hstack![
        vstack![tile("One"), tile("Two"), tile("Three")]
            .gap(8., Axis::Vertical)
            .width(120.),
        hstack![tile("Left"), spacer(), tile("Right")]
            .gap(8., Axis::Horizontal)
            .width(260.),
        zstack![
            panel![]
                .width(120.)
                .height(80.)
                .padding(0., Edge::All)
                .background(ColorToken::Primary.alpha(20))
                .rounded(10.),
            badge("On top"),
        ]
        .width(120.)
        .height(80.),
    ]
    .gap(24., Axis::Horizontal)
    .align(Align::Center);
    let flows = hstack(
        [
            (Flow::TopLeft, "Top left"),
            (Flow::Center, "Center"),
            (Flow::BottomRight, "Bottom right"),
        ]
        .into_iter()
        .map(|(flow, name)| {
            panel![badge(name)]
                .width(150.)
                .height(90.)
                .flow(flow)
                .background(ColorToken::Base200)
                .rounded(10.)
                .into()
        })
        .collect::<Vec<_>>(),
    )
    .gap(12., Axis::Horizontal);
    let adaptive = stack![tile("Sidebar"), tile("Content")]
        .direction(responsive(Direction::Vertical).at(Breakpoint::Lg, Direction::Horizontal))
        .gap(12., Axis::Both);
    Doc {
        sections: vec![
            section(
                "layout-stacks",
                "Stacks and layers",
                "vstack, hstack with a spacer, and a zstack drawing back to front.",
                stacks,
                r#"vstack![a, b, c].gap(8., Axis::Vertical)
hstack![left, spacer(), right]
zstack![background, badge("On top")]"#,
            ),
            section(
                "layout-flow",
                "Placement",
                "Place content anywhere inside a container with flow.",
                flows,
                r#"panel![badge("Center")].height(90.).flow(Flow::Center)"#,
            ),
            section(
                "layout-responsive",
                "Responsive direction",
                "Stacks switch direction at breakpoints; resize the window past 1024px.",
                adaptive,
                r#"stack![sidebar, content]
    .direction(responsive(Direction::Vertical).at(Breakpoint::Lg, Direction::Horizontal))"#,
            ),
        ],
        api: vec![
            (
                "vstack! / hstack! / zstack! / stack!",
                "Vertical, horizontal, layered and responsive stacks.",
            ),
            (
                ".gap(value, Axis)",
                "Space between children on one or both axes.",
            ),
            (
                ".padding(value, Edge)",
                "Inner space on any combination of edges.",
            ),
            (".flow(Flow)", "Place content: corners, edges or centre."),
            (".align(Align)", "Cross-axis alignment, including Stretch."),
            ("spacer()", "Flexible space that pushes siblings apart."),
            (
                "responsive(v).at(Breakpoint, v)",
                "A value that changes with window width.",
            ),
        ],
    }
}

// ------------------------------------------------------------ Actions

fn badge_doc() -> Doc {
    let tones = line(vec![
        badge("Default").into(),
        badge("Primary")
            .background(ColorToken::Primary)
            .color(ColorToken::PrimaryContent)
            .into(),
        badge("Success")
            .background(ColorToken::Success.alpha(15))
            .color(ColorToken::Success)
            .into(),
        badge("Warning")
            .background(ColorToken::Warning.alpha(18))
            .color(ColorToken::Warning)
            .into(),
        badge("Error")
            .background(ColorToken::Error.alpha(15))
            .color(ColorToken::Error)
            .into(),
        badge("Outline")
            .background(ColorToken::Base100)
            .border_color(ColorToken::Border)
            .border_width(1.)
            .into(),
    ]);
    let pills = line(vec![
        badge("New")
            .rounded(Rounding::Full)
            .background(ColorToken::Primary)
            .color(ColorToken::PrimaryContent)
            .into(),
        badge("12").rounded(Rounding::Full).font_weight(700).into(),
        badge("Beta").rounded(Rounding::Full).font_size(11.).into(),
    ]);
    Doc {
        sections: vec![
            section(
                "badge-tones",
                "Tones",
                "Semantic colours with translucent backgrounds.",
                tones,
                r#"badge("Success")
    .background(ColorToken::Success.alpha(15))
    .color(ColorToken::Success)"#,
            ),
            section(
                "badge-pill",
                "Pill",
                "Fully rounded for counts and labels.",
                pills,
                r#"badge("New").rounded(Rounding::Full)
    .background(ColorToken::Primary).color(ColorToken::PrimaryContent)"#,
            ),
        ],
        api: vec![
            ("badge(text)", "A compact, non-interactive label."),
            (".tone(Tone)", "Body, Muted, Accent or Title colouring."),
            (".rounded(Rounding::Full)", "Pill shape."),
        ],
    }
}

fn button_doc(app: &App) -> Doc {
    let variants = line(vec![
        button("Primary")
            .key("b-primary")
            .variant(ButtonVariant::Primary)
            .on_click(noop)
            .into(),
        button("Neutral").key("b-neutral").on_click(noop).into(),
        button("Soft")
            .key("b-soft")
            .variant(ButtonVariant::Soft)
            .on_click(noop)
            .into(),
        button("Outline")
            .key("b-outline")
            .variant(ButtonVariant::Outline)
            .on_click(noop)
            .into(),
        button("Ghost")
            .key("b-ghost")
            .variant(ButtonVariant::Ghost)
            .on_click(noop)
            .into(),
        button("Delete")
            .key("b-danger")
            .variant(ButtonVariant::Danger)
            .on_click(noop)
            .into(),
    ]);
    let sizes = line(vec![
        button("Small")
            .key("b-small")
            .size(ControlSize::Small)
            .variant(ButtonVariant::Outline)
            .on_click(noop)
            .into(),
        button("Medium")
            .key("b-medium")
            .size(ControlSize::Medium)
            .variant(ButtonVariant::Outline)
            .on_click(noop)
            .into(),
        button("Large")
            .key("b-large")
            .size(ControlSize::Large)
            .variant(ButtonVariant::Outline)
            .on_click(noop)
            .into(),
    ]);
    let states = line(vec![
        button("Disabled")
            .key("b-disabled")
            .variant(ButtonVariant::Primary)
            .enabled(false)
            .on_click(noop)
            .into(),
        button(if app.liked { "Liked" } else { "Like" })
            .key("b-like")
            .variant(if app.liked {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Outline
            })
            .animation(Animation::ease_out().duration_ms(180))
            .on_click(|app: &mut App| app.liked = !app.liked)
            .into(),
        button("Sort by date")
            .key("b-disclosure")
            .disclosure(false)
            .width(150.)
            .variant(ButtonVariant::Outline)
            .on_click(noop)
            .into(),
        button("Left aligned")
            .key("b-align")
            .label_align(Align::Start)
            .width(150.)
            .on_click(noop)
            .into(),
    ]);
    Doc {
        sections: vec![
            section(
                "button-variants",
                "Variants",
                "Emphasis presets built from theme tokens.",
                variants,
                r#"button("Primary").variant(ButtonVariant::Primary).on_click(save)
button("Soft").variant(ButtonVariant::Soft)
button("Outline").variant(ButtonVariant::Outline)
button("Ghost").variant(ButtonVariant::Ghost)
button("Delete").variant(ButtonVariant::Danger)"#,
            ),
            section(
                "button-sizes",
                "Sizes",
                "28, 36 and 44 pixels tall with matching type.",
                sizes,
                r#"button("Small").size(ControlSize::Small)
button("Large").size(ControlSize::Large)"#,
            ),
            section(
                "button-states",
                "States",
                "Disabled, toggled with animation, disclosure and label alignment.",
                states,
                r#"button(if app.liked { "Liked" } else { "Like" })
    .variant(if app.liked { ButtonVariant::Primary } else { ButtonVariant::Outline })
    .animation(Animation::ease_out().duration_ms(180))
    .on_click(|app: &mut App| app.liked = !app.liked)
button("Sort by date").disclosure(false)"#,
            ),
        ],
        api: vec![
            (
                "button(label)",
                "A clickable control; needs .on_click(...).",
            ),
            (
                ".variant(ButtonVariant)",
                "Neutral, Primary, Soft, Outline, Ghost or Danger.",
            ),
            (".size(ControlSize)", "Small, Medium or Large."),
            (
                ".disclosure(open)",
                "Trailing chevron for menus and popups.",
            ),
            (".label_align(Align)", "Start, centre or end."),
            (".enabled(bool)", "Disable interaction and dim the button."),
            (
                ".key(id)",
                "Stable identity for focus, animation and tests.",
            ),
        ],
    }
}

fn button_group() -> Doc {
    let joined = button_bar![
        button("Years").key("g-years").on_click(noop),
        button("Months").key("g-months").on_click(noop),
        button("Days").key("g-days").on_click(noop),
    ]
    .joined(true);
    let outline = button_bar![
        button("Copy")
            .key("g-copy")
            .variant(ButtonVariant::Outline)
            .on_click(noop),
        button("Paste")
            .key("g-paste")
            .variant(ButtonVariant::Outline)
            .on_click(noop),
    ]
    .gap(8., Axis::Horizontal);
    Doc {
        sections: vec![
            section(
                "group-joined",
                "Joined",
                "Inner corners square off; 1px seams divide the segments.",
                joined,
                r#"button_bar![
    button("Years").on_click(years),
    button("Months").on_click(months),
    button("Days").on_click(days),
].joined(true)"#,
            ),
            section(
                "group-spaced",
                "Spaced",
                "The same group with a gap.",
                outline,
                r#"button_bar![copy, paste].gap(8., Axis::Horizontal)"#,
            ),
        ],
        api: vec![
            ("button_bar![...]", "A horizontal group of buttons."),
            (".joined(true)", "Fuse into one control with seams."),
            (
                ".rounded(r) / .rounded_on(r, corners)",
                "Outer corner radius of the group.",
            ),
        ],
    }
}

fn toolbar_doc() -> Doc {
    let bar = toolbar![
        button("Back")
            .key("tb-back")
            .variant(ButtonVariant::Ghost)
            .size(ControlSize::Small)
            .on_click(noop),
        button("Forward")
            .key("tb-forward")
            .variant(ButtonVariant::Ghost)
            .size(ControlSize::Small)
            .enabled(false)
            .on_click(noop),
        spacer(),
        badge("Draft"),
        button("Share")
            .key("tb-share")
            .variant(ButtonVariant::Outline)
            .size(ControlSize::Small)
            .on_click(noop),
        button("Publish")
            .key("tb-publish")
            .variant(ButtonVariant::Primary)
            .size(ControlSize::Small)
            .on_click(noop),
    ]
    .width(560.)
    .padding(8., Edge::All)
    .background(ColorToken::Base200)
    .rounded(10.);
    Doc {
        sections: vec![section(
            "toolbar",
            "Toolbar",
            "Ghost actions on the left, primary actions on the right.",
            bar,
            r#"toolbar![
    button("Back").variant(ButtonVariant::Ghost).size(ControlSize::Small).on_click(back),
    spacer(),
    badge("Draft"),
    button("Publish").variant(ButtonVariant::Primary).on_click(publish),
]"#,
        )],
        api: vec![
            ("toolbar![...]", "A centred horizontal row with 8px gaps."),
            ("spacer()", "Push following items to the far edge."),
        ],
    }
}

// ------------------------------------------------------------ Selection

fn segmented(app: &App) -> Doc {
    let control = segmented_control([(0u8, "Day"), (1, "Week"), (2, "Month"), (3, "Year")])
        .key("seg")
        .selected(app.segment)
        .on_select(|app: &mut App, v| app.segment = v);
    let small = segmented_control([(0u8, "List"), (1, "Grid")])
        .key("seg-small")
        .selected(app.segment % 2)
        .on_select(|app: &mut App, v| app.segment = v)
        .height(28.)
        .font_size(12.)
        .rounded(Rounding::Full);
    Doc {
        sections: vec![
            section(
                "seg-basic",
                "Segmented control",
                "Arrow keys move the selection; the selected item is never re-sent.",
                vstack![control, muted(format!("Selected value: {}", app.segment))]
                    .gap(12., Axis::Vertical),
                r#"segmented_control([(0, "Day"), (1, "Week"), (2, "Month")])
    .selected(app.range)
    .on_select(|app: &mut App, v| app.range = v)"#,
            ),
            section(
                "seg-pill",
                "Pill",
                "Compact and fully rounded.",
                small,
                r#"segmented_control([(0, "List"), (1, "Grid")])
    .height(28.).font_size(12.).rounded(Rounding::Full)"#,
            ),
        ],
        api: vec![
            (
                "segmented_control([(value, label), ...])",
                "Typed values, one selected.",
            ),
            (".selected(value) / .on_select(f)", "Controlled selection."),
            (".enabled(bool)", "Disable every option."),
        ],
    }
}

fn tabs_doc(app: &App) -> Doc {
    let panel_text = |t: &str| -> El<App> { muted(t.to_string()).into() };
    let tabbed = tabs([
        tab(
            0u8,
            "Account",
            panel_text("Make changes to your account here. Only this page is mounted."),
        ),
        tab(1, "Password", panel_text("Change your password here.")),
        tab(
            2,
            "Billing",
            panel_text("Billing is disabled on this workspace."),
        )
        .enabled(false),
    ])
    .key("tabs")
    .selected(app.tab)
    .on_select(|app: &mut App, t| app.tab = t)
    .width(420.);
    Doc {
        sections: vec![section(
            "tabs",
            "Tabs",
            "Content for inactive tabs is never built.",
            tabbed,
            r#"tabs([
    tab(0, "Account", account_page(app)),
    tab(1, "Password", password_page(app)),
    tab(2, "Billing", billing_page(app)).enabled(false),
])
.selected(app.tab)
.on_select(|app: &mut App, t| app.tab = t)"#,
        )],
        api: vec![
            (
                "tabs([tab(value, label, content), ...])",
                "A tab strip and the active content.",
            ),
            (
                ".placement(TabPlacement::Bottom)",
                "Put the strip below the content.",
            ),
            (
                "tab_bar([tab_item(...)])",
                "The strip alone, for custom content.",
            ),
        ],
    }
}

fn checkbox_doc(app: &App) -> Doc {
    let basic = vstack![
        checkbox("Accept terms and conditions", app.terms)
            .key("cb-terms")
            .on_change(|app: &mut App, v| app.terms = v),
        checkbox("Send me product updates", app.checked)
            .key("cb-updates")
            .on_change(|app: &mut App, v| app.checked = v),
        checkbox("Disabled", true)
            .key("cb-disabled")
            .enabled(false)
            .on_change(|_: &mut App, _| {}),
    ]
    .gap(10., Axis::Vertical);
    Doc {
        sections: vec![section(
            "checkbox",
            "Checkbox",
            "Space toggles a focused checkbox.",
            basic,
            r#"checkbox("Accept terms and conditions", app.terms)
    .on_change(|app: &mut App, checked| app.terms = checked)"#,
        )],
        api: vec![
            (
                "checkbox(label, checked)",
                "Controlled: pass state, update it in on_change.",
            ),
            (".accent_color(token)", "Colour of the checked box."),
            (".enabled(bool)", "Disable interaction."),
        ],
    }
}

fn switch_doc(app: &App) -> Doc {
    let basic = vstack![
        toggle("Airplane mode", app.airplane)
            .key("sw-air")
            .on_change(|app: &mut App, v| app.airplane = v),
        toggle("Wi-Fi", app.switch_on)
            .key("sw-wifi")
            .on_change(|app: &mut App, v| app.switch_on = v),
        toggle("Bluetooth", true)
            .key("sw-bt")
            .enabled(false)
            .on_change(|_: &mut App, _| {}),
    ]
    .gap(10., Axis::Vertical);
    Doc {
        sections: vec![section(
            "switch",
            "Switch",
            "Use for settings that apply immediately.",
            basic,
            r#"toggle("Airplane mode", app.airplane)
    .on_change(|app: &mut App, on| app.airplane = on)"#,
        )],
        api: vec![
            ("toggle(label, on)", "A controlled switch."),
            (".accent_color(token)", "Colour when on."),
        ],
    }
}

// ------------------------------------------------------------ Inputs

fn text_field_doc(app: &App) -> Doc {
    let valid = app.email.is_empty() || app.email.contains('@');
    let fields = vstack![
        vstack![
            label("Name"),
            text_field(&app.name)
                .key("tf-name")
                .placeholder("Your name")
                .on_change(|app: &mut App, v| app.name = v)
        ]
        .gap(6., Axis::Vertical),
        vstack![
            label("Email"),
            text_field(&app.email)
                .key("tf-email")
                .placeholder("you@example.com")
                .border_color(if valid {
                    ColorToken::Border
                } else {
                    ColorToken::Error
                })
                .on_change(|app: &mut App, v| app.email = v),
            text(if valid {
                "We'll never share your email."
            } else {
                "Enter an email address with an @."
            })
            .font_size(12.)
            .color(if valid {
                ColorToken::Muted
            } else {
                ColorToken::Error
            }),
        ]
        .gap(6., Axis::Vertical),
    ]
    .gap(16., Axis::Vertical)
    .width(340.);
    let modes = vstack![
        text_field(&app.password)
            .key("tf-pass")
            .placeholder("Password")
            .password(true)
            .on_change(|app: &mut App, v| app.password = v),
        text_field(concat!("volt-ui-kit ", env!("CARGO_PKG_VERSION")))
            .key("tf-ro")
            .read_only(true)
            .on_change(|_: &mut App, _| {}),
        text_field("Disabled")
            .key("tf-off")
            .enabled(false)
            .on_change(|_: &mut App, _| {}),
    ]
    .gap(10., Axis::Vertical)
    .width(340.);
    Doc {
        sections: vec![
            section(
                "tf-label",
                "With label and help",
                "Validation is plain Rust: derive styles from state.",
                fields,
                r#"let valid = app.email.is_empty() || app.email.contains('@');
text_field(&app.email)
    .placeholder("you@example.com")
    .border_color(if valid { ColorToken::Border } else { ColorToken::Error })
    .on_change(|app: &mut App, v| app.email = v)"#,
            ),
            section(
                "tf-modes",
                "Password, read-only and disabled",
                "Passwords are never copied or exposed to screen readers.",
                modes,
                r#"text_field(&app.password).password(true)
text_field(version).read_only(true)
text_field("Disabled").enabled(false)"#,
            ),
        ],
        api: vec![
            ("text_field(value)", "Controlled single-line input."),
            (".on_change(f) / .on_submit(f)", "Text edits and Enter."),
            (".placeholder(text)", "Hint shown while empty."),
            (
                ".password(true) / .read_only(true)",
                "Masked or selectable-only text.",
            ),
            (
                "Shortcuts",
                "Cmd/Ctrl A, C, X, V, Z; word and line navigation; IME.",
            ),
        ],
    }
}

fn text_area_doc(app: &App) -> Doc {
    let area = vstack![
        label("Bio"),
        text_area(&app.bio)
            .key("ta-bio")
            .soft_wrap(true)
            .height(120.)
            .on_change(|app: &mut App, v| app.bio = v),
        muted(format!("{} characters", app.bio.chars().count())).font_size(12.),
    ]
    .gap(6., Axis::Vertical)
    .width(420.);
    Doc {
        sections: vec![section(
            "text-area",
            "Text area",
            "Soft-wrapped multi-line editing with a live count.",
            area,
            r#"text_area(&app.bio)
    .soft_wrap(true)
    .height(120.)
    .on_change(|app: &mut App, v| app.bio = v)"#,
        )],
        api: vec![
            ("text_area(value)", "Controlled multi-line input."),
            (".soft_wrap(true)", "Wrap long lines to the width."),
            (".height(px)", "Visible height; content scrolls inside."),
        ],
    }
}

fn slider_doc(app: &App) -> Doc {
    let volume = vstack![
        hstack![
            label("Volume"),
            spacer(),
            muted(format!("{:.0}%", app.volume * 100.))
        ],
        slider(app.volume)
            .key("sl-volume")
            .width(320.)
            .on_change(|app: &mut App, v| app.volume = v),
    ]
    .gap(8., Axis::Vertical)
    .width(320.);
    let price = vstack![
        hstack![
            label("Max price"),
            spacer(),
            muted(format!("${:.0}", app.price))
        ],
        slider(app.price)
            .key("sl-price")
            .range(0.0..=200.0)
            .step(10.)
            .width(320.)
            .on_change(|app: &mut App, v| app.price = v),
        slider(0.3)
            .key("sl-off")
            .width(320.)
            .enabled(false)
            .on_change(|_: &mut App, _| {}),
    ]
    .gap(8., Axis::Vertical)
    .width(320.);
    Doc {
        sections: vec![
            section(
                "slider",
                "Slider",
                "Drag, click the track, or use arrow, Page and Home/End keys.",
                volume,
                r#"slider(app.volume).on_change(|app: &mut App, v| app.volume = v)"#,
            ),
            section(
                "slider-step",
                "Range and step",
                "Values snap to the step.",
                price,
                r#"slider(app.price).range(0.0..=200.0).step(10.)"#,
            ),
        ],
        api: vec![
            (
                "slider(value)",
                "Controlled value in its range (default 0..=1).",
            ),
            (".range(min..=max) / .step(s)", "Bounds and snapping."),
            (".enabled(bool)", "Disable interaction."),
        ],
    }
}

fn select_doc(app: &App) -> Doc {
    let fruits = ["Apple", "Banana", "Blueberry", "Grapes", "Pineapple"];
    let fruit = select(
        fruits
            .iter()
            .enumerate()
            .map(|(i, f)| select_item(i as u8, *f)),
    )
    .key("select-fruit")
    .selected(app.fruit)
    .open(app.fruit_open)
    .width(220.)
    .on_select(|app: &mut App, v| app.fruit = v)
    .on_open_change(|app: &mut App, open| app.fruit_open = open);
    let zones = [
        "Pacific (UTC−8)",
        "Mountain (UTC−7)",
        "Central (UTC−6)",
        "Eastern (UTC−5)",
        "London (UTC+0)",
        "Tokyo (UTC+9)",
    ];
    let tz = select(
        zones
            .iter()
            .enumerate()
            .map(|(i, z)| select_item(i as u8, *z).enabled(i != 1)),
    )
    .key("select-tz")
    .selected(app.timezone)
    .open(app.timezone_open)
    .width(260.)
    .on_select(|app: &mut App, v| app.timezone = v)
    .on_open_change(|app: &mut App, open| app.timezone_open = open);
    Doc {
        sections: vec![
            section(
                "select",
                "Select",
                "Escape or a click outside closes it; arrow keys move through options.",
                fruit,
                r#"select(fruits.iter().enumerate().map(|(i, f)| select_item(i, *f)))
    .key("fruit")
    .selected(app.fruit)
    .open(app.fruit_open)
    .on_select(|app: &mut App, v| app.fruit = v)
    .on_open_change(|app: &mut App, open| app.fruit_open = open)"#,
            ),
            section(
                "select-disabled",
                "Disabled options",
                "Disabled options are skipped by keyboard and pointer.",
                tz,
                r#"select_item(1, "Mountain (UTC−7)").enabled(false)"#,
            ),            section(
                "select-vs-combobox",
                "Select or combobox?",
                "Same field look, different jobs. Select: a short fixed list, picked by click or arrow keys. Combobox: a long list you search by typing.",
                {
                    let countries = [
                        "Argentina", "Australia", "Austria", "Belgium", "Brazil", "Canada",
                        "Chile", "Denmark", "Finland", "France", "Germany", "India",
                        "Ireland", "Italy", "Japan", "Kenya", "Mexico", "Netherlands",
                        "New Zealand", "Norway", "Portugal", "Singapore", "South Korea",
                        "Spain", "Sweden", "Switzerland", "United Kingdom", "United States",
                    ];
                    hstack![
                        vstack![
                            muted("Select · 5 fruits"),
                            text(fruits[app.fruit as usize]).font_size(14.).font_weight(500),
                            muted("(the first example above)").font_size(12.),
                        ]
                        .gap(6., Axis::Vertical)
                        .width(220.),
                        vstack![
                            muted("Combobox · 28 countries"),
                            combobox(
                                countries.iter().enumerate().map(|(i, c)| (i as u8, *c)),
                                &app.kit.combo,
                            )
                            .key("select-country")
                            .selected(app.kit.framework)
                            .placeholder("Search countries…")
                            .width(240.)
                            .state(|app: &mut App| &mut app.kit.combo)
                            .on_select(|app: &mut App, v| app.kit.framework = Some(v)),
                        ]
                        .gap(6., Axis::Vertical),
                    ]
                    .gap(40., Axis::Horizontal)
                },
                r#"// Few options, no typing:
select(sizes).key("size").selected(app.size)
// Many options, type to filter:
combobox(countries, &app.country_search)
    .key("country")
    .state(|app: &mut App| &mut app.country_search)
    .on_select(|app: &mut App, c| app.country = Some(c))"#,
            ),
        ],
        api: vec![
            (
                "select(items)",
                "A trigger and a popup list; requires .key(...).",
            ),
            (
                "select_item(value, label)",
                "A typed option; .enabled(false) to disable.",
            ),
            (".open(bool) / .on_open_change(f)", "Controlled open state."),
        ],
    }
}

// ------------------------------------------------------------ Overlays

fn dialog_doc(app: &App) -> Doc {
    let edit = vstack![
        button("Edit profile")
            .key("dlg-open")
            .variant(ButtonVariant::Outline)
            .on_click(|app: &mut App| app.dialog_open = true),
        button("Delete account")
            .key("dlg-delete")
            .variant(ButtonVariant::Danger)
            .on_click(|app: &mut App| app.delete_open = true),
    ]
    .gap(10., Axis::Vertical);
    let _ = app;
    Doc {
        sections: vec![section("dialog", "Dialog", "Focus is trapped inside and restored when it closes. Escape and the backdrop dismiss it.", edit, r#"dialog(vstack![
    text_field(&app.name).on_change(|app: &mut App, v| app.name = v),
    button("Save changes").variant(ButtonVariant::Primary).on_click(close),
])
.key("edit")
.title("Edit profile")
.open(app.dialog_open)
.on_dismiss(|app: &mut App| app.dialog_open = false)"#)],
        api: vec![
            ("dialog(content)", "A modal card above the window; requires .key(...)."),
            (".title(text)", "Heading for the card."),
            (".open(bool) / .on_dismiss(f)", "Controlled visibility."),
            (".backdrop_dismiss(false) / .escape_dismiss(false)", "Require an explicit choice."),
        ],
    }
}

/// Dialogs for the Dialog page; rendered by the shell above all content.
pub fn dialogs(app: &App) -> Vec<El<App>> {
    vec![
        dialog(vstack![
            muted("Make changes to your profile here. Click save when you're done."),
            vstack![label("Name"), text_field(&app.name).key("dlg-name").on_change(|app: &mut App, v| app.name = v)].gap(6., Axis::Vertical),
            hstack![spacer(), button("Save changes").key("dlg-save").variant(ButtonVariant::Primary).on_click(|app: &mut App| {
                app.dialog_open = false;
                app.notify("Profile saved");
            })],
        ]
        .gap(16., Axis::Vertical))
        .key("dlg-edit")
        .title("Edit profile")
        .open(app.dialog_open)
        .on_dismiss(|app: &mut App| app.dialog_open = false)
        .rounded(12.)
        .background(ColorToken::Base100)
        .into(),
        dialog(vstack![
            muted("This permanently deletes your account and removes your data. This cannot be undone."),
            hstack![
                spacer(),
                button("Cancel").key("del-cancel").variant(ButtonVariant::Outline).on_click(|app: &mut App| app.delete_open = false),
                button("Delete").key("del-confirm").variant(ButtonVariant::Danger).on_click(|app: &mut App| {
                    app.delete_open = false;
                    app.notify("Account deleted (not really)");
                }),
            ]
            .gap(8., Axis::Horizontal),
        ]
        .gap(18., Axis::Vertical))
        .key("dlg-del")
        .title("Are you absolutely sure?")
        .open(app.delete_open)
        .backdrop_dismiss(false)
        .on_dismiss(|app: &mut App| app.delete_open = false)
        .rounded(12.)
        .background(ColorToken::Base100)
        .into(),
    ]
}

fn menu_doc(app: &App) -> Doc {
    let m = menu(
        "Options",
        [
            menu_item(0u8, "New file"),
            menu_item(1, "Duplicate"),
            menu_item(2, "Share").enabled(false),
            menu_separator(),
            menu_item(3, "Delete").danger(true),
        ],
    )
    .key("menu-options")
    .open(app.menu_open)
    .width(180.)
    .on_action(|app: &mut App, id| {
        app.last_action = ["New file", "Duplicate", "Share", "Delete"][id as usize].into();
    })
    .on_open_change(|app: &mut App, open| app.menu_open = open);
    Doc {
        sections: vec![section(
            "menu",
            "Menu",
            "Separators, disabled items and a destructive action.",
            vstack![m, muted(format!("Last action: {}", app.last_action))].gap(12., Axis::Vertical),
            r#"menu("Options", [
    menu_item(0, "New file"),
    menu_item(2, "Share").enabled(false),
    menu_separator(),
    menu_item(3, "Delete").danger(true),
])
.open(app.menu_open)
.on_action(|app: &mut App, id| run(app, id))
.on_open_change(|app: &mut App, open| app.menu_open = open)"#,
        )],
        api: vec![
            (
                "menu(label, items)",
                "A disclosure trigger and popup actions.",
            ),
            ("menu_item(value, label)", ".enabled(false), .danger(true)."),
            ("menu_separator()", "A divider between groups."),
        ],
    }
}

fn context_doc(app: &App) -> Doc {
    let surface = panel![muted("Right-click here")]
        .key("ctx-surface")
        .width(360.)
        .height(140.)
        .flow(Flow::Center)
        .background(ColorToken::Base100)
        .border_color(ColorToken::Border)
        .border_width(1.)
        .rounded(10.);
    let ctx = context_menu(
        surface,
        [
            menu_item(0u8, "Back"),
            menu_item(1, "Reload"),
            menu_separator(),
            menu_item(2, "Inspect"),
        ],
    )
    .key("ctx")
    .open(app.context_open)
    .position(app.context_at)
    .on_context_open(|app: &mut App, at| {
        app.context_at = at;
        app.context_open = true;
    })
    .on_open_change(|app: &mut App, open| app.context_open = open)
    .on_action(|app: &mut App, id| {
        app.last_action = ["Back", "Reload", "Inspect"][id as usize].into()
    });
    Doc {
        sections: vec![section(
            "context",
            "Context menu",
            "Opens at the pointer; the application owns its position.",
            vstack![ctx, muted(format!("Last action: {}", app.last_action))]
                .gap(12., Axis::Vertical),
            r#"context_menu(surface, [menu_item(0, "Back"), menu_item(1, "Reload")])
    .key("ctx")
    .open(app.context_open)
    .position(app.context_at)
    .on_context_open(|app: &mut App, at| { app.context_at = at; app.context_open = true; })
    .on_open_change(|app: &mut App, open| app.context_open = open)"#,
        )],
        api: vec![
            (
                "context_menu(target, items)",
                "Right-click actions for a subtree.",
            ),
            (
                ".position([x, y]) / .on_context_open(f)",
                "Where it opened, in logical pixels.",
            ),
        ],
    }
}

fn popover_doc(app: &App) -> Doc {
    let trigger = button("Dimensions")
        .key("pop-trigger")
        .variant(ButtonVariant::Outline)
        .disclosure(app.popover_open)
        .width(160.)
        .on_click(|app: &mut App| app.popover_open = !app.popover_open);
    let card = popover(
        vstack![
            text("Dimensions").font_weight(600),
            muted("Set the dimensions for the layer."),
            hstack![
                label("Width").width(64.),
                slider(app.width / 200.)
                    .key("pop-w")
                    .width(150.)
                    .on_change(|app: &mut App, v| app.width = (v * 200.).round())
            ]
            .align(Align::Center),
            hstack![
                label("Height").width(64.),
                slider(app.height / 200.)
                    .key("pop-h")
                    .width(150.)
                    .on_change(|app: &mut App, v| app.height = (v * 200.).round())
            ]
            .align(Align::Center),
            muted(format!("{:.0} × {:.0} px", app.width, app.height)),
        ]
        .gap(12., Axis::Vertical),
    )
    .key("pop")
    .anchor("pop-trigger")
    .open(app.popover_open)
    .on_dismiss(|app: &mut App| app.popover_open = false);
    Doc {
        sections: vec![section(
            "popover",
            "Popover",
            "Interactive content anchored to its trigger.",
            vstack![trigger, card],
            r#"button("Dimensions").key("trigger").disclosure(app.open).on_click(toggle)
popover(vstack![title, width_slider, height_slider])
    .key("dimensions")
    .anchor("trigger")
    .open(app.open)
    .on_dismiss(|app: &mut App| app.open = false)"#,
        )],
        api: vec![
            ("popover(content)", "An anchored card; requires .key(...)."),
            (".anchor(key)", "The element it attaches to."),
            (".open(bool) / .on_dismiss(f)", "Controlled visibility."),
        ],
    }
}

fn tooltip_doc() -> Doc {
    let tips = line(vec![
        tooltip(
            "Add to library",
            button("Hover me")
                .key("tip-a")
                .variant(ButtonVariant::Outline)
                .on_click(noop),
        )
        .key("tip-1")
        .into(),
        tooltip(
            "Keyboard focus shows it too",
            button("Or focus me")
                .key("tip-b")
                .variant(ButtonVariant::Outline)
                .on_click(noop),
        )
        .key("tip-2")
        .into(),
    ]);
    Doc {
        sections: vec![section(
            "tooltip",
            "Tooltip",
            "Non-interactive help; no callback or timer needed.",
            tips,
            r#"tooltip("Add to library", button("Hover me").on_click(add))
    .key("add-tip")"#,
        )],
        api: vec![(
            "tooltip(text, target)",
            "Help shown on hover or focus; requires .key(...).",
        )],
    }
}

// ------------------------------------------------------------ Feedback

fn progress_doc(app: &App) -> Doc {
    let bars = vstack![
        progress(app.volume).fill_width(),
        hstack![spacer(), muted(format!("{:.0}%", app.volume * 100.))],
        slider(app.volume)
            .key("pg-drive")
            .fill_width()
            .on_change(|app: &mut App, v| app.volume = v),
    ]
    .gap(8., Axis::Vertical)
    .width(360.);
    let busy = vstack![progress_indeterminate().fill_width(), muted("Syncing…")]
        .gap(8., Axis::Vertical)
        .width(360.);
    Doc {
        sections: vec![
            section(
                "progress",
                "Determinate",
                "Drive it from any state; here, the slider.",
                bars,
                r#"progress(app.done).fill_width()"#,
            ),
            section(
                "progress-busy",
                "Indeterminate",
                "Animates while work is under way; settles with reduced motion.",
                busy,
                r#"progress_indeterminate().fill_width()"#,
            ),
        ],
        api: vec![
            ("progress(0.0..=1.0)", "A determinate bar."),
            ("progress_indeterminate()", "Animated activity bar."),
            (".period_ms(ms)", "Animation period."),
            (".fill_width()", "Use the parent's width."),
        ],
    }
}

fn spinner_doc() -> Doc {
    let s = line(vec![
        spinner().into(),
        loading("Loading projects…").into(),
        button("Saving…")
            .key("sp-btn")
            .enabled(false)
            .on_click(noop)
            .into(),
    ]);
    Doc {
        sections: vec![section(
            "spinner",
            "Spinner and loading",
            "For short, indeterminate waits.",
            s,
            r#"spinner()
loading("Loading projects…")"#,
        )],
        api: vec![
            ("spinner()", "A 20px activity indicator."),
            ("loading(label)", "A spinner with a label."),
        ],
    }
}

fn toast_doc() -> Doc {
    let t = line(vec![
        button("Show toast")
            .key("toast-show")
            .variant(ButtonVariant::Outline)
            .on_click(|app: &mut App| app.notify("Event has been created"))
            .into(),
        button("Show success")
            .key("toast-ok")
            .variant(ButtonVariant::Soft)
            .on_click(|app: &mut App| app.notify("Changes saved"))
            .into(),
    ]);
    Doc {
        sections: vec![section(
            "toast",
            "Toast",
            "These buttons use the catalog's own toaster: a non-modal stack in the corner; the page stays usable.",
            t,
            r#"zstack![
    page,
    toaster(&app.toasts)
        .on_dismiss(|app: &mut App, id| app.toasts.retain(|t| t.id != id)),
]
// elsewhere:
app.toasts.push(Toast::new(id, "Changes saved").tone(Severity::Success));"#,
        )],
        api: vec![
            ("toaster(&toasts)", "A corner stack; layer it last in a zstack."),
            ("Toast::new(id, title)", ".description / .tone(Severity::…) / .action(label)"),
            (".position(ToastPosition::…)", "Top or bottom; left, centre or right."),
            (".on_dismiss / .on_action", "Remove the toast or run its action."),
            ("Timing", "Remove toasts from a worker via UiSender for auto-dismiss."),
        ],
    }
}

// ------------------------------------------------------------ Display

fn card_doc() -> Doc {
    let c = vstack![
        vstack![
            text("Create project").font_size(18.).font_weight(600),
            muted("Deploy your new project in one click.")
        ]
        .gap(4., Axis::Vertical),
        vstack![
            label("Name"),
            text_field("")
                .key("card-name")
                .placeholder("Name of your project")
                .on_change(|_: &mut App, _| {})
        ]
        .gap(6., Axis::Vertical),
        divider().background(ColorToken::Border),
        hstack![
            button("Cancel")
                .key("card-cancel")
                .variant(ButtonVariant::Outline)
                .on_click(noop),
            spacer(),
            button("Deploy")
                .key("card-deploy")
                .variant(ButtonVariant::Primary)
                .on_click(noop),
        ],
    ]
    .width(360.)
    .padding(24., Edge::All)
    .gap(18., Axis::Vertical)
    .background(ColorToken::Base100)
    .border_color(ColorToken::Border)
    .border_width(1.)
    .rounded(12.)
    .shadow(Shadow::new().color("#0000000F").blur(10.).offset(0., 2.));
    Doc {
        sections: vec![section(
            "card",
            "Card",
            "Any container becomes a card with a surface, border, radius and shadow.",
            c,
            r#"vstack![header, form, divider(), actions]
    .padding(24., Edge::All)
    .background(ColorToken::Base100)
    .border_color(ColorToken::Border).border_width(1.)
    .rounded(12.)
    .shadow(Shadow::new().blur(10.).offset(0., 2.))"#,
        )],
        api: vec![
            ("panel![...]", "A padded surface (12px by default)."),
            ("divider()", "A horizontal rule."),
            (".shadow(Shadow) / .rounded(r)", "Elevation and shape."),
        ],
    }
}

fn scroll_doc(app: &App) -> Doc {
    let tags = vstack(
        (1..=40)
            .map(|i| -> El<App> {
                vstack![
                    text(format!("v1.2.0-beta.{i}")).font_size(13.),
                    divider().background(ColorToken::Border)
                ]
                .gap(8., Axis::Vertical)
                .into()
            })
            .collect::<Vec<_>>(),
    )
    .key("scroll-tags")
    .width(220.)
    .height(220.)
    .padding(12., Edge::All)
    .gap(8., Axis::Vertical)
    .scroll(Scroll::Vertical)
    .border_color(ColorToken::Border)
    .border_width(1.)
    .rounded(10.);
    let gallery = hstack(
        (0..8)
            .map(|i| -> El<App> {
                panel![muted(format!("Item {}", i + 1))]
                    .width(140.)
                    .height(100.)
                    .flow(Flow::Center)
                    .background(ColorToken::Base200)
                    .rounded(10.)
                    .into()
            })
            .collect::<Vec<_>>(),
    )
    .key("scroll-row")
    .width(420.)
    .gap(12., Axis::Horizontal)
    .scroll(Scroll::Horizontal)
    .scrollbars(Scrollbars::Hidden);
    let log = vstack![
        vstack(
            app.log_entries
                .iter()
                .enumerate()
                .map(|(i, entry)| -> El<App> {
                    text(entry.clone())
                        .key(format!("log-line-{i}"))
                        .font_family("SF Mono")
                        .font_size(12.)
                        .height(32.)
                        .into()
                })
        )
        .key("follow-log")
        .height(160.)
        .scroll(Scroll::Vertical)
        .anchor_end(true)
        .padding(8., Edge::Horizontal)
        .background(ColorToken::Base200)
        .rounded(8.)
        .clip(Clip::Rounded),
        text_area(&app.log_draft)
            .key("log-composer")
            .accessible_label("Log message")
            .placeholder("Enter to append; Shift+Enter for a new line")
            .soft_wrap(true)
            .submit_on_enter(true)
            .height(64.)
            .on_change(|app: &mut App, value| app.log_draft = value)
            .on_submit(App::append_log),
        button("Append event")
            .key("log-append")
            .on_click(|app: &mut App| {
                if app.log_draft.trim().is_empty() {
                    app.log_draft =
                        format!("Event {} · callback delivered", app.log_entries.len() + 1);
                }
                app.append_log();
            }),
    ]
    .gap(12., Axis::Vertical)
    .width(420.);
    Doc {
        sections: vec![
            section(
                "scroll-v",
                "Vertical",
                "Wheel, trackpad and draggable scrollbars.",
                tags,
                r#"vstack(tags).height(220.).scroll(Scroll::Vertical)"#,
            ),
            section(
                "scroll-h",
                "Horizontal, hidden bars",
                "Scrollbars can be hidden without losing scrolling.",
                gallery,
                r#"hstack(items).scroll(Scroll::Horizontal).scrollbars(Scrollbars::Hidden)"#,
            ),
            section("scroll-follow", "Follow the newest event", "A bounded live log. Scroll up to read history; return to the end to follow new events. The multiline composer uses Rust submission callbacks.", log,
                r#"vstack(lines).scroll(Scroll::Vertical).anchor_end(true)
text_area(&app.draft).submit_on_enter(true)
    .on_change(|app: &mut App, value| app.draft = value)
    .on_submit(App::append_log)"#),
        ],
        api: vec![
            (
                ".scroll(Scroll::Vertical | Horizontal | Both | Auto)",
                "Opt-in overflow scrolling.",
            ),
            (".anchor_end(true)", "Follow growth only while the reader is at the end; scroll up to release."),
            ("text_area(...).submit_on_enter(true)", "Enter submits; Shift+Enter inserts a newline; Cmd/Ctrl+Enter still submits."),
            (
                ".scrollbars(Scrollbars::Hidden)",
                "Hide indicators, keep scrolling.",
            ),
        ],
    }
}

// ------------------------------------------------------------ Data

const INVOICES: [(&str, &str, &str, &str); 6] = [
    ("INV001", "Paid", "Credit card", "$250.00"),
    ("INV002", "Pending", "PayPal", "$150.00"),
    ("INV003", "Unpaid", "Bank transfer", "$350.00"),
    ("INV004", "Paid", "Credit card", "$450.00"),
    ("INV005", "Paid", "PayPal", "$550.00"),
    ("INV006", "Pending", "Bank transfer", "$200.00"),
];
fn invoice_table(app: &App, square: bool) -> El<App> {
    let key = if square {
        "invoices-square"
    } else {
        "invoices"
    };
    let row_prefix = if square { "inv-square" } else { "inv" };
    let mut t = table(
        [
            table_column(0u8, "Invoice").width(130.).sortable(true),
            table_column(1, "Status").width(120.).sortable(true),
            table_column(2, "Method").width(160.).sortable(false),
            table_column(3, "Amount")
                .width(110.)
                .align(Align::End)
                .sortable(true),
        ],
        INVOICES.iter().enumerate().map(|(i, r)| {
            table_row(i as u32, [r.0, r.1, r.2, r.3]).key(format!("{row_prefix}-{i}"))
        }),
    )
    .key(key)
    .width(600.)
    .rounded(if square { 0. } else { 12. })
    .accessible_label(if square {
        "Square invoices"
    } else {
        "Rounded invoices"
    })
    .selected(app.row)
    .on_select(|app: &mut App, r| app.row = Some(r))
    .on_sort(|app: &mut App, column, direction| app.sort = Some((column, direction)));
    if let Some((column, direction)) = app.sort {
        t = t.sort(column, direction);
    }
    t.into()
}

fn table_doc(app: &App) -> Doc {
    let status = || {
        muted(format!(
            "{} invoices{} · sorting and selection are shared between both previews",
            INVOICES.len(),
            app.row.map_or(String::new(), |i| format!(
                " · {} selected",
                INVOICES[i as usize].0
            ))
        ))
        .font_size(12.)
        .height(36.)
    };
    Doc {
        sections: vec![
            section(
                "table",
                "Rounded table",
                "12px outer corners. Sort a header or select a row; the frame stays intact.",
                vstack![invoice_table(app, false), status()].gap(12., Axis::Vertical),
                r#"table(columns, rows)
.key("invoices")
.rounded(12.) // default outer corners
.selected(app.row)
.on_select(|app: &mut App, row| app.row = Some(row))
.on_sort(|app: &mut App, column, direction| app.sort = Some((column, direction)))"#,
            ),
            section(
                "table-square",
                "Square table",
                "The same working component with 0px corners, not a static mockup.",
                vstack![invoice_table(app, true), status()].gap(12., Axis::Vertical),
                r#"table(columns, rows)
.key("invoices-square")
.rounded(0.) // square outer frame and inset rows
.selected(app.row)
.on_select(|app: &mut App, row| app.row = Some(row))
.on_sort(|app: &mut App, column, direction| app.sort = Some((column, direction)))"#,
            ),
        ],
        api: vec![
            (".rounded(12.) / .rounded(0.)", "Rounded by default; explicit square corners retain the same sorting, selection and scrolling."),
            (
                "table(columns, rows)",
                "Up to 8 columns and 96 rows; requires .key(...).",
            ),
            (
                "table_column(value, label).width(px)",
                "Minimum width; wider tables grow columns proportionally. Sortable unless .sortable(false).",
            ),
            ("table_row(value, cells).key(id)", "A typed row."),
            ("table_column(...).align(Align::Start | Center | End)", "Align header and values together; End for amounts."),
            (".width(px).height(px)", "Bounded viewports scroll on either axis rather than cut off columns."),
            (
                ".sort(column, direction) / .sort_by(compare)",
                "Sorted view; custom ordering.",
            ),
            (
                ".selected(value) / .on_select(f) / .on_sort(f)",
                "Controlled selection and sorting.",
            ),
        ],
    }
}

fn tree_doc(app: &App) -> Doc {
    let t = tree_view([
        tree_node(1u16, "src").key("t-src").children([
            tree_node(2, "components").key("t-comp").children([
                tree_node(4, "button.rs").key("t-button"),
                tree_node(5, "dialog.rs").key("t-dialog"),
            ]),
            tree_node(3, "lib.rs").key("t-lib"),
        ]),
        tree_node(6, "Cargo.toml").key("t-cargo"),
        tree_node(7, "README.md").key("t-readme"),
    ])
    .key("files")
    .width(360.)
    .accessible_label("Project files")
    .expanded(app.expanded.iter().copied().filter(|id| *id <= 7))
    .selected(app.file.filter(|id| *id <= 7))
    .on_select(|app: &mut App, f| app.file = Some(f))
    .on_expand(|app: &mut App, id, open| {
        app.expanded.retain(|v| *v != id);
        if open {
            app.expanded.push(id);
        }
    });
    Doc {
        sections: vec![section(
            "tree",
            "Tree view",
            "Expand folders and select files with pointer or keyboard.",
            vstack![
                hstack![
                    text("Project files").font_size(13.).font_weight(600),
                    spacer(),
                    muted("7 items").font_size(12.)
                ]
                .width(360.),
                t,
                muted("Select a row; use the chevron to expand a branch.").font_size(12.),
            ]
            .gap(12., Axis::Vertical),
            r#"tree_view([
    tree_node(1, "src").key("src").children([tree_node(3, "lib.rs").key("lib")]),
    tree_node(6, "Cargo.toml").key("cargo"),
])
.key("files")
.expanded(app.expanded.clone())
.selected(app.file)
.on_select(|app: &mut App, f| app.file = Some(f))
.on_expand(|app: &mut App, id, open| set_expanded(app, id, open))"#,
        )],
        api: vec![
            ("tree_view(roots)", "Up to 128 keyed nodes."),
            (
                "tree_node(value, label).key(id).children([...])",
                "A node and its children.",
            ),
            (".expanded(values) / .on_expand(f)", "Controlled expansion."),
        ],
    }
}

fn virtual_doc(app: &App) -> Doc {
    let list = virtual_list(10_000, 36., |i| {
        data_item(i, format!("Record {i:05}")).key(format!("rec-{i}"))
    })
    .key("records")
    .width(360.)
    .height(260.)
    .window(app.window.clone())
    .overscan(2)
    .selected(app.record)
    .on_window_change(|app: &mut App, w| app.window = w)
    .on_select(|app: &mut App, i| app.record = Some(i));
    Doc {
        sections: vec![section(
            "virtual",
            "10,000 rows",
            "Only the visible window and a little overscan are composed.",
            vstack![
                list,
                muted(format!(
                    "Composed rows {}..{}{}",
                    app.window.start,
                    app.window.end,
                    app.record
                        .map(|r| format!(", selected {r:05}"))
                        .unwrap_or_default()
                )),
            ]
            .gap(10., Axis::Vertical),
            r#"virtual_list(10_000, 36., |i| data_item(i, format!("Record {i:05}")).key(format!("rec-{i}")))
    .key("records")
    .height(260.)
    .window(app.window.clone())
    .on_window_change(|app: &mut App, w| app.window = w)
    .on_select(|app: &mut App, i| app.record = Some(i))"#,
        )],
        api: vec![
            (
                "virtual_list(count, row_height, factory)",
                "Fixed-height rows built on demand.",
            ),
            (
                ".window(range) / .on_window_change(f)",
                "Which rows are mounted.",
            ),
            (".overscan(n)", "Extra rows above and below."),
        ],
    }
}

fn split_doc(app: &App) -> Doc {
    let pane = |title: &str| -> El<App> {
        vstack![
            text(title.to_string()).font_weight(600),
            muted("Drag the divider, or focus it and use the arrow keys.")
        ]
        .gap(6., Axis::Vertical)
        .padding(16., Edge::All)
        .into()
    };
    let s = split(pane("Sidebar"), pane("Content"))
        .key("split")
        .width(520.)
        .height(200.)
        .ratio(app.split)
        .min_first(120.)
        .min_second(160.)
        .on_resize(|app: &mut App, r| app.split = r);
    Doc {
        sections: vec![section(
            "split",
            "Split pane",
            "The application owns the ratio.",
            vstack![s, muted(format!("Ratio {:.0}%", app.split * 100.))].gap(10., Axis::Vertical),
            r#"split(sidebar, content)
    .ratio(app.split)
    .min_first(120.).min_second(160.)
    .on_resize(|app: &mut App, r| app.split = r)"#,
        )],
        api: vec![
            ("split(first, second)", "Two panes and a draggable handle."),
            (".axis(Direction::Vertical)", "Stack panes vertically."),
            (".ratio(r) / .on_resize(f)", "Controlled divider position."),
            (".min_first(px) / .min_second(px)", "Minimum pane sizes."),
        ],
    }
}

// ------------------------------------------------------------ Graphics

fn canvas_doc() -> Doc {
    let chart = canvas(|c| {
        let points = [0.62, 0.48, 0.70, 0.55, 0.82, 0.66, 0.90, 0.74];
        let (w, h) = (360., 160.);
        for i in 0..=4 {
            let y = 10. + i as f32 * (h - 20.) / 4.;
            c.stroke(
                VectorPath::new().move_to(0., y).line_to(w, y),
                ColorToken::Border,
                1.,
            );
        }
        let at = |i: usize| [i as f32 * w / 7., h - 10. - points[i] * (h - 30.)];
        let mut area = VectorPath::new().move_to(0., h - 10.);
        let mut stroke = VectorPath::new().move_to(at(0)[0], at(0)[1]);
        for i in 0..points.len() {
            let [x, y] = at(i);
            area = area.line_to(x, y);
            if i > 0 {
                stroke = stroke.line_to(x, y);
            }
        }
        c.fill(
            area.line_to(w, h - 10.).close(),
            ColorToken::Primary.alpha(14),
        );
        c.stroke(stroke, ColorToken::Primary, 2.5);
        for i in 0..points.len() {
            let [x, y] = at(i);
            c.fill(VectorPath::circle(x, y, 3.5), ColorToken::Primary);
        }
    })
    .width(360.)
    .height(160.);
    let shapes = canvas(|c| {
        c.fill(VectorPath::circle(40., 40., 32.), ColorToken::Primary);
        c.fill(
            VectorPath::rectangle(90., 10., 60., 60.),
            ColorToken::Base300,
        );
        c.stroke(
            VectorPath::ellipse(210., 40., 40., 26.),
            ColorToken::BaseContent,
            2.,
        );
        c.fill(
            VectorPath::new()
                .move_to(270., 70.)
                .line_to(300., 10.)
                .line_to(330., 70.)
                .close()
                .transformed([1., 0., 0., 1., 0., 0.]),
            ColorToken::Warning,
        );
    })
    .width(340.)
    .height(80.);
    Doc {
        sections: vec![
            section(
                "canvas-chart",
                "Chart",
                "Paths, fills and translucent tokens; the closure runs once per composition.",
                chart,
                r#"canvas(|c| {
    c.fill(area, ColorToken::Primary.alpha(14));
    c.stroke(line, ColorToken::Primary, 2.5);
})
.width(360.).height(160.)"#,
            ),
            section(
                "canvas-shapes",
                "Shapes",
                "Circles, rectangles, ellipses and arbitrary paths.",
                shapes,
                r#"c.fill(VectorPath::circle(40., 40., 32.), ColorToken::Primary);
c.stroke(VectorPath::ellipse(210., 40., 40., 26.), ColorToken::BaseContent, 2.);"#,
            ),
        ],
        api: vec![
            (
                "canvas(|c| { ... })",
                "Retained vector drawing in logical pixels.",
            ),
            (
                "VectorPath::new().move_to().line_to().cubic_to().close()",
                "Build any path.",
            ),
            (".transformed([a, b, c, d, tx, ty])", "Affine transform."),
            (
                "c.fill(path, colour) / c.stroke(path, colour, width)",
                "Paint a path.",
            ),
        ],
    }
}

fn image_doc(app: &App) -> Doc {
    let fits = line(
        [
            (ImageFit::Cover, "Cover"),
            (ImageFit::Contain, "Contain"),
            (ImageFit::Tile, "Tile"),
        ]
        .into_iter()
        .map(|(fit, name)| -> El<App> {
            vstack![
                vstack![]
                    .width(150.)
                    .height(100.)
                    .background(ColorToken::Base200)
                    .background_image(app.image.clone().background().fit(fit))
                    .rounded(10.)
                    .clip(Clip::Rounded),
                muted(name).font_size(12.),
            ]
            .gap(6., Axis::Vertical)
            .into()
        })
        .collect(),
    );
    let clips = line(vec![
        vstack![]
            .width(96.)
            .height(96.)
            .background_image(app.image.clone().background())
            .clip(Clip::Circle)
            .into(),
        vstack![]
            .width(140.)
            .height(96.)
            .background_image(app.image.clone().background())
            .rounded(18.)
            .clip(Clip::Rounded)
            .into(),
        vstack![]
            .width(110.)
            .height(96.)
            .background_image(app.image.clone().background())
            .clip(Clip::Path(
                VectorPath::new()
                    .move_to(55., 0.)
                    .line_to(110., 48.)
                    .line_to(55., 96.)
                    .line_to(0., 48.)
                    .close(),
            ))
            .into(),
    ]);
    Doc {
        sections: vec![
            section(
                "image-fit",
                "Fit",
                "Cover, contain or tile any decoded image.",
                fits,
                r#"vstack![].background_image(image.clone().background().fit(ImageFit::Cover))"#,
            ),
            section(
                "image-clip",
                "Clipping",
                "Circles, rounded rectangles and arbitrary paths.",
                clips,
                r#".clip(Clip::Circle)
.clip(Clip::Path(diamond))"#,
            ),
        ],
        api: vec![
            (
                "native::load_image(path) / decode_image(bytes)",
                "Bounded PNG/JPEG decoding.",
            ),
            (
                "Image::from_rgba(w, h, pixels)",
                "Images from generated pixels.",
            ),
            (
                ".background_image(image.background().fit(f).position(p).opacity(o))",
                "Fit, position and opacity.",
            ),
            (
                ".clip(Clip::Rounded | Circle | Path(p))",
                "Clip content and hit testing.",
            ),
        ],
    }
}

fn effects_doc(app: &App) -> Doc {
    let elevation = line(
        [(0., "Flat"), (6., "Raised"), (18., "Floating")]
            .into_iter()
            .map(|(blur, name)| -> El<App> {
                panel![label(name)]
                    .width(130.)
                    .height(80.)
                    .flow(Flow::Center)
                    .background(ColorToken::Base100)
                    .border_color(ColorToken::Border)
                    .border_width(1.)
                    .rounded(12.)
                    .shadow(
                        Shadow::new()
                            .color("#00000022")
                            .blur(blur)
                            .offset(0., blur / 3.),
                    )
                    .into()
            })
            .collect(),
    );
    let interactive = line(vec![
        button("Hover for glow")
            .key("fx-glow")
            .variant(ButtonVariant::Outline)
            .hover_style(|s| {
                s.glow(Glow::new().color(ColorToken::Primary.alpha(60)).blur(14.))
                    .border_color(ColorToken::Primary)
            })
            .animate_on(Interaction::Hover, Animation::ease_out().duration_ms(180))
            .on_click(noop)
            .into(),
        button("Press me")
            .key("fx-press")
            .variant(ButtonVariant::Primary)
            .pressed_style(|s| s.opacity(0.7))
            .animate_on(Interaction::Pressed, Animation::ease_out().duration_ms(90))
            .on_click(noop)
            .into(),
    ]);
    let reveal = vstack![
        button(if app.reveal {
            "Hide message"
        } else {
            "Show message"
        })
        .key("fx-toggle")
        .variant(ButtonVariant::Outline)
        .on_click(|app: &mut App| app.reveal = !app.reveal),
        if app.reveal {
            El::from(
                hstack![
                    icon(Icon::Spark, 16., ColorToken::Primary),
                    text("Fades in and out with a keyed transition.")
                ]
                .key("fx-message")
                .gap(8., Axis::Horizontal)
                .align(Align::Center)
                .padding(12., Edge::All)
                .background(ColorToken::Primary.alpha(10))
                .rounded(10.)
                .transition(Transition::fade().duration_ms(260)),
            )
        } else {
            spacer().height(44.).flex(0.).into()
        },
    ]
    .gap(12., Axis::Vertical);
    Doc {
        sections: vec![
            section(
                "fx-shadow",
                "Elevation",
                "Analytic shadows on the GPU, blurred masks in software.",
                elevation,
                r##".shadow(Shadow::new().color("#00000022").blur(18.).offset(0., 6.))"##,
            ),
            section(
                "fx-states",
                "Interaction states",
                "Hover, press and focus styles with their own easing.",
                interactive,
                r#"button("Hover for glow")
    .hover_style(|s| s.glow(Glow::new().color(ColorToken::Primary.alpha(60)).blur(14.)))
    .animate_on(Interaction::Hover, Animation::ease_out().duration_ms(180))"#,
            ),
            section(
                "fx-transition",
                "Transitions",
                "Keyed elements fade when they appear and leave; reduced motion settles instantly.",
                reveal,
                r#"if app.reveal {
    message.key("message").transition(Transition::fade().duration_ms(260))
}"#,
            ),
        ],
        api: vec![
            (
                ".shadow(Shadow) / .glow(Glow) / .opacity(o)",
                "Paint-only effects.",
            ),
            (
                ".hover_style / .pressed_style / .focus_style",
                "Style patches per interaction.",
            ),
            (
                ".animate_on(Interaction, Animation)",
                "Easing for state changes.",
            ),
            (
                ".animation(Animation)",
                "Animate appearance changes for the same key.",
            ),
            (".transition(Transition::fade())", "Fade on enter and exit."),
        ],
    }
}

// ------------------------------------------------------------ Patterns

fn tasks_doc(app: &App) -> Doc {
    let panel = vstack![
        hstack![
            label("Indexing photos"),
            spacer(),
            muted(if app.job_running {
                format!("{}%", app.job_progress)
            } else if app.job_progress >= 100 {
                "Done".into()
            } else {
                "Idle".into()
            }),
        ],
        progress(app.job_progress as f32 / 100.).fill_width(),
        hstack![
            button(if app.job_running {
                "Restart"
            } else {
                "Start task"
            })
            .key("job-start")
            .variant(ButtonVariant::Primary)
            .on_click(App::start_job),
            button("Cancel")
                .key("job-cancel")
                .variant(ButtonVariant::Outline)
                .enabled(app.job_running)
                .on_click(App::cancel_job),
            spacer(),
            if app.job_running {
                El::from(spinner())
            } else {
                spacer().width(0.).into()
            },
        ]
        .gap(8., Axis::Horizontal)
        .align(Align::Center),
        muted("The window stays responsive: try other pages while it runs."),
    ]
    .gap(12., Axis::Vertical)
    .width(420.);
    Doc {
        sections: vec![section("tasks", "Worker progress", "A thread sends progress; the UI applies it on its own thread and ignores stale results.", panel, r#"// Worker thread
sender.send(Message::Progress { generation, step })?;

// UI thread, batched per event-loop turn
fn update(app: &mut App, message: Message) {
    let Message::Progress { generation, step } = message;
    if generation == app.generation { app.progress = step; }
}

native::run_ui_with_messages_and_theme(options, app, view, theme,
    |app, sender| app.sender = Some(sender), update)?;"#)],
        api: vec![
            ("run_ui_with_messages(options, state, view, init, update)", "Start with a typed message channel."),
            ("UiSender::send(message)", "Non-blocking; Full or Closed errors are explicit."),
            ("Batching", "Up to 64 messages per turn, then one recomposition."),
        ],
    }
}

fn sign_in(app: &App) -> Doc {
    let ready = app.email.contains('@') && app.password.chars().count() >= 8;
    let form = vstack![
        vstack![
            text("Sign in").font_size(22.).font_weight(700),
            muted("Enter your email below to sign in to your account.")
        ]
        .gap(4., Axis::Vertical),
        // Errors appear once something has been typed, never on an empty form.
        form_field(
            "Email",
            text_field(&app.email)
                .key("si-email")
                .placeholder("m@example.com")
                .leading_icon(volt_ui_kit::ui::Icon::Mail)
                .on_change(|app: &mut App, v| app.email = v),
        )
        .key("si-email-field")
        .required(true)
        .error(
            (!app.email.is_empty() && !app.email.contains('@'))
                .then(|| "Enter a full email address".to_owned())
        ),
        vstack![
            form_field(
                "Password",
                text_field(&app.password)
                    .key("si-pass")
                    .password(!app.show_password)
                    .placeholder("8 characters or more")
                    .leading_icon(volt_ui_kit::ui::Icon::Lock)
                    .trailing_action(
                        "si-pass-reveal",
                        if app.show_password {
                            volt_ui_kit::ui::Icon::EyeOff
                        } else {
                            volt_ui_kit::ui::Icon::Eye
                        },
                        if app.show_password {
                            "Hide password"
                        } else {
                            "Show password"
                        },
                        |app: &mut App| app.show_password = !app.show_password,
                    )
                    .on_change(|app: &mut App, v| app.password = v)
                    .on_submit(|app: &mut App| app.notify("Signed in")),
            )
            .key("si-pass-field")
            .required(true)
            .error(
                (!app.password.is_empty() && app.password.chars().count() < 8)
                    .then(|| "Use at least 8 characters".to_owned())
            ),
            hstack![
                spacer(),
                button("Forgot password?")
                    .key("si-forgot")
                    .variant(ButtonVariant::Ghost)
                    .size(ControlSize::Small)
                    .on_click(|app: &mut App| app.notify("Reset link sent")),
            ],
        ]
        .gap(2., Axis::Vertical),
        checkbox("Remember me", app.checked)
            .key("si-remember")
            .on_change(|app: &mut App, v| app.checked = v),
        button("Sign in")
            .key("si-submit")
            .variant(ButtonVariant::Primary)
            .fill_width()
            .enabled(ready)
            .on_click(|app: &mut App| app.notify("Signed in")),
        button("Continue with passkey")
            .key("si-passkey")
            .variant(ButtonVariant::Outline)
            .fill_width()
            .on_click(|app: &mut App| app.notify("Passkey prompt shown")),
    ]
    .width(360.)
    .padding(28., Edge::All)
    .gap(16., Axis::Vertical)
    .background(ColorToken::Base100)
    .border_color(ColorToken::Border)
    .border_width(1.)
    .rounded(14.);
    Doc {
        sections: vec![section(
            "sign-in",
            "Sign-in card",
            "The submit button enables once the form is valid.",
            form,
            r#"form_field("Password", text_field(&app.password)
        .password(!app.show_password)
        .leading_icon(Icon::Lock)
        .trailing_action("reveal", Icon::Eye, "Show password", toggle_reveal))
    .required(true)
    .error(too_short.then(|| "Use at least 8 characters".into()))
button("Sign in").variant(ButtonVariant::Primary).fill_width().enabled(ready)"#,
        )],
        api: vec![
            (
                "form_field(label, control)",
                "Label, required mark, description or error.",
            ),
            (
                ".leading_icon / .trailing_action",
                "Icons and actions inside the field.",
            ),
        ],
    }
}

fn settings_doc(app: &App) -> Doc {
    let row = |title: &'static str, detail: &'static str, control: El<App>| -> El<App> {
        hstack![
            vstack![label(title), muted(detail).font_size(12.)]
                .gap(2., Axis::Vertical)
                .flex(1.),
            control
        ]
        .gap(16., Axis::Horizontal)
        .align(Align::Center)
        .into()
    };
    let zones = [
        "Pacific", "Mountain", "Central", "Eastern", "London", "Tokyo",
    ];
    let card = vstack![
        hstack![
            icon(Icon::Bell, 18., ColorToken::BaseContent),
            text("Notifications").font_size(18.).font_weight(600)
        ]
        .gap(8., Axis::Horizontal)
        .align(Align::Center),
        row(
            "Push notifications",
            "Send alerts to this device.",
            toggle("", app.switch_on)
                .key("set-push")
                .on_change(|app: &mut App, v| app.switch_on = v)
                .into()
        ),
        divider().background(ColorToken::Border),
        row(
            "Do not disturb",
            "Silence everything until tomorrow.",
            toggle("", app.airplane)
                .key("set-dnd")
                .on_change(|app: &mut App, v| app.airplane = v)
                .into()
        ),
        divider().background(ColorToken::Border),
        row(
            "Time zone",
            "Used for reminders.",
            select(
                zones
                    .iter()
                    .enumerate()
                    .map(|(i, z)| select_item(i as u8, *z))
            )
            .key("set-tz")
            .selected(app.timezone.min(5))
            .open(app.timezone_open)
            .width(150.)
            .on_select(|app: &mut App, v| app.timezone = v)
            .on_open_change(|app: &mut App, o| app.timezone_open = o)
            .into()
        ),
        divider().background(ColorToken::Border),
        row(
            "Alert volume",
            "",
            slider(app.volume)
                .key("set-vol")
                .width(150.)
                .on_change(|app: &mut App, v| app.volume = v)
                .into()
        ),
        hstack![
            spacer(),
            button("Save preferences")
                .key("set-save")
                .variant(ButtonVariant::Primary)
                .on_click(|app: &mut App| app.notify("Preferences saved"))
        ],
    ]
    .width(520.)
    .padding(24., Edge::All)
    .gap(14., Axis::Vertical)
    .background(ColorToken::Base100)
    .border_color(ColorToken::Border)
    .border_width(1.)
    .rounded(14.);
    Doc {
        sections: vec![section(
            "settings",
            "Settings card",
            "Rows with a title, a description and a control.",
            card,
            r#"hstack![
    vstack![label(title), muted(detail)].flex(1.),
    toggle("", app.push).on_change(|app: &mut App, v| app.push = v),
]"#,
        )],
        api: vec![],
    }
}

fn navigation_doc(app: &App) -> Doc {
    let make = |key: &str, placement| {
        navigation([
            navigation_item(0u8, "Overview").key("overview"),
            navigation_item(1, "Messages").key("messages"),
            navigation_item(2, "Settings").key("settings"),
            navigation_item(3, "Unavailable")
                .key("unavailable")
                .enabled(false),
        ])
        .key(key)
        .selected(app.navigation)
        .placement(placement)
        .on_select(|app: &mut App, page| app.navigation = page)
    };
    Doc {
        sections: vec![Section {
            id:"navigation-layouts", title:"Navigation, your layout", note:"All three previews share controlled app state. These are independent components, not a mandatory application shell.",
            demo: vstack![
                hstack![make("nav-sidebar",NavigationPlacement::Sidebar).width(190.),
                    panel![vstack![label("YOUR WORKSPACE"), text(format!("Destination {}",app.navigation)).font_size(24.), muted("Compose your editor, settings or mail interface here.")].gap(12.,Axis::Vertical)]
                        .padding(24.,Edge::All).width(340.).height(220.).background(ColorToken::Base200).rounded(12.)
                ].gap(24.,Axis::Horizontal),
                make("nav-rail",NavigationPlacement::Rail).width(140.),
                make("nav-bottom",NavigationPlacement::Bottom),
            ].gap(24.,Axis::Vertical).into(),
            code: r#"navigation([
    navigation_item(Page::Inbox, "Inbox").key("inbox"),
    navigation_item(Page::Sent, "Sent").key("sent"),
]).key("navigation").selected(app.page)
  .placement(NavigationPlacement::Sidebar)
  .on_select(|app: &mut App, page| app.page = page)
// Use Rail or Bottom; compose independently with ordinary stacks/panels."#,
        }],
        api: vec![("navigation(items)","Controlled, typed destinations"),(".placement(Sidebar | Rail | Bottom)","Vertical or horizontal destination layout"),(".selected(value).on_select(callback)","Application owns routing and state"),(".key(...)","Stable group and item identities are required")],
    }
}
