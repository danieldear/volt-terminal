use crate::tab::{PaneSplitDirection, TerminalPane};
use volt_renderer::PaneDivider;

#[derive(Debug, Clone, Copy)]
pub struct PaneRect {
    pub id: usize,
    pub col: usize,
    pub row: usize,
    pub cols: usize,
    pub rows: usize,
}

pub struct DividerInfo {
    pub id: usize,
    pub phys: PaneDivider,
    pub direction: PaneSplitDirection,
}

pub enum PaneNode {
    Leaf {
        pane: TerminalPane,
        id: usize,
    },
    Split {
        direction: PaneSplitDirection,
        ratio: f32,
        children: Box<[PaneNode; 2]>,
        divider_id: usize,
    },
    /// Transient placeholder used only during `remove_node`; never visible to callers.
    Tombstone,
}

pub struct PaneTree {
    pub root: PaneNode,
    pub active_id: usize,
    next_id: usize,
}

impl PaneTree {
    pub fn new(pane: TerminalPane) -> Self {
        Self {
            root: PaneNode::Leaf { pane, id: 0 },
            active_id: 0,
            next_id: 1,
        }
    }

    pub fn split(&mut self, target_id: usize, direction: PaneSplitDirection, new_pane: TerminalPane) {
        let new_pane_id = self.next_id;
        self.next_id += 1;
        let divider_id = self.next_id;
        self.next_id += 1;
        let mut pane_opt = Some(new_pane);
        if split_node(&mut self.root, target_id, direction, &mut pane_opt, new_pane_id, divider_id) {
            self.active_id = new_pane_id;
        }
    }

    /// Returns `false` if `target_id` is the only remaining pane (cannot remove).
    pub fn remove(&mut self, target_id: usize) -> bool {
        if matches!(&self.root, PaneNode::Leaf { id, .. } if *id == target_id) {
            return false;
        }
        remove_node(&mut self.root, target_id, &mut self.active_id)
    }

    pub fn layout(&self, cols: usize, rows: usize) -> Vec<PaneRect> {
        let mut out = Vec::new();
        layout_node(&self.root, 0, 0, cols, rows, &mut out);
        out
    }

    pub fn dividers_with_ids(
        &self,
        cols: usize,
        rows: usize,
        cell_w: f32,
        cell_h: f32,
        phys_pad: f32,
        content_top: f32,
        scale: f32,
    ) -> Vec<DividerInfo> {
        let mut out = Vec::new();
        dividers_node(
            &self.root, 0, 0, cols, rows,
            cell_w, cell_h, phys_pad, content_top, scale,
            &mut out,
        );
        out
    }

    pub fn find_leaf(&self, id: usize) -> Option<&TerminalPane> {
        find_leaf_node(&self.root, id)
    }

    pub fn find_leaf_mut(&mut self, id: usize) -> Option<&mut TerminalPane> {
        find_leaf_mut_node(&mut self.root, id)
    }

    pub fn leaf_ids(&self) -> Vec<usize> {
        let mut out = Vec::new();
        collect_leaf_ids(&self.root, &mut out);
        out
    }

    pub fn adjust_ratio(
        &mut self,
        divider_id: usize,
        delta_px: f32,
        cell_w: f32,
        cell_h: f32,
        total_cols: usize,
        total_rows: usize,
    ) {
        adjust_ratio_node(&mut self.root, divider_id, delta_px, cell_w, cell_h, total_cols, total_rows);
    }

    pub fn active_pane(&self) -> Option<&TerminalPane> {
        self.find_leaf(self.active_id)
    }

    pub fn active_pane_mut(&mut self) -> Option<&mut TerminalPane> {
        self.find_leaf_mut(self.active_id)
    }

    pub fn focus_next(&mut self) {
        let ids = self.leaf_ids();
        if let Some(pos) = ids.iter().position(|&id| id == self.active_id) {
            self.active_id = ids[(pos + 1) % ids.len()];
        }
    }

    pub fn pane_count(&self) -> usize {
        self.leaf_ids().len()
    }
}

// ── recursive helpers ─────────────────────────────────────────────────────────

