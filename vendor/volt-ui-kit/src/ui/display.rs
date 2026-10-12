//! Display components: avatars, alerts, skeletons, key caps, breadcrumbs,
//! pagination, steps, timelines, empty states, stats, small charts, a
//! carousel and images with a fallback.
use super::*;
use crate::graphics::{Clip, Image, ImageFit};
use std::{cell::RefCell, rc::Rc};

fn invalid<C>(message: &str) -> Element<C> {
    let mut e = Element::new(Kind::Column);
    e.validation_error = Some(message.into());
    e
}

/// Status colour and icon shared by alerts, toasts and badges.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Severity {
    #[default]
    Neutral,
    Info,
    Success,
    Warning,
    Error,
}
impl Severity {
    pub(super) fn style(self) -> (Option<Icon>, ColorToken) {
        match self {
            Self::Neutral => (None, ColorToken::BaseContent),
            Self::Info => (Some(Icon::Info), ColorToken::Info),
            Self::Success => (Some(Icon::Success), ColorToken::Success),
            Self::Warning => (Some(Icon::Warning), ColorToken::Warning),
            Self::Error => (Some(Icon::Error), ColorToken::Error),
        }
    }
}

// ── Avatar ──────────────────────────────────────────────────────────────

/// Online presence dot on an avatar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presence {
    Online,
    Away,
    Busy,
    Offline,
}
/// A circular picture, or initials on a colour derived from the name.
pub struct Avatar {
    name: String,
    image: Option<Image>,
    size: f32,
    presence: Option<Presence>,
    square: bool,
}
pub fn avatar(name: impl Into<String>) -> Avatar {
    Avatar {
        name: name.into(),
        image: None,
        size: 40.,
        presence: None,
        square: false,
    }
}
impl Avatar {
    pub fn image(mut self, image: Option<Image>) -> Self {
        self.image = image;
        self
    }
    pub fn size(mut self, value: f32) -> Self {
        self.size = value;
        self
    }
    pub fn presence(mut self, value: Presence) -> Self {
        self.presence = Some(value);
        self
    }
    /// Rounded square instead of a circle.
    pub fn square(mut self, value: bool) -> Self {
        self.square = value;
        self
    }
}
/// Up to two initials: first letters of the first and last words.
pub(super) fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let first = |w: &str| w.chars().next().map(|c| c.to_uppercase().to_string());
    match words.as_slice() {
        [] => "?".into(),
        [one] => first(one).unwrap_or_default(),
        [a, .., b] => format!(
            "{}{}",
            first(a).unwrap_or_default(),
            first(b).unwrap_or_default()
        ),
    }
}
/// A stable hue for a name, from a small pleasant palette.
fn avatar_colour(name: &str) -> &'static str {
    const PALETTE: [&str; 8] = [
        "#6366F1", "#0EA5E9", "#10B981", "#F59E0B", "#EF4444", "#EC4899", "#8B5CF6", "#14B8A6",
    ];
    let hash = name.bytes().fold(2166136261u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(16777619)
    });
    PALETTE[hash as usize % PALETTE.len()]
}
impl<C> From<Avatar> for Element<C> {
    fn from(a: Avatar) -> Self {
        let s = a.size;
        let shape = if a.square {
            Rounding::Radius(s * 0.22)
        } else {
            Rounding::Full
        };
        let face: Element<C> = match a.image {
            Some(image) => vstack![]
                .width(s)
                .height(s)
                .rounded(shape)
                .clip(if a.square {
                    Clip::Rounded
                } else {
                    Clip::Circle
                })
                .background_image(image.background().fit(ImageFit::Cover))
                .into(),
            None => vstack![text(initials(&a.name))
                .font_size((s * 0.4).round())
                .font_weight(600)
                .color("#FFFFFF")]
            .width(s)
            .height(s)
            .flow(Flow::Center)
            .rounded(shape)
            .background(avatar_colour(&a.name))
            .into(),
        };
        let mut layers = vec![face];
        if let Some(p) = a.presence {
            let colour = match p {
                Presence::Online => "#22C55E",
                Presence::Away => "#F59E0B",
                Presence::Busy => "#EF4444",
                Presence::Offline => "#9CA3AF",
            };
            let dot = (s * 0.28).max(8.);
            layers.push(
                zstack![vstack![]
                    .width(dot)
                    .height(dot)
                    .rounded(Rounding::Full)
                    .background(colour)
                    .border_color(ColorToken::Base100)
                    .border_width(2.)]
                .width(s)
                .height(s)
                .flow(Flow::BottomRight)
                .into(),
            );
        }
        let mut e: Element<C> = zstack(layers).width(s).height(s).into();
        e.node.accessible_label = Some(a.name);
        e
    }
}
/// Overlapping avatars with a "+N" counter past `max`.
pub fn avatar_group<C>(avatars: impl IntoIterator<Item = Avatar>, max: usize) -> Element<C> {
    let all: Vec<Avatar> = avatars.into_iter().collect();
    let size = all.first().map_or(32., |a| a.size);
    let extra = all.len().saturating_sub(max);
    let step = size * 0.68;
    let mut layers: Vec<Element<C>> = Vec::new();
    let shown = all.len().min(max);
    for (i, a) in all.into_iter().take(max).enumerate() {
        layers.push(
            hstack![vstack![Element::from(a)]
                .rounded(Rounding::Full)
                .border_color(ColorToken::Base100)
                .border_width(2.)]
            .padding(i as f32 * step, Edge::Left)
            .into(),
        );
    }
    if extra > 0 {
        layers.push(
            hstack![vstack![text(format!("+{extra}"))
                .font_size((size * 0.36).round())
                .font_weight(600)]
            .width(size)
            .height(size)
            .flow(Flow::Center)
            .rounded(Rounding::Full)
            .background(ColorToken::Base300)
            .border_color(ColorToken::Base100)
            .border_width(2.)]
            .padding(shown as f32 * step, Edge::Left)
            .into(),
        );
    }
    let count = shown + usize::from(extra > 0);
    zstack(layers)
        .flow(Flow::TopLeft)
        .width(size + count.saturating_sub(1) as f32 * step)
        .height(size)
        .into()
}

