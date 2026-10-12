//! Dates and times without an external date crate: a proleptic-Gregorian
//! `CalendarDate`, `TimeOfDay`, and the `calendar`, `date_picker`,
//! `date_range_picker` and `time_picker` components.
//!
//! Pickers are controlled. The app keeps a small `*State` value (is it open,
//! which month is shown) and hands the component an accessor with `.state(..)`
//! so it can open, close and page months itself:
//!
//! ```ignore
//! date_picker(app.due, &app.due_picker)
//!     .key("due")
//!     .state(|app: &mut App| &mut app.due_picker)
//!     .on_change(|app: &mut App, date| app.due = Some(date))
//! ```
use super::*;
use std::{cell::RefCell, rc::Rc};

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// A calendar day. Ordered chronologically.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalendarDate {
    year: i32,
    month: u8,
    day: u8,
}
pub fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        _ => 0,
    }
}
impl CalendarDate {
    /// `None` for impossible dates (month 13, 30 February, years beyond ±9999).
    pub fn new(year: i32, month: u8, day: u8) -> Option<Self> {
        ((-9999..=9999).contains(&year)
            && (1..=12).contains(&month)
            && day >= 1
            && day <= days_in_month(year, month))
        .then_some(Self { year, month, day })
    }
    pub fn year(self) -> i32 {
        self.year
    }
    pub fn month(self) -> u8 {
        self.month
    }
    pub fn day(self) -> u8 {
        self.day
    }
    /// Days since 1970-01-01 (negative before it).
    pub fn days_since_epoch(self) -> i64 {
        // Howard Hinnant's days_from_civil.
        let y = i64::from(self.year) - i64::from(self.month <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = i64::from(self.month);
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(self.day) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }
    pub fn from_days_since_epoch(days: i64) -> Option<Self> {
        // Bound before arithmetic: arbitrary public i64 offsets must not
        // overflow the civil-date algorithm, in debug or release builds.
        let min = Self {
            year: -9999,
            month: 1,
            day: 1,
        }
        .days_since_epoch();
        let max = Self {
            year: 9999,
            month: 12,
            day: 31,
        }
        .days_since_epoch();
        if !(min..=max).contains(&days) {
            return None;
        }
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
        let year = yoe + era * 400 + i64::from(month <= 2);
        Self::new(i32::try_from(year).ok()?, month, day)
    }
    /// 0 = Sunday … 6 = Saturday.
    pub fn weekday(self) -> u8 {
        (self.days_since_epoch() + 4).rem_euclid(7) as u8
    }
    pub fn add_days(self, days: i64) -> Option<Self> {
        Self::from_days_since_epoch(self.days_since_epoch().checked_add(days)?)
    }
    /// Same day in another month, clamped to that month's length.
    pub fn add_months(self, months: i32) -> Option<Self> {
        let index = i64::from(self.year) * 12 + i64::from(self.month) - 1 + i64::from(months);
        let (year, month) = (index.div_euclid(12), (index.rem_euclid(12) + 1) as u8);
        let year = i32::try_from(year).ok()?;
        Self::new(year, month, self.day.min(days_in_month(year, month)))
    }
    /// Today's date in UTC, from the system clock.
    pub fn today_utc() -> Self {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        Self::from_days_since_epoch(secs.div_euclid(86_400)).unwrap_or(Self {
            year: 1970,
            month: 1,
            day: 1,
        })
    }
    /// "12 Oct 2026".
    pub fn short(self) -> String {
        format!(
            "{} {} {}",
            self.day,
            &MONTHS[usize::from(self.month - 1)][..3],
            self.year
        )
    }
    /// "Monday, 12 October 2026".
    pub fn long(self) -> String {
        format!(
            "{}, {} {} {}",
            WEEKDAYS[usize::from(self.weekday())],
            self.day,
            MONTHS[usize::from(self.month - 1)],
            self.year
        )
    }
    /// ISO 8601: "2026-10-12".
    pub fn iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// A time of day in whole minutes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimeOfDay {
    hour: u8,
    minute: u8,
}
impl TimeOfDay {
    pub fn new(hour: u8, minute: u8) -> Option<Self> {
        (hour < 24 && minute < 60).then_some(Self { hour, minute })
    }
    pub fn hour(self) -> u8 {
        self.hour
    }
    pub fn minute(self) -> u8 {
        self.minute
    }
    /// Shift by whole minutes, wrapping around midnight.
    pub fn add_minutes(self, minutes: i32) -> Self {
        let total = (i64::from(self.hour) * 60 + i64::from(self.minute) + i64::from(minutes))
            .rem_euclid(1440);
        Self {
            hour: (total / 60) as u8,
            minute: (total % 60) as u8,
        }
    }
    /// "09:30" or, in 12-hour form, "9:30 AM".
    pub fn format(self, twelve_hour: bool) -> String {
        if twelve_hour {
            let h = match self.hour % 12 {
                0 => 12,
                h => h,
            };
            format!(
                "{h}:{:02} {}",
                self.minute,
                if self.hour < 12 { "AM" } else { "PM" }
            )
        } else {
            format!("{:02}:{:02}", self.hour, self.minute)
        }
    }
}

type Pick<C> = Rc<RefCell<dyn FnMut(&mut C, CalendarDate)>>;
type Page<C> = Rc<RefCell<dyn FnMut(&mut C, i32, u8)>>;
type Access<C, S> = Rc<dyn Fn(&mut C) -> &mut S>;

fn invalid<C>(message: &str) -> Element<C> {
    let mut e = Element::new(Kind::Column);
    e.validation_error = Some(message.into());
    e
}
fn chevron_button<C: 'static>(
    key: String,
    label: &str,
    glyph: Icon,
    action: impl FnMut(&mut C) + 'static,
) -> Element<C> {
    zstack![
        button("")
            .key(key.clone())
            .accessible_label(label)
            .variant(ButtonVariant::Ghost)
            .width(32.)
            .height(32.)
            .on_click(action),
        icon(glyph).size(16.).color(ColorToken::Muted),
    ]
    .key(format!("{key}-layer"))
    .width(32.)
    .height(32.)
    .flow(Flow::Center)
    .into()
}

