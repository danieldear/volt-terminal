//! PTY write -> child response -> parsed cell latency, NOT display latency.
//! cargo run --release --locked -p volt-core --example ptybench -- 300
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
use volt_core::{performer::Performer, pty::Pty};

fn wait_for_text(
    performer: &std::sync::Mutex<Performer>,
    notifications: &mpsc::Receiver<()>,
    text: &str,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        {
            let p = performer.lock().unwrap();
            if (0..p.grid.rows).any(|r| p.grid.row_text(p.grid.row_cells(r)).contains(text)) {
                return Ok(());
            }
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            anyhow::bail!("timed out waiting for child response");
        }
        notifications.recv_timeout(remaining)?;
    }
}

fn main() -> anyhow::Result<()> {
    let count: usize = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "300".into())
        .parse()?;
    anyhow::ensure!((10..=100_000).contains(&count), "count must be 10..100000");
    let (tx, rx) = mpsc::sync_channel(1);
    let args = vec!["-c".into(), "stty -echo; printf 'READY\\n'; while IFS= read -r line; do printf 'reply:%s\\n' \"$line\"; done".into()];
    let (mut pty, performer, _events) = Pty::spawn("/bin/sh", &args, 80, 24, move || {
        let _ = tx.try_send(());
    })?;
    wait_for_text(&performer, &rx, "READY")?;
    let mut samples = Vec::with_capacity(count);
    // Each unique marker must come from the child: echo is disabled, and input
    // doesn't contain the reply prefix. Exclude 30 warm-up exchanges.
    for i in 0..count + 30 {
        let marker = format!("{i:08}");
        let start = Instant::now();
        pty.write(format!("{marker}\n").as_bytes())?;
        wait_for_text(&performer, &rx, &format!("reply:{marker}"))?;
        if i >= 30 {
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
        }
    }
    samples.sort_by(f64::total_cmp);
    println!("{{\"metric\":\"pty_child_parse_roundtrip_ms\",\"samples\":{count},\"warmup\":30,\"p50\":{:.6},\"p95\":{:.6},\"p99\":{:.6},\"max\":{:.6},\"includes_display\":false}}",
        samples[count / 2], samples[(count * 95 / 100).min(count - 1)], samples[(count * 99 / 100).min(count - 1)], samples[count - 1]);
    Ok(())
}
