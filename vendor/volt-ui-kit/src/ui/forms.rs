//! Form inputs composed from the core controls: radio groups, number
//! steppers, comboboxes, tag inputs, colour pickers, one-time-code fields,
//! star ratings and labelled form fields. All are controlled: values live in
//! app state; stateful pieces (popup open, typed query) use a small `*State`
//! plus a `.state(..)` accessor, like the date pickers.
use super::*;
use std::{cell::RefCell, rc::Rc};

type Access<C, S> = Rc<dyn Fn(&mut C) -> &mut S>;
fn invalid<C>(message: &str) -> Element<C> {
    let mut e = Element::new(Kind::Column);
    e.validation_error = Some(message.into());
    e
}
/// A ghost button with a glyph on top: the building block for steppers etc.
fn glyph_button<C: 'static>(
    key: String,
    label: &str,
    glyph: Icon,
    size: f32,
    enabled: bool,
    action: impl FnMut(&mut C) + 'static,
) -> Element<C> {
    zstack![
        button("")
            .key(key.clone())
            .accessible_label(label)
            .variant(ButtonVariant::Ghost)
            .width(size)
            .height(size)
            .enabled(enabled)
            .on_click(action),
        icon(glyph).size((size * 0.5).round()).color(if enabled {
            ColorToken::BaseContent
        } else {
            ColorToken::Muted
        }),
    ]
    .key(format!("{key}-layer"))
    .width(size)
    .height(size)
    .flow(Flow::Center)
    .into()
}

// ── Radio group ─────────────────────────────────────────────────────────

type Select<C, T> = Rc<RefCell<dyn FnMut(&mut C, T)>>;
/// Mutually exclusive options shown as radio buttons with labels.
pub struct RadioGroup<C, T> {
    key: Option<String>,
    items: Vec<(T, String, Option<String>)>,
    selected: Option<T>,
    horizontal: bool,
    on_select: Option<Select<C, T>>,
}
pub fn radio_group<C, T>(
    items: impl IntoIterator<Item = (T, impl Into<String>)>,
) -> RadioGroup<C, T> {
    RadioGroup {
        key: None,
        items: items
            .into_iter()
            .map(|(v, l)| (v, l.into(), None))
            .collect(),
        selected: None,
        horizontal: false,
        on_select: None,
    }
}
impl<C, T> RadioGroup<C, T> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn selected(mut self, value: T) -> Self {
        self.selected = Some(value);
        self
    }
    /// Lay options out in a row instead of a column.
    pub fn horizontal(mut self, value: bool) -> Self {
        self.horizontal = value;
        self
    }
    /// A muted description under each option, in item order.
    pub fn descriptions(mut self, texts: impl IntoIterator<Item = impl Into<String>>) -> Self {
        for (item, text) in self.items.iter_mut().zip(texts) {
            item.2 = Some(text.into());
        }
        self
    }
    pub fn on_select(mut self, f: impl FnMut(&mut C, T) + 'static) -> Self {
        self.on_select = Some(Rc::new(RefCell::new(f)));
        self
    }
}
impl<C: 'static, T: Clone + PartialEq + 'static> From<RadioGroup<C, T>> for Element<C> {
    fn from(g: RadioGroup<C, T>) -> Self {
        let Some(key) = g.key.filter(|k| !k.is_empty()) else {
            return invalid("radio_group requires a key");
        };
        let Some(select) = g.on_select else {
            return invalid("radio_group requires on_select");
        };
        if g.items.is_empty() {
            return invalid("radio_group requires at least one option");
        }
        let count = g.items.len();
        let rows: Vec<Element<C>> = g
            .items
            .into_iter()
            .enumerate()
            .map(|(i, (value, label, description))| {
                let on = g.selected.as_ref() == Some(&value);
                let handler = select.clone();
                let mut hit: Element<C> = button("")
                    .key(format!("{key}-{i}"))
                    .accessible_label(label.clone())
                    .variant(ButtonVariant::Ghost)
                    .on_click(move |app: &mut C| (handler.borrow_mut())(app, value.clone()))
                    .into();
                hit.node.access_role = Some(AccessRole::RadioButton);
                hit.node.access_selected = Some(on);
                hit.node.access_position = Some(i + 1);
                hit.node.access_set_size = Some(count);
                let dot = canvas(move |c| {
                    c.stroke(
                        VectorPath::circle(9., 9., 7.5),
                        if on {
                            ColorToken::Primary.into()
                        } else {
                            String::from(ColorToken::Border)
                        },
                        1.5,
                    );
                    if on {
                        c.fill(VectorPath::circle(9., 9., 4.), ColorToken::Primary);
                    }
                })
                .width(18.)
                .height(18.);
                let mut copy: Vec<Element<C>> = vec![text(label).font_size(14.).into()];
                if let Some(d) = description {
                    copy.push(text(d).font_size(12.).color(ColorToken::Muted).into());
                }
                zstack![
                    hit,
                    hstack![dot, vstack(copy).gap(2., Axis::Vertical)]
                        .gap(10., Axis::Horizontal)
                        .align(Align::Center)
                        .padding(8., Edge::Horizontal)
                        .padding(6., Edge::Vertical)
                        .scroll_passthrough(true),
                ]
                .key(format!("{key}-{i}-row"))
                .align(Align::Stretch)
                .into()
            })
            .collect();
        let mut group = if g.horizontal {
            hstack(rows)
        } else {
            vstack(rows)
        }
        .key(key)
        .gap(4., Axis::Both);
        group.0.node.access_role = Some(AccessRole::RadioGroup);
        group.into()
    }
}

