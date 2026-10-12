//! Disclosure and transient surfaces: collapsible sections, accordions,
//! side drawers, toasts and a command palette. All are controlled: open
//! state lives in the application and callbacks report requested changes.
use super::display::Severity;
use super::*;
use crate::declarative::document::OverlayKind;
use std::{cell::RefCell, rc::Rc};

fn invalid<C>(message: &str) -> Element<C> {
    let mut e = Element::new(Kind::Column);
    e.validation_error = Some(message.into());
    e
}

// ── Collapsible ─────────────────────────────────────────────────────────

/// A header button that shows or hides its content.
pub struct Collapsible<C> {
    key: Option<String>,
    title: String,
    content: Element<C>,
    open: bool,
    on_toggle: Option<Change<C>>,
}
pub fn collapsible<C>(title: impl Into<String>, content: impl Into<Element<C>>) -> Collapsible<C> {
    Collapsible {
        key: None,
        title: title.into(),
        content: content.into(),
        open: false,
        on_toggle: None,
    }
}
impl<C> Collapsible<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn open(mut self, value: bool) -> Self {
        self.open = value;
        self
    }
    /// Called with the requested open state.
    pub fn on_toggle(mut self, f: impl FnMut(&mut C, bool) + 'static) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }
}
/// The shared header + body composition used by collapsibles and accordions.
fn section<C>(
    key: &str,
    title: String,
    content: Element<C>,
    open: bool,
    toggle: Click<C>,
    divider: bool,
) -> Element<C> {
    let mut header: Element<C> = button(title)
        .key(format!("{key}-trigger"))
        .variant(ButtonVariant::Ghost)
        .disclosure(open)
        .height(40.)
        .font_weight(500)
        .into();
    header.callback = Some(Callback::Click(toggle));
    header.node.access_expanded = Some(open);
    let mut parts = vec![header];
    if open {
        parts.push(
            vstack![content]
                .key(format!("{key}-content"))
                .padding(10., Edge::Horizontal)
                .padding(4., Edge::Top)
                .padding(12., Edge::Bottom)
                .transition(Transition::slide(0., -6.).duration_ms(160))
                .into(),
        );
    }
    let mut column = vstack(parts).key(key.to_owned()).align(Align::Stretch);
    if divider {
        column = column
            .border_sides(Sides::BOTTOM)
            .border_color(ColorToken::Border)
            .border_width(1.);
    }
    column.into()
}
impl<C: 'static> From<Collapsible<C>> for Element<C> {
    fn from(c: Collapsible<C>) -> Self {
        let Some(key) = c.key.filter(|k| !k.is_empty()) else {
            return invalid("collapsible requires a key");
        };
        let Some(mut toggle) = c.on_toggle else {
            return invalid("collapsible requires on_toggle");
        };
        let open = c.open;
        section(
            &key,
            c.title,
            c.content,
            open,
            Box::new(move |app: &mut C| toggle(app, !open)),
            false,
        )
    }
}

// ── Accordion ───────────────────────────────────────────────────────────

