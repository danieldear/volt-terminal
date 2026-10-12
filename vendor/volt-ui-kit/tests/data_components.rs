#![cfg(feature = "rust-ui")]
use std::{cell::Cell, rc::Rc};
use volt_ui_kit::ui::prelude::*;
#[derive(Default)]
struct App {
    selected: Option<usize>,
    window: std::ops::Range<usize>,
    expanded: Vec<usize>,
    sort: Option<(usize, SortDirection)>,
}
#[test]
fn virtual_list_builds_only_a_bounded_window_of_ten_thousand_records() {
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    let ui: Element<App> = virtual_list(10_000, 32., move |index| {
        count.set(count.get() + 1);
        data_item(index, format!("Record {index}")).key(format!("record-{index}"))
    })
    .key("records")
    .window(500..516)
    .selected(Some(505))
    .on_select(|app: &mut App, id| app.selected = Some(id))
    .on_window_change(|app: &mut App, window| app.window = window)
    .into();
    let (document, _) = ui.build().unwrap().into_parts();
    assert_eq!(calls.get(), 16);
    assert_eq!(document.root().children().len(), 16);
}
#[test]
fn typed_table_sorting_reorders_rows_without_changing_stable_keys() {
    let ui: Element<App> = table(
        [table_column(0, "Name"), table_column(1, "Age")],
        [
            table_row(1, ["Beta", "20"]).key("beta"),
            table_row(2, ["Alpha", "40"]).key("alpha"),
        ],
    )
    .key("table")
    .sort(0, SortDirection::Ascending)
    .selected(Some(1))
    .on_select(|app: &mut App, id| app.selected = Some(id))
    .on_sort(|app: &mut App, col, direction| app.sort = Some((col, direction)))
    .into();
    ui.build().unwrap();
}
#[test]
fn tree_builds_only_expanded_branches() {
    let ui: Element<App> = tree_view([tree_node(0, "Root")
        .key("root")
        .children([tree_node(1, "Child").key("child")])])
    .key("tree")
    .expanded([])
    .selected(None)
    .on_select(|app: &mut App, id| app.selected = Some(id))
    .on_expand(|app: &mut App, id, open| {
        if open {
            app.expanded.push(id)
        }
    })
    .into();
    let (document, _) = ui.build().unwrap().into_parts();
    fn has(n: &volt_ui_kit::declarative::Node, id: &str) -> bool {
        n.id() == id || n.children().iter().any(|n| has(n, id))
    }
    assert!(!has(document.root(), "child"));
}

