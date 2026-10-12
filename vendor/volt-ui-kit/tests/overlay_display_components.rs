#![cfg(feature = "rust-ui")]
use volt_ui_kit::{
    declarative::{Bindings, EditKey, Handlers, Runtime, SelectionKey, TextMetrics},
    ui::prelude::*,
    Viewport,
};
struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, size: f32, max: f32) -> [f32; 2] {
        [(t.chars().count() as f32 * size * 0.5).min(max), size * 1.2]
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Cmd {
    NewFile,
    Open,
    Theme,
}
#[derive(Default)]
struct App {
    details: bool,
    faq: Vec<usize>,
    drawer: bool,
    toasts: Vec<Toast>,
    acted: Vec<u64>,
    palette: CommandPaletteState,
    ran: Vec<Cmd>,
    menu_open: bool,
    submenu: Option<String>,
    wrap: bool,
    picked: Vec<&'static str>,
    page: usize,
    crumb: Option<u8>,
    slide: usize,
    alert_closed: bool,
}
fn view(app: &App) -> Element<App> {
    let page = vstack![
        collapsible("Details", text("Hidden body"))
            .key("details")
            .open(app.details)
            .on_toggle(|app: &mut App, open| app.details = open),
        accordion([
            ("Shipping", Element::from(text("Ships in 2 days"))),
            ("Returns", text("30 days").into()),
            ("Warranty", text("1 year").into()),
        ])
        .key("faq")
        .expanded(app.faq.clone())
        .on_change(|app: &mut App, open| app.faq = open),
        button("Settings")
            .key("open-drawer")
            .on_click(|app: &mut App| app.drawer = true),
        button("Palette")
            .key("open-palette")
            .on_click(|app: &mut App| app.palette.open = true),
        menu(
            "View",
            [
                menu_item("wrap", "Word wrap")
                    .checked(app.wrap)
                    .shortcut("⌥Z"),
                menu_item("zoom", "Zoom in").icon(Icon::Plus).shortcut("⌘+"),
                menu_separator(),
                menu_submenu(
                    "panels",
                    "Panels",
                    [
                        menu_item("files", "Files").key("files"),
                        menu_item("search", "Search")
                    ]
                ),
            ]
        )
        .key("view")
        .open(app.menu_open)
        .submenu(app.submenu.clone())
        .on_submenu(|app: &mut App, s| app.submenu = s)
        .on_open_change(|app: &mut App, o| app.menu_open = o)
        .on_action(|app: &mut App, v| {
            if v == "wrap" {
                app.wrap = !app.wrap;
            }
            app.picked.push(v);
        }),
        pagination(app.page.max(1), 12)
            .key("pages")
            .on_change(|app: &mut App, p| app.page = p),
        breadcrumb([(1u8, "Home"), (2, "Docs"), (3, "Install")])
            .key("crumbs")
            .on_select(|app: &mut App, v| app.crumb = Some(v)),
        carousel(
            (0..3).map(|i| Element::from(text(format!("Slide {i}")))),
            app.slide
        )
        .key("hero")
        .on_change(|app: &mut App, i| app.slide = i),
        alert("Heads up")
            .key("notice")
            .severity(Severity::Warning)
            .description("Something changed")
            .on_close(|app: &mut App| app.alert_closed = true),
    ]
    .key("page")
    .gap(8., Axis::Vertical);
    zstack![
        page,
        drawer(text("Drawer body"))
            .key("settings")
            .title("Settings")
            .open(app.drawer)
            .on_dismiss(|app: &mut App| app.drawer = false),
        command_palette(
            [
                command(Cmd::NewFile, "New file")
                    .group("File")
                    .shortcut("⌘N"),
                command(Cmd::Open, "Open…").group("File").icon(Icon::Folder),
                command(Cmd::Theme, "Toggle theme")
                    .group("View")
                    .keywords(["dark", "light"]),
            ],
            &app.palette
        )
        .key("palette")
        .state(|app: &mut App| &mut app.palette)
        .on_run(|app: &mut App, c| app.ran.push(c)),
        toaster(&app.toasts)
            .key("toasts")
            .on_dismiss(|app: &mut App, id| app.toasts.retain(|t| t.id != id))
            .on_action(|app: &mut App, id| app.acted.push(id)),
    ]
    .key("root")
    .align(Align::Stretch)
    .into()
}
struct Harness {
    app: App,
    runtime: Runtime,
    handlers: Handlers<'static, App>,
}
impl Harness {
    fn new(app: App) -> Self {
        let (d, handlers) = view(&app).build().unwrap().into_parts();
        let runtime = Runtime::new(d, &handlers, &Bindings::default()).unwrap();
        let mut h = Self {
            app,
            runtime,
            handlers,
        };
        h.layout();
        h
    }
    fn layout(&mut self) {
        self.runtime
            .layout(
                Viewport::new(1000., 1600., 1.).unwrap(),
                &Bindings::default(),
                &mut Metrics,
            )
            .unwrap();
    }
    fn rebuild(&mut self) {
        let (d, handlers) = view(&self.app).build().unwrap().into_parts();
        self.runtime
            .replace_document(d, &handlers, &Bindings::default())
            .unwrap();
        self.handlers = handlers;
        self.layout();
    }
    fn press(&mut self, id: &str) -> bool {
        let b = self
            .runtime
            .boxes()
            .iter()
            .find(|b| b.id == id)
            .unwrap_or_else(|| panic!("no {id}"))
            .clone();
        let [x, y, w, h] = b.bounds.bounds();
        let (x, y) = (x + w * 0.5, y + h * 0.5);
        self.runtime.pointer_down(x, y);
        let outcome = self
            .runtime
            .pointer_up_outcome(x, y, &mut self.handlers, &mut self.app)
            .unwrap();
        self.rebuild();
        outcome.activated
    }
    fn click(&mut self, id: &str) {
        assert!(self.press(id), "{id} did not activate");
    }
    fn has(&self, id: &str) -> bool {
        self.runtime.boxes().iter().any(|b| b.id == id)
    }
}

#[test]
fn collapsible_and_accordion_toggle_sections() {
    let mut h = Harness::new(App::default());
    assert!(!h.has("details-content"));
    h.click("details-trigger");
    assert!(h.app.details && h.has("details-content"));
    h.click("faq-0-trigger");
    assert_eq!(h.app.faq, [0]);
    h.click("faq-2-trigger");
    assert_eq!(h.app.faq, [2], "single mode keeps one open");
    h.click("faq-2-trigger");
    assert!(h.app.faq.is_empty());
}

#[test]
fn drawer_opens_modally_and_closes() {
    let mut h = Harness::new(App::default());
    assert!(!h.has("settings-content"));
    h.click("open-drawer");
    assert!(h.has("settings-content"));
    let content = h
        .runtime
        .boxes()
        .iter()
        .find(|b| b.id == "settings-content")
        .unwrap()
        .bounds
        .bounds();
    assert_eq!(content[0] + content[2], 1000., "attached to the right edge");
    assert_eq!(content[3], 1600., "full height");
    // The page is inert: a press outside the panel only dismisses the drawer.
    h.press("open-palette");
    assert!(!h.app.palette.open && !h.app.drawer);
    h.click("open-drawer");
    h.click("settings-close");
    assert!(!h.app.drawer);
}

#[test]
fn toasts_dismiss_and_act_without_blocking_the_page() {
    let mut h = Harness::new(App {
        toasts: vec![
            Toast::new(1, "Saved").tone(Severity::Success),
            Toast::new(2, "Deleted").action("Undo"),
        ],
        ..Default::default()
    });
    h.click("toasts-2-action");
    assert_eq!(h.app.acted, [2]);
    h.click("toasts-1-close");
    assert_eq!(h.app.toasts.len(), 1);
    h.click("details-trigger");
    assert!(h.app.details, "page stays interactive under the toaster");
}

#[test]
fn command_palette_filters_runs_and_closes() {
    let mut h = Harness::new(App::default());
    h.click("open-palette");
    assert_eq!(
        h.runtime.focused(),
        Some("palette-field"),
        "search is focused"
    );
    assert!(h.has("palette-group-0") && h.has("palette-cmd-2"));
    h.runtime
        .text_input("dark", &mut h.handlers, &mut h.app)
        .unwrap();
    h.rebuild();
    assert!(h.has("palette-cmd-2") && !h.has("palette-cmd-0"));
    h.runtime
        .editor_key(EditKey::Enter, false, &mut h.handlers, &mut h.app)
        .unwrap();
    h.rebuild();
    assert_eq!(h.app.ran, [Cmd::Theme]);
    assert!(!h.app.palette.open && h.app.palette.query.is_empty());
    h.click("open-palette");
    h.click("palette-cmd-1");
    assert_eq!(h.app.ran, [Cmd::Theme, Cmd::Open]);
}

#[test]
fn menus_show_checks_shortcuts_and_drill_into_submenus() {
    let mut h = Harness::new(App::default());
    h.click("view-trigger");
    assert!(h.has("view-popup-content"));
    // Decorated rows keep their button first; keyboard navigation reaches them.
    let focused = h.runtime.focused().map(str::to_owned);
    h.runtime.overlay_key(SelectionKey::Next);
    assert_ne!(h.runtime.focused().map(str::to_owned), focused);
    h.click("panels");
    assert_eq!(h.app.submenu.as_deref(), Some("panels"));
    assert!(h.has("view-popup-back"));
    h.click("view-popup-back");
    assert_eq!(h.app.submenu, None);
    h.click("panels");
    h.click("files");
    assert_eq!(h.app.picked, ["files"]);
    assert!(!h.app.menu_open && h.app.submenu.is_none());
}

#[test]
fn pagination_breadcrumb_carousel_and_alert() {
    let mut h = Harness::new(App {
        page: 1,
        ..Default::default()
    });
    assert!(
        !h.runtime
            .boxes()
            .iter()
            .find(|b| b.id == "pages-prev")
            .unwrap()
            .enabled
    );
    h.click("pages-next");
    assert_eq!(h.app.page, 2);
    h.click("pages-12");
    assert_eq!(h.app.page, 12);
    h.click("crumbs-1");
    assert_eq!(h.app.crumb, Some(2));
    assert!(h.has("crumbs-current"));
    h.click("hero-prev");
    assert_eq!(h.app.slide, 2, "wraps around");
    h.click("hero-dot-1");
    assert_eq!(h.app.slide, 1);
    assert!(h.has("hero-slide-1"));
    h.click("notice-close");
    assert!(h.app.alert_closed);
}

#[test]
fn display_components_build() {
    let e: Element<()> = vstack![
        avatar("Ada Lovelace").presence(Presence::Online),
        avatar_group((0..6).map(|i| avatar(format!("User {i}")).size(28.)), 4),
        skeleton(200., 16.),
        skeleton_text(3, 240.),
        kbd("⌘K"),
        steps(["Cart", "Shipping", "Pay"], 1),
        timeline([
            TimelineItem::new("Created").time("9:00"),
            TimelineItem::new("Shipped")
        ]),
        empty_state(Icon::Inbox, "No messages", "You're all caught up.", []),
        stat("Revenue", "$12,400").delta("+8%", true),
        sparkline(&[1., 3., 2., 5.], 120., 32.),
        bar_chart([("Mon", 3.), ("Tue", 5.)], 240., 140.),
        line_chart(
            [("A", vec![1., 2., 3.]), ("B", vec![3., 1., 2.])],
            240.,
            140.
        ),
        image_view(None, 160., 90.),
        hover_card(text("Preview"), text("Hover me")).key("hc"),
    ]
    .key("all")
    .into();
    e.build().unwrap();
    let bad: Vec<Element<()>> = vec![
        pagination(0, 3).key("p").on_change(|_, _| {}).into(),
        carousel(Vec::<Element<()>>::new(), 0)
            .key("c")
            .on_change(|_, _| {})
            .into(),
        accordion(Vec::<(String, Element<()>)>::new())
            .key("a")
            .on_change(|_, _| {})
            .into(),
        drawer(text("x")).key("d").open(true).into(),
        toaster(&[Toast::new(1, "x").action("Undo")])
            .on_dismiss(|_, _| {})
            .into(),
        menu("m", [menu_submenu("s", "S", [menu_item(1, "x")])])
            .key("m")
            .on_action(|_, _| {})
            .on_open_change(|_, _| {})
            .into(),
    ];
    for (i, e) in bad.into_iter().enumerate() {
        assert!(e.build().is_err(), "case {i} should be rejected");
    }
}