/// A month grid of selectable days.
pub struct Calendar<C> {
    key: Option<String>,
    year: i32,
    month: u8,
    selected: Option<CalendarDate>,
    range: Option<(CalendarDate, Option<CalendarDate>)>,
    today: Option<CalendarDate>,
    min: Option<CalendarDate>,
    max: Option<CalendarDate>,
    monday_first: bool,
    on_select: Option<Pick<C>>,
    on_month: Option<Page<C>>,
}
/// The month to show; pair with `on_month_change` for ‹ › paging.
pub fn calendar<C>(year: i32, month: u8) -> Calendar<C> {
    Calendar {
        key: None,
        year,
        month,
        selected: None,
        range: None,
        today: None,
        min: None,
        max: None,
        monday_first: false,
        on_select: None,
        on_month: None,
    }
}
impl<C> Calendar<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn selected(mut self, date: Option<CalendarDate>) -> Self {
        self.selected = date;
        self
    }
    /// Highlight a range; `end` may be pending while the user picks it.
    pub fn range(mut self, start: CalendarDate, end: Option<CalendarDate>) -> Self {
        self.range = Some((start, end));
        self
    }
    /// Mark today's date with a ring.
    pub fn today(mut self, date: CalendarDate) -> Self {
        self.today = Some(date);
        self
    }
    /// Earliest and latest selectable days (inclusive).
    pub fn bounds(mut self, min: Option<CalendarDate>, max: Option<CalendarDate>) -> Self {
        self.min = min;
        self.max = max;
        self
    }
    /// Start weeks on Monday instead of Sunday.
    pub fn monday_first(mut self, value: bool) -> Self {
        self.monday_first = value;
        self
    }
    pub fn on_select(mut self, f: impl FnMut(&mut C, CalendarDate) + 'static) -> Self {
        self.on_select = Some(Rc::new(RefCell::new(f)));
        self
    }
    /// Called with the new (year, month) when the user pages ‹ or ›.
    pub fn on_month_change(mut self, f: impl FnMut(&mut C, i32, u8) + 'static) -> Self {
        self.on_month = Some(Rc::new(RefCell::new(f)));
        self
    }
}
impl<C: 'static> From<Calendar<C>> for Element<C> {
    fn from(c: Calendar<C>) -> Self {
        let Some(key) = c.key.filter(|k| !k.is_empty()) else {
            return invalid("calendar requires a key");
        };
        let Some(first) = CalendarDate::new(c.year, c.month, 1) else {
            return invalid("calendar month must be 1..=12 and year within ±9999");
        };
        let Some(pick) = c.on_select else {
            return invalid("calendar requires on_select");
        };
        let mut header: Vec<Element<C>> = vec![];
        if let Some(page) = &c.on_month {
            let (back, next) = (page.clone(), page.clone());
            let (prev_month, next_month) = (first.add_months(-1), first.add_months(1));
            header.push(chevron_button(
                format!("{key}-prev"),
                "Previous month",
                Icon::ChevronLeft,
                move |app: &mut C| {
                    if let Some(d) = prev_month {
                        (back.borrow_mut())(app, d.year, d.month)
                    }
                },
            ));
            header.push(spacer().into());
            header.push(
                text(format!("{} {}", MONTHS[usize::from(c.month - 1)], c.year))
                    .key(format!("{key}-title"))
                    .font_size(14.)
                    .font_weight(600)
                    .into(),
            );
            header.push(spacer().into());
            header.push(chevron_button(
                format!("{key}-next"),
                "Next month",
                Icon::ChevronRight,
                move |app: &mut C| {
                    if let Some(d) = next_month {
                        (next.borrow_mut())(app, d.year, d.month)
                    }
                },
            ));
        } else {
            header.push(
                text(format!("{} {}", MONTHS[usize::from(c.month - 1)], c.year))
                    .key(format!("{key}-title"))
                    .font_size(14.)
                    .font_weight(600)
                    .into(),
            );
        }
        let week_start = u8::from(c.monday_first);
        let names: Vec<Element<C>> = (0..7)
            .map(|i| {
                let day = (week_start + i) % 7;
                text(&WEEKDAYS[usize::from(day)][..2])
                    .key(format!("{key}-wd-{i}"))
                    .font_size(12.)
                    .color(ColorToken::Muted)
                    .text_align(TextAlign::Center)
                    .into()
            })
            .collect();
        let lead = (7 + first.weekday() - week_start) % 7;
        let start = first.add_days(-i64::from(lead)).unwrap_or(first);
        let in_range = |d: CalendarDate| match c.range {
            Some((s, Some(e))) => d >= s.min(e) && d <= s.max(e),
            _ => false,
        };
        let endpoint = |d: CalendarDate| match c.range {
            Some((s, e)) => d == s || Some(d) == e,
            None => false,
        };
        let days: Vec<Element<C>> = (0..42)
            .filter_map(|i| start.add_days(i))
            .map(|d| {
                let selected = c.selected == Some(d) || endpoint(d);
                let outside = d.month != c.month;
                let enabled = c.min.is_none_or(|m| d >= m) && c.max.is_none_or(|m| d <= m);
                let handler = pick.clone();
                let mut b = button(d.day.to_string())
                    .key(format!("{key}-{}", d.iso()))
                    .accessible_label(d.long())
                    .variant(if selected {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Ghost
                    })
                    .height(36.)
                    .font_size(13.)
                    .enabled(enabled)
                    .on_click(move |app: &mut C| (handler.borrow_mut())(app, d));
                if !selected && in_range(d) {
                    b = b.background(ColorToken::Primary.alpha(14));
                }
                if !selected && outside {
                    b = b.color(ColorToken::Muted);
                }
                if c.today == Some(d) && !selected {
                    b = b.border_color(ColorToken::Primary).border_width(1.);
                }
                let mut e: Element<C> = b.into();
                e.node.access_selected = Some(selected);
                e
            })
            .collect();
        vstack![
            hstack(header)
                .key(format!("{key}-header"))
                .align(Align::Center)
                .height(32.),
            grid(names).key(format!("{key}-weekdays")).columns(7),
            grid(days)
                .key(format!("{key}-days"))
                .columns(7)
                .gap(2., Axis::Both)
                .accessible_label(format!("{} {}", MONTHS[usize::from(c.month - 1)], c.year)),
        ]
        .key(key)
        .width(280.)
        .gap(8., Axis::Vertical)
        .into()
    }
}

