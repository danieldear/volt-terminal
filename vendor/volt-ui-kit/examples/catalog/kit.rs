//! Catalog pages for the Tailwind/shadcn parity components: icons, dates,
//! form inputs, disclosure, data display, charts and layout utilities.
use super::{
    pages::{Doc, Section},
    App,
};
use std::path::PathBuf;
use volt_ui_kit::ui::{prelude::*, Element as El, ALL_ICONS};

/// Demo state for these pages, kept apart from the older catalog fields.
pub struct State {
    pub date: Option<CalendarDate>,
    pub month: (i32, u8),
    pub picker: DatePickerState,
    pub range: DateRange,
    pub range_picker: DatePickerState,
    pub time: TimeOfDay,
    pub plan: u8,
    pub seats: f64,
    pub weight: f64,
    pub framework: Option<u8>,
    pub combo: ComboboxState,
    pub tags: Vec<String>,
    pub tag_draft: TagInputState,
    pub price: (f32, f32),
    pub colour: String,
    pub code: String,
    pub verified: bool,
    pub stars: u8,
    pub search: String,
    pub email: String,
    pub files: Vec<PathBuf>,
    pub details: bool,
    pub faq: Vec<usize>,
    pub drawer: bool,
    pub drawer_side: DrawerSide,
    pub toasts: Vec<Toast>,
    pub next_toast: u64,
    pub palette: CommandPaletteState,
    pub ran: String,
    pub view_menu: bool,
    pub submenu: Option<String>,
    pub wrap: bool,
    pub minimap: bool,
    pub page: usize,
    pub crumb: String,
    pub slide: usize,
    pub alert: bool,
    pub step: usize,
}
impl Default for State {
    fn default() -> Self {
        let today = CalendarDate::today_utc();
        Self {
            date: Some(today),
            month: (today.year(), today.month()),
            picker: DatePickerState::default(),
            range: DateRange {
                start: Some(today),
                end: today.add_days(4),
            },
            range_picker: DatePickerState::default(),
            time: TimeOfDay::new(9, 30).unwrap(),
            plan: 1,
            seats: 3.,
            weight: 2.5,
            framework: Some(0),
            combo: ComboboxState::default(),
            tags: vec!["rust".into(), "desktop".into()],
            tag_draft: TagInputState::default(),
            price: (20., 80.),
            colour: "#6366F1".into(),
            code: String::new(),
            verified: false,
            stars: 4,
            search: String::new(),
            email: "ada@".into(),
            files: Vec::new(),
            details: false,
            faq: vec![0],
            drawer: false,
            drawer_side: DrawerSide::Right,
            toasts: Vec::new(),
            next_toast: 1,
            palette: CommandPaletteState::default(),
            ran: "Nothing yet".into(),
            view_menu: false,
            submenu: None,
            wrap: true,
            minimap: false,
            page: 3,
            crumb: "Installation".into(),
            slide: 0,
            alert: true,
            step: 1,
        }
    }
}
impl State {
    /// Close this page group's overlays when navigating away.
    pub fn close_overlays(&mut self) {
        self.picker.open = false;
        self.range_picker.open = false;
        self.combo.open = false;
        self.drawer = false;
        self.palette.open = false;
        self.view_menu = false;
        self.submenu = None;
    }
}

fn section(
    id: &'static str,
    title: &'static str,
    note: &'static str,
    demo: impl Into<El<App>>,
    code: &'static str,
) -> Section {
    Section {
        id,
        title,
        note,
        demo: demo.into(),
        code,
    }
}
fn muted(value: impl Into<String>) -> volt_ui_kit::ui::Text<App> {
    text(value).font_size(13.).color(ColorToken::Muted)
}
fn caption(value: impl Into<String>) -> volt_ui_kit::ui::Text<App> {
    text(value)
        .font_size(12.)
        .font_weight(500)
        .color(ColorToken::Muted)
}

// ------------------------------------------------------------------ Icons

pub fn icons_doc() -> Doc {
    let tiles: Vec<El<App>> = ALL_ICONS
        .iter()
        .map(|glyph| {
            vstack![
                icon(*glyph).size(22.),
                text(format!("{glyph:?}"))
                    .font_size(11.)
                    .color(ColorToken::Muted)
                    .max_lines(1),
            ]
            .align(Align::Center)
            .gap(8., Axis::Vertical)
            .padding(12., Edge::Vertical)
            .rounded(8.)
            .hover_style(|s| s.background(ColorToken::BaseContent.alpha(5)))
            .into()
        })
        .collect();
    let gallery = grid(tiles)
        .key("icon-gallery")
        .min_column_width(96.)
        .gap(6., Axis::Both)
        .width(640.);
    let sizes = hstack![
        icon(Icon::Bolt).size(16.),
        icon(Icon::Bolt).size(24.),
        icon(Icon::Bolt).size(32.).color(ColorToken::Primary),
        icon(Icon::Heart).size(32.).color(ColorToken::Error),
        icon(Icon::Settings).size(32.).stroke(1.),
        icon(Icon::Settings).size(32.).stroke(2.25),
    ]
    .gap(20., Axis::Horizontal)
    .align(Align::Center);
    Doc {
        sections: vec![
            section(
                "icons-gallery",
                "Gallery",
                "Original drawings on a 24-unit grid. They follow the theme's text colour.",
                gallery,
                r#"icon(Icon::Search)"#,
            ),
            section(
                "icons-style",
                "Size, colour and stroke",
                "Icons scale crisply and take any colour token or hex value.",
                sizes,
                r#"icon(Icon::Bolt).size(32.).color(ColorToken::Primary)
icon(Icon::Settings).size(32.).stroke(1.)"#,
            ),
        ],
        api: vec![
            ("icon(Icon::…)", "A vector glyph element (default 20 px)."),
            (
                ".size(px) / .color(c) / .stroke(w)",
                "Scale, colour and line weight.",
            ),
            (".accessible_label(text)", "Announce a meaningful icon."),
            ("ALL_ICONS", "Every glyph, for galleries and pickers."),
        ],
    }
}