// ── Number input ────────────────────────────────────────────────────────

type Number<C> = Rc<RefCell<dyn FnMut(&mut C, f64)>>;
/// A numeric field with − and + steppers, clamped to a range.
pub struct NumberInput<C> {
    key: Option<String>,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    decimals: usize,
    suffix: String,
    on_change: Option<Number<C>>,
    width: f32,
}
pub fn number_input<C>(value: f64) -> NumberInput<C> {
    NumberInput {
        key: None,
        value,
        min: f64::MIN,
        max: f64::MAX,
        step: 1.,
        decimals: 0,
        suffix: String::new(),
        on_change: None,
        width: 140.,
    }
}
impl<C> NumberInput<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.min = min;
        self.max = max;
        self
    }
    pub fn step(mut self, step: f64) -> Self {
        self.step = step;
        self
    }
    /// Digits after the decimal point shown and kept (default 0).
    pub fn decimals(mut self, digits: usize) -> Self {
        self.decimals = digits.min(6);
        self
    }
    /// A unit shown after the number, e.g. " kg".
    pub fn suffix(mut self, value: impl Into<String>) -> Self {
        self.suffix = value.into();
        self
    }
    pub fn width(mut self, value: f32) -> Self {
        self.width = value;
        self
    }
    pub fn on_change(mut self, f: impl FnMut(&mut C, f64) + 'static) -> Self {
        self.on_change = Some(Rc::new(RefCell::new(f)));
        self
    }
}
impl<C: 'static> From<NumberInput<C>> for Element<C> {
    fn from(n: NumberInput<C>) -> Self {
        let Some(key) = n.key.filter(|k| !k.is_empty()) else {
            return invalid("number_input requires a key");
        };
        let Some(change) = n.on_change else {
            return invalid("number_input requires on_change");
        };
        if !(n.value.is_finite()
            && n.step.is_finite()
            && n.step > 0.
            && n.min.is_finite()
            && n.max.is_finite()
            && n.min <= n.max
            && (n.min..=n.max).contains(&n.value))
        {
            return invalid("number_input needs finite values, a positive step and min <= max");
        }
        let (min, max, decimals) = (n.min, n.max, n.decimals);
        let round = move |v: f64| {
            let v = v.clamp(min, max);
            let f = 10f64.powi(decimals as i32);
            let scaled = v * f;
            // Decimal precision is smaller than an ULP at huge magnitudes.
            // Do not turn a finite value into infinity while rounding it.
            if scaled.is_finite() {
                (scaled.round() / f).clamp(min, max)
            } else {
                v
            }
        };
        let v = n.value;
        let (dec, inc, typed) = (change.clone(), change.clone(), change);
        let step = n.step;
        let shown = format!("{v:.decimals$}{}", n.suffix);
        let suffix = n.suffix.clone();
        hstack![
            glyph_button(
                format!("{key}-dec"),
                "Decrease",
                Icon::Minus,
                32.,
                v > min,
                move |app: &mut C| { (dec.borrow_mut())(app, round(v - step)) }
            ),
            text_field(shown)
                .key(format!("{key}-field"))
                .accessible_label("Value")
                .height(32.)
                .font_size(14.)
                .text_align(TextAlign::Center)
                .flex(1.)
                .on_change(move |app: &mut C, text| {
                    let digits = text
                        .trim()
                        .trim_end_matches(suffix.trim())
                        .trim()
                        .to_owned();
                    if let Ok(parsed) = digits.parse::<f64>() {
                        if parsed.is_finite() {
                            (typed.borrow_mut())(app, round(parsed));
                        }
                    }
                }),
            glyph_button(
                format!("{key}-inc"),
                "Increase",
                Icon::Plus,
                32.,
                v < max,
                move |app: &mut C| { (inc.borrow_mut())(app, round(v + step)) }
            ),
        ]
        .key(key)
        .width(n.width)
        .gap(4., Axis::Horizontal)
        .align(Align::Center)
        .into()
    }
}

// ── Combobox ────────────────────────────────────────────────────────────