/// Open/closed and the month on show, kept in the app's state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DatePickerState {
    pub open: bool,
    /// Displayed (year, month); `None` follows the value (or today).
    pub month: Option<(i32, u8)>,
}
/// A field-like button that opens a calendar popover.
pub struct DatePicker<C> {
    key: Option<String>,
    value: Option<CalendarDate>,
    snapshot: DatePickerState,
    state: Option<Access<C, DatePickerState>>,
    on_change: Option<Pick<C>>,
    placeholder: String,
    today: CalendarDate,
    min: Option<CalendarDate>,
    max: Option<CalendarDate>,
    monday_first: bool,
    width: f32,
}
pub fn date_picker<C>(value: Option<CalendarDate>, state: &DatePickerState) -> DatePicker<C> {
    DatePicker {
        key: None,
        value,
        snapshot: state.clone(),
        state: None,
        on_change: None,
        placeholder: "Pick a date".into(),
        today: CalendarDate::today_utc(),
        min: None,
        max: None,
        monday_first: false,
        width: 220.,
    }
}
impl<C> DatePicker<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    /// Where the picker's `DatePickerState` lives in the app state.
    pub fn state(mut self, access: impl Fn(&mut C) -> &mut DatePickerState + 'static) -> Self {
        self.state = Some(Rc::new(access));
        self
    }
    pub fn on_change(mut self, f: impl FnMut(&mut C, CalendarDate) + 'static) -> Self {
        self.on_change = Some(Rc::new(RefCell::new(f)));
        self
    }
    pub fn placeholder(mut self, value: impl Into<String>) -> Self {
        self.placeholder = value.into();
        self
    }
    /// Override "today" (default: the system date in UTC).
    pub fn today(mut self, date: CalendarDate) -> Self {
        self.today = date;
        self
    }
    pub fn bounds(mut self, min: Option<CalendarDate>, max: Option<CalendarDate>) -> Self {
        self.min = min;
        self.max = max;
        self
    }
    pub fn monday_first(mut self, value: bool) -> Self {
        self.monday_first = value;
        self
    }
    pub fn width(mut self, value: f32) -> Self {
        self.width = value;
        self
    }
}
/// The trigger shared by the date pickers: an outline field with an icon.
fn picker_trigger<C: 'static>(
    key: &str,
    label: String,
    placeholder: bool,
    width: f32,
    glyph: Icon,
    toggle: impl FnMut(&mut C) + 'static,
) -> Element<C> {
    zstack![
        button(label.clone())
            .key(key.to_owned())
            .accessible_label(label)
            .variant(ButtonVariant::Outline)
            .label_align(Align::Start)
            .color(if placeholder {
                ColorToken::Muted
            } else {
                ColorToken::BaseContent
            })
            .width(width)
            .height(36.)
            .on_click(toggle),
        hstack![spacer(), icon(glyph).size(16.).color(ColorToken::Muted)]
            .align(Align::Center)
            .padding(12., Edge::Right),
    ]
    .key(format!("{key}-field"))
    .width(width)
    .height(36.)
    .align(Align::Stretch)
    .into()
}
impl<C: 'static> From<DatePicker<C>> for Element<C> {
    fn from(p: DatePicker<C>) -> Self {
        let Some(key) = p.key.filter(|k| !k.is_empty()) else {
            return invalid("date_picker requires a key");
        };
        let (Some(state), Some(change)) = (p.state, p.on_change) else {
            return invalid("date_picker requires state(..) and on_change(..)");
        };
        let shown = p
            .snapshot
            .month
            .or(p.value.map(|v| (v.year, v.month)))
            .unwrap_or((p.today.year, p.today.month));
        let (open_state, close_state, page_state, pick_state) =
            (state.clone(), state.clone(), state.clone(), state);
        let follow = p.value.map(|v| (v.year, v.month));
        let trigger = picker_trigger(
            &key,
            p.value.map_or(p.placeholder.clone(), CalendarDate::short),
            p.value.is_none(),
            p.width,
            Icon::Calendar,
            move |app: &mut C| {
                let s = open_state(app);
                s.open = !s.open;
                if s.open {
                    s.month = follow;
                }
            },
        );
        let cal = calendar(shown.0, shown.1)
            .key(format!("{key}-calendar"))
            .selected(p.value)
            .today(p.today)
            .bounds(p.min, p.max)
            .monday_first(p.monday_first)
            .on_month_change(move |app: &mut C, y, m| page_state(app).month = Some((y, m)))
            .on_select(move |app: &mut C, d| {
                (change.borrow_mut())(app, d);
                pick_state(app).open = false;
            });
        let pop = popover(cal)
            .key(format!("{key}-popover"))
            .anchor(key.clone())
            .open(p.snapshot.open)
            .width(312.)
            .on_dismiss(move |app: &mut C| close_state(app).open = false);
        vstack![trigger, pop].key(format!("{key}-picker")).into()
    }
}

