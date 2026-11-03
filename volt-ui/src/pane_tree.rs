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

/// Outcome of a topology-only operation. Invalid IDs never mutate the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RearrangeResult {
    Changed,
    Unchanged,
    NotFound { pane_id: usize },
}

pub enum PaneNode<P = TerminalPane> {
    Leaf {
        pane: P,
        id: usize,
    },
    Split {
        direction: PaneSplitDirection,
        ratio: f32,
        children: Box<[PaneNode<P>; 2]>,
        divider_id: usize,
    },
}

pub struct PaneTree<P = TerminalPane> {
    // Invariant: always `Some` after construction; `None` only transiently inside consuming topology edits.
    root: Option<PaneNode<P>>,
    pub active_id: usize,
    next_id: usize,
}

impl<P> PaneTree<P> {
    pub fn new(pane: P) -> Self {
        Self {
            root: Some(PaneNode::Leaf { pane, id: 0 }),
            active_id: 0,
            next_id: 1,
        }
    }

    /// Exchange complete leaves, including their stable IDs. Focus stays with
    /// the same process; split directions, ratios and divider IDs do not change.
    pub fn swap_panes(&mut self, first_id: usize, second_id: usize) -> RearrangeResult {
        rearrange_root(&mut self.root, first_id, second_id, None)
    }

    pub fn swap_active_pane(&mut self, target_id: usize) -> RearrangeResult {
        self.swap_panes(self.active_id, target_id)
    }

    /// Detach an existing leaf and place it beside another existing leaf.
    /// The old parent collapses; its divider ID is reused at the destination
    /// with a 50/50 ratio. Other splits retain their IDs and ratios. No pane
    /// is created, closed, resized or focused by this operation.
    pub fn move_pane(
        &mut self,
        source_id: usize,
        target_id: usize,
        direction: PaneSplitDirection,
        insert_before: bool,
    ) -> RearrangeResult {
        rearrange_root(
            &mut self.root,
            source_id,
            target_id,
            Some((direction, insert_before)),
        )
    }

    pub fn move_active_pane(
        &mut self,
        target_id: usize,
        direction: PaneSplitDirection,
        insert_before: bool,
    ) -> RearrangeResult {
        self.move_pane(self.active_id, target_id, direction, insert_before)
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
                if found {
                    RemoveResult::Removed
                } else {
                    RemoveResult::NotFound
                }
            }
        }
    }

    pub fn layout(&self, cols: usize, rows: usize) -> Vec<PaneRect> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            let (minimum_cols, minimum_rows) = minimum_size_node(root);
            if cols < minimum_cols || rows < minimum_rows {
                // The tree cannot fit without overlapping panes. Keep the
                // focused pane usable until the window is large enough again.
                out.push(PaneRect {
                    id: self.active_id,
                    col: 0,
                    row: 0,
                    cols: cols.max(1),
                    rows: rows.max(1),
                });
            } else {
                layout_node(root, 0, 0, cols, rows, &mut out);
            }
        }
        out
    }

    pub fn leaf_ids(&self) -> Vec<usize> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            collect_leaf_ids(root, &mut out);
        }
        out
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

impl PaneTree {
    pub(crate) fn saved_layout(&self) -> crate::session::Node {
        fn node(n: &PaneNode) -> crate::session::Node {
            match n {
                PaneNode::Leaf { pane, .. } => crate::session::Node::Pane {
                    cwd: pane.local_cwd(),
                },
                PaneNode::Split {
                    direction,
                    ratio,
                    children,
                    ..
                } => crate::session::Node::Split {
                    vertical: matches!(direction, PaneSplitDirection::Vertical),
                    ratio: ratio.clamp(0.1, 0.9),
                    children: Box::new([node(&children[0]), node(&children[1])]),
                },
            }
        }
        node(self.root.as_ref().expect("pane tree root"))
    }

    pub(crate) fn restore_layout(
        layout: &crate::session::Node,
        spawn: &mut impl FnMut(Option<&std::path::Path>) -> anyhow::Result<TerminalPane>,
    ) -> anyhow::Result<Self> {
        fn restore(
            n: &crate::session::Node,
            id: &mut usize,
            spawn: &mut impl FnMut(Option<&std::path::Path>) -> anyhow::Result<TerminalPane>,
        ) -> anyhow::Result<PaneNode> {
            let current = *id;
            *id += 1;
            Ok(match n {
                crate::session::Node::Pane { cwd } => PaneNode::Leaf {
                    id: current,
                    pane: spawn(cwd.as_deref().filter(|p| p.is_dir()))?,
                },
                crate::session::Node::Split {
                    vertical,
                    ratio,
                    children,
                } => PaneNode::Split {
                    direction: if *vertical {
                        PaneSplitDirection::Vertical
                    } else {
                        PaneSplitDirection::Horizontal
                    },
                    ratio: *ratio,
                    divider_id: current,
                    children: Box::new([
                        restore(&children[0], id, spawn)?,
                        restore(&children[1], id, spawn)?,
                    ]),
                },
            })
        }
        let mut next_id = 0;
        let root = restore(layout, &mut next_id, spawn)?;
        let mut leaves = Vec::new();
        collect_leaf_ids(&root, &mut leaves);
        Ok(Self {
            root: Some(root),
            active_id: leaves[0],
            next_id,
        })
    }

    pub fn split(
        &mut self,
        target_id: usize,
        direction: PaneSplitDirection,
        new_pane: TerminalPane,
    ) {
        self.split_positioned(target_id, direction, new_pane, false);
    }

