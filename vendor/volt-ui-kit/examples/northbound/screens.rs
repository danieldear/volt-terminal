//! Northbound screens. Each view is a pure function of `App`.
use super::kit::stat;
use super::{
    icons::{icon, mark, Icon},
    kit::*,
    App, Filter, Leg, Mood, Screen, Transport, Units, ACCENTS, PACKING, PLACES,
};
use volt_ui_kit::ui::{prelude::*, Container};

const DAYS: [(&str, &str, Icon); 14] = [
    (
        "Land in Leknes",
        "Pick up the car, drive the E10 west to Reine",
        Icon::Pin,
    ),
    (
        "Reinebringen at sunrise",
        "1,566 stone steps; go before the tour buses",
        Icon::Sun,
    ),
    (
        "Kayak the Reinefjord",
        "Half-day guided paddle from the harbour",
        Icon::Ferry,
    ),
    (
        "Hamnøy and Sakrisøy",
        "Photograph the cabins, then fish cakes at Anita's",
        Icon::Heart,
    ),
    (
        "Ferry to Andenes",
        "Coastal ferry through the Raftsund strait",
        Icon::Ferry,
    ),
    (
        "Whale safari",
        "Sperm whales feed in the canyon off the shelf",
        Icon::Compass,
    ),
    (
        "Bleik beach walk",
        "White sand, puffin island on the horizon",
        Icon::Cloud,
    ),
    (
        "Drive to Senja",
        "The scenic route over Gryllefjord",
        Icon::Pin,
    ),
    (
        "Segla ridge hike",
        "Steep, exposed, unforgettable; check the wind",
        Icon::Sun,
    ),
    (
        "Aurora at Ersfjord",
        "Clear skies forecast after 22:00",
        Icon::Aurora,
    ),
    (
        "Tromsø old town",
        "Polar museum, cable car to Fløya",
        Icon::Compass,
    ),
    (
        "Drive the E6 north",
        "Alta rock carvings on the way",
        Icon::Pin,
    ),
    (
        "Honningsvåg harbour",
        "King crab dinner, early night",
        Icon::Bed,
    ),
    ("North Cape", "Stay up for the sun at midnight", Icon::Sun),
];
const LEGS: [(Leg, &str, std::ops::Range<usize>, usize); 4] = [
    (Leg::Lofoten, "Lofoten", 0..4, 0),
    (Leg::Vesteralen, "Vesterålen", 4..7, 2),
    (Leg::Troms, "Troms", 7..11, 3),
    (Leg::Finnmark, "Finnmark", 11..14, 5),
];
/// Approximate daylight hours in Tromsø, January to December.
const DAYLIGHT: [f32; 12] = [1.5, 6.5, 11., 15.5, 21.5, 24., 24., 18.5, 13.5, 9., 3.5, 0.];

fn nok(value: u32) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    format!("NOK {out}")
}

fn count(n: usize, noun: &str) -> String {
    let consonant_y = noun.ends_with('y') && !noun.ends_with("ay") && !noun.ends_with("ey");
    match n {
        1 => format!("1 {noun}"),
        _ if consonant_y => format!("{n} {}ies", &noun[..noun.len() - 1]),
        _ => format!("{n} {noun}s"),
    }
}

pub fn view(app: &App) -> Element<App> {
    let page = match app.screen {
        Screen::Route => route(app),
        Screen::Days => days(app),
        Screen::Journal => journal(app),
        Screen::Settings => settings(app),
    };
    let main = vstack![
        top_bar(app),
        vstack![page]
            .key(format!("page-{}", app.screen.slug()))
            .transition(Transition::fade().duration_ms(240))
            .flex(1.)
            .padding(responsive(18.).at(Breakpoint::Lg, 32.), Edge::Horizontal)
            .padding(4., Edge::Top)
            .padding(36., Edge::Bottom)
            .gap(28., Axis::Vertical)
            .scroll(Scroll::Vertical),
    ]
    .flex(1.);
    zstack![
        hstack![rail(app), main].align(Align::Stretch),
        toast_layer(app),
        reserve_dialog(app),
        reset_dialog(app),
    ]
    .key("northbound")
    .align(Align::Stretch)
    .font_family(SANS)
    .font_size(14.)
    .color(ColorToken::BaseContent)
    .background(ColorToken::Base100)
    .into()
}

fn rail(app: &App) -> Element<App> {
    let mut items: Vec<Element<App>> = vec![
        spacer().height(34.).flex(0.).into(),
        mark(34.),
        spacer().height(14.).flex(0.).into(),
    ];
    for screen in Screen::ALL {
        let glyph = match screen {
            Screen::Route => Icon::Compass,
            Screen::Days => Icon::Calendar,
            Screen::Journal => Icon::Journal,
            Screen::Settings => Icon::Sliders,
        };
        let active = app.screen == screen;
        items.push(
            vstack![
                icon_button(
                    glyph,
                    format!("nav-{}", screen.slug()),
                    44.,
                    active,
                    move |app| {
                        app.screen = screen;
                        app.transport_open = false;
                        app.units_open = false;
                    }
                ),
                text(screen.title())
                    .font_size(11.)
                    .font_weight(if active { 600 } else { 500 })
                    .color(if active {
                        ColorToken::BaseContent
                    } else {
                        ColorToken::Muted
                    }),
            ]
            .align(Align::Center)
            .gap(5., Axis::Vertical)
            .into(),
        );
    }
    let light = app.mode == ThemeMode::Light;
    items.push(spacer().into());
    items.push(icon_button(
        if light { Icon::Moon } else { Icon::Sun },
        "rail-theme",
        40.,
        false,
        move |app| {
            app.mode = if light {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            };
        },
    ));
    items.push(
        panel![text("AS")
            .font_size(12.)
            .font_weight(700)
            .color(ColorToken::SecondaryContent)]
        .width(36.)
        .height(36.)
        .padding(0., Edge::All)
        .flow(Flow::Center)
        .background(ColorToken::Secondary)
        .border_width(0.)
        .rounded(Rounding::Full)
        .into(),
    );
    vstack(items)
        .key("rail")
        .width(84.)
        .padding(18., Edge::Bottom)
        .gap(14., Axis::Vertical)
        .align(Align::Center)
        .background(ColorToken::Base200)
        .drag_region(true)
        .into()
}

