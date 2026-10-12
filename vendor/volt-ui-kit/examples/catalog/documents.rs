//! Developer-owned document policy. The toolkit does not choose storage paths.
use super::{
    lifecycle,
    pages::{Doc, Page, Section},
    App, Message,
};
use std::{path::PathBuf, thread, time::Duration};
use volt_ui_kit::{
    native::{
        file_dialog::{self, FileDialogOptions},
        files::{self, SavePolicy},
        ApplicationRequest, CloseReason, UiSendError,
    },
    ui::{prelude::*, Element as El},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum After {
    Open,
    Close(CloseReason),
}
#[derive(Default)]
pub struct State {
    pub path: Option<PathBuf>,
    pub text: String,
    pub saved: String,
    pub busy: bool,
    pub generation: u64,
    pub after: Option<After>,
    pub prompt: bool,
    pub error: Option<String>,
    pub status: String,
}
impl State {
    pub fn dirty(&self) -> bool {
        self.text != self.saved
    }
}
#[derive(Debug)]
pub enum Completion {
    Loaded { path: PathBuf, text: String },
    Saved { path: PathBuf, snapshot: String },
}
/// Completion retries are worker-only and bounded by channel closure. There is
/// one document operation at a time; normal progress coalesces in the other demo.
fn deliver(
    sender: volt_ui_kit::native::UiSender<Message>,
    generation: u64,
    result: Result<Completion, String>,
) {
    // A bounded result needs ownership on retries because UiSender consumes M.
    let result = std::sync::Arc::new(result);
    loop {
        match sender.send(Message::File {
            generation,
            result: result.clone(),
        }) {
            Ok(()) | Err(UiSendError::Closed) => break,
            Err(UiSendError::Full) => thread::sleep(Duration::from_millis(10)),
        }
    }
}
fn ready(app: &mut App) -> bool {
    if app.documents.busy {
        return false;
    }
    if app.sender.is_none() {
        app.documents.error = Some("File operations run in the native Catalog window only".into());
        return false;
    }
    true
}
pub fn open(app: &mut App) {
    app.go(Page::Documents);
    if !ready(app) {
        return;
    }
    app.documents.error = None;
    if app.documents.dirty() {
        app.documents.after = Some(After::Open);
        app.documents.prompt = true;
        return;
    }
    choose_open(app);
}
fn choose_open(app: &mut App) {
    app.documents.after = None;
    app.documents.prompt = false;
    match file_dialog::open_file(&FileDialogOptions::new("Open UTF-8 notes (8 KiB maximum)")) {
        Ok(Some(path)) => load_path(app, path),
        Ok(None) => app.documents.status = "Open cancelled; document unchanged".into(),
        Err(e) => app.documents.error = Some(e.to_string()),
    }
}
pub fn load_path(app: &mut App, path: PathBuf) {
    if !ready(app) {
        return;
    }
    let sender = app.sender.clone().unwrap();
    app.documents.generation = app.documents.generation.wrapping_add(1);
    let generation = app.documents.generation;
    app.documents.busy = true;
    app.documents.error = None;
    app.documents.status = "Opening…".into();
    if let Err(e) = thread::Builder::new()
        .name("volt-catalog-open".into())
        .spawn(move || {
            let result = files::read_text_file(&path)
                .map(|text| Completion::Loaded { path, text })
                .map_err(|e| e.to_string());
            deliver(sender, generation, result);
        })
    {
        app.documents.busy = false;
        app.documents.error = Some(format!("Cannot start open worker: {e}"));
        app.documents.prompt = app.documents.after.is_some();
    }
}
pub fn save(app: &mut App) {
    save_as_or_current(app, false)
}
pub fn save_as(app: &mut App) {
    save_as_or_current(app, true)
}
fn save_as_or_current(app: &mut App, save_as: bool) {
    if app.page != Page::Documents {
        app.go(Page::Documents);
    }
    if !ready(app) {
        return;
    }
    app.documents.error = None;
    let selected = if !save_as {
        app.documents.path.clone()
    } else {
        None
    };
    let (path, approved) = if let Some(path) = selected {
        (path, false)
    } else {
        let name = app
            .documents
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("notes.txt");
        match file_dialog::save_file(
            &FileDialogOptions::new("Save UTF-8 notes").suggested_name(name),
        ) {
            Ok(Some(path)) => (path, true),
            Ok(None) => {
                app.documents.after = None;
                app.documents.prompt = false;
                app.documents.status = "Save cancelled; draft unchanged".into();
                return;
            }
            Err(e) => {
                app.documents.error = Some(e.to_string());
                return;
            }
        }
    };
    save_path(app, path, approved);
}
pub fn save_path(app: &mut App, path: PathBuf, approved: bool) {
    if !ready(app) {
        return;
    }
    let sender = app.sender.clone().unwrap();
    let snapshot = app.documents.text.clone();
    let expected = app.documents.saved.clone();
    app.documents.generation = app.documents.generation.wrapping_add(1);
    let generation = app.documents.generation;
    app.documents.busy = true;
    app.documents.error = None;
    app.documents.prompt = false;
    app.documents.status = "Saving…".into();
    if let Err(e) = thread::Builder::new()
        .name("volt-catalog-save".into())
        .spawn(move || {
            let policy = if approved {
                SavePolicy::ApprovedReplacement
            } else {
                SavePolicy::MatchContents(&expected)
            };
            let result = files::save_text_file(&path, &snapshot, policy)
                .map(|()| Completion::Saved { path, snapshot })
                .map_err(|e| e.to_string());
            deliver(sender, generation, result);
        })
    {
        app.documents.busy = false;
        app.documents.error = Some(format!("Cannot start save worker: {e}"));
        app.documents.prompt = app.documents.after.is_some();
    }
}
/// Main-thread completion; stale generations and failed writes cannot mark saved.
pub fn finish(app: &mut App, generation: u64, result: &Result<Completion, String>) {
    if !app.documents.busy || generation != app.documents.generation {
        return;
    }
    app.documents.busy = false;
    match result {
        Err(e) => {
            app.documents.error = Some(e.clone());
            app.documents.status = "Operation failed; draft retained".into();
            app.documents.prompt = app.documents.after.is_some();
            return;
        }
        Ok(Completion::Loaded { path, text }) => {
            app.documents.path = Some(path.clone());
            app.documents.text.clone_from(text);
            app.documents.saved.clone_from(text);
            app.documents.status = "Opened from disk".into();
            app.go(Page::Documents);
        }
        Ok(Completion::Saved { path, snapshot }) => {
            app.documents.path = Some(path.clone());
            app.documents.saved.clone_from(snapshot);
            app.documents.status = "Saved to disk".into();
        }
    }
    if app.documents.dirty() {
        app.documents.prompt = app.documents.after.is_some();
        return;
    }
    proceed(app);
}
fn proceed(app: &mut App) {
    app.documents.prompt = false;
    match app.documents.after.take() {
        Some(After::Open) => choose_open(app),
        Some(After::Close(reason)) => lifecycle::request(app, ApplicationRequest::Close(reason)),
        None => {}
    }
}
pub fn cancel(app: &mut App) {
    app.documents.after = None;
    app.documents.prompt = false;
}
pub fn discard(app: &mut App) {
    if app.documents.busy {
        return;
    }
    app.documents.text.clone_from(&app.documents.saved);
    proceed(app);
}
pub fn close_guard(app: &mut App, reason: CloseReason) -> bool {
    if !app.documents.busy && !app.documents.dirty() {
        return false;
    }
    app.app_commands.open = false;
    app.palette.open = false;
    app.documents.after = Some(After::Close(reason));
    app.documents.prompt = true;
    true
}
pub fn doc(app: &App) -> Doc {
    let state = &app.documents;
    let name = state
        .path
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or("Untitled.txt".into());
    let error: El<App> = if let Some(error) = &state.error {
        panel![text(error).font_size(13.).color(ColorToken::Error)]
            .padding(12., Edge::All)
            .rounded(8.)
            .background(ColorToken::Error.alpha(10))
            .into()
    } else {
        spacer().height(0.).into()
    };
    Doc {sections:vec![Section {id:"document-files",title:"A real notes document",note:"Native macOS file panels and worker-thread UTF-8 I/O. 8 KiB maximum; failed/cancelled operations retain your draft. No automatic saving or hidden path selection.",
        demo:vstack![
            hstack![text(name).font_family("SF Mono").font_size(13.),spacer(),badge(if state.busy {"Working…"} else if state.dirty(){"Unsaved"} else {"No unsaved changes"})],
            hstack![
                button("Open…").key("files-open").variant(ButtonVariant::Outline).enabled(!state.busy).on_click(|app:&mut App|lifecycle::request(app,ApplicationRequest::Command("open-file".into()))),
                button("Save").key("files-save").enabled(!state.busy&&(state.dirty()||state.path.is_none())).on_click(|app:&mut App|lifecycle::request(app,ApplicationRequest::Command("save-file".into()))),
                button("Save As…").key("files-save-as").variant(ButtonVariant::Outline).enabled(!state.busy).on_click(|app:&mut App|lifecycle::request(app,ApplicationRequest::Command("save-as-file".into()))),
            ].gap(8.,Axis::Horizontal),
            text_area(&state.text).key("files-editor").accessible_label("File document editor").height(240.).soft_wrap(true).read_only(state.busy).font_family("SF Mono").font_size(13.).on_change(|app:&mut App,text|app.documents.text=text),
            text(if state.status.is_empty(){"Choose a file or write a new note. Cmd+O opens; Cmd+S saves the active notes document."} else {&state.status}).font_size(12.).color(ColorToken::Muted),error,
            text("Matching saves detect ordinary external edits. This is not a hostile-directory sandbox or a multi-process transaction; ACLs/xattrs are not preserved.").font_size(12.).color(ColorToken::Muted),
        ].width(640.).gap(14.,Axis::Vertical).into(),
        code:r#"// Application owns document state and paths.
// Native panels choose a path; read/write on a worker.
let selected = file_dialog::open_file(&FileDialogOptions::new("Open notes"))?;
// Worker: files::read_text_file(&path)
// Worker: files::save_text_file(&path, &snapshot, SavePolicy::MatchContents(&original))
// Main-thread completion marks ONLY that saved snapshot clean.
// Close stays vetoed until saving succeeds; errors keep the draft."#,
    }],api:vec![("file_dialog::open_file / save_file", "Main-thread, native-desktop macOS panels; Cancel is Ok(None)."),("files::read_text_file", "Bounded UTF-8 regular file; no silent truncation."),("files::save_text_file", "Private sibling + sync + commit; explicit create/replace policy."),("SavePolicy::MatchContents(original)","Rejects ordinary external edits before replacement; not a cross-process lock.")]}
}
pub fn overlay(app: &App) -> El<App> {
    let error: El<App> = if let Some(error) = &app.documents.error {
        text(error).color(ColorToken::Error).font_size(13.).into()
    } else {
        spacer().height(0.).into()
    };
    dialog(vstack![
        text(if app.documents.busy {"A file operation is running. Keep the window open until it completes; Cancel cancels the pending close/open action, not the disk operation."} else {"This notes document has unsaved changes. Save writes to disk; Discard restores the last saved text; Cancel keeps editing."}).font_size(14.),
        error,
        hstack![spacer(),button("Cancel").key("files-close-cancel").variant(ButtonVariant::Outline).on_click(cancel),
            button("Discard").key("files-close-discard").variant(ButtonVariant::Danger).enabled(!app.documents.busy).on_click(discard),
            button("Save").key("files-close-save").enabled(!app.documents.busy).on_click(save),
        ].gap(8.,Axis::Horizontal),
    ].gap(18.,Axis::Vertical)).key("files-confirm").title(if app.documents.busy {"File operation in progress"} else {"Save this document?"}).open(app.documents.prompt).width(540.).rounded(12.).background(ColorToken::Base100).backdrop_dismiss(false).on_dismiss(cancel).into()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_or_failed_save_retains_draft_and_does_not_close() {
        let mut app = App::new();
        app.documents.text = "edited".into();
        app.documents.busy = true;
        app.documents.generation = 4;
        app.documents.after = Some(After::Close(CloseReason::Quit));
        finish(
            &mut app,
            3,
            &Ok(Completion::Saved {
                path: "/tmp/irrelevant".into(),
                snapshot: "old".into(),
            }),
        );
        assert!(app.documents.busy);
        assert!(app.documents.dirty());
        finish(&mut app, 4, &Err("disk full".into()));
        assert!(!app.documents.busy);
        assert!(app.documents.dirty());
        assert!(app.documents.prompt);
        assert_eq!(app.documents.text, "edited");
        cancel(&mut app);
        assert_eq!(app.documents.after, None);
        assert!(app.documents.dirty());
    }
    #[test]
    fn save_completion_marks_only_written_snapshot_clean() {
        let mut app = App::new();
        app.documents.text = "newer edit".into();
        app.documents.busy = true;
        app.documents.generation = 1;
        app.documents.after = Some(After::Close(CloseReason::Window));
        finish(
            &mut app,
            1,
            &Ok(Completion::Saved {
                path: "/tmp/irrelevant".into(),
                snapshot: "older snapshot".into(),
            }),
        );
        assert!(app.documents.dirty());
        assert!(app.documents.prompt);
        assert_eq!(app.documents.saved, "older snapshot");
        discard(&mut app);
        assert!(!app.documents.dirty());
        assert_eq!(app.documents.after, None);
    }
    #[test]
    fn busy_close_veto_survives_cancel_and_failed_load() {
        let mut app = App::new();
        app.documents.text = "draft".into();
        app.documents.busy = true;
        app.documents.generation = 2;
        assert!(close_guard(&mut app, CloseReason::Window));
        cancel(&mut app);
        assert!(app.documents.busy);
        finish(&mut app, 2, &Err("cannot open".into()));
        assert_eq!(app.documents.text, "draft");
        assert!(!app.documents.prompt);
    }
}