use volt_ui_kit::{
    declarative::{Bindings, DocumentPainter, Handlers, Runtime, TextMetrics},
    Painter, Viewport,
};
struct Metrics(f32);
impl TextMetrics for Metrics {
    fn measure(&mut self, s: &str, size: f32, width: f32) -> [f32; 2] {
        [(s.len() as f32 * size * 0.5).min(width), size * 1.2]
    }
}
impl Painter for Metrics {
    type Shape = ();
    type Glyph = ();
    fn viewport(&self) -> Viewport {
        Viewport::new(600. * self.0, 600. * self.0, self.0).unwrap()
    }
    fn draw_rect(&self, _: &mut Vec<()>, _: f32, _: f32, _: f32, _: f32, _: [f32; 4]) {}
    fn card_triangle(&self, _: &mut Vec<()>, _: [[f32; 2]; 3], _: [f32; 4]) {}
    fn card_text(&mut self, _: &mut Vec<()>, _: &str, _: f32, _: f32, _: f32, _: [f32; 4]) {}
    fn draw_text_colored(
        &mut self,
        _: &mut Vec<()>,
        _: &str,
        _: f32,
        _: f32,
        _: f32,
        _: f32,
        _: &dyn Fn(usize) -> [f32; 4],
    ) {
    }
}
impl DocumentPainter for Metrics {
    fn ui_text(&mut self, _: &mut Vec<()>, _: &str, _: [f32; 4], _: f32, _: [f32; 4]) {}
}
fn mount(ui: Element<App>, scale: f32) -> (Runtime, Handlers<'static, App>) {
    let (d, h) = ui.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    let mut p = Metrics(scale);
    r.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    (r, h)
}
fn replace(r: &mut Runtime, h: &mut Handlers<'static, App>, ui: Element<App>, scale: f32) {
    let (d, next) = ui.build().unwrap().into_parts();
    r.replace_document(d, &next, &Bindings::default()).unwrap();
    *h = next;
    let mut p = Metrics(scale);
    r.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
}
fn click(r: &mut Runtime, h: &mut Handlers<'static, App>, app: &mut App, id: &str) -> bool {
    let [x, y, w, height] = r
        .boxes()
        .iter()
        .find(|b| b.id == id)
        .unwrap()
        .bounds
        .bounds();
    r.pointer_down_outcome(x + w / 2., y + height / 2., h, app)
        .unwrap();
    r.pointer_up_outcome(x + w / 2., y + height / 2., h, app)
        .unwrap()
        .activated
}
fn list(app: &App) -> Element<App> {
    virtual_list(10_000, 32., |id| {
        data_item(id, format!("Record {id}"))
            .key(format!("r-{id}"))
            .enabled(id != 2)
    })
    .key("records")
    .window(app.window.clone())
    .selected(app.selected)
    .on_select(|app: &mut App, id| app.selected = Some(id))
    .on_window_change(|app: &mut App, window| app.window = window)
    .into()
}
#[test]
fn virtual_wheel_updates_the_mounted_window_not_the_entire_model() {
    for scale in [1., 2.] {
        let mut app = App {
            window: 0..15,
            ..App::default()
        };
        let (mut r, mut h) = mount(list(&app), scale);
        assert_eq!(r.document().root().children().len(), 15);
        assert!(
            r.boxes()
                .iter()
                .find(|b| b.id == "records")
                .unwrap()
                .scroll_max[1]
                > 300_000.
        );
        assert!(
            r.scroll_outcome(100. * scale, 100. * scale, [0., 1600.], &mut h, &mut app)
                .unwrap()
                .activated
        );
        assert!(app.window.start >= 48);
        assert!(app.window.len() <= 96);
        replace(&mut r, &mut h, list(&app), scale);
        assert!(r.document().root().children().len() <= 96);
        assert!(r.boxes().iter().any(|b| b.id == "r-50"));
        assert!(click(&mut r, &mut h, &mut app, "r-50"));
        assert_eq!(app.selected, Some(50));
        assert!(!r.boxes().iter().any(|b| b.id == "r-0"));
    }
}
#[test]
fn virtual_list_accepts_a_factory_borrowing_view_local_data() {
    let labels = ["One", "Two", "Three"];
    let ui: Element<App> = virtual_list(labels.len(), 32., |i| {
        data_item(i, labels[i]).key(format!("borrow-{i}"))
    })
    .key("borrow")
    .selected(None)
    .on_select(|_: &mut App, _| {})
    .on_window_change(|_, _| {})
    .into();
    ui.build().unwrap();
    assert_eq!(labels.len(), 3);
}
#[test]
fn virtual_invalid_windows_fail_before_calling_the_factory() {
    for window in [std::ops::Range { start: 20, end: 10 }, 0..97, 99..101] {
        let calls = Rc::new(Cell::new(0));
        let count = calls.clone();
        let ui: Element<App> = virtual_list(100, 32., move |i| {
            count.set(count.get() + 1);
            data_item(i, "Row").key(format!("invalid-{i}"))
        })
        .key("invalid")
        .window(window)
        .on_select(|_: &mut App, _| {})
        .on_window_change(|_, _| {})
        .into();
        assert!(ui.build().is_err());
        assert_eq!(calls.get(), 0);
    }
}
#[test]
fn virtual_empty_disabled_rows_and_selection_reorder_are_safe() {
    let mut app = App {
        window: 0..5,
        ..App::default()
    };
    let (mut r, mut h) = mount(list(&app), 1.);
    assert!(!click(&mut r, &mut h, &mut app, "r-2"));
    assert_eq!(app.selected, None);
    assert!(click(&mut r, &mut h, &mut app, "r-1"));
    assert_eq!(app.selected, Some(1));
    let ui: Element<App> = virtual_list(0, 32., |i| data_item(i, "Never").key("never"))
        .key("empty")
        .on_select(|_: &mut App, _| {})
        .on_window_change(|_, _| {})
        .into();
    assert!(ui
        .build()
        .unwrap()
        .into_parts()
        .0
        .root()
        .children()
        .is_empty());
}
fn sorted_table(direction: SortDirection) -> Element<App> {
    table(
        [table_column(0, "Name"), table_column(1, "Age")],
        [
            table_row(1, ["Beta", "20"]).key("beta"),
            table_row(2, ["Alpha", "40"]).key("alpha"),
        ],
    )
    .key("table")
    .sort(0, direction)
    .selected(Some(1))
    .on_select(|app: &mut App, id| app.selected = Some(id))
    .on_sort(|app: &mut App, col, direction| app.sort = Some((col, direction)))
    .into()
}
#[test]
fn table_really_sorts_and_headers_emit_typed_sort_direction() {
    for scale in [1., 2.] {
        let (mut r, mut h) = mount(sorted_table(SortDirection::Ascending), scale);
        assert_eq!(
            r.document()
                .root()
                .children()
                .iter()
                .find(|n| n.id() == "alpha" || n.id() == "beta")
                .unwrap()
                .id(),
            "alpha"
        );
        let mut app = App::default();
        assert!(click(&mut r, &mut h, &mut app, "table-column-0"));
        assert_eq!(app.sort, Some((0, SortDirection::Descending)));
        replace(
            &mut r,
            &mut h,
            sorted_table(SortDirection::Descending),
            scale,
        );
        assert_eq!(
            r.document()
                .root()
                .children()
                .iter()
                .find(|n| n.id() == "alpha" || n.id() == "beta")
                .unwrap()
                .id(),
            "beta"
        );
        assert!(click(&mut r, &mut h, &mut app, "alpha"));
        assert_eq!(app.selected, Some(2));
    }
}
#[test]
fn table_typed_comparator_can_sort_numbers_not_display_strings() {
    let ui: Element<App> = table(
        [table_column(0, "Count")],
        [
            table_row(20, ["20"]).key("twenty"),
            table_row(3, ["3"]).key("three"),
        ],
    )
    .key("numbers")
    .sort(0, SortDirection::Ascending)
    .sort_by(|_, a, b| a.cmp(b))
    .on_select(|_: &mut App, _| {})
    .on_sort(|_, _, _| {})
    .into();
    let (d, _) = ui.build().unwrap().into_parts();
    assert_eq!(
        d.root()
            .children()
            .iter()
            .find(|n| n.id() == "three" || n.id() == "twenty")
            .unwrap()
            .id(),
        "three"
    );
}
#[test]
fn table_rejects_wrong_cells_duplicates_missing_callback_and_node_budget() {
    let make = || {
        table::<App, _, _>([table_column(0, "A")], [table_row(1, ["A"]).key("one")]).key("table")
    };
    assert!(Element::from(make()).build().is_err());
    let ui: Element<App> = table(
        (0..8).map(|i| table_column(i, "C")),
        // 2 + 8 + 400 * (3 + 8) nodes exceeds the 4096-node document budget.
        (0..400).map(|i| {
            table_row(i, ["1", "2", "3", "4", "5", "6", "7", "8"]).key(format!("row-{i}"))
        }),
    )
    .key("big")
    .on_select(|_: &mut App, _| {})
    .on_sort(|_, _, _| {})
    .into();
    assert!(ui.build().is_err());
    let ui: Element<App> = table(
        [table_column(0, "A")],
        [table_row(1, ["A", "B"]).key("one")],
    )
    .key("bad")
    .on_select(|_: &mut App, _| {})
    .on_sort(|_, _, _| {})
    .into();
    assert!(ui.build().is_err());
}
fn tree(app: &App) -> Element<App> {
    tree_view([
        tree_node(0, "Root")
            .key("root")
            .children([tree_node(1, "Child").key("child")]),
        tree_node(2, "Disabled").key("disabled").enabled(false),
    ])
    .key("tree")
    .expanded(app.expanded.clone())
    .selected(app.selected)
    .on_select(|app: &mut App, id| app.selected = Some(id))
    .on_expand(|app: &mut App, id, open| {
        if open {
            app.expanded.push(id);
        } else {
            app.expanded.retain(|value| *value != id);
        }
    })
    .into()
}
#[test]
fn tree_expand_select_and_collapse_mounts_only_live_rows() {
    for scale in [1., 2.] {
        let mut app = App::default();
        let (mut r, mut h) = mount(tree(&app), scale);
        assert!(!r.boxes().iter().any(|b| b.id == "child"));
        assert!(click(&mut r, &mut h, &mut app, "root-expand"));
        replace(&mut r, &mut h, tree(&app), scale);
        assert!(click(&mut r, &mut h, &mut app, "child"));
        assert_eq!(app.selected, Some(1));
        assert!(!click(&mut r, &mut h, &mut app, "disabled"));
        assert!(click(&mut r, &mut h, &mut app, "root-expand"));
        replace(&mut r, &mut h, tree(&app), scale);
        assert!(!r.boxes().iter().any(|b| b.id == "child"));
        assert_eq!(app.selected, Some(1));
    }
}
#[test]
fn tree_duplicate_values_and_oversized_models_are_rejected() {
    let ui: Element<App> =
        tree_view([tree_node(0, "Root")
            .key("root")
            .children([tree_node(0, "Duplicate").key("child")])])
        .key("tree")
        .on_select(|_: &mut App, _| {})
        .on_expand(|_, _, _| {})
        .into();
    assert!(ui.build().is_err());
    let ui: Element<App> =
        tree_view((0..129).map(|id| tree_node(id, "Row").key(format!("node-{id}"))))
            .key("big")
            .on_select(|_: &mut App, _| {})
            .on_expand(|_, _, _| {})
            .into();
    assert!(ui.build().is_err());
}

