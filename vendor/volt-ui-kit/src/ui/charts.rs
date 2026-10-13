//! Data charts with axes: area, candlestick, donut and stacked bars. Drawn
//! as retained vector paths; colours follow the theme (series 1 is
//! `primary`). Sizes are logical pixels.
//!
//! Give a chart a `.key(..)` to make it interactive: every data point gets
//! an invisible, focusable hit target with a value tooltip (hover or
//! keyboard focus, no recomposition), and `.selected(..)` /
//! `.on_select(..)` turn clicks into a highlighted selection.
use super::*;
use std::{cell::RefCell, rc::Rc};

type Pick<C> = Rc<RefCell<dyn FnMut(&mut C, usize)>>;
/// Interaction settings shared by every chart.
struct Interact<C> {
    key: Option<String>,
    selected: Option<usize>,
    on_select: Option<Pick<C>>,
}
impl<C> Default for Interact<C> {
    fn default() -> Self {
        Self {
            key: None,
            selected: None,
            on_select: None,
        }
    }
}
/// Builder methods every chart shares.
macro_rules! interactive {
    ($chart:ident) => {
        impl<C> $chart<C> {
            /// Make the chart interactive: hover/focus tooltips per data point.
            pub fn key(mut self, key: impl Into<String>) -> Self {
                self.interact.key = Some(key.into());
                self
            }
            /// Highlight one data point (index into the data).
            pub fn selected(mut self, index: Option<usize>) -> Self {
                self.interact.selected = index;
                self
            }
            /// Called with a data point's index when it is clicked or
            /// activated from the keyboard. Requires `.key(..)`.
            pub fn on_select(mut self, f: impl FnMut(&mut C, usize) + 'static) -> Self {
                self.interact.on_select = Some(Rc::new(RefCell::new(f)));
                self
            }
        }
    };
}
/// One invisible hit target: a focusable button with a value tooltip,
/// tinted while hovered. `tip` lines become the accessible name too.
fn hit_target<C: 'static>(
    key: &str,
    index: usize,
    size: [f32; 2],
    tip: String,
    on_select: Option<Pick<C>>,
) -> Element<C> {
    let label = tip.replace('\n', ", ");
    let mut b = button("")
        .key(format!("{key}-{index}"))
        .accessible_label(label)
        .variant(ButtonVariant::Ghost)
        .width(size[0])
        .height(size[1])
        .rounded(0.)
        .border_width(0.)
        .background("#00000000")
        .hover_style(|s| s.background(ColorToken::BaseContent.alpha(7)))
        .cursor(if on_select.is_some() {
            Cursor::Pointer
        } else {
            Cursor::Default
        });
    b = match on_select {
        Some(f) => b.on_click(move |app: &mut C| (f.borrow_mut())(app, index)),
        None => b.on_click(|_: &mut C| {}),
    };
    tooltip(tip, b).key(format!("{key}-{index}-tip")).into()
}
/// A row of full-height hit columns over the plot.
fn hit_columns<C: 'static>(
    key: &str,
    widths: &[f32],
    height: f32,
    tips: Vec<String>,
    on_select: &Option<Pick<C>>,
) -> Element<C> {
    hstack(
        widths
            .iter()
            .zip(tips)
            .enumerate()
            .map(|(i, (w, tip))| hit_target(key, i, [*w, height], tip, on_select.clone()))
            .collect::<Vec<_>>(),
    )
    .key(format!("{key}-hits"))
    .into()
}

/// Series colours: the theme's primary, then hues that stay distinct.
const SERIES: [&str; 6] = [
    "@primary", "#0EA5E9", "#F59E0B", "#10B981", "#8B5CF6", "#EF4444",
];
fn series_color(i: usize) -> &'static str {
    SERIES[i % SERIES.len()]
}
/// A colour at a percentage opacity: tokens use `@name/NN`, hex gets an
/// alpha byte.
fn faded(color: &str, percent: u8) -> String {
    if color.starts_with('@') {
        format!("{color}/{percent}")
    } else {
        format!("{color}{:02X}", (u32::from(percent.min(100)) * 255 / 100))
    }
}

/// Up to `count` round tick values covering `lo..=hi` (1, 2, 2.5 or 5 × 10ⁿ
/// steps), plus the expanded range they span.
pub fn nice_ticks(lo: f32, hi: f32, count: usize) -> (Vec<f32>, (f32, f32)) {
    let (lo, hi) = if (hi - lo).abs() < f32::EPSILON {
        (lo - 1., hi + 1.)
    } else {
        (lo.min(hi), lo.max(hi))
    };
    let raw = (hi - lo) / count.max(1) as f32;
    let mag = 10f32.powf(raw.log10().floor());
    let step = [1., 2., 2.5, 5., 10.]
        .into_iter()
        .map(|m| m * mag)
        .find(|s| *s >= raw)
        .unwrap_or(10. * mag);
    let start = (lo / step).floor() * step;
    let end = (hi / step).ceil() * step;
    let n = ((end - start) / step).round() as usize;
    let ticks = (0..=n.min(64)).map(|i| start + i as f32 * step).collect();
    (ticks, (start, end))
}
/// Compact tick labels: 1.2k, 3.4M, trimmed decimals.
pub fn short_number(v: f32) -> String {
    let a = v.abs();
    let (scaled, suffix) = if a >= 1e9 {
        (v / 1e9, "B")
    } else if a >= 1e6 {
        (v / 1e6, "M")
    } else if a >= 1e3 {
        (v / 1e3, "k")
    } else {
        (v, "")
    };
    let text = if scaled.fract().abs() < 1e-3 {
        format!("{scaled:.0}")
    } else if scaled.abs() < 10. {
        format!("{scaled:.2}")
    } else {
        format!("{scaled:.1}")
    };
    let text = if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text
    };
    format!("{text}{suffix}")
}

