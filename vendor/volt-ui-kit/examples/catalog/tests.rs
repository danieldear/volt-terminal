use super::*;
use volt_ui_kit::{
    declarative::{Bindings, Runtime},
    native::RasterPainter,
    Viewport,
};

fn mount(app: &App, size: [f32; 2]) -> Runtime {
    let (document, handlers) = view(app)
        .build()
        .unwrap_or_else(|e| panic!("{:?}: {e}", app.page))
        .into_parts();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    let viewport = Viewport::new(size[0], size[1], 1.).unwrap();
    runtime
        .layout(
            viewport,
            &Bindings::default(),
            &mut RasterPainter::new(viewport),
        )
        .unwrap();
    runtime
}

#[test]
fn desktop_services_cancellation_controls_and_modal_scope_are_guarded() {
    let mut app = App::new();
    app.page = Page::DesktopServices;
    for busy in [false, true] {
        app.services.busy = busy;
        app.services.cancellation_test = false;
        let (document, mut handlers) = view(&app).build().unwrap().into_parts();
        let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
        let viewport = Viewport::new(1440., 920., 1.).unwrap();
        runtime
            .layout(
                viewport,
                &Bindings::default(),
                &mut RasterPainter::new(viewport),
            )
            .unwrap();
        let outcome = runtime
            .accessibility_activate("notification-cancellation-test", &mut handlers, &mut app)
            .unwrap();
        assert_eq!(outcome.activated, !busy);
        assert_eq!(app.services.cancellation_test, !busy);
    }
    app.services.authorization = Some(volt_ui_kit::native::notifications::Authorization::Granted);
    let mut registry = lifecycle::commands();
    for id in ["notification-send", "notification-remove"] {
        assert_eq!(
            registry.dispatch(id, &mut app),
            volt_ui_kit::native::CommandDispatch::Disabled
        );
    }
    assert!(app.services.error.is_none()); // Disabled replay never reaches an OS adapter.
    app.services.busy = false;
    for modal in [false, true, false] {
        app.app_commands.open = modal;
        let mut runtime = mount(&app, [1440., 920.]);
        assert_eq!(
            runtime.accessibility_focus("notification-cancellation-test"),
            !modal
        );
    }
}

#[test]
fn catalog_live_log_follows_new_events_but_preserves_a_readers_scroll_position() {
    for scale in [1., 2.] {
        let mut app = App::new();
        let demo = |app: &App| {
            pages::doc(Page::ScrollArea, app)
                .sections
                .into_iter()
                .find(|s| s.id == "scroll-follow")
                .unwrap()
                .demo
        };
        let (doc, mut handlers) = demo(&app).build().unwrap().into_parts();
        let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
        let viewport = Viewport::new(640. * scale, 500. * scale, scale).unwrap();
        let mut painter = RasterPainter::new(viewport);
        runtime
            .layout(viewport, &Bindings::default(), &mut painter)
            .unwrap();
        assert_eq!(runtime.scroll_offset("follow-log")[1], 224.);
        assert!(
            runtime
                .accessibility_activate("log-append", &mut handlers, &mut app)
                .unwrap()
                .activated
        );
        let (doc, next) = demo(&app).build().unwrap().into_parts();
        runtime
            .replace_document(doc, &next, &Bindings::default())
            .unwrap();
        runtime
            .layout(viewport, &Bindings::default(), &mut painter)
            .unwrap();
        assert_eq!(runtime.scroll_offset("follow-log")[1], 256.);
        assert!(runtime.scroll_at(20. * scale, 20. * scale, [0., -80.]));
        runtime
            .layout(viewport, &Bindings::default(), &mut painter)
            .unwrap();
        let before = runtime.scroll_offset("follow-log");
        app.log_draft = "Reader stays in place".into();
        app.append_log();
        let (doc, next) = demo(&app).build().unwrap().into_parts();
        runtime
            .replace_document(doc, &next, &Bindings::default())
            .unwrap();
        runtime
            .layout(viewport, &Bindings::default(), &mut painter)
            .unwrap();
        assert_eq!(runtime.scroll_offset("follow-log"), before);
    }
}

#[test]
fn every_page_builds_in_both_modes_with_code_and_overlays() {
    let mut app = App::new();
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        app.mode = mode;
        for page in PAGES {
            app.go(page);
            mount(&app, [1440., 920.]);
            mount(&app, [900., 920.]);
            // Every example also renders its source view.
            for section in pages::doc(page, &app).sections {
                app.code.insert(section.id.to_owned());
            }
            mount(&app, [1440., 920.]);
        }
    }
    app.palette.open = true;
    app.dialog_open = true;
    app.fruit_open = true;
    mount(&app, [1440., 920.]);
}

#[test]
fn slugs_are_unique_and_every_theme_is_valid() {
    let mut slugs: Vec<String> = PAGES.iter().map(|p| p.slug()).collect();
    slugs.sort();
    slugs.dedup();
    assert_eq!(slugs.len(), PAGES.len());
    let mut app = App::new();
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        for theme_index in 0..THEMES.len() {
            app.mode = mode;
            app.theme = theme_index;
            assert!(theme(&app, ThemeMode::Light).palette.is_valid());
        }
    }
}