    /// Split `target_id`, placing the new pane before (left/above) the
    /// original when `insert_before` is true, or after (right/below, the
    /// default `split` behavior) otherwise.
    pub fn split_positioned(
        &mut self,
        target_id: usize,
        direction: PaneSplitDirection,
        new_pane: TerminalPane,
        insert_before: bool,
    ) {
        let new_pane_id = self.next_id;
        self.next_id += 1;
        let divider_id = self.next_id;
        self.next_id += 1;
        let mut pane_opt = Some(new_pane);
        if let Some(root) = &mut self.root {
            if split_node(
                root,
                target_id,
                direction,
                &mut pane_opt,
                new_pane_id,
                divider_id,
                insert_before,
            ) {
                self.active_id = new_pane_id;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn dividers_with_ids(
        &self,
        cols: usize,
        rows: usize,
        cell_w: f32,
        cell_h: f32,
        phys_pad: f32,
        content_top: f32,
        phys_right: f32,
        phys_bottom: f32,
        scale: f32,
        hover_id: Option<usize>,
        drag_id: Option<usize>,
        divider_opacity: f32,
    ) -> Vec<DividerInfo> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            let (minimum_cols, minimum_rows) = minimum_size_node(root);
            if cols < minimum_cols || rows < minimum_rows {
                return out;
            }
            dividers_node(
                root,
                0,
                0,
                cols,
                rows,
                cell_w,
                cell_h,
                phys_pad,
                content_top,
                phys_right,
                phys_bottom,
                scale,
                hover_id,
                drag_id,
                divider_opacity,
                &mut out,
            );
        }
        out
    }

    pub fn find_leaf(&self, id: usize) -> Option<&TerminalPane> {
        self.root.as_ref().and_then(|r| find_leaf_node(r, id))
    }

    pub fn find_leaf_mut(&mut self, id: usize) -> Option<&mut TerminalPane> {
        self.root.as_mut().and_then(|r| find_leaf_mut_node(r, id))
    }

    pub fn get_ratio(&self, divider_id: usize) -> Option<f32> {
        self.root
            .as_ref()
            .and_then(|r| get_ratio_node(r, divider_id))
    }

    /// Apply an **absolute** ratio from `start_ratio + total_delta_px / local_span_px`.
    /// Uses local (sub-tree) dimensions, so nested splits are handled correctly.
    #[allow(clippy::too_many_arguments)]
    pub fn set_ratio_from_drag(
        &mut self,
        divider_id: usize,
        start_ratio: f32,
        delta_px: f32,
        cell_w: f32,
        cell_h: f32,
        total_cols: usize,
        total_rows: usize,
    ) {
        if let Some(root) = &mut self.root {
            set_ratio_from_drag_node(
                root,
                divider_id,
                start_ratio,
                delta_px,
                cell_w,
                cell_h,
                total_cols,
                total_rows,
            );
        }
    }

    /// Zero-allocation, borrow-safe leaf iteration with mutation.
    pub fn for_each_leaf_mut(&mut self, f: &mut impl FnMut(usize, &mut TerminalPane)) {
        if let Some(root) = &mut self.root {
            for_each_leaf_mut_node(root, f);
        }
    }

    pub fn active_pane(&self) -> Option<&TerminalPane> {
        self.find_leaf(self.active_id)
    }

    pub fn active_pane_mut(&mut self) -> Option<&mut TerminalPane> {
        self.find_leaf_mut(self.active_id)
    }
}

// ── recursive helpers ─────────────────────────────────────────────────────────

// Generic leaf payloads let tests exercise the exact production topology code
// with owned, non-Clone sentinels instead of spawning shells.
fn leaf_path<P>(node: &PaneNode<P>, id: usize, path: &mut Vec<usize>) -> bool {
    match node {
        PaneNode::Leaf { id: leaf_id, .. } => *leaf_id == id,
        PaneNode::Split { children, .. } => {
            for index in 0..2 {
                path.push(index);
                if leaf_path(&children[index], id, path) {
                    return true;
                }
                path.pop();
            }
            false
        }
    }
}

fn node_at_path_mut<'a, P>(mut node: &'a mut PaneNode<P>, path: &[usize]) -> &'a mut PaneNode<P> {
    for &index in path {
        let PaneNode::Split { children, .. } = node else {
            unreachable!("validated leaf path")
        };
        node = &mut children[index];
    }
    node
}

fn rearrange_root<P>(
    root: &mut Option<PaneNode<P>>,
    source: usize,
    target: usize,
    placement: Option<(PaneSplitDirection, bool)>,
) -> RearrangeResult {
    let mut source_path = Vec::new();
    let mut target_path = Vec::new();
    for (id, path) in [(source, &mut source_path), (target, &mut target_path)] {
        if !root.as_ref().is_some_and(|node| leaf_path(node, id, path)) {
            return RearrangeResult::NotFound { pane_id: id };
        }
    }
    if source == target {
        return RearrangeResult::Unchanged;
    }
    if let Some((direction, before)) = placement {
        let (remaining, leaf, divider_id) =
            detach_leaf(root.take().expect("validated root"), &source_path);
        *root = Some(insert_leaf(
            remaining, target, leaf, divider_id, direction, before,
        ));
    } else {
        let common = source_path
            .iter()
            .zip(&target_path)
            .take_while(|(a, b)| a == b)
            .count();
        let ancestor = node_at_path_mut(
            root.as_mut().expect("validated root"),
            &source_path[..common],
        );
        let PaneNode::Split { children, .. } = ancestor else {
            unreachable!("distinct leaves")
        };
        let [left, right] = children.as_mut();
        let (source_branch, target_branch) = if source_path[common] == 0 {
            (left, right)
        } else {
            (right, left)
        };
        std::mem::swap(
            node_at_path_mut(source_branch, &source_path[common + 1..]),
            node_at_path_mut(target_branch, &target_path[common + 1..]),
        );
    }
    RearrangeResult::Changed
}