// ------------------------------------------------------------ Date & time

pub fn dates_doc(app: &App) -> Doc {
    let s = &app.kit;
    let today = CalendarDate::today_utc();
    let cal = calendar(s.month.0, s.month.1)
        .key("kit-cal")
        .selected(s.date)
        .today(today)
        .on_select(|app: &mut App, d| app.kit.date = Some(d))
        .on_month_change(|app: &mut App, y, m| app.kit.month = (y, m));
    let picker = vstack![
        date_picker(s.date, &s.picker)
            .key("kit-due")
            .today(today)
            .state(|app: &mut App| &mut app.kit.picker)
            .on_change(|app: &mut App, d| app.kit.date = Some(d)),
        muted(
            s.date
                .map_or("No date".into(), |d| format!("Due {}", d.long()))
        ),
    ]
    .gap(8., Axis::Vertical);
    let range = vstack![
        date_range_picker(s.range, &s.range_picker)
            .key("kit-trip")
            .today(today)
            .state(|app: &mut App| &mut app.kit.range_picker)
            .on_change(|app: &mut App, r| app.kit.range = r),
        muted(match s.range.days() {
            Some(n) => format!("{n} days"),
            None => "Pick a start and an end".into(),
        }),
    ]
    .gap(8., Axis::Vertical);
    let time = hstack![
        time_picker(s.time)
            .key("kit-alarm")
            .twelve_hour(true)
            .minute_step(5)
            .on_change(|app: &mut App, t| app.kit.time = t),
        muted(format!("Alarm at {}", s.time.format(false))),
    ]
    .gap(16., Axis::Horizontal)
    .align(Align::Center);
    Doc {
        sections: vec![
            section(
                "date-calendar",
                "Calendar",
                "A month grid with keyboard-reachable days, today marker and paging.",
                cal,
                r#"calendar(year, month)
    .key("cal")
    .selected(app.date)
    .on_select(|app: &mut App, d| app.date = Some(d))
    .on_month_change(|app: &mut App, y, m| app.month = (y, m))"#,
            ),
            section(
                "date-picker",
                "Date picker",
                "A field that opens a calendar popover.",
                picker,
                r#"date_picker(app.date, &app.picker)
    .key("due")
    .state(|app: &mut App| &mut app.picker)
    .on_change(|app: &mut App, d| app.date = Some(d))"#,
            ),
            section(
                "date-range",
                "Date range",
                "Two months side by side; the first click sets the start.",
                range,
                r#"date_range_picker(app.range, &app.range_picker)
    .key("trip")
    .state(|app: &mut App| &mut app.range_picker)
    .on_change(|app: &mut App, r| app.range = r)"#,
            ),
            section(
                "time-picker",
                "Time picker",
                "Hour and minute steppers with an optional AM/PM switch.",
                time,
                r#"time_picker(app.time).key("alarm").twelve_hour(true).minute_step(5)"#,
            ),
        ],
        api: vec![
            (
                "CalendarDate::new(y, m, d)",
                "A validated civil date; add_days, add_months, weekday.",
            ),
            (
                "calendar(y, m)",
                ".selected / .range / .bounds / .monday_first",
            ),
            (
                "date_picker(value, &state)",
                "Popover calendar; state keeps open/month.",
            ),
            (
                "date_range_picker(range, &state)",
                "DateRange { start, end } with days().",
            ),
            (
                "time_picker(TimeOfDay)",
                ".twelve_hour(bool) / .minute_step(n)",
            ),
        ],
    }
}

// ------------------------------------------------------------ Form inputs

