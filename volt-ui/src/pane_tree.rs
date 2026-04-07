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

/// Result of a `PaneTree::remove` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveResult {
    /// Pane found and removed; tree restructured.
    Removed,
    /// `target_id` was the only pane — caller should close the tab.
    RemovedLastLeaf,
    /// `target_id` not found in tree (stale/duplicate event). Safe to ignore.
    NotFound,
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
}

pub struct PaneTree {
    // Invariant: always `Some` after construction; `None` only transiently inside `remove`.
    root: Option<PaneNode>,
    pub active_id: usize,
    next_id: usize,
}

impl PaneTree {
    pub fn new(pane: TerminalPane) -> Self {
        Self {
            root: Some(PaneNode::Leaf { pane, id: 0 }),
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
        if let Some(root) = &mut self.root {
            if split_node(root, target_id, direction, &mut pane_opt, new_pane_id, divider_id) {
                self.active_id = new_pane_id;
            }
        }
    }

    pub fn remove(&mut self, target_id: usize) -> RemoveResult {
        match self.root.as_ref() {
            None => RemoveResult::NotFound,
            Some(PaneNode::Leaf { id, .. }) if *id == target_id => RemoveResult::RemovedLastLeaf,
            Some(PaneNode::Leaf { .. }) => RemoveResult::NotFound,
            Some(PaneNode::Split { .. }) => {
                let old_root = self.root.take().expect("root is Some");
                let (new_root, found) = remove_node_bv(old_root, target_id, &mut self.active_id);
                self.root = Some(new_root);
                if found { RemoveResult::Removed } else { RemoveResult::NotFound }
            }
        }
    }

    pub fn layout(&self, cols: usize, rows: usize) -> Vec<PaneRect> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            layout_node(root, 0, 0, cols, rows, &mut out);
        }
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
        hover_id: Option<usize>,
        drag_id: Option<usize>,
    ) -> Vec<DividerInfo> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            dividers_node(root, 0, 0, cols, rows, cell_w, cell_h, phys_pad, content_top, scale, hover_id, drag_id, &mut out);
        }
        out
    }

    pub fn find_leaf(&self, id: usize) -> Option<&TerminalPane> {
        self.root.as_ref().and_then(|r| find_leaf_node(r, id))
    }

    pub fn find_leaf_mut(&mut self, id: usize) -> Option<&mut TerminalPane> {
        self.root.as_mut().and_then(|r| find_leaf_mut_node(r, id))
    }

    pub fn leaf_ids(&self) -> Vec<usize> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            collect_leaf_ids(root, &mut out);
        }
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
        if let Some(root) = &mut self.root {
            adjust_ratio_node(root, divider_id, delta_px, cell_w, cell_h, total_cols, total_rows);
        }
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
            out.push(PaneRect { id: *id, col, row, cols: cols.max(1), rows: rows.max(1) });
        }
        PaneNode::Split { direction, ratio, children, .. } => {
            match direction {
                PaneSplitDirection::Vertical => {
                    let left_cols = left_cols_from_ratio(*ratio, cols);
                    let right_cols = cols.saturating_sub(left_cols + 1).max(1);
                    layout_node(&children[0], col, row, left_cols, rows, out);
                    layout_node(&children[1], col + left_cols + 1, row, right_cols, rows, out);
                }
                PaneSplitDirection::Horizontal => {
                    let top_rows = top_rows_from_ratio(*ratio, rows);
                    let bot_rows = rows.saturating_sub(top_rows + 1).max(1);
                    layout_node(&children[0], col, row, cols, top_rows, out);
                    layout_node(&children[1], col, row + top_rows + 1, cols, bot_rows, out);
                }
            }
        }
    }
}