// ── Alert ───────────────────────────────────────────────────────────────

/// An inline callout with a tone, icon, text and optional actions.
pub struct Alert<C> {
    title: String,
    description: Option<String>,
    severity: Severity,
    actions: Vec<Element<C>>,
    on_close: Option<Click<C>>,
    key: Option<String>,
}
pub fn alert<C>(title: impl Into<String>) -> Alert<C> {
    Alert {
        title: title.into(),
        description: None,
        severity: Severity::Info,
        actions: Vec::new(),
        on_close: None,
        key: None,
    }
}
impl<C> Alert<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn description(mut self, value: impl Into<String>) -> Self {
        self.description = Some(value.into());
        self
    }
    pub fn severity(mut self, value: Severity) -> Self {
        self.severity = value;
        self
    }
    pub fn action(mut self, control: impl Into<Element<C>>) -> Self {
        self.actions.push(control.into());
        self
    }
    /// Shows a close button (requires a key).
    pub fn on_close(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.on_close = Some(Box::new(f));
        self
    }
}
impl<C: 'static> From<Alert<C>> for Element<C> {
    fn from(a: Alert<C>) -> Self {
        if a.on_close.is_some() && a.key.as_deref().is_none_or(str::is_empty) {
            return invalid("a closable alert requires a key");
        }
        let (glyph, tint) = a.severity.style();
        let mut copy: Vec<Element<C>> = vec![text(a.title).font_size(14.).font_weight(600).into()];
        if let Some(d) = a.description {
            copy.push(text(d).font_size(13.).color(ColorToken::Muted).into());
        }
        if !a.actions.is_empty() {
            copy.push(
                hstack(a.actions)
                    .gap(8., Axis::Horizontal)
                    .padding(4., Edge::Top)
                    .into(),
            );
        }
        let mut row: Vec<Element<C>> = Vec::new();
        row.push(
            icon(glyph.unwrap_or(Icon::Bell))
                .size(18.)
                .color(tint)
                .into(),
        );
        row.push(vstack(copy).gap(3., Axis::Vertical).flex(1.).into());
        if let (Some(close), Some(key)) = (a.on_close, &a.key) {
            let mut b: Element<C> = button("")
                .key(format!("{key}-close"))
                .accessible_label("Dismiss")
                .variant(ButtonVariant::Ghost)
                .width(24.)
                .height(24.)
                .into();
            b.callback = Some(Callback::Click(close));
            row.push(
                zstack![b, icon(Icon::Close).size(13.).color(ColorToken::Muted)]
                    .key(format!("{key}-close-layer"))
                    .width(24.)
                    .height(24.)
                    .flow(Flow::Center)
                    .into(),
            );
        }
        let neutral = a.severity == Severity::Neutral;
        let mut e = hstack(row)
            .gap(12., Axis::Horizontal)
            .align(Align::Start)
            .padding(14., Edge::All)
            .rounded(10.)
            .background(if neutral {
                ColorToken::Base200.into()
            } else {
                tint.alpha(10)
            })
            .border_color(if neutral {
                ColorToken::Border.into()
            } else {
                tint.alpha(35)
            })
            .border_width(1.);
        if let Some(key) = a.key {
            e = e.key(key);
        }
        e.into()
    }
}

// ── Skeleton, kbd ───────────────────────────────────────────────────────

