use super::{App, Screen};
use volt_ui_kit::{
    declarative::{Bindings, Handlers, Runtime, ValueKey},
    native::{RasterPainter, WindowBackground},
    ui::prelude::*,
    Painter, Viewport,
};
struct Harness {
    app: App,
    runtime: Runtime,
    callbacks: Handlers<'static, App>,
    painter: RasterPainter,
}

fn app(screen: Screen) -> App {
    let mut app = App {
        section: crate::lab::Section::Showcase,
        ..App::default()
    };
    app.showcase.screen = screen;
    app
}

#[test]
fn atelier_activity_preserves_callbacks_across_overlays_wrap_and_cancellation() {
    for scale in [1., 2.] {
        let mut h = Harness::new(app(Screen::Activity), 1280., 900., scale);
        h.click("activity-counter");
        assert_eq!(h.app.count, 1);
        h.click("activity-menu-trigger");
        assert!(h.app.menu_open);
        assert!(
            h.runtime
                .dismiss_overlay(&mut h.callbacks, &mut h.app)
                .unwrap()
                .activated
        );
        h.recompose();
        assert!(!h.app.menu_open);
        h.click("activity-inspect");
        assert!(h.app.popover_open);
        h.click("activity-inspector-close");
        assert!(!h.app.popover_open);
        h.reveal("activity-notes");
        let [x, y, w, height] = h.bounds("activity-notes");
        h.runtime.pointer_down(x + w / 2., y + height / 2.);
        h.runtime.editor_select_all();
        h.runtime
            .text_input(
                "A soft-wrapped note keeps its Rust callback after scrolling.",
                &mut h.callbacks,
                &mut h.app,
            )
            .unwrap();
        h.recompose();
        assert!(h.app.notes.starts_with("A soft-wrapped"));
        h.app.job_running = true;
        h.app.job_generation = 9;
        crate::lab::jobs::cancel(&mut h.app);
        crate::lab::jobs::update(
            &mut h.app,
            crate::lab::jobs::Message::Progress {
                generation: 9,
                step: 100,
            },
        );
        assert_eq!(h.app.job_progress, 0);
        assert!(!h.app.job_running);
    }
}

#[test]
fn atelier_explorer_virtual_selection_sort_tree_and_divider_dispatch_rust() {
    for scale in [1., 2.] {
        let mut h = Harness::new(app(Screen::Explorer), 1280., 1000., scale);
        let [x, y, w, _] = h.bounds("explorer-records");
        assert!(
            h.runtime
                .scroll_outcome(
                    x + w / 2.,
                    y + 80. * scale,
                    [0., 3600.],
                    &mut h.callbacks,
                    &mut h.app
                )
                .unwrap()
                .activated
        );
        assert!(h.app.list_window.start >= 98);
        assert!(h.app.list_window.len() <= 96);
        h.recompose();
        h.click("explorer-record-100");
        assert_eq!(h.app.list_selected, Some(100));
        h.reveal("explorer-table-column-0");
        h.click("explorer-table-column-0");
        assert_eq!(h.app.table_direction, SortDirection::Descending);
        h.reveal("explorer-tree-sources-expand");
        h.click("explorer-tree-sources-expand");
        assert!(h.app.tree_expanded.contains(&2));
        h.reveal("explorer-tree-main");
        h.click("explorer-tree-main");
        assert_eq!(h.app.tree_selected, Some(3));
        h.reveal("explorer-split-divider");
        let [x, y, w, height] = h.bounds("explorer-split-divider");
        h.runtime
            .pointer_down_outcome(x + w / 2., y + height / 2., &mut h.callbacks, &mut h.app)
            .unwrap();
        assert!(
            h.runtime
                .split_key(ValueKey::Increase, &mut h.callbacks, &mut h.app)
                .unwrap()
                .activated
        );
        h.recompose();
        assert!(h.app.split_ratio > 0.5);
    }
}

