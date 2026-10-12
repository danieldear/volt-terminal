//! The landing page: an editorial, typography-led tour in seven screens —
//! hero, a live product shot, numbers, features, code with a live preview,
//! live theming and the component index — closed by a call to action.
//! Everything shown is a real component bound to app state, not a picture.
use super::{
    art::{icon, mark, orbit, wordmark, Icon},
    pages::{Page, PAGES},
    App, THEMES,
};
use volt_ui_kit::ui::{prelude::*, Button, Element as El, Text};

const SCREEN_GAP: f32 = 128.;
/// Equal columns: a flexible child starts at its natural width and siblings
/// that overflow shrink proportionally, so giving every column the same
/// oversized width yields equal shares. Stacked vertically, it clamps to the
/// available width.
const COLUMN: f32 = 4096.;

// Type system: an editorial serif for display, SF for reading and UI, a mono
// for labels and code. Lists fall back to whatever the machine has installed.
/// Apple's New York, cut for very large sizes.
pub const DISPLAY: &str =
    "New York Extra Large, New York Large, New York, Iowan Old Style, Charter, serif";
/// New York's large-heading cut, for section titles.
pub const HEADING: &str = "New York Large, New York, Iowan Old Style, Charter, serif";
const LABEL: &str = "SF Mono";
pub const CODE: &str = "JetBrainsMono NF, JetBrains Mono, SF Mono, Menlo, monospace";

/// Letter-spaced mono label: "02 — WHY VOLT".
fn eyebrow(label: &'static str) -> Text<App> {
    text(label)
        .font_family(LABEL)
        .font_size(11.)
        .font_weight(500)
        .letter_spacing(0.18)
        .color(ColorToken::Muted)
}
/// Two-line display heading: a bold sans line, then a sans lead-in with an
/// italic serif accent — the type pairing used across the page.
fn headline(first: &'static str, lead: &'static str, accent: &'static str) -> El<App> {
    let sans = |value: &'static str| {
        text(value)
            .font_family("SF")
            .font_size(56.)
            .font_weight(700)
    };
    let mut second: Vec<El<App>> = vec![];
    if !lead.is_empty() {
        second.push(sans(lead).into());
    }
    second.push(
        text(accent)
            .font_family(DISPLAY)
            .font_size(60.)
            .font_weight(400)
            .italic(true)
            .color(ColorToken::Primary)
            .into(),
    );
    vstack![
        sans(first),
        hstack(second).gap(18., Axis::Horizontal).align(Align::End)
    ]
    .gap(2., Axis::Vertical)
    .into()
}
fn lede(value: &'static str) -> Text<App> {
    text(value).font_size(18.).color(ColorToken::Muted)
}
/// A section opener: eyebrow, display headline and a short lede.
fn opener(
    eyebrow_text: &'static str,
    title: (&'static str, &'static str, &'static str),
    body: &'static str,
) -> El<App> {
    vstack![
        eyebrow(eyebrow_text),
        headline(title.0, title.1, title.2),
        lede(body)
    ]
    .gap(16., Axis::Vertical)
    .width(920.)
    .into()
}
/// Square, letter-spaced mono call to action: filled ink or a hairline.
fn cta(key: &'static str, label: &'static str, filled: bool) -> Button<App> {
    let b = button(label)
        .key(key)
        .font_family(LABEL)
        .font_size(12.)
        .font_weight(600)
        .letter_spacing(0.16)
        .height(50.)
        .rounded(2.)
        .border_width(1.);
    if filled {
        b.background(ColorToken::BaseContent)
            .color(ColorToken::Base100)
            .border_color(ColorToken::BaseContent)
            .hover_style(|s| s.background(ColorToken::BaseContent.alpha(85)))
    } else {
        b.background("#00000000")
            .color(ColorToken::BaseContent)
            .border_color(ColorToken::Border)
            .hover_style(|s| s.background(ColorToken::BaseContent.alpha(5)))
    }
}
/// Center a single line horizontally (the toolkit has no text alignment).
fn centered(child: impl Into<El<App>>) -> El<App> {
    let child: El<App> = child.into();
    hstack![spacer(), child, spacer()].into()
}

/// The full-bleed hero; it sits outside the page's padding.
pub fn landing_hero(app: &App) -> El<App> {
    hero(app)
}
/// Everything below the hero.
pub fn landing(app: &App) -> El<App> {
    vstack![
        ticker(),
        product(app),
        numbers(),
        features(),
        code(app),
        theming(app),
        index(),
        closing(),
    ]
    .gap(SCREEN_GAP, Axis::Vertical)
    .into()
}

// ── 1. Hero ─────────────────────────────────────────────────────────────

/// A small floating card built from real components: the hero shows what
/// Volt makes rather than decorating around it.
fn fragment(key: &'static str, body: El<App>, width: f32, drift: Ambient) -> El<App> {
    vstack![body]
        .key(key)
        .width(width)
        .padding(14., Edge::All)
        .rounded(12.)
        .background(ColorToken::Base100)
        .border_color(ColorToken::Border)
        .border_width(1.)
        .shadow(Shadow::new().color("#00000014").blur(28.).offset(0., 14.))
        .ambient(drift)
        .into()
}

/// Hero sizes per window width. Views do not know the window size, so each
/// tier is built and only the one matching the breakpoint is laid out.
#[derive(Clone, Copy, PartialEq)]
enum Tier {
    /// Below `Lg`: the sidebar is collapsed and the window may be narrow.
    Compact,
    /// `Lg`..`Xl`: sidebar shown, content around 780–1030 px.
    Medium,
    /// `Xl` and up.
    Full,
}
impl Tier {
    fn name(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Medium => "medium",
            Self::Full => "full",
        }
    }
    fn visible(self) -> Responsive<bool> {
        match self {
            Self::Compact => responsive(true).at(Breakpoint::Lg, false),
            Self::Medium => responsive(false)
                .at(Breakpoint::Lg, true)
                .at(Breakpoint::Xl, false),
            Self::Full => responsive(false).at(Breakpoint::Xl, true),
        }
    }
}