#[test]
fn worker_progress_ignores_stale_generations() {
    let mut app = App::new();
    app.job_running = true;
    update(
        &mut app,
        Message::Progress {
            generation: 0,
            step: 40,
        },
    );
    assert_eq!(app.job_progress, 40);
    app.cancel_job();
    update(
        &mut app,
        Message::Progress {
            generation: 0,
            step: 80,
        },
    );
    assert_eq!(app.job_progress, 40, "cancelled task results are ignored");
}

#[test]
fn completion_retries_full_queue_but_stops_on_cancel_or_close() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let cancel = AtomicBool::new(false);
    let mut calls = 0;
    assert!(send_progress(&cancel, 100, || {
        calls += 1;
        if calls < 3 {
            Err(UiSendError::Full)
        } else {
            Ok(())
        }
    }));
    assert_eq!(calls, 3);
    assert!(send_progress(&cancel, 42, || Err(UiSendError::Full)));
    assert!(!send_progress(&cancel, 100, || Err(UiSendError::Closed)));
    assert!(!send_progress(&cancel, 100, || {
        cancel.store(true, Ordering::Relaxed);
        Err(UiSendError::Full)
    }));
}

#[test]
fn navigation_clears_page_overlays_and_notifications_wrap_safely() {
    let mut app = App::new();
    app.dialog_open = true;
    app.delete_open = true;
    app.palette.open = true;
    app.go(Page::Button);
    assert!(!app.dialog_open && !app.delete_open && !app.palette.open);
    app.next_toast = u32::MAX;
    app.notify("Saved");
    assert_eq!(app.toast, Some((0, "Saved".into())));
}

#[test]
fn launch_options_are_order_independent_and_glass_follows_the_renderer() {
    let args = |items: &[&str]| crate::parse_args(items.iter().map(|s| s.to_string()));
    let (app, options) = args(&["--code", "--page", "button", "--theme", "dark"]).unwrap();
    assert!(app.code.contains(first_section_id(&app).unwrap()));
    // Frosted by default only where the window can actually be translucent.
    assert_eq!(app.glass, cfg!(feature = "native-gpu"));
    assert_eq!(
        options.background,
        if cfg!(feature = "native-gpu") {
            // Root clear owns background opacity; semantic component surfaces stay opaque.
            volt_ui_kit::native::WindowBackground::Blur { opacity: 0.72 }
        } else {
            volt_ui_kit::native::WindowBackground::Opaque
        }
    );
    let (app, options) = args(&["--background", "opaque"]).unwrap();
    assert!(!app.glass);
    assert_eq!(
        options.background,
        volt_ui_kit::native::WindowBackground::Opaque
    );
    assert_eq!(
        options.chrome,
        if cfg!(target_os = "macos") {
            volt_ui_kit::native::WindowChrome::Overlay
        } else {
            volt_ui_kit::native::WindowChrome::Native
        }
    );
    for (flag, expected) in [
        ("--native-chrome", volt_ui_kit::native::WindowChrome::Native),
        (
            "--compact-chrome",
            volt_ui_kit::native::WindowChrome::CompactOverlay,
        ),
    ] {
        assert_eq!(args(&[flag]).unwrap().1.chrome, expected);
    }
    let (app, options) = args(&["--background", "blur", "--window-opacity", "0.82"]).unwrap();
    assert!(app.glass);
    assert_eq!(app.opacity, 82);
    assert_eq!(app.tint, 0);
    assert_eq!(
        options.background,
        volt_ui_kit::native::WindowBackground::Blur { opacity: 0.82 }
    );
    assert_eq!(
        args(&["--window-opacity", "0"]).unwrap().0.opacity,
        OPACITY_MIN
    );
    for bad in [
        vec!["--theme", "oops"],
        vec!["--background", "oops"],
        vec!["--window-opacity", "NaN"],
        vec!["--window-opacity", "1.1"],
    ] {
        assert!(args(&bad).is_err());
    }
}

#[test]
fn catalog_named_controls_callbacks_and_scroll_reveal_work_after_recomposition() {
    let mut app = App::new();
    app.go(Page::Accessibility);
    let (doc, mut handlers) = view(&app).build().unwrap().into_parts();
    let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
    let viewport = Viewport::new(1440., 920., 1.).unwrap();
    runtime
        .layout(
            viewport,
            &Bindings::default(),
            &mut RasterPainter::new(viewport),
        )
        .unwrap();
    let nodes = runtime.accessibility_nodes();
    assert!(nodes
        .iter()
        .any(|n| n.id == "ax-name" && n.label == "Profile name"));
    assert!(nodes
        .iter()
        .any(|n| n.id == "ax-notifications" && n.label == "Show notifications"));
    assert!(runtime.accessibility_focus("ax-action-11"));
    assert!(runtime.scroll_offset("ax-scroll")[1] > 0.);
    runtime
        .accessibility_activate("ax-action-11", &mut handlers, &mut app)
        .unwrap();
    assert!(app.toast.as_ref().unwrap().1.contains("Action 12"));
    let (doc, next) = view(&app).build().unwrap().into_parts();
    runtime
        .replace_document(doc, &next, &Bindings::default())
        .unwrap();
    runtime
        .layout(
            viewport,
            &Bindings::default(),
            &mut RasterPainter::new(viewport),
        )
        .unwrap();
    assert_eq!(runtime.focused(), Some("ax-action-11"));
    assert!(runtime.scroll_offset("ax-scroll")[1] > 0.);
    assert!(runtime
        .accessibility_nodes()
        .iter()
        .any(|n| n.id == "ax-action-11" && n.focused));
}