/// Popup open state and the typed filter, kept in app state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComboboxState {
    pub open: bool,
    pub query: String,
}
/// A searchable select: type to filter, click or press Enter to choose.
pub struct Combobox<C, T> {
    key: Option<String>,
    items: Vec<(T, String)>,
    selected: Option<T>,
    snapshot: ComboboxState,
    state: Option<Access<C, ComboboxState>>,
    on_select: Option<Select<C, T>>,
    placeholder: String,
    width: f32,
}
pub fn combobox<C, T>(
    items: impl IntoIterator<Item = (T, impl Into<String>)>,
    state: &ComboboxState,
) -> Combobox<C, T> {
    Combobox {
        key: None,
        items: items.into_iter().map(|(v, l)| (v, l.into())).collect(),
        selected: None,
        snapshot: state.clone(),
        state: None,
        on_select: None,
        placeholder: "Search…".into(),
        width: 240.,
    }
}
impl<C, T> Combobox<C, T> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn selected(mut self, value: Option<T>) -> Self {
        self.selected = value;
        self
    }
    pub fn state(mut self, access: impl Fn(&mut C) -> &mut ComboboxState + 'static) -> Self {
        self.state = Some(Rc::new(access));
        self
    }
    pub fn on_select(mut self, f: impl FnMut(&mut C, T) + 'static) -> Self {
        self.on_select = Some(Rc::new(RefCell::new(f)));
        self
    }
    pub fn placeholder(mut self, value: impl Into<String>) -> Self {
        self.placeholder = value.into();
        self
    }
    pub fn width(mut self, value: f32) -> Self {
        self.width = value;
        self
    }
}
/// Case-insensitive "contains" match, best (prefix) matches first.
pub(super) fn filter_items<'a, T>(items: &'a [(T, String)], query: &str) -> Vec<(usize, &'a str)> {
    let q = query.trim().to_lowercase();
    let mut hits: Vec<(usize, &str, bool)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, (_, label))| {
            let l = label.to_lowercase();
            l.contains(&q)
                .then_some((i, label.as_str(), l.starts_with(&q)))
        })
        .collect();
    hits.sort_by_key(|(_, _, prefix)| !*prefix);
    hits.into_iter().map(|(i, l, _)| (i, l)).collect()
}
/// What was typed into `before` to make `after` (the edit between their
/// common prefix and suffix).
fn inserted(before: &str, after: &str) -> String {
    let prefix: usize = before
        .chars()
        .zip(after.chars())
        .take_while(|(a, b)| a == b)
        .map(|(c, _)| c.len_utf8())
        .sum();
    let (b, a) = (&before[prefix..], &after[prefix..]);
    let suffix: usize = b
        .chars()
        .rev()
        .zip(a.chars().rev())
        .take_while(|(x, y)| x == y)
        .map(|(c, _)| c.len_utf8())
        .sum();
    a[..a.len() - suffix.min(a.len())].to_owned()
}
impl<C: 'static, T: Clone + PartialEq + 'static> From<Combobox<C, T>> for Element<C> {
    fn from(c: Combobox<C, T>) -> Self {
        let Some(key) = c.key.filter(|k| !k.is_empty()) else {
            return invalid("combobox requires a key");
        };
        let (Some(state), Some(select)) = (c.state, c.on_select) else {
            return invalid("combobox requires state(..) and on_select(..)");
        };
        let label = c
            .selected
            .as_ref()
            .and_then(|v| c.items.iter().find(|(i, _)| i == v))
            .map(|(_, l)| l.clone());
        // While open, the field shows the query; closed, the chosen label.
        let shown = if c.snapshot.open {
            c.snapshot.query.clone()
        } else {
            label.clone().unwrap_or_default()
        };
        let matches: Vec<(usize, String)> = filter_items(&c.items, &c.snapshot.query)
            .into_iter()
            .take(8)
            .map(|(i, l)| (i, l.to_owned()))
            .collect();
        let first = matches.first().map(|(i, _)| c.items[*i].0.clone());
        let (typing, toggle, submit_state, submit_select) =
            (state.clone(), state.clone(), state.clone(), select.clone());
        let open = c.snapshot.open;
        let chosen_label = label.clone();
        let field = text_field(shown)
            .key(format!("{key}-field"))
            .accessible_label(label.unwrap_or_else(|| c.placeholder.clone()))
            .placeholder(c.placeholder.clone())
            .width(c.width)
            .height(36.)
            .trailing_action(
                format!("{key}-toggle"),
                if open {
                    Icon::ChevronUp
                } else {
                    Icon::ChevronDown
                },
                "Show options",
                move |app: &mut C| {
                    let s = toggle(app);
                    s.open = !s.open;
                    s.query.clear();
                },
            )
            .on_change(move |app: &mut C, text: String| {
                let s = typing(app);
                // Typing into a closed box starts a fresh query rather than
                // editing the chosen label.
                s.query = match (&chosen_label, s.open) {
                    (Some(l), false) => inserted(l, &text),
                    _ => text,
                };
                s.open = true;
            })
            .on_submit(move |app: &mut C| {
                if let Some(v) = first.clone() {
                    (submit_select.borrow_mut())(app, v);
                }
                let s = submit_state(app);
                s.open = false;
                s.query.clear();
            });
        let mut options: Vec<Element<C>> = matches
            .into_iter()
            .map(|(i, l)| {
                let (value, pick, close) = (c.items[i].0.clone(), select.clone(), state.clone());
                let chosen = c.selected.as_ref() == Some(&value);
                hstack![
                    button(l)
                        .key(format!("{key}-option-{i}"))
                        .variant(ButtonVariant::Ghost)
                        .label_align(Align::Start)
                        .height(32.)
                        .flex(1.)
                        .on_click(move |app: &mut C| {
                            (pick.borrow_mut())(app, value.clone());
                            let s = close(app);
                            s.open = false;
                            s.query.clear();
                        }),
                    if chosen {
                        Element::from(icon(Icon::Check).size(14.).color(ColorToken::Primary))
                    } else {
                        spacer().width(14.).flex(0.).into()
                    },
                ]
                .key(format!("{key}-option-{i}-row"))
                .align(Align::Center)
                .gap(6., Axis::Horizontal)
                .into()
            })
            .collect();
        if options.is_empty() {
            options.push(
                text("No results")
                    .key(format!("{key}-empty"))
                    .font_size(13.)
                    .color(ColorToken::Muted)
                    .into(),
            );
        }
        let close = state;
        let pop = popover(vstack(options).gap(2., Axis::Vertical))
            .key(format!("{key}-popover"))
            .anchor(format!("{key}-field"))
            .keep_anchor_interactive(true)
            .open(open)
            .width(c.width)
            .on_dismiss(move |app: &mut C| {
                let s = close(app);
                s.open = false;
                s.query.clear();
            });
        vstack![field, pop].key(key).into()
    }
}