#[test]
fn table_cells_belong_to_rows_and_numeric_alignment_matches_the_header() {
    use volt_ui_kit::declarative::AccessRole;
    for scale in [1., 2.] {
        let ui: Element<App> = table(
            [
                table_column(0, "Name").width(140.).sortable(false),
                table_column(1, "Amount").width(120.).align(Align::End),
            ],
            [
                table_row(0, ["First", "$12.00"]).key("first"),
                table_row(1, ["Last", "$24.00"]).key("last"),
            ],
        )
        .key("table")
        .width(360.)
        .sort(1, SortDirection::Descending)
        .on_select(|a: &mut App, id| a.selected = Some(id))
        .on_sort(|a: &mut App, col, dir| a.sort = Some((col, dir)))
        .into();
        let (mut r, mut h) = mount(ui, scale);
        let nodes = r.accessibility_nodes();
        let frame = nodes.iter().find(|n| n.id == "table").unwrap();
        let info = frame.table.unwrap();
        assert_eq!((info.rows, info.columns), (Some(3), Some(2)));
        let header = nodes.iter().find(|n| n.id == "table-column-1").unwrap();
        assert_eq!(header.role, AccessRole::ColumnHeader);
        assert_eq!(header.label, "Amount"); // no arrow glyph in the name
        assert_eq!(header.table.unwrap().sort_ascending, Some(false));
        assert!(header.clickable);
        let fixed_header = nodes
            .iter()
            .find(|n| n.role == AccessRole::ColumnHeader && n.label == "Name")
            .unwrap();
        assert!(!fixed_header.clickable);
        for id in ["first", "last"] {
            let row = nodes.iter().find(|n| n.id == id).unwrap();
            let box_ = r.boxes().iter().find(|b| b.id == id).unwrap();
            let [x, y, w, height] = box_.bounds.bounds();
            assert_eq!(box_.clip.bounds(), [x, y, w, height]);
            assert!(x >= frame.bounds[0] + scale);
            assert!(x + w <= frame.bounds[0] + frame.bounds[2] - scale + 0.01);
            assert_eq!(height, 44. * scale);
            for (index, cell) in nodes.iter().enumerate().filter(|(_, n)| {
                n.role == AccessRole::Cell && n.parent.is_some_and(|p| nodes[p].id == id)
            }) {
                assert_eq!(cell.table.unwrap().row, row.table.unwrap().row);
                if cell.table.unwrap().column == Some(1) {
                    let end = cell.bounds[0] + cell.bounds[2];
                    assert!(
                        (end - (header.bounds[0] + header.bounds[2] - 12. * scale)).abs() < 0.01
                    );
                }
                // Text itself is not an independent target; every cell selects its row.
                let [cx, cy, cw, ch] = nodes[index].bounds;
                let mut app = App::default();
                r.pointer_down_outcome(cx + cw / 2., cy + ch / 2., &mut h, &mut app)
                    .unwrap();
                assert!(
                    r.pointer_up_outcome(cx + cw / 2., cy + ch / 2., &mut h, &mut app)
                        .unwrap()
                        .activated
                );
                assert_eq!(app.selected, Some(if id == "first" { 0 } else { 1 }));
            }
        }
    }
}