/// Return the surviving tree, detached leaf and collapsed parent's divider.
fn detach_leaf<P>(node: PaneNode<P>, path: &[usize]) -> (PaneNode<P>, PaneNode<P>, usize) {
    let PaneNode::Split {
        direction,
        ratio,
        children,
        divider_id,
    } = node
    else {
        unreachable!("distinct validated leaves require a parent")
    };
    let [left, right] = *children;
    let (branch, sibling) = if path[0] == 0 {
        (left, right)
    } else {
        (right, left)
    };
    if path.len() == 1 {
        return (sibling, branch, divider_id);
    }
    let (remaining, leaf, collapsed_divider) = detach_leaf(branch, &path[1..]);
    let children = if path[0] == 0 {
        [remaining, sibling]
    } else {
        [sibling, remaining]
    };
    (
        PaneNode::Split {
            direction,
            ratio,
            children: Box::new(children),
            divider_id,
        },
        leaf,
        collapsed_divider,
    )
}

fn insert_leaf<P>(
    node: PaneNode<P>,
    target: usize,
    leaf: PaneNode<P>,
    divider_id: usize,
    direction: PaneSplitDirection,
    before: bool,
) -> PaneNode<P> {
    match node {
        PaneNode::Leaf { id, .. } if id == target => PaneNode::Split {
            direction,
            ratio: 0.5,
            divider_id,
            children: Box::new(ordered_children(node, leaf, before)),
        },
        PaneNode::Split {
            direction: old_direction,
            ratio,
            children,
            divider_id: old_divider,
        } => {
            let [left, right] = *children;
            let mut path = Vec::new();
            let children = if leaf_path(&left, target, &mut path) {
                [
                    insert_leaf(left, target, leaf, divider_id, direction, before),
                    right,
                ]
            } else {
                [
                    left,
                    insert_leaf(right, target, leaf, divider_id, direction, before),
                ]
            };
            PaneNode::Split {
                direction: old_direction,
                ratio,
                children: Box::new(children),
                divider_id: old_divider,
            }
        }
        PaneNode::Leaf { .. } => unreachable!("validated destination survives detach"),
    }
}

fn minimum_size_for_split(
    direction: PaneSplitDirection,
    first: (usize, usize),
    second: (usize, usize),
) -> (usize, usize) {
    match direction {
        PaneSplitDirection::Vertical => (
            first.0.saturating_add(second.0).saturating_add(1),
            first.1.max(second.1),
        ),
        PaneSplitDirection::Horizontal => (
            first.0.max(second.0),
            first.1.saturating_add(second.1).saturating_add(1),
        ),
    }
}

fn minimum_size_node<P>(node: &PaneNode<P>) -> (usize, usize) {
    match node {
        PaneNode::Leaf { .. } => (1, 1),
        PaneNode::Split {
            direction,
            children,
            ..
        } => minimum_size_for_split(
            *direction,
            minimum_size_node(&children[0]),
            minimum_size_node(&children[1]),
        ),
    }
}