fn hero(app: &App) -> El<App> {
    vstack![
        hero_tier(app, Tier::Compact),
        hero_tier(app, Tier::Medium),
        hero_tier(app, Tier::Full),
    ]
    .key("hero-tiers")
    .align(Align::Stretch)
    .into()
}

fn hero_tier(_app: &App, tier: Tier) -> El<App> {
    let t = tier.name();
    let k = |name: &str| format!("{name}-{t}");
    let (mark_size, sans, serif, lead, height, orbit_size) = match tier {
        Tier::Compact => (60., 28., 31., 15., 560., 460.),
        Tier::Medium => (88., 34., 38., 17., 640., 560.),
        Tier::Full => (112., 42., 46., 18., 700., 640.),
    };
    let headline: El<App> = if tier == Tier::Compact {
        // Stacked: the two halves of the line never collide.
        vstack![
            centered(
                text("Native interfaces,")
                    .font_family("SF")
                    .font_size(sans)
                    .font_weight(700)
            ),
            centered(
                text("written in Rust.")
                    .font_family(DISPLAY)
                    .font_size(serif)
                    .font_weight(400)
                    .italic(true)
                    .color(ColorToken::Primary)
            ),
        ]
        .gap(2., Axis::Vertical)
        .into()
    } else {
        centered(
            hstack![
                text("Native interfaces,")
                    .font_family("SF")
                    .font_size(sans)
                    .font_weight(700),
                text("written in Rust.")
                    .font_family(DISPLAY)
                    .font_size(serif)
                    .font_weight(400)
                    .italic(true)
                    .color(ColorToken::Primary),
            ]
            .gap(14., Axis::Horizontal)
            .align(Align::End),
        )
    };
    let lead_copy: El<App> = if tier == Tier::Compact {
        text("Declare your interface as a function of your state. Volt lays it out, draws it on the GPU and speaks it to screen readers.")
            .font_size(lead)
            .line_height(1.45)
            .color(ColorToken::Muted)
            .text_align(TextAlign::Center)
            .max_width(460.)
            .into()
    } else {
        vstack![
            centered(
                text("Declare your interface as a function of your state.")
                    .font_size(lead)
                    .color(ColorToken::Muted)
            ),
            centered(
                text("Volt lays it out, draws it on the GPU and speaks it to screen readers.")
                    .font_size(lead)
                    .color(ColorToken::Muted)
            ),
        ]
        .into()
    };
    let news = cta(
        "hero-news",
        if tier == Tier::Compact {
            concat!(
                "NEW IN ",
                env!("CARGO_PKG_VERSION_MAJOR"),
                ".",
                env!("CARGO_PKG_VERSION_MINOR"),
                "  →"
            )
        } else {
            concat!(
                "NEW IN ",
                env!("CARGO_PKG_VERSION_MAJOR"),
                ".",
                env!("CARGO_PKG_VERSION_MINOR"),
                " — COMPONENTS · NATIVE APPS · GPU  →"
            )
        },
        false,
    )
    .key(k("hero-news"))
    .font_size(10.)
    .height(30.)
    // Solid, so the hairline orbit passes behind it, not through it.
    .background(ColorToken::Base100)
    .on_click(|app: &mut App| app.go(Page::Settings));
    let gap = |v: f32| -> El<App> {
        let scale = if tier == Tier::Compact { 0.7 } else { 1. };
        spacer().height(v * scale).flex(0.).into()
    };
    let copy = vstack![
        centered(news),
        gap(40.),
        centered(
            hstack![
                wordmark(mark_size, ColorToken::BaseContent),
                vstack![text("UI")
                    .font_family(LABEL)
                    .font_size(13.)
                    .font_weight(600)]
                .width(36.)
                .height(28.)
                .flow(Flow::Center)
                .border_color(ColorToken::Border)
                .border_width(1.),
            ]
            .gap(12., Axis::Horizontal)
        ),
        gap(36.),
        headline,
        gap(18.),
        centered(lead_copy),
        gap(34.),
        centered(
            hstack![
                cta("hero-browse", "BROWSE COMPONENTS", true)
                    .key(k("hero-browse"))
                    .width(if tier == Tier::Compact { 200. } else { 230. })
                    .on_click(|app: &mut App| app.go(Page::Button)),
                cta("hero-theming", "THEMING GUIDE", false)
                    .key(k("hero-theming"))
                    .width(if tier == Tier::Compact { 170. } else { 190. })
                    .on_click(|app: &mut App| app.go(Page::Theming)),
            ]
            .gap(12., Axis::Horizontal)
        ),
        gap(28.),
        centered(
            text(if tier == Tier::Compact {
                "MIT · MACOS · WINDOWS · LINUX"
            } else {
                "OPEN SOURCE · MIT      MACOS · WINDOWS · LINUX      NO WEBVIEW"
            })
            .font_family(LABEL)
            .font_size(10.)
            .letter_spacing(0.2)
            .color(ColorToken::Muted)
        ),
    ]
    .key(k("hero-copy"))
    .flow(Flow::Center);
    let mut layers: Vec<El<App>> = vec![
        // Two hairline orbits behind the copy, each with one slow accent arc.
        vstack![orbit(orbit_size, orbit_size * 0.47, 70., 210.)
            .key(k("orbit-outer"))
            .ambient(Ambient::spin().period_ms(90_000))]
        .key(k("orbit-outer-layer"))
        .flow(Flow::Center)
        .into(),
        vstack![orbit(orbit_size, orbit_size * 0.34, 50., 30.)
            .key(k("orbit-inner"))
            .ambient(Ambient::spin().period_ms(64_000).phase(0.5))]
        .key(k("orbit-inner-layer"))
        .flow(Flow::Center)
        .into(),
    ];
    // Floating fragments need room beside the copy: widest tier only.
    if tier == Tier::Full {
        layers.extend(fragments());
    }
    layers.push(copy.into());
    // Full bleed for the orbits, but the floating cards stay near the copy
    // on very wide windows.
    let stage = zstack(layers)
        .key(k("hero"))
        .height(height)
        .align(Align::Stretch)
        .width_fraction(1.)
        .max_width(1400.);
    vstack![stage]
        .key(k("hero-frame"))
        .align(Align::Center)
        .clip(Clip::Bounds)
        .visible(tier.visible())
        .into()
}