fn top_bar(app: &App) -> Element<App> {
    let (eyebrow, title) = match app.screen {
        Screen::Route => ("Two weeks, Lofoten to the North Cape", "Your route north"),
        Screen::Days => ("Fourteen days, four regions", "Day by day"),
        Screen::Journal => ("Write it down before it fades", "Field journal"),
        Screen::Settings => ("Appearance and travel preferences", "Settings"),
    };
    let done = app.done_days.iter().filter(|d| **d).count();
    let mut bar: Vec<Element<App>> = vec![
        // Left keeps the pair vertically centred in the bar without also
        // centring the shorter title under the eyebrow (Center does both).
        vstack![small(eyebrow), display(title, 26.)]
            .flow(Flow::Left)
            .gap(2., Axis::Vertical)
            .into(),
        spacer().into(),
    ];
    if app.screen == Screen::Route {
        bar.push(
            zstack![text_field(&app.query)
                .key("search")
                .placeholder("Search places")
                .width(240.)
                .height(40.)
                .rounded(10.)
                .background(ColorToken::Base200)
                .border_color(ColorToken::Border)
                .on_change(|app: &mut App, value| app.query = value),]
            .into(),
        );
    }
    bar.push(
        chip(
            format!("Day {} of 14", done + 1),
            ColorToken::Base300,
            ColorToken::BaseContent,
        )
        .into(),
    );
    toolbar(bar)
        .key("top-bar")
        .height(92.)
        .padding(responsive(18.).at(Breakpoint::Lg, 32.), Edge::Horizontal)
        .padding(18., Edge::Top)
        .gap(14., Axis::Horizontal)
        .drag_region(true)
        .into()
}

// ---------------------------------------------------------------- Route

fn route(app: &App) -> Element<App> {
    let filter = segmented_control([
        (Filter::All, "All"),
        (Filter::Peaks, "Peaks"),
        (Filter::Coast, "Coast"),
        (Filter::Night, "Night sky"),
    ])
    .key("filter")
    .selected(app.filter)
    .on_select(|app: &mut App, f| app.filter = f)
    .height(34.)
    .font_size(13.)
    .rounded(10.);
    let visible = app.visible_places();
    let strip: Element<App> = if visible.is_empty() {
        card(vec![
            heading("No stops match").into(),
            muted("Try another name, or show every stop on the route.").into(),
            quiet("Show all stops", "clear-search", |app| {
                app.query.clear();
                app.filter = Filter::All;
            })
            .width(160.)
            .into(),
        ])
        .into()
    } else {
        hstack(
            visible
                .into_iter()
                .map(|i| place_card(app, i))
                .collect::<Vec<_>>(),
        )
        .key("places")
        .gap(16., Axis::Horizontal)
        .padding(6., Edge::Vertical)
        .scroll(Scroll::Horizontal)
        .scrollbars(Scrollbars::Hidden)
        .into()
    };
    vstack![
        hero(app),
        section_header("Along the route", filter.into()),
        strip,
        stack![map_card(app).flex(1.3), daylight_card().flex(1.)]
            .direction(responsive(Direction::Vertical).at(Breakpoint::Xl, Direction::Horizontal))
            .gap(20., Axis::Both),
    ]
    .gap(20., Axis::Vertical)
    .into()
}