type Expanded<C> = Rc<RefCell<dyn FnMut(&mut C, Vec<usize>)>>;
/// Stacked collapsible sections. Single mode (default) keeps one open.
pub struct Accordion<C> {
    key: Option<String>,
    sections: Vec<(String, Element<C>)>,
    expanded: Vec<usize>,
    multiple: bool,
    on_change: Option<Expanded<C>>,
}
pub fn accordion<C>(
    sections: impl IntoIterator<Item = (impl Into<String>, Element<C>)>,
) -> Accordion<C> {
    Accordion {
        key: None,
        sections: sections.into_iter().map(|(t, c)| (t.into(), c)).collect(),
        expanded: Vec::new(),
        multiple: false,
        on_change: None,
    }
}
impl<C> Accordion<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    /// Indices of the open sections.
    pub fn expanded(mut self, indices: impl IntoIterator<Item = usize>) -> Self {
        self.expanded = indices.into_iter().collect();
        self
    }
    /// Allow several sections open at once.
    pub fn multiple(mut self, value: bool) -> Self {
        self.multiple = value;
        self
    }
    /// Called with the next set of open indices.
    pub fn on_change(mut self, f: impl FnMut(&mut C, Vec<usize>) + 'static) -> Self {
        self.on_change = Some(Rc::new(RefCell::new(f)));
        self
    }
}
/// The open set after toggling `index`.
pub(super) fn toggled(expanded: &[usize], index: usize, multiple: bool) -> Vec<usize> {
    let open = expanded.contains(&index);
    match (open, multiple) {
        (true, _) => expanded.iter().copied().filter(|&i| i != index).collect(),
        (false, true) => {
            let mut next = expanded.to_vec();
            next.push(index);
            next.sort_unstable();
            next
        }
        (false, false) => vec![index],
    }
}
impl<C: 'static> From<Accordion<C>> for Element<C> {
    fn from(a: Accordion<C>) -> Self {
        let Some(key) = a.key.filter(|k| !k.is_empty()) else {
            return invalid("accordion requires a key");
        };
        let Some(change) = a.on_change else {
            return invalid("accordion requires on_change");
        };
        if a.sections.is_empty() {
            return invalid("accordion requires at least one section");
        }
        let (expanded, multiple) = (a.expanded, a.multiple);
        let rows: Vec<Element<C>> = a
            .sections
            .into_iter()
            .enumerate()
            .map(|(i, (title, content))| {
                let (change, current) = (change.clone(), expanded.clone());
                section(
                    &format!("{key}-{i}"),
                    title,
                    content,
                    expanded.contains(&i),
                    Box::new(move |app: &mut C| {
                        (change.borrow_mut())(app, toggled(&current, i, multiple))
                    }),
                    true,
                )
            })
            .collect();
        vstack(rows).key(key).into()
    }
}

// ── Drawer ──────────────────────────────────────────────────────────────

/// The window edge a drawer slides in from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DrawerSide {
    Left,
    #[default]
    Right,
    Top,
    Bottom,
}
/// A modal panel attached to a window edge (a "sheet" in some libraries).
pub struct Drawer<C> {
    key: Option<String>,
    content: Element<C>,
    title: Option<String>,
    side: DrawerSide,
    size: f32,
    open: bool,
    on_dismiss: Option<Click<C>>,
}
pub fn drawer<C>(content: impl Into<Element<C>>) -> Drawer<C> {
    Drawer {
        key: None,
        content: content.into(),
        title: None,
        side: DrawerSide::Right,
        size: 360.,
        open: false,
        on_dismiss: None,
    }
}
impl<C> Drawer<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn title(mut self, value: impl Into<String>) -> Self {
        self.title = Some(value.into());
        self
    }
    pub fn side(mut self, value: DrawerSide) -> Self {
        self.side = value;
        self
    }
    /// Width for left/right drawers, height for top/bottom ones.
    pub fn size(mut self, value: f32) -> Self {
        self.size = value;
        self
    }
    pub fn open(mut self, value: bool) -> Self {
        self.open = value;
        self
    }
    /// Escape, a backdrop press or the close button. Set `open = false`.
    pub fn on_dismiss(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.on_dismiss = Some(Box::new(f));
        self
    }
}
impl<C: 'static> From<Drawer<C>> for Element<C> {
    fn from(d: Drawer<C>) -> Self {
        let Some(key) = d.key.filter(|k| !k.is_empty()) else {
            return invalid("drawer requires a key");
        };
        if !d.open {
            return zstack![]
                .key(key)
                .width(0.)
                .height(0.)
                .scroll_passthrough(true)
                .into();
        }
        let Some(dismiss) = d.on_dismiss else {
            return invalid("open drawer requires on_dismiss");
        };
        let dismiss = Rc::new(RefCell::new(dismiss));
        let close = dismiss.clone();
        let mut head: Vec<Element<C>> = Vec::new();
        if let Some(title) = d.title {
            head.push(text(title).font_size(17.).font_weight(600).flex(1.).into());
        } else {
            head.push(spacer().into());
        }
        head.push(
            zstack![
                button("")
                    .key(format!("{key}-close"))
                    .accessible_label("Close")
                    .variant(ButtonVariant::Ghost)
                    .width(30.)
                    .height(30.)
                    .on_click(move |app: &mut C| (close.borrow_mut())(app)),
                icon(Icon::Close).size(16.),
            ]
            .key(format!("{key}-close-layer"))
            .width(30.)
            .height(30.)
            .flow(Flow::Center)
            .into(),
        );
        let horizontal = matches!(d.side, DrawerSide::Left | DrawerSide::Right);
        let (slide, flow, sides) = match d.side {
            DrawerSide::Left => ([-24., 0.], Flow::Left, Sides::RIGHT),
            DrawerSide::Right => ([24., 0.], Flow::Right, Sides::LEFT),
            DrawerSide::Top => ([0., -24.], Flow::Top, Sides::BOTTOM),
            DrawerSide::Bottom => ([0., 24.], Flow::Bottom, Sides::TOP),
        };
        let mut card = vstack![
            hstack(head)
                .key(format!("{key}-head"))
                .align(Align::Center)
                .gap(8., Axis::Horizontal),
            vstack![d.content]
                .key(format!("{key}-body"))
                .flex(1.)
                .scroll(Scroll::Auto),
        ]
        .key(format!("{key}-content"))
        .gap(12., Axis::Vertical)
        .padding(20., Edge::All)
        .background(ColorToken::Base200)
        .border_sides(sides)
        .border_color(ColorToken::Border)
        .border_width(1.)
        .shadow(tokens::shadow::xl())
        .transition(Transition::slide(slide[0], slide[1]).duration_ms(220));
        card = if horizontal {
            card.width(d.size)
        } else {
            card.height(d.size)
        };
        let mut root = super::dialogs::overlay_scope(
            card.into(),
            key,
            OverlayKind::Modal,
            Box::new(move |app: &mut C| (dismiss.borrow_mut())(app)),
        );
        root.node.flow = Some(flow);
        root.node.align = Align::Stretch;
        root
    }
}