/// Small floating cards built from real components (widest hero only).
fn fragments() -> Vec<El<App>> {
    let label = |value: &'static str| {
        text(value)
            .font_family(LABEL)
            .font_size(10.)
            .letter_spacing(0.16)
            .color(ColorToken::Muted)
    };
    let build = fragment(
        "fragment-build",
        vstack![
            hstack![label("CARGO BUILD"), spacer(), label("72%")],
            progress(0.72).key("fragment-build-bar").height(4.),
        ]
        .gap(10., Axis::Vertical)
        .into(),
        190.,
        Ambient::float(6., 4.).period_ms(11_000),
    );
    let metric = fragment(
        "fragment-metric",
        vstack![
            label("SAMPLE FRAME TIME"),
            hstack![
                text("3.7").font_size(26.).font_weight(600),
                text("ms").font_size(13.).color(ColorToken::Muted),
            ]
            .gap(4., Axis::Horizontal)
            .align(Align::End),
            canvas(|c| {
                let points = [14., 11., 12., 8., 9., 6., 7., 5., 6., 4.];
                let mut path = VectorPath::new().move_to(0., points[0]);
                for (i, y) in points.iter().enumerate().skip(1) {
                    path = path.line_to(i as f32 * 16., *y);
                }
                c.stroke(path, ColorToken::Primary, 1.5);
            })
            .key("fragment-spark")
            .width(150.)
            .height(18.),
        ]
        .gap(6., Axis::Vertical)
        .into(),
        180.,
        Ambient::float(5., 6.).period_ms(13_000).phase(0.4),
    );
    let status = fragment(
        "fragment-status",
        hstack![
            vstack![]
                .width(7.)
                .height(7.)
                .rounded(Rounding::Full)
                .background(ColorToken::Success),
            text("Native rendering demo")
                .font_size(12.)
                .font_weight(500),
        ]
        .gap(8., Axis::Horizontal)
        .align(Align::Center)
        .into(),
        176.,
        Ambient::float(7., 3.).period_ms(9_500).phase(0.75),
    );
    let place = |child: El<App>, key: &'static str, flow: Flow, x: f32, y: f32| -> El<App> {
        let horizontal = if matches!(flow, Flow::TopRight | Flow::Right | Flow::BottomRight) {
            Edge::Right
        } else {
            Edge::Left
        };
        let vertical = if matches!(flow, Flow::BottomLeft | Flow::Bottom | Flow::BottomRight) {
            Edge::Bottom
        } else {
            Edge::Top
        };
        vstack![child]
            .key(key)
            .flow(flow)
            .padding(x, horizontal)
            .padding(y, vertical)
            .into()
    };
    vec![
        place(build, "fragment-build-layer", Flow::TopLeft, 56., 70.),
        place(metric, "fragment-metric-layer", Flow::Right, 64., 0.),
        place(status, "fragment-status-layer", Flow::BottomLeft, 96., 64.),
    ]
}