fn hero(app: &App) -> Element<App> {
    let place = &PLACES[app.place];
    let i = app.place;
    let saved = app.saved[i];
    let reserved = app.booked.contains(&i);
    zstack![
        vstack![]
            .background_image(
                app.photo(i)
                    .clone()
                    .background()
                    .fit(ImageFit::Cover)
                    .position(ImagePosition::Bottom)
            )
            .rounded(22.)
            .clip(Clip::Rounded),
        vstack![
            hstack![
                photo_chip(format!("Arrive day {}", place.day)),
                photo_chip(format!(
                    "{}, {}",
                    app.temperature(place.temp),
                    place.sky.to_lowercase()
                )),
                spacer(),
                icon_button(
                    if saved {
                        Icon::HeartFilled
                    } else {
                        Icon::Heart
                    },
                    "hero-save",
                    40.,
                    saved,
                    move |app| {
                        app.saved[i] = !app.saved[i];
                        let name = PLACES[i].name;
                        app.notify(if app.saved[i] {
                            format!("Saved {name} to your shortlist")
                        } else {
                            format!("Removed {name} from your shortlist")
                        });
                    },
                ),
            ]
            .gap(8., Axis::Horizontal)
            .align(Align::Center),
            spacer(),
            small(place.region).font_size(14.).color("#D7E3E1"),
            display(place.name, 56.).font_weight(700).color("#F7FAF9"),
            text(place.blurb)
                .font_size(15.)
                .color("#DCE6E4")
                .width(480.),
            hstack![
                primary(
                    if reserved {
                        "Stay reserved"
                    } else {
                        "Reserve a stay"
                    },
                    "reserve",
                    |app| {
                        app.reserve_open = true;
                    }
                )
                .enabled(!reserved)
                .width(170.),
                button("Write about it")
                    .key("hero-journal")
                    .height(40.)
                    .width(150.)
                    .font_weight(600)
                    .background("#0B141C80")
                    .color("#F3F7F6")
                    .border_color("#FFFFFF40")
                    .border_width(1.)
                    .rounded(10.)
                    .hover_style(|s| s.background("#0B141CB8"))
                    .animate_on(Interaction::Hover, Animation::ease_out().duration_ms(140))
                    .on_click(move |app: &mut App| {
                        app.screen = Screen::Journal;
                        app.draft_photo = i;
                        if app.draft_title.is_empty() {
                            app.draft_title = PLACES[i].name.into();
                        }
                    }),
            ]
            .padding(10., Edge::Top)
            .gap(10., Axis::Horizontal),
        ]
        .padding(responsive(22.).at(Breakpoint::Lg, 32.), Edge::All)
        .gap(6., Axis::Vertical),
    ]
    .key(format!("hero-{i}"))
    .transition(Transition::fade().duration_ms(320))
    .align(Align::Stretch)
    .height(430.)
    .into()
}

fn place_card(app: &App, i: usize) -> Element<App> {
    let place = &PLACES[i];
    let selected = app.place == i;
    let kinds: Vec<Element<App>> = place
        .kinds
        .iter()
        .map(|k| {
            let (label, bg, fg) = match k {
                super::Kind::Peaks => ("Peaks", ColorToken::Base300, ColorToken::BaseContent),
                super::Kind::Coast => ("Coast", ColorToken::Base300, ColorToken::BaseContent),
                super::Kind::Night => {
                    ("Night sky", ColorToken::Primary, ColorToken::PrimaryContent)
                }
            };
            chip(label, bg, fg).font_size(11.).into()
        })
        .collect();
    zstack![
        button("")
            .key(format!("place-{i}"))
            .width(232.)
            .height(282.)
            .background(ColorToken::Base200)
            .border_color(if selected {
                ColorToken::Primary
            } else {
                ColorToken::Border
            })
            .border_width(if selected { 2. } else { 1. })
            .rounded(18.)
            .hover_style(|s| s
                .border_color(ColorToken::Muted)
                .shadow(Shadow::new().color("#0000004D").blur(18.).offset(0., 8.)))
            .pressed_style(|s| s.opacity(0.85))
            .focus_style(|s| s.glow(Glow::new().color(ColorToken::Primary).blur(6.).spread(2.)))
            .animate_on(Interaction::Hover, Animation::ease_out().duration_ms(160))
            .animation(Animation::ease_in_out().duration_ms(220))
            .on_click(move |app: &mut App| app.place = i),
        vstack![
            zstack![
                vstack![]
                    .background_image(app.thumb(i).clone().background().fit(ImageFit::Cover))
                    .rounded(12.)
                    .clip(Clip::Rounded),
                vstack![hstack![
                    spacer(),
                    if app.saved[i] {
                        Element::from(photo_chip("Saved").font_size(11.))
                    } else {
                        spacer().width(0.).into()
                    }
                ]]
                .padding(8., Edge::All),
            ]
            .align(Align::Stretch)
            .height(150.),
            display(place.name, 21.),
            hstack![
                small(format!("{}, day {}", place.region, place.day)),
                spacer(),
                small(app.distance(place.km))
            ],
            hstack(kinds).gap(6., Axis::Horizontal),
        ]
        .width(232.)
        .padding(10., Edge::All)
        .gap(8., Axis::Vertical),
    ]
    .key(format!("place-card-{i}"))
    .flow(Flow::TopLeft)
    .width(232.)
    .height(282.)
    .into()
}