pub fn forms_doc(app: &App) -> Doc {
    let s = &app.kit;
    let radios = radio_group([(0u8, "Hobby"), (1, "Pro"), (2, "Team")])
        .key("kit-plan")
        .selected(s.plan)
        .descriptions([
            "For side projects. Free forever.",
            "Unlimited projects and priority builds.",
            "Shared workspaces and roles.",
        ])
        .on_select(|app: &mut App, p| app.kit.plan = p);
    let numbers = hstack![
        vstack![
            caption("Seats"),
            number_input(s.seats)
                .key("kit-seats")
                .range(1., 50.)
                .on_change(|app: &mut App, v| app.kit.seats = v),
        ]
        .gap(6., Axis::Vertical),
        vstack![
            caption("Weight"),
            number_input(s.weight)
                .key("kit-weight")
                .range(0., 20.)
                .step(0.5)
                .decimals(1)
                .suffix(" kg")
                .width(160.)
                .on_change(|app: &mut App, v| app.kit.weight = v),
        ]
        .gap(6., Axis::Vertical),
    ]
    .gap(24., Axis::Horizontal);
    let frameworks = [
        (0u8, "Volt UI"),
        (1, "Iced"),
        (2, "egui"),
        (3, "Slint"),
        (4, "Dioxus"),
        (5, "Tauri"),
        (6, "GPUI"),
    ];
    let combo = combobox(frameworks, &s.combo)
        .key("kit-framework")
        .selected(s.framework)
        .placeholder("Search frameworks…")
        .state(|app: &mut App| &mut app.kit.combo)
        .on_select(|app: &mut App, v| app.kit.framework = Some(v));
    let tags = tag_input(&s.tags, &s.tag_draft)
        .key("kit-tags")
        .state(|app: &mut App| &mut app.kit.tag_draft)
        .on_change(|app: &mut App, t| app.kit.tags = t);
    let range = vstack![
        hstack![
            caption("Price"),
            spacer(),
            muted(format!("${:.0} – ${:.0}", s.price.0, s.price.1)),
        ],
        range_slider(s.price.0, s.price.1)
            .key("kit-price")
            .range(0. ..=200.)
            .step(5.)
            .width(320.)
            .on_change(|app: &mut App, lo, hi| app.kit.price = (lo, hi)),
    ]
    .gap(8., Axis::Vertical)
    .width(320.);
    let colour = color_picker(s.colour.clone())
        .key("kit-colour")
        .on_change(|app: &mut App, c| app.kit.colour = c);
    let otp = vstack![
        otp_input(s.code.clone(), 6)
            .key("kit-otp")
            .on_change(|app: &mut App, c| {
                app.kit.code = c;
                app.kit.verified = false;
            })
            .on_complete(|app: &mut App, _| app.kit.verified = true),
        muted(if s.verified {
            "Verified ✓"
        } else {
            "Enter the 6-digit code"
        }),
    ]
    .gap(10., Axis::Vertical)
    .align(Align::Center);
    let stars = hstack![
        rating(s.stars)
            .key("kit-stars")
            .on_change(|app: &mut App, n| app.kit.stars = n),
        muted(format!("{} of 5", s.stars)),
        rating(3).key("kit-stars-read").size(18.),
    ]
    .gap(16., Axis::Horizontal)
    .align(Align::Center);
    let email_error = (!s.email.is_empty() && !s.email.contains('.'))
        .then(|| "Enter a full address, like ada@example.com".to_owned());
    let fields = vstack![
        form_field(
            "Search",
            text_field(s.search.clone())
                .key("kit-search")
                .placeholder("Search components")
                .leading_icon(Icon::Search)
                .trailing_action("kit-search-clear", Icon::Close, "Clear", |app: &mut App| {
                    app.kit.search.clear()
                })
                .width(320.)
                .on_change(|app: &mut App, v| app.kit.search = v),
        )
        .key("kit-search-field")
        .description("Leading icon and a trailing clear button."),
        form_field(
            "Email",
            text_field(s.email.clone())
                .key("kit-email")
                .leading_icon(Icon::Mail)
                .width(320.)
                .on_change(|app: &mut App, v| app.kit.email = v),
        )
        .key("kit-email-field")
        .required(true)
        .error(email_error),
    ]
    .gap(16., Axis::Vertical);
    let drop = vstack![
        drop_zone()
            .key("kit-drop")
            .title("Drop images here")
            .accept(["png", "jpg", "jpeg", "gif", "webp"])
            .height(150.)
            .on_drop(|app: &mut App, paths| app.kit.files = paths),
        muted(if s.files.is_empty() {
            "Drag files from Finder onto the zone.".to_owned()
        } else {
            s.files
                .iter()
                .filter_map(|p| p.file_name()?.to_str().map(str::to_owned))
                .collect::<Vec<_>>()
                .join(", ")
        }),
    ]
    .gap(10., Axis::Vertical)
    .width(420.);
    Doc {
        sections: vec![
            section(
                "form-radio",
                "Radio group",
                "Mutually exclusive options with optional descriptions.",
                radios,
                r#"radio_group([(0, "Hobby"), (1, "Pro"), (2, "Team")])
    .key("plan")
    .selected(app.plan)
    .on_select(|app: &mut App, p| app.plan = p)"#,
            ),
            section(
                "form-number",
                "Number input",
                "Steppers clamp to the range; typed values are parsed and rounded.",
                numbers,
                r#"number_input(app.weight).range(0., 20.).step(0.5).decimals(1).suffix(" kg")"#,
            ),
            section(
                "form-combobox",
                "Combobox",
                "Type to filter, click or press Enter to choose.",
                combo,
                r#"combobox(items, &app.combo)
    .key("framework")
    .selected(app.framework)
    .state(|app: &mut App| &mut app.combo)
    .on_select(|app: &mut App, v| app.framework = Some(v))"#,
            ),
            section(
                "form-tags",
                "Tag input",
                "Enter adds the draft; × removes a tag; duplicates are ignored.",
                tags,
                r#"tag_input(&app.tags, &app.draft)
    .key("tags")
    .state(|app: &mut App| &mut app.draft)
    .on_change(|app: &mut App, tags| app.tags = tags)"#,
            ),
            section(
                "form-range",
                "Range slider",
                "Two thumbs; a press grabs the nearer one and they never cross.",
                range,
                r#"range_slider(lo, hi).range(0. ..=200.).step(5.)
    .on_change(|app: &mut App, lo, hi| app.price = (lo, hi))"#,
            ),
            section(
                "form-colour",
                "Colour picker",
                "Swatches, hue/saturation/lightness sliders and a hex field.",
                colour,
                r##"color_picker(app.colour.clone()).key("colour").on_change(|app: &mut App, hex| app.colour = hex)"##,
            ),
            section(
                "form-otp",
                "One-time code",
                "One field behind six boxes, so paste and backspace just work.",
                otp,
                r#"otp_input(app.code.clone(), 6).key("otp")
    .on_change(|app: &mut App, c| app.code = c)
    .on_complete(|app: &mut App, _| app.verified = true)"#,
            ),
            section(
                "form-rating",
                "Rating",
                "Click a star; click the current rating again to clear. Read-only without a handler.",
                stars,
                r#"rating(app.stars).key("stars").on_change(|app: &mut App, n| app.stars = n)"#,
            ),
            section(
                "form-field",
                "Form fields and adornments",
                "Labels, descriptions, required marks and errors around any control.",
                fields,
                r#"form_field("Email", text_field(&app.email).leading_icon(Icon::Mail))
    .key("email")
    .required(true)
    .error(validate(&app.email))"#,
            ),
            section(
                "form-drop",
                "Drop zone",
                "Native drag and drop. Extensions filter what is accepted.",
                drop,
                r#"drop_zone().key("drop").accept(["png", "jpg"])
    .on_drop(|app: &mut App, paths| app.files = paths)"#,
            ),
        ],
        api: vec![
            ("radio_group(items)", ".selected / .descriptions / .horizontal / .on_select"),
            ("number_input(v)", ".range / .step / .decimals / .suffix / .on_change"),
            ("combobox(items, &state)", ".state(accessor) / .selected / .on_select"),
            ("tag_input(&tags, &state)", ".max_tags / .placeholder / .on_change"),
            ("range_slider(lo, hi)", ".range / .step / .on_change(lo, hi)"),
            ("color_picker(hex)", ".swatches / .on_change(#RRGGBB); Hsl helpers"),
            ("otp_input(code, len)", ".on_change / .on_complete"),
            ("rating(n)", ".max / .size / .on_change"),
            ("form_field(label, control)", ".description / .error / .required"),
            ("text_field(..).leading_icon / .trailing_action", "Adornments inside the field."),
            ("drop_zone()", ".accept / .multiple / .on_browse / .on_drop"),
        ],
    }
}