/// A pulsing placeholder block shown while content loads.
pub fn skeleton<C>(width: f32, height: f32) -> Container<C> {
    vstack![]
        .width(width)
        .height(height)
        .rounded(6.)
        .background(ColorToken::BaseContent.alpha(10))
        .ambient(Ambient::pulse(0.45).period_ms(1400))
}
/// Placeholder text lines; the last is shorter, like a paragraph's end.
pub fn skeleton_text<C>(lines: usize, width: f32) -> Element<C> {
    let rows: Vec<Element<C>> = (0..lines.max(1))
        .map(|i| {
            let w = if i + 1 == lines.max(1) && lines > 1 {
                width * 0.6
            } else {
                width
            };
            skeleton(w, 12.).into()
        })
        .collect();
    vstack(rows).gap(8., Axis::Vertical).into()
}
/// A keyboard key cap, e.g. `kbd("⌘K")`.
pub fn kbd<C>(keys: impl Into<String>) -> Container<C> {
    hstack![text(keys)
        .font_size(11.)
        .font_weight(500)
        .color(ColorToken::Muted)]
    .padding(6., Edge::Horizontal)
    .height(20.)
    .align(Align::Center)
    .rounded(5.)
    .background(ColorToken::Base100)
    .border_color(ColorToken::Border)
    .border_width(1.)
    .border_sides(Sides::ALL)
}

// ── Breadcrumb ──────────────────────────────────────────────────────────

type Pick<C, T> = Rc<RefCell<dyn FnMut(&mut C, T)>>;
/// A path of links; the last item is the current page and not clickable.
pub struct Breadcrumb<C, T> {
    key: Option<String>,
    items: Vec<(T, String)>,
    on_select: Option<Pick<C, T>>,
}
pub fn breadcrumb<C, T>(
    items: impl IntoIterator<Item = (T, impl Into<String>)>,
) -> Breadcrumb<C, T> {
    Breadcrumb {
        key: None,
        items: items.into_iter().map(|(v, l)| (v, l.into())).collect(),
        on_select: None,
    }
}
impl<C, T> Breadcrumb<C, T> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn on_select(mut self, f: impl FnMut(&mut C, T) + 'static) -> Self {
        self.on_select = Some(Rc::new(RefCell::new(f)));
        self
    }
}
impl<C: 'static, T: Clone + 'static> From<Breadcrumb<C, T>> for Element<C> {
    fn from(b: Breadcrumb<C, T>) -> Self {
        let Some(key) = b.key.filter(|k| !k.is_empty()) else {
            return invalid("breadcrumb requires a key");
        };
        let Some(select) = b.on_select else {
            return invalid("breadcrumb requires on_select");
        };
        let last = b.items.len().saturating_sub(1);
        let mut parts: Vec<Element<C>> = Vec::new();
        for (i, (value, label)) in b.items.into_iter().enumerate() {
            if i > 0 {
                parts.push(
                    icon(Icon::ChevronRight)
                        .size(13.)
                        .color(ColorToken::Muted)
                        .into(),
                );
            }
            if i == last {
                parts.push(
                    text(label)
                        .key(format!("{key}-current"))
                        .font_size(13.)
                        .font_weight(500)
                        .into(),
                );
            } else {
                let pick = select.clone();
                parts.push(
                    button(label)
                        .key(format!("{key}-{i}"))
                        .variant(ButtonVariant::Ghost)
                        .height(26.)
                        .font_size(13.)
                        .color(ColorToken::Muted)
                        .on_click(move |app: &mut C| (pick.borrow_mut())(app, value.clone()))
                        .into(),
                );
            }
        }
        let mut row = hstack(parts)
            .key(key)
            .gap(2., Axis::Horizontal)
            .align(Align::Center);
        row.0.node.accessible_label = Some("Breadcrumb".into());
        row.into()
    }
}

// ── Pagination ──────────────────────────────────────────────────────────