fn map_card(app: &App) -> Container<App> {
    let (w, h) = (520., 230.);
    let current = app.place;
    let map = canvas(move |c| {
        let at = |p: [f32; 2]| [24. + p[0] * (w - 48.), 20. + p[1] * (h - 40.)];
        c.fill(VectorPath::rectangle(0., 0., w, h), ColorToken::Base300);
        // A stylized coastline: the mainland to the south-east, islands to the west.
        c.fill(
            VectorPath::new()
                .move_to(w, h)
                .line_to(w * 0.08, h)
                .cubic_to(
                    [w * 0.2, h * 0.9],
                    [w * 0.3, h * 0.95],
                    [w * 0.36, h * 0.72],
                )
                .cubic_to(
                    [w * 0.45, h * 0.62],
                    [w * 0.5, h * 0.6],
                    [w * 0.55, h * 0.5],
                )
                .cubic_to(
                    [w * 0.62, h * 0.45],
                    [w * 0.7, h * 0.52],
                    [w * 0.76, h * 0.4],
                )
                .cubic_to([w * 0.84, h * 0.3], [w * 0.9, h * 0.36], [w, h * 0.22])
                .close(),
            ColorToken::Base200,
        );
        for (x, y, rx, ry) in [
            (0.13, 0.82, 0.07, 0.035),
            (0.24, 0.72, 0.06, 0.03),
            (0.38, 0.55, 0.05, 0.05),
            (0.5, 0.42, 0.06, 0.035),
            (0.92, 0.14, 0.04, 0.03),
        ] {
            c.fill(
                VectorPath::ellipse(w * x, h * y, w * rx, h * ry),
                ColorToken::Base200,
            );
        }
        // Route line with short dashes between stops.
        for pair in PLACES.windows(2) {
            let (a, b) = (at(pair[0].map), at(pair[1].map));
            let steps = 14;
            for s in (0..steps).step_by(2) {
                let t0 = s as f32 / steps as f32;
                let t1 = (s + 1) as f32 / steps as f32;
                c.stroke(
                    VectorPath::new()
                        .move_to(a[0] + (b[0] - a[0]) * t0, a[1] + (b[1] - a[1]) * t0)
                        .line_to(a[0] + (b[0] - a[0]) * t1, a[1] + (b[1] - a[1]) * t1),
                    ColorToken::Muted,
                    2.,
                );
            }
        }
        for (i, place) in PLACES.iter().enumerate() {
            let [x, y] = at(place.map);
            if i == current {
                c.fill(VectorPath::circle(x, y, 13.), "#86E3C333");
                c.fill(VectorPath::circle(x, y, 7.), ColorToken::Primary);
                c.stroke(VectorPath::circle(x, y, 7.), ColorToken::Base100, 2.);
            } else {
                c.fill(VectorPath::circle(x, y, 5.), ColorToken::Secondary);
                c.stroke(VectorPath::circle(x, y, 5.), ColorToken::Base100, 1.5);
            }
        }
    })
    .key("route-map")
    .width(w)
    .height(h);
    let place = &PLACES[current];
    card(vec![
        section_header(
            "Route map",
            chip(
                format!("{} from Reine", app.distance(place.km)),
                ColorToken::Base300,
                ColorToken::BaseContent,
            )
            .into(),
        ),
        vstack![map]
            .rounded(12.)
            .clip(Clip::Rounded)
            .height(h)
            .into(),
        hstack![
            row_with_icon(
                Icon::Pin,
                ColorToken::Primary,
                vstack![
                    body(format!("Now viewing {}", place.name)).font_weight(600),
                    small(format!("Stop {} of 6", current + 1))
                ]
                .into()
            ),
            spacer(),
            quiet("Previous", "map-prev", |app| app.place =
                app.place.saturating_sub(1))
            .enabled(current > 0)
            .width(96.),
            quiet("Next stop", "map-next", |app| app.place =
                (app.place + 1).min(PLACES.len() - 1))
            .enabled(current + 1 < PLACES.len())
            .width(104.),
        ]
        .gap(8., Axis::Horizontal)
        .align(Align::Center)
        .into(),
    ])
}

fn daylight_card() -> Container<App> {
    let (w, h) = (500., 150.);
    let chart = canvas(move |c| {
        let bar = w / 12.;
        for (i, hours) in DAYLIGHT.iter().enumerate() {
            let x = i as f32 * bar + bar * 0.2;
            let full = h - 4.;
            c.fill(
                VectorPath::rectangle(x, 0., bar * 0.6, full),
                ColorToken::Base300,
            );
            let lit = full * hours / 24.;
            if lit > 0. {
                c.fill(
                    VectorPath::rectangle(x, full - lit, bar * 0.6, lit),
                    if *hours >= 24. {
                        ColorToken::Secondary
                    } else {
                        ColorToken::Primary
                    },
                );
            }
        }
    })
    .width(w)
    .height(h);
    let months = "JFMAMJJASOND"
        .chars()
        .map(|m| small(m.to_string()).font_size(11.).width(w / 12.).into())
        .collect::<Vec<Element<App>>>();
    card(vec![
        section_header(
            "Daylight in Tromsø",
            icon(Icon::Sun, 20., ColorToken::Secondary),
        ),
        muted("From late November the sun stays below the horizon. By late May it no longer sets.")
            .into(),
        vstack![chart, hstack(months)]
            .gap(6., Axis::Vertical)
            .into(),
        hstack![
            stat("0 h", "Shortest day"),
            spacer(),
            stat("24 h", "Longest day"),
            spacer(),
            stat("Kp 4", "Aurora tonight"),
        ]
        .into(),
    ])
}

// ---------------------------------------------------------------- Days

fn days(app: &App) -> Element<App> {
    let legs = LEGS
        .iter()
        .map(|(leg, name, range, place)| {
            tab(*leg, *name, leg_page(app, name, range.clone(), *place))
                .key(format!("leg-{}", name.to_lowercase().replace('å', "a")))
        })
        .collect::<Vec<_>>();
    let plan = tabs(legs)
        .key("legs")
        .selected(app.leg)
        .on_select(|app: &mut App, leg| app.leg = leg)
        .gap(16., Axis::Vertical);
    stack![
        vstack![plan].flex(1.6),
        vstack![budget_card(app), packing_card(app)]
            .gap(20., Axis::Vertical)
            .flex(1.),
    ]
    .direction(responsive(Direction::Vertical).at(Breakpoint::Lg, Direction::Horizontal))
    .gap(24., Axis::Both)
    .into()
}