// ── Ticker: a slow, swaying strip of what Volt is ───────────────────────

fn ticker() -> El<App> {
    let words = [
        "DECLARATIVE",
        "GPU RENDERED",
        "ACCESSIBLE",
        "DAMAGE TRACKED",
        "THEMEABLE",
        "NATIVE MENUS",
        "FILE PANELS",
        "NOTIFICATIONS",
        "100% RUST",
        "NO WEBVIEW",
    ];
    let mut row: Vec<El<App>> = vec![];
    for (i, word) in words.iter().cycle().take(20).enumerate() {
        row.push(
            text(*word)
                .key(format!("tick-{i}"))
                .font_family(LABEL)
                .font_size(12.)
                .letter_spacing(0.22)
                .color(ColorToken::Muted)
                .into(),
        );
        row.push(
            text("·")
                .key(format!("tick-star-{i}"))
                .font_size(14.)
                .color(ColorToken::Muted)
                .into(),
        );
    }
    vstack![hstack(row)
        .key("ticker-row")
        .gap(22., Axis::Horizontal)
        .align(Align::Center)
        .ambient(Ambient::float(180., 0.).period_ms(40_000))]
    .key("ticker")
    .height(52.)
    .padding(15., Edge::Vertical)
    .border_color(ColorToken::Border)
    .border_width(1.)
    // A horizontal scroller measures the strip at its natural width (a plain
    // row would squeeze every word to fit) and clips it; bars stay hidden.
    .scroll(Scroll::Horizontal)
    .scrollbars(Scrollbars::Hidden)
    .into()
}

// ── 2. Product shot: a live miniature app ───────────────────────────────

