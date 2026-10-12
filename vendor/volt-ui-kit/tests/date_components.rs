#![cfg(feature = "rust-ui")]
use volt_ui_kit::{
    declarative::{Bindings, Handlers, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};
struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, size: f32, max: f32) -> [f32; 2] {
        [(t.chars().count() as f32 * size * 0.5).min(max), size * 1.2]
    }
}
#[derive(Default)]
struct App {
    date: Option<CalendarDate>,
    month: (i32, u8),
    picker: DatePickerState,
    range: DateRange,
    range_picker: DatePickerState,
    time: TimeOfDay,
}
fn view(app: &App) -> Element<App> {
    let today = CalendarDate::new(2026, 10, 12).unwrap();
    vstack![
        calendar(app.month.0, app.month.1)
            .key("cal")
            .selected(app.date)
            .today(today)
            .bounds(None, CalendarDate::new(2026, 10, 25))
            .on_select(|app: &mut App, d| app.date = Some(d))
            .on_month_change(|app: &mut App, y, m| app.month = (y, m)),
        date_picker(app.date, &app.picker)
            .key("due")
            .today(today)
            .state(|app: &mut App| &mut app.picker)
            .on_change(|app: &mut App, d| app.date = Some(d)),
        date_range_picker(app.range, &app.range_picker)
            .key("trip")
            .today(today)
            .state(|app: &mut App| &mut app.range_picker)
            .on_change(|app: &mut App, r| app.range = r),
        time_picker(app.time)
            .key("alarm")
            .twelve_hour(true)
            .on_change(|app: &mut App, t| app.time = t),
    ]
    .key("page")
    .gap(12., Axis::Vertical)
    .into()
}
struct Harness {
    app: App,
    runtime: Runtime,
    handlers: Handlers<'static, App>,
}
impl Harness {
    fn new() -> Self {
        let app = App {
            month: (2026, 10),
            ..Default::default()
        };
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
                Viewport::new(900., 1400., 1.).unwrap(),
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
    fn click(&mut self, id: &str) {
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
        assert!(outcome.activated, "{id} did not activate");
        self.rebuild();
    }
    fn has(&self, id: &str) -> bool {
        self.runtime.boxes().iter().any(|b| b.id == id)
    }
}

#[test]
fn calendar_selects_days_pages_months_and_respects_bounds() {
    let mut h = Harness::new();
    // Six weeks of days, starting on the Sunday before 1 October 2026.
    assert!(h.has("cal-2026-09-27"));
    assert!(h.has("cal-2026-11-07"));
    h.click("cal-2026-10-14");
    assert_eq!(h.app.date, CalendarDate::new(2026, 10, 14));
    let disabled = h
        .runtime
        .boxes()
        .iter()
        .find(|b| b.id == "cal-2026-10-28")
        .unwrap()
        .enabled;
    assert!(!disabled, "after the max bound");
    h.click("cal-next");
    assert_eq!(h.app.month, (2026, 11));
    h.click("cal-prev");
    h.click("cal-prev");
    assert_eq!(h.app.month, (2026, 9));
}

#[test]
fn date_picker_opens_picks_and_closes() {
    let mut h = Harness::new();
    assert!(!h.has("due-calendar-2026-10-20"));
    h.click("due");
    assert!(h.app.picker.open);
    h.click("due-calendar-next");
    assert_eq!(h.app.picker.month, Some((2026, 11)));
    h.click("due-calendar-2026-11-20");
    assert_eq!(h.app.date, CalendarDate::new(2026, 11, 20));
    assert!(!h.app.picker.open, "picking closes the popover");
}

#[test]
fn range_picker_collects_start_and_end_across_two_months() {
    let mut h = Harness::new();
    h.click("trip");
    h.click("trip-from-2026-10-30");
    assert!(h.app.range_picker.open, "still open after the start");
    h.click("trip-to-2026-11-03");
    assert_eq!(h.app.range.days(), Some(5));
    assert!(!h.app.range_picker.open);
}

#[test]
fn time_picker_steps_hours_minutes_and_meridiem() {
    let mut h = Harness::new();
    h.click("alarm-hour-up");
    assert_eq!(h.app.time, TimeOfDay::new(1, 0).unwrap());
    h.click("alarm-minute-up");
    assert_eq!(h.app.time, TimeOfDay::new(1, 5).unwrap());
    h.click("alarm-minute-down");
    h.click("alarm-minute-down");
    assert_eq!(h.app.time, TimeOfDay::new(0, 55).unwrap());
    h.click("alarm-pm");
    assert_eq!(h.app.time, TimeOfDay::new(12, 55).unwrap());
}

#[test]
fn pickers_require_keys_and_handlers() {
    let state = DatePickerState::default();
    let missing: Element<App> = date_picker(None, &state).key("x").into();
    assert!(missing.build().is_err());
    let keyless: Element<App> = calendar(2026, 10).on_select(|_: &mut App, _| {}).into();
    assert!(keyless.build().is_err());
    let bad_month: Element<App> = calendar(2026, 13)
        .key("c")
        .on_select(|_: &mut App, _| {})
        .into();
    assert!(bad_month.build().is_err());
}