fn leg_page(app: &App, name: &str, range: std::ops::Range<usize>, place: usize) -> Element<App> {
    let done = range.clone().filter(|d| app.done_days[*d]).count();
    let total = range.len();
    let mut rows: Vec<Element<App>> = vec![hstack![
        vstack![]
            .background_image(app.thumb(place).clone().background().fit(ImageFit::Cover))
            .width(64.)
            .height(64.)
            .clip(Clip::Circle),
        vstack![
            display(name.to_string(), 26.),
            muted(format!(
                "Days {} to {}, {} of {} done",
                range.start + 1,
                range.end,
                done,
                total
            ))
        ]
        .gap(2., Axis::Vertical)
        .flex(1.),
        meter(done as f32 / total as f32, 120., ColorToken::Primary),
    ]
    .gap(16., Axis::Horizontal)
    .align(Align::Center)
    .padding(4., Edge::Bottom)
    .into()];
    for day in range {
        let (title, detail, glyph) = DAYS[day];
        let finished = app.done_days[day];
        rows.push(
            hstack![
                panel![text(format!("{}", day + 1))
                    .font_size(15.)
                    .font_weight(700)
                    .color(if finished {
                        ColorToken::PrimaryContent
                    } else {
                        ColorToken::BaseContent
                    })]
                .width(40.)
                .height(40.)
                .padding(0., Edge::All)
                .flow(Flow::Center)
                .background(if finished {
                    ColorToken::Primary
                } else {
                    ColorToken::Base300
                })
                .border_width(0.)
                .rounded(Rounding::Full)
                .key(format!("day-dot-{day}"))
                .animation(Animation::ease_out().duration_ms(220)),
                vstack![body(title).font_weight(600), muted(detail)]
                    .gap(2., Axis::Vertical)
                    .flex(1.),
                icon(glyph, 20., ColorToken::Muted),
                checkbox("Done", finished)
                    .key(format!("day-{day}"))
                    .accent_color(ColorToken::Primary)
                    .on_change(move |app: &mut App, value| app.done_days[day] = value),
            ]
            .key(format!("day-row-{day}"))
            .padding(14., Edge::All)
            .gap(14., Axis::Horizontal)
            .align(Align::Center)
            .background(ColorToken::Base200)
            .border_color(if finished {
                ColorToken::Primary
            } else {
                ColorToken::Border
            })
            .border_width(1.)
            .rounded(14.)
            .animation(Animation::ease_out().duration_ms(220))
            .into(),
        );
    }
    vstack(rows).gap(10., Axis::Vertical).into()
}

fn budget_card(app: &App) -> Element<App> {
    let transport = select(
        Transport::ALL
            .into_iter()
            .map(|(t, label)| select_item(t, label)),
    )
    .key("transport")
    .selected(app.transport)
    .open(app.transport_open)
    .on_select(|app: &mut App, t| app.transport = t)
    .on_open_change(|app: &mut App, open| app.transport_open = open)
    .width(300.);
    card(vec![
        section_header(
            "Budget",
            chip(
                nok(app.trip_total()),
                ColorToken::Secondary,
                ColorToken::SecondaryContent,
            )
            .into(),
        ),
        vstack![
            hstack![
                muted("Food and stays per day"),
                spacer(),
                body(nok(app.budget as u32)).font_weight(600)
            ],
            slider(app.budget)
                .key("budget")
                .range(600.0..=4000.0)
                .step(100.)
                .width(300.)
                .accent_color(ColorToken::Primary)
                .on_change(|app: &mut App, v| app.budget = v),
        ]
        .gap(8., Axis::Vertical)
        .into(),
        vstack![small("Getting around"), transport]
            .gap(6., Axis::Vertical)
            .into(),
        hstack![
            row_with_icon(
                Icon::Ferry,
                ColorToken::Secondary,
                vstack![
                    body(nok(app.transport.per_day())).font_weight(600),
                    small("Transport per day")
                ]
                .into()
            ),
            spacer(),
            stat("14", "Days"),
        ]
        .align(Align::Center)
        .into(),
    ])
    .into()
}

fn packing_card(app: &App) -> Element<App> {
    let packed = app.packing.iter().filter(|p| **p).count();
    let mut items: Vec<Element<App>> = vec![
        section_header(
            "Packing",
            small(format!("{packed} of {}", PACKING.len())).into(),
        ),
        meter(
            packed as f32 / PACKING.len() as f32,
            300.,
            ColorToken::Secondary,
        ),
    ];
    for (i, label) in PACKING.iter().enumerate() {
        items.push(
            checkbox(*label, app.packing[i])
                .key(format!("pack-{i}"))
                .accent_color(ColorToken::Secondary)
                .on_change(move |app: &mut App, v| app.packing[i] = v)
                .into(),
        );
    }
    card(items).gap(8., Axis::Vertical).into()
}

// ---------------------------------------------------------------- Journal

fn mood_label(mood: Mood) -> &'static str {
    match mood {
        Mood::Calm => "Calm",
        Mood::Awed => "Awed",
        Mood::Windswept => "Windswept",
        Mood::Cozy => "Cozy",
    }
}