fn product(app: &App) -> El<App> {
    let nav = |label: &'static str, active: bool| -> El<App> {
        text(label)
            .font_size(13.)
            .font_weight(if active { 600 } else { 400 })
            .color(if active {
                ColorToken::BaseContent
            } else {
                ColorToken::Muted
            })
            .height(30.)
            .into()
    };
    let sidebar = vstack![
        hstack![mark(20.), text("Northwind").font_size(14.).font_weight(700)]
            .gap(8., Axis::Horizontal)
            .align(Align::Center),
        spacer().height(10.).flex(0.),
        nav("Overview", true),
        nav("Deployments", false),
        nav("Analytics", false),
        nav("Settings", false),
    ]
    .width(190.)
    .padding(18., Edge::All)
    .gap(2., Axis::Vertical)
    .rounded_on(16., [Corner::BottomLeft])
    .background(ColorToken::Base200);
    let stat = |label: &'static str, value: String, delta: &'static str| -> El<App> {
        vstack![
            text(label).font_size(12.).color(ColorToken::Muted),
            text(value).font_size(26.).font_weight(700),
            text(delta).font_size(12.).color(ColorToken::Success),
        ]
        .gap(4., Axis::Vertical)
        .padding(16., Edge::All)
        .flex(1.)
        .rounded(12.)
        .border_color(ColorToken::Border)
        .border_width(1.)
        .into()
    };
    let controls = vstack![
        hstack![
            vstack![
                text("Auto-deploy").font_size(14.).font_weight(600),
                text("Ship every push to main.")
                    .font_size(12.)
                    .color(ColorToken::Muted),
            ]
            .flex(1.),
            toggle("", app.switch_on)
                .key("shot-deploy")
                .accessible_label("Auto-deploy")
                .width(48.)
                .on_change(|app: &mut App, v| app.switch_on = v),
        ]
        .align(Align::Center),
        divider().background(ColorToken::Border),
        hstack![
            text("Traffic to canary")
                .font_size(14.)
                .font_weight(600)
                .flex(1.),
            text(format!("{:.0}%", app.volume * 100.))
                .font_size(13.)
                .color(ColorToken::Muted),
        ]
        .align(Align::Center),
        slider(app.volume)
            .key("shot-canary")
            .accessible_label("Traffic to canary")
            .on_change(|app: &mut App, v| app.volume = v),
        segmented_control([(0u8, "Day"), (1, "Week"), (2, "Month")])
            .key("shot-range")
            .selected(app.segment.min(2))
            .on_select(|app: &mut App, v| app.segment = v),
    ]
    .gap(14., Axis::Vertical)
    .padding(18., Edge::All)
    .rounded(12.)
    .border_color(ColorToken::Border)
    .border_width(1.);
    let requests = 1240 + (app.volume * 860.) as u32;
    let main = vstack![
        hstack![
            text("Overview").font_size(20.).font_weight(700).flex(1.),
            badge(if app.switch_on { "Live" } else { "Paused" })
                .rounded(Rounding::Full)
                .background(if app.switch_on {
                    ColorToken::Success.alpha(16)
                } else {
                    ColorToken::BaseContent.alpha(8)
                })
                .color(if app.switch_on {
                    ColorToken::Success
                } else {
                    ColorToken::Muted
                }),
        ]
        .align(Align::Center),
        hstack![
            stat("Requests / min", format!("{requests}"), "+12.4%"),
            stat("p95 latency", "38 ms".into(), "−6 ms"),
            stat("Error rate", "0.02%".into(), "−0.01%"),
        ]
        .gap(12., Axis::Horizontal),
        controls,
    ]
    .flex(1.)
    .gap(16., Axis::Vertical)
    .padding(24., Edge::All);
    let window = vstack![
        hstack![
            dot("#FF5F57"),
            dot("#FEBC2E"),
            dot("#28C840"),
            spacer(),
            text("northwind — Volt UI")
                .font_size(12.)
                .color(ColorToken::Muted),
            spacer(),
            spacer().width(52.).flex(0.),
        ]
        .gap(8., Axis::Horizontal)
        .align(Align::Center)
        .height(38.)
        .padding(14., Edge::Horizontal),
        divider().background(ColorToken::Border),
        hstack![sidebar, main].align(Align::Stretch),
    ]
    .key("product-shot")
    .accessible_label("Live example application")
    // Rounded without a clip layer: only the sidebar fill reaches a corner,
    // and it rounds its own.
    .rounded(16.)
    .background(ColorToken::Base100)
    .border_color(ColorToken::Border)
    .border_width(1.)
    .shadow(Shadow::new().color("#00000040").blur(40.).offset(0., 24.));
    vstack![
        hstack![
            eyebrow("01 — LIVE, NOT A SCREENSHOT"),
            spacer(),
            text("Flip the switch, drag the slider.")
                .font_size(13.)
                .color(ColorToken::Muted),
        ]
        .align(Align::Center),
        window,
    ]
    .key("product")
    .gap(16., Axis::Vertical)
    .into()
}

fn dot(color: &'static str) -> El<App> {
    vstack![]
        .width(12.)
        .height(12.)
        .rounded(Rounding::Full)
        .background(color)
        .into()
}

// ── 3. Numbers ──────────────────────────────────────────────────────────