// ── Tag input ───────────────────────────────────────────────────────────

/// The draft being typed into a tag input, kept in app state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TagInputState {
    pub draft: String,
}
type Tags<C> = Rc<RefCell<dyn FnMut(&mut C, Vec<String>)>>;
/// Chips plus a field: Enter adds the draft as a tag, × removes one.
pub struct TagInput<C> {
    key: Option<String>,
    tags: Vec<String>,
    snapshot: TagInputState,
    state: Option<Access<C, TagInputState>>,
    on_change: Option<Tags<C>>,
    placeholder: String,
    max: usize,
}
pub fn tag_input<C>(tags: &[String], state: &TagInputState) -> TagInput<C> {
    TagInput {
        key: None,
        tags: tags.to_vec(),
        snapshot: state.clone(),
        state: None,
        on_change: None,
        placeholder: "Add a tag…".into(),
        max: 32,
    }
}
impl<C> TagInput<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn state(mut self, access: impl Fn(&mut C) -> &mut TagInputState + 'static) -> Self {
        self.state = Some(Rc::new(access));
        self
    }
    pub fn on_change(mut self, f: impl FnMut(&mut C, Vec<String>) + 'static) -> Self {
        self.on_change = Some(Rc::new(RefCell::new(f)));
        self
    }
    pub fn placeholder(mut self, value: impl Into<String>) -> Self {
        self.placeholder = value.into();
        self
    }
    /// Most tags allowed (default 32).
    pub fn max_tags(mut self, value: usize) -> Self {
        self.max = value;
        self
    }
}
impl<C: 'static> From<TagInput<C>> for Element<C> {
    fn from(t: TagInput<C>) -> Self {
        let Some(key) = t.key.filter(|k| !k.is_empty()) else {
            return invalid("tag_input requires a key");
        };
        let (Some(state), Some(change)) = (t.state, t.on_change) else {
            return invalid("tag_input requires state(..) and on_change(..)");
        };
        let mut row: Vec<Element<C>> = t
            .tags
            .iter()
            .enumerate()
            .map(|(i, tag)| {
                let (tags, remove) = (t.tags.clone(), change.clone());
                hstack![
                    text(tag.clone()).font_size(12.).font_weight(500),
                    glyph_button(
                        format!("{key}-remove-{i}"),
                        &format!("Remove {tag}"),
                        Icon::Close,
                        18.,
                        true,
                        move |app: &mut C| {
                            let mut next = tags.clone();
                            next.remove(i);
                            (remove.borrow_mut())(app, next);
                        },
                    ),
                ]
                .key(format!("{key}-tag-{i}"))
                .gap(2., Axis::Horizontal)
                .align(Align::Center)
                .padding(8., Edge::Left)
                .padding(2., Edge::Right)
                .height(26.)
                .rounded(Rounding::Full)
                .background(ColorToken::BaseContent.alpha(8))
                .into()
            })
            .collect();
        let (typing, submit, tags, max) = (state.clone(), state, t.tags.clone(), t.max);
        row.push(
            text_field(t.snapshot.draft.clone())
                .key(format!("{key}-field"))
                .accessible_label("New tag")
                .placeholder(t.placeholder)
                .width(160.)
                .height(28.)
                .font_size(13.)
                .background("#00000000")
                .border_width(0.)
                .on_change(move |app: &mut C, text| typing(app).draft = text)
                .on_submit(move |app: &mut C| {
                    let draft = submit(app).draft.trim().to_owned();
                    if !draft.is_empty() && !tags.contains(&draft) && tags.len() < max {
                        let mut next = tags.clone();
                        next.push(draft);
                        (change.borrow_mut())(app, next);
                    }
                    submit(app).draft.clear();
                })
                .into(),
        );
        hstack(row)
            .key(key)
            .wrap(true)
            .gap(6., Axis::Both)
            .align(Align::Center)
            .padding(6., Edge::All)
            .rounded(8.)
            .border_color(ColorToken::Border)
            .border_width(1.)
            .into()
    }
}

// ── Colour picker ───────────────────────────────────────────────────────