fn journal(app: &App) -> Element<App> {
    let photos = (0..PLACES.len())
        .map(|i| {
            let selected = app.draft_photo == i;
            zstack![
                button("")
                    .key(format!("photo-{i}"))
                    .width(56.)
                    .height(56.)
                    .background("#00000000")
                    .border_color(if selected {
                        ColorToken::Primary
                    } else {
                        ColorToken::Border
                    })
                    .border_width(if selected { 3. } else { 1. })
                    .rounded(Rounding::Full)
                    .focus_style(
                        |s| s.glow(Glow::new().color(ColorToken::Primary).blur(6.).spread(2.))
                    )
                    .on_click(move |app: &mut App| app.draft_photo = i),
                vstack![]
                    .background_image(app.thumb(i).clone().background().fit(ImageFit::Cover))
                    .width(46.)
                    .height(46.)
                    .clip(Clip::Circle),
            ]
            .key(format!("photo-layer-{i}"))
            .width(56.)
            .height(56.)
            .into()
        })
        .collect::<Vec<Element<App>>>();
    let can_save = !app.draft_title.trim().is_empty() && !app.draft_body.trim().is_empty();
    let composer = card(vec![
        section_header(
            "New entry",
            chip(
                mood_label(app.mood),
                ColorToken::Base300,
                ColorToken::BaseContent,
            )
            .into(),
        ),
        text_field(&app.draft_title)
            .key("draft-title")
            .placeholder("Where are you?")
            .height(44.)
            .font_size(18.)
            .background(ColorToken::Base100)
            .border_color(ColorToken::Border)
            .rounded(10.)
            .on_change(|app: &mut App, v| app.draft_title = v)
            .into(),
        text_area(&app.draft_body)
            .key("draft-body")
            .placeholder("What did today feel like?")
            .height(150.)
            .font_size(15.)
            .background(ColorToken::Base100)
            .border_color(ColorToken::Border)
            .rounded(10.)
            .on_change(|app: &mut App, v| app.draft_body = v)
            .into(),
        vstack![
            small("How it felt"),
            segmented_control([
                (Mood::Calm, "Calm"),
                (Mood::Awed, "Awed"),
                (Mood::Windswept, "Windswept"),
                (Mood::Cozy, "Cozy")
            ])
            .key("mood")
            .selected(app.mood)
            .on_select(|app: &mut App, m| app.mood = m)
            .height(36.)
            .rounded(10.),
        ]
        .gap(6., Axis::Vertical)
        .into(),
        vstack![small("Photo"), hstack(photos).gap(8., Axis::Horizontal)]
            .gap(6., Axis::Vertical)
            .into(),
        hstack![
            small(if can_save {
                "Ready to save"
            } else {
                "Add a place and a few words to save"
            }),
            spacer(),
            button_bar![
                quiet("Clear", "draft-clear", |app| {
                    app.draft_title.clear();
                    app.draft_body.clear();
                })
                .width(90.),
                primary("Save entry", "draft-save", |app| {
                    let entry = super::Entry {
                        title: std::mem::take(&mut app.draft_title),
                        body: std::mem::take(&mut app.draft_body),
                        mood: app.mood,
                        photo: app.draft_photo,
                    };
                    app.notify(format!("Saved \"{}\" to your journal", entry.title));
                    app.entries.insert(0, entry);
                })
                .enabled(can_save)
                .width(120.),
            ]
            .gap(8., Axis::Horizontal),
        ]
        .align(Align::Center)
        .into(),
    ]);
    let entries = app
        .entries
        .iter()
        .enumerate()
        .map(|(n, e)| {
            hstack![
                vstack![]
                    .background_image(app.thumb(e.photo).clone().background().fit(ImageFit::Cover))
                    .width(104.)
                    .height(104.)
                    .rounded(12.)
                    .clip(Clip::Rounded),
                vstack![
                    display(e.title.clone(), 19.),
                    text(e.body.clone()).font_size(14.).color(ColorToken::Muted),
                    chip(
                        mood_label(e.mood),
                        ColorToken::Base300,
                        ColorToken::BaseContent
                    )
                    .font_size(11.),
                ]
                .gap(6., Axis::Vertical)
                .flex(1.),
            ]
            .key(format!("entry-{}", app.entries.len() - n))
            .transition(Transition::fade().duration_ms(260))
            .padding(14., Edge::All)
            .gap(16., Axis::Horizontal)
            .background(ColorToken::Base200)
            .border_color(ColorToken::Border)
            .border_width(1.)
            .rounded(16.)
            .into()
        })
        .collect::<Vec<Element<App>>>();
    stack![
        composer.flex(1.1),
        vstack![
            section_header(
                "Entries",
                small(format!("{} written", app.entries.len())).into()
            ),
            vstack(entries).gap(12., Axis::Vertical)
        ]
        .gap(14., Axis::Vertical)
        .flex(1.),
    ]
    .direction(responsive(Direction::Vertical).at(Breakpoint::Lg, Direction::Horizontal))
    .gap(24., Axis::Both)
    .into()
}

// ---------------------------------------------------------------- Settings