fn numbers() -> El<App> {
    let documented = PAGES.len().to_string();
    let figure = |value: String, label: &'static str| -> El<App> {
        vstack![
            text(value)
                .font_family("SF")
                .font_size(84.)
                .font_weight(200),
            text(label.to_uppercase())
                .font_family(LABEL)
                .font_size(11.)
                .letter_spacing(0.14)
                .color(ColorToken::Muted),
        ]
        .gap(4., Axis::Vertical)
        .width(COLUMN)
        .padding(20., Edge::Left)
        .border_color(ColorToken::Border)
        .border_width(0.)
        .into()
    };
    stack([
        figure(documented, "documented pages, each with live previews"),
        figure("8K".into(), "Retina displays rendered on the GPU"),
        figure("0".into(), "webviews, browsers or JavaScript"),
        figure("100%".into(), "Rust, with one declarative API"),
    ])
    .key("numbers")
    .direction(responsive(Direction::Vertical).at(Breakpoint::Lg, Direction::Horizontal))
    .gap(24., Axis::Both)
    .into()
}

// ── 4. Features ─────────────────────────────────────────────────────────

fn features() -> El<App> {
    let feature = |glyph: Icon, title: &'static str, body: &'static str| -> El<App> {
        vstack![
            zstack![icon(glyph, 20., ColorToken::Primary)]
                .width(40.)
                .height(40.)
                .flow(Flow::Center)
                .rounded(10.)
                .background(ColorToken::Primary.alpha(12)),
            text(title).font_size(17.).font_weight(600),
            text(body).font_size(14.).color(ColorToken::Muted),
        ]
        .gap(12., Axis::Vertical)
        .padding(24., Edge::All)
        .width(COLUMN)
        .rounded(16.)
        .background(ColorToken::Base200.alpha(60))
        .border_color(ColorToken::Border)
        .border_width(1.)
        .into()
    };
    let row = |cells: [El<App>; 3]| -> El<App> {
        stack(cells)
            .direction(responsive(Direction::Vertical).at(Breakpoint::Lg, Direction::Horizontal))
            .gap(16., Axis::Both)
            .into()
    };
    vstack![
        opener(
            "02 — WHY VOLT",
            ("Everything a desktop app", "", "needs, built in."),
            "A small, auditable toolkit: layout, input, text, accessibility and rendering in one Rust crate with no unsafe code."
        ),
        row([
            feature(Icon::Layers, "Declarative", "Rebuild your view from state on every change; stable keys preserve input state, and software damage tracking limits repaint work."),
            feature(Icon::Chip, "GPU rendered", "Vector shapes and text drawn by Vello on Metal, Vulkan or DX12, with strips for 8K displays and a software fallback."),
            feature(Icon::Person, "Accessible", "A real screen-reader tree with roles, values and actions, plus full keyboard focus and reduced motion."),
        ]),
        row([
            feature(Icon::Palette, "Themeable", "Semantic colour roles, light and dark palettes and translucent tokens restyle every component at once."),
            feature(Icon::Window, "Truly native", "Unified title bars, frosted materials, menus, tray, file panels and notifications on macOS."),
            feature(Icon::Bolt, "Fast to start", "cargo volt new scaffolds an app; cargo volt dev rebuilds and restarts on save."),
        ]),
    ]
    .key("features")
    .gap(36., Axis::Vertical)
    .into()
}

// ── 5. Code with a live preview ─────────────────────────────────────────

const COUNTER: &str = r#"struct App { count: u32 }

fn view(app: &App) -> Element<App> {
    vstack![
        text(format!("Clicked {} times", app.count)),
        button("Click me")
            .key("increment")
            .variant(ButtonVariant::Primary)
            .on_click(|app: &mut App| app.count += 1),
    ]
    .gap(12., Axis::Vertical)
    .into()
}"#;

