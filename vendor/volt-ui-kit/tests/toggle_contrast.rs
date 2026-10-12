#![cfg(feature = "native")]
//! A checked switch's knob and a checkbox's tick contrast with the accent
//! fill, including a white accent (monochrome dark themes).
use volt_ui_kit::{
    declarative::{Bindings, Runtime},
    native::RasterPainter,
    ui::{prelude::*, Palette, UiTheme},
    Viewport,
};

fn render(e: Element<()>, accent: [f32; 4]) -> tiny_skia::Pixmap {
    let (d, h) = e.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    let vp = Viewport::new(200., 40., 1.).unwrap();
    let mut p = RasterPainter::new(vp);
    r.layout(vp, &Bindings::default(), &mut p).unwrap();
    let mut palette = Palette::dark();
    palette.accent = accent;
    palette.content = [0.98, 0.98, 0.98, 1.];
    let ui = UiTheme::from_palette(palette);
    let batches = r.paint_ui(&mut p, &ui);
    p.rasterize(&batches, [0., 0., 0., 1.]).unwrap().clone()
}

#[test]
fn checked_switch_knob_is_visible_on_a_white_accent() {
    let white = [0.98, 0.98, 0.98, 1.];
    let switch: Element<()> = toggle("Push", true)
        .key("push")
        .on_change(|_: &mut (), _| {})
        .width(200.)
        .height(40.)
        .into();
    let pixels = render(switch, white);
    // Track spans x 2..44 and y 9..31; the checked knob sits at its right end.
    let knob = pixels.pixel(33, 20).unwrap();
    let track = pixels.pixel(8, 20).unwrap();
    assert!(track.red() > 200, "track is the white accent");
    assert!(knob.red() < 60, "knob contrasts with the white track");
}
