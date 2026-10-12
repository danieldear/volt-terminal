#![cfg(feature = "native")]
//! A revealing divider is invisible at rest, shows a pill on hover and keeps
//! the resize cursor for the whole drag.
use volt_ui_kit::{
    declarative::{Bindings, Cursor, Runtime},
    native::RasterPainter,
    ui::{prelude::*, UiTheme},
    Viewport,
};

#[derive(Default)]
struct App {
    width: f32,
}

fn view(app: &App) -> Element<App> {
    // A full-window passthrough layer on top (like a toast stack) must not
    // hide the divider's cursor.
    zstack![
        divider_split(app),
        vstack![text("toast")]
            .key("toast-layer")
            .flow(Flow::BottomRight)
            .scroll_passthrough(true),
    ]
    .key("root")
    .align(Align::Stretch)
    .into()
}

fn divider_split(app: &App) -> Element<App> {
    split(text("Sidebar"), text("Content"))
        .key("s")
        .width(600.)
        .height(300.)
        .first_size(app.width)
        .max_first(400.)
        .handle_size(10.)
        .handle_color("#00000000")
        .handle_style(SplitHandle::Reveal)
        .on_resize(|app: &mut App, w| app.width = w)
        .into()
}

/// Darkest pixel inside the divider (white background, dark pill).
fn darkest(r: &Runtime, p: &mut RasterPainter, divider: [f32; 4]) -> u8 {
    let batches = r.paint_ui(p, &UiTheme::light());
    let pix = p.rasterize(&batches, [1., 1., 1., 1.]).unwrap().clone();
    let mut min = 255;
    for y in divider[1] as u32..(divider[1] + divider[3]) as u32 {
        for x in divider[0] as u32..(divider[0] + divider[2]) as u32 {
            if let Some(c) = pix.pixel(x, y) {
                min = min.min(c.red());
            }
        }
    }
    min
}

#[test]
fn reveal_handle_shows_on_hover_and_keeps_the_cursor_while_dragging() {
    let mut app = App { width: 200. };
    let (d, mut h) = view(&app).build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    let vp = Viewport::new(600., 300., 1.).unwrap();
    let mut p = RasterPainter::new(vp);
    r.layout(vp, &Bindings::default(), &mut p).unwrap();
    let divider = r
        .boxes()
        .iter()
        .find(|b| b.id == "s-divider")
        .unwrap()
        .bounds
        .bounds();
    assert_eq!(divider[0], 200.);
    assert_eq!(divider[2], 10.);
    assert!(darkest(&r, &mut p, divider) > 250, "invisible at rest");
    let (cx, cy) = (divider[0] + 5., divider[1] + divider[3] * 0.5);
    r.pointer_move(cx, cy);
    assert!(darkest(&r, &mut p, divider) < 200, "pill on hover");
    assert_eq!(r.cursor_at(cx, cy), Cursor::ResizeColumn);
    r.pointer_down_outcome(cx, cy, &mut h, &mut app).unwrap();
    r.pointer_move_outcome(cx + 80., cy, &mut h, &mut app)
        .unwrap();
    assert_eq!(app.width, 280.);
    // Far from the divider mid-drag: still the resize cursor.
    assert_eq!(r.cursor_at(cx + 200., cy), Cursor::ResizeColumn);
    r.pointer_up_outcome(cx + 80., cy, &mut h, &mut app)
        .unwrap();
    assert_eq!(r.cursor_at(cx + 200., cy), Cursor::Default);
}
