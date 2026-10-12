//! Controlled plain-text inputs with host-owned cursor/undo/IME state.
use super::*;
use crate::declarative::document::EditorOptions;
pub struct TextInput<C>(pub(super) Element<C>, InputCallbacks<C>);
type TextChange<C> = Box<dyn FnMut(&mut C, String)>;
struct InputCallbacks<C> {
    change: Option<TextChange<C>>,
    submit: Option<Click<C>>,
    leading: Option<Icon>,
    trailing: Option<Trailing<C>>,
}
enum Trailing<C> {
    Icon(Icon),
    Action {
        key: String,
        glyph: Icon,
        label: String,
        click: Click<C>,
    },
}
/// Width of an icon slot inside a field, in logical pixels.
const SLOT: f32 = 26.;
pub fn text_field<C>(value: impl Into<String>) -> TextInput<C> {
    input(value, false)
}
pub fn text_area<C>(value: impl Into<String>) -> TextInput<C> {
    input(value, true)
}
fn input<C>(value: impl Into<String>, multiline: bool) -> TextInput<C> {
    let mut e = Element::new(Kind::Text);
    e.node.text = Some(value.into());
    e.node.height = Some(if multiline { 128. } else { 36. });
    e.node.style.radius = Some(Rounding::Radius(6.));
    e.node.editor = Some(EditorOptions {
        multiline,
        ..Default::default()
    });
    TextInput(
        e,
        InputCallbacks {
            change: None,
            submit: None,
            leading: None,
            trailing: None,
        },
    )
}
impl<C> TextInput<C> {
    pub fn on_change(mut self, f: impl FnMut(&mut C, String) + 'static) -> Self {
        self.1.change = Some(Box::new(f));
        self
    }
    pub fn on_submit(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.1.submit = Some(Box::new(f));
        self
    }
    pub fn placeholder(mut self, value: impl Into<String>) -> Self {
        self.0.node.editor.as_mut().unwrap().placeholder = value.into();
        self
    }
    pub fn read_only(mut self, value: bool) -> Self {
        self.0.node.editor.as_mut().unwrap().read_only = value;
        self
    }
    /// Opt-in visual wrapping for multiline input; stored text is unchanged.
    pub fn soft_wrap(mut self, value: bool) -> Self {
        self.0.node.editor.as_mut().unwrap().soft_wrap = value;
        self
    }
    /// Multiline only: Enter submits and Shift+Enter inserts a newline, as
    /// chat composers do. Cmd/Ctrl+Enter submits either way.
    pub fn submit_on_enter(mut self, value: bool) -> Self {
        self.0.node.editor.as_mut().unwrap().enter_submits = value;
        self
    }
    /// A decorative icon inside the field's leading edge (search, mail…).
    pub fn leading_icon(mut self, glyph: Icon) -> Self {
        self.1.leading = Some(glyph);
        self
    }
    /// A decorative icon inside the trailing edge.
    pub fn trailing_icon(mut self, glyph: Icon) -> Self {
        self.1.trailing = Some(Trailing::Icon(glyph));
        self
    }
    /// A small icon button inside the trailing edge: clear, reveal password…
    pub fn trailing_action(
        mut self,
        key: impl Into<String>,
        glyph: Icon,
        label: impl Into<String>,
        click: impl FnMut(&mut C) + 'static,
    ) -> Self {
        self.1.trailing = Some(Trailing::Action {
            key: key.into(),
            glyph,
            label: label.into(),
            click: Box::new(click),
        });
        self
    }
    pub fn password(mut self, value: bool) -> Self {
        self.0.node.editor.as_mut().unwrap().password = value;
        self
    }
    pub fn enabled(mut self, value: bool) -> Self {
        self.0.node.enabled = value;
        self
    }
}
impl<C: 'static> From<TextInput<C>> for Element<C> {
    fn from(mut input: TextInput<C>) -> Self {
        input.0.callback = Some(Callback::Text {
            change: input.1.change,
            submit: input.1.submit,
        });
        let (leading, trailing) = (input.1.leading, input.1.trailing);
        if leading.is_none() && trailing.is_none() {
            return input.0;
        }
        // Make room for the slots, then layer the icons over the field.
        if let Some(editor) = input.0.node.editor.as_mut() {
            editor.inset = [
                if leading.is_some() { SLOT } else { 0. },
                if trailing.is_some() { SLOT } else { 0. },
            ];
        }
        let key = input.0.node.id.clone();
        let height = input.0.node.height;
        // The layer takes the field's size so the slots sit inside it.
        let width = input.0.node.width.take();
        let flex = std::mem::take(&mut input.0.node.flex);
        let mut overlay: Vec<Element<C>> = vec![];
        if let Some(glyph) = leading {
            overlay.push(icon(glyph).size(16.).color(ColorToken::Muted).into());
        }
        overlay.push(spacer().into());
        match trailing {
            Some(Trailing::Icon(glyph)) => {
                overlay.push(icon(glyph).size(16.).color(ColorToken::Muted).into())
            }
            Some(Trailing::Action {
                key: action,
                glyph,
                label,
                click,
            }) => {
                let mut b = button("")
                    .key(action.clone())
                    .accessible_label(label)
                    .variant(ButtonVariant::Ghost)
                    .width(24.)
                    .height(24.);
                b.0.callback = Some(Callback::Click(click));
                overlay.push(
                    zstack![b, icon(glyph).size(14.).color(ColorToken::Muted)]
                        .key(format!("{action}-layer"))
                        .width(24.)
                        .height(24.)
                        .flow(Flow::Center)
                        .into(),
                );
            }
            None => {}
        }
        let mut layer = zstack![
            input.0,
            hstack(overlay)
                .key(format!("{key}-slots"))
                .align(Align::Center)
                .padding(10., Edge::Left)
                .padding(6., Edge::Right)
                .scroll_passthrough(true),
        ]
        .key(format!("{key}-adorned"))
        .align(Align::Stretch);
        if let Some(h) = height {
            layer = layer.height(h);
        }
        if let Some(w) = width {
            layer = layer.width(w);
        }
        layer.0.node.flex = flex;
        layer.into()
    }
}