fn code(app: &App) -> El<App> {
    let checks = [
        "Plain functions and structs: no macros to learn, no lifetimes in views",
        "Stable keys keep focus, scroll and animation across rebuilds",
        "Every callback receives your state mutably, on the UI thread",
    ]
    .into_iter()
    .enumerate()
    .map(|(i, line)| -> El<App> {
        hstack![
            icon(Icon::Check, 16., ColorToken::Success),
            text(line).font_size(15.).flex(1.),
        ]
        .key(format!("code-point-{i}"))
        .gap(10., Axis::Horizontal)
        .into()
    });
    let left = vstack![
        eyebrow("03 — DECLARATIVE"),
        headline("Your interface is", "a function", "of state."),
        lede("Write the view once. Change state in a callback, and Volt works out what to redraw."),
        vstack(checks).gap(12., Axis::Vertical),
    ]
    .gap(14., Axis::Vertical)
    .width(COLUMN);
    let count = app.counter;
    let preview = vstack![
        text(format!(
            "Clicked {count} time{}",
            if count == 1 { "" } else { "s" }
        ))
        .font_size(15.),
        button("Click me")
            .key("landing-counter")
            .variant(ButtonVariant::Primary)
            .on_click(|app: &mut App| app.counter = app.counter.saturating_add(1)),
    ]
    .key("counter-preview")
    .gap(12., Axis::Vertical)
    .padding(20., Edge::All)
    .background(ColorToken::Base100)
    .border_color(ColorToken::Border)
    .border_width(1.)
    .rounded(12.);
    let right = vstack![
        hstack![
            icon(Icon::Code, 16., "#A1A1AA"),
            text("counter.rs")
                .font_family(CODE)
                .font_size(12.)
                .color("#A1A1AA"),
        ]
        .gap(8., Axis::Horizontal)
        .align(Align::Center),
        text(COUNTER)
            .font_family(CODE)
            .font_size(13.)
            .color("#E4E4E7"),
        preview,
    ]
    .key("code-card")
    .gap(16., Axis::Vertical)
    .padding(22., Edge::All)
    .width(COLUMN)
    .rounded(16.)
    .background("#0B0B10")
    .border_color("#27272A")
    .border_width(1.);
    stack([left.into(), right.into()])
        .key("code")
        .direction(responsive(Direction::Vertical).at(Breakpoint::Xl, Direction::Horizontal))
        .gap(48., Axis::Both)
        .align(Align::Center)
        .into()
}

// ── 6. Live theming ─────────────────────────────────────────────────────

fn theming(app: &App) -> El<App> {
    let swatches: Vec<El<App>> = THEMES
        .iter()
        .enumerate()
        .map(|(i, (name, light, dark))| -> El<App> {
            let dark_mode = app.mode == ThemeMode::Dark;
            let rgb = if dark_mode { *dark } else { *light };
            let selected = app.theme == i;
            zstack![button("")
                .key(format!("swatch-{}", name.to_lowercase()))
                .accessible_label(format!("{name} accent"))
                .width(44.)
                .height(44.)
                .rounded(Rounding::Full)
                .background(format!("#{rgb:06X}"))
                .border_color(if selected {
                    ColorToken::BaseContent
                } else {
                    ColorToken::Border
                })
                .border_width(if selected { 3. } else { 1. })
                .on_click(move |app: &mut App| app.theme = i),]
            .key(format!("swatch-{}-layer", name.to_lowercase()))
            .width(44.)
            .height(44.)
            .into()
        })
        .collect();
    let mode = segmented_control([(false, "Light"), (true, "Dark")])
        .key("landing-mode")
        .selected(app.mode == ThemeMode::Dark)
        .on_select(|app: &mut App, dark| {
            app.mode = if dark {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            }
        });
    let sample = vstack![
        hstack![
            text("Pro plan").font_size(18.).font_weight(700).flex(1.),
            badge("Popular")
                .rounded(Rounding::Full)
                .background(ColorToken::Primary.alpha(14))
                .color(ColorToken::Primary),
        ]
        .align(Align::Center),
        hstack![
            text("$24")
                .font_family(DISPLAY)
                .font_size(48.)
                .font_weight(500),
            text("/ month").font_size(14.).color(ColorToken::Muted),
        ]
        .gap(6., Axis::Horizontal)
        .align(Align::End),
        progress(0.68).key("plan-usage").height(8.),
        text("68% of build minutes used")
            .font_size(12.)
            .color(ColorToken::Muted),
        checkbox("Email me usage reports", app.checked)
            .key("plan-reports")
            .on_change(|app: &mut App, v| app.checked = v),
        hstack![
            button("Upgrade")
                .key("plan-upgrade")
                .variant(ButtonVariant::Primary)
                .flex(1.)
                .on_click(|app: &mut App| app.notify("This is a theming preview")),
            button("Compare")
                .key("plan-compare")
                .variant(ButtonVariant::Outline)
                .on_click(|app: &mut App| app.go(Page::Theming)),
        ]
        .gap(10., Axis::Horizontal),
    ]
    .key("theme-sample")
    .gap(14., Axis::Vertical)
    .padding(24., Edge::All)
    .width(360.)
    .rounded(16.)
    .background(ColorToken::Base100)
    .border_color(ColorToken::Border)
    .border_width(1.)
    .shadow(Shadow::new().color("#0000001F").blur(28.).offset(0., 12.));
    let left = vstack![
        eyebrow("04 — THEMING"),
        headline("One palette", "restyles", "everything."),
        lede("Components draw with semantic roles, never literal colours. Pick an accent or a mode — this whole site follows."),
        hstack(swatches).gap(12., Axis::Horizontal),
        hstack![mode, spacer()],
    ]
    .gap(16., Axis::Vertical)
    .flex(1.);
    stack([left.into(), sample.into()])
        .key("theming")
        .direction(responsive(Direction::Vertical).at(Breakpoint::Xl, Direction::Horizontal))
        .gap(48., Axis::Both)
        .align(Align::Center)
        .into()
}

