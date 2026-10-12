use super::*;
use volt_ui_kit::{
    declarative::{Bindings, Handlers, Runtime, SelectionKey},
    native::RasterPainter,
    ui::prelude::*,
    Painter, Viewport,
};

struct Harness {
    app: App,
    runtime: Runtime,
    callbacks: Handlers<'static, App>,
    painter: RasterPainter,
}

#[test]
fn reusable_bars_and_tabs_work_in_the_lab_at_compact_and_retina_sizes() {
    use super::state::{ComponentPage, Density};
    fn find<'a>(
        node: &'a volt_ui_kit::declarative::Node,
        key: &str,
    ) -> Option<&'a volt_ui_kit::declarative::Node> {
        if node.id() == key {
            return Some(node);
        }
        node.children().iter().find_map(|child| find(child, key))
    }
    for (width, scale) in [(400., 1.), (1120., 2.)] {
        let mut h = Harness::new(App::default(), width, 900., scale);
        h.reveal("bars-save");
        h.click("bars-save");
        assert_eq!(h.app.saved, 1);
        h.reveal("density-selector");
        let compact = find(h.runtime.document().root(), "density-selector")
            .unwrap()
            .children()[0]
            .id()
            .to_owned();
        h.click(&compact);
        assert_eq!(h.app.density, Density::Compact);
        h.reveal("tab-activity");
        h.click("tab-activity");
        assert_eq!(h.app.component_page, ComponentPage::Activity);
        assert!(find(h.runtime.document().root(), "tabs-overview-add").is_none());
        h.reveal("tabs-activity-record");
        h.click("tabs-activity-record");
        h.reveal("tabs-placement");
        h.click("tabs-placement");
        assert!(h.app.bottom_tabs);
        assert_eq!(
            find(h.runtime.document().root(), "controls-tabs")
                .unwrap()
                .children()[1]
                .id(),
            "controls-tabs-strip"
        );
        h.reveal("tab-activity");
        // Clicking the selected item changes focus but emits no callback.
        let [x, y, w, height] = h.bounds("tab-activity");
        h.runtime.pointer_down(x + w / 2., y + height / 2.);
        assert!(
            !h.runtime
                .pointer_up_outcome(x + w / 2., y + height / 2., &mut h.callbacks, &mut h.app)
                .unwrap()
                .activated
        );
        assert!(
            h.runtime
                .selection_key(SelectionKey::Previous, &mut h.callbacks, &mut h.app)
                .unwrap()
                .activated
        );
        h.recompose();
        assert_eq!(h.app.component_page, ComponentPage::Overview);
        assert!(find(h.runtime.document().root(), "tabs-overview-add").is_some());
        assert!(find(h.runtime.document().root(), "tabs-activity-record").is_none());
        assert_eq!(h.app.saved, 1);
        assert_eq!(h.app.density, Density::Compact);
    }
}
impl Harness {
    fn new(app: App, width: f32, height: f32, scale: f32) -> Self {
        let vp = Viewport::new(width * scale, height * scale, scale).unwrap();
        let (doc, callbacks) = view(&app).build().unwrap().into_parts();
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
        let (doc, callbacks) = view(&self.app).build().unwrap().into_parts();
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
    fn keyboard(&mut self, key: &str) {
        for _ in 0..256 {
            if self.runtime.focused() == Some(key) {
                break;
            }
            self.runtime.focus_next(false);
        }
        assert_eq!(self.runtime.focused(), Some(key));
        self.runtime.key_down();
        assert!(
            self.runtime
                .key_up_outcome(&mut self.callbacks, &mut self.app)
                .unwrap()
                .activated
        );
        self.recompose();
    }
    fn scroll_to_end(&mut self, key: &str) {
        let b = self.runtime.boxes().iter().find(|b| b.id == key).unwrap();
        let [x, y, w, _] = b.bounds.bounds();
        let delta = [
            b.scroll_max[0] - b.scroll_offset[0],
            b.scroll_max[1] - b.scroll_offset[1],
        ];
        self.runtime.scroll_at(x + w - 2., y + 2., delta);
        self.layout();
    }
    fn reveal(&mut self, key: &str) {
        for _ in 0..160 {
            if self.runtime.boxes().iter().any(|b| {
                let [x, y, w, h] = b.bounds.bounds();
                b.id == key && b.clip.contains(x + w / 2., y + h / 2.)
            }) {
                return;
            }
            let [x, y, w, _] = self.bounds(&if self.app.section == Section::Showcase {
                format!("content-showcase-{}", self.app.showcase.screen.slug())
            } else {
                format!("content-{}", self.app.section.slug())
            });
            if !self.runtime.scroll_at(x + w - 2., y + 2., [0., 60.]) {
                break;
            }
            self.layout();
        }
        panic!("could not reveal {key}");
    }
}

#[test]
fn every_section_builds_and_renders_at_small_and_large_sizes() {
    for section in Section::ALL {
        for (width, height, scale) in [(320., 640., 1.), (600., 760., 2.), (1120., 820., 1.)] {
            let mut h = Harness::new(
                App {
                    section,
                    ..App::default()
                },
                width,
                height,
                scale,
            );
            let document = view(&h.app).build().unwrap().into_parts().0;
            fn contains(node: &volt_ui_kit::declarative::Node, key: &str) -> bool {
                node.id() == key || node.children().iter().any(|child| contains(child, key))
            }
            for target in Section::ALL
                .into_iter()
                .filter(|_| section != Section::Showcase)
            {
                // Compact horizontal navigation legitimately culls offscreen boxes;
                // every route must still be registered in the composed document.
                assert!(contains(document.root(), &format!("nav-{}", target.slug())));
            }
            let body = h
                .runtime
                .boxes()
                .iter()
                .find(|b| {
                    b.id == if section == Section::Showcase {
                        format!("content-showcase-{}", h.app.showcase.screen.slug())
                    } else {
                        format!("content-{}", section.slug())
                    }
                })
                .unwrap();
            assert!(body.bounds.bounds()[3] > 100., "body shrank in {section:?}");
            assert!(body
                .scroll_max
                .iter()
                .all(|v| v.is_finite() && *v < 20_000.));
            let style = theme(&h.app, ThemeMode::Dark);
            h.runtime.settle_animations();
            let batches = h.runtime.paint_ui(&mut h.painter, &style);
            assert!(!batches.is_empty());
            h.painter
                .rasterize(&batches, style.palette.base[0])
                .unwrap();
        }
    }
}

#[test]
fn navigation_keeps_state_and_theme_actions_work() {
    let mut h = Harness::new(App::default(), 1120., 820., 1.);
    h.reveal("increment");
    h.click("increment");
    h.reveal("notifications");
    h.click("notifications");
    h.click("nav-effects");
    h.click("save-effects");
    h.click("theme-light");
    h.click("accent-next");
    h.click("nav-controls");
    assert_eq!(h.app.count, 1);
    assert!(h.app.notifications);
    assert_eq!(h.app.saved, 1);
    assert_eq!(h.app.mode, ThemeMode::Light);
    assert_eq!(h.app.accent, 1);
    assert_eq!(h.app.actions, 7);
    assert_eq!(theme(&h.app, ThemeMode::Dark).palette.name, "Lab light");
}

#[test]
fn compact_next_button_can_reach_every_section() {
    let mut h = Harness::new(App::default(), 320., 640., 1.);
    for expected in Section::ALL.into_iter().skip(1).chain([Section::Controls]) {
        h.click("section-next");
        assert_eq!(h.app.section, expected);
    }
}

#[test]
fn keyed_task_callback_survives_reordering() {
    let mut h = Harness::new(App::default(), 1120., 820., 1.);
    // Focus auto-reveal is not implemented; explicitly reveal the list first.
    h.scroll_to_end("content-controls");
    h.keyboard("task-remove-1");
    h.keyboard("tasks-reverse");
    h.keyboard("task-remove-3");
    assert_eq!(h.app.tasks, [2]);
    assert_eq!(h.app.actions, 3);
}

#[test]
fn last_scroll_row_is_reachable_and_scroll_does_not_dispatch() {
    for scale in [1., 2.] {
        let mut h = Harness::new(
            App {
                section: Section::Scrolling,
                ..App::default()
            },
            600.,
            760.,
            scale,
        );
        let b = h
            .runtime
            .boxes()
            .iter()
            .find(|b| b.id == "scroll-documents")
            .unwrap();
        let [x, y, _, _] = b.bounds.bounds();
        let delta = b.scroll_max[1] - b.scroll_offset[1];
        assert!(h
            .runtime
            .scroll_at(x + 20. * scale, y + 20. * scale, [0., delta]));
        h.layout();
        assert_eq!(h.app.actions, 0);
        h.click("document-59");
        assert_eq!(h.app.opened, Some(59));
        assert_eq!(h.app.actions, 1);
    }
}

#[test]
fn lab_cli_validates_its_options_and_forwards_shared_flags() {
    let (app, options) = parse_args(
        [
            "--section",
            "surfaces",
            "--theme",
            "light",
            "--accent",
            "violet",
            "--font",
            "Helvetica",
            "--background",
            "blur",
            "--window-opacity",
            "0.4",
            "--chrome",
            "overlay",
            "--size",
            "600x760",
        ]
        .map(str::to_string),
    )
    .unwrap();
    assert_eq!(app.section, Section::Surfaces);
    assert_eq!(app.mode, ThemeMode::Light);
    assert_eq!(app.accent, 1);
    assert_eq!(app.font, "Helvetica");
    assert_eq!(
        options.background,
        volt_ui_kit::native::WindowBackground::Blur { opacity: 0.4 }
    );
    assert!(options.args.windows(2).any(|v| v == ["--size", "600x760"]));
    for args in [
        vec!["--section", "missing"],
        vec!["--section"],
        vec!["--accent", "bad"],
        vec!["--window-opacity", "NaN"],
        vec!["--window-opacity", "2"],
        vec!["--background", "bad"],
        vec!["--font", ""],
    ] {
        assert!(parse_args(args.into_iter().map(str::to_string)).is_err());
    }
}

#[test]
fn horizontal_and_both_axis_demo_input_work_with_hidden_indicators() {
    let mut h = Harness::new(
        App {
            section: Section::Scrolling,
            ..App::default()
        },
        600.,
        760.,
        1.,
    );
    h.click("hide-scrollbars");
    assert!(h.app.hidden_bars);
    h.reveal("horizontal-scroll");
    let b = h
        .runtime
        .boxes()
        .iter()
        .find(|b| b.id == "horizontal-scroll")
        .unwrap();
    assert_eq!(b.scrollbars, Scrollbars::Hidden);
    let [x, y, _, _] = b.bounds.bounds();
    let delta = b.scroll_max[0];
    assert!(h.runtime.scroll_at(x + 10., y + 10., [delta, 0.]));
    h.layout();
    h.click("horizontal-11");
    assert_eq!(h.app.actions, 2);
    assert!(h.app.last_action().contains("card 12"));
    h.reveal("both-scroll");
    let b = h
        .runtime
        .boxes()
        .iter()
        .find(|b| b.id == "both-scroll")
        .unwrap();
    let [x, y, _, _] = b.bounds.bounds();
    let max = b.scroll_max;
    assert!(max[0] > 0. && max[1] > 0.);
    assert!(h.runtime.scroll_at(x + 10., y + 10., max));
    h.layout();
    assert_eq!(h.runtime.scroll_offset("both-scroll"), max);
    assert_eq!(h.app.actions, 2);
}

#[test]
fn ellipse_clip_rejects_corner_clicks_and_accepts_center() {
    let mut h = Harness::new(
        App {
            section: Section::Surfaces,
            clip: 2,
            ..App::default()
        },
        600.,
        760.,
        2.,
    );
    h.reveal("clipped-action");
    let [x, y, w, height] = h.bounds("clipped-action");
    h.runtime.pointer_down(x + 2., y + 2.);
    assert!(
        !h.runtime
            .pointer_up_outcome(x + 2., y + 2., &mut h.callbacks, &mut h.app)
            .unwrap()
            .activated
    );
    assert_eq!(h.app.actions, 0);
    h.runtime.pointer_down(x + w / 2., y + height / 2.);
    assert!(
        h.runtime
            .pointer_up_outcome(x + w / 2., y + height / 2., &mut h.callbacks, &mut h.app)
            .unwrap()
            .activated
    );
    assert_eq!(h.app.actions, 1);
}

#[test]
fn reset_preserves_appearance_launch_settings_and_bounded_history() {
    let mut app = App {
        section: Section::Window,
        mode: ThemeMode::Light,
        accent: 2,
        count: 42,
        hidden_bars: true,
        background: volt_ui_kit::native::WindowBackground::Transparent { opacity: 0.5 },
        ..App::default()
    };
    for i in 0..50 {
        app.record(format!("Action {i}"));
    }
    app.reset_demos();
    assert_eq!(app.section, Section::Window);
    assert_eq!(app.mode, ThemeMode::Light);
    assert_eq!(app.accent, 2);
    assert_eq!(app.count, 0);
    assert!(!app.hidden_bars);
    assert_eq!(app.history.len(), 20);
    assert_eq!(app.actions, 51);
    assert_eq!(
        app.background,
        volt_ui_kit::native::WindowBackground::Transparent { opacity: 0.5 }
    );
}

#[test]
fn adaptive_panel_geometry_does_not_inflate_or_squash_text_inside_page_scroll() {
    for scale in [1., 2.] {
        for width in [600., 1120.] {
            let h = Harness::new(
                App {
                    section: Section::Layout,
                    ..App::default()
                },
                width,
                900.,
                scale,
            );
            let a = h.bounds("adaptive-first").map(|v| v / scale);
            let b = h.bounds("adaptive-second").map(|v| v / scale);
            let axis = usize::from(width < 768.);
            assert!(
                (b[axis] - a[axis] - a[axis + 2] - 16.).abs() < 0.01,
                "gap: {a:?} {b:?}"
            );
            assert!((a[3] - 100.).abs() < 0.01 && (b[3] - 100.).abs() < 0.01);
            assert!(h.bounds("layout-intro")[3] / scale >= 13.);
        }
    }
}

#[test]
fn showcase_cli_routes_all_three_screens() {
    for screen in ["welcome", "workspace", "studio"] {
        let (app, _) = parse_args(["--section", "showcase", "--screen", screen].map(str::to_owned))
            .expect("showcase screen must be a valid direct launch route");
        let document = view(&app).build().unwrap().into_parts().0;
        assert_eq!(app.section.slug(), "showcase");
        fn has(node: &volt_ui_kit::declarative::Node, key: &str) -> bool {
            node.id() == key || node.children().iter().any(|n| has(n, key))
        }
        assert!(has(document.root(), &format!("showcase-{screen}")));
    }
    assert!(parse_args(["--screen", "unknown"].map(str::to_owned)).is_err());
}

#[test]
fn showcase_callbacks_share_state_across_screens_and_return_to_lab() {
    use super::showcase::Screen;
    let mut h = Harness::new(
        App {
            section: Section::Showcase,
            ..App::default()
        },
        1280.,
        1000.,
        1.,
    );
    h.reveal("welcome-increment");
    h.click("welcome-increment");
    h.reveal("welcome-save");
    h.click("welcome-save");
    h.click("showcase-nav-workspace");
    assert_eq!(h.app.showcase.screen, Screen::Workspace);
    h.reveal("atelier-task-2");
    h.click("atelier-task-2");
    h.reveal("atelier-project-save");
    h.click("atelier-project-save");
    assert!(h.app.showcase.notice);
    assert!(!h.app.showcase.saved);
    h.click("showcase-nav-studio");
    h.reveal("studio-add");
    h.click("studio-add");
    h.reveal("atelier-palette-1");
    h.click("atelier-palette-1");
    h.reveal("atelier-radius-2");
    h.click("atelier-radius-2");
    assert_eq!(h.app.showcase.counter, 2);
    assert_eq!(h.app.showcase.palette, 1);
    assert_eq!(h.app.showcase.radius, 2);
    assert!(h.app.showcase.completed[2]);
    h.click("atelier-theme-Paper");
    assert_eq!(theme(&h.app, ThemeMode::Dark).palette.name, "Atelier paper");
    h.click("section-next");
    assert_eq!(h.app.section, Section::Controls);
    h.reveal("increment");
    h.click("increment");
    assert_eq!(h.app.count, 1);
}

#[test]
fn showcase_screens_reflow_and_use_distinct_scroll_keys() {
    use super::showcase::Screen;
    for screen in Screen::ALL {
        for scale in [1., 2.] {
            for width in [400., 1280.] {
                let mut app = App {
                    section: Section::Showcase,
                    ..App::default()
                };
                app.showcase.screen = screen;
                let mut h = Harness::new(app, width, 1000., scale);
                let (a, b) = match screen {
                    Screen::Welcome => ("welcome-copy", "welcome-art"),
                    Screen::Workspace => ("workspace-main", "workspace-side"),
                    Screen::Studio => ("studio-left", "studio-center"),
                    Screen::Activity => ("activity-work", "activity-actions"),
                    Screen::Explorer => ("explorer-records-card", "explorer-detail"),
                };
                if width >= 1024. {
                    let a = h.bounds(a).map(|v| v / scale);
                    let b = h.bounds(b).map(|v| v / scale);
                    assert!(b[0] >= a[0] + a[2] + 19., "{screen:?}: {a:?}, {b:?}");
                } else {
                    // Offscreen children are legitimately culled at layout time;
                    // use the page's natural extent to prove overflow is scrollable.
                    let body = h
                        .runtime
                        .boxes()
                        .iter()
                        .find(|b| b.id == format!("content-showcase-{}", screen.slug()))
                        .unwrap();
                    assert!(body.scroll_max[1] > 0.);
                }
                let style = theme(&h.app, ThemeMode::Dark);
                let batches = h.runtime.paint_ui(&mut h.painter, &style);
                h.painter
                    .rasterize(&batches, style.palette.base[0])
                    .unwrap();
            }
        }
    }
}

#[test]
fn showcase_hidden_horizontal_gallery_reaches_the_last_card() {
    use super::showcase::Screen;
    let mut app = App {
        section: Section::Showcase,
        ..App::default()
    };
    app.showcase.screen = Screen::Studio;
    let mut h = Harness::new(app, 1280., 1000., 1.);
    h.scroll_to_end("content-showcase-studio");
    h.scroll_to_end("studio-gallery");
    let b = h
        .runtime
        .boxes()
        .iter()
        .find(|b| b.id == "studio-gallery")
        .unwrap();
    assert!(b.scroll_offset[0] > 0.);
    assert_eq!(b.scrollbars, Scrollbars::Hidden);
    assert!(h.runtime.boxes().iter().any(|b| b.id == "studio-gallery-5"));
}

#[test]
fn showcase_reset_keeps_appearance_and_current_screen() {
    use super::showcase::Screen;
    let mut app = App {
        section: Section::Showcase,
        ..App::default()
    };
    app.showcase.screen = Screen::Studio;
    app.showcase.palette = 2;
    app.showcase.counter = 42;
    app.showcase.saved = true;
    app.reset_demos();
    assert_eq!(app.showcase.screen, Screen::Studio);
    assert_eq!(app.showcase.palette, 2);
    assert_eq!(app.showcase.counter, 0);
    assert!(!app.showcase.saved);
    assert!(parse_args(["--palette", "unknown"].map(str::to_owned)).is_err());
}

#[test]
fn showcase_all_radius_presets_build_and_render_at_both_dpi_scales() {
    use super::showcase::Screen;
    for radius in 0..4 {
        for screen in Screen::ALL {
            for scale in [1., 2.] {
                for width in [400., 1280.] {
                    let mut app = App {
                        section: Section::Showcase,
                        ..App::default()
                    };
                    app.showcase.screen = screen;
                    app.showcase.radius = radius;
                    let mut h = Harness::new(app, width, 1000., scale);
                    for appearance in [ThemeMode::Dark, ThemeMode::Light] {
                        let theme = theme(&h.app, appearance);
                        let batches = h.runtime.paint_ui(&mut h.painter, &theme);
                        h.painter
                            .rasterize(&batches, theme.palette.base[0])
                            .unwrap();
                    }
                }
            }
        }
    }
}

#[test]
fn showcase_radius_callbacks_recompose_and_keep_follow_on_controls_live() {
    use super::showcase::Screen;
    for scale in [1., 2.] {
        for width in [400., 1280.] {
            let mut app = App {
                section: Section::Showcase,
                ..App::default()
            };
            app.showcase.screen = Screen::Studio;
            let mut h = Harness::new(app, width, 1000., scale);
            for radius in [0, 1, 2, 3, 2, 3] {
                let key = format!("atelier-radius-{radius}");
                h.reveal(&key);
                h.click(&key);
                assert_eq!(h.app.showcase.radius, radius);
            }
            h.reveal("studio-add");
            let [x, y, w, height] = h.bounds("studio-add");
            h.runtime.pointer_down(x + 1., y + 1.);
            assert!(
                !h.runtime
                    .pointer_up_outcome(x + 1., y + 1., &mut h.callbacks, &mut h.app)
                    .unwrap()
                    .activated,
                "Full rounding must reject its cut-out corner"
            );
            assert!(w > height);
            assert_eq!(h.app.showcase.counter, 0);
            h.click("studio-add");
            assert_eq!(h.app.showcase.counter, 1);
            h.keyboard("studio-add");
            assert_eq!(h.app.showcase.counter, 2);
            h.click("atelier-theme-Paper");
            assert_eq!(h.app.mode, ThemeMode::Light);
            h.click("showcase-nav-workspace");
            assert_eq!(h.app.showcase.screen, Screen::Workspace);
            assert_eq!(h.app.showcase.radius, 3);
        }
    }
}

#[test]
fn all_new_controls_are_live_and_controlled_in_the_single_lab() {
    use super::state::SortOrder;
    use volt_ui_kit::declarative::{EditKey, ValueKey};
    for (width, scale) in [(400., 1.), (1120., 2.)] {
        let mut h = Harness::new(App::default(), width, 900., scale);
        h.reveal("input-draft");
        let [x, y, w, height] = h.bounds("input-draft");
        h.runtime.pointer_down(x + w / 2., y + height / 2.);
        assert!(h.runtime.editing_focus());
        h.runtime.editor_select_all();
        h.runtime
            .text_input("Café 👋", &mut h.callbacks, &mut h.app)
            .unwrap();
        h.recompose();
        assert_eq!(h.app.draft, "Café 👋");
        h.runtime
            .editor_key(EditKey::Undo, false, &mut h.callbacks, &mut h.app)
            .unwrap();
        h.recompose();
        assert_eq!(h.app.draft, "Hello, Volt");
        h.runtime
            .editor_key(EditKey::Redo, false, &mut h.callbacks, &mut h.app)
            .unwrap();
        h.recompose();
        h.runtime
            .editor_key(EditKey::Enter, false, &mut h.callbacks, &mut h.app)
            .unwrap();
        h.recompose();
        assert_eq!(h.app.submitted, 1);
        h.reveal("volume-slider");
        let [x, y, w, height] = h.bounds("volume-slider");
        h.runtime.pointer_down(x + w / 2., y + height / 2.);
        h.runtime
            .pointer_up_outcome(x + w / 2., y + height / 2., &mut h.callbacks, &mut h.app)
            .unwrap();
        h.runtime
            .slider_key(ValueKey::Last, &mut h.callbacks, &mut h.app)
            .unwrap();
        h.recompose();
        assert_eq!(h.app.volume, 100.);
        h.reveal("sort-order-trigger");
        h.click("sort-order-trigger");
        assert!(h.app.sort_open);
        h.click("sort-name");
        assert_eq!(h.app.sort, SortOrder::Name);
        assert!(!h.app.sort_open);
        h.reveal("dialog-open");
        h.click("dialog-open");
        assert!(h.app.dialog_open);
        for _ in 0..8 {
            h.runtime.focus_next(false);
            assert!(matches!(
                h.runtime.focused(),
                Some("dialog-cancel" | "dialog-confirm")
            ));
        }
        h.click("dialog-confirm");
        assert!(!h.app.dialog_open);
        assert_eq!(h.app.submitted, 2);
        assert_eq!(h.runtime.focused(), Some("dialog-open"));
    }
}

#[test]
fn data_lab_virtual_scroll_selection_sort_tree_and_split_are_live() {
    use volt_ui_kit::declarative::ValueKey;
    for scale in [1., 2.] {
        let mut h = Harness::new(
            App {
                section: Section::Data,
                ..App::default()
            },
            1120.,
            900.,
            scale,
        );
        let [x, y, w, _] = h.bounds("data-records");
        let outcome = h
            .runtime
            .scroll_outcome(
                x + w / 2.,
                y + 80. * scale,
                [0., 3200.],
                &mut h.callbacks,
                &mut h.app,
            )
            .unwrap();
        assert!(outcome.activated);
        assert!(h.app.list_window.start >= 98);
        assert!(h.app.list_window.len() <= 96);
        h.recompose();
        h.click("data-record-100");
        assert_eq!(h.app.list_selected, Some(100));
        h.reveal("data-table-column-0");
        h.click("data-table-column-0");
        assert_eq!(h.app.table_direction, SortDirection::Descending);
        h.reveal("data-tree-sources-expand");
        h.click("data-tree-sources-expand");
        assert!(h.app.tree_expanded.contains(&2));
        h.reveal("data-tree-main");
        h.click("data-tree-main");
        assert_eq!(h.app.tree_selected, Some(3));
        h.reveal("data-split-divider");
        let [x, y, w, height] = h.bounds("data-split-divider");
        h.runtime
            .pointer_down_outcome(x + w / 2., y + height / 2., &mut h.callbacks, &mut h.app)
            .unwrap();
        let outcome = h
            .runtime
            .split_key(ValueKey::Increase, &mut h.callbacks, &mut h.app)
            .unwrap();
        assert!(outcome.activated);
        h.recompose();
        assert!(h.app.split_ratio > 0.5);
    }
}
#[test]
fn runtime_lab_menus_popover_wrap_and_cancel_survive_recomposition() {
    for scale in [1., 2.] {
        let mut h = Harness::new(
            App {
                section: Section::Runtime,
                ..App::default()
            },
            1120.,
            900.,
            scale,
        );
        h.click("job-counter");
        assert_eq!(h.app.count, 1);
        h.click("runtime-menu-trigger");
        assert!(h.app.menu_open);
        assert!(
            h.runtime
                .dismiss_overlay(&mut h.callbacks, &mut h.app)
                .unwrap()
                .activated
        );
        h.recompose();
        assert!(!h.app.menu_open);
        h.click("runtime-inspect");
        assert!(h.app.popover_open);
        h.click("runtime-inspector-close");
        assert!(!h.app.popover_open);
        h.reveal("runtime-wrapped-editor");
        let [x, y, w, height] = h.bounds("runtime-wrapped-editor");
        h.runtime.pointer_down(x + w / 2., y + height / 2.);
        h.runtime.editor_select_all();
        h.runtime
            .text_input(
                "Wrapped text retains a real Rust callback even after scrolling.",
                &mut h.callbacks,
                &mut h.app,
            )
            .unwrap();
        h.recompose();
        assert!(h.app.notes.starts_with("Wrapped text"));
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