// ----------------------------------------------------- Disclosure & overlays

pub fn disclosure_doc(app: &App) -> Doc {
    let s = &app.kit;
    let collapse = collapsible(
        "What is Volt UI?",
        muted("A Rust toolkit for native desktop interfaces, declared as functions of your state.")
            .max_width(420.),
    )
    .key("kit-details")
    .open(s.details)
    .on_toggle(|app: &mut App, open| app.kit.details = open);
    let collapse = vstack![collapse].width(460.);
    let faq = vstack![accordion([
        (
            "Is it accessible?",
            El::from(muted("Yes: roles, labels and keyboard focus are built in.")),
        ),
        (
            "Does it use the GPU?",
            muted("Vello on Metal, with a CPU fallback.").into()
        ),
        (
            "Can I theme it?",
            muted("Every colour is a semantic token.").into()
        ),
    ])
    .key("kit-faq")
    .expanded(s.faq.clone())
    .on_change(|app: &mut App, open| app.kit.faq = open)]
    .width(460.);
    let sides = [
        (DrawerSide::Left, "Left"),
        (DrawerSide::Right, "Right"),
        (DrawerSide::Bottom, "Bottom"),
    ];
    let drawer_demo = vstack![
        hstack(
            sides
                .into_iter()
                .map(|(side, name)| {
                    El::from(
                        button(format!("Open {}", name.to_lowercase()))
                            .key(format!("kit-drawer-{name}"))
                            .variant(ButtonVariant::Outline)
                            .on_click(move |app: &mut App| {
                                app.kit.drawer_side = side;
                                app.kit.drawer = true;
                            }),
                    )
                })
                .collect::<Vec<_>>()
        )
        .gap(8., Axis::Horizontal),
        drawer(
            vstack![
                muted("Drawers slide in from an edge and keep focus until dismissed."),
                toggle("Word wrap", app.kit.wrap)
                    .key("kit-drawer-wrap")
                    .on_change(|app: &mut App, v| app.kit.wrap = v),
                button("Done")
                    .key("kit-drawer-done")
                    .variant(ButtonVariant::Primary)
                    .on_click(|app: &mut App| app.kit.drawer = false),
            ]
            .gap(14., Axis::Vertical)
        )
        .key("kit-drawer")
        .title("Preferences")
        .side(s.drawer_side)
        .size(if s.drawer_side == DrawerSide::Bottom {
            260.
        } else {
            340.
        })
        .open(s.drawer)
        .on_dismiss(|app: &mut App| app.kit.drawer = false),
    ];
    let toasts = zstack![
        vstack![hstack![
            button("Success toast")
                .key("kit-toast-ok")
                .variant(ButtonVariant::Outline)
                .on_click(|app: &mut App| {
                    let id = app.kit.next_toast;
                    app.kit.next_toast += 1;
                    app.kit.toasts.push(
                        Toast::new(id, "Changes saved")
                            .description("Your profile is up to date.")
                            .tone(Severity::Success),
                    );
                }),
            button("With action")
                .key("kit-toast-undo")
                .variant(ButtonVariant::Outline)
                .on_click(|app: &mut App| {
                    let id = app.kit.next_toast;
                    app.kit.next_toast += 1;
                    app.kit
                        .toasts
                        .push(Toast::new(id, "Message archived").action("Undo"));
                }),
        ]
        .gap(8., Axis::Horizontal)],
        toaster(&s.toasts)
            .key("kit-toasts")
            .on_dismiss(|app: &mut App, id| app.kit.toasts.retain(|t| t.id != id))
            .on_action(|app: &mut App, id| {
                app.kit.toasts.retain(|t| t.id != id);
                app.kit.ran = "Undone".into();
            }),
    ]
    .key("kit-toast-stage")
    .flow(Flow::TopLeft)
    .width(640.)
    .height(280.);
    let commands = [
        command("new", "New file")
            .group("File")
            .icon(Icon::File)
            .shortcut("⌘N"),
        command("open", "Open folder…")
            .group("File")
            .icon(Icon::Folder)
            .shortcut("⌘O"),
        command("theme", "Toggle dark mode")
            .group("View")
            .icon(Icon::Moon)
            .keywords(["light", "appearance"]),
        command("settings", "Settings")
            .group("View")
            .icon(Icon::Settings)
            .shortcut("⌘,"),
        command("docs", "Open documentation")
            .group("Help")
            .icon(Icon::ExternalLink),
    ];
    let palette = vstack![
        hstack![
            button("Open command palette")
                .key("kit-palette-open")
                .on_click(|app: &mut App| app.kit.palette.open = true),
            kbd("⌘K"),
        ]
        .gap(10., Axis::Horizontal)
        .align(Align::Center),
        muted(format!("Last command: {}", s.ran)),
        command_palette(commands, &s.palette)
            .key("kit-palette")
            .state(|app: &mut App| &mut app.kit.palette)
            .on_run(|app: &mut App, c| app.kit.ran = c.into()),
    ]
    .gap(10., Axis::Vertical)
    .align(Align::Center);
    let menus = menu(
        "View",
        [
            menu_item("wrap", "Word wrap")
                .checked(s.wrap)
                .shortcut("⌥Z"),
            menu_item("minimap", "Minimap").checked(s.minimap),
            menu_separator(),
            menu_item("zoom-in", "Zoom in")
                .icon(Icon::Plus)
                .shortcut("⌘+"),
            menu_item("zoom-out", "Zoom out")
                .icon(Icon::Minus)
                .shortcut("⌘−"),
            menu_separator(),
            menu_submenu(
                "panels",
                "Panels",
                [
                    menu_item("explorer", "Explorer").shortcut("⇧⌘E"),
                    menu_item("search", "Search").shortcut("⇧⌘F"),
                    menu_item("terminal", "Terminal").shortcut("⌃`"),
                ],
            ),
        ],
    )
    .key("kit-view-menu")
    .width(240.)
    .open(s.view_menu)
    .submenu(s.submenu.clone())
    .on_submenu(|app: &mut App, k| app.kit.submenu = k)
    .on_open_change(|app: &mut App, o| app.kit.view_menu = o)
    .on_action(|app: &mut App, v| {
        match v {
            "wrap" => app.kit.wrap = !app.kit.wrap,
            "minimap" => app.kit.minimap = !app.kit.minimap,
            _ => {}
        }
        app.kit.ran = v.into();
    });
    let hover = hover_card(
        hstack![
            avatar("Ada Lovelace").size(44.),
            vstack![
                text("Ada Lovelace").font_weight(600),
                muted("Wrote the first published algorithm for a machine."),
                hstack![
                    icon(Icon::Calendar).size(13.).color(ColorToken::Muted),
                    muted("Joined 1843")
                ]
                .gap(6., Axis::Horizontal)
                .align(Align::Center),
            ]
            .gap(4., Axis::Vertical)
            .flex(1.),
        ]
        .gap(12., Axis::Horizontal),
        text("@ada")
            .color(ColorToken::Primary)
            .font_weight(500)
            .underline(true),
    )
    .key("kit-hover");
    Doc {
        sections: vec![
            section(
                "disc-collapsible",
                "Collapsible",
                "One section that shows or hides its content.",
                collapse,
                r#"collapsible("What is Volt UI?", body).key("about").open(app.open)
    .on_toggle(|app: &mut App, open| app.open = open)"#,
            ),
            section(
                "disc-accordion",
                "Accordion",
                "Stacked sections; one open at a time unless .multiple(true).",
                faq,
                r#"accordion([("Is it accessible?", answer), ...])
    .key("faq")
    .expanded(app.faq.clone())
    .on_change(|app: &mut App, open| app.faq = open)"#,
            ),
            section(
                "disc-drawer",
                "Drawer",
                "A modal panel attached to a window edge.",
                drawer_demo,
                r#"drawer(body).key("prefs").title("Preferences").side(DrawerSide::Right)
    .open(app.drawer)
    .on_dismiss(|app: &mut App| app.drawer = false)"#,
            ),
            section(
                "disc-toaster",
                "Toaster",
                "Non-modal notifications stacked in a corner; the page stays usable.",
                toasts,
                r#"zstack![page, toaster(&app.toasts)
    .on_dismiss(|app: &mut App, id| app.toasts.retain(|t| t.id != id))]"#,
            ),
            section(
                "disc-palette",
                "Command palette",
                "Type to search; prefix, substring, keyword and fuzzy matches. Enter runs.",
                palette,
                r#"command_palette([command("new", "New file").group("File").shortcut("⌘N")], &app.palette)
    .state(|app: &mut App| &mut app.palette)
    .on_run(|app: &mut App, c| run(app, c))"#,
            ),
            section(
                "disc-menu",
                "Menu items",
                "Check items, icons, shortcut hints and drill-down submenus.",
                menus,
                r#"menu_item("wrap", "Word wrap").checked(app.wrap).shortcut("⌥Z")
menu_submenu("panels", "Panels", [menu_item("explorer", "Explorer")])"#,
            ),
            section(
                "disc-hover",
                "Hover card",
                "A rich preview shown while the target is hovered or focused.",
                hover,
                r#"hover_card(profile_preview, text("@ada")).key("ada")"#,
            ),
        ],
        api: vec![
            ("collapsible(title, content)", ".open / .on_toggle"),
            ("accordion(sections)", ".expanded / .multiple / .on_change"),
            (
                "drawer(content)",
                ".side / .size / .title / .open / .on_dismiss",
            ),
            (
                "toaster(&toasts)",
                ".position / .on_dismiss / .on_action; Toast::new(id, title)",
            ),
            (
                "command_palette(commands, &state)",
                ".state / .on_run; command(v, label)",
            ),
            (
                "menu_item(..).checked / .shortcut / .icon",
                "Rich menu rows.",
            ),
            (
                "menu_submenu(key, label, items)",
                "With menu .submenu(..) / .on_submenu(..).",
            ),
            (
                "hover_card(content, target)",
                "A noninteractive rich preview.",
            ),
        ],
    }
}

