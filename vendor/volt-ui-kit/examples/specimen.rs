//! Every component at its default styling, for design review and visual
//! regression. Nothing here is styled by the application.
//!
//! cargo run --features native --example specimen -- --snapshot target/specimen.png --size 1400x1000
#[path = "support/rust_ui.rs"]
mod support;
use volt_ui_kit::ui::prelude::*;
use volt_ui_kit::ui::{
    menu, menu_item, menu_separator, progress, progress_indeterminate, spinner, split, table,
    table_column, table_row, tooltip, tree_node, tree_view,
};

#[derive(Default)]
struct App {
    on: bool,
    tab: u8,
    seg: u8,
    value: f32,
    text: String,
    select_open: bool,
    menu_open: bool,
    dialog: bool,
}

fn section(title: &str, body: Vec<Element<App>>) -> Element<App> {
    vstack![
        text(title).font_size(12.).color(ColorToken::Muted),
        vstack(body).gap(10., Axis::Vertical),
    ]
    .gap(8., Axis::Vertical)
    .into()
}

fn view(app: &App) -> Element<App> {
    let buttons = section(
        "Buttons",
        vec![
            hstack![
                button("Default").key("b1").on_click(|_: &mut App| {}),
                button("Primary")
                    .key("b2")
                    .primary(true)
                    .on_click(|_: &mut App| {}),
                button("Disabled")
                    .key("b3")
                    .enabled(false)
                    .on_click(|_: &mut App| {}),
            ]
            .gap(8., Axis::Horizontal)
            .into(),
            hstack![
                button("Soft")
                    .key("v1")
                    .variant(ButtonVariant::Soft)
                    .on_click(|_: &mut App| {}),
                button("Outline")
                    .key("v2")
                    .variant(ButtonVariant::Outline)
                    .on_click(|_: &mut App| {}),
                button("Ghost")
                    .key("v3")
                    .variant(ButtonVariant::Ghost)
                    .on_click(|_: &mut App| {}),
                button("Delete")
                    .key("v4")
                    .variant(ButtonVariant::Danger)
                    .on_click(|_: &mut App| {}),
            ]
            .gap(8., Axis::Horizontal)
            .into(),
            hstack![
                button("Small")
                    .key("s1")
                    .size(ControlSize::Small)
                    .on_click(|_: &mut App| {}),
                button("Medium")
                    .key("s2")
                    .size(ControlSize::Medium)
                    .on_click(|_: &mut App| {}),
                button("Large")
                    .key("s3")
                    .size(ControlSize::Large)
                    .primary(true)
                    .on_click(|_: &mut App| {}),
                button("Options")
                    .key("s4")
                    .disclosure(false)
                    .width(120.)
                    .on_click(|_: &mut App| {}),
            ]
            .gap(8., Axis::Horizontal)
            .align(Align::Center)
            .into(),
            button_bar![
                button("Left").key("bb1").on_click(|_: &mut App| {}),
                button("Middle").key("bb2").on_click(|_: &mut App| {}),
                button("Right").key("bb3").on_click(|_: &mut App| {}),
            ]
            .joined(true)
            .into(),
            toolbar![
                button("Back").key("t1").on_click(|_: &mut App| {}),
                button("Share").key("t2").on_click(|_: &mut App| {}),
                spacer(),
                badge("Badge"),
            ]
            .into(),
        ],
    );
    let selection = section(
        "Selection",
        vec![
            segmented_control([(0u8, "Day"), (1, "Week"), (2, "Month")])
                .key("seg")
                .selected(app.seg)
                .on_select(|app: &mut App, v| app.seg = v)
                .into(),
            tab_bar([
                tab_item(0u8, "Overview"),
                tab_item(1, "Activity"),
                tab_item(2, "Settings"),
            ])
            .key("tabs")
            .selected(app.tab)
            .on_select(|app: &mut App, v| app.tab = v)
            .into(),
            hstack![
                checkbox("Checkbox", app.on)
                    .key("c1")
                    .on_change(|a: &mut App, v| a.on = v),
                checkbox("Unchecked", false)
                    .key("c2")
                    .on_change(|_: &mut App, _| {}),
                toggle("Toggle", app.on)
                    .key("tg")
                    .on_change(|a: &mut App, v| a.on = v),
            ]
            .gap(16., Axis::Horizontal)
            .into(),
        ],
    );
    let inputs = section(
        "Inputs",
        vec![
            text_field(&app.text)
                .key("tf")
                .placeholder("Text field")
                .on_change(|a: &mut App, v| a.text = v)
                .into(),
            text_area("Multi-line\ntext area")
                .key("ta")
                .height(72.)
                .on_change(|_: &mut App, _| {})
                .into(),
            slider(app.value)
                .key("sl")
                .on_change(|a: &mut App, v| a.value = v)
                .into(),
            select([select_item(0u8, "Select option"), select_item(1, "Second")])
                .key("sel")
                .selected(0)
                .open(app.select_open)
                .on_select(|_: &mut App, _| {})
                .on_open_change(|a: &mut App, o| a.select_open = o)
                .into(),
        ],
    );
    let feedback = section(
        "Feedback",
        vec![
            progress(0.6).into(),
            progress_indeterminate().into(),
            hstack![spinner(), text("Text"), badge("Badge")]
                .gap(12., Axis::Horizontal)
                .align(Align::Center)
                .into(),
            tooltip(
                "Tooltip",
                button("Hover me").key("tt").on_click(|_: &mut App| {}),
            )
            .key("tip")
            .into(),
            menu(
                "Menu",
                [
                    menu_item(0u8, "Open"),
                    menu_item(1, "Rename"),
                    menu_separator(),
                    menu_item(2, "Delete").danger(true),
                ],
            )
            .key("menu")
            .open(app.menu_open)
            .on_action(|_: &mut App, _| {})
            .on_open_change(|a: &mut App, o| a.menu_open = o)
            .into(),
        ],
    );
    let data = section(
        "Data",
        vec![
            table(
                [table_column(0u8, "Name"), table_column(1, "Status")],
                [
                    table_row(0u8, ["Lofoten", "Booked"]).key("r0"),
                    table_row(1, ["Senja", "Planned"]).key("r1"),
                ],
            )
            .key("table")
            .on_select(|_: &mut App, _| {})
            .on_sort(|_, _, _| {})
            .into(),
            tree_view([tree_node(0u8, "Trip").key("n0").children([
                tree_node(1, "Day 1").key("n1"),
                tree_node(2, "Day 2").key("n2"),
            ])])
            .key("tree")
            .expanded([0])
            .on_select(|_: &mut App, _| {})
            .on_expand(|_: &mut App, _, _| {})
            .into(),
            split(panel![text("First pane")], panel![text("Second pane")])
                .key("split")
                .height(80.)
                .on_resize(|_: &mut App, _| {})
                .into(),
        ],
    );
    let containers = section(
        "Containers",
        vec![
            panel![text("Panel"), divider(), text("Content")].into(),
            button("Open dialog")
                .key("dlg")
                .on_click(|a: &mut App| a.dialog = true)
                .into(),
        ],
    );
    zstack![
        hstack![
            vstack![buttons, selection, inputs]
                .gap(24., Axis::Vertical)
                .flex(1.),
            vstack![feedback, data].gap(24., Axis::Vertical).flex(1.),
            vstack![containers].flex(1.),
        ]
        .padding(28., Edge::All)
        .gap(28., Axis::Horizontal),
        dialog(vstack![
            text("Dialog body text."),
            button("Close")
                .key("close")
                .on_click(|a: &mut App| a.dialog = false),
        ])
        .key("modal")
        .title("Dialog")
        .open(app.dialog)
        .on_dismiss(|a: &mut App| a.dialog = false),
    ]
    .align(Align::Stretch)
    .background(ColorToken::Base100)
    .color(ColorToken::BaseContent)
    .into()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App {
        value: 0.4,
        on: true,
        ..Default::default()
    };
    if std::env::args().any(|a| a == "--open-overlays") {
        app.select_open = true;
    }
    if std::env::args().any(|a| a == "--dialog") {
        app.dialog = true;
    }
    let args: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| a != "--open-overlays" && a != "--dialog")
        .collect();
    support::launch_themed_with_args(
        "Component specimen",
        app,
        view,
        |_, mode| {
            if mode == volt_ui_kit::ThemeMode::Light {
                volt_ui_kit::UiTheme::light()
            } else {
                volt_ui_kit::UiTheme::dark()
            }
        },
        support::RustLaunchOptions {
            chrome: volt_ui_kit::native::WindowChrome::Native,
            background: volt_ui_kit::native::WindowBackground::Opaque,
            args,
        },
    )
}
