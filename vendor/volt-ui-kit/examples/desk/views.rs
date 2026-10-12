use super::{
    state::{Role, PROMPTS},
    App,
};
use volt_ui_kit::ui::{prelude::*, Button, Text};
fn note(value: impl Into<String>) -> Text<App> {
    text(value).font_size(11.).color(ColorToken::Muted)
}
fn mono(value: impl Into<String>) -> Text<App> {
    note(value).font_family("SF Mono")
}
fn control(
    label: impl Into<String>,
    key: impl Into<String>,
    active: bool,
    callback: impl FnMut(&mut App) + 'static,
) -> Button<App> {
    button(label)
        .key(key)
        .height(34.)
        .font_size(12.)
        .background(if active {
            ColorToken::Base300
        } else {
            ColorToken::Base200
        })
        .color(ColorToken::BaseContent)
        .border_width(0.)
        .rounded(8.)
        .hover_style(|s| s.background(ColorToken::Base300))
        .pressed_style(|s| s.opacity(0.7))
        .animate_on(Interaction::Hover, Animation::ease_out().duration_ms(100))
        .on_click(callback)
}
fn sidebar(app: &App) -> Element<App> {
    let threads = app
        .threads
        .iter()
        .map(|t| {
            let id = t.id;
            control(
                t.title.clone(),
                format!("desk-thread-{id}"),
                app.active == id,
                move |app| app.select(id),
            )
            .height(48.)
            .font_size(12.)
            .into()
        })
        .collect::<Vec<Element<App>>>();
    vstack![
        hstack![text("Volt Desk").font_size(16.).font_weight(600),spacer(),mono("LOCAL")].align(Align::Center),
        control("+  New conversation","desk-new",false,App::new_chat).height(40.),
        note("Conversations").font_weight(600),vstack(threads).gap(5.,Axis::Vertical),
        spacer().height(18.),note("Project").font_weight(600),
        panel![text("volt-ui-kit").font_family("SF Mono").font_size(12.).font_weight(600),note("Native desktop experiments"),
            badge("Rust / renderer-independent").font_size(9.).background(ColorToken::Base300).color(ColorToken::BaseContent).rounded(6.)]
            .padding(12.,Edge::All).gap(10.,Axis::Vertical).background(ColorToken::Base300).rounded(10.),
        spacer().height(18.),note("Try in this workspace").font_weight(600),
        note("Switch conversations\nEdit and send a local draft\nToggle the details panel\nCompare light and dark\nTab through native controls"),
        spacer(),
        hstack![panel![text("V").font_size(11.).font_weight(700).color(ColorToken::PrimaryContent)].width(28.).height(28.)
            .flow(Flow::Center).background(ColorToken::Primary).rounded(8.),vstack![text("Local workspace").font_size(12.),note("No account required")].flex(1.)].gap(10.,Axis::Horizontal).align(Align::Center),
    ].key("desk-sidebar").width(224.).padding(16.,Edge::All).gap(16.,Axis::Vertical)
        .scroll(Scroll::Auto).background(ColorToken::Base200).into()
}
fn inspector(app: &App) -> Element<App> {
    vstack![
        hstack![text("Workspace details").font_size(13.).font_weight(600),spacer(),
            control("×","desk-close-inspector",false,|app|{app.inspector=false;app.status="Details panel hidden".into();}).width(28.)],
        divider().background(ColorToken::Border),
        note("Session").font_weight(600),
        hstack![mono("Messages"),spacer(),mono(app.current().messages.len().to_string())],
        hstack![mono("Local sends"),spacer(),mono(app.sent.to_string())],
        hstack![mono("Runtime"),spacer(),mono("Native Rust")],
        divider().background(ColorToken::Border),
        note("Component sources").font_weight(600),
        mono("src/ui.rs\nsrc/declarative/runtime.rs\nsrc/native/raster.rs\nexamples/desk/views.rs"),
        divider().background(ColorToken::Border),
        note("Interface layers").font_weight(600),
        panel![badge("Layout").font_size(10.).background(ColorToken::Base300),note("Stacks and independently scrolling panels")]
            .padding(12.,Edge::All).gap(8.,Axis::Vertical).background(ColorToken::Base300).rounded(10.),
        panel![badge("Typography").font_size(10.).background(ColorToken::Base300),note("SF interface / SF Mono metadata")]
            .padding(12.,Edge::All).gap(8.,Axis::Vertical).background(ColorToken::Base300).rounded(10.),
        panel![badge("Interaction").font_size(10.).background(ColorToken::Base300),note("Stable IDs and typed Rust callbacks")]
            .padding(12.,Edge::All).gap(8.,Axis::Vertical).background(ColorToken::Base300).rounded(10.),
        control(if app.saved {"Sample saved ✓"} else {"Save sample in memory"},"desk-save",app.saved,|app|{app.saved= !app.saved;app.status="Changed local saved flag; no file written".into();}),
        note("This is a component demonstration, not an AI client. No files are read by these controls; paths above are labels, not links."),
    ].key("desk-inspector").width(248.).padding(18.,Edge::All).gap(18.,Axis::Vertical)
        .scroll(Scroll::Auto).background(ColorToken::Base200).into()
}
fn conversation(app: &App) -> Element<App> {
    let mut messages = Vec::new();
    if app.current().messages.is_empty() {
        messages.push(
            vstack![
                text("A clean slate.").font_size(30.).font_weight(600),
                note("Choose a sample prompt below to start a local conversation.")
            ]
            .gap(12., Axis::Vertical)
            .padding(32., Edge::Vertical)
            .into(),
        );
    }
    for m in &app.current().messages {
        let body = text(m.body.clone()).font_size(14.);
        let item: Element<App> = if m.role == Role::User {
            vstack![mono("You"), body]
                .padding(18., Edge::All)
                .gap(10., Axis::Vertical)
                .background(ColorToken::Base200)
                .rounded(16.)
                .into()
        } else {
            vstack![
                hstack![
                    badge("V")
                        .font_size(10.)
                        .background(ColorToken::Primary)
                        .color(ColorToken::PrimaryContent)
                        .rounded(6.),
                    text("Volt").font_size(12.).font_weight(600),
                    mono("Local sample")
                ]
                .gap(10., Axis::Horizontal)
                .align(Align::Center),
                body
            ]
            .gap(14., Axis::Vertical)
            .padding(8., Edge::Vertical)
            .into()
        };
        messages.push(
            vstack![item]
                .key(format!("desk-message-{}", m.id))
                .padding(if m.role == Role::User { 40. } else { 0. }, Edge::Left)
                .into(),
        );
    }
    if app.show_code {
        messages.push(panel![hstack![mono("Rust composition"),spacer(),
            control("Hide code","desk-hide-code",false,|app|{app.show_code=false;app.status="Code sample collapsed".into();}).width(92.)],
            text("vstack![\n    text(\"A focused workspace\"),\n    button(\"Send sample\")\n        .on_click(App::send),\n]\n.padding(24., Edge::All)")
                .font_family("SF Mono").font_size(12.).color(ColorToken::Primary)]
            .key("desk-code").padding(18.,Edge::All).gap(14.,Axis::Vertical).background(ColorToken::Base200).border_width(1.)
            .border_color(ColorToken::Border).rounded(12.).into());
    } else {
        messages.push(
            control("Show Rust composition", "desk-show-code", false, |app| {
                app.show_code = true;
                app.status = "Code sample expanded".into();
            })
            .into(),
        );
    }
    messages.push(note("All replies are fixed local demo text. There is no model, tool execution or external service connected.").into());
    vstack(messages)
        .key(format!("desk-conversation-{}", app.active))
        .flex(1.)
        .padding(responsive(18.).at(Breakpoint::Lg, 36.), Edge::All)
        .gap(24., Axis::Vertical)
        .scroll(Scroll::Vertical)
        .into()
}
pub fn view(app: &App) -> Element<App> {
    let composer = panel![
        mono("COMPOSER / editable local draft"),
        text_area(app.draft.clone())
            .key("desk-prompt")
            .height(72.)
            .font_size(14.)
            .font_family("SF")
            .background(ColorToken::Base100)
            .placeholder("Write a message…")
            .on_change(|app: &mut App, value| app.draft = value)
            .on_submit(App::send),
        hstack![
            control("Next prompt", "desk-prompt-next", false, |app| {
                app.prompt = (app.prompt + 1) % PROMPTS.len();
                app.draft = PROMPTS[app.prompt].into();
                app.status = "Loaded next sample into the editable draft".into();
            })
            .width(112.),
            spacer(),
            control("Send ↑", "desk-send", true, App::send)
                .enabled(!app.draft.trim().is_empty())
                .width(132.)
                .background(ColorToken::Primary)
                .color(ColorToken::PrimaryContent)
        ]
        .gap(8., Axis::Horizontal)
        .align(Align::Center)
    ]
    .key("desk-composer")
    .padding(16., Edge::All)
    .gap(14., Axis::Vertical)
    .background(ColorToken::Base200)
    .rounded(18.)
    .border_color(ColorToken::Border)
    .border_width(1.);
    let center = vstack![
        hstack![
            text(app.current().title.clone())
                .font_size(13.)
                .font_weight(600)
                .flex(1.),
            badge("Local demo")
                .font_size(10.)
                .background(ColorToken::Base300)
                .color(ColorToken::BaseContent)
                .rounded(6.)
        ]
        .padding(18., Edge::Horizontal)
        .height(44.)
        .align(Align::Center),
        conversation(app),
        vstack![composer]
            .padding(18., Edge::Horizontal)
            .padding(12., Edge::Bottom),
    ]
    .key("desk-center")
    .flex(1.);
    let center = vstack![center]
        .padding(if app.sidebar { 225. } else { 0. }, Edge::Left)
        .padding(if app.inspector { 249. } else { 0. }, Edge::Right);
    let mut sides = Vec::new();
    if app.sidebar {
        sides.push(sidebar(app));
    }
    sides.push(spacer().scroll_passthrough(true).into());
    if app.inspector {
        sides.push(inspector(app));
    }
    let panes = zstack![
        center,
        hstack(sides).align(Align::Stretch).scroll_passthrough(true)
    ]
    .key("desk-panes")
    .flex(1.)
    .align(Align::Stretch);
    vstack![
        hstack![
            control("≡", "desk-sidebar-toggle", false, |app| {
                app.sidebar = !app.sidebar;
                app.status = "Toggled sidebar".into();
            })
            .width(32.),
            mono("Desk").font_weight(600).width(40.),
            spacer(),
            control(
                if app.theme == ThemeMode::Light {
                    "Dark"
                } else {
                    "Light"
                },
                "desk-theme",
                false,
                |app| {
                    app.theme = if app.theme == ThemeMode::Light {
                        ThemeMode::Dark
                    } else {
                        ThemeMode::Light
                    };
                    app.status = "Changed workspace appearance".into();
                }
            )
            .width(64.),
            control("Details", "desk-inspector-toggle", false, |app| {
                app.inspector = !app.inspector;
                app.status = "Toggled details panel".into();
            })
            .width(72.)
        ]
        .key("desk-titlebar")
        .height(48.)
        .padding(80., Edge::Left)
        .padding(12., Edge::Right)
        .gap(6., Axis::Horizontal)
        .align(Align::Center)
        .drag_region(true)
        .background(ColorToken::Base200),
        panes,
        hstack![mono(&app.status).key("desk-status")]
            .height(26.)
            .padding(12., Edge::Horizontal)
            .align(Align::Center)
            .clip_contents(true),
    ]
    .key("desk-root")
    .font_family("SF")
    .font_size(13.)
    .color(ColorToken::BaseContent)
    .background(ColorToken::Base100)
    .into()
}