// ── Toasts ──────────────────────────────────────────────────────────────

/// One notification. `id` must be unique among the visible toasts.
#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    pub id: u64,
    pub title: String,
    pub description: Option<String>,
    pub tone: Severity,
    pub action: Option<String>,
}
impl Toast {
    pub fn new(id: u64, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            description: None,
            tone: Severity::Neutral,
            action: None,
        }
    }
    pub fn description(mut self, value: impl Into<String>) -> Self {
        self.description = Some(value.into());
        self
    }
    pub fn tone(mut self, value: Severity) -> Self {
        self.tone = value;
        self
    }
    /// A button label; presses call the toaster's `on_action`.
    pub fn action(mut self, label: impl Into<String>) -> Self {
        self.action = Some(label.into());
        self
    }
}
/// Where the toast stack sits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToastPosition {
    TopRight,
    TopCenter,
    #[default]
    BottomRight,
    BottomCenter,
    BottomLeft,
}
type ToastEvent<C> = Rc<RefCell<dyn FnMut(&mut C, u64)>>;
/// A non-modal stack of toasts. Layer it over the page as the last child of
/// a `zstack!`; the page underneath stays interactive. Timing is the app's:
/// remove a toast after a delay (e.g. via `UiSender`) for auto-dismissal.
pub struct Toaster<C> {
    key: String,
    toasts: Vec<Toast>,
    position: ToastPosition,
    on_dismiss: Option<ToastEvent<C>>,
    on_action: Option<ToastEvent<C>>,
}
pub fn toaster<C>(toasts: &[Toast]) -> Toaster<C> {
    Toaster {
        key: "toaster".into(),
        toasts: toasts.iter().rev().take(5).rev().cloned().collect(),
        position: ToastPosition::BottomRight,
        on_dismiss: None,
        on_action: None,
    }
}
impl<C> Toaster<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = key.into();
        self
    }
    pub fn position(mut self, value: ToastPosition) -> Self {
        self.position = value;
        self
    }
    pub fn on_dismiss(mut self, f: impl FnMut(&mut C, u64) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(RefCell::new(f)));
        self
    }
    pub fn on_action(mut self, f: impl FnMut(&mut C, u64) + 'static) -> Self {
        self.on_action = Some(Rc::new(RefCell::new(f)));
        self
    }
}
impl<C: 'static> From<Toaster<C>> for Element<C> {
    fn from(t: Toaster<C>) -> Self {
        let key = t.key;
        let Some(dismiss) = t.on_dismiss else {
            return invalid("toaster requires on_dismiss");
        };
        if t.toasts.iter().any(|x| x.action.is_some()) && t.on_action.is_none() {
            return invalid("toasts with actions require on_action");
        }
        let cards: Vec<Element<C>> = t
            .toasts
            .into_iter()
            .map(|toast| {
                let id = toast.id;
                let (glyph, tint) = toast.tone.style();
                let mut copy: Vec<Element<C>> =
                    vec![text(toast.title).font_size(14.).font_weight(600).into()];
                if let Some(d) = toast.description {
                    copy.push(text(d).font_size(13.).color(ColorToken::Muted).into());
                }
                let mut row: Vec<Element<C>> = Vec::new();
                if let Some(g) = glyph {
                    row.push(icon(g).size(18.).color(tint).into());
                }
                row.push(vstack(copy).gap(2., Axis::Vertical).flex(1.).into());
                if let (Some(label), Some(act)) = (toast.action, &t.on_action) {
                    let act = act.clone();
                    row.push(
                        button(label)
                            .key(format!("{key}-{id}-action"))
                            .variant(ButtonVariant::Outline)
                            .height(28.)
                            .font_size(12.)
                            .on_click(move |app: &mut C| (act.borrow_mut())(app, id))
                            .into(),
                    );
                }
                let close = dismiss.clone();
                row.push(
                    zstack![
                        button("")
                            .key(format!("{key}-{id}-close"))
                            .accessible_label("Dismiss")
                            .variant(ButtonVariant::Ghost)
                            .width(24.)
                            .height(24.)
                            .on_click(move |app: &mut C| (close.borrow_mut())(app, id)),
                        icon(Icon::Close).size(13.).color(ColorToken::Muted),
                    ]
                    .key(format!("{key}-{id}-close-layer"))
                    .width(24.)
                    .height(24.)
                    .flow(Flow::Center)
                    .into(),
                );
                hstack(row)
                    .key(format!("{key}-{id}"))
                    .width(340.)
                    .gap(12., Axis::Horizontal)
                    .align(Align::Start)
                    .padding(14., Edge::All)
                    .rounded(12.)
                    .background(ColorToken::Base200)
                    .border_color(ColorToken::Border)
                    .border_width(1.)
                    .shadow(tokens::shadow::lg())
                    .transition(Transition::slide(0., 12.).duration_ms(200))
                    .into()
            })
            .collect();
        let flow = match t.position {
            ToastPosition::TopRight => Flow::TopRight,
            ToastPosition::TopCenter => Flow::Top,
            ToastPosition::BottomRight => Flow::BottomRight,
            ToastPosition::BottomCenter => Flow::Bottom,
            ToastPosition::BottomLeft => Flow::BottomLeft,
        };
        zstack![vstack(cards)
            .key(format!("{key}-stack"))
            .gap(8., Axis::Vertical)
            .scroll_passthrough(true)]
        .key(key)
        .flow(flow)
        .padding(16., Edge::All)
        .scroll_passthrough(true)
        .into()
    }
}

