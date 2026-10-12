use std::path::PathBuf;
use std::time::Instant;
use volt_ui_kit::{
    declarative::{Bindings, Document, Handlers, Runtime},
    native::{run, NativeOptions, RasterPainter},
    Theme, Viewport,
};

/// Common CLI and host setup; each example supplies its own state/callbacks.
pub fn launch<C: 'static>(
    title: &str,
    embedded: &str,
    path: Option<&str>,
    mut state: C,
    mut handlers: Handlers<'static, C>,
    update: impl Fn(&C, &mut Bindings) + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut document_path = path.map(|p| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(p));
    let mut snapshot = None;
    let mut scale = 1_f32;
    let mut bench = None;
    let mut bench_target = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--document" => {
                document_path = Some(PathBuf::from(args.next().ok_or("missing --document path")?))
            }
            "--snapshot" => {
                snapshot = Some(PathBuf::from(args.next().ok_or("missing --snapshot path")?))
            }
            "--scale" => scale = args.next().ok_or("missing --scale value")?.parse()?,
            "--bench" => {
                bench = Some(
                    args.next()
                        .ok_or("missing --bench count")?
                        .parse::<usize>()?,
                )
            }
            "--bench-target" => {
                bench_target = Some(args.next().ok_or("missing --bench-target ID")?)
            }
            "--help" => {
                println!("{title}\n--document FILE   initial document + F5 reload\n--snapshot PNG    render without opening a window\n--scale NUMBER    snapshot/benchmark scale (0.25 <= scale <= 4)\n--bench COUNT     simulate clicks; report warm CPU timings\n--bench-target ID choose a button/checkbox/toggle for the benchmark\nVOLT_UI_TRACE=1   log native frame and event-to-present timings");
                return Ok(());
            }
            _ => return Err(format!("unknown option {arg}").into()),
        }
    }
    if !scale.is_finite() || !(0.25..=4.).contains(&scale) {
        return Err("scale must be in [0.25, 4]".into());
    }
    if snapshot.is_some() && bench.is_some() {
        return Err("--snapshot and --bench are mutually exclusive".into());
    }
    if bench_target.is_some() && bench.is_none() {
        return Err("--bench-target requires --bench".into());
    }
    let source = if let Some(path) = &document_path {
        volt_ui_kit::native::load_document(path)?
    } else {
        embedded.into()
    };
    if let Some(count) = bench {
        if !(1..=1000).contains(&count) {
            return Err("benchmark count must be in [1, 1000]".into());
        }
        let mut bindings = Bindings::default();
        update(&state, &mut bindings);
        let mut runtime = Runtime::new(Document::parse(&source)?, &handlers, &bindings)?;
        let viewport = Viewport::new(960. * scale, 760. * scale, scale).unwrap();
        let mut painter = RasterPainter::new(viewport);
        let theme = Theme::dark();
        runtime.layout(viewport, &bindings, &mut painter)?;
        let target = runtime
            .boxes()
            .iter()
            .find(|b| {
                use volt_ui_kit::declarative::Kind;
                b.enabled
                    && match &bench_target {
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
                return Err("benchmark target is clipped or not hittable at this scale".into());
            }
            // Include the pressed frame as a real native click would, before release.
            let batches = runtime.paint(&mut painter, &theme);
            let _pressed = painter.rasterize(&batches, theme.background)?;
            let start = Instant::now();
            runtime.pointer_up(x, y, &mut handlers, &mut state)?;
            update(&state, &mut bindings);
            runtime.layout(viewport, &bindings, &mut painter)?;
            runtime.pointer_move(x, y);
            let layout = start.elapsed().as_secs_f64() * 1000.;
            let stage = Instant::now();
            let batches = runtime.paint(&mut painter, &theme);
            let paint = stage.elapsed().as_secs_f64() * 1000.;
            let stage = Instant::now();
            let pixmap = painter.rasterize(&batches, theme.background)?;
            let raster = stage.elapsed().as_secs_f64() * 1000.;
            let stage = Instant::now();
            for (dst, pixel) in packed.iter_mut().zip(pixmap.pixels()) {
                *dst = (u32::from(pixel.red()) << 16)
                    | (u32::from(pixel.green()) << 8)
                    | u32::from(pixel.blue());
            }
            std::hint::black_box(&packed);
            let pack = stage.elapsed().as_secs_f64() * 1000.;
            let total = start.elapsed().as_secs_f64() * 1000.;
            if i >= 4 {
                samples.push([layout, paint, raster, pack, total]);
            }
        }
        println!("BENCH {title} {}x{} scale={scale} target={target} samples={count} warmup=4 (CPU only; excludes OS event queue/presentation)", viewport.width, viewport.height);
        for (column, name) in [
            "dispatch+layout",
            "paint",
            "raster",
            "pack",
            "release-to-pixels",
        ]
        .iter()
        .enumerate()
        {
            let mut values: Vec<_> = samples.iter().map(|s| s[column]).collect();
            values.sort_by(f64::total_cmp);
            let p50 = values[(values.len() - 1) / 2];
            let p95 = values[(values.len() as f64 * 0.95).ceil() as usize - 1];
            println!(
                "{name}: p50={p50:.3}ms p95={p95:.3}ms max={:.3}ms",
                values[values.len() - 1]
            );
        }
        println!("text_cache_misses={}", painter.text_cache_misses);
        return Ok(());
    }
    if let Some(path) = snapshot {
        let mut bindings = Bindings::default();
        update(&state, &mut bindings);
        let mut runtime = Runtime::new(Document::parse(&source)?, &handlers, &bindings)?;
        let viewport = Viewport::new(960. * scale, 760. * scale, scale).unwrap();
        let mut painter = RasterPainter::new(viewport);
        runtime.layout(viewport, &bindings, &mut painter)?;
        let theme = Theme::dark();
        let batches = runtime.paint(&mut painter, &theme);
        painter
            .rasterize(&batches, theme.background)?
            .save_png(&path)?;
        println!(
            "Rendered {} nodes to {} (not native runtime proof)",
            runtime.boxes().len(),
            path.display()
        );
        return Ok(());
    }
    run(
        NativeOptions {
            title: title.into(),
            size: [960, 760],
            document_path,
        },
        &source,
        state,
        handlers,
        update,
    )
}