#[test]
fn catalog_incremental_frames_match_full_paint_after_scroll_and_reveal() {
    let mut app = App::new();
    app.go(Page::Accessibility);
    let viewport = Viewport::new(1800., 1840., 2.).unwrap();
    let (doc, handlers) = view(&app).build().unwrap().into_parts();
    let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
    let mut painter = RasterPainter::new(viewport);
    runtime
        .layout(viewport, &Bindings::default(), &mut painter)
        .unwrap();
    runtime.settle_animations();
    let theme = theme(&app, ThemeMode::Light);
    let clear = theme.palette.base[0];
    let mut check = |runtime: &mut Runtime| {
        runtime
            .layout(viewport, &Bindings::default(), &mut painter)
            .unwrap();
        let batches = runtime.paint_ui(&mut painter, &theme);
        let (incremental, _) = painter.rasterize_incremental(&batches, clear).unwrap();
        let actual = incremental.data().to_vec();
        let full = painter.rasterize(&batches, clear).unwrap();
        assert_eq!(actual, full.data(), "incremental Catalog frame differs");
    };
    check(&mut runtime);
    assert!(runtime.accessibility_focus("ax-action-11"));
    check(&mut runtime);
    runtime.pointer_move(10., 10.);
    check(&mut runtime);
    runtime.scroll_at(1200., 600., [0., 90.]);
    check(&mut runtime);
}

#[cfg(feature = "native-gpu")]
#[test]
#[ignore = "requires a real GPU; includes blocking readback, not native frame timing"]
fn catalog_pages_and_glass_remain_on_gpu_at_both_scales() {
    use volt_ui_kit::native::GpuOffscreenRenderer;
    let mut gpu = GpuOffscreenRenderer::new().expect("GPU required");
    let mut app = App::new();
    app.mode = ThemeMode::Dark;
    for scale in [1., 2.] {
        let viewport = Viewport::new(1440. * scale, 920. * scale, scale).unwrap();
        let mut painter = RasterPainter::new(viewport);
        for page in PAGES {
            app.go(page);
            let (doc, handlers) = view(&app).build().unwrap().into_parts();
            let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
            runtime
                .layout(viewport, &Bindings::default(), &mut painter)
                .unwrap();
            runtime.settle_animations();
            let ui = theme(&app, ThemeMode::Dark);
            let batches = runtime.paint_ui(&mut painter, &ui);
            gpu.render(&mut painter, &batches, ui.palette.base[0])
                .unwrap();
            assert_eq!(
                gpu.last_over_budget(),
                None,
                "{page:?} at {scale}x fell back"
            );
        }
        app.go(Page::Theming);
        app.glass = true;
        let (doc, handlers) = view(&app).build().unwrap().into_parts();
        let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
        runtime
            .layout(viewport, &Bindings::default(), &mut painter)
            .unwrap();
        runtime.settle_animations();
        let ui = theme(&app, ThemeMode::Dark);
        let batches = runtime.paint_ui(&mut painter, &ui);
        // As the native host does: the theme's root background at the window
        // opacity (72% by default) is the clear; the view paints no overlay.
        assert_eq!(app.opacity, 72);
        let mut clear = ui.legacy_theme().background;
        clear[3] = app.opacity as f32 / 100.;
        let pixels = gpu.render(&mut painter, &batches, clear).unwrap();
        // The sidebar shows the translucent material; the content sheet is solid.
        assert!((182..=185).contains(&pixels.pixel(0, 0).unwrap().alpha()));
        let content = pixels
            .pixel((1000. * scale) as u32, (600. * scale) as u32)
            .unwrap();
        assert_eq!(content.alpha(), 255);
        assert_eq!(gpu.last_over_budget(), None);
        app.glass = false;
    }
}