fn dividers_node(
    node: &PaneNode,
    base_col: usize, base_row: usize,
    cols: usize, rows: usize,
    cell_w: f32, cell_h: f32,
    phys_pad: f32, content_top: f32,
    scale: f32,
    hover_id: Option<usize>,
    drag_id: Option<usize>,
    out: &mut Vec<DividerInfo>,
) {
    match node {
        PaneNode::Leaf { .. } => {}
        PaneNode::Split { direction, ratio, children, divider_id } => {
            let line_w = scale.max(1.0);
            // Color priority: drag (blue accent) > hover (lighter gray) > idle (dark gray)
            let color = if drag_id == Some(*divider_id) {
                [0.29_f32, 0.56, 0.85, 1.0]   // drag: blue accent
            } else if hover_id == Some(*divider_id) {
                [0.50_f32, 0.50, 0.50, 1.0]   // hover: lighter gray
            } else {
                [0.24_f32, 0.24, 0.24, 1.0]   // idle: dark gray (crisp, Ghostty-like)
            };
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
                    let right_cols = cols.saturating_sub(left_cols + 1).max(1);
                    dividers_node(&children[0], base_col, base_row, left_cols, rows, cell_w, cell_h, phys_pad, content_top, scale, hover_id, drag_id, out);
                    dividers_node(&children[1], base_col + left_cols + 1, base_row, right_cols, rows, cell_w, cell_h, phys_pad, content_top, scale, hover_id, drag_id, out);
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
                    let bot_rows = rows.saturating_sub(top_rows + 1).max(1);
                    dividers_node(&children[0], base_col, base_row, cols, top_rows, cell_w, cell_h, phys_pad, content_top, scale, hover_id, drag_id, out);
                    dividers_node(&children[1], base_col, base_row + top_rows + 1, cols, bot_rows, cell_w, cell_h, phys_pad, content_top, scale, hover_id, drag_id, out);
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
    }
}

fn find_leaf_mut_node(node: &mut PaneNode, id: usize) -> Option<&mut TerminalPane> {
    match node {
        PaneNode::Leaf { pane, id: leaf_id } => if *leaf_id == id { Some(pane) } else { None },
        PaneNode::Split { children, .. } => {
            let [left, right] = children.as_mut();
            find_leaf_mut_node(left, id).or_else(|| find_leaf_mut_node(right, id))
        }
    }
}

fn collect_leaf_ids(node: &PaneNode, out: &mut Vec<usize>) {
    match node {
        PaneNode::Leaf { id, .. } => out.push(*id),
        PaneNode::Split { children, .. } => {
            collect_leaf_ids(&children[0], out);
            collect_leaf_ids(&children[1], out);
        }
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
            // SAFETY: we immediately overwrite `node` before the old value can be observed
            // again; the old Leaf is moved into the new Split's children array.
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
        PaneNode::Leaf { .. } => false,
        PaneNode::Split { children, .. } => {
            let [left, right] = children.as_mut();
            if split_node(left, target_id, direction, new_pane, new_pane_id, divider_id) {
                return true;
            }
            split_node(right, target_id, direction, new_pane, new_pane_id, divider_id)
        }
    }
}

/// Removes `target_id` from the tree by consuming the node and reconstructing
/// the tree without any placeholders.  Returns `(new_node, was_found)`.
fn remove_node_bv(node: PaneNode, target_id: usize, active_id: &mut usize) -> (PaneNode, bool) {
    match node {
        // A bare Leaf at this call level means the public API's guard missed it — treat as NotFound.
        PaneNode::Leaf { .. } => (node, false),
        PaneNode::Split { direction, ratio, children, divider_id } => {
            let [left, right] = *children;

            let left_is_target = matches!(&left, PaneNode::Leaf { id, .. } if *id == target_id);
            let right_is_target = matches!(&right, PaneNode::Leaf { id, .. } if *id == target_id);

            if left_is_target {
                // Only update active_id if it pointed at the removed pane.
                if *active_id == target_id {
                    let mut ids = Vec::new();
                    collect_leaf_ids(&right, &mut ids);
                    if let Some(&id) = ids.first() { *active_id = id; }
                }
                return (right, true);
            }
            if right_is_target {
                if *active_id == target_id {
                    let mut ids = Vec::new();
                    collect_leaf_ids(&left, &mut ids);
                    if let Some(&id) = ids.first() { *active_id = id; }
                }
                return (left, true);
            }

            // Try left subtree first.
            let (new_left, found_left) = remove_node_bv(left, target_id, active_id);
            if found_left {
                return (PaneNode::Split {
                    direction, ratio, divider_id,
                    children: Box::new([new_left, right]),
                }, true);
            }
            // Try right subtree.
            let (new_right, found_right) = remove_node_bv(right, target_id, active_id);
            (PaneNode::Split {
                direction, ratio, divider_id,
                children: Box::new([new_left, new_right]),
            }, found_right)
        }
    }
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
        PaneNode::Leaf { .. } => false,
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
                if ratio.is_nan() { *ratio = 0.5; }
                return true;
            }
            let (lc, lr, rc, rr) = match direction {
                PaneSplitDirection::Vertical => {
                    let lc = left_cols_from_ratio(*ratio, cols);
                    let rc = cols.saturating_sub(lc + 1).max(1);
                    (lc, rows, rc, rows)
                }
                PaneSplitDirection::Horizontal => {
                    let tr = top_rows_from_ratio(*ratio, rows);
                    let br = rows.saturating_sub(tr + 1).max(1);
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
    let lc = (usable as f32 * ratio.clamp(0.1, 0.9)).round() as usize;
    lc.max(1).min(usable.saturating_sub(1).max(1))
}

fn top_rows_from_ratio(ratio: f32, rows: usize) -> usize {
    let usable = rows.saturating_sub(1);
    let tr = (usable as f32 * ratio.clamp(0.1, 0.9)).round() as usize;
    tr.max(1).min(usable.saturating_sub(1).max(1))
}