/// A colour as `#RRGGBB`, with hue/saturation/lightness helpers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsl {
    pub hue: f32,
    pub saturation: f32,
    pub lightness: f32,
}
impl Hsl {
    pub fn to_rgb(self) -> [f32; 3] {
        let (h, s, l) = (
            self.hue.rem_euclid(360.) / 360.,
            self.saturation,
            self.lightness,
        );
        let q = if l < 0.5 { l * (1. + s) } else { l + s - l * s };
        let p = 2. * l - q;
        let channel = |mut t: f32| {
            t = t.rem_euclid(1.);
            if t < 1. / 6. {
                p + (q - p) * 6. * t
            } else if t < 0.5 {
                q
            } else if t < 2. / 3. {
                p + (q - p) * (2. / 3. - t) * 6.
            } else {
                p
            }
        };
        [channel(h + 1. / 3.), channel(h), channel(h - 1. / 3.)]
    }
    pub fn from_rgb([r, g, b]: [f32; 3]) -> Self {
        let (max, min) = (r.max(g).max(b), r.min(g).min(b));
        let l = (max + min) * 0.5;
        if max == min {
            return Self {
                hue: 0.,
                saturation: 0.,
                lightness: l,
            };
        }
        let d = max - min;
        let s = if l > 0.5 {
            d / (2. - max - min)
        } else {
            d / (max + min)
        };
        let h = if max == r {
            (g - b) / d + if g < b { 6. } else { 0. }
        } else if max == g {
            (b - r) / d + 2.
        } else {
            (r - g) / d + 4.
        };
        Self {
            hue: h * 60.,
            saturation: s,
            lightness: l,
        }
    }
    pub fn hex(self) -> String {
        let [r, g, b] = self
            .to_rgb()
            .map(|v| (v.clamp(0., 1.) * 255.).round() as u8);
        format!("#{r:02X}{g:02X}{b:02X}")
    }
    pub fn parse(hex: &str) -> Option<Self> {
        let h = hex.trim().trim_start_matches('#');
        if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let c = |i: usize| {
            u8::from_str_radix(&h[i..i + 2], 16)
                .ok()
                .map(|v| f32::from(v) / 255.)
        };
        Some(Self::from_rgb([c(0)?, c(2)?, c(4)?]))
    }
}
type Colour<C> = Rc<RefCell<dyn FnMut(&mut C, String)>>;
/// Preset swatches, hue/saturation/lightness sliders and a hex field.
pub struct ColorPicker<C> {
    key: Option<String>,
    value: String,
    swatches: Vec<String>,
    on_change: Option<Colour<C>>,
}
pub fn color_picker<C>(hex: impl Into<String>) -> ColorPicker<C> {
    ColorPicker {
        key: None,
        value: hex.into(),
        swatches: [
            "#EF4444", "#F97316", "#F59E0B", "#EAB308", "#84CC16", "#22C55E", "#10B981", "#14B8A6",
            "#06B6D4", "#0EA5E9", "#3B82F6", "#6366F1", "#8B5CF6", "#A855F7", "#D946EF", "#EC4899",
            "#F43F5E", "#78716C", "#64748B", "#18181B",
        ]
        .map(String::from)
        .into(),
        on_change: None,
    }
}
impl<C> ColorPicker<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    /// Replace the preset swatches (`#RRGGBB`).
    pub fn swatches(mut self, colours: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.swatches = colours.into_iter().map(Into::into).collect();
        self
    }
    /// Called with `#RRGGBB`.
    pub fn on_change(mut self, f: impl FnMut(&mut C, String) + 'static) -> Self {
        self.on_change = Some(Rc::new(RefCell::new(f)));
        self
    }
}
impl<C: 'static> From<ColorPicker<C>> for Element<C> {
    fn from(p: ColorPicker<C>) -> Self {
        let Some(key) = p.key.filter(|k| !k.is_empty()) else {
            return invalid("color_picker requires a key");
        };
        let Some(change) = p.on_change else {
            return invalid("color_picker requires on_change");
        };
        let hsl = Hsl::parse(&p.value).unwrap_or(Hsl {
            hue: 220.,
            saturation: 0.8,
            lightness: 0.55,
        });
        let current = hsl.hex();
        let swatches: Vec<Element<C>> = p
            .swatches
            .iter()
            .filter(|c| Hsl::parse(c).is_some())
            .enumerate()
            .map(|(i, colour)| {
                let (pick, colour) = (change.clone(), colour.to_uppercase());
                let chosen = colour == current;
                button("")
                    .key(format!("{key}-swatch-{i}"))
                    .accessible_label(colour.clone())
                    .width(24.)
                    .height(24.)
                    .rounded(6.)
                    .background(colour.clone())
                    .border_color(if chosen {
                        ColorToken::BaseContent.into()
                    } else {
                        String::from("#00000022")
                    })
                    .border_width(if chosen { 2. } else { 1. })
                    .on_click(move |app: &mut C| (pick.borrow_mut())(app, colour.clone()))
                    .into()
            })
            .collect();
        let channel = |name: &'static str,
                       value: f32,
                       range: std::ops::RangeInclusive<f32>,
                       set: fn(Hsl, f32) -> Hsl|
         -> Element<C> {
            let pick = change.clone();
            hstack![
                text(name)
                    .font_size(12.)
                    .color(ColorToken::Muted)
                    .width(14.),
                slider(value)
                    .key(format!("{key}-{}", name.to_lowercase()))
                    .accessible_label(match name {
                        "H" => "Hue",
                        "S" => "Saturation",
                        _ => "Lightness",
                    })
                    .range(range)
                    .flex(1.)
                    .on_change(move |app: &mut C, v| (pick.borrow_mut())(app, set(hsl, v).hex())),
            ]
            .key(format!("{key}-{name}-row"))
            .width(300.)
            .gap(8., Axis::Horizontal)
            .align(Align::Center)
            .into()
        };
        let typed = change.clone();
        vstack![
            hstack![
                vstack![]
                    .key(format!("{key}-preview"))
                    .width(48.)
                    .height(48.)
                    .rounded(10.)
                    .background(current.clone())
                    .border_color(ColorToken::Border)
                    .border_width(1.),
                text_field(current.clone())
                    .key(format!("{key}-hex"))
                    .accessible_label("Hex colour")
                    .font_family("SF Mono, monospace")
                    .flex(1.)
                    .on_change(move |app: &mut C, text| {
                        if let Some(c) = Hsl::parse(&text) {
                            (typed.borrow_mut())(app, c.hex());
                        }
                    }),
            ]
            .key(format!("{key}-head"))
            .gap(10., Axis::Horizontal)
            .align(Align::Center),
            grid(swatches)
                .key(format!("{key}-swatches"))
                .columns(10)
                .gap(6., Axis::Both),
            channel("H", hsl.hue, 0. ..=360., |c, v| Hsl { hue: v, ..c }),
            channel("S", hsl.saturation, 0. ..=1., |c, v| Hsl {
                saturation: v,
                ..c
            }),
            channel("L", hsl.lightness, 0. ..=1., |c, v| Hsl {
                lightness: v,
                ..c
            }),
        ]
        .key(key)
        .width(300.)
        .gap(12., Axis::Vertical)
        .into()
    }
}