#[test]
fn constrained_tables_scroll_instead_of_squeezing_or_truncating_columns() {
    for scale in [1., 2.] {
        let ui: Element<App> = table(
            [
                table_column(0, "A"),
                table_column(1, "B"),
                table_column(2, "C"),
            ],
            (0..8).map(|id| table_row(id, ["A", "B", "C"]).key(format!("row-{id}"))),
        )
        .key("table")
        .width(180.)
        .height(100.)
        .on_select(|a: &mut App, id| a.selected = Some(id))
        .on_sort(|_, _, _| {})
        .into();
        let (mut r, _) = mount(ui, scale);
        let frame = r.boxes().iter().find(|b| b.id == "table").unwrap();
        assert!(frame.scroll_max[0] > 180.);
        assert!(frame.scroll_max[1] > 200.);
        let nodes = r.accessibility_nodes();
        let last = nodes.iter().find(|n| n.id == "row-7").unwrap();
        assert_eq!(last.bounds[2], 360. * scale);
        assert!(r.accessibility_focus("row-7"));
        assert!(r.scroll_offset("table")[1] > 200.);
    }
}

#[test]
fn tree_uses_vector_disclosures_inset_rows_and_real_semantic_hierarchy() {
    use volt_ui_kit::declarative::{AccessRole, Kind, Node};
    fn find<'a>(n: &'a Node, id: &str) -> Option<&'a Node> {
        if n.id() == id {
            Some(n)
        } else {
            n.children().iter().find_map(|c| find(c, id))
        }
    }
    fn has_canvas(n: &Node) -> bool {
        n.kind() == Kind::Canvas || n.children().iter().any(has_canvas)
    }
    for scale in [1., 2.] {
        let app = App {
            expanded: vec![0],
            ..App::default()
        };
        let (mut r, mut h) = mount(tree(&app), scale);
        assert!(has_canvas(
            find(r.document().root(), "root-expand").unwrap()
        ));
        let nodes = r.accessibility_nodes();
        let root = nodes.iter().find(|n| n.id == "root").unwrap();
        let child = nodes.iter().find(|n| n.id == "child").unwrap();
        assert_eq!(root.role, AccessRole::TreeItem);
        assert_eq!(root.size_of_set, Some(1));
        assert_eq!(child.position_in_set, Some(0));
        assert_eq!(child.level, Some(1));
        assert_eq!(nodes[child.parent.unwrap()].id, "root");
        for id in ["root", "child", "disabled"] {
            let b = r.boxes().iter().find(|b| b.id == id).unwrap();
            assert_eq!(b.clip.bounds(), b.bounds.bounds());
            assert_eq!(b.bounds.bounds()[0], 6. * scale);
            assert_eq!(b.bounds.bounds()[3], 36. * scale);
        }
        let mut app = app;
        assert!(click(&mut r, &mut h, &mut app, "root-expand"));
        assert!(app.expanded.is_empty());
        assert_eq!(app.selected, None); // expansion never leaks into row selection
        replace(&mut r, &mut h, tree(&app), scale);
        assert!(!r.accessibility_nodes().iter().any(|n| n.id == "child"));
    }
}