fn layout_node<P>(
    node: &PaneNode<P>,
    col: usize,
    row: usize,
    cols: usize,
    rows: usize,
    out: &mut Vec<PaneRect>,
) {
    match node {
        PaneNode::Leaf { id, .. } => {
            out.push(PaneRect {
                id: *id,
                col,
                row,
                cols: cols.max(1),
                rows: rows.max(1),
            });
        }
        PaneNode::Split {
            direction,
            ratio,
            children,
            ..
        } => match direction {
            PaneSplitDirection::Vertical => {
                let left_cols = first_extent(children, PaneSplitDirection::Vertical, *ratio, cols);
                let right_cols = cols.saturating_sub(left_cols + 1).max(1);
                layout_node(&children[0], col, row, left_cols, rows, out);
                layout_node(
                    &children[1],
                    col + left_cols + 1,
                    row,
                    right_cols,
                    rows,
                    out,
                );
            }
            PaneSplitDirection::Horizontal => {
                let top_rows = first_extent(children, PaneSplitDirection::Horizontal, *ratio, rows);
                let bot_rows = rows.saturating_sub(top_rows + 1).max(1);
                layout_node(&children[0], col, row, cols, top_rows, out);
                layout_node(&children[1], col, row + top_rows + 1, cols, bot_rows, out);
            }
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn dividers_node(
    node: &PaneNode,
    base_col: usize,
    base_row: usize,
    cols: usize,
    rows: usize,
    cell_w: f32,
    cell_h: f32,
    phys_pad: f32,
    content_top: f32,
    // Physical extents for this subtree — used to extend dividers to fill
    // leftover pixels that don't fit in a whole cell.
    phys_right: f32,
    phys_bottom: f32,
    scale: f32,
    hover_id: Option<usize>,
    drag_id: Option<usize>,
    divider_opacity: f32,
    out: &mut Vec<DividerInfo>,
) {
    match node {
        PaneNode::Leaf { .. } => {}
        PaneNode::Split {
            direction,
            ratio,
            children,
            divider_id,
        } => {
            let line_w = scale.max(1.0);
            // Colors blend over the terminal background via alpha compositing.
            let color = if drag_id == Some(*divider_id) {
                [0.29_f32, 0.56, 0.85, 0.75]
            } else if hover_id == Some(*divider_id) {
                [1.00_f32, 1.00, 1.00, (divider_opacity * 3.0).min(1.0)]
            } else {
                [1.00_f32, 1.00, 1.00, divider_opacity.clamp(0.0, 1.0)]
            };
            match direction {
                PaneSplitDirection::Vertical => {
                    let left_cols =
                        first_extent(children, PaneSplitDirection::Vertical, *ratio, cols);
                    let div_x = phys_pad + (base_col + left_cols) as f32 * cell_w;
                    let div_y = content_top + base_row as f32 * cell_h;
                    // Extend to phys_bottom to fill sub-cell remainder at the bottom edge.
                    let div_h = phys_bottom - div_y;
                    out.push(DividerInfo {
                        id: *divider_id,
                        phys: PaneDivider {
                            x: div_x,
                            y: div_y,
                            width: line_w,
                            height: div_h,
                            color,
                        },
                        direction: PaneSplitDirection::Vertical,
                    });
                    let right_cols = cols.saturating_sub(left_cols + 1).max(1);
                    // Left child's right boundary is the divider line; right child keeps parent's right.
                    dividers_node(
                        &children[0],
                        base_col,
                        base_row,
                        left_cols,
                        rows,
                        cell_w,
                        cell_h,
                        phys_pad,
                        content_top,
                        div_x,
                        phys_bottom,
                        scale,
                        hover_id,
                        drag_id,
                        divider_opacity,
                        out,
                    );
                    dividers_node(
                        &children[1],
                        base_col + left_cols + 1,
                        base_row,
                        right_cols,
                        rows,
                        cell_w,
                        cell_h,
                        phys_pad,
                        content_top,
                        phys_right,
                        phys_bottom,
                        scale,
                        hover_id,
                        drag_id,
                        divider_opacity,
                        out,
                    );
                }
                PaneSplitDirection::Horizontal => {
                    let top_rows =
                        first_extent(children, PaneSplitDirection::Horizontal, *ratio, rows);
                    let div_x = phys_pad + base_col as f32 * cell_w;
                    let div_y = content_top + (base_row + top_rows) as f32 * cell_h;
                    // Extend to phys_right to fill sub-cell remainder at the right edge.
                    let div_w = phys_right - div_x;
                    out.push(DividerInfo {
                        id: *divider_id,
                        phys: PaneDivider {
                            x: div_x,
                            y: div_y,
                            width: div_w,
                            height: line_w,
                            color,
                        },
                        direction: PaneSplitDirection::Horizontal,
                    });
                    let bot_rows = rows.saturating_sub(top_rows + 1).max(1);
                    // Top child's bottom boundary is the divider line; bottom child keeps parent's bottom.
                    dividers_node(
                        &children[0],
                        base_col,
                        base_row,
                        cols,
                        top_rows,
                        cell_w,
                        cell_h,
                        phys_pad,
                        content_top,
                        phys_right,
                        div_y,
                        scale,
                        hover_id,
                        drag_id,
                        divider_opacity,
                        out,
                    );
                    dividers_node(
                        &children[1],
                        base_col,
                        base_row + top_rows + 1,
                        cols,
                        bot_rows,
                        cell_w,
                        cell_h,
                        phys_pad,
                        content_top,
                        phys_right,
                        phys_bottom,
                        scale,
                        hover_id,
                        drag_id,
                        divider_opacity,
                        out,
                    );
                }
            }
        }
    }
}

fn find_leaf_node(node: &PaneNode, id: usize) -> Option<&TerminalPane> {
    match node {
        PaneNode::Leaf { pane, id: leaf_id } => {
            if *leaf_id == id {
                Some(pane)
            } else {
                None
            }
        }
        PaneNode::Split { children, .. } => {
            find_leaf_node(&children[0], id).or_else(|| find_leaf_node(&children[1], id))
        }
    }
}

fn find_leaf_mut_node(node: &mut PaneNode, id: usize) -> Option<&mut TerminalPane> {
    match node {
        PaneNode::Leaf { pane, id: leaf_id } => {
            if *leaf_id == id {
                Some(pane)
            } else {
                None
            }
        }
        PaneNode::Split { children, .. } => {
            let [left, right] = children.as_mut();
            find_leaf_mut_node(left, id).or_else(|| find_leaf_mut_node(right, id))
        }
    }
}

fn collect_leaf_ids<P>(node: &PaneNode<P>, out: &mut Vec<usize>) {
    match node {
        PaneNode::Leaf { id, .. } => out.push(*id),
        PaneNode::Split { children, .. } => {
            collect_leaf_ids(&children[0], out);
            collect_leaf_ids(&children[1], out);
        }
    }
}

/// Returns `true` if target was found and split was performed.
#[allow(clippy::too_many_arguments)]
fn split_node(
    node: &mut PaneNode,
    target_id: usize,
    direction: PaneSplitDirection,
    new_pane: &mut Option<TerminalPane>,
    new_pane_id: usize,
    divider_id: usize,
    insert_before: bool,
) -> bool {
    match node {
        PaneNode::Leaf { id, .. } if *id == target_id => {
            let pane = new_pane
                .take()
                .expect("split_node: pane consumed prematurely");
            // SAFETY: we immediately overwrite `node` before the old value can be observed
            // again; the old Leaf is moved into the new Split's children array.
            let old_leaf = unsafe { std::ptr::read(node) };
            let new_leaf = PaneNode::Leaf {
                pane,
                id: new_pane_id,
            };
            let new_split = PaneNode::Split {
                direction,
                ratio: 0.5,
                children: Box::new(ordered_children(old_leaf, new_leaf, insert_before)),
                divider_id,
            };
            unsafe { std::ptr::write(node, new_split) };
            true
        }
        PaneNode::Leaf { .. } => false,
        PaneNode::Split { children, .. } => {
            let [left, right] = children.as_mut();
            if split_node(
                left,
                target_id,
                direction,
                new_pane,
                new_pane_id,
                divider_id,
                insert_before,
            ) {
                return true;
            }
            split_node(
                right,
                target_id,
                direction,
                new_pane,
                new_pane_id,
                divider_id,
                insert_before,
            )
        }
    }
}

/// Removes `target_id` from the tree by consuming the node and reconstructing
/// the tree without any placeholders.  Returns `(new_node, was_found)`.
fn remove_node_bv<P>(
    node: PaneNode<P>,
    target_id: usize,
    active_id: &mut usize,
) -> (PaneNode<P>, bool) {
    match node {
        // A bare Leaf at this call level means the public API's guard missed it — treat as NotFound.
        PaneNode::Leaf { .. } => (node, false),
        PaneNode::Split {
            direction,
            ratio,
            children,
            divider_id,
        } => {
            let [left, right] = *children;

            let left_is_target = matches!(&left, PaneNode::Leaf { id, .. } if *id == target_id);
            let right_is_target = matches!(&right, PaneNode::Leaf { id, .. } if *id == target_id);

            if left_is_target {
                // Only update active_id if it pointed at the removed pane.
                if *active_id == target_id {
                    let mut ids = Vec::new();
                    collect_leaf_ids(&right, &mut ids);
                    if let Some(&id) = ids.first() {
                        *active_id = id;
                    }
                }
                return (right, true);
            }
            if right_is_target {
                if *active_id == target_id {
                    let mut ids = Vec::new();
                    collect_leaf_ids(&left, &mut ids);
                    if let Some(&id) = ids.first() {
                        *active_id = id;
                    }
                }
                return (left, true);
            }

            // Try left subtree first.
            let (new_left, found_left) = remove_node_bv(left, target_id, active_id);
            if found_left {
                return (
                    PaneNode::Split {
                        direction,
                        ratio,
                        divider_id,
                        children: Box::new([new_left, right]),
                    },
                    true,
                );
            }
            // Try right subtree.
            let (new_right, found_right) = remove_node_bv(right, target_id, active_id);
            (
                PaneNode::Split {
                    direction,
                    ratio,
                    divider_id,
                    children: Box::new([new_left, new_right]),
                },
                found_right,
            )
        }
    }
}

fn get_ratio_node(node: &PaneNode, target: usize) -> Option<f32> {
    match node {
        PaneNode::Leaf { .. } => None,
        PaneNode::Split {
            divider_id,
            ratio,
            children,
            ..
        } => {
            if *divider_id == target {
                return Some(*ratio);
            }
            get_ratio_node(&children[0], target).or_else(|| get_ratio_node(&children[1], target))
        }
    }
}

/// Set ratio absolutely: `new_ratio = (start_ratio + delta_px / local_span_px).clamp(0.1, 0.9)`.
/// Recurses with local sub-rect dimensions so nested splits are handled correctly.
#[allow(clippy::too_many_arguments)]
fn set_ratio_from_drag_node(
    node: &mut PaneNode,
    target_divider_id: usize,
    start_ratio: f32,
    delta_px: f32,
    cell_w: f32,
    cell_h: f32,
    cols: usize,
    rows: usize,
) -> bool {
    match node {
        PaneNode::Leaf { .. } => false,
        PaneNode::Split {
            divider_id,
            direction,
            ratio,
            children,
        } => {
            if *divider_id == target_divider_id {
                // Determine usable span (total cells minus the 1-cell divider).
                let usable = match direction {
                    PaneSplitDirection::Vertical => cols.saturating_sub(1),
                    PaneSplitDirection::Horizontal => rows.saturating_sub(1),
                };
                // Both children must get ≥ 1 cell — need at least 2 usable.
                if usable < 2 {
                    return true;
                }
                let span_px = usable as f32
                    * match direction {
                        PaneSplitDirection::Vertical => cell_w,
                        PaneSplitDirection::Horizontal => cell_h,
                    };
                if span_px > 0.0 {
                    // Cell-aware bounds: left ≥ 1 cell, right ≥ 1 cell.
                    let min_ratio = 1.0 / usable as f32;
                    let max_ratio = (usable - 1) as f32 / usable as f32;
                    *ratio = (start_ratio + delta_px / span_px).clamp(min_ratio, max_ratio);
                    if ratio.is_nan() {
                        *ratio = 0.5_f32.clamp(min_ratio, max_ratio);
                    }
                }
                return true;
            }
            let (lc, lr, rc, rr) = match direction {
                PaneSplitDirection::Vertical => {
                    let lc = first_extent(children, PaneSplitDirection::Vertical, *ratio, cols);
                    (lc, rows, cols.saturating_sub(lc + 1).max(1), rows)
                }
                PaneSplitDirection::Horizontal => {
                    let tr = first_extent(children, PaneSplitDirection::Horizontal, *ratio, rows);
                    (cols, tr, cols, rows.saturating_sub(tr + 1).max(1))
                }
            };
            let [left, right] = children.as_mut();
            set_ratio_from_drag_node(
                left,
                target_divider_id,
                start_ratio,
                delta_px,
                cell_w,
                cell_h,
                lc,
                lr,
            ) || set_ratio_from_drag_node(
                right,
                target_divider_id,
                start_ratio,
                delta_px,
                cell_w,
                cell_h,
                rc,
                rr,
            )
        }
    }
}

fn for_each_leaf_mut_node(node: &mut PaneNode, f: &mut impl FnMut(usize, &mut TerminalPane)) {
    match node {
        PaneNode::Leaf { id, pane } => f(*id, pane),
        PaneNode::Split { children, .. } => {
            let [left, right] = children.as_mut();
            for_each_leaf_mut_node(left, f);
            for_each_leaf_mut_node(right, f);
        }
    }
}

/// `layout_node`/`dividers_node` place `children[0]` left-or-top and
/// `children[1]` right-or-bottom, purely positionally — so "insert before"
/// (Split Left / Split Up) is just swapping which slot the new pane lands
/// in. Extracted as a plain function (not inlined into `split_node`) so the
/// ordering decision is unit-testable without needing a real `TerminalPane`,
/// which requires a live winit `EventLoopProxy` to construct.
fn ordered_children<T>(old: T, new: T, insert_before: bool) -> [T; 2] {
    if insert_before {
        [new, old]
    } else {
        [old, new]
    }
}

// ── layout arithmetic helpers ─────────────────────────────────────────────────

/// Clamp against complete subtree minima, not just one cell per child.
/// Rearrangement can turn a leaf child into a multi-split subtree.
fn first_extent<P>(
    children: &[PaneNode<P>; 2],
    direction: PaneSplitDirection,
    ratio: f32,
    total: usize,
) -> usize {
    let first = minimum_size_node(&children[0]);
    let second = minimum_size_node(&children[1]);
    let (minimum, other_minimum, desired) = match direction {
        PaneSplitDirection::Vertical => (first.0, second.0, left_cols_from_ratio(ratio, total)),
        PaneSplitDirection::Horizontal => (first.1, second.1, top_rows_from_ratio(ratio, total)),
    };
    let maximum = total.saturating_sub(1).saturating_sub(other_minimum);
    if maximum < minimum {
        desired
    } else {
        desired.clamp(minimum, maximum)
    }
}

fn left_cols_from_ratio(ratio: f32, cols: usize) -> usize {
    let usable = cols.saturating_sub(1);
    if usable < 2 {
        return 1;
    }
    let min_ratio = 1.0 / usable as f32;
    let max_ratio = (usable - 1) as f32 / usable as f32;
    let lc = (usable as f32 * ratio.clamp(min_ratio, max_ratio)).round() as usize;
    lc.max(1).min(usable - 1)
}

fn top_rows_from_ratio(ratio: f32, rows: usize) -> usize {
    let usable = rows.saturating_sub(1);
    if usable < 2 {
        return 1;
    }
    let min_ratio = 1.0 / usable as f32;
    let max_ratio = (usable - 1) as f32 / usable as f32;
    let tr = (usable as f32 * ratio.clamp(min_ratio, max_ratio)).round() as usize;
    tr.max(1).min(usable - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    struct Payload {
        title: String,
        drops: Arc<AtomicUsize>,
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }
    type TestNode = PaneNode<Box<Payload>>;
    type TestTree = PaneTree<Box<Payload>>;

    fn leaf(id: usize, drops: &Arc<AtomicUsize>) -> TestNode {
        PaneNode::Leaf {
            id,
            pane: Box::new(Payload {
                title: format!("shell-{id}"),
                drops: drops.clone(),
            }),
        }
    }
    fn branch(
        id: usize,
        direction: PaneSplitDirection,
        ratio: f32,
        a: TestNode,
        b: TestNode,
    ) -> TestNode {
        PaneNode::Split {
            divider_id: id,
            direction,
            ratio,
            children: Box::new([a, b]),
        }
    }
    fn fixture() -> (TestTree, Arc<AtomicUsize>) {
        let drops = Arc::new(AtomicUsize::new(0));
        let root = branch(
            10,
            PaneSplitDirection::Vertical,
            0.4,
            branch(
                11,
                PaneSplitDirection::Horizontal,
                0.3,
                leaf(0, &drops),
                leaf(1, &drops),
            ),
            branch(
                12,
                PaneSplitDirection::Horizontal,
                0.6,
                leaf(2, &drops),
                leaf(3, &drops),
            ),
        );
        (
            PaneTree {
                root: Some(root),
                active_id: 1,
                next_id: 13,
            },
            drops,
        )
    }
    fn signature<P>(node: &PaneNode<P>) -> String {
        match node {
            PaneNode::Leaf { id, .. } => id.to_string(),
            PaneNode::Split {
                divider_id,
                direction,
                ratio,
                children,
            } => format!(
                "{divider_id}:{direction:?}:{ratio}({},{})",
                signature(&children[0]),
                signature(&children[1])
            ),
        }
    }
    fn payloads(node: &TestNode, out: &mut Vec<(usize, usize, String)>) {
        match node {
            PaneNode::Leaf { id, pane } => {
                out.push((*id, &**pane as *const Payload as usize, pane.title.clone()))
            }
            PaneNode::Split { children, .. } => {
                payloads(&children[0], out);
                payloads(&children[1], out);
            }
        }
        out.sort_by_key(|entry| entry.0);
    }
    fn assert_layout(tree: &TestTree) {
        let rects = tree.layout(121, 81);
        assert_eq!(rects.len(), tree.pane_count());
        for (i, a) in rects.iter().enumerate() {
            assert!(a.cols > 0 && a.rows > 0);
            assert!(a.col + a.cols <= 121 && a.row + a.rows <= 81);
            for b in &rects[i + 1..] {
                assert!(
                    a.col + a.cols <= b.col
                        || b.col + b.cols <= a.col
                        || a.row + a.rows <= b.row
                        || b.row + b.rows <= a.row
                );
            }
        }
        // Tiny windows keep the same focused process visible.
        let tiny = tree.layout(1, 1);
        assert_eq!(tiny.len(), 1);
        assert_eq!(tiny[0].id, tree.active_id);
    }

    #[test]
    fn rearrangement_rejects_invalid_divider_and_same_ids_without_mutation() {
        for placement in [
            None,
            Some((PaneSplitDirection::Vertical, true)),
            Some((PaneSplitDirection::Horizontal, false)),
        ] {
            for (source, target, expected) in [
                (99, 1, RearrangeResult::NotFound { pane_id: 99 }),
                (1, 99, RearrangeResult::NotFound { pane_id: 99 }),
                (10, 1, RearrangeResult::NotFound { pane_id: 10 }),
                (1, 12, RearrangeResult::NotFound { pane_id: 12 }),
                (99, 99, RearrangeResult::NotFound { pane_id: 99 }),
                (1, 1, RearrangeResult::Unchanged),
            ] {
                let (mut tree, drops) = fixture();
                let before = signature(tree.root.as_ref().unwrap());
                assert_eq!(
                    rearrange_root(&mut tree.root, source, target, placement),
                    expected
                );
                assert_eq!(signature(tree.root.as_ref().unwrap()), before);
                assert_eq!((tree.active_id, tree.next_id), (1, 13));
                assert_eq!(drops.load(Ordering::SeqCst), 0);
            }
        }
    }

    #[test]
    fn all_swaps_exchange_rectangles_and_preserve_owned_payloads() {
        for source in 0..4 {
            for target in 0..4 {
                let (mut tree, drops) = fixture();
                let mut before = Vec::new();
                payloads(tree.root.as_ref().unwrap(), &mut before);
                let old = tree.layout(121, 81);
                let topology = signature(tree.root.as_ref().unwrap());
                tree.active_id = source;
                assert_eq!(
                    tree.swap_active_pane(target),
                    if source == target {
                        RearrangeResult::Unchanged
                    } else {
                        RearrangeResult::Changed
                    }
                );
                assert_eq!(tree.active_id, source);
                assert_eq!(tree.next_id, 13);
                let mut after = Vec::new();
                payloads(tree.root.as_ref().unwrap(), &mut after);
                assert_eq!(before, after);
                for (a, b) in old.iter().zip(tree.layout(121, 81)) {
                    assert_eq!(
                        (a.col, a.row, a.cols, a.rows),
                        (b.col, b.row, b.cols, b.rows)
                    );
                    assert_eq!(
                        b.id,
                        if a.id == source {
                            target
                        } else if a.id == target {
                            source
                        } else {
                            a.id
                        }
                    );
                }
                assert_layout(&tree);
                assert_eq!(drops.load(Ordering::SeqCst), 0);
                tree.swap_panes(source, target);
                assert_eq!(signature(tree.root.as_ref().unwrap()), topology);
            }
        }
    }

    #[test]
    fn all_moves_preserve_process_identity_focus_ids_and_close_semantics() {
        for source in 0..4 {
            for target in 0..4 {
                if source == target {
                    continue;
                }
                for direction in [PaneSplitDirection::Vertical, PaneSplitDirection::Horizontal] {
                    for before in [false, true] {
                        let (mut tree, drops) = fixture();
                        let mut original = Vec::new();
                        payloads(tree.root.as_ref().unwrap(), &mut original);
                        tree.active_id = source;
                        assert_eq!(
                            tree.move_active_pane(target, direction, before),
                            RearrangeResult::Changed
                        );
                        assert_eq!(
                            (tree.active_id, tree.next_id, tree.pane_count()),
                            (source, 13, 4)
                        );
                        let mut moved = Vec::new();
                        payloads(tree.root.as_ref().unwrap(), &mut moved);
                        assert_eq!(original, moved);
                        assert_eq!(drops.load(Ordering::SeqCst), 0);
                        let mut path = Vec::new();
                        assert!(leaf_path(tree.root.as_ref().unwrap(), target, &mut path));
                        let parent =
                            node_at_path_mut(tree.root.as_mut().unwrap(), &path[..path.len() - 1]);
                        let PaneNode::Split {
                            direction: actual,
                            ratio,
                            divider_id,
                            ..
                        } = parent
                        else {
                            panic!("destination parent")
                        };
                        assert_eq!((*actual, *ratio), (direction, 0.5));
                        assert_eq!(*divider_id, if source < 2 { 11 } else { 12 });
                        let mut ids = Vec::new();
                        collect_leaf_ids(parent, &mut ids);
                        assert_eq!(
                            ids,
                            if before {
                                vec![source, target]
                            } else {
                                vec![target, source]
                            }
                        );
                        assert_layout(&tree);
                        assert_eq!(tree.remove(source), RemoveResult::Removed);
                        assert_ne!(tree.active_id, source);
                        assert!(tree.leaf_ids().contains(&tree.active_id));
                        assert_eq!(drops.load(Ordering::SeqCst), 1);
                        assert_eq!(tree.remove(source), RemoveResult::NotFound);
                        for id in tree.leaf_ids().into_iter().take(2) {
                            assert_eq!(tree.remove(id), RemoveResult::Removed);
                        }
                        assert_eq!(
                            tree.remove(tree.leaf_ids()[0]),
                            RemoveResult::RemovedLastLeaf
                        );
                        assert_eq!(tree.pane_count(), 1);
                        drop(tree);
                        assert_eq!(drops.load(Ordering::SeqCst), 4);
                    }
                }
            }
        }
    }

    #[test]
    fn moving_across_root_preserves_unrelated_split_and_non_source_focus() {
        let (mut tree, _) = fixture();
        tree.active_id = 3;
        assert_eq!(
            tree.move_pane(0, 2, PaneSplitDirection::Vertical, true),
            RearrangeResult::Changed
        );
        assert_eq!(tree.active_id, 3);
        assert_eq!(
            signature(tree.root.as_ref().unwrap()),
            "10:Vertical:0.4(1,12:Horizontal:0.6(11:Vertical:0.5(0,2),3))"
        );
        tree.focus_next();
        assert_eq!(tree.active_id, 1);
    }

    #[test]
    fn moves_layout_safely_at_every_small_window_size() {
        for direction in [PaneSplitDirection::Vertical, PaneSplitDirection::Horizontal] {
            for source in 0..4 {
                for target in 0..4 {
                    let (mut tree, _) = fixture();
                    tree.move_pane(source, target, direction, false);
                    for cols in 0..14 {
                        for rows in 0..14 {
                            let rects = tree.layout(cols, rows);
                            for (index, a) in rects.iter().enumerate() {
                                assert!(a.cols > 0 && a.rows > 0);
                                assert!(
                                    a.col + a.cols <= cols.max(1),
                                    "col overflow {cols}x{rows}"
                                );
                                assert!(
                                    a.row + a.rows <= rows.max(1),
                                    "row overflow {cols}x{rows}"
                                );
                                for b in &rects[index + 1..] {
                                    assert!(
                                        a.col + a.cols <= b.col
                                            || b.col + b.cols <= a.col
                                            || a.row + a.rows <= b.row
                                            || b.row + b.rows <= a.row
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn grid_selection_and_titles_travel_with_stable_id() {
        use crate::tab::{Selection, SelectionMode};
        use volt_core::grid::Grid;
        struct State {
            grid: Grid,
            selection: Selection,
            title: String,
            scroll: usize,
        }
        let make = |id| {
            let mut grid = Grid::new(4, 3);
            let mut cell = volt_core::cell::Cell::default();
            cell.set_char('x');
            grid.put_char(0, 0, cell);
            let selection =
                Selection::at_viewport(id, false, SelectionMode::Linear, &grid, 0, 0, 0).unwrap();
            Box::new(State {
                grid,
                selection,
                title: format!("custom-{id}"),
                scroll: 0,
            })
        };
        let mut tree = PaneTree {
            root: Some(PaneNode::Split {
                direction: PaneSplitDirection::Vertical,
                ratio: 0.7,
                divider_id: 10,
                children: Box::new([
                    PaneNode::Leaf {
                        id: 7,
                        pane: make(7),
                    },
                    PaneNode::Leaf {
                        id: 9,
                        pane: make(9),
                    },
                ]),
            }),
            active_id: 7,
            next_id: 11,
        };
        for swap in [true, false, true, false] {
            let result = if swap {
                tree.swap_active_pane(9)
            } else {
                tree.move_active_pane(9, PaneSplitDirection::Horizontal, true)
            };
            assert_eq!(result, RearrangeResult::Changed);
            let mut path = Vec::new();
            assert!(leaf_path(tree.root.as_ref().unwrap(), 7, &mut path));
            let PaneNode::Leaf { pane, id } = node_at_path_mut(tree.root.as_mut().unwrap(), &path)
            else {
                panic!("leaf")
            };
            assert_eq!((*id, pane.selection.pane_id), (7, 7));
            assert_eq!(pane.selection.text(&pane.grid).as_deref(), Some("x"));
            assert_eq!(pane.title, "custom-7");
            assert_eq!((pane.grid.cols, pane.grid.rows, pane.scroll), (4, 3, 0));
            assert_eq!(tree.active_id, 7);
        }
    }

    #[test]
    fn two_leaf_and_single_leaf_trees_have_safe_rearrangement() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut tree = PaneTree::new(Box::new(Payload {
            title: "only".into(),
            drops: drops.clone(),
        }));
        assert_eq!(tree.swap_active_pane(0), RearrangeResult::Unchanged);
        assert_eq!(
            tree.move_active_pane(0, PaneSplitDirection::Horizontal, true),
            RearrangeResult::Unchanged
        );
        assert_eq!(
            tree.move_active_pane(1, PaneSplitDirection::Horizontal, true),
            RearrangeResult::NotFound { pane_id: 1 }
        );
        tree.root = Some(branch(
            7,
            PaneSplitDirection::Vertical,
            0.8,
            leaf(0, &drops),
            leaf(1, &drops),
        ));
        tree.next_id = 8;
        assert_eq!(
            tree.move_active_pane(1, PaneSplitDirection::Horizontal, false),
            RearrangeResult::Changed
        );
        assert_eq!(
            signature(tree.root.as_ref().unwrap()),
            "7:Horizontal:0.5(1,0)"
        );
        assert_eq!(tree.next_id, 8); // rearrangement never allocates a new ID
        assert_layout(&tree);
    }

    #[test]
    fn ordered_children_after_is_old_then_new() {
        // Split Right / Split Down: original pane stays left/top (slot 0).
        assert_eq!(ordered_children("old", "new", false), ["old", "new"]);
    }

    #[test]
    fn ordered_children_before_is_new_then_old() {
        // Split Left / Split Up: new pane takes left/top (slot 0), original
        // pane is pushed to right/bottom (slot 1).
        assert_eq!(ordered_children("old", "new", true), ["new", "old"]);
    }

    #[test]
    fn split_minimum_size_accounts_for_divider_and_nested_children() {
        assert_eq!(
            minimum_size_for_split(PaneSplitDirection::Vertical, (1, 1), (1, 1)),
            (3, 1)
        );
        assert_eq!(
            minimum_size_for_split(PaneSplitDirection::Horizontal, (3, 1), (1, 1)),
            (3, 3)
        );
    }
}