// ── 7. Component index and closing ──────────────────────────────────────

fn index() -> El<App> {
    let pages: Vec<Page> = PAGES
        .into_iter()
        .filter(|p| *p != Page::Introduction)
        .collect();
    let card = |page: Page| -> El<App> {
        let key = format!("card-{}", page.slug());
        zstack![
            button("")
                .key(key.clone())
                .accessible_label(format!("Open {}", page.title()))
                .variant(ButtonVariant::Outline)
                .rounded(14.)
                .hover_style(|s| s
                    .background(ColorToken::Base200)
                    .border_color(ColorToken::Muted))
                .animate_on(Interaction::Hover, Animation::ease_out().duration_ms(140))
                .on_click(move |app: &mut App| app.go(page)),
            vstack![
                hstack![
                    text(page.title()).font_size(15.).font_weight(600),
                    spacer(),
                    if page.is_new() {
                        El::from(
                            badge("New")
                                .font_size(10.)
                                .rounded(Rounding::Full)
                                .background(ColorToken::Primary.alpha(14))
                                .color(ColorToken::Primary),
                        )
                    } else {
                        icon(Icon::Arrow, 14., ColorToken::Muted)
                    },
                ]
                .align(Align::Center),
                text(page.summary()).font_size(13.).color(ColorToken::Muted),
            ]
            .padding(18., Edge::All)
            .gap(6., Axis::Vertical),
        ]
        .key(format!("{key}-layer"))
        .flow(Flow::TopLeft)
        .align(Align::Stretch)
        .flex(1.)
        .height(108.)
        .into()
    };
    let rows: Vec<El<App>> = pages
        .chunks(3)
        .map(|chunk| {
            let mut cells: Vec<El<App>> = chunk.iter().map(|p| card(*p)).collect();
            while cells.len() < 3 {
                cells.push(spacer().into());
            }
            stack(cells)
                .direction(
                    responsive(Direction::Vertical).at(Breakpoint::Lg, Direction::Horizontal),
                )
                .gap(12., Axis::Both)
                .into()
        })
        .collect();
    vstack![
        opener(
            "05 — COMPONENTS",
            ("Every component,", "", "documented."),
            "Each page has live previews, the Rust that produced them and an API reference."
        ),
        vstack(rows).gap(12., Axis::Vertical),
    ]
    .key("index")
    .gap(36., Axis::Vertical)
    .into()
}

fn closing() -> El<App> {
    vstack![
        eyebrow("06 — GET STARTED"),
        hstack![
            wordmark(72., ColorToken::BaseContent),
            text("Build something")
                .font_family("SF")
                .font_size(56.)
                .font_weight(700),
        ]
        .gap(28., Axis::Horizontal)
        .align(Align::Center),
        text("native.")
            .font_family(DISPLAY)
            .font_size(96.)
            .font_weight(400)
            .italic(true)
            .color(ColorToken::Primary),
        lede("Start with a template, or open any component and copy its source."),
        hstack![
            cta("closing-start", "GET STARTED", true)
                .width(200.)
                .on_click(|app: &mut App| app.go(Page::Layout)),
            cta("closing-patterns", "SEE THE PATTERNS", false)
                .width(220.)
                .on_click(|app: &mut App| app.go(Page::SignIn)),
        ]
        .gap(12., Axis::Horizontal),
        spacer().height(48.).flex(0.),
        divider().background(ColorToken::Border),
        hstack![
            mark(20.),
            text(concat!(
                "VOLT UI ",
                env!("CARGO_PKG_VERSION_MAJOR"),
                ".",
                env!("CARGO_PKG_VERSION_MINOR")
            ))
            .font_family(LABEL)
            .font_size(11.)
            .letter_spacing(0.18),
            spacer(),
            text("MIT LICENSED · RENDERED BY VELLO · BUILT IN RUST")
                .font_family(LABEL)
                .font_size(11.)
                .letter_spacing(0.18)
                .color(ColorToken::Muted),
        ]
        .gap(10., Axis::Horizontal)
        .align(Align::Center),
    ]
    .key("closing")
    .gap(22., Axis::Vertical)
    .padding(24., Edge::Top)
    .into()
}
