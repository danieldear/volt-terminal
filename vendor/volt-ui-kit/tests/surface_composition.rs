#![cfg(feature = "native")]
use volt_ui_kit::{
    declarative::{Bindings, Runtime},
    native::RasterPainter,
    ui::prelude::*,
    Painter, Viewport,
};
fn runtime(
    ui: Element<usize>,
    scale: f32,
    size: [f32; 2],
) -> (
    Runtime,
    volt_ui_kit::declarative::Handlers<'static, usize>,
    RasterPainter,
) {
    let (doc, h) = ui.build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    let vp = Viewport::new(size[0] * scale, size[1] * scale, scale).unwrap();
    let mut p = RasterPainter::new(vp);
    r.layout(vp, &Bindings::default(), &mut p).unwrap();
    (r, h, p)
}

#[test]
fn hidden_horizontal_scrollbar_removes_only_indicator_not_scrolling_or_callbacks() {
    for scale in [1., 2.] {
        for hidden in [false, true] {
            let ui = hstack![
                spacer().width(360.),
                button("End")
                    .key("end")
                    .width(80.)
                    .on_click(|count: &mut usize| *count += 1)
            ]
            .height(80.)
            .width(200.)
            .scroll(Scroll::Horizontal)
            .scrollbars(if hidden {
                Scrollbars::Hidden
            } else {
                Scrollbars::Auto
            })
            .into();
            let (mut r, mut callbacks, mut p) = runtime(ui, scale, [200., 80.]);
            let batch = r.paint_ui(&mut p, &UiTheme::dark());
            let pixmap = p.rasterize(&batch, [0., 0., 0., 1.]).unwrap();
            let pixel = pixmap
                .pixel((12. * scale) as u32, (75. * scale) as u32)
                .unwrap();
            assert_eq!(pixel.red() == 0, hidden);
            assert!(r.scroll_at(20. * scale, 20. * scale, [16_384., 0.]));
            r.layout(p.viewport(), &Bindings::default(), &mut p)
                .unwrap();
            let [x, y, w, h] = r
                .boxes()
                .iter()
                .find(|b| b.id == "end")
                .unwrap()
                .bounds
                .bounds();
            let mut count = 0;
            r.pointer_down(x + w / 2., y + h / 2.);
            r.pointer_up(x + w / 2., y + h / 2., &mut callbacks, &mut count)
                .unwrap();
            assert_eq!(count, 1);
        }
    }
}