// ----------------------------------------------------------- Data display

pub fn display_doc(app: &App) -> Doc {
    let s = &app.kit;
    let people = [
        "Ada Lovelace",
        "Grace Hopper",
        "Alan Turing",
        "Katherine Johnson",
        "Linus T",
        "Margaret Hamilton",
    ];
    let avatars = vstack![
        hstack![
            avatar("Ada Lovelace").size(48.).presence(Presence::Online),
            avatar("Grace Hopper").size(40.).presence(Presence::Away),
            avatar("Alan Turing").size(32.),
            avatar("Volt").size(40.).square(true),
            avatar("Studio").size(40.).image(Some(app.image.clone())),
        ]
        .gap(14., Axis::Horizontal)
        .align(Align::Center),
        avatar_group(people.iter().map(|n| avatar(*n).size(34.)), 4),
    ]
    .gap(18., Axis::Vertical)
    .align(Align::Center);
    let alerts = vstack![
        alert("A new version is available")
            .severity(Severity::Info)
            .description("Volt UI 0.4 adds date pickers, form inputs and charts."),
        alert("Payment received").severity(Severity::Success),
        alert("Your trial ends in 3 days")
            .severity(Severity::Warning)
            .description("Add a payment method to keep your projects.")
            .action(
                button("Upgrade")
                    .key("kit-upgrade")
                    .size(ControlSize::Small)
                    .variant(ButtonVariant::Primary)
                    .on_click(|_: &mut App| {})
            ),
        if s.alert {
            El::from(
                alert("Build failed")
                    .key("kit-alert")
                    .severity(Severity::Error)
                    .description("error[E0308]: mismatched types in src/main.rs:12")
                    .on_close(|app: &mut App| app.kit.alert = false),
            )
        } else {
            button("Show the error again")
                .key("kit-alert-reset")
                .variant(ButtonVariant::Ghost)
                .on_click(|app: &mut App| app.kit.alert = true)
                .into()
        },
    ]
    .gap(10., Axis::Vertical)
    .width(520.);
    let skeletons = hstack![
        skeleton(56., 56.).rounded(Rounding::Full),
        vstack![skeleton(220., 14.), skeleton_text(3, 320.)].gap(12., Axis::Vertical),
    ]
    .gap(16., Axis::Horizontal);
    let keys = hstack![
        kbd("⌘"),
        kbd("⇧"),
        kbd("P"),
        muted("opens the palette ·"),
        kbd("Esc"),
        muted("closes it"),
    ]
    .gap(6., Axis::Horizontal)
    .align(Align::Center);
    let nav = vstack![
        breadcrumb([
            ("Home", "Home"),
            ("Docs", "Docs"),
            ("Components", "Components"),
            ("Installation", "Installation"),
        ])
        .key("kit-crumbs")
        .on_select(|app: &mut App, v: &str| app.kit.crumb = v.into()),
        muted(format!("Went to: {}", s.crumb)),
        pagination(s.page, 20)
            .key("kit-pages")
            .on_change(|app: &mut App, p| app.kit.page = p),
    ]
    .gap(14., Axis::Vertical)
    .align(Align::Center);
    let progress = vstack![
        steps(["Account", "Shipping", "Payment", "Review"], s.step),
        hstack![
            button("Back")
                .key("kit-step-back")
                .variant(ButtonVariant::Outline)
                .enabled(s.step > 0)
                .on_click(|app: &mut App| app.kit.step = app.kit.step.saturating_sub(1)),
            button("Continue")
                .key("kit-step-next")
                .variant(ButtonVariant::Primary)
                .enabled(s.step < 4)
                .on_click(|app: &mut App| app.kit.step = (app.kit.step + 1).min(4)),
        ]
        .gap(8., Axis::Horizontal),
    ]
    .gap(18., Axis::Vertical)
    .width(600.)
    .align(Align::Center);
    let events = vstack![timeline([
        TimelineItem::new("Order placed")
            .time("9:41")
            .description("We've received your order.")
            .icon(Icon::Check),
        TimelineItem::new("Packed").time("13:05").icon(Icon::Inbox),
        TimelineItem::new("Out for delivery")
            .time("Tomorrow")
            .description("Arrives between 10:00 and 12:00.")
            .icon(Icon::Send),
    ])]
    .width(420.);
    let empty = empty_state(
        Icon::Inbox,
        "No messages yet",
        "When someone writes to you, their messages will show up here.",
        [button("Compose")
            .key("kit-compose")
            .variant(ButtonVariant::Primary)
            .on_click(|_: &mut App| {})
            .into()],
    );
    let stats = grid([
        El::from(
            stat("Revenue", "$48,210")
                .delta("12.5%", true)
                .hint("vs last month"),
        ),
        stat("Subscribers", "2,318").delta("4.1%", true).into(),
        stat("Churn", "1.9%").delta("0.3%", false).into(),
    ])
    .columns(3)
    .gap(32., Axis::Both)
    .width(600.);
    let slides: Vec<El<App>> = [
        ("Design in Rust", "#6366F1", "#EC4899"),
        ("Ship native apps", "#0EA5E9", "#10B981"),
        ("Stay fast", "#F59E0B", "#EF4444"),
    ]
    .iter()
    .map(|(title, a, b)| {
        vstack![text(*title)
            .font_size(28.)
            .font_weight(700)
            .color("#FFFFFF")]
        .flow(Flow::Center)
        .background_gradient(Gradient::linear(135., [(0., *a), (1., *b)]))
        .into()
    })
    .collect();
    let slideshow = vstack![carousel(slides, s.slide)
        .key("kit-carousel")
        .height(220.)
        .on_change(|app: &mut App, i| app.kit.slide = i)]
    .width(520.);
    Doc {
        sections: vec![
            section(
                "show-avatar",
                "Avatar",
                "Pictures or initials on a colour derived from the name, with presence.",
                avatars,
                r#"avatar("Ada Lovelace").size(48.).presence(Presence::Online)
avatar_group(people.map(avatar), 4)"#,
            ),
            section(
                "show-alert",
                "Alert",
                "Inline callouts in four severities, with actions or a close button.",
                alerts,
                r#"alert("Build failed").severity(Severity::Error).description(..)
    .on_close(|app: &mut App| app.alert = false)"#,
            ),
            section(
                "show-skeleton",
                "Skeleton",
                "Pulsing placeholders while content loads.",
                skeletons,
                r#"skeleton(220., 14.)
skeleton_text(3, 320.)"#,
            ),
            section(
                "show-kbd",
                "Keyboard keys",
                "Key caps for shortcuts in text and menus.",
                keys,
                r#"kbd("⌘K")"#,
            ),
            section(
                "show-nav",
                "Breadcrumb and pagination",
                "Where you are, and how to move through pages.",
                nav,
                r#"breadcrumb([("home", "Home"), ("docs", "Docs")]).on_select(..)
pagination(app.page, 20).on_change(|app: &mut App, p| app.page = p)"#,
            ),
            section(
                "show-steps",
                "Steps",
                "Progress through a multi-step flow.",
                progress,
                r#"steps(["Account", "Shipping", "Payment", "Review"], app.step)"#,
            ),
            section(
                "show-timeline",
                "Timeline",
                "Events joined by a rail.",
                events,
                r#"timeline([TimelineItem::new("Order placed").time("9:41").icon(Icon::Check)])"#,
            ),
            section(
                "show-empty",
                "Empty state",
                "A friendly placeholder for views with nothing to show.",
                empty,
                r#"empty_state(Icon::Inbox, "No messages yet", "…", [compose_button])"#,
            ),
            section(
                "show-stat",
                "Stat",
                "Key metrics with change indicators.",
                stats,
                r#"stat("Revenue", "$48,210").delta("12.5%", true).hint("vs last month")"#,
            ),
            section(
                "show-carousel",
                "Carousel",
                "One slide at a time, with arrows and dots.",
                slideshow,
                r#"carousel(slides, app.slide).key("hero").on_change(|app: &mut App, i| app.slide = i)"#,
            ),
        ],
        api: vec![
            (
                "avatar(name)",
                ".image / .size / .presence / .square; avatar_group(list, max)",
            ),
            (
                "alert(title)",
                ".severity / .description / .action / .on_close",
            ),
            (
                "skeleton(w, h) / skeleton_text(lines, w)",
                "Pulsing placeholders.",
            ),
            ("kbd(keys)", "A key cap."),
            (
                "breadcrumb(items) / pagination(page, pages)",
                "Navigation aids.",
            ),
            (
                "steps(labels, current) / timeline(items)",
                "Progress and history.",
            ),
            (
                "empty_state(icon, title, text, actions)",
                "Nothing-here views.",
            ),
            ("stat(label, value)", ".delta(text, positive) / .hint"),
            (
                "carousel(slides, index)",
                ".height / .wrap_around / .on_change",
            ),
            (
                "image_view(Option<&Image>, w, h)",
                "An image or a placeholder.",
            ),
        ],
    }
}

