//! Headless parser throughput benchmark.
//!
//! Feeds a capture file (e.g. termbench output) through the VTE parser and
//! Performer exactly like the PTY reader thread does, and reports MB/s.
//!
//! Usage: cargo run --release -p volt-core --example parsebench <capture-file>

use std::time::Instant;

use volt_core::performer::Performer;

/// Perform impl that does nothing — isolates raw vte dispatch cost.
struct Noop;
impl vte::Perform for Noop {
    fn print(&mut self, _: char) {}
    fn execute(&mut self, _: u8) {}
    fn hook(&mut self, _: &vte::Params, _: &[u8], _: bool, _: char) {}
    fn put(&mut self, _: u8) {}
    fn unhook(&mut self) {}
    fn osc_dispatch(&mut self, _: &[&[u8]], _: bool) {}
    fn csi_dispatch(&mut self, _: &vte::Params, _: &[u8], _: bool, _: char) {}
    fn esc_dispatch(&mut self, _: &[u8], _: bool, _: u8) {}
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let noop = args.iter().any(|a| a == "--noop");
    args.retain(|a| a != "--noop");
    let path = args
        .first()
        .expect("usage: parsebench [--noop] <capture-file>");
    let data = std::fs::read(path).expect("failed to read capture file");

    if noop {
        let mut parser = vte::Parser::new();
        let mut sink = Noop;
        let start = Instant::now();
        parser.advance(&mut sink, &data);
        let elapsed = start.elapsed();
        let mb = data.len() as f64 / (1024.0 * 1024.0);
        println!(
            "noop: parsed {:.1} MB in {:.3}s = {:.1} MB/s",
            mb,
            elapsed.as_secs_f64(),
            mb / elapsed.as_secs_f64()
        );
        return;
    }
    let mut performer = Performer::new(200, 60);
    performer.set_scrollback_limit(10_000);
    let mut parser = vte::Parser::new();

    let start = Instant::now();
    for chunk in data.chunks(4096) {
        parser.advance(&mut performer, chunk);
        performer.display_dirty = false;
        let _ = performer.take_damage_rows();
        performer.pending_events.clear();
        performer.pending_writes.clear();
    }
    let elapsed = start.elapsed();
    let mb = data.len() as f64 / (1024.0 * 1024.0);
    println!(
        "parsed {:.1} MB in {:.3}s = {:.1} MB/s",
        mb,
        elapsed.as_secs_f64(),
        mb / elapsed.as_secs_f64()
    );
}