fn layout_node(node: &PaneNode, col: usize, row: usize, cols: usize, rows: usize, out: &mut Vec<PaneRect>) {
    match node {
        PaneNode::Leaf { id, .. } => {
            out.push(PaneRect { id: *id, col, row, cols, rows });
        }
        PaneNode::Split { direction, ratio, children, .. } => {
            match direction {
                PaneSplitDirection::Vertical => {
                    let left_cols = left_cols_from_ratio(*ratio, cols);
                    let right_cols = cols.saturating_sub(left_cols + 1);
                    layout_node(&children[0], col, row, left_cols, rows, out);
                    layout_node(&children[1], col + left_cols + 1, row, right_cols, rows, out);
                }
                PaneSplitDirection::Horizontal => {
                    let top_rows = top_rows_from_ratio(*ratio, rows);
                    let bot_rows = rows.saturating_sub(top_rows + 1);
                    layout_node(&children[0], col, row, cols, top_rows, out);
                    layout_node(&children[1], col, row + top_rows + 1, cols, bot_rows, out);
                }
            }
        }
        PaneNode::Tombstone => {}
    }
}

fn dividers_node(
    node: &PaneNode,
    base_col: usize, base_row: usize,
    cols: usize, rows: usize,
    cell_w: f32, cell_h: f32,
    phys_pad: f32, content_top: f32,
    scale: f32,
    out: &mut Vec<DividerInfo>,
) {
    match node {
        PaneNode::Leaf { .. } | PaneNode::Tombstone => {}
        PaneNode::Split { direction, ratio, children, divider_id } => {
            let line_w = scale.max(1.0);
            let color = [0.35_f32, 0.35, 0.35, 1.0];
            match direction {
                PaneSplitDirection::Vertical => {
                    let left_cols = left_cols_from_ratio(*ratio, cols);
                    let div_x = phys_pad + (base_col + left_cols) as f32 * cell_w;
                    let div_y = content_top + base_row as f32 * cell_h;
                    let div_h = rows as f32 * cell_h;
                    out.push(DividerInfo {
                        id: *divider_id,
                        phys: PaneDivider { x: div_x, y: div_y, width: line_w, height: div_h, color },
                        direction: PaneSplitDirection::Vertical,
                    });
                    let right_cols = cols.saturating_sub(left_cols + 1);
                    dividers_node(&children[0], base_col, base_row, left_cols, rows, cell_w, cell_h, phys_pad, content_top, scale, out);
                    dividers_node(&children[1], base_col + left_cols + 1, base_row, right_cols, rows, cell_w, cell_h, phys_pad, content_top, scale, out);
                }
                PaneSplitDirection::Horizontal => {
                    let top_rows = top_rows_from_ratio(*ratio, rows);
                    let div_x = phys_pad + base_col as f32 * cell_w;
                    let div_y = content_top + phys_pad + (base_row + top_rows) as f32 * cell_h;
                    let div_w = cols as f32 * cell_w;
                    out.push(DividerInfo {
                        id: *divider_id,
                        phys: PaneDivider { x: div_x, y: div_y, width: div_w, height: line_w, color },
                        direction: PaneSplitDirection::Horizontal,
                    });
                    let bot_rows = rows.saturating_sub(top_rows + 1);
                    dividers_node(&children[0], base_col, base_row, cols, top_rows, cell_w, cell_h, phys_pad, content_top, scale, out);
                    dividers_node(&children[1], base_col, base_row + top_rows + 1, cols, bot_rows, cell_w, cell_h, phys_pad, content_top, scale, out);
                }
            }
        }
    }
}

fn find_leaf_node(node: &PaneNode, id: usize) -> Option<&TerminalPane> {
    match node {
        PaneNode::Leaf { pane, id: leaf_id } => if *leaf_id == id { Some(pane) } else { None },
        PaneNode::Split { children, .. } => {
            find_leaf_node(&children[0], id).or_else(|| find_leaf_node(&children[1], id))
        }
        PaneNode::Tombstone => None,
    }
}

fn find_leaf_mut_node(node: &mut PaneNode, id: usize) -> Option<&mut TerminalPane> {
    match node {
        PaneNode::Leaf { pane, id: leaf_id } => if *leaf_id == id { Some(pane) } else { None },
        PaneNode::Split { children, .. } => {
            let [left, right] = children.as_mut();
            find_leaf_mut_node(left, id).or_else(|| find_leaf_mut_node(right, id))
        }
        PaneNode::Tombstone => None,
    }
}

fn collect_leaf_ids(node: &PaneNode, out: &mut Vec<usize>) {
    match node {
        PaneNode::Leaf { id, .. } => out.push(*id),
        PaneNode::Split { children, .. } => {
            collect_leaf_ids(&children[0], out);
            collect_leaf_ids(&children[1], out);
        }
        PaneNode::Tombstone => {}
    }
}