// ── One-time code ───────────────────────────────────────────────────────

type Code<C> = Rc<RefCell<dyn FnMut(&mut C, String)>>;
/// A fixed number of digit boxes backed by one field, as for sign-in codes.
pub struct OtpInput<C> {
    key: Option<String>,
    value: String,
    length: usize,
    on_change: Option<Code<C>>,
    on_complete: Option<Code<C>>,
}
pub fn otp_input<C>(value: impl Into<String>, length: usize) -> OtpInput<C> {
    OtpInput {
        key: None,
        value: value.into(),
        length,
        on_change: None,
        on_complete: None,
    }
}
impl<C> OtpInput<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    /// Called with the digits typed so far (non-digits are dropped).
    pub fn on_change(mut self, f: impl FnMut(&mut C, String) + 'static) -> Self {
        self.on_change = Some(Rc::new(RefCell::new(f)));
        self
    }
    /// Called once every box is filled.
    pub fn on_complete(mut self, f: impl FnMut(&mut C, String) + 'static) -> Self {
        self.on_complete = Some(Rc::new(RefCell::new(f)));
        self
    }
}
impl<C: 'static> From<OtpInput<C>> for Element<C> {
    fn from(o: OtpInput<C>) -> Self {
        let Some(key) = o.key.filter(|k| !k.is_empty()) else {
            return invalid("otp_input requires a key");
        };
        let Some(change) = o.on_change else {
            return invalid("otp_input requires on_change");
        };
        if !(2..=12).contains(&o.length) {
            return invalid("otp_input length must be 2..=12");
        }
        let digits: Vec<char> = o
            .value
            .chars()
            .filter(char::is_ascii_digit)
            .take(o.length)
            .collect();
        let boxes: Vec<Element<C>> = (0..o.length)
            .map(|i| {
                let active = i == digits.len();
                vstack![text(digits.get(i).map_or(String::new(), |c| c.to_string()))
                    .font_size(20.)
                    .font_weight(600)
                    .text_align(TextAlign::Center)]
                .key(format!("{key}-box-{i}"))
                .width(42.)
                .height(48.)
                .flow(Flow::Center)
                .rounded(8.)
                .border_color(if active {
                    ColorToken::Primary.into()
                } else {
                    String::from(ColorToken::Border)
                })
                .border_width(if active { 2. } else { 1. })
                .into()
            })
            .collect();
        let (length, complete) = (o.length, o.on_complete);
        let row_width = o.length as f32 * 50. - 8.;
        // The real field sits on top, transparent, so clicks focus it and
        // typing, paste and backspace all work; the boxes show its digits.
        zstack![
            hstack(boxes)
                .key(format!("{key}-boxes"))
                .gap(8., Axis::Horizontal),
            text_field(digits.iter().collect::<String>())
                .key(format!("{key}-field"))
                .accessible_label(format!("{length}-digit code"))
                .color("#00000000")
                .background("#00000000")
                .border_width(0.)
                .height(48.)
                .on_change(move |app: &mut C, text| {
                    let code: String = text
                        .chars()
                        .filter(char::is_ascii_digit)
                        .take(length)
                        .collect();
                    let done = code.len() == length;
                    (change.borrow_mut())(app, code.clone());
                    if done {
                        if let Some(f) = &complete {
                            (f.borrow_mut())(app, code);
                        }
                    }
                }),
        ]
        .key(key)
        .width(row_width)
        .height(48.)
        .align(Align::Stretch)
        .into()
    }
}

// ── Rating ──────────────────────────────────────────────────────────────