#[test]
fn data_view_first_last_focus_and_selection_never_overpaint_the_frame() {
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        for scale in [1., 2.] {
            for (page, frame_id, targets) in [
                (Page::Table, "invoices", ["inv-0", "inv-5"]),
                (
                    Page::Table,
                    "invoices-square",
                    ["inv-square-0", "inv-square-5"],
                ),
                (Page::TreeView, "files", ["t-src", "t-readme"]),
            ] {
                let viewport = Viewport::new(1080. * scale, 1600. * scale, scale).unwrap();
                let render = |target: Option<(usize, &str)>| {
                    let mut app = App::new();
                    app.mode = mode;
                    app.go(page);
                    app.row = None;
                    app.file = None;
                    if let Some((index, _)) = target {
                        if page == Page::Table {
                            app.row = Some(if index == 0 { 0 } else { 5 });
                        } else {
                            app.file = Some(if index == 0 { 1 } else { 7 });
                        }
                    }
                    let (doc, handlers) = view(&app).build().unwrap().into_parts();
                    let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
                    let mut painter = RasterPainter::new(viewport);
                    runtime
                        .layout(viewport, &Bindings::default(), &mut painter)
                        .unwrap();
                    if let Some((_, id)) = target {
                        let row = runtime.boxes().iter().find(|b| b.id == id).unwrap();
                        assert!(
                            row.bounds
                                .bounds()
                                .into_iter()
                                .zip(row.clip.bounds())
                                .all(|(a, b)| (a - b).abs() < 0.01),
                            "{id} was truncated"
                        );
                        assert!(runtime.accessibility_focus(id));
                        runtime
                            .layout(viewport, &Bindings::default(), &mut painter)
                            .unwrap();
                    }
                    let bounds = runtime
                        .boxes()
                        .iter()
                        .find(|b| b.id == frame_id)
                        .unwrap()
                        .bounds
                        .bounds();
                    let theme = theme(&app, mode);
                    runtime.settle_animations();
                    let batches = runtime.paint_ui(&mut painter, &theme);
                    let pixels = painter
                        .rasterize(&batches, theme.palette.base[0])
                        .unwrap()
                        .data()
                        .to_vec();
                    // Compare pixels wholly outside the content inset. Fractional edge
                    // pixels straddle border/content and legitimately blend both colors.
                    let [x, y, w, h] = bounds.map(|v| v.round() as usize);
                    let band = (if page == Page::Table {
                        scale
                    } else {
                        2. * scale
                    }) as usize;
                    let stride = viewport.width as usize;
                    let mut border = Vec::new();
                    for py in y..y + h {
                        for px in x..x + w {
                            if px < x + band
                                || px >= x + w - band
                                || py < y + band
                                || py >= y + h - band
                            {
                                if page == Page::Table
                                    && !(px as f32 + 1. <= bounds[0] + scale
                                        || px as f32 >= bounds[0] + bounds[2] - scale
                                        || py as f32 + 1. <= bounds[1] + scale
                                        || py as f32 >= bounds[1] + bounds[3] - scale)
                                {
                                    continue;
                                }
                                border.extend_from_slice(
                                    &pixels[4 * (py * stride + px)..4 * (py * stride + px) + 4],
                                );
                            }
                        }
                    }
                    (bounds, border)
                };
                let before = render(None);
                for (index, id) in targets.iter().enumerate() {
                    let after = render(Some((index, id)));
                    assert_eq!(before.0, after.0, "frame geometry changed for {id}");
                    let changed = before
                        .1
                        .iter()
                        .zip(&after.1)
                        .filter(|(a, b)| a != b)
                        .count();
                    assert_eq!(
                        changed, 0,
                        "{page:?}, {mode:?}, {scale}x, {id}: {changed} changed frame channels"
                    );
                }
            }
        }
    }
}

#[cfg(feature = "native-gpu")]
#[test]
#[ignore = "requires real GPU readback; not presentation timing"]
fn data_view_frame_borders_survive_gpu_selection_and_focus() {
    use volt_ui_kit::native::GpuOffscreenRenderer;
    let mut gpu = GpuOffscreenRenderer::new().expect("GPU required");
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        for scale in [1., 2.] {
            let viewport = Viewport::new(1080. * scale, 1600. * scale, scale).unwrap();
            for (page, frame_id, ids) in [
                (Page::Table, "invoices", ["inv-0", "inv-5"]),
                (
                    Page::Table,
                    "invoices-square",
                    ["inv-square-0", "inv-square-5"],
                ),
                (Page::TreeView, "files", ["t-src", "t-readme"]),
            ] {
                let mut before: Option<Vec<tiny_skia::PremultipliedColorU8>> = None;
                for state in [None, Some(0), Some(1)] {
                    let mut app = App::new();
                    app.mode = mode;
                    app.go(page);
                    app.row = state
                        .filter(|_| page == Page::Table)
                        .map(|i| if i == 0 { 0 } else { 5 });
                    app.file =
                        state
                            .filter(|_| page == Page::TreeView)
                            .map(|i| if i == 0 { 1 } else { 7 });
                    let (doc, handlers) = view(&app).build().unwrap().into_parts();
                    let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
                    let mut painter = RasterPainter::new(viewport);
                    runtime
                        .layout(viewport, &Bindings::default(), &mut painter)
                        .unwrap();
                    if let Some(i) = state {
                        assert!(runtime.accessibility_focus(ids[i]));
                        runtime
                            .layout(viewport, &Bindings::default(), &mut painter)
                            .unwrap();
                    }
                    let bounds = runtime
                        .boxes()
                        .iter()
                        .find(|b| b.id == frame_id)
                        .unwrap()
                        .bounds
                        .bounds();
                    let [x, y, w, h] = bounds.map(|v| v.round() as u32);
                    let ui = theme(&app, mode);
                    runtime.settle_animations();
                    let batches = runtime.paint_ui(&mut painter, &ui);
                    let pixels = gpu
                        .render(&mut painter, &batches, ui.palette.base[0])
                        .unwrap();
                    let mut border = Vec::new();
                    let band = (if page == Page::Table {
                        scale
                    } else {
                        2. * scale
                    }) as u32;
                    for py in y..y + h {
                        for px in x..x + w {
                            if px < x + band
                                || px >= x + w - band
                                || py < y + band
                                || py >= y + h - band
                            {
                                if page == Page::Table
                                    && !(px as f32 + 1. <= bounds[0] + scale
                                        || px as f32 >= bounds[0] + bounds[2] - scale
                                        || py as f32 + 1. <= bounds[1] + scale
                                        || py as f32 >= bounds[1] + bounds[3] - scale)
                                {
                                    continue;
                                }
                                border.push(pixels.pixel(px, py).unwrap());
                            }
                        }
                    }
                    assert_eq!(gpu.last_over_budget(), None);
                    if let Some(before) = &before {
                        let max_delta = before
                            .iter()
                            .zip(&border)
                            .map(|(a, b)| {
                                [
                                    a.red().abs_diff(b.red()),
                                    a.green().abs_diff(b.green()),
                                    a.blue().abs_diff(b.blue()),
                                    a.alpha().abs_diff(b.alpha()),
                                ]
                                .into_iter()
                                .max()
                                .unwrap()
                            })
                            .max()
                            .unwrap();
                        // Vello's antialiased edges can differ by one readback LSB as
                        // scene binning changes. Larger changes indicate overpainting.
                        assert!(
                            max_delta <= 1,
                            "{page:?}, {mode:?}, {scale}x, {state:?}: frame delta {max_delta}"
                        );
                    } else {
                        before = Some(border);
                    }
                }
            }
        }
    }
}