/// Packs many small shapes of one colour into as few canvas paths as possible.
/// A canvas holds at most 256 paths, so one path per candle or bar would cap a
/// chart at roughly a hundred items. Shapes in one batch must not overlap:
/// canvas fills use the even-odd rule.
pub(super) struct PathBatch(Vec<VectorPath>);
impl PathBatch {
    pub(super) fn new() -> Self {
        Self(Vec::new())
    }
    pub(super) fn add(&mut self, shape: VectorPath) {
        match self.0.last_mut() {
            Some(path)
                if path.commands.len() + shape.commands.len()
                    <= crate::graphics::MAX_PATH_COMMANDS =>
            {
                path.commands.extend(shape.commands)
            }
            _ => self.0.push(shape),
        }
    }
    pub(super) fn fill(self, c: &mut Canvas, color: impl Into<String>) {
        let color = color.into();
        for path in self.0 {
            c.fill(path, color.clone());
        }
    }
    pub(super) fn stroke(self, c: &mut Canvas, color: impl Into<String>, width: f32) {
        let color = color.into();
        for path in self.0 {
            c.stroke(path, color.clone(), width);
        }
    }
}

type Format = fn(f32) -> String;
const AXIS: f32 = 44.;
const X_LABELS: f32 = 20.;

/// A plot area with Y tick labels at the left and X labels underneath.
/// `draw` receives the plot size and the value range mapped to it.
#[allow(clippy::too_many_arguments)]
fn framed<C>(
    width: f32,
    height: f32,
    range: (f32, f32),
    ticks: Vec<f32>,
    format: Format,
    x_labels: Vec<(f32, String)>,
    overlay: Option<Element<C>>,
    draw: impl FnOnce(&mut Canvas, f32, f32, &dyn Fn(f32) -> f32),
) -> Element<C> {
    let plot_w = (width - AXIS).max(10.);
    let plot_h = (height - X_LABELS).max(10.);
    let to_y = move |v: f32| plot_h - (v - range.0) / (range.1 - range.0) * plot_h;
    let grid_ticks = ticks.clone();
    let plot = canvas(move |c| {
        for t in &grid_ticks {
            let y = to_y(*t).round() + 0.5;
            c.stroke(
                VectorPath::new().move_to(0., y).line_to(plot_w, y),
                ColorToken::Border,
                1.,
            );
        }
        draw(c, plot_w, plot_h, &to_y);
    })
    .width(plot_w)
    .height(plot_h);
    let plot: Element<C> = match overlay {
        Some(hits) => zstack![plot, hits]
            .width(plot_w)
            .height(plot_h)
            .flow(Flow::TopLeft)
            .into(),
        None => plot,
    };
    // Tick labels sit on their gridlines.
    let labels: Vec<Element<C>> = ticks
        .iter()
        .map(|t| {
            hstack![text(format(*t)).font_size(11.).color(ColorToken::Muted)]
                .width(AXIS - 8.)
                .justify(Justify::End)
                .padding((to_y(*t) - 7.).clamp(0., plot_h - 14.), Edge::Top)
                .into()
        })
        .collect();
    let axis = zstack(labels)
        .width(AXIS)
        .height(plot_h)
        .flow(Flow::TopLeft);
    let mut parts = vec![hstack![axis, plot].align(Align::Start).into()];
    if !x_labels.is_empty() {
        parts.push(x_label_row(x_labels, width));
    }
    vstack(parts).width(width).into()
}
/// Labels centred under their x positions in the plot area.
fn x_label_row<C>(x_labels: Vec<(f32, String)>, width: f32) -> Element<C> {
    let xs: Vec<Element<C>> = x_labels
        .into_iter()
        .map(|(x, label)| {
            // Centred on x, but kept inside the chart at both ends.
            let w = label.chars().count() as f32 * 6.5;
            let left = (AXIS + x - w / 2.).clamp(AXIS, (width - w - 2.).max(AXIS));
            hstack![text(label)
                .font_size(11.)
                .color(ColorToken::Muted)
                .no_wrap(true)]
            .padding(left, Edge::Left)
            .into()
        })
        .collect();
    zstack(xs)
        .height(X_LABELS)
        .flow(Flow::BottomLeft)
        .width(width)
        .into()
}
/// X positions for `n` labels across `w`, thinned so they never crowd.
fn spread_labels(labels: &[String], w: f32, centered_slots: bool) -> Vec<(f32, String)> {
    let n = labels.len();
    if n == 0 {
        return Vec::new();
    }
    // Room per label from its length (≈6.5 px per character at 11 px).
    let longest = labels.iter().map(|l| l.chars().count()).max().unwrap_or(1) as f32;
    let room = longest * 6.5 + 14.;
    let every = ((n as f32 * room) / w).ceil().max(1.) as usize;
    labels
        .iter()
        .enumerate()
        .filter(|(i, _)| i % every == 0)
        .map(|(i, l)| {
            let x = if centered_slots {
                (i as f32 + 0.5) * w / n as f32
            } else if n > 1 {
                i as f32 * w / (n - 1) as f32
            } else {
                w / 2.
            };
            (x, l.clone())
        })
        .collect()
}
fn legend<C>(names: &[String]) -> Element<C> {
    hstack(
        names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                hstack![
                    vstack![]
                        .width(10.)
                        .height(10.)
                        .rounded(3.)
                        .background(series_color(i)),
                    text(name.clone()).font_size(12.).color(ColorToken::Muted),
                ]
                .gap(6., Axis::Horizontal)
                .align(Align::Center)
                .into()
            })
            .collect::<Vec<Element<C>>>(),
    )
    .gap(14., Axis::Horizontal)
    .wrap(true)
    .into()
}