// ----------------------------------------------------------------- Charts

pub fn charts_doc() -> Doc {
    let visits = [12., 18., 14., 22., 30., 26., 34., 41., 38., 47., 52., 49.];
    let sparks = hstack![
        vstack![
            stat("Visitors", "4,870").delta("8%", true),
            sparkline(&visits, 180., 44.),
        ]
        .gap(8., Axis::Vertical),
        vstack![
            stat("Errors", "23").delta("31%", false),
            sparkline(&[9., 7., 8., 5., 6., 4., 5., 3., 2., 3.], 180., 44.),
        ]
        .gap(8., Axis::Vertical),
    ]
    .gap(48., Axis::Horizontal);
    let bars = bar_chart(
        [
            ("Mon", 32.),
            ("Tue", 45.),
            ("Wed", 28.),
            ("Thu", 61.),
            ("Fri", 54.),
            ("Sat", 18.),
            ("Sun", 12.),
        ],
        560.,
        220.,
    );
    let lines = line_chart(
        [
            ("Desktop", vec![30., 42., 38., 55., 61., 58., 72., 80.]),
            ("Mobile", vec![12., 18., 26., 24., 35., 47., 52., 64.]),
        ],
        560.,
        220.,
    );
    Doc {
        sections: vec![
            section(
                "chart-spark",
                "Sparkline",
                "A compact trend beside a number.",
                sparks,
                r#"sparkline(&visits, 180., 44.)"#,
            ),
            section(
                "chart-bar",
                "Bar chart",
                "Labelled bars from zero with light gridlines.",
                bars,
                r#"bar_chart([("Mon", 32.), ("Tue", 45.), ...], 560., 220.)"#,
            ),
            section(
                "chart-line",
                "Line chart",
                "Several series on one scale, with a legend.",
                lines,
                r#"line_chart([("Desktop", desktop), ("Mobile", mobile)], 560., 220.)"#,
            ),
        ],
        api: vec![
            ("sparkline(values, w, h)", "Trend line with a soft fill."),
            (
                "bar_chart(bars, w, h)",
                "(label, value) pairs; negative values hang below zero.",
            ),
            (
                "line_chart(series, w, h)",
                "(name, values) series in theme colours.",
            ),
        ],
    }
}