#[test]
fn rounded_and_square_catalog_tables_share_working_sort_and_selection() {
    use volt_ui_kit::declarative::AccessRole;
    let mut app = App::new();
    app.go(Page::Table);
    for scale in [1., 2.] {
        let viewport = Viewport::new(1080. * scale, 1600. * scale, scale).unwrap();
        let (doc, mut handlers) = view(&app).build().unwrap().into_parts();
        let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
        let mut painter = RasterPainter::new(viewport);
        for (frame, row_id, row_value) in [
            ("invoices", "inv-0", 0),
            ("invoices-square", "inv-square-5", 5),
        ] {
            runtime
                .layout(viewport, &Bindings::default(), &mut painter)
                .unwrap();
            runtime
                .accessibility_activate(row_id, &mut handlers, &mut app)
                .unwrap();
            assert_eq!(app.row, Some(row_value));
            let header = format!("{frame}-column-3");
            runtime
                .accessibility_activate(&header, &mut handlers, &mut app)
                .unwrap();
            assert_eq!(app.sort.as_ref().unwrap().0, 3);
            let (doc, next) = view(&app).build().unwrap().into_parts();
            runtime
                .replace_document(doc, &next, &Bindings::default())
                .unwrap();
            handlers = next;
            runtime
                .layout(viewport, &Bindings::default(), &mut painter)
                .unwrap();
            let nodes = runtime.accessibility_nodes();
            for id in ["invoices", "invoices-square"] {
                let table = nodes.iter().position(|n| n.id == id).unwrap();
                assert_eq!(nodes[table].role, AccessRole::Table);
                let selected = nodes
                    .iter()
                    .filter(|n| n.parent == Some(table) && n.selected == Some(true))
                    .collect::<Vec<_>>();
                assert_eq!(selected.len(), 1);
                assert!(selected[0].label.contains(if row_value == 0 {
                    "INV001"
                } else {
                    "INV006"
                }));
                let sort_header = nodes
                    .iter()
                    .find(|n| n.id == format!("{id}-column-3"))
                    .unwrap();
                assert_eq!(
                    sort_header.table.unwrap().sort_ascending,
                    Some(app.sort.as_ref().unwrap().1 == SortDirection::Ascending)
                );
            }
        }
    }
}

