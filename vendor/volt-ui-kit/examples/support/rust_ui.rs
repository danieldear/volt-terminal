use std::{path::PathBuf, time::Instant};
use volt_ui_kit::{
    declarative::{Bindings, Kind, PaintBatch, Runtime},
    native::{
        run_ui_with_messages_and_theme, run_ui_with_options, run_ui_with_theme, NativeOptions,
        RasterPainter, SurfaceOptions, UiSender, WindowBackground, WindowChrome,
    },
    ui::Element,
    Painter, ThemeMode, UiTheme, Viewport,
};

/// Rust examples share the native host. Source edits require a Rust rebuild.
#[allow(dead_code)]
pub fn launch<C: 'static>(
    title: &str,
    state: C,
    view: impl Fn(&C) -> Element<C> + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    launch_with_chrome(title, state, view, WindowChrome::Native)
}
/// Opt into integrated window chrome without changing other examples.
#[allow(dead_code)]
pub fn launch_with_chrome<C: 'static>(
    title: &str,
    state: C,
    view: impl Fn(&C) -> Element<C> + 'static,
    chrome: WindowChrome,
) -> Result<(), Box<dyn std::error::Error>> {
    launch_configured(
        title,
        state,
        view,
        chrome,
        None,
        WindowBackground::Opaque,
        std::env::args().skip(1).collect(),
        None,
    )
}
#[allow(dead_code)]
pub fn launch_themed<C: 'static>(
    title: &str,
    state: C,
    view: impl Fn(&C) -> Element<C> + 'static,
    theme: fn(&C, ThemeMode) -> UiTheme,
) -> Result<(), Box<dyn std::error::Error>> {
    launch_configured(
        title,
        state,
        view,
        WindowChrome::Native,
        Some(theme),
        WindowBackground::Opaque,
        std::env::args().skip(1).collect(),
        None,
    )
}
/// The lab adds its own flags, then delegates shared snapshot/benchmark hosting here.
#[allow(dead_code)]
pub struct RustLaunchOptions {
    pub chrome: WindowChrome,
    pub background: WindowBackground,
    pub args: Vec<String>,
}
#[allow(dead_code)]
pub fn launch_themed_with_args<C: 'static>(
    title: &str,
    state: C,
    view: impl Fn(&C) -> Element<C> + 'static,
    theme: fn(&C, ThemeMode) -> UiTheme,
    options: RustLaunchOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    launch_configured(
        title,
        state,
        view,
        options.chrome,
        Some(theme),
        options.background,
        options.args,
        None,
    )
}
type NativeLaunch<C> = Box<
    dyn FnOnce(
        SurfaceOptions,
        C,
        Box<dyn Fn(&C) -> Element<C>>,
        Box<dyn Fn(&C, ThemeMode) -> UiTheme>,
    ) -> Result<(), Box<dyn std::error::Error>>,
