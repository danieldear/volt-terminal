//! Integrated macOS chrome, corner selections, bounded shadows and paint-only motion.
#[path = "support/rust_ui.rs"]
mod support;
use volt_ui_kit::{native::WindowChrome, ui::prelude::*};

struct App {
    count: usize,
    notice: bool,
}
impl Default for App {
    fn default() -> Self {
        Self {
            count: 0,
            notice: true,
        }
    }
}
fn view(app: &App) -> Element<App> {
    let notification: Element<App> = if app.notice {
        panel![
            text("Saved — fade out is paint-only; removed controls cannot activate.")
                .font_size(13.)
        ]
        .key("notification")
        .height(56.)
        .background(ColorToken::Success)
        .color(ColorToken::SuccessContent)
        .rounded_on(12, Corners::Left)
        .padding(14., [Edge::Top, Edge::Left, Edge::Right])
        .transition(Transition::fade().duration_ms(180))
        .into()
    } else {
        spacer().key("notification-space").height(56.).into()
    };
    vstack![
        hstack![
            text("VOLT UI / EFFECTS").font_size(13.).font_weight(700),
            spacer(),
            button("Toggle notice").key("notice-toggle")
                .on_click(|app:&mut App|app.notice = !app.notice),
        ].key("header").height(48.).padding(12.,Edge::Right).padding(80.,Edge::Left)
            .gap(12.,Axis::Horizontal).flow(Flow::Center).background(ColorToken::Base200).drag_region(true),
        vstack![
            text("Small corners. Intentional effects.").font_size(28.).font_weight(700),
            text("Move over Save; press it; use Tab. Callbacks run immediately, independently of animation.").font_size(13.).color(ColorToken::Muted),
            panel![
                text(format!("Saved {} times",app.count)).font_size(22.).font_weight(600),
                stack![
                    button("Save").key("save").width(130.).height(42.).background(ColorToken::Primary).color(ColorToken::PrimaryContent)
                        .border_width(1.).border_color(ColorToken::Border)
                        .hover_style(|s|s.background(ColorToken::Accent).border_color(ColorToken::Accent)
                            .glow(Glow::new().color("#60A5FA90").blur(16.).spread(1.)))
                        .pressed_style(|s|s.opacity(0.7).background(ColorToken::Primary))
                        .focus_style(|s|s.border_color(ColorToken::Accent))
                        .animate_on(Interaction::Hover,Animation::ease_out().duration_ms(120))
                        .animate_on(Interaction::Pressed,Animation::ease_out().duration_ms(70))
                        .animate_on(Interaction::Focus,Animation::ease_out().duration_ms(120))
                        .on_click(|app:&mut App|app.count+=1),
                    button("Square").key("square").rounded(0).width(110.).height(42.)
                        .on_click(|app:&mut App|app.count+=1),
                    button("Left only").key("left").rounded_on(Rounding::Full,Corners::Left)
                        .width(130.).height(42.).on_click(|app:&mut App|app.count+=1),
                ].direction(responsive(Direction::Vertical).at(Breakpoint::Md,Direction::Horizontal)).gap(16.,[Axis::Horizontal,Axis::Vertical]),
                divider().background(ColorToken::Border),
                text("The panel shadow is blurred and cached. Hover glow animates alpha, not blur geometry.").font_size(12.).color(ColorToken::Muted),
            ].key("effects-panel").padding(24.,Edge::All).gap(18.,Axis::Vertical)
                .background(ColorToken::Base200).rounded_on(14,[Corner::TopLeft,Corner::BottomRight])
                .shadow(Shadow::new().blur(18.).offset(0.,6.).color("#00000070")),
            notification,
            text("2px default / selected corners / native window controls / reduced-motion opt-out").font_size(12.).color(ColorToken::Muted),
        ].padding(28.,[Edge::Left,Edge::Right,Edge::Top]).gap(24.,Axis::Vertical),
    ].background(ColorToken::Base100).color(ColorToken::BaseContent).into()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    support::launch_with_chrome(
        "Volt UI Kit — Effects",
        App::default(),
        view,
        WindowChrome::Overlay,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use volt_ui_kit::{
        declarative::{Bindings, Runtime},
        native::RasterPainter,
        Theme, Viewport,
    };
    #[test]
    fn real_fonts_effects_dpi_callbacks_and_fade_exit() {
        for scale in [1., 1.5, 2.] {
            let vp = Viewport::new(960. * scale, 760. * scale, scale).unwrap();
            let mut painter = RasterPainter::new(vp);
            let values = Bindings::default();
            let mut app = App::default();
            let (doc, mut h) = view(&app).build().unwrap().into_parts();
            let mut r = Runtime::new(doc, &h, &values).unwrap();
            r.layout(vp, &values, &mut painter).unwrap();
            let theme = Theme::dark();
            for ms in [0, 60, 120, 180] {
                r.advance_animations(Duration::from_millis(ms));
                let b = r.paint(&mut painter, &theme);
                painter.rasterize(&b, theme.background).unwrap();
            }
            assert!(!r.is_animating());
            let save = r
                .boxes()
                .iter()
                .find(|b| b.id == "save")
                .unwrap()
                .bounds
                .bounds();
            let (x, y) = (save[0] + save[2] / 2., save[1] + save[3] / 2.);
            r.pointer_move(x, y);
            let b = r.paint(&mut painter, &theme);
            painter.rasterize(&b, theme.background).unwrap();
            r.pointer_down(x, y);
            r.pointer_up(x, y, &mut h, &mut app).unwrap();
            assert_eq!(app.count, 1);
            app.notice = false;
            let (next, nh) = view(&app).build().unwrap().into_parts();
            r.replace_document(next, &nh, &values).unwrap();
            r.layout(vp, &values, &mut painter).unwrap();
            assert!(!r.boxes().iter().any(|b| b.id == "notification"));
            for ms in [180, 240, 300, 360] {
                r.advance_animations(Duration::from_millis(ms));
                let b = r.paint(&mut painter, &theme);
                painter.rasterize(&b, theme.background).unwrap();
            }
            assert!(painter.effect_cache_misses > 0);
            assert!(r.drag_region_hit(300. * scale, 24. * scale));
            assert!(!r.drag_region_hit(x, y));
        }
    }
}