/// Page numbers to show: first, last, a window around the current page,
/// and `None` for each gap (rendered as "…"). Pages are 1-based.
pub(super) fn page_window(page: usize, pages: usize) -> Vec<Option<usize>> {
    if pages <= 7 {
        return (1..=pages).map(Some).collect();
    }
    let (lo, hi) = if page <= 4 {
        (2, 5)
    } else if page >= pages - 3 {
        (pages - 4, pages - 1)
    } else {
        (page - 1, page + 1)
    };
    let mut out = vec![Some(1)];
    if lo > 2 {
        out.push(None);
    }
    out.extend((lo..=hi).map(Some));
    if hi < pages - 1 {
        out.push(None);
    }
    out.push(Some(pages));
    out
}
type Page<C> = Rc<RefCell<dyn FnMut(&mut C, usize)>>;
/// Previous / numbered / next page buttons. Pages are 1-based.
pub struct Pagination<C> {
    key: Option<String>,
    page: usize,
    pages: usize,
    on_change: Option<Page<C>>,
}
pub fn pagination<C>(page: usize, pages: usize) -> Pagination<C> {
    Pagination {
        key: None,
        page,
        pages,
        on_change: None,
    }
}
impl<C> Pagination<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn on_change(mut self, f: impl FnMut(&mut C, usize) + 'static) -> Self {
        self.on_change = Some(Rc::new(RefCell::new(f)));
        self
    }
}
impl<C: 'static> From<Pagination<C>> for Element<C> {
    fn from(p: Pagination<C>) -> Self {
        let Some(key) = p.key.filter(|k| !k.is_empty()) else {
            return invalid("pagination requires a key");
        };
        let Some(change) = p.on_change else {
            return invalid("pagination requires on_change");
        };
        if p.pages == 0 || p.page == 0 || p.page > p.pages {
            return invalid("pagination requires 1 <= page <= pages");
        }
        let page = p.page;
        let go = |target: usize, label: String, id: String, enabled: bool, current: bool| {
            let f = change.clone();
            let mut b = button(label)
                .key(id)
                .height(32.)
                .font_size(13.)
                .variant(if current {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Ghost
                })
                .enabled(enabled)
                .on_click(move |app: &mut C| (f.borrow_mut())(app, target));
            if !current {
                b = b.min_width(32.);
            } else {
                b = b.width(32.);
            }
            let mut e: Element<C> = b.into();
            e.node.access_selected = current.then_some(true);
            e
        };
        let mut parts = vec![go(
            page.saturating_sub(1).max(1),
            "‹ Prev".into(),
            format!("{key}-prev"),
            page > 1,
            false,
        )];
        for (i, slot) in page_window(page, p.pages).into_iter().enumerate() {
            parts.push(match slot {
                Some(n) => go(n, n.to_string(), format!("{key}-{n}"), true, n == page),
                None => text("…")
                    .key(format!("{key}-gap-{i}"))
                    .color(ColorToken::Muted)
                    .width(24.)
                    .text_align(TextAlign::Center)
                    .into(),
            });
        }
        parts.push(go(
            page.saturating_add(1).min(p.pages),
            "Next ›".into(),
            format!("{key}-next"),
            page < p.pages,
            false,
        ));
        hstack(parts)
            .key(key)
            .gap(4., Axis::Horizontal)
            .align(Align::Center)
            .into()
    }
}

// ── Steps, timeline ─────────────────────────────────────────────────────