#[derive(Clone, Debug)]
struct CornerShape {
    bounds: [f32; 4],
    radii: [f32; 4],
    outline: bool,
}
struct CornerPainter(Metrics);
impl TextMetrics for CornerPainter {
    fn measure(&mut self, s: &str, size: f32, width: f32) -> [f32; 2] {
        self.0.measure(s, size, width)
    }
}
impl Painter for CornerPainter {
    type Shape = CornerShape;
    type Glyph = ();
    fn viewport(&self) -> Viewport {
        self.0.viewport()
    }
    fn draw_rect(&self, out: &mut Vec<CornerShape>, x: f32, y: f32, w: f32, h: f32, _: [f32; 4]) {
        out.push(CornerShape {
            bounds: [x, y, w, h],
            radii: [0.; 4],
            outline: false,
        });
    }
    fn card_round(&self, out: &mut Vec<CornerShape>, bounds: [f32; 4], radius: f32, _: [f32; 4]) {
        out.push(CornerShape {
            bounds,
            radii: [radius; 4],
            outline: false,
        });
    }
    fn card_corners(
        &self,
        out: &mut Vec<CornerShape>,
        bounds: [f32; 4],
        radii: [f32; 4],
        _: [f32; 4],
    ) {
        out.push(CornerShape {
            bounds,
            radii,
            outline: false,
        });
    }
    fn card_outline(
        &self,
        out: &mut Vec<CornerShape>,
        bounds: [f32; 4],
        radii: [f32; 4],
        _: f32,
        _: [f32; 4],
    ) {
        out.push(CornerShape {
            bounds,
            radii,
            outline: true,
        });
    }
    fn card_triangle(&self, _: &mut Vec<CornerShape>, _: [[f32; 2]; 3], _: [f32; 4]) {}
    fn card_text(&mut self, _: &mut Vec<()>, _: &str, _: f32, _: f32, _: f32, _: [f32; 4]) {}
    fn draw_text_colored(
        &mut self,
        _: &mut Vec<()>,
        _: &str,
        _: f32,
        _: f32,
        _: f32,
        _: f32,
        _: &dyn Fn(usize) -> [f32; 4],
    ) {
    }
}
impl DocumentPainter for CornerPainter {
    fn ui_text(&mut self, _: &mut Vec<()>, _: &str, _: [f32; 4], _: f32, _: [f32; 4]) {}
}