#[test]
fn catalog_rounded_table_clips_header_and_last_focus_without_changing_cells() {
    use volt_ui_kit::ui::prelude::{vstack, Edge};
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        for scale in [1., 2.] {
            for square in [false, true] {
                let mut app = App::new();
                app.go(Page::Table);
                app.mode = mode;
                let viewport = Viewport::new(800. * scale, 600. * scale, scale).unwrap();
                let demo = pages::doc(Page::Table, &app)
                    .sections
                    .into_iter()
                    .nth(usize::from(square))
                    .unwrap()
                    .demo;
                let root: volt_ui_kit::ui::Element<App> =
                    vstack![demo].padding(24., Edge::All).into();
                let (doc, mut handlers) = root.build().unwrap().into_parts();
                let mut r = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
                let mut p = RasterPainter::new(viewport);
                r.layout(viewport, &Bindings::default(), &mut p).unwrap();
                let key = if square {
                    "invoices-square"
                } else {
                    "invoices"
                };
                let row_key = if square { "inv-square-5" } else { "inv-5" };
                let frame = r
                    .boxes()
                    .iter()
                    .find(|b| b.id == key)
                    .unwrap()
                    .bounds
                    .bounds();
                let row = r
                    .boxes()
                    .iter()
                    .find(|b| b.id == row_key)
                    .unwrap()
                    .bounds
                    .bounds();
                // Real mouse, not accessibility activation. Cell padding is unchanged.
                let (x, y) = (row[0] + 20. * scale, row[1] + 22. * scale);
                r.pointer_down_outcome(x, y, &mut handlers, &mut app)
                    .unwrap();
                assert!(
                    r.pointer_up_outcome(x, y, &mut handlers, &mut app)
                        .unwrap()
                        .activated
                );
                assert_eq!(app.row, Some(5));
                let demo = pages::doc(Page::Table, &app)
                    .sections
                    .into_iter()
                    .nth(usize::from(square))
                    .unwrap()
                    .demo;
                let root: volt_ui_kit::ui::Element<App> =
                    vstack![demo].padding(24., Edge::All).into();
                let (doc, next) = root.build().unwrap().into_parts();
                r.replace_document(doc, &next, &Bindings::default())
                    .unwrap();
                handlers = next;
                r.layout(viewport, &Bindings::default(), &mut p).unwrap();
                r.settle_animations();
                let ui = theme(&app, mode);
                let batches = r.paint_ui(&mut p, &ui);
                let pixels = p.rasterize(&batches, ui.palette.base[0]).unwrap();
                let pixel = |x: f32, y: f32| {
                    pixels.pixels()[(y as usize) * viewport.width as usize + x as usize]
                };
                let corners = [
                    pixel(frame[0] + 2. * scale, frame[1] + 2. * scale),
                    pixel(frame[0] + 2. * scale, frame[1] + frame[3] - 3. * scale),
                ];
                let outside = pixel(0., 0.);
                if !square {
                    assert!(
                        corners.iter().all(|c| *c == outside),
                        "{mode:?}, {scale}x: rounded header/last focus escaped silhouette"
                    );
                } else {
                    assert!(
                        corners.iter().all(|c| *c != outside),
                        "square variant must remain square"
                    );
                }
                let header = format!("{key}-column-0");
                let b = r
                    .boxes()
                    .iter()
                    .find(|b| b.id == header)
                    .unwrap()
                    .bounds
                    .bounds();
                let (x, y) = (b[0] + 20. * scale, b[1] + 20. * scale);
                r.pointer_down_outcome(x, y, &mut handlers, &mut app)
                    .unwrap();
                assert!(
                    r.pointer_up_outcome(x, y, &mut handlers, &mut app)
                        .unwrap()
                        .activated
                );
                assert!(app.sort.is_some());
            }
        }
    }
}

#[test]
fn application_draft_cancel_save_discard_and_palette_preserve_the_data_contract() {
    use volt_ui_kit::native::{CloseDecision, CloseReason, CommandDispatch};
    let mut app = App::new();
    app.go(Page::ApplicationLifecycle);
    let original = app.saved_document.clone();
    assert!(!lifecycle::dirty(&app));
    app.document_text.push_str("\nEdited");
    assert!(lifecycle::dirty(&app));
    assert_eq!(
        lifecycle::close_request(&mut app, CloseReason::Window),
        CloseDecision::KeepOpen
    );
    let before = app.document_text.clone();
    let (doc, mut handlers) = view(&app).build().unwrap().into_parts();
    let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
    let viewport = Viewport::new(1440., 920., 1.).unwrap();
    runtime
        .layout(
            viewport,
            &Bindings::default(),
            &mut RasterPainter::new(viewport),
        )
        .unwrap();
    assert!(
        runtime
            .accessibility_activate("application-close-cancel", &mut handlers, &mut app)
            .unwrap()
            .activated
    );
    assert_eq!(app.document_text, before);
    assert_eq!(app.saved_document, original);
    assert_eq!(app.close_prompt, None);
    let mut registry = lifecycle::commands();
    assert_eq!(
        registry.dispatch("save-draft", &mut app),
        CommandDispatch::Handled(None)
    );
    assert!(!lifecycle::dirty(&app));
    assert_eq!(
        lifecycle::close_request(&mut app, CloseReason::Window),
        CloseDecision::Close
    );
    app.document_text.push_str("\nTemporary");
    assert_eq!(
        lifecycle::close_request(&mut app, CloseReason::Quit),
        CloseDecision::KeepOpen
    );
    let (doc, next) = view(&app).build().unwrap().into_parts();
    runtime
        .replace_document(doc, &next, &Bindings::default())
        .unwrap();
    handlers = next;
    runtime
        .layout(
            viewport,
            &Bindings::default(),
            &mut RasterPainter::new(viewport),
        )
        .unwrap();
    assert!(
        runtime
            .accessibility_activate("application-close-discard", &mut handlers, &mut app)
            .unwrap()
            .activated
    );
    assert_eq!(app.document_text, app.saved_document);
    assert_eq!(app.close_prompt, None);
    // No request sender in this headless fixture: it must not pretend an OS window closed.
    assert!(app.toast.as_ref().unwrap().1.contains("native Catalog"));
    registry.dispatch("command-palette", &mut app);
    assert!(app.app_commands.open);
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        app.mode = mode;
        mount(&app, [900., 920.]);
    }
}