/// A horizontal progress indicator through numbered steps.
/// Steps before `current` show a check; `current` is highlighted.
pub fn steps<C>(labels: impl IntoIterator<Item = impl Into<String>>, current: usize) -> Element<C> {
    let labels: Vec<String> = labels.into_iter().map(Into::into).collect();
    let count = labels.len();
    let mut parts: Vec<Element<C>> = Vec::new();
    for (i, label) in labels.into_iter().enumerate() {
        let (done, active) = (i < current, i == current);
        let marker: Element<C> = if done {
            vstack![icon(Icon::Check)
                .size(14.)
                .color(ColorToken::PrimaryContent)]
            .width(28.)
            .height(28.)
            .flow(Flow::Center)
            .rounded(Rounding::Full)
            .background(ColorToken::Primary)
            .into()
        } else {
            vstack![text((i + 1).to_string())
                .font_size(13.)
                .font_weight(600)
                .color(if active {
                    ColorToken::Primary
                } else {
                    ColorToken::Muted
                })]
            .width(28.)
            .height(28.)
            .flow(Flow::Center)
            .rounded(Rounding::Full)
            .border_color(if active {
                ColorToken::Primary
            } else {
                ColorToken::Border
            })
            .border_width(if active { 2. } else { 1.5 })
            .into()
        };
        parts.push(
            hstack![
                marker,
                text(label)
                    .font_size(13.)
                    .font_weight(if active { 600 } else { 400 })
                    .color(if done || active {
                        ColorToken::BaseContent
                    } else {
                        ColorToken::Muted
                    })
                    .no_wrap(true),
            ]
            .gap(8., Axis::Horizontal)
            .align(Align::Center)
            .into(),
        );
        if i + 1 < count {
            parts.push(
                vstack![]
                    .height(2.)
                    .min_width(16.)
                    .flex(1.)
                    .rounded(1.)
                    .background(if done {
                        ColorToken::Primary.into()
                    } else {
                        String::from(ColorToken::Border)
                    })
                    .into(),
            );
        }
    }
    let mut row = hstack(parts)
        .gap(10., Axis::Horizontal)
        .align(Align::Center);
    row.0.node.accessible_label = Some(format!("Step {} of {count}", (current + 1).min(count)));
    row.into()
}
/// One event in a timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct TimelineItem {
    pub title: String,
    pub time: Option<String>,
    pub description: Option<String>,
    pub icon: Option<Icon>,
}
impl TimelineItem {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            time: None,
            description: None,
            icon: None,
        }
    }
    pub fn time(mut self, value: impl Into<String>) -> Self {
        self.time = Some(value.into());
        self
    }
    pub fn description(mut self, value: impl Into<String>) -> Self {
        self.description = Some(value.into());
        self
    }
    pub fn icon(mut self, glyph: Icon) -> Self {
        self.icon = Some(glyph);
        self
    }
}
/// A vertical list of events joined by a rail.
pub fn timeline<C>(items: impl IntoIterator<Item = TimelineItem>) -> Element<C> {
    let items: Vec<TimelineItem> = items.into_iter().collect();
    let count = items.len();
    let rows: Vec<Element<C>> = items
        .into_iter()
        .enumerate()
        .map(|(i, item)| {
            let marker: Element<C> = match item.icon {
                Some(g) => vstack![icon(g).size(14.).color(ColorToken::Primary)]
                    .width(28.)
                    .height(28.)
                    .flow(Flow::Center)
                    .rounded(Rounding::Full)
                    .background(ColorToken::Primary.alpha(14))
                    .into(),
                None => vstack![vstack![]
                    .width(10.)
                    .height(10.)
                    .rounded(Rounding::Full)
                    .background(ColorToken::Primary)]
                .width(28.)
                .height(28.)
                .flow(Flow::Center)
                .into(),
            };
            let mut rail = vec![marker];
            if i + 1 < count {
                rail.push(
                    vstack![]
                        .width(2.)
                        .flex(1.)
                        .min_height(12.)
                        .background(ColorToken::BaseContent.alpha(15))
                        .into(),
                );
            }
            let mut copy: Vec<Element<C>> = Vec::new();
            let mut head: Vec<Element<C>> = vec![text(item.title)
                .font_size(14.)
                .font_weight(600)
                .flex(1.)
                .into()];
            if let Some(t) = item.time {
                head.push(text(t).font_size(12.).color(ColorToken::Muted).into());
            }
            copy.push(hstack(head).gap(8., Axis::Horizontal).into());
            if let Some(d) = item.description {
                copy.push(text(d).font_size(13.).color(ColorToken::Muted).into());
            }
            hstack![
                vstack(rail)
                    .align(Align::Center)
                    .gap(4., Axis::Vertical)
                    .width(28.),
                vstack(copy)
                    .gap(4., Axis::Vertical)
                    .flex(1.)
                    .padding(4., Edge::Top)
                    .padding(18., Edge::Bottom),
            ]
            .gap(12., Axis::Horizontal)
            .align(Align::Stretch)
            .into()
        })
        .collect();
    vstack(rows).into()
}

// ── Empty state, stat ───────────────────────────────────────────────────

/// A centred icon, title, description and optional actions for empty views.
pub fn empty_state<C>(
    glyph: Icon,
    title: impl Into<String>,
    description: impl Into<String>,
    actions: impl IntoIterator<Item = Element<C>>,
) -> Element<C> {
    let actions: Vec<Element<C>> = actions.into_iter().collect();
    let mut parts: Vec<Element<C>> = vec![
        vstack![icon(glyph).size(26.).color(ColorToken::Muted)]
            .width(56.)
            .height(56.)
            .flow(Flow::Center)
            .rounded(Rounding::Full)
            .background(ColorToken::BaseContent.alpha(6))
            .into(),
        text(title).font_size(16.).font_weight(600).into(),
        text(description)
            .font_size(13.)
            .color(ColorToken::Muted)
            .text_align(TextAlign::Center)
            .max_width(360.)
            .into(),
    ];
    if !actions.is_empty() {
        parts.push(
            hstack(actions)
                .gap(8., Axis::Horizontal)
                .padding(6., Edge::Top)
                .into(),
        );
    }
    vstack(parts)
        .align(Align::Center)
        .gap(8., Axis::Vertical)
        .padding(32., Edge::All)
        .into()
}
/// A labelled metric with an optional change indicator.
pub struct Stat {
    label: String,
    value: String,
    delta: Option<(String, bool)>,
    hint: Option<String>,
}
pub fn stat(label: impl Into<String>, value: impl Into<String>) -> Stat {
    Stat {
        label: label.into(),
        value: value.into(),
        delta: None,
        hint: None,
    }
}
impl Stat {
    /// e.g. `.delta("+12.5%", true)`: green and up when positive.
    pub fn delta(mut self, value: impl Into<String>, positive: bool) -> Self {
        self.delta = Some((value.into(), positive));
        self
    }
    pub fn hint(mut self, value: impl Into<String>) -> Self {
        self.hint = Some(value.into());
        self
    }
}
impl<C> From<Stat> for Element<C> {
    fn from(s: Stat) -> Self {
        let mut foot: Vec<Element<C>> = Vec::new();
        if let Some((d, up)) = s.delta {
            let tint = if up {
                ColorToken::Success
            } else {
                ColorToken::Error
            };
            foot.push(
                icon(if up { Icon::ArrowUp } else { Icon::ArrowDown })
                    .size(13.)
                    .color(tint)
                    .into(),
            );
            foot.push(text(d).font_size(12.).font_weight(600).color(tint).into());
        }
        if let Some(h) = s.hint {
            foot.push(text(h).font_size(12.).color(ColorToken::Muted).into());
        }
        let mut parts: Vec<Element<C>> = vec![
            text(s.label).font_size(13.).color(ColorToken::Muted).into(),
            text(s.value)
                .font_size(28.)
                .font_weight(700)
                .letter_spacing(-0.02)
                .into(),
        ];
        if !foot.is_empty() {
            parts.push(
                hstack(foot)
                    .gap(4., Axis::Horizontal)
                    .align(Align::Center)
                    .into(),
            );
        }
        vstack(parts).gap(4., Axis::Vertical).into()
    }
}

