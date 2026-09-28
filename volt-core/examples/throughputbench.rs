//! Deterministic parser-only throughput. No PTY, GPU, shell, or output dropping.
//! Set VOLT_BENCH_PROMPTS=1 to include the shell-anchor scroll bookkeeping.
//! cargo run --release -p volt-core --example throughputbench -- [MiB=128] [cols=91] [rows=16]
use std::{hint::black_box, time::Instant};
use volt_core::performer::Performer;

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let arg = |i: usize, default| {
        args.get(i)
            .map(|s| s.parse::<usize>().unwrap())
            .unwrap_or(default)
    };
    let mib = arg(0, 128);
    let cols = arg(1, 91);
    let rows = arg(2, 16);
    let shell_seed = std::env::var_os("VOLT_BENCH_PROMPTS").is_some();
    assert!((1..=4096).contains(&mib) && cols > 0 && rows > 0);
    for (name, pattern) in [
        ("many", b"abcdefghijklmnopqrstuvwxyz\r\n".to_vec()),
        ("long", b"abcdefghijklmnopqrstuvwxyz".to_vec()),
        ("fg", b"\x1b[38;2;123;45;67ma".to_vec()),
        ("fgbg", b"\x1b[48;2;76;54;32m\x1b[38;2;123;45;67ma".to_vec()),
    ] {
        let block = pattern.repeat((64 * 1024) / pattern.len());
        for seeded_wide in [false, true] {
            let mut p = Performer::new(cols, rows);
            p.set_scrollback_limit(10_000);
            let mut parser = vte::Parser::new();
            if seeded_wide {
                parser.advance(&mut p, "界\r\n".as_bytes());
            }
            if shell_seed {
                parser.advance(&mut p, b"\x1b]133;A\x07");
            }
            let start = Instant::now();
            let mut bytes = 0;
            while bytes < mib * 1024 * 1024 {
                for chunk in block.chunks(16 * 1024) {
                    parser.advance(&mut p, black_box(chunk));
                    p.display_dirty = false;
                    black_box(p.take_damage_rows());
                }
                bytes += block.len();
            }
            let seconds = start.elapsed().as_secs_f64();
            black_box(&p);
            println!("{{\"case\":\"{name}\",\"wide_seed\":{seeded_wide},\"cols\":{cols},\"rows\":{rows},\"bytes\":{bytes},\"seconds\":{seconds:.6},\"mib_s\":{:.2}}}", bytes as f64 / 1048576. / seconds);
        }
    }
}
