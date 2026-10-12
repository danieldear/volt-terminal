use super::*;
use std::sync::atomic::AtomicBool;
use volt_ui_kit::{
    declarative::{Bindings, Runtime},
    native::RasterPainter,
    Viewport,
};

/// Building validates keys, callbacks and the node/depth budgets.
fn mount(app: &App, size: [f32; 2]) -> Runtime {
    let (document, handlers) = view(app)
        .build()
        .unwrap_or_else(|e| panic!("thread {}: {e}", app.active))
        .into_parts();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    let viewport = Viewport::new(size[0], size[1], 1.).unwrap();
    runtime
        .layout(
            viewport,
            &Bindings::default(),
            &mut RasterPainter::new(viewport),
        )
        .unwrap();
    runtime
}

/// Drive a run to completion the way the worker would.
fn finish(app: &mut App, text: &str, files: usize) {
    let run = app.run.as_ref().unwrap().id;
    let mut send = |kind| update(app, Event { run, kind });
    send(EventKind::Step("Read README.md".into()));
    send(EventKind::StepDone("Overview".into()));
    for i in 0..files {
        send(EventKind::File(FileChange {
            path: format!("src/{i}.rs"),
            added: 3,
            removed: 1,
        }));
    }
    for word in text.split_inclusive(' ') {
        send(EventKind::Text(word.into()));
    }
    send(EventKind::Done(7));
}

#[test]
fn every_state_builds_and_lays_out_in_both_modes() {
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        for glass in [false, true] {
            let mut app = App::new();
            app.mode = mode;
            app.glass = glass;
            for id in app.threads.iter().map(|t| t.id).collect::<Vec<_>>() {
                app.open(id);
                mount(&app, [1320., 860.]);
                mount(&app, [900., 640.]);
            }
            app.threads[0].messages[1].expanded = true;
            app.search_open = true;
            app.query = "tab".into();
            app.settings_open = true;
            mount(&app, [1320., 860.]);
            app.settings_open = false;
            app.thread_menu_open = true;
            app.model_open = true;
            mount(&app, [1320., 860.]);
            app.sidebar = false;
            app.inspector = false;
            app.new_chat();
            mount(&app, [1320., 860.]);
            app.send(Some("fix the border bug".into()));
            mount(&app, [1320., 860.]);
        }
    }
}

#[test]
fn a_streamed_run_fills_the_reply_and_marks_other_threads_unread() {
    let mut app = App::new();
    app.open(2);
    app.send(Some("run the tests".into()));
    assert!(app.running());
    // The thread moves to the top of Recents while it runs.
    assert_eq!(app.threads[0].id, 2);
    app.open(1);
    finish(&mut app, "All green. ```code``` done", 2);
    assert!(app.run.is_none());
    let t = app.threads.iter().find(|t| t.id == 2).unwrap();
    let reply = t.messages.last().unwrap();
    assert_eq!(reply.text, "All green. ```code``` done");
    assert_eq!(reply.worked, Some(7));
    assert_eq!(reply.files.len(), 2);
    assert!(reply.steps.iter().all(|s| s.done));
    assert!(!reply.streaming);
    assert!(t.unread, "finished in the background");
    assert_eq!(t.changes(), (6, 2));
    app.open(2);
    assert!(!app.thread().unread);
}

#[test]
fn stale_events_stop_and_retry_are_safe() {
    let mut app = App::new();
    app.send(Some("hello".into()));
    let stale = app.run.as_ref().unwrap().id;
    app.stop();
    assert!(app.run.is_none());
    let reply = app.thread().messages.last().unwrap().clone();
    assert_eq!(reply.text, "Stopped.");
    assert!(!reply.streaming);
    // Events from the cancelled run are ignored.
    update(
        &mut app,
        Event {
            run: stale,
            kind: EventKind::Text("late".into()),
        },
    );
    assert_eq!(app.thread().messages.last().unwrap().text, "Stopped.");
    // Retry replaces the last reply rather than appending another.
    let count = app.thread().messages.len();
    app.retry();
    assert_eq!(app.thread().messages.len(), count);
    assert!(app.thread().messages.last().unwrap().streaming);
    // Only one run at a time; a second send is refused.
    app.send(Some("again".into()));
    assert_eq!(app.thread().messages.len(), count);
    finish(&mut app, "ok", 0);
    assert_eq!(app.thread().messages.last().unwrap().text, "ok");
}

#[test]
fn deleting_a_running_thread_cancels_it() {
    let mut app = App::new();
    app.send(Some("hello".into()));
    let cancel = app.run.as_ref().unwrap().cancel.clone();
    app.delete_thread();
    assert!(cancel.load(Ordering::Relaxed));
    assert!(app.run.is_none());
    while app.threads.len() > 1 {
        app.delete_thread();
    }
    app.delete_thread();
    assert_eq!(app.threads.len(), 1, "the last thread is kept");
}