// ── Charts ──────────────────────────────────────────────────────────────

/// Value range with a little headroom; flat data gets a unit range.
fn bounds(values: &[f32], zero: bool) -> (f32, f32) {
    let finite = values.iter().copied().filter(|v| v.is_finite());
    let (mut lo, mut hi) = finite.fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(v), h.max(v)));
    if lo > hi {
        return (0., 1.);
    }
    if zero {
        lo = lo.min(0.);
        hi = hi.max(0.);
    }
    if (hi - lo).abs() < f32::EPSILON {
        hi = lo + 1.;
    }
    (lo, hi)
}
/// Points of a series scaled into a `w`×`h` box (y grows downwards).
pub(super) fn chart_points(values: &[f32], w: f32, h: f32, range: (f32, f32)) -> Vec<[f32; 2]> {
    let n = values.len();
    let dx = if n > 1 { w / (n - 1) as f32 } else { 0. };
    values
        .iter()
        .enumerate()
        .filter(|(_, v)| v.is_finite())
        .map(|(i, v)| {
            let t = (v - range.0) / (range.1 - range.0);
            [i as f32 * dx, h - t * h]
        })
        .collect()
}
fn polyline(points: &[[f32; 2]]) -> VectorPath {
    let mut p = VectorPath::new();
    for (i, [x, y]) in points.iter().enumerate() {
        p = if i == 0 {
            p.move_to(*x, *y)
        } else {
            p.line_to(*x, *y)
        };
    }
    p
}
/// A tiny trend line with an optional soft fill.
pub fn sparkline<C>(values: &[f32], width: f32, height: f32) -> Element<C> {
    let range = bounds(values, false);
    let pad = 2.;
    let points: Vec<[f32; 2]> = chart_points(values, width - 2. * pad, height - 2. * pad, range)
        .into_iter()
        .map(|[x, y]| [x + pad, y + pad])
        .collect();
    canvas(move |c| {
        if points.len() >= 2 {
            let mut area = polyline(&points);
            let (first, last) = (points[0], points[points.len() - 1]);
            area = area
                .line_to(last[0], height)
                .line_to(first[0], height)
                .close();
            c.fill(area, ColorToken::Primary.alpha(14));
            c.stroke(polyline(&points), ColorToken::Primary, 1.75);
        }
    })
    .width(width)
    .height(height)
}
/// A labelled bar chart with a baseline and value gridlines.
pub fn bar_chart<C>(
    bars: impl IntoIterator<Item = (impl Into<String>, f32)>,
    width: f32,
    height: f32,
) -> Element<C> {
    let bars: Vec<(String, f32)> = bars.into_iter().map(|(l, v)| (l.into(), v)).collect();
    let values: Vec<f32> = bars.iter().map(|(_, v)| *v).collect();
    let range = bounds(&values, true);
    let plot = height - 22.;
    let n = bars.len().max(1) as f32;
    let slot = width / n;
    let bar = (slot * 0.62).min(48.);
    let to_y = move |v: f32| plot - (v - range.0) / (range.1 - range.0) * plot;
    let chart = canvas(move |c| {
        for i in 1..4 {
            let y = plot * i as f32 / 4.;
            c.stroke(
                VectorPath::new().move_to(0., y).line_to(width, y),
                ColorToken::Border,
                1.,
            );
        }
        let base = to_y(0.);
        for (i, v) in values.iter().enumerate() {
            if !v.is_finite() {
                continue;
            }
            let x = i as f32 * slot + (slot - bar) * 0.5;
            let (top, bottom) = (to_y(*v).min(base), to_y(*v).max(base));
            c.fill(
                VectorPath::rectangle(x, top, bar, (bottom - top).max(1.)),
                ColorToken::Primary,
            );
        }
        c.stroke(
            VectorPath::new().move_to(0., base).line_to(width, base),
            ColorToken::Muted,
            1.,
        );
    })
    .width(width)
    .height(plot);
    let labels: Vec<Element<C>> = bars
        .into_iter()
        .map(|(l, _)| {
            text(l)
                .font_size(11.)
                .color(ColorToken::Muted)
                .width(slot)
                .text_align(TextAlign::Center)
                .max_lines(1)
                .into()
        })
        .collect();
    vstack![chart, hstack(labels).height(16.)]
        .gap(6., Axis::Vertical)
        .width(width)
        .into()
}
/// One or more series over shared x positions, each in its own colour.
pub fn line_chart<C>(
    series: impl IntoIterator<Item = (impl Into<String>, Vec<f32>)>,
    width: f32,
    height: f32,
) -> Element<C> {
    // Primary first, then fixed hues that stay distinct in every theme.
    const COLOURS: [&str; 4] = ["@primary", "#0EA5E9", "#F59E0B", "#10B981"];
    let series: Vec<(String, Vec<f32>)> = series.into_iter().map(|(n, v)| (n.into(), v)).collect();
    let all: Vec<f32> = series.iter().flat_map(|(_, v)| v.iter().copied()).collect();
    let range = bounds(&all, false);
    let plot = height - 24.;
    let lines: Vec<(Vec<[f32; 2]>, &str)> = series
        .iter()
        .enumerate()
        .map(|(i, (_, v))| (chart_points(v, width, plot - 4., range), COLOURS[i % 4]))
        .collect();
    let chart = canvas(move |c| {
        for i in 0..=4 {
            let y = 2. + (plot - 4.) * i as f32 / 4.;
            c.stroke(
                VectorPath::new().move_to(0., y).line_to(width, y),
                ColorToken::Border,
                1.,
            );
        }
        for (points, colour) in &lines {
            let shifted: Vec<[f32; 2]> = points.iter().map(|[x, y]| [*x, y + 2.]).collect();
            if shifted.len() >= 2 {
                c.stroke(polyline(&shifted), *colour, 2.);
            }
            for [x, y] in &shifted {
                c.fill(VectorPath::circle(*x, *y, 2.5), *colour);
            }
        }
    })
    .width(width)
    .height(plot);
    let legend: Vec<Element<C>> = series
        .iter()
        .enumerate()
        .map(|(i, (name, _))| {
            hstack![
                vstack![]
                    .width(10.)
                    .height(10.)
                    .rounded(3.)
                    .background(COLOURS[i % 4]),
                text(name.clone()).font_size(12.).color(ColorToken::Muted),
            ]
            .gap(6., Axis::Horizontal)
            .align(Align::Center)
            .into()
        })
        .collect();
    vstack![chart, hstack(legend).gap(14., Axis::Horizontal).height(16.)]
        .gap(8., Axis::Vertical)
        .width(width)
        .into()
}