/// Returns `true` if target was found and split was performed.
fn split_node(
    node: &mut PaneNode,
    target_id: usize,
    direction: PaneSplitDirection,
    new_pane: &mut Option<TerminalPane>,
    new_pane_id: usize,
    divider_id: usize,
) -> bool {
    match node {
        PaneNode::Leaf { id, .. } if *id == target_id => {
            let pane = new_pane.take().expect("split_node: pane consumed prematurely");
            // SAFETY: we immediately overwrite `node` with a new valid PaneNode;
            // the old leaf is placed into the Split's children and not used again here.
            let old_leaf = unsafe { std::ptr::read(node) };
            let new_split = PaneNode::Split {
                direction,
                ratio: 0.5,
                children: Box::new([old_leaf, PaneNode::Leaf { pane, id: new_pane_id }]),
                divider_id,
            };
            unsafe { std::ptr::write(node, new_split) };
            true
        }
        PaneNode::Leaf { .. } | PaneNode::Tombstone => false,
        PaneNode::Split { children, .. } => {
            let [left, right] = children.as_mut();
            if split_node(left, target_id, direction, new_pane, new_pane_id, divider_id) {
                return true;
            }
            split_node(right, target_id, direction, new_pane, new_pane_id, divider_id)
        }
    }
}

fn remove_node(node: &mut PaneNode, target_id: usize, active_id: &mut usize) -> bool {
    let PaneNode::Split { children, .. } = node else { return false; };
    let [left, right] = children.as_mut();

    let left_is_target = matches!(left, PaneNode::Leaf { id, .. } if *id == target_id);
    let right_is_target = matches!(right, PaneNode::Leaf { id, .. } if *id == target_id);

    if left_is_target || right_is_target {
        let survivor = if left_is_target {
            let mut ids = Vec::new();
            collect_leaf_ids(right, &mut ids);
            if let Some(&id) = ids.first() { *active_id = id; }
            std::mem::replace(right, PaneNode::Tombstone)
        } else {
            let mut ids = Vec::new();
            collect_leaf_ids(left, &mut ids);
            if let Some(&id) = ids.first() { *active_id = id; }
            std::mem::replace(left, PaneNode::Tombstone)
        };
        *node = survivor;
        return true;
    }

    // Recurse — re-borrow children after the match guard above
    let PaneNode::Split { children, .. } = node else { return false; };
    let [left, right] = children.as_mut();
    remove_node(left, target_id, active_id) || remove_node(right, target_id, active_id)
}

fn adjust_ratio_node(
    node: &mut PaneNode,
    target_divider_id: usize,
    delta_px: f32,
    cell_w: f32,
    cell_h: f32,
    cols: usize,
    rows: usize,
) -> bool {
    match node {
        PaneNode::Leaf { .. } | PaneNode::Tombstone => false,
        PaneNode::Split { divider_id, direction, ratio, children } => {
            if *divider_id == target_divider_id {
                match direction {
                    PaneSplitDirection::Vertical => {
                        let total_px = cols.saturating_sub(1) as f32 * cell_w;
                        if total_px > 0.0 {
                            *ratio = (*ratio + delta_px / total_px).clamp(0.1, 0.9);
                        }
                    }
                    PaneSplitDirection::Horizontal => {
                        let total_px = rows.saturating_sub(1) as f32 * cell_h;
                        if total_px > 0.0 {
                            *ratio = (*ratio + delta_px / total_px).clamp(0.1, 0.9);
                        }
                    }
                }
                return true;
            }
            let (lc, lr, rc, rr) = match direction {
                PaneSplitDirection::Vertical => {
                    let lc = left_cols_from_ratio(*ratio, cols);
                    let rc = cols.saturating_sub(lc + 1);
                    (lc, rows, rc, rows)
                }
                PaneSplitDirection::Horizontal => {
                    let tr = top_rows_from_ratio(*ratio, rows);
                    let br = rows.saturating_sub(tr + 1);
                    (cols, tr, cols, br)
                }
            };
            let [left, right] = children.as_mut();
            adjust_ratio_node(left, target_divider_id, delta_px, cell_w, cell_h, lc, lr)
                || adjust_ratio_node(right, target_divider_id, delta_px, cell_w, cell_h, rc, rr)
        }
    }
}

// ── layout arithmetic helpers ─────────────────────────────────────────────────

fn left_cols_from_ratio(ratio: f32, cols: usize) -> usize {
    let usable = cols.saturating_sub(1);
    let lc = (usable as f32 * ratio).round() as usize;
    lc.max(1).min(usable.saturating_sub(1).max(1))
}

fn top_rows_from_ratio(ratio: f32, rows: usize) -> usize {
    let usable = rows.saturating_sub(1);
    let tr = (usable as f32 * ratio).round() as usize;
    tr.max(1).min(usable.saturating_sub(1).max(1))
}