>;
/// The lab's native worker channel; headless snapshots do not start workers.
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
pub fn launch_themed_with_messages<C: 'static, M: Send + 'static>(
    title: &str,
    state: C,
    view: impl Fn(&C) -> Element<C> + 'static,
    theme: fn(&C, ThemeMode) -> UiTheme,
    options: RustLaunchOptions,
    initialize: impl FnOnce(&mut C, UiSender<M>) + 'static,
    update: impl FnMut(&mut C, M) + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    let launch: NativeLaunch<C> = Box::new(move |options, state, view, theme| {
        run_ui_with_messages_and_theme(options, state, view, theme, initialize, update)
    });
    launch_configured(
        title,
        state,
        view,
        options.chrome,
        Some(theme),
        options.background,
        options.args,
        Some(launch),
    )
}
/// Add application policies without changing snapshot/benchmark behavior.
#[allow(dead_code)]
pub fn launch_themed_with_application<C: 'static>(
    title: &str,
    state: C,
    view: impl Fn(&C) -> Element<C> + 'static,
    theme: fn(&C, ThemeMode) -> UiTheme,
    options: RustLaunchOptions,
    configure: impl FnOnce(volt_ui_kit::native::Application<C>) -> volt_ui_kit::native::Application<C>
        + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    let launch: NativeLaunch<C> = Box::new(move |options, state, view, theme| {
        configure(
            volt_ui_kit::native::Application::new(options.native.title.clone(), state, view)
                .with_options(options)
                .theme_with(theme),
        )
        .run()
    });
    launch_configured(
        title,
        state,
        view,
        options.chrome,
        Some(theme),
        options.background,
        options.args,
        Some(launch),
    )
}
#[allow(clippy::too_many_arguments)]
fn launch_configured<C: 'static>(
    title: &str,
    mut state: C,
    view: impl Fn(&C) -> Element<C> + 'static,
    mut chrome: WindowChrome,
    custom_theme: Option<fn(&C, ThemeMode) -> UiTheme>,
    background: WindowBackground,
    args: Vec<String>,
    native_launch: Option<NativeLaunch<C>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut theme_mode = ThemeMode::Dark;
    let mut reduced_motion = false;
    let mut snapshot = None;
    let mut scale = 1_f32;
    let mut bench = None;
    let mut scroll_step = None::<f32>;
    let mut target = None;
    let mut size = [960_u32, 760_u32];
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--theme" => {
                theme_mode = match args.next().as_deref() {
                    Some("light") => ThemeMode::Light,
                    Some("dark") => ThemeMode::Dark,
                    Some("system") => ThemeMode::System,
                    _ => return Err("--theme must be light, dark or system".into()),
                };
            }
            "--reduced-motion" => reduced_motion = true,
            "--chrome" => {
                chrome = match args.next().as_deref() {
                    Some("native") => WindowChrome::Native,
                    Some("overlay") => WindowChrome::Overlay,
                    Some("borderless") => WindowChrome::Borderless,
                    _ => return Err("--chrome must be native, overlay or borderless".into()),
                };
            }
            "--snapshot" => {
                snapshot = Some(PathBuf::from(args.next().ok_or("missing --snapshot path")?))
            }
            "--scale" => scale = args.next().ok_or("missing --scale value")?.parse()?,
            "--size" => {
                let value = args.next().ok_or("missing --size WIDTHxHEIGHT")?;
                let (width, height) = value.split_once('x').ok_or("size must be WIDTHxHEIGHT")?;
                size = [width.parse()?, height.parse()?];
            }
            "--bench" => {
                bench = Some(
                    args.next()
                        .ok_or("missing --bench count")?
                        .parse::<usize>()?,
                )
            }
            "--bench-target" => target = Some(args.next().ok_or("missing --bench-target key")?),
            "--bench-scroll" => {
                scroll_step = Some(
                    args.next()
                        .ok_or("missing --bench-scroll pixels")?
                        .parse::<f32>()?,
                )
            }
            "--help" => {
                println!("{title}\n--snapshot PNG    render without opening a window\n--size WIDTHxHEIGHT logical window/snapshot/benchmark size (default 960x760)\n--scale NUMBER    snapshot/benchmark scale (0.25..=4)\n--bench COUNT     simulate actions including Rust composition; settled warm render timings (GPU mode includes blocking readback)\n--bench-target KEY choose an enabled button/checkbox/toggle\nVOLT_UI_RENDERER  auto / gpu / software (gpu requires native-gpu)\nVOLT_UI_TRACE=1   native frame/event-to-present diagnostics\n--chrome MODE    native / overlay / borderless\n--reduced-motion settle effects without animation\n--theme MODE     light / dark / system (headless system falls back to dark)\nRust source edits require rebuild; no JSON or F5 source reload.");
                return Ok(());
            }
            _ => {
                return Err(
                    format!("unknown option {arg}; Rust examples do not accept --document").into(),
                )
            }
        }
    }
    if !scale.is_finite() || !(0.25..=4.).contains(&scale) {
        return Err("scale must be in [0.25, 4]".into());
    }
    if size.contains(&0) || size.iter().any(|&v| v as f32 * scale > 16384.) {
        return Err("size must be positive and scaled dimensions must be <=16384".into());
    }
    if snapshot.is_some() && bench.is_some() {
        return Err("--snapshot and --bench are mutually exclusive".into());
    }
    if target.is_some() && bench.is_none() {
        return Err("--bench-target requires --bench".into());
    }
    if let Some(count) = bench {
        if !(1..=1000).contains(&count) {
            return Err("benchmark count must be in [1, 1000]".into());
        }
    }
    if snapshot.is_none() && bench.is_none() {
        let options = NativeOptions {
            title: title.into(),
            size,
            document_path: None,
        }
        .with_chrome(chrome)
        .with_reduced_motion(reduced_motion)
        .with_theme(theme_mode)
        .with_background(background);
        if let Some(launch) = native_launch {
            return launch(
                options,
                state,
                Box::new(view),
                Box::new(move |state, system| {
                    let mode = if theme_mode == ThemeMode::System {
                        system
                    } else {
                        theme_mode
                    };
                    custom_theme.map_or_else(
                        || {
                            if mode == ThemeMode::Light {
                                UiTheme::light()
                            } else {
                                UiTheme::dark()
                            }
                        },
                        |theme| theme(state, mode),
                    )
                }),
            );
        }
        return if let Some(theme) = custom_theme {
            run_ui_with_theme(options, state, view, move |state, system| {
                theme(
                    state,
                    if theme_mode == ThemeMode::System {
                        system
                    } else {
                        theme_mode
                    },
                )
            })
        } else {
            run_ui_with_options(options, state, view)
        };
    }
    let values = Bindings::default();
    let (document, mut handlers) = view(&state).build()?.into_parts();
    let mut runtime = Runtime::new(document, &handlers, &values)?;
    let viewport = Viewport::new(size[0] as f32 * scale, size[1] as f32 * scale, scale).unwrap();
    let mut painter = RasterPainter::new(viewport);
    let resolve_theme = |state: &C| {
        custom_theme.map_or_else(
            || {
                if theme_mode == ThemeMode::Light {
                    UiTheme::light()
                } else {
                    UiTheme::dark()
                }
            },
            |f| f(state, theme_mode),
        )
    };
    let theme = resolve_theme(&state);
    let clear = |theme: &UiTheme| {
        let mut color = theme.legacy_theme().background;
        color[3] = match background {
            WindowBackground::Opaque => 1.,
            WindowBackground::Transparent { opacity }
            | WindowBackground::Blur { opacity }
            | WindowBackground::CompositorBlur { opacity } => opacity,
        };
        color
    };
    runtime.set_reduced_motion(reduced_motion);
    runtime.layout(viewport, &values, &mut painter)?;
    let requested = std::env::var("VOLT_UI_RENDERER").ok();
    if requested
        .as_deref()
        .is_some_and(|r| !["auto", "gpu", "software"].contains(&r))
    {
        return Err("VOLT_UI_RENDERER must be auto, gpu or software".into());
    }
    #[cfg(feature = "native-gpu")]
    let mut gpu = if requested.as_deref() != Some("software") {
        match volt_ui_kit::native::GpuOffscreenRenderer::new() {
            Ok(gpu) => {
                eprintln!(
                    "Headless GPU drawing: {} (readback enabled)",
                    gpu.adapter_name
                );
                Some(gpu)
            }
            Err(error) if requested.as_deref() == Some("gpu") => return Err(error.into()),
            Err(error) => {
                eprintln!("GPU unavailable ({error}); software snapshot/benchmark fallback");
                None
            }
        }
    } else {
        None
    };
    #[cfg(feature = "native-gpu")]
    let gpu_active = gpu.is_some();
    #[cfg(not(feature = "native-gpu"))]
    let gpu_active = false;
    #[cfg(not(feature = "native-gpu"))]
    if requested.as_deref() == Some("gpu") {
        return Err("GPU rendering requires native-gpu".into());
    }
    #[allow(unused_mut)] // GPU closure mutably borrows the optional renderer.
    let mut render = |p: &mut RasterPainter,
                      batches: &[PaintBatch<
        <RasterPainter as Painter>::Shape,
        <RasterPainter as Painter>::Glyph,
    >],
                      clear: [f32; 4]|
     -> Result<tiny_skia::Pixmap, String> {
        #[cfg(feature = "native-gpu")]
        if let Some(gpu) = &mut gpu {
            return gpu.render(p, batches, clear);
        }
        p.rasterize(batches, clear)
    };
    if let Some(path) = snapshot {
        runtime.settle_animations(); // snapshots show the settled design, not fade frame zero
        let batches = runtime.paint_ui(&mut painter, &theme);
        render(&mut painter, &batches, clear(&theme))?.save_png(&path)?;
        println!(
            "Rendered {} Rust nodes to {} (not native input proof)",
            runtime.boxes().len(),
            path.display()
        );
        return Ok(());
    }
    let count = bench.unwrap();
    if let Some(step) = scroll_step {
        // Scroll frames: offset change, layout, paint and render, as the
        // native host does per wheel/trackpad event. Reverses at either end.
        let (x, y) = (viewport.width * 0.6, viewport.height * 0.6);
        let mut direction = 1.;
        let mut samples: Vec<[f64; 4]> = Vec::with_capacity(count);
        for i in 0..count + 4 {
            let start = Instant::now();
            if !runtime.scroll_at(x, y, [0., step * direction]) {
                direction = -direction;
                runtime.scroll_at(x, y, [0., step * direction]);
            }
            runtime.layout(viewport, &values, &mut painter)?;
            let layout = start.elapsed().as_secs_f64() * 1000.;
            let stage = Instant::now();
            let theme = resolve_theme(&state);
            let batches = runtime.paint_ui(&mut painter, &theme);
            let paint = stage.elapsed().as_secs_f64() * 1000.;
            let stage = Instant::now();
            std::hint::black_box(render(&mut painter, &batches, clear(&theme))?);
            let raster = stage.elapsed().as_secs_f64() * 1000.;
            if i >= 4 {
                samples.push([layout, paint, raster, start.elapsed().as_secs_f64() * 1000.]);
            }
        }
        println!(
            "BENCH-SCROLL {title} {}x{} scale={scale} step={step}px samples={count}",
            viewport.width, viewport.height
        );
        for (column, name) in ["layout", "paint", "render", "frame"].iter().enumerate() {
            let mut values: Vec<_> = samples.iter().map(|s| s[column]).collect();
            values.sort_by(f64::total_cmp);
            println!(
                "{name}: p50={:.3}ms p95={:.3}ms max={:.3}ms",
                values[(values.len() - 1) / 2],
                values[(values.len() as f64 * 0.95).ceil() as usize - 1],
                values[values.len() - 1]
            );
        }
        println!(
            "text cache misses (all frames): {}",
            painter.text_cache_misses
        );
        return Ok(());
    }
    let target = runtime
        .boxes()
        .iter()
        .find(|b| {
            b.enabled
                && match &target {
                    Some(id) => {
                        b.id == *id
                            && matches!(b.kind, Kind::Button | Kind::Checkbox | Kind::Toggle)
                    }
                    None => b.kind == Kind::Button,
                }
        })
        .ok_or("benchmark target must be an enabled interactive control")?
        .id
        .clone();
    let mut packed = vec![0_u32; (viewport.width * viewport.height) as usize];
    let mut samples: Vec<[f64; 5]> = Vec::with_capacity(count);
    for i in 0..count + 4 {
        let area = runtime
            .boxes()
            .iter()
            .find(|b| b.id == target)
            .ok_or("benchmark target removed")?
            .bounds;
        let [x, y, w, h] = area.bounds();
        let (x, y) = (x + w * 0.5, y + h * 0.5);
        runtime.pointer_move(x, y);
        runtime.pointer_down(x, y);
        if runtime.focused() != Some(target.as_str()) {
            return Err("benchmark target is clipped or not hittable".into());
        }
        let pressed = runtime.paint_ui(&mut painter, &theme);
        render(&mut painter, &pressed, clear(&theme))?;
        let start = Instant::now();
        let outcome = runtime.pointer_up_outcome(x, y, &mut handlers, &mut state)?;
        if !outcome.activated {
            return Err("benchmark target did not activate".into());
        }
        let (next, next_handlers) = view(&state).build()?.into_parts();
        runtime.replace_document(next, &next_handlers, &values)?;
        handlers = next_handlers;
        runtime.layout(viewport, &values, &mut painter)?;
        runtime.pointer_move(x, y);
        // Time the settled result of the action. Headless runs never advance a
        // clock, so without this every sample was frame zero of a fade-in.
        runtime.settle_animations();
        let compose = start.elapsed().as_secs_f64() * 1000.;
        let stage = Instant::now();
        let theme = resolve_theme(&state);
        let batches = runtime.paint_ui(&mut painter, &theme);
        let paint = stage.elapsed().as_secs_f64() * 1000.;
        let stage = Instant::now();
        let pixmap = render(&mut painter, &batches, clear(&theme))?;
        let raster = stage.elapsed().as_secs_f64() * 1000.;
        let stage = Instant::now();
        for (dst, pixel) in packed.iter_mut().zip(pixmap.pixels()) {
            *dst = (u32::from(pixel.red()) << 16)
                | (u32::from(pixel.green()) << 8)
                | u32::from(pixel.blue());
        }
        std::hint::black_box(&packed);
        let pack = stage.elapsed().as_secs_f64() * 1000.;
        if i >= 4 {
            samples.push([
                compose,
                paint,
                raster,
                pack,
                start.elapsed().as_secs_f64() * 1000.,
            ]);
        }
    }
    let boundary = if gpu_active {
        "GPU completion + CPU readback; excludes OS event queue/presentation"
    } else {
        "CPU only; excludes OS event queue/presentation"
    };
    println!(
        "BENCH {title} {}x{} scale={scale} target={target} samples={count} warmup=4 ({boundary})",
        viewport.width, viewport.height
    );
    for (column, name) in [
        "dispatch+compose+layout",
        "paint",
        if gpu_active {
            "GPU render+readback"
        } else {
            "raster"
        },
        "pack",
        "release-to-pixels",
    ]
    .iter()
    .enumerate()
    {
        let mut values: Vec<_> = samples.iter().map(|s| s[column]).collect();
        values.sort_by(f64::total_cmp);
        println!(
            "{name}: p50={:.3}ms p95={:.3}ms max={:.3}ms",
            values[(values.len() - 1) / 2],
            values[(values.len() as f64 * 0.95).ceil() as usize - 1],
            values[values.len() - 1]
        );
    }
    println!(
        "cache totals (including warmup/pressed frames): text={} effect-masks={} effect-tints={} layer-allocations={}",
        painter.text_cache_misses, painter.effect_cache_misses,
        painter.effect_tint_misses, painter.layer_allocations,
    );
    Ok(())
}
