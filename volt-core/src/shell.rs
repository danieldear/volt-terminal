//! Sparse shell prompt anchors. No command text, per-cell metadata, or I/O.
use std::collections::VecDeque;

const MAX_PROMPTS: usize = 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PromptMarks {
    /// Number of history rows evicted since this anchor set was created.
    origin: u64,
    rows: VecDeque<u64>,
    outputs: VecDeque<((u64, usize), (u64, usize))>,
    active_output: Option<(u64, usize)>,
}

impl PromptMarks {
    pub(crate) fn add(&mut self, row: usize) {
        self.rows.retain(|&r| r >= self.origin);
        let row = self.origin.saturating_add(row as u64);
        if self.rows.back() == Some(&row) {
            return;
        }
        if self.rows.len() == MAX_PROMPTS {
            self.rows.pop_front();
        }
        self.rows.push_back(row);
    }

    pub(crate) fn start_output(&mut self, row: usize, col: usize) {
        self.active_output = Some((self.origin.saturating_add(row as u64), col));
    }
    pub(crate) fn end_output(&mut self, row: usize, col: usize) {
        if let Some(start) = self.active_output.take() {
            let end = (self.origin.saturating_add(row as u64), col);
            if start < end {
                if self.outputs.len() == MAX_PROMPTS {
                    self.outputs.pop_front();
                }
                self.outputs.push_back((start, end));
            }
        }
    }
    pub(crate) fn latest_output(&self) -> Option<((usize, usize), (usize, usize))> {
        let &(start, end) = self.outputs.back()?;
        if start.0 < self.origin {
            return None;
        }
        Some((
            (usize::try_from(start.0 - self.origin).ok()?, start.1),
            (usize::try_from(end.0 - self.origin).ok()?, end.1),
        ))
    }
    // Stable sparse positions, fed through the same mapping as text reflow.
    // Never store command strings or scan history while parsing output.
    pub(crate) fn anchors(&self) -> Vec<(usize, usize)> {
        let relative = |r: u64, c| {
            (
                r.checked_sub(self.origin)
                    .and_then(|r| usize::try_from(r).ok())
                    .unwrap_or(usize::MAX),
                c,
            )
        };
        self.rows
            .iter()
            .map(|&r| relative(r, 0))
            .chain(
                self.outputs
                    .iter()
                    .flat_map(|&(a, b)| [relative(a.0, a.1), relative(b.0, b.1)]),
            )
            .chain(self.active_output.iter().map(|&(r, c)| relative(r, c)))
            .collect()
    }
    pub(crate) fn remap(
        &mut self,
        positions: &[Option<(usize, usize)>],
        skip: usize,
        retained: usize,
    ) {
        let mapped = |p: Option<(usize, usize)>, exclusive_end: bool| {
            p.and_then(|(r, c)| {
                let r = r.checked_sub(skip)?;
                // An output's exclusive end may be just beyond the last row.
                // Prompt/start anchors must always address a retained cell.
                (r < retained || exclusive_end && r == retained && c == 0).then_some((r as u64, c))
            })
        };
        let old_count = self.rows.len();
        self.rows = positions[..old_count]
            .iter()
            .filter_map(|&p| mapped(p, false).map(|p| p.0))
            .collect();
        let output_count = self.outputs.len();
        self.outputs = positions[old_count..old_count + output_count * 2]
            .as_chunks::<2>()
            .0
            .iter()
            .filter_map(|p| {
                let start = mapped(p[0], false)?;
                let end = mapped(p[1], true)?;
                (start < end).then_some((start, end))
            })
            .collect();
        self.active_output = if self.active_output.is_some() {
            positions.last().copied().and_then(|p| mapped(p, false))
        } else {
            None
        };
        self.origin = 0;
    }

    pub(crate) fn evict(&mut self, count: usize) {
        // Constant time even for a full 1024-entry set and a large scroll.
        if let Some(origin) = self.origin.checked_add(count as u64) {
            self.origin = origin;
        } else {
            *self = Self::default();
        }
    }

    pub(crate) fn invalidate_live(&mut self, history: usize) {
        let end = self.origin.saturating_add(history as u64);
        self.rows.retain(|&r| r >= self.origin && r < end);
        self.outputs
            .retain(|&(a, b)| a.0 >= self.origin && b.0 < end);
        self.active_output = None;
    }

    pub(crate) fn offset(&self, history: usize, offset: usize, previous: bool) -> usize {
        let offset = offset.min(history);
        let top = history - offset;
        let candidates = self.rows.iter().filter_map(|&r| {
            let row = usize::try_from(r.checked_sub(self.origin)?).ok()?;
            (row < history).then_some(row)
        });
        let row = if previous {
            candidates.filter(|&r| r < top).max()
        } else {
            candidates.filter(|&r| r > top).min()
        };
        row.map(|r| history - r)
            .unwrap_or(if previous { offset } else { 0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exclusive_end_can_follow_last_retained_row_but_start_cannot() {
        let mut marks = PromptMarks::default();
        marks.add(0);
        marks.start_output(0, 0);
        marks.end_output(1, 0);
        marks.remap(&[Some((0, 0)), Some((0, 0)), Some((2, 0))], 0, 2);
        assert_eq!(marks.latest_output(), Some(((0, 0), (2, 0))));
        // An invalid/out-of-range start cannot turn into a selectable range.
        marks.remap(&[Some((2, 0)), Some((2, 0)), Some((2, 0))], 0, 2);
        assert!(marks.rows.is_empty());
        assert_eq!(marks.latest_output(), None);
    }
    #[test]
    fn bounded_deduplicated_and_eviction_aware() {
        let mut marks = PromptMarks::default();
        for row in 0..2000 {
            marks.add(row);
            marks.add(row);
        }
        assert_eq!(marks.rows.len(), MAX_PROMPTS);
        marks.evict(1900);
        assert_eq!(marks.offset(100, 0, true), 1);
        marks.add(100);
        assert_eq!(marks.rows.len(), 101);
        marks.invalidate_live(90);
        assert_eq!(marks.offset(100, 0, true), 11);
    }
    #[test]
    fn navigation_is_spatial_not_insertion_order() {
        let mut marks = PromptMarks::default();
        for row in [2, 8, 4] {
            marks.add(row);
        }
        assert_eq!(marks.offset(10, 0, true), 2);
        assert_eq!(marks.offset(10, 2, true), 6);
        assert_eq!(marks.offset(10, 6, false), 2);
        assert_eq!(marks.offset(10, 2, false), 0);
        assert_eq!(marks.offset(10, 8, true), 8);
    }
}