#[test]
fn new_chat_reuses_an_empty_draft_and_titles_from_the_first_prompt() {
    let mut app = App::new();
    app.new_chat();
    let (id, count) = (app.active, app.threads.len());
    app.open(1);
    app.new_chat();
    assert_eq!((app.active, app.threads.len()), (id, count));
    app.send(Some(
        "explain how the damage tracking in the GPU renderer works".into(),
    ));
    assert_eq!(app.thread().title, "Explain how the damage tracking in…");
    assert!(app.draft.is_empty());
}

#[test]
fn delivery_waits_out_backpressure_but_not_cancellation() {
    let cancel = AtomicBool::new(false);
    let mut attempts = 0;
    assert!(deliver(&cancel, || {
        attempts += 1;
        if attempts < 3 {
            Err(UiSendError::Full)
        } else {
            Ok(())
        }
    }));
    assert_eq!(attempts, 3);
    assert!(!deliver(&cancel, || Err(UiSendError::Closed)));
    assert!(!deliver(&cancel, || {
        cancel.store(true, Ordering::Relaxed);
        Err(UiSendError::Full)
    }));
}

#[test]
fn long_sessions_stay_inside_the_demo_document_budget() {
    let mut app = App::new();
    // Repeated sends used to grow the rendered transcript without bound,
    // eventually exceeding the toolkit's validated 4096-node limit.
    for _ in 0..100 {
        app.send(Some("another question".into()));
        finish(&mut app, "paragraph\n\n".repeat(100).as_str(), 8);
    }
    assert!(app.thread().messages.len() <= 16);
    for message in &mut app.threads[0].messages {
        message.expanded = true;
    }
    mount(&app, [1320., 860.]);
    for _ in 0..100 {
        app.new_chat();
        app.send(Some("new thread".into()));
        finish(&mut app, "done", 0);
    }
    assert!(app.threads.len() <= 16);
    mount(&app, [900., 640.]);
}

#[test]
fn oversized_prompts_and_worker_output_are_bounded_without_utf8_panics() {
    let mut app = App::new();
    app.draft = "é".repeat(MAX_PROMPT_BYTES);
    let original = app.draft.clone();
    app.send(None);
    assert!(app.run.is_none());
    assert_eq!(app.draft, original, "rejected draft is not discarded");
    app.send(Some("small prompt".into()));
    let run = app.run.as_ref().unwrap().id;
    for i in 0..100 {
        update(
            &mut app,
            Event {
                run,
                kind: EventKind::Text("🙂".repeat(1000)),
            },
        );
        update(
            &mut app,
            Event {
                run,
                kind: EventKind::Step("work".into()),
            },
        );
        update(
            &mut app,
            Event {
                run,
                kind: EventKind::File(FileChange {
                    path: format!("{i}.rs"),
                    added: u32::MAX,
                    removed: u32::MAX,
                }),
            },
        );
    }
    let reply = app.thread().messages.last().unwrap();
    assert!(reply.text.len() <= MAX_REPLY_BYTES);
    assert_eq!(reply.steps.len(), MAX_STEPS);
    assert_eq!(reply.files.len(), MAX_FILES);
    assert_eq!(app.thread().changes(), (u32::MAX, u32::MAX));
    mount(&app, [1320., 860.]);
}

#[test]
fn launch_options() {
    let args = |items: &[&str]| crate::parse_args(items.iter().map(|s| s.to_string()));
    let (app, options) = args(&["--thread", "3", "--theme", "light", "--no-inspector"]).unwrap();
    assert_eq!(app.active, 3);
    assert_eq!(app.mode, ThemeMode::Light);
    assert!(!app.inspector);
    assert_eq!(app.glass, cfg!(feature = "native-gpu"));
    assert!(options.args.contains(&"light".to_owned()));
    let (app, options) = args(&["--window-opacity", "0.5"]).unwrap();
    assert_eq!(app.tint, 50);
    if cfg!(feature = "native-gpu") {
        // Transparent clear; the view paints the adjustable tint.
        assert_eq!(
            options.background,
            volt_ui_kit::native::WindowBackground::Blur { opacity: 0. }
        );
    }
    assert_eq!(args(&["--window-opacity", "0"]).unwrap().0.tint, TINT_MIN);
    let (app, options) = args(&["--background", "opaque"]).unwrap();
    assert!(!app.glass);
    assert_eq!(
        options.background,
        volt_ui_kit::native::WindowBackground::Opaque
    );
    for bad in [
        vec!["--thread", "99"],
        vec!["--theme", "sepia"],
        vec!["--background", "frosted"],
        vec!["--window-opacity", "2"],
    ] {
        assert!(args(&bad).is_err(), "{bad:?}");
    }
}

#[test]
fn themes_are_valid() {
    let mut app = App::new();
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        app.mode = mode;
        assert!(theme(&app, mode).palette.is_valid());
    }
}

#[test]
fn rich_text_handles_code_bullets_and_unterminated_fences() {
    let mut app = App::new();
    let id = app.active;
    let t = app.threads.iter_mut().find(|t| t.id == id).unwrap();
    t.messages.push(Message::agent(
        "Intro line\nwraps here.\n\n- one\n- two\n\n```\nlet x = 1;\n```\n\n```\nstill streaming",
    ));
    mount(&app, [1320., 860.]);
}