#[test]
fn natural_badge_text_is_not_rewrapped_by_fractional_width_roundoff() {
    let labels = ["Left", "Primary", "Accent", "Saved status"];
    for family in ["system", "monospace", "serif"] {
        for scale in [1., 1.25, 1.5, 2.] {
            let ui = vstack(
                labels
                    .into_iter()
                    .enumerate()
                    .map(|(i, label)| badge(label).key(format!("badge-{i}")).into())
                    .collect::<Vec<_>>(),
            )
            .font_family(family)
            .font_size(14.)
            .gap(4., Axis::Vertical)
            .into();
            let (r, _, mut p) = runtime(ui, scale, [600., 400.]);
            for (i, label) in labels.into_iter().enumerate() {
                let b = r
                    .boxes()
                    .iter()
                    .find(|b| b.id == format!("badge-{i}"))
                    .unwrap();
                let [_, _, width, height] = b.bounds.bounds();
                let measured = volt_ui_kit::declarative::TextMetrics::measure_styled(
                    &mut p,
                    label,
                    &b.text_style,
                    width - 20. * scale,
                );
                assert!(
                    measured[1] <= height - 10. * scale + 0.01,
                    "{family} {scale} {label}: measured {measured:?}, badge {width}x{height}"
                );
            }
        }
    }
}
#[test]
fn glow_escapes_tight_layout_parents_on_all_four_sides_at_both_dpis() {
    for s in [1., 2.] {
        let ui = vstack![hstack![button("Glow")
            .key("glow")
            .width(100.)
            .height(42.)
            .background("#000000")
            .glow(Glow::new().color("#FFFFFF").blur(16.).spread(1.))
            .on_click(|_: &mut usize| {})]]
        .padding(24., Edge::All)
        .background("#000000")
        .into();
        let (r, _, mut p) = runtime(ui, s, [400., 240.]);
        let b = r.boxes().iter().find(|b| b.id == "glow").unwrap();
        let [x, y, w, h] = b.bounds.bounds();
        let batches = r.paint_ui(&mut p, &UiTheme::dark());
        let pix = p.rasterize(&batches, [0., 0., 0., 1.]).unwrap();
        for [px, py] in [
            [x - 3. * s, y + h / 2.],
            [x + w + 3. * s, y + h / 2.],
            [x + w / 2., y - 3. * s],
            [x + w / 2., y + h + 3. * s],
        ] {
            assert!(
                pix.pixel(px as u32, py as u32).unwrap().red() > 0,
                "missing side at {s}x ({px},{py})"
            );
        }
    }
}
#[test]
fn semantic_tokens_update_without_layout_or_callbacks() {
    let (r, _, mut p) = runtime(
        vstack![spacer()
            .height(80.)
            .flex(0.)
            .background(ColorToken::Primary)]
        .background(ColorToken::Base100)
        .into(),
        1.,
        [200., 120.],
    );
    let a = r.paint_ui(&mut p, &UiTheme::dark().with_accent([1., 0., 0., 1.]));
    let a = p.rasterize(&a, [0., 0., 0., 1.]).unwrap();
    let b = r.paint_ui(&mut p, &UiTheme::light().with_accent([0., 0., 1., 1.]));
    let b = p.rasterize(&b, [0., 0., 0., 1.]).unwrap();
    assert!(a.pixel(10, 10).unwrap().red() > 240);
    assert!(b.pixel(10, 10).unwrap().blue() > 240);
    assert_ne!(a.pixel(10, 110), b.pixel(10, 110));
}
#[test]
fn background_alpha_does_not_fade_vectors_and_group_alpha_does() {
    let make = |group: f32| {
        canvas(|c| c.fill(VectorPath::rectangle(10., 10., 20., 20.), "#FFFFFF"))
            .width(60.)
            .height(60.)
            .background("#FF0000")
            .background_opacity(0.25)
            .opacity(group)
    };
    let (r, _, mut p) = runtime(make(1.), 1., [60., 60.]);
    let b = r.paint_ui(&mut p, &UiTheme::dark());
    let pix = p.rasterize(&b, [0., 0., 0., 0.]).unwrap();
    assert!(pix.pixel(2, 2).unwrap().alpha() < 70);
    assert_eq!(pix.pixel(15, 15).unwrap().alpha(), 255);
    let (r, _, mut p) = runtime(make(0.5), 1., [60., 60.]);
    let b = r.paint_ui(&mut p, &UiTheme::dark());
    let pix = p.rasterize(&b, [0.; 4]).unwrap();
    assert!((126..=129).contains(&pix.pixel(15, 15).unwrap().alpha()));
}
#[test]
fn rounded_and_path_clipping_hide_descendant_pixels_and_clicks() {
    for clip in [
        Clip::Rounded,
        Clip::Circle,
        Clip::Path(VectorPath::rectangle(20., 20., 60., 60.)),
    ] {
        let ui = zstack![button("")
            .key("child")
            .width(100.)
            .height(100.)
            .background("#FF0000")
            .on_click(|c: &mut usize| *c += 1)]
        .width(100.)
        .height(100.)
        .rounded(32.)
        .clip(clip)
        .into();
        let (mut r, mut h, mut p) = runtime(ui, 1., [100., 100.]);
        let b = r.paint_ui(&mut p, &UiTheme::dark());
        let pix = p.rasterize(&b, [0., 0., 0., 1.]).unwrap();
        assert_eq!(pix.pixel(1, 1).unwrap().red(), 0);
        assert_eq!(pix.pixel(50, 50).unwrap().red(), 255);
        let mut count = 0;
        r.pointer_down(1., 1.);
        r.pointer_up(1., 1., &mut h, &mut count).unwrap();
        assert_eq!(count, 0);
        r.pointer_down(50., 50.);
        r.pointer_up(50., 50., &mut h, &mut count).unwrap();
        assert_eq!(count, 1);
    }
}
#[test]
fn images_fit_clip_and_alpha_are_separate_from_content() {
    let image = Image::from_rgba(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255]).unwrap();
    for fit in [
        ImageFit::Cover,
        ImageFit::Contain,
        ImageFit::Stretch,
        ImageFit::Tile,
    ] {
        let ui = canvas(|c| c.fill(VectorPath::rectangle(20., 20., 10., 10.), "#FFFFFF"))
            .width(100.)
            .height(100.)
            .rounded(30.)
            .background_image(image.clone().background().fit(fit).opacity(0.5));
        let (r, _, mut p) = runtime(ui, 1., [100., 100.]);
        let b = r.paint_ui(&mut p, &UiTheme::dark());
        let pix = p.rasterize(&b, [0.; 4]).unwrap();
        assert_eq!(pix.pixel(1, 1).unwrap().alpha(), 0);
        assert_eq!(pix.pixel(25, 25).unwrap().alpha(), 255);
        let a = pix.pixel(50, 50).unwrap().alpha();
        assert!((126..=129).contains(&a), "image alpha {fit:?}: {a}");
    }
}
#[test]
fn scrolling_preserves_natural_sizes_clamps_and_routes_visible_controls() {
    for scale in [1., 2.] {
        let children = (0..10)
            .map(|i| {
                button(format!("Item {i}"))
                    .key(format!("item-{i}"))
                    .height(40.)
                    .on_click(|c: &mut usize| *c += 1)
                    .into()
            })
            .collect::<Vec<Element<usize>>>();
        let (mut r, mut h, mut p) = runtime(
            vstack(children)
                .key("scroll")
                .scroll(Scroll::Vertical)
                .into(),
            scale,
            [180., 120.],
        );
        assert_eq!(r.boxes()[0].scroll_max, [0., 280.]);
        assert_eq!(
            r.boxes()
                .iter()
                .find(|b| b.id == "item-0")
                .unwrap()
                .bounds
                .bounds()[3],
            40. * scale
        );
        assert!(r.scroll_at(20. * scale, 20. * scale, [0., 100.]));
        r.layout(p.viewport(), &Bindings::default(), &mut p)
            .unwrap();
        assert_eq!(r.scroll_offset("scroll"), [0., 100.]);
        assert!(!r.boxes().iter().any(|b| b.id == "item-0"));
        let mut n = 0;
        r.pointer_down(20. * scale, 20. * scale);
        r.pointer_up(20. * scale, 20. * scale, &mut h, &mut n)
            .unwrap();
        assert_eq!(n, 1);
        assert!(r.scroll_at(20. * scale, 20. * scale, [0., 10_000.]));
        r.layout(p.viewport(), &Bindings::default(), &mut p)
            .unwrap();
        assert_eq!(r.scroll_offset("scroll"), [0., 280.]);
        assert!(!r.scroll_at(20. * scale, 20. * scale, [0., 10.]));
        let pix = r.paint_ui(&mut p, &UiTheme::dark());
        p.rasterize(&pix, [0., 0., 0., 1.]).unwrap();
    }
}
#[test]
fn invalid_surfaces_are_rejected() {
    assert!(Image::from_rgba(0, 1, vec![]).is_err());
    assert!(canvas::<usize>(|c| c.fill(VectorPath::new(), "#FFFFFF"))
        .build()
        .is_err());
    assert!(
        Element::from(spacer::<usize>().background_opacity(f32::NAN))
            .build()
            .is_err()
    );
    assert!(Element::from(spacer::<usize>().background("@unknown"))
        .build()
        .is_err());
}
#[test]
fn auto_scroll_does_not_make_dividers_infinite_and_explicit_width_overflows() {
    let ui = vstack![
        divider(),
        canvas(|c| c.fill(VectorPath::rectangle(0., 0., 400., 300.), "#FF0000"))
            .width(400.)
            .height(300.)
    ]
    .key("auto")
    .scroll(Scroll::Auto)
    .into();
    let (r, _, _) = runtime(ui, 1., [200., 100.]);
    assert_eq!(r.boxes()[0].scroll_max, [200., 201.]);
    let ui = vstack![divider(), text("Short"), text("Short")]
        .key("auto")
        .scroll(Scroll::Auto)
        .into();
    let (r, _, _) = runtime(ui, 1., [200., 100.]);
    assert_eq!(r.boxes()[0].scroll_max, [0., 0.]);
}
#[test]
fn nested_scroll_bubbles_only_unconsumed_delta_and_offsets_survive_recompose() {
    let view = || {
        vstack![
            vstack![canvas(|_| {}).height(400.)]
                .key("inner")
                .height(100.)
                .scroll(Scroll::Vertical),
            canvas(|_| {}).height(500.),
        ]
        .key("outer")
        .scroll(Scroll::Vertical)
        .into()
    };
    let (mut r, _, mut p) = runtime(view(), 1., [200., 200.]);
    assert!(r.scroll_at(20., 20., [0., 350.]));
    r.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    assert_eq!(r.scroll_offset("inner"), [0., 300.]);
    assert_eq!(r.scroll_offset("outer"), [0., 50.]);
    let (doc, h) = view().build().unwrap().into_parts();
    r.replace_document(doc, &h, &Bindings::default()).unwrap();
    r.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    assert_eq!(r.scroll_offset("outer"), [0., 50.]);
    let vp = Viewport::new(200., 800., 1.).unwrap();
    p.set_viewport(vp);
    r.layout(vp, &Bindings::default(), &mut p).unwrap();
    assert_eq!(r.scroll_offset("outer"), [0., 0.]);
}
#[test]
fn clipping_respects_outer_effects_borders_text_and_reuses_masks() {
    let image = Image::from_rgba(1, 1, vec![0, 0, 255, 255]).unwrap();
    let ui = vstack![panel![
        text("Visible text"),
        canvas(|c| c.fill(VectorPath::rectangle(0., 0., 200., 100.), "#FFFFFF"))
    ]
    .key("panel")
    .width(100.)
    .height(80.)
    .background_image(image)
    .border_width(4.)
    .border_color("#FF0000")
    .rounded(16.)
    .clip_contents(true)
    .shadow(Shadow::new().color("#FFFFFF").blur(12.).offset(0., 0.))]
    .padding(24., Edge::All)
    .into();
    let (r, _, mut p) = runtime(ui, 1., [200., 160.]);
    let b = r.paint_ui(&mut p, &UiTheme::dark());
    let first = p.rasterize(&b, [0., 0., 0., 1.]).unwrap();
    let misses = p.clip_cache_misses;
    assert!(first.pixel(20, 60).unwrap().red() > 0);
    assert_eq!(first.pixel(25, 60).unwrap().red(), 255);
    assert_eq!(first.pixel(25, 60).unwrap().blue(), 0);
    p.rasterize(&b, [0., 0., 0., 1.]).unwrap();
    assert_eq!(p.clip_cache_misses, misses);
}
#[test]
fn flexible_scroll_body_does_not_shrink_fixed_header() {
    let ui = vstack![
        text("Header").key("header").height(40.),
        vstack(
            (0..20)
                .map(|_| text("Row").height(30.).into())
                .collect::<Vec<_>>()
        )
        .key("body")
        .flex(1.)
        .scroll(Scroll::Vertical)
    ]
    .into();
    let (r, _, _) = runtime(ui, 1., [200., 200.]);
    assert_eq!(
        r.boxes()
            .iter()
            .find(|b| b.id == "header")
            .unwrap()
            .bounds
            .bounds()[3],
        40.
    );
    assert_eq!(
        r.boxes()
            .iter()
            .find(|b| b.id == "body")
            .unwrap()
            .bounds
            .bounds()[3],
        160.
    );
}
#[test]
fn adaptive_flex_stack_in_scroll_view_has_no_unbounded_flex_gaps() {
    let ui = vstack![stack![
        panel![text("One")].key("one").height(120.).flex(1.),
        panel![text("Two")].key("two").height(120.).flex(1.)
    ]
    .gap(10., Axis::Vertical)]
    .key("scroll")
    .scroll(Scroll::Vertical)
    .into();
    let (r, _, _) = runtime(ui, 1., [300., 300.]);
    let a = r.boxes().iter().find(|b| b.id == "one").unwrap();
    let b = r.boxes().iter().find(|b| b.id == "two").unwrap();
    assert_eq!(b.bounds.bounds()[1] - a.bounds.bounds()[1], 130.);
    assert_eq!(r.boxes()[0].scroll_max, [0.; 2]);
}
#[test]
fn scrollbar_indicator_uses_scroll_view_clip_not_last_child_clip() {
    let ui = vstack(
        (0..10)
            .map(|_| text("Row").height(40.).into())
            .collect::<Vec<_>>(),
    )
    .scroll(Scroll::Vertical)
    .background("#000000")
    .into();
    let (r, _, mut p) = runtime(ui, 1., [180., 120.]);
    let b = r.paint_ui(&mut p, &UiTheme::dark());
    let pix = p.rasterize(&b, [0., 0., 0., 1.]).unwrap();
    assert!(pix.pixel(175, 10).unwrap().red() > 50);
}
#[test]
fn rounded_clip_masks_glyphs_at_both_dpis() {
    for scale in [1., 2.] {
        let ui = zstack![text("MMMMMMMM").font_size(40.).color("#FFFFFF")]
            .width(100.)
            .height(100.)
            .clip(Clip::Circle)
            .into();
        let (r, _, mut p) = runtime(ui, scale, [100., 100.]);
        let b = r.paint_ui(&mut p, &UiTheme::dark());
        let pix = p.rasterize(&b, [0.; 4]).unwrap();
        for y in 0..pix.height() {
            for x in 0..pix.width() {
                if (x as f32 + 0.5 - 50. * scale).hypot(y as f32 + 0.5 - 50. * scale)
                    > 50. * scale + 1.
                {
                    assert_eq!(
                        pix.pixel(x, y).unwrap().alpha(),
                        0,
                        "outside at {scale}x {x},{y}"
                    );
                }
            }
        }
        assert!(pix.pixels().iter().any(|p| p.alpha() > 0));
    }
}