// ── Area chart ──────────────────────────────────────────────────────────

/// Filled series over shared x positions, overlaid or stacked.
pub struct AreaChart<C> {
    series: Vec<(String, Vec<f32>)>,
    size: [f32; 2],
    x_labels: Vec<String>,
    stacked: bool,
    legend: bool,
    format: Format,
    interact: Interact<C>,
}
pub fn area_chart<C>(
    series: impl IntoIterator<Item = (impl Into<String>, Vec<f32>)>,
) -> AreaChart<C> {
    AreaChart {
        series: series.into_iter().map(|(n, v)| (n.into(), v)).collect(),
        size: [480., 220.],
        x_labels: Vec::new(),
        stacked: false,
        legend: true,
        format: short_number,
        interact: Interact::default(),
    }
}
interactive!(AreaChart);
impl<C> AreaChart<C> {
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = [width, height];
        self
    }
    pub fn x_labels(mut self, labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.x_labels = labels.into_iter().map(Into::into).collect();
        self
    }
    /// Draw each series on top of the ones before it.
    pub fn stacked(mut self, value: bool) -> Self {
        self.stacked = value;
        self
    }
    pub fn legend(mut self, value: bool) -> Self {
        self.legend = value;
        self
    }
    pub fn value_format(mut self, f: fn(f32) -> String) -> Self {
        self.format = f;
        self
    }
}
impl<C: 'static> From<AreaChart<C>> for Element<C> {
    fn from(a: AreaChart<C>) -> Self {
        let n = a.series.iter().map(|(_, v)| v.len()).max().unwrap_or(0);
        let value = |s: usize, x: usize| {
            a.series[s]
                .1
                .get(x)
                .copied()
                .filter(|v| v.is_finite())
                .unwrap_or(0.)
        };
        // Cumulative tops when stacked.
        let mut tops: Vec<Vec<f32>> = Vec::new();
        for i in 0..a.series.len() {
            let row: Vec<f32> = (0..n)
                .map(|x| {
                    if a.stacked && i > 0 {
                        tops[i - 1][x] + value(i, x)
                    } else {
                        value(i, x)
                    }
                })
                .collect();
            tops.push(row);
        }
        let all: Vec<f32> = tops.iter().flatten().copied().collect();
        let lo = if a.stacked {
            0.
        } else {
            all.iter().copied().fold(f32::MAX, f32::min).min(0.)
        };
        let hi = all.iter().copied().fold(f32::MIN, f32::max);
        let (ticks, range) = nice_ticks(lo, hi.max(lo + 1.), 4);
        let legend_names: Vec<String> = a.series.iter().map(|(n, _)| n.clone()).collect();
        let height = a.size[1] - if a.legend { 26. } else { 0. };
        let plot_w = a.size[0] - AXIS;
        let plot_h = (height - X_LABELS).max(10.);
        let x_labels = spread_labels(&a.x_labels, plot_w, false);
        // One hit column per x position, centred on it.
        let overlay = a.interact.key.as_deref().filter(|_| n > 1).map(|key| {
            let dx = plot_w / (n - 1) as f32;
            let widths: Vec<f32> = (0..n)
                .map(|i| if i == 0 || i == n - 1 { dx / 2. } else { dx })
                .collect();
            let tips = (0..n)
                .map(|x| {
                    let mut tip = a
                        .x_labels
                        .get(x)
                        .cloned()
                        .unwrap_or_else(|| format!("#{}", x + 1));
                    for (s, name) in legend_names.iter().enumerate() {
                        tip.push_str(&format!("\n{name}: {}", (a.format)(value(s, x))));
                    }
                    tip
                })
                .collect();
            hit_columns(key, &widths, plot_h, tips, &a.interact.on_select)
        });
        let (stacked, selected) = (a.stacked, a.interact.selected.filter(|i| *i < n));
        let chart = framed(
            a.size[0],
            height,
            range,
            ticks,
            a.format,
            x_labels,
            overlay,
            move |c, w, h, to_y| {
                if n < 2 {
                    return;
                }
                let dx = w / (n - 1) as f32;
                // Paint later series first when overlaid so the first stays on top.
                let order: Vec<usize> = if stacked {
                    (0..tops.len()).collect()
                } else {
                    (0..tops.len()).rev().collect()
                };
                for i in order {
                    let top: Vec<[f32; 2]> =
                        (0..n).map(|x| [x as f32 * dx, to_y(tops[i][x])]).collect();
                    let base: Vec<[f32; 2]> = (0..n)
                        .rev()
                        .map(|x| {
                            let b = if stacked && i > 0 {
                                tops[i - 1][x]
                            } else {
                                range.0.max(0.)
                            };
                            [x as f32 * dx, to_y(b).min(h)]
                        })
                        .collect();
                    let mut area = VectorPath::new().move_to(top[0][0], top[0][1]);
                    for p in &top[1..] {
                        area = area.line_to(p[0], p[1]);
                    }
                    for p in &base {
                        area = area.line_to(p[0], p[1]);
                    }
                    let color = series_color(i);
                    c.fill(area.close(), faded(color, if stacked { 55 } else { 22 }));
                    let mut line = VectorPath::new().move_to(top[0][0], top[0][1]);
                    for p in &top[1..] {
                        line = line.line_to(p[0], p[1]);
                    }
                    c.stroke(line, color, 2.);
                }
                // Selection: a rule through the point and a dot per series.
                if let Some(x) = selected {
                    let px = (x as f32 * dx).round() + 0.5;
                    c.stroke(
                        VectorPath::new().move_to(px, 0.).line_to(px, h),
                        ColorToken::BaseContent.alpha(45),
                        1.,
                    );
                    for (i, row) in tops.iter().enumerate() {
                        let y = to_y(row[x]);
                        c.fill(VectorPath::circle(px, y, 5.), ColorToken::Base100);
                        c.fill(VectorPath::circle(px, y, 3.5), series_color(i));
                    }
                }
            },
        );
        if a.legend {
            vstack![chart, legend(&legend_names)]
                .gap(10., Axis::Vertical)
                .into()
        } else {
            chart
        }
    }
}

