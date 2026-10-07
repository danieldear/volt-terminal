//! Opt-in aggregate timing. Never records terminal contents, commands, paths,
//! environment values or credentials. Set VOLT_PERF=1 before launching Volt.
use std::io::Write;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
#[repr(usize)]
pub enum Stage {
    ReadWait,
    Read,
    QueueSend,
    QueueWait,
    ParseLock,
    Parse,
    Snapshot,
    Drawable,
    Geometry,
    Submit,
}
const STAGES: [&str; 10] = [
    "read_wait",
    "read",
    "queue_send",
    "queue_wait",
    "parse_lock",
    "parse",
    "snapshot",
    "drawable_wait",
    "geometry",
    "submit_present_cpu",
];
#[derive(Clone, Copy)]
#[repr(usize)]
pub enum Counter {
    Bytes,
    Reads,
    Batches,
    Chunks,
    Notifications,
    Frames,
    ReusedRows,
    PendingBytesPeak,
    SurfaceTimeout,
    SurfaceOccluded,
    SurfaceOutdated,
    SurfaceLost,
    SurfaceValidation,
}
const COUNTERS: [&str; 13] = [
    "bytes",
    "reads",
    "batches",
    "chunks",
    "notifications",
    "frames",
    "reused_rows",
    "pending_bytes_peak",
    "surface_timeout",
    "surface_occluded",
    "surface_outdated",
    "surface_lost",
    "surface_validation",
];
#[derive(Clone, Copy, Default)]
struct Timing {
    count: u64,
    total: Duration,
    max: Duration,
}

pub struct PerfStats {
    component: &'static str,
    timings: [Timing; 10],
    counters: [u64; 13],
}
impl PerfStats {
    pub fn from_env(component: &'static str) -> Option<Self> {
        (std::env::var_os("VOLT_PERF").as_deref() == Some(std::ffi::OsStr::new("1")))
            .then(|| Self::new(component))
    }
    fn new(component: &'static str) -> Self {
        Self {
            component,
            timings: [Timing::default(); 10],
            counters: [0; 13],
        }
    }
    #[inline]
    pub fn start(stats: &Option<Self>) -> Option<Instant> {
        stats.as_ref().map(|_| Instant::now())
    }
    pub fn finish(&mut self, stage: Stage, start: Instant) {
        self.record(stage, start.elapsed());
    }
    fn record(&mut self, stage: Stage, duration: Duration) {
        let timing = &mut self.timings[stage as usize];
        timing.count = timing.count.saturating_add(1);
        timing.total = timing.total.saturating_add(duration);
        timing.max = timing.max.max(duration);
    }
    pub fn add(&mut self, counter: Counter, value: u64) {
        self.counters[counter as usize] = self.counters[counter as usize].saturating_add(value);
    }
    pub fn peak(&mut self, counter: Counter, value: u64) {
        self.counters[counter as usize] = self.counters[counter as usize].max(value);
    }
    fn report(&self, writer: &mut impl Write) -> std::io::Result<()> {
        write!(writer, "volt-perf component={}", self.component)?;
        for (name, count) in COUNTERS.iter().zip(self.counters) {
            if count != 0 {
                write!(writer, " {name}={count}")?;
            }
        }
        for (name, timing) in STAGES.iter().zip(self.timings) {
            if timing.count != 0 {
                write!(
                    writer,
                    " {name}[n={},total_ms={:.3},max_ms={:.3}]",
                    timing.count,
                    timing.total.as_secs_f64() * 1000.0,
                    timing.max.as_secs_f64() * 1000.0
                )?;
            }
        }
        writeln!(writer)
    }
}
impl Drop for PerfStats {
    fn drop(&mut self) {
        let _ = self.report(&mut std::io::stderr().lock());
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_diagnostics_do_not_read_clock() {
        assert!(PerfStats::start(&None).is_none());
    }
    #[test]
    fn report_has_only_aggregates_and_fixed_labels() {
        let mut stats = PerfStats::new("test");
        stats.add(Counter::Bytes, 10);
        stats.add(Counter::Bytes, 20);
        stats.peak(Counter::PendingBytesPeak, 40);
        stats.peak(Counter::PendingBytesPeak, 15);
        stats.record(Stage::Parse, Duration::from_millis(2));
        stats.record(Stage::Parse, Duration::from_millis(3));
        let mut out = Vec::new();
        stats.report(&mut out).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "volt-perf component=test bytes=30 pending_bytes_peak=40 parse[n=2,total_ms=5.000,max_ms=3.000]\n");
    }
}