fn settings(app: &App) -> Element<App> {
    let accents = ACCENTS
        .iter()
        .enumerate()
        .map(|(i, (name, color))| {
            let selected = app.accent == i;
            button(if selected {
                format!("✓ {name}")
            } else {
                name.to_string()
            })
            .key(format!("accent-{}", name.to_lowercase()))
            .height(40.)
            .width(96.)
            .font_weight(600)
            .background(format!("#{color:06X}"))
            .color("#10202A")
            // A ring marks the selection; dimming would need an opacity layer.
            .border_color(ColorToken::BaseContent)
            .border_width(if selected { 2. } else { 0. })
            .hover_style(|s| s.border_color(ColorToken::Muted).border_width(2.))
            .focus_style(|s| {
                s.glow(
                    Glow::new()
                        .color(ColorToken::BaseContent)
                        .blur(4.)
                        .spread(2.),
                )
            })
            .animate_on(Interaction::Hover, Animation::ease_out().duration_ms(120))
            .animation(Animation::ease_out().duration_ms(200))
            .on_click(move |app: &mut App| app.accent = i)
            .into()
        })
        .collect::<Vec<Element<App>>>();
    let appearance = card(vec![
        heading("Appearance").into(),
        vstack![
            small("Theme"),
            segmented_control([
                (ThemeMode::Dark, "Polar night"),
                (ThemeMode::Light, "Midnight sun"),
                (ThemeMode::System, "Match system")
            ])
            .key("theme")
            .selected(app.mode)
            .on_select(|app: &mut App, m| app.mode = m)
            .height(36.)
            .rounded(10.),
        ]
        .gap(6., Axis::Vertical)
        .into(),
        vstack![
            small("Accent"),
            button_bar(accents).joined(true).rounded(10.)
        ]
        .gap(6., Axis::Vertical)
        .into(),
        zstack![
            vstack![]
                .background_image(
                    app.thumb(app.place)
                        .clone()
                        .background()
                        .fit(ImageFit::Cover)
                )
                .rounded(14.)
                .clip(Clip::Rounded),
            vstack![
                hstack![
                    photo_chip("Preview"),
                    spacer(),
                    chip("Night sky", ColorToken::Primary, ColorToken::PrimaryContent)
                        .font_size(11.)
                ],
                spacer(),
                display(PLACES[app.place].name, 28.).color("#F7FAF9"),
                hstack![
                    primary("Reserve a stay", "preview-primary", |_| {})
                        .width(150.)
                        .height(36.),
                    quiet("Details", "preview-quiet", |_| {})
                        .width(90.)
                        .height(36.)
                ]
                .gap(8., Axis::Horizontal),
            ]
            .padding(16., Edge::All)
            .gap(6., Axis::Vertical),
        ]
        .align(Align::Stretch)
        .height(200.)
        .into(),
    ]);
    let units = select([
        select_item(Units::Metric, "Kilometres and °C"),
        select_item(Units::Imperial, "Miles and °F"),
    ])
    .key("units")
    .selected(app.units)
    .open(app.units_open)
    .on_select(|app: &mut App, u| app.units = u)
    .on_open_change(|app: &mut App, open| app.units_open = open)
    .width(260.);
    let travel = card(vec![
        heading("Travel").into(),
        hstack![
            vstack![
                body("Units").font_weight(600),
                small("Distances and temperatures")
            ]
            .flex(1.),
            units
        ]
        .align(Align::Center)
        .gap(12., Axis::Horizontal)
        .into(),
        divider().background(ColorToken::Border).into(),
        toggle("Download maps for offline use", app.offline_maps)
            .key("offline")
            .accent_color(ColorToken::Primary)
            .on_change(|app: &mut App, v| app.offline_maps = v)
            .into(),
        toggle("Ferry departure reminders", app.ferry_reminders)
            .key("ferries")
            .accent_color(ColorToken::Primary)
            .on_change(|app: &mut App, v| app.ferry_reminders = v)
            .into(),
        toggle("Aurora alerts", app.aurora_alerts)
            .key("aurora")
            .accent_color(ColorToken::Primary)
            .on_change(|app: &mut App, v| app.aurora_alerts = v)
            .into(),
        vstack![
            hstack![
                icon(Icon::Aurora, 18., ColorToken::Primary),
                muted(format!("Alert me at Kp {} and above", app.kp as u8)).flex(1.)
            ]
            .gap(10., Axis::Horizontal)
            .align(Align::Center),
            slider(app.kp)
                .key("kp")
                .range(1.0..=9.0)
                .step(1.)
                .width(320.)
                .enabled(app.aurora_alerts)
                .accent_color(ColorToken::Primary)
                .on_change(|app: &mut App, v| app.kp = v),
        ]
        .opacity(if app.aurora_alerts { 1. } else { 0.45 })
        .key("kp-group")
        .animation(Animation::ease_out().duration_ms(200))
        .gap(8., Axis::Vertical)
        .into(),
    ]);
    let trip = card(vec![
        heading("Trip data").into(),
        muted(format!(
            "{}, {} and {}. Everything stays on this computer; all places and prices are sample data.",
            count(app.saved.iter().filter(|s| **s).count(), "saved place"),
            count(app.booked.len(), "reserved stay"),
            count(app.entries.len(), "journal entry"),
        ))
        .into(),
        hstack![spacer(), danger("Reset trip", "reset", |app| app.reset_open = true).width(120.)].into(),
    ]);
    stack![
        vstack![appearance, trip].gap(20., Axis::Vertical).flex(1.),
        vstack![travel].flex(1.),
    ]
    .direction(responsive(Direction::Vertical).at(Breakpoint::Lg, Direction::Horizontal))
    .gap(24., Axis::Both)
    .into()
}

// ---------------------------------------------------------------- Overlays