#[test]
fn wheel_passthrough_is_explicit_and_does_not_disable_descendant_callbacks() {
    for scale in [1., 2.] {
        for passthrough in [false, true] {
            let ui = zstack![
                vstack![text("Scrollable content").height(420.)]
                    .key("under-scroll")
                    .scroll(Scroll::Vertical),
                vstack![
                    button("Overlay action")
                        .key("overlay-action")
                        .width(120.)
                        .height(38.)
                        .on_click(|count: &mut usize| *count += 1),
                    spacer().scroll_passthrough(passthrough)
                ]
                .key("overlay-layout")
                .scroll_passthrough(passthrough),
            ]
            .height(180.)
            .width(300.)
            .align(Align::Stretch)
            .into();
            let (mut r, mut callbacks, mut p) = runtime(ui, scale, [300., 180.]);
            assert_eq!(
                r.scroll_at(280. * scale, 120. * scale, [0., 40.]),
                passthrough
            );
            r.layout(p.viewport(), &Bindings::default(), &mut p)
                .unwrap();
            assert_eq!(
                r.scroll_offset("under-scroll")[1],
                if passthrough { 40. } else { 0. }
            );
            let b = r.boxes().iter().find(|b| b.id == "overlay-action").unwrap();
            let [x, y, w, h] = b.bounds.bounds();
            let mut count = 0;
            r.pointer_down(x + w / 2., y + h / 2.);
            assert!(
                r.pointer_up_outcome(x + w / 2., y + h / 2., &mut callbacks, &mut count)
                    .unwrap()
                    .activated
            );
            assert_eq!(count, 1);
        }
    }
}