// ── Candlesticks ────────────────────────────────────────────────────────

/// One period of open/high/low/close prices and optional volume.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candle {
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: f32,
}
impl Candle {
    pub fn new(open: f32, high: f32, low: f32, close: f32) -> Self {
        Self {
            open,
            high,
            low,
            close,
            volume: 0.,
        }
    }
    pub fn volume(mut self, value: f32) -> Self {
        self.volume = value;
        self
    }
    fn valid(&self) -> bool {
        [self.open, self.high, self.low, self.close, self.volume]
            .iter()
            .all(|v| v.is_finite())
            && self.low <= self.high
    }
}
/// Price candles: rising periods in `success`, falling in `error`, with an
/// optional volume strip underneath.
pub struct CandlestickChart<C> {
    candles: Vec<Candle>,
    size: [f32; 2],
    x_labels: Vec<String>,
    volume: bool,
    format: Format,
    interact: Interact<C>,
}
pub fn candlestick_chart<C>(candles: impl IntoIterator<Item = Candle>) -> CandlestickChart<C> {
    CandlestickChart {
        candles: candles.into_iter().filter(Candle::valid).collect(),
        size: [560., 280.],
        x_labels: Vec::new(),
        volume: false,
        format: short_number,
        interact: Interact::default(),
    }
}
interactive!(CandlestickChart);
impl<C> CandlestickChart<C> {
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = [width, height];
        self
    }
    pub fn x_labels(mut self, labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.x_labels = labels.into_iter().map(Into::into).collect();
        self
    }
    /// Show volume bars in a strip below the prices.
    pub fn volume(mut self, value: bool) -> Self {
        self.volume = value;
        self
    }
    pub fn value_format(mut self, f: fn(f32) -> String) -> Self {
        self.format = f;
        self
    }
}
impl<C: 'static> From<CandlestickChart<C>> for Element<C> {
    fn from(k: CandlestickChart<C>) -> Self {
        let lo = k.candles.iter().map(|c| c.low).fold(f32::MAX, f32::min);
        let hi = k.candles.iter().map(|c| c.high).fold(f32::MIN, f32::max);
        let (ticks, range) = if k.candles.is_empty() {
            nice_ticks(0., 1., 4)
        } else {
            nice_ticks(lo, hi, 4)
        };
        let vol_h = if k.volume {
            (k.size[1] * 0.18).round()
        } else {
            0.
        };
        let plot_w = k.size[0] - AXIS;
        let height = k.size[1] - vol_h - if k.volume { X_LABELS } else { 0. };
        let plot_h = (height - X_LABELS).max(10.);
        let x_labels = spread_labels(&k.x_labels, plot_w, true);
        // With volume, the dates go under the volume strip instead.
        let (inline_labels, trailing_labels) = if k.volume {
            (Vec::new(), x_labels)
        } else {
            (x_labels, Vec::new())
        };
        let n = k.candles.len().max(1);
        let f = k.format;
        let overlay = k
            .interact
            .key
            .as_deref()
            .filter(|_| !k.candles.is_empty())
            .map(|key| {
                let tips = k
                    .candles
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let mut tip = k
                            .x_labels
                            .get(i)
                            .cloned()
                            .unwrap_or_else(|| format!("#{}", i + 1));
                        let change = if c.open != 0. {
                            (c.close - c.open) / c.open * 100.
                        } else {
                            0.
                        };
                        tip.push_str(&format!(
                            "\nOpen {}  High {}\nLow {}  Close {}\nChange {}{change:.2}%",
                            f(c.open),
                            f(c.high),
                            f(c.low),
                            f(c.close),
                            if change >= 0. { "+" } else { "" }
                        ));
                        if c.volume > 0. {
                            tip.push_str(&format!("\nVolume {}", short_number(c.volume)));
                        }
                        tip
                    })
                    .collect();
                hit_columns(
                    key,
                    &vec![plot_w / n as f32; n],
                    plot_h,
                    tips,
                    &k.interact.on_select,
                )
            });
        let candles = k.candles.clone();
        let selected = k.interact.selected.filter(|i| *i < k.candles.len());
        let chart = framed(
            k.size[0],
            height,
            range,
            ticks,
            k.format,
            inline_labels,
            overlay,
            move |c, w, h, to_y| {
                let slot = w / n as f32;
                let body = (slot * 0.6).clamp(1., 18.);
                if let Some(i) = selected {
                    c.fill(
                        VectorPath::rectangle(i as f32 * slot, 0., slot, h),
                        ColorToken::BaseContent.alpha(9),
                    );
                }
                let body_rect = |i: usize, k: &Candle| {
                    let x = (i as f32 + 0.5) * slot;
                    let (top, bottom) = (to_y(k.open.max(k.close)), to_y(k.open.min(k.close)));
                    VectorPath::rectangle(x - body / 2., top, body, (bottom - top).max(1.))
                };
                // [rising, falling] wicks and bodies, each batched by colour.
                let mut wicks = [PathBatch::new(), PathBatch::new()];
                let mut bodies = [PathBatch::new(), PathBatch::new()];
                for (i, k) in candles.iter().enumerate() {
                    let side = usize::from(k.close < k.open);
                    let wick_x = ((i as f32 + 0.5) * slot).round() + 0.5;
                    wicks[side].add(
                        VectorPath::new()
                            .move_to(wick_x, to_y(k.high))
                            .line_to(wick_x, to_y(k.low)),
                    );
                    bodies[side].add(body_rect(i, k));
                }
                let [up_wicks, down_wicks] = wicks;
                let [up_bodies, down_bodies] = bodies;
                up_wicks.stroke(c, ColorToken::Success, 1.);
                down_wicks.stroke(c, ColorToken::Error, 1.);
                up_bodies.fill(c, ColorToken::Success);
                down_bodies.fill(c, ColorToken::Error);
                if let Some(i) = selected {
                    c.stroke(body_rect(i, &candles[i]), ColorToken::BaseContent, 1.5);
                    // The selected close as a dashed level across the plot.
                    let y = to_y(candles[i].close).round() + 0.5;
                    let mut dashes = PathBatch::new();
                    let mut x = 0.;
                    while x < w {
                        dashes.add(VectorPath::new().move_to(x, y).line_to((x + 4.).min(w), y));
                        x += 8.;
                    }
                    dashes.stroke(c, ColorToken::BaseContent.alpha(50), 1.);
                }
            },
        );
        if !k.volume {
            return chart;
        }
        let candles = k.candles;
        let max_v = candles
            .iter()
            .map(|c| c.volume)
            .fold(0f32, f32::max)
            .max(1.);
        let strip = canvas(move |c| {
            let slot = plot_w / n as f32;
            let body = (slot * 0.6).clamp(1., 18.);
            // [rising, falling] bars; the selected bar is drawn stronger.
            let mut bars = [PathBatch::new(), PathBatch::new()];
            let mut picked = None;
            for (i, k) in candles.iter().enumerate() {
                let h = k.volume / max_v * (vol_h - 4.);
                let x = (i as f32 + 0.5) * slot;
                let bar = VectorPath::rectangle(x - body / 2., vol_h - h, body, h.max(1.));
                let side = usize::from(k.close < k.open);
                if selected == Some(i) {
                    picked = Some((bar, side));
                } else {
                    bars[side].add(bar);
                }
            }
            let [up, down] = bars;
            up.fill(c, ColorToken::Success.alpha(45));
            down.fill(c, ColorToken::Error.alpha(45));
            if let Some((bar, side)) = picked {
                let token = [ColorToken::Success, ColorToken::Error][side];
                c.fill(bar, token.alpha(80));
            }
        })
        .width(plot_w)
        .height(vol_h);
        let mut parts = vec![chart, hstack![spacer().width(AXIS).flex(0.), strip].into()];
        if !trailing_labels.is_empty() {
            parts.push(x_label_row(trailing_labels, k.size[0]));
        }
        vstack(parts).gap(4., Axis::Vertical).into()
    }
}

