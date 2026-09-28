//! Sparse shell prompt anchors. No command text, per-cell metadata, or I/O.
use std::collections::VecDeque;

const MAX_PROMPTS: usize = 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PromptMarks {
    /// Number of history rows evicted since this anchor set was created.
    origin: u64,
    rows: VecDeque<u64>,
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
