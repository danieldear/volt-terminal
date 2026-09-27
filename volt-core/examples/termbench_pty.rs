//! Headless production PTY/parser benchmark driver, NOT GPU/render latency.
//! Usage: termbench_pty /absolute/path/to/termbench-report /absolute/report/path
//! The child must save its own timed summary to VOLT_BENCH_REPORT. No output
//! filtering, line-discipline changes, or intermediate stdout pipe is used.
use std::time::{Duration, Instant};
use volt_core::{events::CoreEvent, pty::Pty};

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    anyhow::ensure!(
        args.len() == 2,
        "expected benchmark executable and report path"
    );
    anyhow::ensure!(
        std::path::Path::new(&args[0]).is_absolute(),
        "use an absolute benchmark path"
    );
    std::env::set_var("VOLT_BENCH_REPORT", &args[1]);
    let (_pty, performer, mut events) = Pty::spawn(&args[0], &[], 91, 16, || {})?;
    let start = Instant::now();
    let (completed_tx, completed_rx) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = loop {
            match events.blocking_recv() {
                Some(CoreEvent::PtyClosed) => break Ok(()),
                Some(CoreEvent::PtyError(error)) => break Err(anyhow::anyhow!(error)),
                Some(_) => {}
                None => break Err(anyhow::anyhow!("missing PTY completion")),
            }
        };
        let _ = completed_tx.send(result);
    });
    completed_rx.recv_timeout(Duration::from_secs(180))??;
    let grid = &performer.lock().unwrap().grid;
    for row in 0..grid.rows {
        println!("{}", grid.row_text(grid.row_cells(row)));
    }
    println!(
        "headless PTY + parse completion: {:.4}s",
        start.elapsed().as_secs_f64()
    );
    Ok(())
}