#[test]
fn opacity_tint_and_blur_source_are_independent_and_live() {
    use volt_ui_kit::native::WindowBackground;
    let mut app = App::new();
    app.glass = true;
    app.compositor_blur = true;
    let (doc, mut handlers) = view(&app).build().unwrap().into_parts();
    let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
    let viewport = Viewport::new(1440., 920., 1.).unwrap();
    let mut painter = RasterPainter::new(viewport);
    runtime
        .layout(viewport, &Bindings::default(), &mut painter)
        .unwrap();
    for opacity in [10., 72., 100., 10.] {
        runtime
            .accessibility_set_value(
                "background-opacity",
                volt_ui_kit::declarative::AccessValue::Number(opacity),
                &mut handlers,
                &mut app,
            )
            .unwrap();
        assert_eq!(app.opacity, opacity as u8);
        assert_eq!(app.tint, 0);
        assert_eq!(
            window_background(&app),
            WindowBackground::CompositorBlur {
                opacity: opacity as f32 / 100.
            }
        );
        let (doc, next) = view(&app).build().unwrap().into_parts();
        runtime
            .replace_document(doc, &next, &Bindings::default())
            .unwrap();
        handlers = next;
        runtime
            .layout(viewport, &Bindings::default(), &mut painter)
            .unwrap();
        runtime.settle_animations();
        let ui = theme(&app, ThemeMode::Light);
        assert_eq!(ui.palette.base[0][3], 1.);
        let mut clear = ui.legacy_theme().background;
        clear[3] = opacity as f32 / 100.;
        let batches = runtime.paint_ui(&mut painter, &ui);
        let pixels = painter.rasterize(&batches, clear).unwrap();
        assert!(
            pixels
                .pixel(0, 0)
                .unwrap()
                .alpha()
                .abs_diff((opacity * 2.55).round() as u8)
                <= 1
        );
        assert_eq!(
            pixels.pixel(1000, 500).unwrap().alpha(),
            255,
            "content must not fade"
        );
    }
    runtime
        .accessibility_set_value(
            "sidebar-tint",
            volt_ui_kit::declarative::AccessValue::Number(30.),
            &mut handlers,
            &mut app,
        )
        .unwrap();
    assert_eq!(app.tint, 30);
    assert_eq!(app.opacity, 10);
    let untinted = {
        let mut state = App::new();
        state.glass = true;
        theme(&state, ThemeMode::Light)
    };
    let tinted = theme(&app, ThemeMode::Light);
    assert_eq!(tinted.palette, untinted.palette);
    assert_ne!(
        tinted.legacy_theme().background,
        untinted.legacy_theme().background
    );
    app.blur = false;
    assert_eq!(
        window_background(&app),
        WindowBackground::Transparent { opacity: 0.1 }
    );
    app.blur = true;
    app.compositor_blur = false;
    assert_eq!(
        window_background(&app),
        WindowBackground::Blur { opacity: 0.1 }
    );
}