#[test]
fn table_frame_header_last_selection_and_focus_share_one_corner_contour() {
    for scale in [1., 2.] {
        for radius in [0., 12., 20.] {
            for direction in [SortDirection::Ascending, SortDirection::Descending] {
                let last = if direction == SortDirection::Ascending {
                    2
                } else {
                    0
                };
                let ui: Element<App> = table(
                    [table_column(0usize, "Name")],
                    (0..3)
                        .map(|id| table_row(id, [format!("Record {id}")]).key(format!("row-{id}"))),
                )
                .key("table")
                .rounded(radius)
                .sort(0, direction)
                .selected(Some(last))
                .on_select(|a: &mut App, id| a.selected = Some(id))
                .on_sort(|a: &mut App, k, d| a.sort = Some((k, d)))
                .into();
                let (doc, h) = ui.build().unwrap().into_parts();
                let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
                let mut p = CornerPainter(Metrics(scale));
                r.layout(p.viewport(), &Bindings::default(), &mut p)
                    .unwrap();
                assert!(r.accessibility_focus(&format!("row-{last}")));
                r.settle_animations();
                let batches = r.paint_ui(&mut p, &volt_ui_kit::UiTheme::dark());
                let shapes = batches.iter().flat_map(|b| &b.shapes).collect::<Vec<_>>();
                let frame = r
                    .boxes()
                    .iter()
                    .find(|b| b.id == "table")
                    .unwrap()
                    .bounds
                    .bounds();
                assert!(
                    shapes
                        .iter()
                        .any(|s| s.outline && s.bounds == frame && s.radii == [radius * scale; 4]),
                    "outer border must be one continuous outline"
                );
                let header_id = r.document().root().children()[0].id();
                let header = r
                    .boxes()
                    .iter()
                    .find(|b| b.id == header_id)
                    .unwrap()
                    .bounds
                    .bounds();
                assert_eq!(
                    header[0],
                    frame[0] + scale,
                    "only the border stroke separates the header from the frame"
                );
                let inner = (radius - 1.).max(0.) * scale;
                let small = if radius == 0. { 0. } else { 4. * scale };
                assert!(shapes.iter().any(|s| !s.outline
                    && s.bounds == header
                    && s.radii == [inner, inner, small, small]));
                let row = r
                    .boxes()
                    .iter()
                    .find(|b| b.id == format!("row-{last}"))
                    .unwrap()
                    .bounds
                    .bounds();
                assert!(
                    shapes.iter().any(|s| !s.outline
                        && s.bounds == row
                        && s.radii == [small, small, inner, inner]),
                    "last row selection must follow bottom table corners after sorting"
                );
                assert!(
                    shapes.iter().any(|s| s.outline
                        && s.bounds == row
                        && s.radii == [small, small, inner, inner]),
                    "focus outline must follow the same corners"
                );
            }
        }
    }
}
