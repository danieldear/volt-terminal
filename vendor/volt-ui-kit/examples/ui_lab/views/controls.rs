use super::{description, heading, specimen};
use crate::lab::state::{ComponentPage, Density, SortOrder};
use crate::lab::App;
use volt_ui_kit::ui::prelude::*;

pub fn view(app: &App) -> Element<App> {
    let density_padding = match app.density {
        Density::Compact => 8.,
        Density::Comfortable => 16.,
        Density::Spacious => 24.,
    };
    let tasks = app
        .tasks
        .iter()
        .map(|&id| {
            hstack![
                text(format!("Test item {id}")).flex(1.),
                button("Remove")
                    .key(format!("task-remove-{id}"))
                    .width(92.)
                    .on_click(move |app: &mut App| {
                        app.tasks.retain(|&item| item != id);
                        app.record(format!("Removed item {id} by stable ID"));
                    }),
            ]
            .key(format!("task-{id}"))
            .gap(10., Axis::Horizontal)
            .align(Align::Center)
            .into()
        })
        .collect::<Vec<_>>();
    vstack![
        description("Real Rust callbacks. Values stay in memory when you switch sections."),
        specimen("Editable text / real Rust state", vec![
            description("Click to edit. Cmd/Ctrl+A, C, V, Z; Shift+arrows select. No idle caret animation."),
            text_field(app.draft.clone()).key("input-draft").placeholder("Name your draft")
                .font_family("SF").background(ColorToken::Base100).border_color(ColorToken::Border).border_width(1.)
                .on_change(|app: &mut App, value| app.draft = value)
                .on_submit(|app: &mut App| { app.submitted += 1; app.record("Submitted editable draft"); }).into(),
            description(format!("Draft: {} / submitted {}", app.draft, app.submitted)),
            text_area(app.notes.clone()).key("input-notes").height(92.).font_family("SF Mono")
                .background(ColorToken::Base100).border_color(ColorToken::Border).border_width(1.)
                .on_change(|app: &mut App, value| app.notes = value).into(),
            text_field(app.password.clone()).key("input-password").password(true).placeholder("Demo password")
                .background(ColorToken::Base100).on_change(|app: &mut App, value| app.password = value).into(),
            text_field("Read-only sample").key("input-readonly").read_only(true).background(ColorToken::Base300).into(),
        ]),
        specimen("Slider, selection popup and modal", vec![
            description(format!("Level: {:.0}% / sorting: {:?}", app.volume, app.sort)),
            slider(app.volume).key("volume-slider").range(0.0..=100.0).step(5.).width(260.)
                .on_change(|app: &mut App, value| app.volume = value).into(),
            select([
                select_item(SortOrder::Recent, "Most recent").key("sort-recent"),
                select_item(SortOrder::Name, "Name").key("sort-name"),
                select_item(SortOrder::Size, "Size (disabled)").key("sort-size").enabled(false),
            ]).key("sort-order").selected(app.sort).open(app.sort_open)
                .on_open_change(|app: &mut App, open| app.sort_open = open)
                .on_select(|app: &mut App, sort| app.sort = sort).into(),
            button("Review draft").key("dialog-open").width(160.)
                .on_click(|app: &mut App| app.dialog_open = true).into(),
            description("Sliders: arrows/Home/End. Popups: Up/Down/Home/End. Dialogs trap Tab; Escape closes."),
        ]),
        specimen("Buttons and state",vec![
            text(format!("Count: {}",app.count)).key("count-value").font_size(32.).font_weight(700).into(),
            button_bar![button("Add one").key("increment").primary(true).color(ColorToken::PrimaryContent).width(112.)
                .enabled(app.enabled).on_click(|app:&mut App|{app.count+=1;app.record("Added one");}),
                button("Clear").key("counter-clear").width(92.).enabled(app.count>0)
                .on_click(|app:&mut App|{app.count=0;app.record("Cleared count");})]
                .gap(10.,Axis::Horizontal).into(),
            toggle("Enable Add one",app.enabled).key("enable-counter")
                .on_change(|app:&mut App,value|{app.enabled=value;app.record(format!("Add one enabled: {value}"));}).into(),
        ]),
        specimen("Toggles and checkboxes",vec![
            toggle("Notifications",app.notifications).key("notifications")
                .on_change(|app:&mut App,value|{app.notifications=value;app.record(format!("Notifications: {value}"));}).into(),
            checkbox("Require approval",app.approved).key("approval")
                .on_change(|app:&mut App,value|{app.approved=value;app.record(format!("Approval: {value}"));}).into(),
            description(format!("Notifications {} / Approval {}",app.notifications,app.approved)),
            checkbox("Disabled checkbox",true).key("disabled-checkbox").enabled(false).on_change(|_:&mut App,_|{}).into(),
        ]),
        specimen("Toolbars, joined actions and typed selection",vec![
            toolbar![
                text("LOCAL DRAFT").font_size(11.).font_weight(600), spacer(),
                button_bar![
                    button("Save").key("bars-save").on_click(|app:&mut App|{app.saved+=1;app.record("Saved from joined toolbar");}),
                    button("Reset").key("bars-reset").on_click(|app:&mut App|{app.saved=0;app.record("Reset toolbar saves");}),
                ].joined(true).rounded(8.).font_size(12.),
            ].key("controls-toolbar").align(Align::Center).into(),
            description(format!("Saved {} times. Joined buttons share outer corners, not handlers.",app.saved)),
            segmented_control([
                (Density::Compact,"Compact"), (Density::Comfortable,"Comfortable"), (Density::Spacious,"Spacious"),
            ]).key("density-selector").selected(app.density).rounded(8.).font_size(12.)
                .on_select(|app:&mut App,value|{app.density=value;app.record(format!("Density: {value:?}"));})
                .height(42.).scroll(Scroll::Horizontal).scrollbars(Scrollbars::Hidden).into(),
            panel![text("Spacing is application state.").font_size(13.)]
                .padding(density_padding,Edge::All).background(ColorToken::Base300).rounded(8.).into(),
            description("Focus an option, then use Left/Right or Home/End. Tab still visits enabled controls."),
        ]),
        specimen("Tabs / active page only",vec![
            toggle("Place the tab bar below its page",app.bottom_tabs).key("tabs-placement")
                .on_change(|app:&mut App,value|{app.bottom_tabs=value;app.record("Changed tab placement");}).into(),
            tabs![
                tab(ComponentPage::Overview,"Overview",
                    vstack![text("A focused view.").font_size(21.).font_weight(600),
                        description("Only this page is mounted. Values survive in App when you leave."),
                        button("Add from overview").key("tabs-overview-add")
                            .on_click(|app:&mut App|{app.count+=1;app.record("Added from the active overview page");})]
                        .gap(10.,Axis::Vertical))
                    .key("tab-overview"),
                tab(ComponentPage::Activity,"Activity",
                    vstack![text(format!("{} actions this session",app.actions)).font_size(21.).font_weight(600),
                        description("The Overview button is not installed on this page."),
                        button("Record activity").key("tabs-activity-record")
                            .on_click(|app:&mut App|app.record("Recorded from the active activity page"))]
                        .gap(10.,Axis::Vertical))
                    .key("tab-activity"),
                tab(ComponentPage::Details,"Later",text("Disabled page")).key("tab-details").enabled(false),
            ].key("controls-tabs").selected(app.component_page)
                .placement(if app.bottom_tabs {TabPlacement::Bottom} else {TabPlacement::Top})
                .on_select(|app:&mut App,value|{app.component_page=value;app.record(format!("Tab: {value:?}"));})
                .gap(14.,Axis::Vertical).font_size(12.).into(),
        ]),
        specimen("Keyed list",vec![
            hstack![button("Reverse").key("tasks-reverse").width(112.).enabled(app.tasks.len()>1)
                .on_click(|app:&mut App|{app.tasks.reverse();app.record("Reversed stable items");}),
                button("Restore").key("tasks-restore").width(92.)
                .on_click(|app:&mut App|{app.tasks=vec![1,2,3];app.record("Restored items");})]
                .gap(10.,Axis::Horizontal).into(),
            if tasks.is_empty() {description("No items. Choose Restore to try again.")} else {vstack(tasks).gap(10.,Axis::Vertical).into()},
        ]),
        heading("Activity"),
        vstack(app.history.iter().take(6).map(|message|description(message.clone())).collect::<Vec<_>>()).gap(8.,Axis::Vertical),
        description("These controls are custom-painted. OS accessibility and mobile hosts are still pending."),
    ].gap(18.,Axis::Vertical).into()
}

/// Mounted after the ordinary shell so a modal covers, rather than relayouts, it.
pub fn overlays(app: &App) -> Element<App> {
    dialog(
        vstack![
            description("Escape or the backdrop cancels. Underlying controls are blocked."),
            text(format!("Draft: {}", app.draft)).font_size(18.),
            button_bar![
                button("Cancel")
                    .key("dialog-cancel")
                    .on_click(|app: &mut App| app.dialog_open = false),
                button("Confirm")
                    .key("dialog-confirm")
                    .primary(true)
                    .color(ColorToken::PrimaryContent)
                    .on_click(|app: &mut App| {
                        app.dialog_open = false;
                        app.submitted += 1;
                        app.record("Confirmed draft in modal");
                    }),
            ]
            .joined(true)
            .rounded(8.),
        ]
        .gap(14., Axis::Vertical),
    )
    .key("draft-dialog")
    .title("Review your draft")
    .open(app.dialog_open)
    .on_dismiss(|app: &mut App| app.dialog_open = false)
    .into()
}