// --------------------------------------------------------------- Utilities

pub fn utilities_doc() -> Doc {
    let tile = |label: &str, colour: ColorToken| -> El<App> {
        vstack![text(label.to_owned()).font_size(13.).font_weight(600)]
            .flow(Flow::Center)
            .height(56.)
            .rounded(8.)
            .background(colour.alpha(16))
            .border_color(colour.alpha(40))
            .border_width(1.)
            .into()
    };
    let grid_demo = grid([
        tile("1", ColorToken::Primary),
        vstack![tile("span 2", ColorToken::Accent)]
            .column_span(2)
            .into(),
        tile("4", ColorToken::Primary),
        tile("5", ColorToken::Primary),
        tile("6", ColorToken::Primary),
    ])
    .columns(3)
    .gap(10., Axis::Both)
    .width(560.);
    let wrap_demo = hstack(
        [
            "Rust",
            "Desktop",
            "GPU",
            "Accessibility",
            "Vello",
            "Typography",
            "Animation",
            "Themes",
            "Native menus",
            "Drag and drop",
        ]
        .iter()
        .map(|t| El::from(badge(*t).rounded(Rounding::Full)))
        .collect::<Vec<_>>(),
    )
    .wrap(true)
    .gap(8., Axis::Both)
    .width(420.);
    let justify_demo = vstack(
        [
            (Justify::Start, "start"),
            (Justify::Center, "center"),
            (Justify::SpaceBetween, "between"),
            (Justify::SpaceEvenly, "evenly"),
        ]
        .into_iter()
        .map(|(j, name)| {
            hstack![
                caption(name).width(64.),
                hstack![
                    vstack![]
                        .width(36.)
                        .height(20.)
                        .rounded(4.)
                        .background(ColorToken::Primary),
                    vstack![]
                        .width(36.)
                        .height(20.)
                        .rounded(4.)
                        .background(ColorToken::Primary),
                    vstack![]
                        .width(36.)
                        .height(20.)
                        .rounded(4.)
                        .background(ColorToken::Primary),
                ]
                .justify(j)
                .flex(1.)
                .padding(6., Edge::All)
                .rounded(6.)
                .border_color(ColorToken::Border)
                .border_width(1.)
                .border_style(BorderStyle::Dashed),
            ]
            .gap(12., Axis::Horizontal)
            .align(Align::Center)
            .into()
        })
        .collect::<Vec<_>>(),
    )
    .gap(8., Axis::Vertical)
    .width(520.);
    let type_demo = vstack![
        text("UPPERCASE TRACKING")
            .uppercase()
            .letter_spacing(0.12)
            .font_size(12.)
            .font_weight(600),
        text("Centred, relaxed line height for a short paragraph of text that wraps.")
            .text_align(TextAlign::Center)
            .line_height(1.6)
            .max_width(360.),
        text("A very long single line that will be truncated with an ellipsis at the end")
            .truncate()
            .max_width(320.),
        hstack![
            text("Underline").underline(true),
            text("Strikethrough")
                .strikethrough(true)
                .color(ColorToken::Muted),
        ]
        .gap(16., Axis::Horizontal),
    ]
    .gap(12., Axis::Vertical)
    .align(Align::Center);
    let style_demo = hstack![
        vstack![]
            .width(120.)
            .height(80.)
            .rounded(12.)
            .background_gradient(Gradient::linear(135., [(0., "#6366F1"), (1., "#EC4899")])),
        vstack![]
            .width(120.)
            .height(80.)
            .rounded(12.)
            .background_gradient(Gradient::radial(
                [0.3, 0.3],
                [(0., "#FDE68A"), (1., "#F97316")]
            )),
        vstack![muted("dashed")]
            .flow(Flow::Center)
            .width(120.)
            .height(80.)
            .rounded(12.)
            .border_color(ColorToken::Border)
            .border_width(2.)
            .border_style(BorderStyle::Dashed),
        vstack![muted("bottom only")]
            .flow(Flow::Center)
            .width(120.)
            .height(80.)
            .border_sides(Sides::BOTTOM)
            .border_color(ColorToken::Primary)
            .border_width(3.),
    ]
    .gap(14., Axis::Horizontal);
    let motion_demo = hstack![
        vstack![icon(Icon::Bolt).size(26.).color(ColorToken::Primary)]
            .flow(Flow::Center)
            .width(72.)
            .height(72.)
            .rounded(16.)
            .background(ColorToken::Primary.alpha(10))
            .ambient(Ambient::float(0., -6.).period_ms(2400)),
        vstack![icon(Icon::Refresh).size(26.)]
            .flow(Flow::Center)
            .width(72.)
            .height(72.)
            .ambient(Ambient::spin().period_ms(2000)),
        button("Hover me")
            .key("kit-lift")
            .variant(ButtonVariant::Outline)
            .cursor(Cursor::Pointer)
            .on_click(|_: &mut App| {})
            .hover_style(|s| s
                .offset(0., -3.)
                .shadow(volt_ui_kit::ui::tokens::shadow::md())),
    ]
    .gap(24., Axis::Horizontal)
    .align(Align::Center);
    Doc {
        sections: vec![
            section(
                "util-grid",
                "Grid",
                "Fixed or auto-fit columns; children can span several.",
                grid_demo,
                r#"grid(tiles).columns(3).gap(10., Axis::Both)
vstack![wide].column_span(2)"#,
            ),
            section(
                "util-wrap",
                "Wrapping rows",
                "Items flow onto the next line when the row is full.",
                wrap_demo,
                r#"hstack(badges).wrap(true).gap(8., Axis::Both)"#,
            ),
            section(
                "util-justify",
                "Justify",
                "Distribute free space along the main axis.",
                justify_demo,
                r#"hstack(items).justify(Justify::SpaceBetween)"#,
            ),
            section(
                "util-type",
                "Typography",
                "Alignment, line height, truncation, case, tracking and decoration.",
                type_demo,
                r#"text(..).uppercase().letter_spacing(0.12)
text(..).text_align(TextAlign::Center).line_height(1.6)
text(..).truncate()"#,
            ),
            section(
                "util-style",
                "Gradients and borders",
                "Linear and radial gradients, dashed borders and single sides.",
                style_demo,
                r##"vstack![].background_gradient(Gradient::linear(135., [(0., "#6366F1"), (1., "#EC4899")]))
vstack![].border_style(BorderStyle::Dashed)
vstack![].border_sides(Sides::BOTTOM)"##,
            ),
            section(
                "util-motion",
                "Ambient motion and hover lift",
                "Float, spin and pulse loops; offsets animate on hover without moving hit targets.",
                motion_demo,
                r#"vstack![..].ambient(Ambient::float(0., -6.).period_ms(2400))
button(..).hover_style(|s| s.offset(0., -3.).shadow(volt_ui_kit::ui::tokens::shadow::md()))"#,
            ),
        ],
        api: vec![
            (
                "grid(children)",
                ".columns(n) / .min_column_width(px); child .column_span(n)",
            ),
            (
                ".wrap(true) / .justify(Justify::…)",
                "Row wrapping and distribution.",
            ),
            (
                ".min_width / .max_width / .width_fraction / .aspect_ratio",
                "Sizing constraints.",
            ),
            (
                ".text_align / .line_height / .max_lines / .truncate",
                "Text layout.",
            ),
            (
                ".uppercase / .letter_spacing / .underline / .strikethrough",
                "Text style.",
            ),
            (
                ".background_gradient(Gradient::linear | radial)",
                "Gradient fills.",
            ),
            (
                ".border_style / .border_sides",
                "Dashed, dotted and single-side borders.",
            ),
            (
                ".offset / .ambient / .cursor",
                "Motion and pointer affordances.",
            ),
            (
                "tokens::{space, radius, text_size, shadow, motion}",
                "Design scales.",
            ),
        ],
    }
}
