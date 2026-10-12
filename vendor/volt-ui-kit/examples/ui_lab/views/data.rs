use super::*;

pub fn view(app: &App) -> Element<App> {
    let list = virtual_list(10_000, 32., |index| {
        data_item(index, format!("Record {index:05}  ·  native Rust row"))
            .key(format!("data-record-{index}"))
    })
    .key("data-records")
    .window(app.list_window.clone())
    .selected(app.list_selected)
    .height(300.)
    .overscan(2)
    .on_window_change(|app: &mut App, window| app.list_window = window)
    .on_select(|app: &mut App, id| {
        app.list_selected = Some(id);
        app.record(format!(
            "Selected record {id}; only visible rows are mounted"
        ));
    });
    let rows = (0..6usize).map(|id| {
        table_row(
            id,
            [
                format!("Project {}", ['F', 'A', 'D', 'B', 'E', 'C'][id]),
                if id % 2 == 0 {
                    "Ready".into()
                } else {
                    "Review".into()
                },
                format!("{} ms", 8 + id * 3),
            ],
        )
        .key(format!("data-table-row-{id}"))
    });
    let projects = table(
        [
            table_column(0usize, "Project"),
            table_column(1, "Status"),
            table_column(2, "Latency"),
        ],
        rows,
    )
    .key("data-table")
    .sort(app.table_column, app.table_direction)
    .selected(app.table_selected)
    .on_sort(|app: &mut App, column, direction| {
        app.table_column = column;
        app.table_direction = direction;
    })
    .on_select(|app: &mut App, id| {
        app.table_selected = Some(id);
        app.record(format!("Selected table row {id}"));
    });
    let tree = tree_view([
        tree_node(1usize, "Workspace")
            .key("data-tree-workspace")
            .children([
                tree_node(2, "Sources").key("data-tree-sources").children([
                    tree_node(3, "main.rs").key("data-tree-main"),
                    tree_node(4, "components.rs").key("data-tree-components"),
                ]),
                tree_node(5, "Assets")
                    .key("data-tree-assets")
                    .children([tree_node(6, "cover.png").key("data-tree-cover")]),
            ]),
        tree_node(7, "Tests")
            .key("data-tree-tests")
            .children([tree_node(8, "interaction.rs").key("data-tree-interaction")]),
    ])
    .key("data-tree")
    .expanded(app.tree_expanded.clone())
    .selected(app.tree_selected)
    .on_expand(|app: &mut App, id, open| {
        app.tree_expanded.retain(|value| *value != id);
        if open {
            app.tree_expanded.push(id);
        }
    })
    .on_select(|app: &mut App, id| {
        app.tree_selected = Some(id);
        app.record(format!("Selected tree node {id}"));
    });
    vstack![
        specimen("10,000 rows, bounded composition", [description("Fixed-height virtualization: only the controlled visible window + overscan is composed. Drag the scrollbar or wheel; stable record keys retain selection."),
            text(format!("Mounted range: {}..{}  /  10000  ·  selected {:?}",app.list_window.start,app.list_window.end,app.list_selected))
                .key("data-window-label").font_family("monospace").font_size(12.).into(),list.into()]),
        specimen("Sortable table & expandable tree", [description("Drag the divider. Tab to it and use arrows, Home/End or Page keys. Sort headers and expand branches use typed Rust callbacks."),
            split(vstack![heading("Projects"),projects].gap(12.,Axis::Vertical).padding(10.,Edge::All).scroll(Scroll::Auto),
                  vstack![heading("Workspace tree"),tree].gap(12.,Axis::Vertical).padding(10.,Edge::All).scroll(Scroll::Auto))
                .key("data-split").ratio(app.split_ratio).min_first(140.).min_second(140.).height(370.)
                .on_resize(|app:&mut App,ratio|app.split_ratio=ratio).into(),
            text(format!("Split: {:.0}% / {:.0}%",app.split_ratio*100.,(1.-app.split_ratio)*100.)).font_family("monospace").font_size(12.).into()]),
    ].gap(18.,Axis::Vertical).into()
}
