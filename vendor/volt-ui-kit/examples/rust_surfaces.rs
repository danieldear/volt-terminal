//! Semantic themes, shared image surfaces, retained vectors and clipping in Rust.
#[path = "support/rust_ui.rs"]
mod support;
use volt_ui_kit::ui::prelude::*;
struct App {
    mode: Option<ThemeMode>,
    saved: usize,
    image: Image,
}
fn theme(app: &App, system: ThemeMode) -> UiTheme {
    let base = if app.mode.unwrap_or(system) == ThemeMode::Light {
        UiTheme::light()
    } else {
        UiTheme::dark()
    };
    base.with_accent([0.38, 0.42, 0.92, 1.])
}
fn view(app: &App) -> Element<App> {
    let artwork = canvas(|c| {
        c.fill(VectorPath::circle(90., 80., 52.), ColorToken::Accent);
        c.stroke(
            VectorPath::circle(90., 80., 64.),
            ColorToken::BaseContent,
            2.,
        );
        c.fill(
            VectorPath::new()
                .move_to(160., 120.)
                .line_to(190., 25.)
                .line_to(230., 120.)
                .close(),
            ColorToken::Success,
        );
        c.stroke(
            VectorPath::new()
                .move_to(20., 130.)
                .cubic_to([80., 20.], [180., 170.], [250., 40.]),
            ColorToken::Warning,
            4.,
        );
    })
    .height(160.);
    vstack![
        hstack![text("SURFACES / RUST").font_size(12.).font_weight(700),spacer(),
            button("Light / Dark").key("theme").on_click(|app:&mut App|app.mode=Some(if app.mode==Some(ThemeMode::Light){ThemeMode::Dark}else{ThemeMode::Light}))
        ].height(44.).flow(Flow::Center),
        text("One surface language.").font_size(32.).font_weight(700),
        text("Images, vectors, colors and clipping compose together. Scroll when the window gets smaller.").color(ColorToken::Muted),
        stack![
            panel![text("IMAGE BACKGROUND").font_size(12.).font_weight(700),spacer(),
                text("The image fades. The text stays crisp.").font_size(23.).font_weight(600)]
                .height(240.).flex(1.).background(ColorToken::Base200)
                .background_image(app.image.clone().background().fit(ImageFit::Cover).opacity(0.45))
                .padding(24.,Edge::All).rounded(18.).clip_contents(true),
            panel![text("VECTOR CANVAS").font_size(12.).font_weight(700),artwork,
                text("Fill, stroke, curves and Rust transforms.").color(ColorToken::Muted)]
                .height(240.).flex(1.).padding(18.,Edge::All).rounded(18.).clip_contents(true)
                .background(ColorToken::Base200),
        ].direction(responsive(Direction::Vertical).at(Breakpoint::Md,Direction::Horizontal)).gap(20.,Axis::Both),
        panel![
            text("CLIPPED CONTENT").font_size(12.).font_weight(700),
            zstack![canvas(|c|c.fill(VectorPath::rectangle(0.,0.,120.,120.),ColorToken::Primary)).width(120.).height(120.)]
                .height(100.).width(100.).clip(Clip::Circle),
            hstack![button("Save").key("save").primary(true).on_click(|app:&mut App|app.saved+=1)
                .glow(Glow::new().color(ColorToken::Accent).blur(12.).spread(1.)),
                text(format!("Saved {} times",app.saved)).color(ColorToken::Muted)].gap(20.,Axis::Horizontal),
        ].padding(24.,Edge::All).gap(16.,Axis::Vertical).background(ColorToken::Base200).rounded(16.),
    ].padding(28.,Edge::All).gap(22.,Axis::Vertical).background(ColorToken::Base100).color(ColorToken::BaseContent).scroll(Scroll::Vertical).into()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A generated demo asset avoids file-location assumptions. For a real asset:
    // let image = volt_ui_kit::native::load_image("assets/panel.jpg")?;
    let mut pixels = Vec::with_capacity(256 * 256 * 4);
    for y in 0..256 {
        for x in 0..256 {
            let band = (((x + y) / 32) % 2) * 25;
            pixels.extend([((x / 2) + 40 + band) as u8, ((y / 2) + 40) as u8, 180, 255]);
        }
    }
    let image = Image::from_rgba(256, 256, pixels)?;
    support::launch_themed(
        "Volt UI — Surface composition",
        App {
            mode: None,
            saved: 0,
            image,
        },
        view,
        theme,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use volt_ui_kit::{
        declarative::{Bindings, Runtime},
        native::RasterPainter,
        Viewport,
    };
    #[test]
    fn responsive_surface_layout_theme_and_scrolled_callbacks_at_both_dpis() {
        for [width, height] in [[600., 820.], [960., 900.]] {
            for scale in [1., 2.] {
                let mut app = App {
                    mode: None,
                    saved: 0,
                    image: Image::from_rgba(1, 1, vec![100, 140, 180, 255]).unwrap(),
                };
                let (doc, mut handlers) = view(&app).build().unwrap().into_parts();
                let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
                let vp = Viewport::new(width * scale, height * scale, scale).unwrap();
                let mut p = RasterPainter::new(vp);
                runtime.layout(vp, &Bindings::default(), &mut p).unwrap();
                assert!(
                    runtime.boxes()[0].scroll_max[1] < 512.,
                    "unbounded flex gap"
                );
                let b = runtime.paint_ui(&mut p, &theme(&app, ThemeMode::Dark));
                p.rasterize(&b, theme(&app, ThemeMode::Dark).palette.base[0])
                    .unwrap();
                runtime.scroll_at(width / 2. * scale, height / 2. * scale, [0., 16_384.]);
                runtime.layout(vp, &Bindings::default(), &mut p).unwrap();
                let save = runtime
                    .boxes()
                    .iter()
                    .find(|b| b.id == "save")
                    .expect("Save reachable by scrolling");
                let [x, y, w, h] = save.bounds.bounds();
                let center = [x + w / 2., y + h / 2.];
                runtime.pointer_down(center[0], center[1]);
                runtime
                    .pointer_up(center[0], center[1], &mut handlers, &mut app)
                    .unwrap();
                assert_eq!(app.saved, 1);
                app.mode = Some(ThemeMode::Light);
                let b = runtime.paint_ui(&mut p, &theme(&app, ThemeMode::Light));
                p.rasterize(&b, theme(&app, ThemeMode::Light).palette.base[0])
                    .unwrap();
            }
        }
    }
}