// ── Carousel ────────────────────────────────────────────────────────────

/// One slide at a time with previous/next buttons and position dots.
pub struct Carousel<C> {
    key: Option<String>,
    slides: Vec<Element<C>>,
    index: usize,
    on_change: Option<Page<C>>,
    height: f32,
    wrap: bool,
}
pub fn carousel<C>(slides: impl IntoIterator<Item = Element<C>>, index: usize) -> Carousel<C> {
    Carousel {
        key: None,
        slides: slides.into_iter().collect(),
        index,
        on_change: None,
        height: 240.,
        wrap: true,
    }
}
impl<C> Carousel<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn height(mut self, value: f32) -> Self {
        self.height = value;
        self
    }
    /// Loop from the last slide to the first (default true).
    pub fn wrap_around(mut self, value: bool) -> Self {
        self.wrap = value;
        self
    }
    /// Called with the requested slide index.
    pub fn on_change(mut self, f: impl FnMut(&mut C, usize) + 'static) -> Self {
        self.on_change = Some(Rc::new(RefCell::new(f)));
        self
    }
}
impl<C: 'static> From<Carousel<C>> for Element<C> {
    fn from(c: Carousel<C>) -> Self {
        let Some(key) = c.key.filter(|k| !k.is_empty()) else {
            return invalid("carousel requires a key");
        };
        let Some(change) = c.on_change else {
            return invalid("carousel requires on_change");
        };
        let n = c.slides.len();
        if n == 0 {
            return invalid("carousel requires at least one slide");
        }
        let i = c.index.min(n - 1);
        let (prev, next) = if c.wrap {
            ((i + n - 1) % n, (i + 1) % n)
        } else {
            (i.saturating_sub(1), (i + 1).min(n - 1))
        };
        let mut slide = c.slides.into_iter().nth(i).unwrap();
        if slide.node.flex == 0. {
            slide.node.flex = 1.;
        }
        let arrow = |id: String, label: &str, glyph: Icon, target: usize, enabled: bool| {
            let f = change.clone();
            zstack![
                button("")
                    .key(id.clone())
                    .accessible_label(label)
                    .width(34.)
                    .height(34.)
                    .rounded(Rounding::Full)
                    .background(ColorToken::Base100.alpha(85))
                    .border_color(ColorToken::Border)
                    .border_width(1.)
                    .enabled(enabled)
                    .on_click(move |app: &mut C| (f.borrow_mut())(app, target)),
                icon(glyph).size(16.),
            ]
            .key(format!("{id}-layer"))
            .width(34.)
            .height(34.)
            .flow(Flow::Center)
        };
        let dots: Vec<Element<C>> = (0..n)
            .map(|d| {
                let f = change.clone();
                button("")
                    .key(format!("{key}-dot-{d}"))
                    .accessible_label(format!("Slide {} of {n}", d + 1))
                    .width(if d == i { 18. } else { 8. })
                    .height(8.)
                    .rounded(Rounding::Full)
                    .border_width(0.)
                    .background(if d == i {
                        ColorToken::Primary.into()
                    } else {
                        ColorToken::BaseContent.alpha(25)
                    })
                    .on_click(move |app: &mut C| (f.borrow_mut())(app, d))
                    .into()
            })
            .collect();
        vstack![
            zstack![
                vstack![slide]
                    .key(format!("{key}-slide-{i}"))
                    .align(Align::Stretch)
                    .transition(Transition::fade().duration_ms(180)),
                hstack![
                    arrow(
                        format!("{key}-prev"),
                        "Previous slide",
                        Icon::ChevronLeft,
                        prev,
                        c.wrap || i > 0
                    ),
                    spacer(),
                    arrow(
                        format!("{key}-next"),
                        "Next slide",
                        Icon::ChevronRight,
                        next,
                        c.wrap || i + 1 < n
                    ),
                ]
                .key(format!("{key}-arrows"))
                .align(Align::Center)
                .padding(10., Edge::Horizontal)
                .scroll_passthrough(true),
            ]
            .key(format!("{key}-stage"))
            .align(Align::Stretch)
            .height(c.height)
            .rounded(12.)
            .clip_contents(true),
            hstack(dots)
                .key(format!("{key}-dots"))
                .gap(6., Axis::Horizontal)
                .align(Align::Center),
        ]
        .key(key)
        .gap(10., Axis::Vertical)
        .align(Align::Center)
        .into()
    }
}