/// A date range being chosen: start first, then end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DateRange {
    pub start: Option<CalendarDate>,
    pub end: Option<CalendarDate>,
}
impl DateRange {
    /// Apply a click: the first click starts a range, the second ends it
    /// (swapping if earlier), a third starts over.
    pub fn pick(self, d: CalendarDate) -> Self {
        match (self.start, self.end) {
            (Some(s), None) if d >= s => Self {
                start: Some(s),
                end: Some(d),
            },
            (Some(s), None) => Self {
                start: Some(d),
                end: Some(s),
            },
            _ => Self {
                start: Some(d),
                end: None,
            },
        }
    }
    pub fn is_complete(self) -> bool {
        self.start.is_some() && self.end.is_some()
    }
    /// Inclusive day count of a complete range.
    pub fn days(self) -> Option<i64> {
        Some(self.end?.days_since_epoch() - self.start?.days_since_epoch() + 1)
    }
}
type RangePick<C> = Rc<RefCell<dyn FnMut(&mut C, DateRange)>>;
/// Two months side by side for picking a start and end date.
pub struct DateRangePicker<C> {
    key: Option<String>,
    value: DateRange,
    snapshot: DatePickerState,
    state: Option<Access<C, DatePickerState>>,
    on_change: Option<RangePick<C>>,
    today: CalendarDate,
    monday_first: bool,
    width: f32,
}
pub fn date_range_picker<C>(value: DateRange, state: &DatePickerState) -> DateRangePicker<C> {
    DateRangePicker {
        key: None,
        value,
        snapshot: state.clone(),
        state: None,
        on_change: None,
        today: CalendarDate::today_utc(),
        monday_first: false,
        width: 260.,
    }
}
impl<C> DateRangePicker<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn state(mut self, access: impl Fn(&mut C) -> &mut DatePickerState + 'static) -> Self {
        self.state = Some(Rc::new(access));
        self
    }
    /// Called on every click with the updated range; it closes once complete.
    pub fn on_change(mut self, f: impl FnMut(&mut C, DateRange) + 'static) -> Self {
        self.on_change = Some(Rc::new(RefCell::new(f)));
        self
    }
    pub fn today(mut self, date: CalendarDate) -> Self {
        self.today = date;
        self
    }
    pub fn monday_first(mut self, value: bool) -> Self {
        self.monday_first = value;
        self
    }
    pub fn width(mut self, value: f32) -> Self {
        self.width = value;
        self
    }
}
impl<C: 'static> From<DateRangePicker<C>> for Element<C> {
    fn from(p: DateRangePicker<C>) -> Self {
        let Some(key) = p.key.filter(|k| !k.is_empty()) else {
            return invalid("date_range_picker requires a key");
        };
        let (Some(state), Some(change)) = (p.state, p.on_change) else {
            return invalid("date_range_picker requires state(..) and on_change(..)");
        };
        let anchor = p.value.start.unwrap_or(p.today);
        let (y, m) = p.snapshot.month.unwrap_or((anchor.year, anchor.month));
        let first = CalendarDate::new(y, m, 1).unwrap_or(anchor);
        let second = first.add_months(1).unwrap_or(first);
        let label = match (p.value.start, p.value.end) {
            (Some(s), Some(e)) => format!("{} – {}", s.short(), e.short()),
            (Some(s), None) => format!("{} – …", s.short()),
            _ => "Pick a date range".into(),
        };
        let value = p.value;
        let toggle_state = state.clone();
        let trigger = picker_trigger(
            &key,
            label,
            value.start.is_none(),
            p.width,
            Icon::Calendar,
            move |app: &mut C| {
                let s = toggle_state(app);
                s.open = !s.open;
            },
        );
        // `shift` maps a panel's paged month back to the first panel's month.
        let month = |which: CalendarDate, suffix: &str, shift: i32| -> Element<C> {
            let (page, close, change) = (state.clone(), state.clone(), change.clone());
            let mut cal = calendar(which.year, which.month)
                .key(format!("{key}-{suffix}"))
                .today(p.today)
                .monday_first(p.monday_first)
                .on_month_change(move |app: &mut C, y, m| {
                    // Paging either panel pages both, keeping them adjacent.
                    if let Some(d) = CalendarDate::new(y, m, 1).and_then(|d| d.add_months(shift)) {
                        page(app).month = Some((d.year, d.month));
                    }
                })
                .on_select(move |app: &mut C, d| {
                    let next = value.pick(d);
                    (change.borrow_mut())(app, next);
                    if next.is_complete() {
                        close(app).open = false;
                    }
                });
            if let Some(start) = value.start {
                cal = cal.range(start, value.end);
            }
            cal.into()
        };
        let close_state = state.clone();
        let months = hstack![month(first, "from", 0), month(second, "to", -1)]
            .key(format!("{key}-months"))
            .gap(20., Axis::Horizontal);
        let pop = popover(months)
            .key(format!("{key}-popover"))
            .anchor(key.clone())
            .open(p.snapshot.open)
            .width(620.)
            .on_dismiss(move |app: &mut C| close_state(app).open = false);
        vstack![trigger, pop].key(format!("{key}-picker")).into()
    }
}
type TimePick<C> = Rc<RefCell<dyn FnMut(&mut C, TimeOfDay)>>;
/// Hour and minute steppers, optionally with AM/PM.
pub struct TimePicker<C> {
    key: Option<String>,
    value: TimeOfDay,
    twelve_hour: bool,
    minute_step: u8,
    on_change: Option<TimePick<C>>,
}
pub fn time_picker<C>(value: TimeOfDay) -> TimePicker<C> {
    TimePicker {
        key: None,
        value,
        twelve_hour: false,
        minute_step: 5,
        on_change: None,
    }
}
impl<C> TimePicker<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn twelve_hour(mut self, value: bool) -> Self {
        self.twelve_hour = value;
        self
    }
    /// Minutes per step of the minute control (1..=30, default 5).
    pub fn minute_step(mut self, value: u8) -> Self {
        self.minute_step = value;
        self
    }
    pub fn on_change(mut self, f: impl FnMut(&mut C, TimeOfDay) + 'static) -> Self {
        self.on_change = Some(Rc::new(RefCell::new(f)));
        self
    }
}
impl<C: 'static> From<TimePicker<C>> for Element<C> {
    fn from(p: TimePicker<C>) -> Self {
        let Some(key) = p.key.filter(|k| !k.is_empty()) else {
            return invalid("time_picker requires a key");
        };
        let Some(change) = p.on_change else {
            return invalid("time_picker requires on_change");
        };
        if !(1..=30).contains(&p.minute_step) {
            return invalid("time_picker minute_step must be 1..=30");
        }
        let v = p.value;
        let step = i32::from(p.minute_step);
        let column = |part: &str, shown: String, up: i32, down: i32, label: &str| -> Element<C> {
            let (inc, dec) = (change.clone(), change.clone());
            vstack![
                chevron_button(
                    format!("{key}-{part}-up"),
                    &format!("Increase {label}"),
                    Icon::ChevronUp,
                    move |app: &mut C| (inc.borrow_mut())(app, v.add_minutes(up)),
                ),
                text(shown)
                    .key(format!("{key}-{part}"))
                    .font_family("SF Mono, monospace")
                    .font_size(22.)
                    .font_weight(500)
                    .text_align(TextAlign::Center),
                chevron_button(
                    format!("{key}-{part}-down"),
                    &format!("Decrease {label}"),
                    Icon::ChevronDown,
                    move |app: &mut C| (dec.borrow_mut())(app, v.add_minutes(down)),
                ),
            ]
            .key(format!("{key}-{part}-column"))
            .width(48.)
            .align(Align::Center)
            .gap(2., Axis::Vertical)
            .into()
        };
        let hour = if p.twelve_hour {
            match v.hour % 12 {
                0 => 12,
                h => h,
            }
        } else {
            v.hour
        };
        // Minute steps snap to the step grid before moving.
        let snap = i32::from(v.minute) % step;
        let mut parts: Vec<Element<C>> = vec![
            column("hour", format!("{hour:02}"), 60, -60, "hour"),
            text(":")
                .key(format!("{key}-colon"))
                .font_size(22.)
                .color(ColorToken::Muted)
                .into(),
            column(
                "minute",
                format!("{:02}", v.minute),
                step - snap,
                if snap == 0 { -step } else { -snap },
                "minutes",
            ),
        ];
        if p.twelve_hour {
            let set = change.clone();
            let mut period = tab_bar([
                tab_item(false, "AM").key(format!("{key}-am")),
                tab_item(true, "PM").key(format!("{key}-pm")),
            ])
            .joined(true);
            period.0.node.access_role = Some(AccessRole::RadioGroup);
            parts.push(
                period
                    .key(format!("{key}-period"))
                    .selected(v.hour >= 12)
                    .on_select(move |app: &mut C, pm| {
                        let shift = match (v.hour >= 12, pm) {
                            (false, true) => 720,
                            (true, false) => -720,
                            _ => 0,
                        };
                        (set.borrow_mut())(app, v.add_minutes(shift))
                    })
                    .into(),
            );
        }
        let mut row = hstack(parts)
            .key(key)
            .gap(6., Axis::Horizontal)
            .align(Align::Center);
        row.0.node.accessible_label = Some(format!("Time {}", v.format(p.twelve_hour)));
        row.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn civil_dates_round_trip_and_know_their_weekday() {
        for (y, m, d, wd) in [
            (1970, 1, 1, 4),
            (2000, 2, 29, 2),
            (2026, 10, 12, 1),
            (1900, 3, 1, 4),
        ] {
            let date = CalendarDate::new(y, m, d).unwrap();
            assert_eq!(date.weekday(), wd, "{y}-{m}-{d}");
            assert_eq!(
                CalendarDate::from_days_since_epoch(date.days_since_epoch()),
                Some(date)
            );
        }
        assert!(CalendarDate::new(2026, 2, 29).is_none());
        assert!(CalendarDate::new(2024, 2, 29).is_some());
        assert!(CalendarDate::new(2026, 13, 1).is_none());
        let jan31 = CalendarDate::new(2026, 1, 31).unwrap();
        assert_eq!(
            jan31.add_months(1),
            CalendarDate::new(2026, 2, 28),
            "clamped"
        );
        assert_eq!(jan31.add_months(-1), CalendarDate::new(2025, 12, 31));
        assert_eq!(jan31.add_days(1), CalendarDate::new(2026, 2, 1));
        assert_eq!(jan31.iso(), "2026-01-31");
        assert_eq!(
            CalendarDate::new(2026, 10, 12).unwrap().long(),
            "Monday, 12 October 2026"
        );
    }
    #[test]
    fn times_wrap_and_format() {
        let t = TimeOfDay::new(23, 50).unwrap();
        assert_eq!(t.add_minutes(15), TimeOfDay::new(0, 5).unwrap());
        assert_eq!(
            TimeOfDay::new(0, 5).unwrap().add_minutes(-10),
            t.add_minutes(5)
        );
        assert_eq!(TimeOfDay::new(0, 7).unwrap().format(true), "12:07 AM");
        assert_eq!(TimeOfDay::new(13, 0).unwrap().format(true), "1:00 PM");
        assert_eq!(TimeOfDay::new(9, 5).unwrap().format(false), "09:05");
        assert!(TimeOfDay::new(24, 0).is_none());
    }
    #[test]
    fn ranges_pick_start_then_end_and_restart() {
        let d = |day| CalendarDate::new(2026, 10, day).unwrap();
        let r = DateRange::default().pick(d(10));
        assert_eq!((r.start, r.end), (Some(d(10)), None));
        let r = r.pick(d(4));
        assert_eq!((r.start, r.end), (Some(d(4)), Some(d(10))), "swapped");
        assert_eq!(r.days(), Some(7));
        let r = r.pick(d(20));
        assert_eq!(
            (r.start, r.end),
            (Some(d(20)), None),
            "third click restarts"
        );
    }
}