// ── Donut ───────────────────────────────────────────────────────────────

/// Parts of a whole as a ring, with an optional centre label and legend.
pub struct DonutChart<C> {
    slices: Vec<(String, f32)>,
    size: f32,
    thickness: f32,
    center: Option<(String, String)>,
    legend: bool,
    format: Format,
    interact: Interact<C>,
}
pub fn donut_chart<C>(slices: impl IntoIterator<Item = (impl Into<String>, f32)>) -> DonutChart<C> {
    DonutChart {
        slices: slices
            .into_iter()
            .map(|(l, v)| (l.into(), v))
            .filter(|(_, v)| v.is_finite() && *v > 0.)
            .collect(),
        size: 180.,
        thickness: 26.,
        center: None,
        legend: true,
        format: short_number,
        interact: Interact::default(),
    }
}
interactive!(DonutChart);
impl<C> DonutChart<C> {
    pub fn size(mut self, diameter: f32) -> Self {
        self.size = diameter;
        self
    }
    pub fn thickness(mut self, value: f32) -> Self {
        self.thickness = value;
        self
    }
    /// A large value with a small caption in the hole, e.g. ("$48k", "Revenue").
    pub fn center(mut self, value: impl Into<String>, caption: impl Into<String>) -> Self {
        self.center = Some((value.into(), caption.into()));
        self
    }
    pub fn legend(mut self, value: bool) -> Self {
        self.legend = value;
        self
    }
    pub fn value_format(mut self, f: fn(f32) -> String) -> Self {
        self.format = f;
        self
    }
}
/// An annular sector from `a0` to `a1` radians (0 = 12 o'clock, clockwise).
fn sector(cx: f32, cy: f32, r: f32, inner: f32, a0: f32, a1: f32) -> VectorPath {
    let p = |rad: f32, a: f32| [cx + rad * a.sin(), cy - rad * a.cos()];
    let large = u8::from(a1 - a0 > std::f32::consts::PI);
    let (o0, o1, i1, i0) = (p(r, a0), p(r, a1), p(inner, a1), p(inner, a0));
    let d = format!(
        "M{} {} A{r} {r} 0 {large} 1 {} {} L{} {} A{inner} {inner} 0 {large} 0 {} {} Z",
        o0[0], o0[1], o1[0], o1[1], i1[0], i1[1], i0[0], i0[1]
    );
    VectorPath::from_svg(&d).unwrap_or_default()
}
/// Start and end angles of each slice (with a small gap between slices).
fn slice_angles(values: &[f32]) -> Vec<(f32, f32)> {
    let total: f32 = values.iter().sum();
    let gap = if values.len() > 1 { 0.025 } else { 0. };
    let mut a = 0.;
    values
        .iter()
        .map(|v| {
            let sweep = if total > 0. {
                v / total * std::f32::consts::TAU
            } else {
                0.
            };
            let span = (a + gap / 2., a + sweep - gap / 2.);
            a += sweep;
            span
        })
        .collect()
}
impl<C: 'static> From<DonutChart<C>> for Element<C> {
    fn from(d: DonutChart<C>) -> Self {
        let total: f32 = d.slices.iter().map(|(_, v)| v).sum();
        let (size, thick) = (d.size, d.thickness.min(d.size / 2.));
        let values: Vec<f32> = d.slices.iter().map(|(_, v)| *v).collect();
        let angles = slice_angles(&values);
        let selected = d.interact.selected.filter(|i| *i < values.len());
        // The selected slice is drawn at full radius; the others leave room.
        let pop = 5f32.min(size * 0.04);
        let ring_angles = angles.clone();
        let ring = canvas(move |c| {
            let (cx, cy, r) = (size / 2., size / 2., size / 2.);
            if total <= 0. {
                // A full turn's endpoints coincide (no arc); stop just short.
                c.fill(
                    sector(cx, cy, r, r - thick, 0., std::f32::consts::TAU - 1e-3),
                    ColorToken::Border,
                );
                return;
            }
            for (i, (a0, a1)) in ring_angles.iter().enumerate() {
                if a1 <= a0 {
                    continue;
                }
                let (outer, inner) = if selected == Some(i) {
                    (r, r - thick - pop)
                } else {
                    (r - pop, r - thick)
                };
                let color = series_color(i);
                let paint = match selected {
                    Some(s) if s != i => faded(color, 55),
                    _ => color.to_owned(),
                };
                c.fill(sector(cx, cy, outer, inner, *a0, *a1), paint);
            }
        })
        .width(size)
        .height(size);
        let mut layers: Vec<Element<C>> = vec![ring];
        let pct = |v: f32| if total > 0. { v / total * 100. } else { 0. };
        // Hit targets: a full-size button per slice, clipped to its sector.
        if let Some(key) = d.interact.key.as_deref() {
            for (i, ((label, v), (a0, a1))) in d.slices.iter().zip(&angles).enumerate() {
                if a1 <= a0 {
                    continue;
                }
                let tip = format!("{label}\n{} · {:.0}%", (d.format)(*v), pct(*v));
                let r = size / 2.;
                layers.push(
                    zstack![hit_target(
                        key,
                        i,
                        [size, size],
                        tip,
                        d.interact.on_select.clone()
                    )]
                    .key(format!("{key}-{i}-slice"))
                    .width(size)
                    .height(size)
                    .clip(Clip::Path(sector(r, r, r, r - thick - pop, *a0, *a1)))
                    .into(),
                );
            }
        }
        if let Some((value, caption)) = d.center {
            layers.push(
                vstack![
                    text(value)
                        .font_size((size * 0.13).max(14.))
                        .font_weight(700),
                    text(caption).font_size(11.).color(ColorToken::Muted),
                ]
                .width(size)
                .height(size)
                .flow(Flow::Center)
                .align(Align::Center)
                .gap(2., Axis::Vertical)
                .scroll_passthrough(true)
                .into(),
            );
        }
        // Never squeezed by a crowded row: hit sectors match the drawing.
        let donut: Element<C> = zstack(layers)
            .width(size)
            .height(size)
            .min_width(size)
            .min_height(size)
            .into();
        if !d.legend {
            return donut;
        }
        let rows: Vec<Element<C>> = d
            .slices
            .iter()
            .enumerate()
            .map(|(i, (label, v))| {
                let row: Element<C> = hstack![
                    vstack![]
                        .width(10.)
                        .height(10.)
                        .rounded(3.)
                        .background(series_color(i)),
                    text(label.clone())
                        .font_size(13.)
                        .font_weight(if selected == Some(i) { 600 } else { 400 })
                        .no_wrap(true)
                        .flex(1.),
                    text(format!("{:.0}%", pct(*v)))
                        .font_size(13.)
                        .font_weight(600)
                        .color(ColorToken::Muted),
                ]
                .gap(8., Axis::Horizontal)
                .align(Align::Center)
                .min_width(110.)
                .height(26.)
                .padding(4., Edge::Horizontal)
                .scroll_passthrough(true)
                .into();
                // Legend rows select their slice too.
                match (d.interact.key.as_deref(), d.interact.on_select.clone()) {
                    (Some(key), Some(f)) => zstack![
                        button("")
                            .key(format!("{key}-legend-{i}"))
                            .accessible_label(format!("{label}, {:.0}%", pct(*v)))
                            .variant(ButtonVariant::Ghost)
                            .height(26.)
                            .rounded(6.)
                            .on_click(move |app: &mut C| (f.borrow_mut())(app, i)),
                        row,
                    ]
                    .key(format!("{key}-legend-{i}-row"))
                    .height(26.)
                    .align(Align::Stretch)
                    .into(),
                    _ => row,
                }
            })
            .collect();
        hstack![donut, vstack(rows).gap(2., Axis::Vertical).flex(1.)]
            .gap(16., Axis::Horizontal)
            .align(Align::Center)
            .into()
    }
}