type Stars<C> = Rc<RefCell<dyn FnMut(&mut C, u8)>>;
/// A row of stars; clicking the current rating again clears it.
pub struct Rating<C> {
    key: Option<String>,
    value: u8,
    max: u8,
    on_change: Option<Stars<C>>,
    size: f32,
}
pub fn rating<C>(value: u8) -> Rating<C> {
    Rating {
        key: None,
        value,
        max: 5,
        on_change: None,
        size: 28.,
    }
}
impl<C> Rating<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn max(mut self, value: u8) -> Self {
        self.max = value;
        self
    }
    pub fn size(mut self, value: f32) -> Self {
        self.size = value;
        self
    }
    /// Read-only when not set.
    pub fn on_change(mut self, f: impl FnMut(&mut C, u8) + 'static) -> Self {
        self.on_change = Some(Rc::new(RefCell::new(f)));
        self
    }
}
impl<C: 'static> From<Rating<C>> for Element<C> {
    fn from(r: Rating<C>) -> Self {
        let Some(key) = r.key.filter(|k| !k.is_empty()) else {
            return invalid("rating requires a key");
        };
        if !(1..=10).contains(&r.max) {
            return invalid("rating max must be 1..=10");
        }
        let stars: Vec<Element<C>> = (1..=r.max)
            .map(|n| {
                let lit = n <= r.value;
                let glyph = icon(if lit { Icon::StarFilled } else { Icon::Star })
                    .size(r.size * 0.7)
                    .color(if lit {
                        String::from("#F59E0B")
                    } else {
                        ColorToken::Muted.into()
                    });
                match &r.on_change {
                    Some(change) => {
                        let (set, current) = (change.clone(), r.value);
                        zstack![
                            button("")
                                .key(format!("{key}-{n}"))
                                .accessible_label(format!("{n} of {}", r.max))
                                .variant(ButtonVariant::Ghost)
                                .width(r.size)
                                .height(r.size)
                                .on_click(move |app: &mut C| {
                                    (set.borrow_mut())(app, if current == n { 0 } else { n })
                                }),
                            glyph,
                        ]
                        .key(format!("{key}-{n}-layer"))
                        .width(r.size)
                        .height(r.size)
                        .flow(Flow::Center)
                        .into()
                    }
                    None => vstack![glyph]
                        .key(format!("{key}-{n}"))
                        .width(r.size)
                        .height(r.size)
                        .flow(Flow::Center)
                        .into(),
                }
            })
            .collect();
        let mut row = hstack(stars).key(key).gap(2., Axis::Horizontal);
        row.0.node.accessible_label = Some(format!("Rated {} of {}", r.value, r.max));
        row.into()
    }
}

// ── Form field ──────────────────────────────────────────────────────────

/// A labelled control with optional description and error message.
pub struct FormField<C> {
    key: Option<String>,
    label: String,
    control: Element<C>,
    description: Option<String>,
    error: Option<String>,
    required: bool,
}
pub fn form_field<C>(label: impl Into<String>, control: impl Into<Element<C>>) -> FormField<C> {
    FormField {
        key: None,
        label: label.into(),
        control: control.into(),
        description: None,
        error: None,
        required: false,
    }
}
impl<C> FormField<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn description(mut self, value: impl Into<String>) -> Self {
        self.description = Some(value.into());
        self
    }
    /// An error message replaces the description and turns red.
    pub fn error(mut self, value: Option<String>) -> Self {
        self.error = value;
        self
    }
    pub fn required(mut self, value: bool) -> Self {
        self.required = value;
        self
    }
}
impl<C: 'static> From<FormField<C>> for Element<C> {
    fn from(mut f: FormField<C>) -> Self {
        let Some(key) = f.key.filter(|k| !k.is_empty()) else {
            return invalid("form_field requires a key");
        };
        // The painted label must name the control for assistive technology
        // too. Preserve an explicit application-provided accessible name.
        if f.control.node.accessible_label.is_none() {
            f.control.node.accessible_label = Some(f.label.clone());
        }
        let mut label: Vec<Element<C>> = vec![text(f.label)
            .key(format!("{key}-label"))
            .font_size(13.)
            .font_weight(500)
            .into()];
        if f.required {
            label.push(
                text("*")
                    .key(format!("{key}-required"))
                    .font_size(13.)
                    .color(ColorToken::Error)
                    .into(),
            );
        }
        let mut parts: Vec<Element<C>> =
            vec![hstack(label).gap(3., Axis::Horizontal).into(), f.control];
        match (f.error, f.description) {
            (Some(error), _) => parts.push(
                hstack![
                    icon(Icon::Error).size(14.).color(ColorToken::Error),
                    text(error).font_size(12.).color(ColorToken::Error).flex(1.),
                ]
                .key(format!("{key}-error"))
                .gap(6., Axis::Horizontal)
                .align(Align::Center)
                .into(),
            ),
            (None, Some(d)) => parts.push(
                text(d)
                    .key(format!("{key}-description"))
                    .font_size(12.)
                    .color(ColorToken::Muted)
                    .into(),
            ),
            (None, None) => {}
        }
        vstack(parts).key(key).gap(6., Axis::Vertical).into()
    }
}
// ── Drop zone ───────────────────────────────────────────────────────────

