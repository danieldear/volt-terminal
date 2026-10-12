//! Application-owned cancellation and stale-result policy, not hidden framework state.
use super::App;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
#[derive(Debug)]
pub enum Message {
    Progress { generation: u64, step: usize },
}
pub fn start(app: &mut App) {
    let Some(sender) = app.sender.clone() else {
        app.record("Workers are native-only; snapshots stay deterministic");
        return;
    };
    app.job_cancel.store(true, Ordering::Relaxed);
    app.job_cancel = Arc::new(AtomicBool::new(false));
    app.job_generation = app.job_generation.wrapping_add(1);
    app.job_progress = 0;
    app.job_running = true;
    let generation = app.job_generation;
    let cancel = app.job_cancel.clone();
    app.record("Background job started. Try clicking while it runs.");
    std::thread::spawn(move || {
        for step in 1..=100 {
            std::thread::sleep(Duration::from_millis(35));
            if cancel.load(Ordering::Relaxed) || sender.is_closed() {
                break;
            }
            // This producer sends <30 messages/sec, well below the bounded queue.
            match sender.send(Message::Progress { generation, step }) {
                Ok(()) => {}
                Err(volt_ui_kit::native::UiSendError::Closed) => break,
                Err(volt_ui_kit::native::UiSendError::Full) => {
                    // Backpressure is explicit. Do not silently lose completion.
                    if step == 100 {
                        while !cancel.load(Ordering::Relaxed) {
                            std::thread::sleep(Duration::from_millis(10));
                            if sender.send(Message::Progress { generation, step }).is_ok()
                                || sender.is_closed()
                            {
                                break;
                            }
                        }
                    }
                }
            }
        }
    });
}
pub fn cancel(app: &mut App) {
    app.job_cancel.store(true, Ordering::Relaxed);
    app.job_generation = app.job_generation.wrapping_add(1);
    app.job_running = false;
    app.record("Job cancelled; any queued results from its generation are ignored");
}
pub fn update(app: &mut App, message: Message) {
    match message {
        Message::Progress { generation, step }
            if generation == app.job_generation && app.job_running =>
        {
            app.job_progress = step.min(100);
            if step >= 100 {
                app.job_running = false;
                app.record("Background job complete — UI state updated on event thread");
            }
        }
        _ => {}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_and_cancelled_results_cannot_restore_old_job() {
        let mut app = App::default();
        app.job_generation = 3;
        app.job_running = true;
        update(
            &mut app,
            Message::Progress {
                generation: 2,
                step: 100,
            },
        );
        assert_eq!(app.job_progress, 0);
        update(
            &mut app,
            Message::Progress {
                generation: 3,
                step: 20,
            },
        );
        assert_eq!(app.job_progress, 20);
        cancel(&mut app);
        update(
            &mut app,
            Message::Progress {
                generation: 3,
                step: 100,
            },
        );
        assert_eq!(app.job_progress, 20);
        assert!(!app.job_running);
    }
}