#[test]
fn atelier_glass_never_adds_an_opaque_root_or_fades_text() {
    for background in [
        WindowBackground::Opaque,
        WindowBackground::Transparent { opacity: 0.22 },
        WindowBackground::Blur { opacity: 0.22 },
    ] {
        for screen in [Screen::Activity, Screen::Explorer] {
            let mut app = app(screen);
            app.background = background;
            let mut h = Harness::new(app, 1280., 900., 1.);
            let theme = super::theme(&h.app, false);
            let batches = h.runtime.paint_ui(&mut h.painter, &theme);
            let clear = if background == WindowBackground::Opaque {
                theme.palette.base[0]
            } else {
                [0., 0., 0., 0.]
            };
            let pixels = h.painter.rasterize(&batches, clear).unwrap();
            assert!(
                pixels.pixels().iter().any(|p| p.alpha() == 255),
                "text/controls must not be group-faded"
            );
            if background != WindowBackground::Opaque {
                assert_eq!(
                    pixels.pixels()[(140 * 1280) + 2].alpha(),
                    0,
                    "unpainted backdrop stays transparent"
                );
                assert!(
                    pixels
                        .pixels()
                        .iter()
                        .any(|p| p.alpha() > 0 && p.alpha() < 255),
                    "translucent surfaces"
                );
            }
        }
    }
}

#[test]
fn atelier_components_stretch_with_the_surface_and_menu_keeps_its_corners() {
    for (width, scale) in [(400., 1.), (1280., 2.)] {
        let h = Harness::new(app(Screen::Explorer), width, 1000., scale);
        let list = h.bounds("explorer-records");
        let row = h.bounds("explorer-record-0");
        assert!(
            (row[2] - list[2]).abs() < 1.,
            "virtual rows stretch with their viewport"
        );
        let card = h.bounds("explorer-records-card");
        assert!(
            (list[2] - (card[2] - 44. * scale)).abs() < 1.,
            "list fills padded surface"
        );
        let mut h = Harness::new(app(Screen::Activity), width, 1000., scale);
        let card = h.bounds("activity-work");
        let progress = h.bounds("activity-progress");
        assert!((progress[2] - (card[2] - 44. * scale)).abs() < 1.);
        h.reveal("activity-menu-trigger");
        assert!((h.bounds("activity-menu-trigger")[3] - 40. * scale).abs() < 1.);
    }
}
impl Harness {
    fn new(app: App, width: f32, height: f32, scale: f32) -> Self {
        let vp = Viewport::new(width * scale, height * scale, scale).unwrap();
        let (doc, callbacks) = super::view(&app).build().unwrap().into_parts();
        let runtime = Runtime::new(doc, &callbacks, &Bindings::default()).unwrap();
        let mut this = Self {
            app,
            runtime,
            callbacks,
            painter: RasterPainter::new(vp),
        };
        this.layout();
        this
    }
    fn layout(&mut self) {
        self.runtime
            .layout(
                self.painter.viewport(),
                &Bindings::default(),
                &mut self.painter,
            )
            .unwrap();
    }
    fn recompose(&mut self) {
        let (doc, callbacks) = super::view(&self.app).build().unwrap().into_parts();
        self.runtime
            .replace_document(doc, &callbacks, &Bindings::default())
            .unwrap();
        self.callbacks = callbacks;
        self.layout();
    }
    fn bounds(&self, key: &str) -> [f32; 4] {
        self.runtime
            .boxes()
            .iter()
            .find(|b| b.id == key)
            .unwrap_or_else(|| panic!("missing {key}"))
            .bounds
            .bounds()
    }
    fn click(&mut self, key: &str) {
        let [x, y, w, h] = self.bounds(key);
        self.runtime.pointer_down(x + w / 2., y + h / 2.);
        assert!(
            self.runtime
                .pointer_up_outcome(x + w / 2., y + h / 2., &mut self.callbacks, &mut self.app)
                .unwrap()
                .activated,
            "unreachable {key}"
        );
        self.recompose();
    }
    fn reveal(&mut self, key: &str) {
        for _ in 0..160 {
            if self.runtime.boxes().iter().any(|b| {
                let [x, y, w, h] = b.bounds.bounds();
                b.id == key && b.clip.contains(x + w / 2., y + h / 2.)
            }) {
                return;
            }
            let [x, y, w, _] =
                self.bounds(&{ format!("content-showcase-{}", self.app.showcase.screen.slug()) });
            if !self.runtime.scroll_at(x + w - 2., y + 2., [0., 60.]) {
                break;
            }
            self.layout();
        }
        panic!("could not reveal {key}");
    }
}