// ── Command palette ─────────────────────────────────────────────────────

/// Open flag and typed query, kept in app state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommandPaletteState {
    pub open: bool,
    pub query: String,
}
/// One runnable command.
pub struct Command<T> {
    value: T,
    label: String,
    group: Option<String>,
    shortcut: Option<String>,
    icon: Option<Icon>,
    keywords: Vec<String>,
}
pub fn command<T>(value: T, label: impl Into<String>) -> Command<T> {
    Command {
        value,
        label: label.into(),
        group: None,
        shortcut: None,
        icon: None,
        keywords: Vec::new(),
    }
}
impl<T> Command<T> {
    /// Heading the command is listed under.
    pub fn group(mut self, value: impl Into<String>) -> Self {
        self.group = Some(value.into());
        self
    }
    pub fn shortcut(mut self, hint: impl Into<String>) -> Self {
        self.shortcut = Some(hint.into());
        self
    }
    pub fn icon(mut self, glyph: Icon) -> Self {
        self.icon = Some(glyph);
        self
    }
    /// Extra words matched by the search but not shown.
    pub fn keywords(mut self, words: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.keywords = words.into_iter().map(Into::into).collect();
        self
    }
    fn score(&self, query: &str) -> Option<u8> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Some(2);
        }
        let label = self.label.to_lowercase();
        if label.starts_with(&q) {
            Some(0)
        } else if label.contains(&q) {
            Some(1)
        } else if self.keywords.iter().any(|k| k.to_lowercase().contains(&q)) {
            Some(2)
        } else {
            // Subsequence ("nwf" → "New file").
            let mut chars = label.chars();
            q.chars()
                .filter(|c| !c.is_whitespace())
                .all(|c| chars.any(|l| l == c))
                .then_some(3)
        }
    }
}
type Run<C, T> = Rc<RefCell<dyn FnMut(&mut C, T)>>;
type Access<C, S> = Rc<dyn Fn(&mut C) -> &mut S>;
/// A searchable modal list of commands (⌘K). Enter runs the best match.
pub struct CommandPalette<C, T> {
    key: String,
    commands: Vec<Command<T>>,
    snapshot: CommandPaletteState,
    state: Option<Access<C, CommandPaletteState>>,
    on_run: Option<Run<C, T>>,
    placeholder: String,
}
pub fn command_palette<C, T>(
    commands: impl IntoIterator<Item = Command<T>>,
    state: &CommandPaletteState,
) -> CommandPalette<C, T> {
    CommandPalette {
        key: "command-palette".into(),
        commands: commands.into_iter().collect(),
        snapshot: state.clone(),
        state: None,
        on_run: None,
        placeholder: "Type a command or search…".into(),
    }
}
impl<C, T> CommandPalette<C, T> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = key.into();
        self
    }
    pub fn state(mut self, access: impl Fn(&mut C) -> &mut CommandPaletteState + 'static) -> Self {
        self.state = Some(Rc::new(access));
        self
    }
    pub fn on_run(mut self, f: impl FnMut(&mut C, T) + 'static) -> Self {
        self.on_run = Some(Rc::new(RefCell::new(f)));
        self
    }
    pub fn placeholder(mut self, value: impl Into<String>) -> Self {
        self.placeholder = value.into();
        self
    }
}
impl<C: 'static, T: Clone + 'static> From<CommandPalette<C, T>> for Element<C> {
    fn from(p: CommandPalette<C, T>) -> Self {
        let key = p.key;
        let (Some(state), Some(run)) = (p.state, p.on_run) else {
            return invalid("command_palette requires state(..) and on_run(..)");
        };
        if !p.snapshot.open {
            return zstack![]
                .key(key)
                .width(0.)
                .height(0.)
                .scroll_passthrough(true)
                .into();
        }
        // Rank, then keep group order by first appearance.
        let mut hits: Vec<(u8, usize)> = p
            .commands
            .iter()
            .enumerate()
            .filter_map(|(i, c)| Some((c.score(&p.snapshot.query)?, i)))
            .collect();
        hits.sort();
        hits.truncate(50);
        let mut groups: Vec<(Option<String>, Vec<usize>)> = Vec::new();
        for (_, i) in &hits {
            let g = p.commands[*i].group.clone();
            match groups.iter_mut().find(|(k, _)| *k == g) {
                Some((_, list)) => list.push(*i),
                None => groups.push((g, vec![*i])),
            }
        }
        let best = groups
            .first()
            .and_then(|(_, l)| l.first())
            .map(|&i| p.commands[i].value.clone());
        let close = |s: &mut CommandPaletteState| {
            s.open = false;
            s.query.clear();
        };
        let (typing, submit_state, submit_run) = (state.clone(), state.clone(), run.clone());
        let field = text_field(p.snapshot.query.clone())
            .key(format!("{key}-field"))
            .accessible_label("Search commands")
            .placeholder(p.placeholder)
            .leading_icon(Icon::Search)
            .height(44.)
            .font_size(15.)
            .on_change(move |app: &mut C, text| typing(app).query = text)
            .on_submit(move |app: &mut C| {
                if let Some(v) = best.clone() {
                    close(submit_state(app));
                    (submit_run.borrow_mut())(app, v);
                }
            });
        let mut list: Vec<Element<C>> = Vec::new();
        for (g, (group, items)) in groups.into_iter().enumerate() {
            if let Some(name) = group {
                list.push(
                    hstack![text(name.to_uppercase())
                        .font_size(11.)
                        .font_weight(600)
                        .letter_spacing(0.06)
                        .color(ColorToken::Muted)]
                    .key(format!("{key}-group-{g}"))
                    .padding(10., Edge::Left)
                    .padding(8., Edge::Top)
                    .into(),
                );
            }
            for i in items {
                let c = &p.commands[i];
                let (value, go, s) = (c.value.clone(), run.clone(), state.clone());
                let mut b: Element<C> = button(c.label.clone())
                    .key(format!("{key}-cmd-{i}"))
                    .variant(ButtonVariant::Ghost)
                    .label_align(Align::Start)
                    .label_inset(if c.icon.is_some() { 38. } else { 12. })
                    .height(38.)
                    .on_click(move |app: &mut C| {
                        close(s(app));
                        (go.borrow_mut())(app, value.clone());
                    })
                    .into();
                b.node.access_role = Some(AccessRole::ListBoxOption);
                let mut decor: Vec<Element<C>> = Vec::new();
                if let Some(g) = c.icon {
                    decor.push(icon(g).size(16.).color(ColorToken::Muted).into());
                }
                decor.push(spacer().into());
                if let Some(hint) = &c.shortcut {
                    decor.push(super::display::kbd(hint.clone()).into());
                }
                list.push(
                    zstack![
                        b,
                        hstack(decor)
                            .key(format!("{key}-cmd-{i}-decor"))
                            .align(Align::Center)
                            .padding(12., Edge::Horizontal)
                            .scroll_passthrough(true),
                    ]
                    .key(format!("{key}-cmd-{i}-row"))
                    .align(Align::Stretch)
                    .height(38.)
                    .into(),
                );
            }
        }
        if list.is_empty() {
            list.push(
                hstack![text("No matching commands")
                    .color(ColorToken::Muted)
                    .font_size(13.)]
                .key(format!("{key}-empty"))
                .padding(12., Edge::All)
                .into(),
            );
        }
        let mut results = vstack(list)
            .key(format!("{key}-results"))
            .gap(2., Axis::Vertical)
            .max_height(360.)
            .scroll(Scroll::Auto);
        results.0.node.access_role = Some(AccessRole::ListBox);
        let card = vstack![field, separator(), results]
            .key(format!("{key}-content"))
            .width(560.)
            .gap(8., Axis::Vertical)
            .padding(10., Edge::All)
            .rounded(14.)
            .background(ColorToken::Base200)
            .border_color(ColorToken::Border)
            .border_width(1.)
            .shadow(tokens::shadow::xl2())
            .transition(Transition::slide(0., -8.).duration_ms(160));
        let dismiss = state;
        let mut root = super::dialogs::overlay_scope(
            card.into(),
            key,
            OverlayKind::Modal,
            Box::new(move |app: &mut C| close(dismiss(app))),
        );
        root.node.flow = Some(Flow::Top);
        root.node.responsive.padding[0] = Some(96f32.into());
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accordion_toggling_in_single_and_multiple_modes() {
        assert_eq!(toggled(&[], 2, false), [2]);
        assert_eq!(toggled(&[2], 1, false), [1]);
        assert_eq!(toggled(&[2], 2, false), Vec::<usize>::new());
        assert_eq!(toggled(&[2], 0, true), [0, 2]);
        assert_eq!(toggled(&[0, 2], 0, true), [2]);
    }
    #[test]
    fn command_scoring_prefers_prefix_then_substring_then_fuzzy() {
        let c = command((), "New file").keywords(["create"]);
        assert_eq!(c.score("new"), Some(0));
        assert_eq!(c.score("file"), Some(1));
        assert_eq!(c.score("create"), Some(2));
        assert_eq!(c.score("nwf"), Some(3));
        assert_eq!(c.score("zzz"), None);
    }
}