fn reserve_dialog(app: &App) -> Element<App> {
    let i = app.place;
    let place = &PLACES[i];
    let nights = select((1..=5_u8).map(|n| {
        select_item(
            n,
            if n == 1 {
                "1 night".to_string()
            } else {
                format!("{n} nights")
            },
        )
    }))
    .key("nights")
    .selected(app.nights)
    .open(app.nights_open)
    .on_select(|app: &mut App, n| app.nights = n)
    .on_open_change(|app: &mut App, open| app.nights_open = open)
    .width(160.);
    let can_book = !app.guest.trim().is_empty();
    dialog(
        vstack![
            zstack![
                vstack![]
                    .background_image(app.photo(i).clone().background().fit(ImageFit::Cover))
                    .rounded(12.)
                    .clip(Clip::Rounded),
                vstack![
                    spacer(),
                    display(place.name, 30.).color("#F7FAF9"),
                    small("Harbour cabin, sleeps two").color("#DCE6E4")
                ]
                .padding(16., Edge::All),
            ]
            .align(Align::Stretch)
            .height(170.),
            text_field(&app.guest)
                .key("guest")
                .placeholder("Name on the booking")
                .height(42.)
                .background(ColorToken::Base100)
                .border_color(ColorToken::Border)
                .rounded(10.)
                .on_change(|app: &mut App, v| app.guest = v)
                .on_submit(|app: &mut App| {
                    if !app.guest.trim().is_empty() {
                        confirm_booking(app);
                    }
                }),
            hstack![
                row_with_icon(
                    Icon::Bed,
                    ColorToken::Primary,
                    body("Length of stay").into()
                ),
                spacer(),
                nights
            ]
            .align(Align::Center),
            toggle("Breakfast included", app.breakfast)
                .key("breakfast")
                .accent_color(ColorToken::Primary)
                .on_change(|app: &mut App, v| app.breakfast = v),
            checkbox("I'll arrive after 22:00", app.late_arrival)
                .key("late")
                .accent_color(ColorToken::Primary)
                .on_change(|app: &mut App, v| app.late_arrival = v),
            divider().background(ColorToken::Border),
            hstack![
                vstack![
                    muted("Total"),
                    text(nok(app.stay_price())).font_size(24.).font_weight(700)
                ],
                spacer(),
                button_bar![
                    quiet("Cancel", "reserve-cancel", |app| app.reserve_open = false).width(90.),
                    primary("Reserve", "reserve-confirm", confirm_booking)
                        .enabled(can_book)
                        .width(110.),
                ]
                .gap(8., Axis::Horizontal)
            ]
            .align(Align::End),
        ]
        .gap(14., Axis::Vertical),
    )
    .key("reserve-dialog")
    .open(app.reserve_open)
    .on_dismiss(|app: &mut App| {
        app.reserve_open = false;
        app.nights_open = false;
    })
    .width(460.)
    .rounded(20.)
    .shadow(Shadow::new().color("#00000066").blur(40.).offset(0., 20.))
    .into()
}

fn confirm_booking(app: &mut App) {
    app.reserve_open = false;
    app.nights_open = false;
    if !app.booked.contains(&app.place) {
        app.booked.push(app.place);
    }
    let nights = if app.nights == 1 {
        "1 night".to_string()
    } else {
        format!("{} nights", app.nights)
    };
    app.notify(format!(
        "Reserved {nights} in {} for {}",
        PLACES[app.place].name,
        app.guest.trim()
    ));
}

fn reset_dialog(app: &App) -> Element<App> {
    dialog(vstack![
        muted("Saved places, reservations, checklists and journal entries return to the sample trip. Theme and units are kept."),
        hstack![spacer(), quiet("Keep my trip", "reset-cancel", |app| app.reset_open = false).width(130.), danger("Reset trip", "reset-confirm", |app| app.reset()).width(120.)]
            .gap(8., Axis::Horizontal),
    ]
    .gap(18., Axis::Vertical))
    .key("reset-dialog")
    .title("Reset the whole trip?")
    .open(app.reset_open)
    .on_dismiss(|app: &mut App| app.reset_open = false)
    .width(420.)
    .rounded(20.)
    .into()
}

fn toast_layer(app: &App) -> Element<App> {
    let content: Element<App> = match &app.toast {
        Some(toast) => hstack![
            panel![icon(Icon::Check, 16., ColorToken::PrimaryContent)]
                .width(30.)
                .height(30.)
                .padding(0., Edge::All)
                .flow(Flow::Center)
                .background(ColorToken::Primary)
                .border_width(0.)
                .rounded(Rounding::Full),
            body(toast.message.clone()).flex(1.),
            icon_button(Icon::Close, "toast-close", 32., false, |app| app.toast =
                None),
        ]
        .key(format!("toast-{}", toast.id))
        .transition(Transition::fade().duration_ms(260))
        .width(400.)
        .padding(12., Edge::All)
        .padding(16., Edge::Left)
        .gap(12., Axis::Horizontal)
        .align(Align::Center)
        .background(ColorToken::Base300)
        .border_color(ColorToken::Border)
        .border_width(1.)
        .rounded(14.)
        .shadow(Shadow::new().color("#00000055").blur(24.).offset(0., 10.))
        .into(),
        None => spacer().width(0.).height(0.).into(),
    };
    vstack![content]
        .key("toast-layer")
        .flow(Flow::BottomRight)
        .padding(24., Edge::All)
        .scroll_passthrough(true)
        .into()
}
