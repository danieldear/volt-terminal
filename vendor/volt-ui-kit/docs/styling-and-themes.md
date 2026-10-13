# Styling and themes

## Modifiers and inheritance

Every element takes style modifiers. Typography and text colour inherit into
children; surfaces, borders, corners and accents stay on the element that sets
them.

| Area | Modifiers |
| --- | --- |
| Text | `font_family`, `font_size`, `font_weight`, `italic`, `color`, `line_height`, `letter_spacing`, `text_align`, `uppercase`, `underline`, `strikethrough`, `max_lines` (ellipsis), `no_wrap` |
| Surface | `background`, `background_gradient`, `border_color`, `border_width`, `border_style`, `border_sides`, `rounded`, `rounded_on` |
| Effects | `shadow`, `glow`, `opacity`, `hover_style`, `pressed_style`, `focus_style`, `disabled_style` |
| Interaction | `accent_color`, `cursor`, `enabled` |

Colours are `#RRGGBB`, `#RRGGBBAA` (alpha last) or theme roles, below.
Use `rounded(Rounding::Full)` for pills and circles, and
`rounded_on(12., Corners::Left)` or a list of `Corner`s for selected corners.

## Theme colours

Use semantic roles instead of fixed colours and the whole interface follows
the active theme, light or dark:

```rust
use volt_ui_kit::ui::prelude::*;
let card: Element<()> = vstack![
    text("Profile").color(ColorToken::BaseContent),
    text("Saved").color(ColorToken::Success),
    text("Muted note").color(ColorToken::Muted),
]
.background(ColorToken::Base200)
.border_color(ColorToken::Border)
.border_width(1.)
.padding(16., Edge::All)
.into();
card.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

The roles are `Base100`, `Base200`, `Base300` and `BaseContent` (surfaces and
their text); `Primary`, `Secondary`, `Accent` and `Neutral`; the status colours
`Info`, `Success`, `Warning` and `Error`; each with a matching `*Content` role
for text drawn on it; plus `Border` and `Muted`. `ColorToken::Primary.alpha(15)`
is the role at 15% opacity, which keeps tints correct in both modes.

## Custom themes

A `Palette` assigns a colour to every role, and you can define all of them:

```rust,no_run
use volt_ui_kit::{native::Application, ui::prelude::*, ColorToken, Palette, ThemeMode};

#[derive(Default)]
struct App;

fn view(_: &App) -> Element<App> {
    text("Hello").into()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Start from a built-in palette and change any role.
    let mut palette = Palette::dark().with_accent([0.55, 0.36, 0.96, 1.]);
    palette.set(ColorToken::Base100, [0.07, 0.07, 0.09, 1.]);
    palette.set(ColorToken::Success, [0.20, 0.78, 0.45, 1.]);

    Application::new("Themed", App, view)
        .theme(ThemeMode::Dark)
        .palette(palette)
        .run()
}
```

- `with_accent(rgba)` sets the brand roles and picks readable text for them.
- `Palette::to_json()` and `Palette::from_json(text, &base)` save and load a
  palette. Theme files list roles by name (`"base-100"`, `"primary-content"`,
  …) with `#RRGGBB` values; roles a file leaves out keep the base palette.
- `.theme_file("theme.json")` loads a theme file at start and reloads it while
  the app runs whenever the file changes, which suits design iteration.
- `.theme_with(|app, mode| UiTheme::from_palette(..))` derives the theme from
  your own state, for in-app theme switching.
- `contrast_ratio(a, b)` gives the WCAG contrast of two colours; the Catalog
  uses it to grade every text/background pair.

The Catalog's **Theme editor** page (`--page theme-editor`) edits every role
live, checks contrast, and imports and exports theme JSON.

## Typography and fonts

`font_family("system")` resolves to the platform UI font; `"monospace"`,
`"serif"` and `"sans-serif"` pick suitable installed families; any other name
selects an installed font. On macOS, `"SF"` and `"SF Mono"` resolve to the
system San Francisco faces (no Apple fonts are bundled).

To ship your own font, register it before opening a window:

```rust,no_run
let families = volt_ui_kit::native::register_font(std::fs::read("assets/Inter.ttf")?)?;
println!("registered {families:?}");
# Ok::<(), Box<dyn std::error::Error>>(())
```

Up to 64 font files (256 MiB in total) can be registered.

## Effects and interaction states

```rust
use volt_ui_kit::ui::prelude::*;
let save: Element<usize> = button("Save").key("save")
    .border_color("#475569").border_width(1.)
    .shadow(Shadow::new().color("#00000040").blur(8.).offset(0., 2.))
    .hover_style(|s| s.background("#326481").glow(Glow::new().color("#60a5fa80").blur(12.)))
    .pressed_style(|s| s.opacity(0.7))
    .focus_style(|s| s.border_color("#B3E6FF"))
    .animate_on(Interaction::Hover, Animation::ease_out().duration_ms(120))
    .on_click(|count: &mut usize| *count += 1)
    .into();
save.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

- Shadows and glows draw outside the element without changing its layout or
  click area.
- `.opacity(..)` fades a whole subtree as one layer. It does not disable input.
- State styles apply in the order focus, hover, pressed; disabled controls use
  `disabled_style` instead. Focus rings appear only after keyboard navigation.

## Animation and transitions

- `animate_on(Interaction::Hover, Animation::ease_out().duration_ms(120))`
  animates the change into a state style.
- `.animation(..)` animates appearance changes after recomposition, for
  elements with a stable key.
- `.transition(Transition::fade().duration_ms(180))` fades an element in and out
  as it is added and removed; `Transition::slide(dx, dy)` slides it.
- `Ambient::pulse(0.4)` pulses opacity, for skeleton loaders.
- Curves: `linear`, `ease_out`, `ease_in_out` and `Easing::CubicBezier(..)`,
  with optional `delay_ms`.

Colours, borders and opacity interpolate. Animation never calls `view` or
your callbacks, and the app goes back to idle when motion ends.
`Application::reduced_motion(true)` (or `VOLT_UI_REDUCED_MOTION=1`) jumps
straight to final states.

## Images

```rust,no_run
use volt_ui_kit::{native::load_image, ui::prelude::*};
let image = load_image("assets/cover.jpg")?; // load once, outside `view`
let hero: Element<()> = vstack![text("Welcome")]
    .background(ColorToken::Base200)
    .background_image(image.background().fit(ImageFit::Cover).opacity(0.35))
    .rounded(16.)
    .clip_contents(true)
    .into();
hero.build()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Fits are `Cover`, `Contain`, `Stretch` and `Tile`, placed by `ImagePosition`.
Image handles share their pixels, so cloning one into `view` is cheap. PNG and
JPEG files are limited to 16 MiB and 4096 px per side; nothing is ever fetched
from a URL.

## Vector drawing and clipping

```rust
use volt_ui_kit::ui::prelude::*;
let badge: Element<()> = canvas(|c| {
    c.fill(VectorPath::circle(60., 60., 40.), ColorToken::Accent);
    c.stroke(
        VectorPath::new().move_to(10., 100.).cubic_to([35., 10.], [90., 10.], [110., 100.]),
        "#FFFFFF",
        2.,
    );
})
.width(120.)
.height(120.)
.clip(Clip::Circle);
badge.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

- `canvas` records rectangles, circles, ellipses, lines and curves with fills
  and strokes in local logical coordinates. `VectorPath::from_svg(d)` imports
  SVG path data.
- `.clip(Clip::Rounded | Circle | Path(path))` masks an element's contents, and
  clicks follow the same shape. `.clip_contents(true)` clips to the rounded
  corners.