// ── Image with fallback ─────────────────────────────────────────────────

/// A sized image (cover-fitted, rounded) or a neutral placeholder with an
/// icon while it is missing or loading.
pub fn image_view<C>(image: Option<&Image>, width: f32, height: f32) -> Container<C> {
    let frame = |children: Vec<Element<C>>| {
        vstack(children)
            .width(width)
            .height(height)
            .rounded(10.)
            .clip(Clip::Rounded)
    };
    match image {
        Some(i) => frame(Vec::new()).background_image(i.clone().background().fit(ImageFit::Cover)),
        None => frame(vec![icon(Icon::Image)
            .size((width.min(height) * 0.25).clamp(16., 40.))
            .color(ColorToken::Muted)
            .into()])
        .flow(Flow::Center)
        .background(ColorToken::BaseContent.alpha(6))
        .border_color(ColorToken::Border)
        .border_width(1.),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn initials_from_names() {
        assert_eq!(initials("Ada Lovelace"), "AL");
        assert_eq!(initials("grace"), "G");
        assert_eq!(initials("Jean Paul Sartre"), "JS");
        assert_eq!(initials("  "), "?");
    }
    #[test]
    fn page_window_keeps_ends_and_neighbours() {
        assert_eq!(page_window(1, 5), [1, 2, 3, 4, 5].map(Some));
        assert_eq!(
            page_window(1, 20),
            [Some(1), Some(2), Some(3), Some(4), Some(5), None, Some(20)]
        );
        assert_eq!(
            page_window(10, 20),
            [Some(1), None, Some(9), Some(10), Some(11), None, Some(20)]
        );
        assert_eq!(
            page_window(20, 20),
            [
                Some(1),
                None,
                Some(16),
                Some(17),
                Some(18),
                Some(19),
                Some(20)
            ]
        );
    }
    #[test]
    fn chart_points_scale_into_the_box() {
        let p = chart_points(&[0., 5., 10.], 100., 50., (0., 10.));
        assert_eq!(p, [[0., 50.], [50., 25.], [100., 0.]]);
        assert_eq!(bounds(&[3., 3.], false), (3., 4.));
        assert_eq!(bounds(&[2., 5.], true), (0., 5.));
        assert_eq!(bounds(&[], false), (0., 1.));
    }
}