type Paths<C> = Rc<RefCell<dyn FnMut(&mut C, Vec<std::path::PathBuf>)>>;
/// A dashed target that accepts files dragged in from the system, with an
/// optional browse button for a file dialog the app opens itself.
pub struct DropZone<C> {
    key: Option<String>,
    title: String,
    description: Option<String>,
    extensions: Vec<String>,
    multiple: bool,
    on_drop: Option<Paths<C>>,
    on_browse: Option<Click<C>>,
    height: f32,
}
pub fn drop_zone<C>() -> DropZone<C> {
    DropZone {
        key: None,
        title: "Drop files here".into(),
        description: None,
        extensions: Vec::new(),
        multiple: true,
        on_drop: None,
        on_browse: None,
        height: 160.,
    }
}
impl<C> DropZone<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn title(mut self, value: impl Into<String>) -> Self {
        self.title = value.into();
        self
    }
    pub fn description(mut self, value: impl Into<String>) -> Self {
        self.description = Some(value.into());
        self
    }
    /// Accept only these extensions (case-insensitive, without the dot).
    pub fn accept(mut self, extensions: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.extensions = extensions
            .into_iter()
            .map(|e| e.into().trim_start_matches('.').to_lowercase())
            .collect();
        self
    }
    /// Keep only the first accepted file when false.
    pub fn multiple(mut self, value: bool) -> Self {
        self.multiple = value;
        self
    }
    pub fn height(mut self, value: f32) -> Self {
        self.height = value;
        self
    }
    /// Called with the accepted paths; never called with none.
    pub fn on_drop(mut self, f: impl FnMut(&mut C, Vec<std::path::PathBuf>) + 'static) -> Self {
        self.on_drop = Some(Rc::new(RefCell::new(f)));
        self
    }
    /// Shows a "Browse…" button.
    pub fn on_browse(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.on_browse = Some(Box::new(f));
        self
    }
}
/// Paths whose extension is accepted (all when the list is empty).
pub(super) fn accepted(
    paths: Vec<std::path::PathBuf>,
    extensions: &[String],
    multiple: bool,
) -> Vec<std::path::PathBuf> {
    let mut kept: Vec<_> = paths
        .into_iter()
        .filter(|p| {
            extensions.is_empty()
                || p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| extensions.contains(&e.to_lowercase()))
        })
        .collect();
    if !multiple {
        kept.truncate(1);
    }
    kept
}
impl<C: 'static> From<DropZone<C>> for Element<C> {
    fn from(d: DropZone<C>) -> Self {
        let Some(key) = d.key.filter(|k| !k.is_empty()) else {
            return invalid("drop_zone requires a key");
        };
        let Some(drop) = d.on_drop else {
            return invalid("drop_zone requires on_drop");
        };
        let mut copy: Vec<Element<C>> = vec![
            icon(Icon::Upload2)
                .size(28.)
                .color(ColorToken::Muted)
                .into(),
            text(d.title).font_size(14.).font_weight(600).into(),
        ];
        let hint = d.description.or_else(|| {
            (!d.extensions.is_empty()).then(|| {
                d.extensions
                    .iter()
                    .map(|e| e.to_uppercase())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
        });
        if let Some(hint) = hint {
            copy.push(text(hint).font_size(12.).color(ColorToken::Muted).into());
        }
        if let Some(browse) = d.on_browse {
            let mut b: Button<C> = button("Browse…")
                .key(format!("{key}-browse"))
                .variant(ButtonVariant::Outline)
                .height(30.);
            b.0.callback = Some(Callback::Click(browse));
            copy.push(b.into());
        }
        let (extensions, multiple) = (d.extensions, d.multiple);
        let mut zone = vstack(copy)
            .key(key)
            .height(d.height)
            .flow(Flow::Center)
            .gap(8., Axis::Vertical)
            .padding(16., Edge::All)
            .rounded(12.)
            .border_color(ColorToken::Border)
            .border_width(1.5)
            .border_style(BorderStyle::Dashed)
            .hover_style(|s| {
                s.background(ColorToken::Primary.alpha(14))
                    .border_color(ColorToken::Primary)
            });
        zone.0.callback = Some(Callback::Files(Box::new(move |app: &mut C, paths| {
            let paths = accepted(paths, &extensions, multiple);
            if !paths.is_empty() {
                (drop.borrow_mut())(app, paths);
            }
        })));
        zone.0.node.accessible_label = Some("File drop zone".into());
        zone.into()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hsl_round_trips_through_hex() {
        for hex in [
            "#EF4444", "#22C55E", "#3B82F6", "#18181B", "#FFFFFF", "#808080",
        ] {
            assert_eq!(Hsl::parse(hex).unwrap().hex(), hex);
        }
        assert!(Hsl::parse("#12345").is_none());
        assert!(Hsl::parse("zzzzzz").is_none());
    }
    #[test]
    fn filtering_prefers_prefix_matches() {
        let items: Vec<((), String)> = ["Banana", "Apple", "Pineapple", "Apricot"]
            .iter()
            .map(|s| ((), s.to_string()))
            .collect();
        let hits: Vec<&str> = filter_items(&items, "ap")
            .into_iter()
            .map(|(_, l)| l)
            .collect();
        assert_eq!(hits, ["Apple", "Apricot", "Pineapple"]);
    }
    #[test]
    fn drop_zone_filters_extensions_and_single_file() {
        use std::path::PathBuf;
        let paths = || {
            ["a.PNG", "b.txt", "c.jpg", "noext"]
                .map(PathBuf::from)
                .to_vec()
        };
        let exts = ["png".to_owned(), "jpg".to_owned()];
        assert_eq!(
            accepted(paths(), &exts, true),
            ["a.PNG", "c.jpg"].map(PathBuf::from)
        );
        assert_eq!(accepted(paths(), &exts, false), [PathBuf::from("a.PNG")]);
        assert_eq!(accepted(paths(), &[], true).len(), 4);
    }
    #[test]
    fn inserted_text_is_the_edit_between_prefix_and_suffix() {
        assert_eq!(inserted("Apricot", "Apribancot"), "ban");
        assert_eq!(inserted("Apricot", "Apricotb"), "b");
        assert_eq!(inserted("Apricot", "xApricot"), "x");
        assert_eq!(inserted("aa", "aaa"), "a");
    }
}
