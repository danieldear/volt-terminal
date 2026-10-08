//! Generate a terminal-free component gallery using the real toolkit paint recipes.
use std::{error::Error, path::PathBuf};
use volt_ui_kit::{
    card_paint::CardPaint,
    form_paint::FormPaint,
    search_paint::SearchPaint,
    search_palette::{SearchLayout, SearchRow, SearchView},
    settings::{SettingsLayout, SettingsRow, SettingsView},
    settings_paint::SettingsPaint,
    strip_paint::StripPaint,
    svg::SvgPainter,
    task_form::{FormField, TaskFormLayout, TaskFormView},
    task_strip::{StripState, StripTask, TaskStripView},
    workspace_card::{CardIcon, CardRow, CardTone, WorkspaceCard},
    Button, ButtonColors, Colors, Painter, Panel, Rect, SurfaceStyle, Theme, Tint, Viewport,
};
fn main() -> Result<(), Box<dyn Error>> {
    let mut output = PathBuf::from("target/gallery");
    let mut scale = 1.;
    let mut light = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--output" => output = args.next().ok_or("missing output path")?.into(),
            "--scale" => scale = args.next().ok_or("missing scale")?.parse::<f32>()?,
            "--light" => light = true,
            _ => return Err(format!("unknown argument: {a}").into()),
        }
    }
    let v = Viewport::new(960. * scale, 700. * scale, scale)
        .ok_or("scale must be finite and positive")?;
    std::fs::create_dir_all(&output)?;
    let theme = if light { Theme::light() } else { Theme::dark() };
    let colors = Colors::from_theme(&theme);
    let mut settings = SettingsView {
        title: "Settings".into(),
        subtitle: "Preview changes. Apply when ready.".into(),
        sections: vec!["Appearance".into(), "Shortcuts".into(), "Workspace".into()],
        section: 0,
        focus: 0,
        status: String::new(),
        onboarding: false,
        preview: false,
        rows: vec![
            SettingsRow {
                label: "Theme".into(),
                value: "Amber / dark".into(),
                hint: "Make this space yours".into(),
                editing: false,
                adjustable: true,
                actionable: true,
                caret: None,
                selected: false,
            },
            SettingsRow {
                label: "Font family".into(),
                value: "SF Mono".into(),
                hint: String::new(),
                editing: false,
                adjustable: true,
                actionable: true,
                caret: None,
                selected: false,
            },
        ],
    };
    settings.rows.extend(
        [
            ("Font size", "14"),
            ("Line height", "1.2"),
            ("Padding", "12"),
            ("Opacity", "0.95"),
        ]
        .into_iter()
        .map(|(label, value)| SettingsRow {
            label: label.into(),
            value: value.into(),
            hint: String::new(),
            editing: false,
            adjustable: true,
            actionable: true,
            caret: None,
            selected: false,
        }),
    );
    if light {
        settings.rows[0].value = "Paper / light".into();
    }
    let strip = TaskStripView {
        tasks: vec![
            StripTask {
                name: "Build".into(),
                state: StripState::Succeeded,
            },
            StripTask {
                name: "Test".into(),
                state: StripState::Running,
            },
            StripTask {
                name: "Run".into(),
                state: StripState::Idle,
            },
            StripTask {
                name: "Deploy".into(),
                state: StripState::Failed,
            },
        ],
        more: false,
        message: None,
    };
    let card = WorkspaceCard {
        title: "Workspace".into(),
        subtitle: "Rust project".into(),
        status: "Changed".into(),
        tone: CardTone::Amber,
        rows: vec![
            CardRow {
                label: "Changes".into(),
                detail: String::new(),
                icon: CardIcon::Branch,
                action: Some(1),
                expanded: false,
                section: true,
                tone: CardTone::Purple,
                diff: Some((128, 14)),
            },
            CardRow {
                label: "Tasks".into(),
                detail: "4".into(),
                icon: CardIcon::Play,
                action: Some(2),
                expanded: true,
                section: true,
                tone: CardTone::Blue,
                diff: None,
            },
            CardRow {
                label: "Build".into(),
                detail: "Done".into(),
                icon: CardIcon::Check,
                action: Some(3),
                expanded: false,
                section: false,
                tone: CardTone::Green,
                diff: None,
            },
        ],
        ..Default::default()
    };
    let form = TaskFormView {
        title: "Add task".into(),
        project: ".app/tasks.toml".into(),
        fields: [
            ("Name", "Build"),
            ("Command", "cargo build --release"),
            ("Folder", ""),
        ]
        .into_iter()
        .map(|(l, v)| FormField {
            label: l.into(),
            value: v.into(),
            placeholder: "Project folder".into(),
            cursor: None,
        })
        .collect(),
        confirm: true,
        focus: 0,
        status: None,
    };
    let search = SearchView {
        query: "widget".into(),
        query_selected: false,
        cursor: 6,
        scope: 0,
        selected: 0,
        root: "UI toolkit".into(),
        status: "2 results".into(),
        hint: "Enter opens".into(),
        rows: vec![
            SearchRow {
                kind: "File".into(),
                title: "src/widgets.rs".into(),
                detail: "Reusable drawing recipes".into(),
                hits: vec![(4, 10)],
            },
            SearchRow {
                kind: "Symbol".into(),
                title: "WidgetPaint".into(),
                detail: "Render into your application".into(),
                hits: vec![(0, 6)],
            },
        ],
    };
    let names = [
        "card",
        "surfaces",
        "settings",
        "onboarding",
        "buttons",
        "form",
        "search",
        "icons",
    ];
    for name in names {
        let mut painter = SvgPainter::new(v).unwrap();
        let (mut b, mut g) = (vec![], vec![]);
        match name {
            "card" => painter.build_workspace_card_with_footer(
                &mut b,
                &mut g,
                &card,
                &theme,
                0.,
                Some("Reusable card / host-owned actions"),
            ),
            "settings" | "onboarding" => {
                let mut view = settings.clone();
                view.onboarding = name == "onboarding";
                if view.onboarding {
                    view.title = "Make it yours".into();
                    view.subtitle = "Theme, typography and shortcuts".into();
                }
                let layout = SettingsLayout::new(v.width, v.height, 0., v.scale).unwrap();
                painter.build_settings(&mut b, &mut g, &view, &layout, &theme);
            }
            "buttons" => painter.build_task_strip(&mut b, &mut g, &strip, &theme),
            "form" => {
                let l = TaskFormLayout::new(v.width, v.height, 0., v.scale).unwrap();
                painter.build_task_form(
                    &mut b,
                    &mut g,
                    &form,
                    &l,
                    &colors,
                    (theme.ansi[1], theme.ansi[2]),
                );
            }
            "search" => {
                let l =
                    SearchLayout::new(v.width, v.height, 0., v.scale, search.rows.len()).unwrap();
                painter.build_search_palette(&mut b, &mut g, &search, &l, &colors);
            }
            "surfaces" => {
                for (x, tint, title) in [
                    (40., Tint::Dark, "Smoked surface"),
                    (500., Tint::Light, "Frosted surface"),
                ] {
                    let style = SurfaceStyle::glass(tint);
                    let rect =
                        Rect::new(x * scale, 50. * scale, 400. * scale, 300. * scale).unwrap();
                    Panel::new(rect, 14., scale, style)
                        .unwrap()
                        .paint(&painter, &mut b);
                    painter.card_icon(
                        &mut b,
                        CardIcon::Folder,
                        (x + 24.) * scale,
                        76. * scale,
                        scale,
                        style.text,
                    );
                    painter.card_text(
                        &mut g,
                        title,
                        (x + 52.) * scale,
                        75. * scale,
                        16. * scale,
                        style.text,
                    );
                    painter.card_text(
                        &mut g,
                        "Opaque fallback · no GPU blur",
                        (x + 24.) * scale,
                        120. * scale,
                        12. * scale,
                        style.muted,
                    );
                    painter.draw_rect(
                        &mut b,
                        (x + 24.) * scale,
                        161. * scale,
                        352. * scale,
                        scale,
                        style.border,
                    );
                    painter.card_text(
                        &mut g,
                        "Rounded frames. Adaptive tint.",
                        (x + 24.) * scale,
                        190. * scale,
                        13. * scale,
                        style.text,
                    );
                    painter.card_text(
                        &mut g,
                        "Your renderer. Your application.",
                        (x + 24.) * scale,
                        220. * scale,
                        13. * scale,
                        style.muted,
                    );
                    Button {
                        label: "Explore",
                        icon: Some(CardIcon::ArrowRight),
                        enabled: true,
                    }
                    .paint(
                        &mut painter,
                        &mut b,
                        &mut g,
                        [(x + 24.) * scale, 282. * scale, 140. * scale, 36. * scale],
                        scale,
                        ButtonColors {
                            surface: colors.accent,
                            text: theme.background,
                        },
                    );
                }
            }
            "icons" => {
                for (i, icon) in [
                    CardIcon::Folder,
                    CardIcon::Branch,
                    CardIcon::File,
                    CardIcon::Play,
                    CardIcon::Agent,
                    CardIcon::Link,
                    CardIcon::Server,
                    CardIcon::Close,
                    CardIcon::Check,
                    CardIcon::ArrowLeft,
                    CardIcon::ArrowRight,
                    CardIcon::Refresh,
                    CardIcon::Minimize,
                    CardIcon::Pin,
                    CardIcon::Unpin,
                    CardIcon::Expand,
                    CardIcon::Search,
                    CardIcon::Plus,
                    CardIcon::Edit,
                ]
                .into_iter()
                .enumerate()
                {
                    let x = (40. + (i % 5) as f32 * 160.) * scale;
                    let y = (60. + (i / 5) as f32 * 120.) * scale;
                    painter.card_icon(&mut b, icon, x, y, 32. * scale, colors.accent);
                    painter.card_text(
                        &mut g,
                        &format!("{icon:?}"),
                        x,
                        y + 46. * scale,
                        13. * scale,
                        colors.text,
                    );
                }
            }
            _ => unreachable!(),
        }
        let margin = 20. * scale;
        let bounds = match name {
            "settings" | "onboarding" => {
                let l = SettingsLayout::new(v.width, v.height, 0., scale).unwrap();
                [l.x, l.y, l.w, l.h]
            }
            "card" => {
                let l = volt_ui_kit::workspace_card::CardLayout::for_card(
                    v.width, v.height, 0., scale, &card,
                )
                .unwrap();
                [l.x, l.y, l.w, l.h]
            }
            "form" => {
                let l = TaskFormLayout::new(v.width, v.height, 0., scale).unwrap();
                [l.x, l.y, l.w, l.h]
            }
            "search" => {
                let l = SearchLayout::new(v.width, v.height, 0., scale, search.rows.len()).unwrap();
                [l.x, l.y, l.w, l.h]
            }
            "surfaces" => [40. * scale, 50. * scale, 860. * scale, 300. * scale],
            "buttons" => {
                let l = volt_ui_kit::task_strip::TaskStripLayout::new(&strip, v.width, 0., scale)
                    .unwrap();
                let first = l.buttons[0];
                [first[0], first[1], l.add[0] + l.add[2] - first[0], first[3]]
            }
            _ => [20. * scale, 40. * scale, 820. * scale, 480. * scale],
        };
        let bounds = [
            bounds[0] - margin,
            bounds[1] - margin,
            bounds[2] + 2. * margin,
            bounds[3] + 2. * margin,
        ];
        std::fs::write(
            output.join(format!("{name}.svg")),
            painter
                .document_region(&b, &g, theme.background, bounds)
                .unwrap(),
        )?;
    }
    let mut html = String::from(
        r#"<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Volt UI Kit — component gallery</title><style>body{margin:0;background:#171a1a;color:#e6d9b8;font:15px SFMono-Regular,Consolas,monospace}header,nav{padding:20px 28px}h1{font-size:24px;margin:0 0 10px}p{color:#a7a599;line-height:1.6}nav{display:flex;gap:10px;flex-wrap:wrap}a{color:inherit;text-decoration:none;border:1px solid #62533b;padding:10px 14px;border-radius:9px}a:hover,a:focus-visible{background:#3b3427;outline:2px solid #ddb366}img{width:auto;max-width:100%;height:auto;display:block;margin:auto}section{padding:24px 28px;border-top:1px solid #333832}h2{font-size:16px;font-weight:normal}</style><header><h1>Volt UI Kit</h1><p>The same paint recipes and vector icons as Volt. A renderer-independent Rust toolkit.<br>These are static SVG previews; native text shaping and application behavior remain host responsibilities.</p></header><nav>"#,
    );
    for name in names {
        html.push_str(&format!("<a href=\"#{name}\">{name}</a>"));
    }
    html.push_str("</nav>");
    for name in names {
        html.push_str(&format!("<section id=\"{name}\"><h2>{name}</h2><img src=\"{name}.svg\" alt=\"{name} component preview\"></section>"));
    }
    html.push_str("</html>");
    std::fs::write(output.join("index.html"), html)?;
    println!("Gallery: {}", output.join("index.html").display());
    Ok(())
}