// ── Stacked bars ────────────────────────────────────────────────────────

/// Bars per category with series stacked on top of each other.
pub struct StackedBarChart<C> {
    categories: Vec<String>,
    series: Vec<(String, Vec<f32>)>,
    size: [f32; 2],
    legend: bool,
    format: Format,
    interact: Interact<C>,
}
pub fn stacked_bar_chart<C>(
    categories: impl IntoIterator<Item = impl Into<String>>,
    series: impl IntoIterator<Item = (impl Into<String>, Vec<f32>)>,
) -> StackedBarChart<C> {
    StackedBarChart {
        categories: categories.into_iter().map(Into::into).collect(),
        series: series.into_iter().map(|(n, v)| (n.into(), v)).collect(),
        size: [480., 240.],
        legend: true,
        format: short_number,
        interact: Interact::default(),
    }
}
interactive!(StackedBarChart);
impl<C> StackedBarChart<C> {
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = [width, height];
        self
    }
    pub fn legend(mut self, value: bool) -> Self {
        self.legend = value;
        self
    }
    pub fn value_format(mut self, f: fn(f32) -> String) -> Self {
        self.format = f;
        self
    }
}
impl<C: 'static> From<StackedBarChart<C>> for Element<C> {
    fn from(b: StackedBarChart<C>) -> Self {
        let n = b.categories.len();
        let value = |s: usize, i: usize| {
            b.series[s]
                .1
                .get(i)
                .copied()
                .filter(|v| v.is_finite() && *v > 0.)
                .unwrap_or(0.)
        };
        let totals: Vec<f32> = (0..n)
            .map(|i| (0..b.series.len()).map(|s| value(s, i)).sum())
            .collect();
        let hi = totals.iter().copied().fold(0f32, f32::max);
        let (ticks, range) = nice_ticks(0., hi.max(1.), 4);
        let height = b.size[1] - if b.legend { 26. } else { 0. };
        let plot_w = b.size[0] - AXIS;
        let plot_h = (height - X_LABELS).max(10.);
        let x_labels = spread_labels(&b.categories, plot_w, true);
        let names: Vec<String> = b.series.iter().map(|(n, _)| n.clone()).collect();
        let overlay = b.interact.key.as_deref().filter(|_| n > 0).map(|key| {
            let tips = (0..n)
                .map(|i| {
                    let mut tip = b.categories[i].clone();
                    for (s, name) in names.iter().enumerate() {
                        tip.push_str(&format!("\n{name}: {}", (b.format)(value(s, i))));
                    }
                    tip.push_str(&format!("\nTotal: {}", (b.format)(totals[i])));
                    tip
                })
                .collect();
            hit_columns(
                key,
                &vec![plot_w / n as f32; n],
                plot_h,
                tips,
                &b.interact.on_select,
            )
        });
        let series: Vec<Vec<f32>> = (0..b.series.len())
            .map(|s| (0..n).map(|i| value(s, i)).collect())
            .collect();
        let selected = b.interact.selected.filter(|i| *i < n);
        let chart = framed(
            b.size[0],
            height,
            range,
            ticks,
            b.format,
            x_labels,
            overlay,
            move |c, w, _h, to_y| {
                let slot = w / n.max(1) as f32;
                let bar = (slot * 0.6).min(40.);
                // Per series: [normal, faded]. Other columns step back while
                // one is selected.
                let mut batches: Vec<[PathBatch; 2]> = series
                    .iter()
                    .map(|_| [PathBatch::new(), PathBatch::new()])
                    .collect();
                for i in 0..n {
                    let x = (i as f32 + 0.5) * slot - bar / 2.;
                    let faded_column = usize::from(selected.is_some_and(|sel| sel != i));
                    let mut base = 0.;
                    for (s, values) in series.iter().enumerate() {
                        let v = values[i];
                        if v <= 0. {
                            continue;
                        }
                        let (top, bottom) = (to_y(base + v), to_y(base));
                        batches[s][faded_column].add(VectorPath::rectangle(
                            x,
                            top,
                            bar,
                            (bottom - top - 1.).max(1.),
                        ));
                        base += v;
                    }
                }
                for (s, [normal, dim]) in batches.into_iter().enumerate() {
                    let color = series_color(s);
                    normal.fill(c, color);
                    dim.fill(c, faded(color, 40));
                }
            },
        );
        if b.legend {
            vstack![chart, legend(&names)]
                .gap(10., Axis::Vertical)
                .into()
        } else {
            chart
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nice_ticks_are_round_and_cover_the_range() {
        let (t, r) = nice_ticks(3., 97., 4);
        assert_eq!(t, [0., 25., 50., 75., 100.]);
        assert_eq!(r, (0., 100.));
        let (t, _) = nice_ticks(0.12, 0.81, 4);
        assert!(t.first().unwrap() <= &0.12 && t.last().unwrap() >= &0.81);
        assert!(t.windows(2).all(|w| ((w[1] - w[0]) - 0.2).abs() < 1e-4));
        let (t, _) = nice_ticks(5., 5., 4);
        assert!(t.len() >= 2, "flat data still gets an axis");
    }
    #[test]
    fn short_numbers_read_well() {
        assert_eq!(short_number(0.), "0");
        assert_eq!(short_number(950.), "950");
        assert_eq!(short_number(1500.), "1.5k");
        assert_eq!(short_number(2_000_000.), "2M");
        assert_eq!(short_number(0.25), "0.25");
        assert_eq!(short_number(-12_500.), "-12.5k");
    }
    #[test]
    fn charts_build_and_reject_bad_data_quietly() {
        let e: Element<()> = vstack![
            area_chart([("A", vec![1., 3., 2.]), ("B", vec![2., 1., 4.])])
                .stacked(true)
                .x_labels(["Mon", "Tue", "Wed"]),
            candlestick_chart([
                Candle::new(10., 12., 9., 11.).volume(100.),
                Candle::new(11., 11.5, 8., 9.).volume(80.),
                Candle::new(f32::NAN, 1., 0., 1.),
            ])
            .volume(true),
            donut_chart([("Web", 60.), ("App", 30.), ("Zero", 0.)]).center("90", "Visits"),
            stacked_bar_chart(
                ["Q1", "Q2"],
                [("North", vec![3., 4.]), ("South", vec![2., -1.])]
            ),
            donut_chart(Vec::<(&str, f32)>::new()),
        ]
        .into();
        e.build().unwrap();
        let sector = sector(50., 50., 50., 30., 0., 1.5);
        assert!(sector.is_valid());
    }
}