#[test]
fn compositor_launch_is_explicit_and_defaults_remain_public() {
    use volt_ui_kit::native::WindowBackground;
    let (app, options) = crate::parse_args(
        [
            "--background",
            "compositor",
            "--window-opacity",
            "0.3",
            "--tint",
            "12",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(
        options.background,
        WindowBackground::CompositorBlur { opacity: 0.3 }
    );
    assert_eq!((app.opacity, app.tint), (30, 12));
    assert!(app.compositor_blur && app.blur);
    let (app, _) = crate::parse_args(Vec::<String>::new()).unwrap();
    assert!(!app.compositor_blur);
    assert!(crate::parse_args(["--tint", "101"].map(str::to_owned)).is_err());
}

#[test]
fn scrolling_pages_by_shifting_matches_a_full_layout() {
    for page in [Page::Introduction, Page::Table, Page::Settings] {
        let mut app = App::new();
        app.go(page);
        app.glass = true;
        let (doc, handlers) = view(&app).build().unwrap().into_parts();
        let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
        let viewport = Viewport::new(2880., 1840., 2.).unwrap();
        let mut painter = RasterPainter::new(viewport);
        runtime
            .layout(viewport, &Bindings::default(), &mut painter)
            .unwrap();
        runtime.settle_animations();
        let summary = |r: &Runtime| -> Vec<String> {
            r.boxes()
                .iter()
                .map(|b| {
                    let round = |v: [f32; 4]| v.map(|x| (x * 100.).round() / 100.);
                    format!(
                        "{} {:?} {:?}",
                        b.id,
                        round(b.bounds.bounds()),
                        round(b.clip.bounds())
                    )
                })
                .collect()
        };
        for delta in [120., 480., 2000., -900., 33.] {
            runtime.scroll_at(1800., 1200., [0., delta]);
            // Fast path: no layout pass is needed before painting.
            assert!(!runtime
                .layout(viewport, &Bindings::default(), &mut painter)
                .unwrap());
            let shifted = summary(&runtime);
            runtime.invalidate_layout();
            runtime
                .layout(viewport, &Bindings::default(), &mut painter)
                .unwrap();
            assert_eq!(shifted, summary(&runtime), "{page:?} after {delta}");
        }
    }
}

#[test]
fn layout_adapts_to_window_width() {
    let ids = |runtime: &Runtime| -> Vec<String> {
        runtime.boxes().iter().map(|b| b.id.clone()).collect()
    };
    let has = |runtime: &Runtime, id: &str| ids(runtime).iter().any(|b| b == id);
    let mut app = App::new();
    // Wide: sidebar and full search; no menu button.
    let wide = mount(&app, [1440., 900.]);
    assert!(has(&wide, "side-button") && has(&wide, "open-palette"));
    assert!(!has(&wide, "open-nav") && !has(&wide, "open-palette-compact"));
    assert!(has(&wide, "hero-full") && !has(&wide, "hero-compact"));
    // Narrow: sidebar collapses behind a menu button, search becomes an icon.
    let narrow = mount(&app, [720., 900.]);
    assert!(!has(&narrow, "side-button") && !has(&narrow, "open-palette"));
    assert!(has(&narrow, "open-nav") && has(&narrow, "open-palette-compact"));
    assert!(has(&narrow, "hero-compact") && !has(&narrow, "hero-full"));
    // The drawer lists the same pages and navigation closes it.
    app.nav_open = true;
    let drawer = mount(&app, [720., 900.]);
    assert!(has(&drawer, "drawer-button"));
    app.go(Page::Button);
    assert!(!app.nav_open);
    // Very wide: the doc column stops growing and centres.
    let huge = mount(&app, [2560., 1300.]);
    let column = huge
        .boxes()
        .iter()
        .find(|b| b.id == "page-column")
        .unwrap()
        .bounds
        .bounds();
    assert_eq!(column[2], 1040.);
}

#[test]
fn dragging_the_divider_resizes_the_sidebar_in_pixels() {
    let mut app = App::new();
    app.go(Page::Button);
    let (doc, mut handlers) = view(&app).build().unwrap().into_parts();
    let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
    let layout = |runtime: &mut Runtime, width: f32| {
        let viewport = Viewport::new(width, 800., 1.).unwrap();
        runtime
            .layout(
                viewport,
                &Bindings::default(),
                &mut RasterPainter::new(viewport),
            )
            .unwrap();
    };
    layout(&mut runtime, 1300.);
    let bounds = |runtime: &Runtime, id: &str| {
        runtime
            .boxes()
            .iter()
            .find(|b| b.id == id)
            .unwrap()
            .bounds
            .bounds()
    };
    assert_eq!(bounds(&runtime, "sidebar-column")[2], 248.);
    let [x, y, w, h] = bounds(&runtime, "body-split-divider");
    let (cx, cy) = (x + w * 0.5, y + h * 0.5);
    runtime
        .pointer_down_outcome(cx, cy, &mut handlers, &mut app)
        .unwrap();
    runtime
        .pointer_move_outcome(cx + 60., cy, &mut handlers, &mut app)
        .unwrap();
    runtime
        .pointer_up_outcome(cx + 60., cy, &mut handlers, &mut app)
        .unwrap();
    assert_eq!(app.sidebar_width, 308.);
    // Past the maximum it clamps (recompose first, as the host does).
    let (doc, next) = view(&app).build().unwrap().into_parts();
    runtime
        .replace_document(doc, &next, &Bindings::default())
        .unwrap();
    handlers = next;
    layout(&mut runtime, 1300.);
    let [x, y, w, h] = bounds(&runtime, "body-split-divider");
    let (cx, cy) = (x + w * 0.5, y + h * 0.5);
    runtime
        .pointer_down_outcome(cx, cy, &mut handlers, &mut app)
        .unwrap();
    runtime
        .pointer_move_outcome(cx + 600., cy, &mut handlers, &mut app)
        .unwrap();
    assert_eq!(app.sidebar_width, 420.);
    // Recomposed, the width holds as the window resizes.
    let (doc, next) = view(&app).build().unwrap().into_parts();
    runtime
        .replace_document(doc, &next, &Bindings::default())
        .unwrap();
    layout(&mut runtime, 1600.);
    assert_eq!(bounds(&runtime, "sidebar-column")[2], 420.);
}

#[test]
#[ignore = "timing probe"]
fn divider_drag_cost_per_page() {
    use std::time::Instant;
    let viewport = Viewport::new(2880., 1840., 2.).unwrap();
    let mut rows = vec![];
    for page in PAGES {
        let mut app = App::new();
        app.go(page);
        let mut painter = RasterPainter::new(viewport);
        let (doc, mut handlers) = view(&app).build().unwrap().into_parts();
        let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
        runtime
            .layout(viewport, &Bindings::default(), &mut painter)
            .unwrap();
        let d = runtime
            .boxes()
            .iter()
            .find(|b| b.id == "body-split-divider")
            .unwrap()
            .bounds
            .bounds();
        let (cx, cy) = (d[0] + d[2] * 0.5, d[1] + d[3] * 0.5);
        runtime
            .pointer_down_outcome(cx, cy, &mut handlers, &mut app)
            .unwrap();
        let t = Instant::now();
        for i in 1..=20 {
            runtime
                .pointer_move_outcome(cx + i as f32 * 4., cy, &mut handlers, &mut app)
                .unwrap();
            runtime
                .layout(viewport, &Bindings::default(), &mut painter)
                .unwrap();
        }
        rows.push((t.elapsed().as_secs_f64() * 1000. / 20., page.title()));
    }
    rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    for (ms, title) in rows.iter().take(12) {
        eprintln!("PAGE {ms:7.2}ms {title}");
    }
}
