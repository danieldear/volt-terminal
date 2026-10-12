use super::{style::*, App};
use volt_ui_kit::ui::prelude::*;

pub fn view(app: &App) -> Element<App> {
    let records = virtual_list(10_000, 36., |index| {
        data_item(index, format!("Study {index:05}   /   local collection"))
            .key(format!("explorer-record-{index}"))
    })
    .key("explorer-records")
    .fill_width()
    .window(app.list_window.clone())
    .height(300.)
    .overscan(2)
    .selected(app.list_selected)
    .font_size(13.)
    .on_window_change(|app: &mut App, range| app.list_window = range)
    .on_select(|app: &mut App, id| {
        app.list_selected = Some(id);
        app.record(format!("Atelier: selected study {id:05}"));
    });
    let collection = glass_card(
        app,
        [
            hstack![
                title("The collection"),
                spacer(),
                pill("10,000", ColorToken::Base300, ColorToken::BaseContent)
            ]
            .into(),
            muted("Scroll freely. Only the visible window and overscan are composed.").into(),
            records.into(),
            label(format!(
                "VISIBLE {}..{}",
                app.list_window.start, app.list_window.end
            ))
            .key("explorer-range")
            .into(),
        ],
    )
    .key("explorer-records-card")
    .flex(1.);
    let details = glass_card(app, [
        label("SELECTION / STABLE KEY").into(),
        text(app.list_selected.map(|i| format!("Study {i:05}")).unwrap_or_else(|| "Find a little inspiration.".into()))
            .key("explorer-selection").font_size(28.).font_weight(600).into(),
        muted("Choose any row to inspect it. The selected record remains in application state when its row scrolls offscreen.").into(),
        canvas(|c| {
            c.stroke(VectorPath::ellipse(120., 90., 98., 58.), ColorToken::Primary, 1.5);
            c.fill(VectorPath::new().move_to(22., 100.).cubic_to([55., 10.], [135., 10.], [206., 92.])
                .line_to(159., 115.).cubic_to([110., 52.], [72., 51.], [71., 130.]).close(), ColorToken::Primary);
            c.fill(VectorPath::circle(189., 42., 8.), ColorToken::Secondary);
            c.stroke(VectorPath::new().move_to(30., 155.).line_to(215., 155.), ColorToken::Border, 1.);
        }).height(190.).width(240.),
        pill("LOCAL COLLECTION", ColorToken::Primary, ColorToken::PrimaryContent).into(),
    ]).key("explorer-detail").flex(0.65);
    let names = [
        "Ribbon studies",
        "Quiet interface",
        "Paper collection",
        "Motion notes",
        "Color field",
        "Native workspace",
    ];
    let rows = (0..6).map(|id| {
        table_row(
            id,
            [
                names[id].to_owned(),
                if id % 2 == 0 {
                    "Ready".into()
                } else {
                    "Review".into()
                },
                format!("{:02} ms", 8 + id * 3),
            ],
        )
        .key(format!("explorer-project-{id}"))
    });
    let projects = table(
        [
            table_column(0usize, "Project"),
            table_column(1, "Status"),
            table_column(2, "Time"),
        ],
        rows,
    )
    .key("explorer-table")
    .sort(app.table_column, app.table_direction)
    .selected(app.table_selected)
    .font_family("SF")
    .font_size(12.)
    .on_sort(|app: &mut App, column, direction| {
        app.table_column = column;
        app.table_direction = direction;
    })
    .on_select(move |app: &mut App, id| {
        app.table_selected = Some(id);
        app.record(format!("Atelier: opened {}", names[id]));
    });
    let files = tree_view([
        tree_node(1usize, "Atelier")
            .key("explorer-tree-root")
            .children([
                tree_node(2, "Sources")
                    .key("explorer-tree-sources")
                    .children([
                        tree_node(3, "main.rs").key("explorer-tree-main"),
                        tree_node(4, "components.rs").key("explorer-tree-components"),
                    ]),
                tree_node(5, "Illustrations")
                    .key("explorer-tree-art")
                    .children([tree_node(6, "ribbon.svg").key("explorer-tree-ribbon")]),
            ]),
        tree_node(7, "Tests")
            .key("explorer-tree-tests")
            .children([tree_node(8, "interaction.rs").key("explorer-tree-interaction")]),
    ])
    .key("explorer-tree")
    .expanded(app.tree_expanded.clone())
    .selected(app.tree_selected)
    .font_family("SF")
    .font_size(12.)
    .on_expand(|app: &mut App, id, open| {
        app.tree_expanded.retain(|&v| v != id);
        if open {
            app.tree_expanded.push(id);
        }
    })
    .on_select(|app: &mut App, id| {
        app.tree_selected = Some(id);
        app.record(format!("Atelier: selected source {id}"));
    });
    let browser = glass_card(app, [
        hstack![title("A place for every detail"), spacer(), label(format!("SPLIT {:.0}%", app.split_ratio * 100.))].into(),
        muted("Sort a column, expand the source tree, or resize the divider. Arrow keys also resize a focused divider.").into(),
        split(vstack![title("Projects"), projects].key("explorer-project-pane").gap(12., Axis::Vertical).padding(10., Edge::All).scroll(Scroll::Auto),
            vstack![title("Sources"), files].key("explorer-source-pane").gap(12., Axis::Vertical).padding(10., Edge::All).scroll(Scroll::Auto))
            .key("explorer-split").fill_width().height(350.).ratio(app.split_ratio).min_first(140.).min_second(140.)
            .on_resize(|app: &mut App, ratio| app.split_ratio = ratio).into(),
    ]).key("explorer-browser");
    vstack![
        text("Room for the details.")
            .font_size(34.)
            .font_weight(700),
        muted("Collections and source navigation, without mounting every record."),
        adaptive([collection.into(), details.into()]),
        browser
    ]
    .gap(20., Axis::Vertical)
    .into()
}
