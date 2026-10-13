//! In-window overlay input isolation and focus lifecycle.
use super::*;
use crate::declarative::document::{Overlay, OverlayKind};

#[derive(Clone)]
struct Scope {
    id: String,
    content: String,
    ids: BTreeSet<String>,
    options: Overlay,
    restore_focus: Option<String>,
}
#[derive(Default)]
pub(super) struct OverlayState {
    scopes: Vec<Scope>,
    outside_capture: Option<String>,
    restore_pending: Option<String>,
    focus_pending: bool,
    focus_target: Option<String>,
    tooltip_pointer: Option<[f32; 2]>,
    pub(super) visible_tooltips: BTreeSet<String>,
    tooltip_ids: BTreeSet<String>,
    tooltip_layout_dirty: bool,
    tooltips_inactive: bool,
}
/// Overlay roots are portals: the ordinary placement walk skips them, then the
/// host appends these roots in document order against the window viewport.
pub(super) fn collect_overlay_nodes(root: &Node) -> Vec<&Node> {
    fn visit<'a>(node: &'a Node, nodes: &mut Vec<&'a Node>) {
        if node.overlay.is_some() {
            nodes.push(node);
        }
        for child in &node.children {
            visit(child, nodes);
        }
    }
    let mut nodes = Vec::new();
    visit(root, &mut nodes);
    nodes
}
/// Overlay roots to lay out and paint: every non-tooltip overlay, plus the
/// tooltips currently shown. References into the installed document.
pub(super) fn visible_overlays<'a>(
    root: &'a Node,
    visible_tooltips: &BTreeSet<String>,
) -> Vec<&'a Node> {
    collect_overlay_nodes(root)
        .into_iter()
        .filter(|node| {
            node.overlay.as_ref().is_none_or(|options| {
                options.kind != OverlayKind::Tooltip || visible_tooltips.contains(&node.id)
            })
        })
        .collect()
}
fn scopes(root: &Node) -> Vec<Scope> {
    fn keys(node: &Node, ids: &mut BTreeSet<String>) {
        ids.insert(node.id.clone());
        for child in &node.children {
            keys(child, ids);
        }
    }
    collect_overlay_nodes(root)
        .into_iter()
        .filter_map(|node| {
            if node.overlay.as_ref()?.kind == OverlayKind::Tooltip {
                return None;
            }
            let content = node.children.first()?;
            let mut ids = BTreeSet::new();
            keys(content, &mut ids);
            let options = node.overlay.as_ref()?;
            if let Some(anchor) = options
                .anchor
                .as_deref()
                .filter(|_| options.keep_anchor)
                .and_then(|id| find_node(root, id))
            {
                keys(anchor, &mut ids);
            }
            Some(Scope {
                id: node.id.clone(),
                content: content.id.clone(),
                ids,
                options: node.overlay.clone()?,
                restore_focus: None,
            })
        })
        .collect()
}
#[derive(Clone)]
struct MenuItem {
    id: String,
    start: f32,
    height: f32,
    enabled: bool,
    checked: bool,
}
struct Menu {
    items: Vec<MenuItem>,
    padding: [f32; 4],
    height: f32,
}
fn find_node<'a>(node: &'a Node, id: &str) -> Option<&'a Node> {
    if node.id == id {
        Some(node)
    } else {
        node.children.iter().find_map(|child| find_node(child, id))
    }
}
/// Selection popups use fixed-height direct button children. Their logical
/// slots remain knowable when clipping omits offscreen children from boxes.
fn menu(root: &Node, scope: &Scope) -> Option<Menu> {
    if scope.options.kind != OverlayKind::Popup {
        return None;
    }
    let content = find_node(root, &scope.content)?;
    // A decorated row is a fixed-height layer whose first child is its button.
    let target = |child: &'_ Node| -> Option<(String, bool, bool)> {
        match child.kind {
            Kind::Button => Some((child.id.clone(), child.enabled, child.checked)),
            Kind::Separator => Some((child.id.clone(), false, false)),
            Kind::ZStack => child
                .children
                .first()
                .filter(|b| b.kind == Kind::Button)
                .map(|b| (b.id.clone(), b.enabled && child.enabled, b.checked)),
            _ => None,
        }
    };
    if content
        .children
        .iter()
        .any(|child| target(child).is_none() || child.height.is_none())
    {
        return None;
    }
    let mut start = 0.;
    let gap = content.main_gap(1);
    let items = content
        .children
        .iter()
        .map(|child| {
            let height = child.height.unwrap();
            let (id, enabled, checked) = target(child).unwrap();
            let item = MenuItem {
                id,
                start,
                height,
                enabled: enabled && child.kind != Kind::Separator,
                checked,
            };
            start += height + gap;
            item
        })
        .collect();
    Some(Menu {
        items,
        padding: content.padding_sides(),
        height: content.height?,
    })
}
impl Runtime {
    /// Call after document installation, before old layout boxes are discarded.
    pub(super) fn sync_overlay_state(&mut self) {
        let mut next = scopes(self.document.root());
        fn ids(node: &Node, all: &mut BTreeSet<String>) {
            all.insert(node.id.clone());
            for child in &node.children {
                ids(child, all);
            }
        }
        let mut tooltip_ids = BTreeSet::new();
        let mut tooltip_roots = BTreeSet::new();
        for node in collect_overlay_nodes(self.document.root())
            .into_iter()
            .filter(|node| {
                node.overlay
                    .as_ref()
                    .is_some_and(|o| o.kind == OverlayKind::Tooltip)
            })
        {
            tooltip_roots.insert(node.id.clone());
            ids(node, &mut tooltip_ids);
        }
        self.overlays.tooltip_ids = tooltip_ids;
        self.overlays
            .visible_tooltips
            .retain(|id| tooltip_roots.contains(id));
        let common = self
            .overlays
            .scopes
            .iter()
            .zip(&next)
            .take_while(|(old, new)| old.id == new.id && old.options.kind == new.options.kind)
            .count();
        let removed = self.overlays.scopes.len() > common;
        let added = next.len() > common;
        let restore = if removed {
            self.overlays.scopes[common].restore_focus.clone()
        } else {
            None
        };
        for (old, new) in self.overlays.scopes.iter().zip(&mut next).take(common) {
            new.restore_focus.clone_from(&old.restore_focus);
        }
        for (index, frame) in next.iter_mut().enumerate().skip(common) {
            frame.restore_focus = if index == common {
                restore
                    .clone()
                    .or_else(|| self.focused.clone())
                    .or_else(|| frame.options.anchor.clone())
            } else {
                None
            };
        }
        self.overlays.scopes = next;
        if removed || added {
            self.overlays.outside_capture = None;
            self.overlays.restore_pending = restore;
            self.overlays.focus_pending = true;
            self.overlays.focus_target = None;
            self.cancel_input();
        }
        if self
            .focused
            .as_deref()
            .is_some_and(|id| !self.scope_allows_id(id))
        {
            self.focused = None;
            self.overlays.focus_pending = true;
        }
    }
    /// Call after layout's standard focus validity check. Restores keyed enabled
    /// controls; if an opener was removed, no stale callback can be activated.
    pub(super) fn finish_overlay_layout(&mut self) {
        // Runtime::new may be implemented without an explicit initial sync hook.
        if self.overlays.scopes.is_empty()
            && !collect_overlay_nodes(self.document.root()).is_empty()
        {
            self.sync_overlay_state();
        }
        let target = self.overlays.focus_target.take();
        let restore = target.or(self.overlays.restore_pending.take());
        let valid = |id: &str| {
            self.scope_allows_id(id)
                && self
                    .boxes
                    .iter()
                    .any(|b| b.id == id && interactive_box(b) && b.enabled && !b.departing)
        };
        let mut focused = restore
            .filter(|id| valid(id))
            .or_else(|| self.focused.clone());
        if !self.overlays.scopes.is_empty() && focused.as_deref().is_none_or(|id| !valid(id)) {
            let popup = self
                .overlays
                .scopes
                .last()
                .is_some_and(|s| s.options.kind == OverlayKind::Popup);
            let mut eligible = self.boxes.iter().filter(|b| valid(&b.id));
            let checked = popup
                .then(|| eligible.clone().find(|b| b.checked))
                .flatten();
            focused = checked.or_else(|| eligible.next()).map(|b| b.id.clone());
        }
        self.focused = focused;
        self.overlays.focus_pending = false;
        self.sync_tooltip_visibility();
    }
    /// Seed a newly opened menu's scroll position before its first placement.
    /// This includes a selected option beyond the viewport at any scale.
    pub(super) fn prepare_overlay_layout(&mut self, viewport: Viewport) {
        self.overlays.tooltip_layout_dirty = false;
        self.cancel_overlay_capture();
        if !self.overlays.focus_pending {
            return;
        }
        let Some(scope) = self.overlays.scopes.last() else {
            return;
        };
        let Some(menu) = menu(self.document.root(), scope) else {
            return;
        };
        let Some(item) = self
            .overlays
            .restore_pending
            .as_deref()
            .and_then(|id| menu.items.iter().find(|item| item.enabled && item.id == id))
            .or_else(|| menu.items.iter().find(|item| item.enabled && item.checked))
            .or_else(|| menu.items.iter().find(|item| item.enabled))
        else {
            return;
        };
        let height = menu.height.min(viewport.height / viewport.scale);
        let visible = (height - menu.padding[0] - menu.padding[2]).max(0.);
        let offset = (item.start + item.height - visible).max(0.);
        let old = self
            .scroll_offsets
            .get(&scope.content)
            .copied()
            .unwrap_or([0.; 2]);
        self.scroll_offsets
            .insert(scope.content.clone(), [old[0], offset]);
        self.overlays.focus_target = Some(item.id.clone());
    }
    /// Blur, resize, scrolling and focus navigation cannot leave an armed
    /// backdrop press that later dismisses a different/recomposed overlay.
    pub(super) fn cancel_overlay_capture(&mut self) -> bool {
        self.overlays.outside_capture.take().is_some()
    }
    /// Only visible pointer/focus anchors in the active input scope expose help.
    pub(super) fn sync_tooltip_visibility(&mut self) -> bool {
        let mut visible = BTreeSet::new();
        for node in collect_overlay_nodes(self.document.root())
            .into_iter()
            .filter(|_| !self.overlays.tooltips_inactive)
        {
            let Some(options) = &node.overlay else {
                continue;
            };
            if options.kind != OverlayKind::Tooltip {
                continue;
            }
            let Some(anchor) = options.anchor.as_deref() else {
                continue;
            };
            if !self.scope_allows_id(anchor) {
                continue;
            }
            let Some(bounds) = self.boxes.iter().find(|b| b.id == anchor && !b.departing) else {
                continue;
            };
            let hovered = self.overlays.tooltip_pointer.is_some_and(|[x, y]| {
                bounds.clip.contains(x, y)
                    && bounds.clip_regions.iter().all(|clip| clip.contains(x, y))
                    && bounds
                        .content_clip
                        .as_ref()
                        .is_none_or(|clip| clip.contains(x, y))
                    && bounds
                        .bounds
                        .contains_corners(bounds.radii(self.scale), x, y)
            });
            fn contains(node: &Node, id: &str) -> bool {
                node.id == id || node.children.iter().any(|child| contains(child, id))
            }
            let focused = self.focused.as_deref().is_some_and(|id| {
                find_node(self.document.root(), anchor).is_some_and(|n| contains(n, id))
            });
            if hovered || focused {
                visible.insert(node.id.clone());
            }
        }
        let changed = visible != self.overlays.visible_tooltips;
        if changed {
            self.overlays.visible_tooltips = visible;
            self.overlays.tooltip_layout_dirty = true;
            self.invalidate_layout();
        }
        changed
    }
    /// Hosts suppress transient help while the operating-system window is inactive.
    pub fn set_tooltips_active(&mut self, active: bool) -> bool {
        self.overlays.tooltips_inactive = !active;
        if !active {
            self.overlays.tooltip_pointer = None;
        }
        self.sync_tooltip_visibility()
    }
    pub(super) fn tooltip_pointer(&mut self, point: Option<[f32; 2]>) -> bool {
        self.overlays.tooltip_pointer = point.filter(|p| p.iter().all(|v| v.is_finite()));
        self.sync_tooltip_visibility()
    }
    pub(super) fn take_tooltip_layout_change(&mut self) -> bool {
        std::mem::take(&mut self.overlays.tooltip_layout_dirty)
    }
    pub(super) fn visible_overlay_nodes(&self) -> Vec<&Node> {
        visible_overlays(self.document.root(), &self.overlays.visible_tooltips)
    }
    pub(super) fn portal_hit_passthrough(&self, id: &str) -> bool {
        self.overlays.tooltip_ids.contains(id)
    }
    /// Request a context menu on right-button press without activating the target.
    /// The callback receives logical pixels; opening/closing remains app-owned.
    pub fn context_menu_at<C>(
        &mut self,
        x: f32,
        y: f32,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        if !x.is_finite() || !y.is_finite() {
            return Ok(InputOutcome::default());
        }
        let mut marker = None;
        if let Some(mut index) = self.boxes.iter().rposition(|b| {
            !self.portal_hit_passthrough(&b.id)
                && self.scope_allows_id(&b.id)
                && b.clip.contains(x, y)
                && b.clip_regions.iter().all(|clip| clip.contains(x, y))
                && b.content_clip
                    .as_ref()
                    .is_none_or(|clip| clip.contains(x, y))
                && b.bounds.contains_corners(b.radii(self.scale), x, y)
        }) {
            loop {
                let b = &self.boxes[index];
                if !self.scope_allows_id(&b.id) {
                    break;
                }
                if let Some(node) = find_node(self.document.root(), &b.id) {
                    if node.enabled {
                        if let Some(action) = &node.on_context {
                            // A root wrapper is allotted the whole window, but
                            // its context target is the first composed child.
                            // Empty wrapper space must not request its menu.
                            let target_hit = node.children.first().is_some_and(|target| {
                                self.boxes
                                    .iter()
                                    .find(|candidate| candidate.id == target.id)
                                    .is_some_and(|b| {
                                        b.clip.contains(x, y)
                                            && b.clip_regions.iter().all(|clip| clip.contains(x, y))
                                            && b.content_clip
                                                .as_ref()
                                                .is_none_or(|clip| clip.contains(x, y))
                                            && b.bounds.contains_corners(b.radii(self.scale), x, y)
                                    })
                            });
                            if target_hit {
                                marker = Some(action.clone());
                                break;
                            }
                        }
                    }
                }
                if b.depth == 0 {
                    break;
                }
                let Some(parent) = self.boxes[..index]
                    .iter()
                    .rposition(|p| p.depth + 1 == b.depth)
                else {
                    break;
                };
                index = parent;
            }
        }
        let Some(action) = marker else {
            return Ok(InputOutcome::default());
        };
        let focus = self.hit(x, y);
        self.cancel_input();
        if focus.is_some() {
            self.focused = focus;
        }
        let [x, y] = [x / self.scale, y / self.scale];
        handlers
            .dispatch(
                &action,
                Event::ContextRequested {
                    x_bits: x.to_bits(),
                    y_bits: y.to_bits(),
                },
                context,
            )
            .map_err(|e| DocumentError(e.to_string()))?;
        Ok(InputOutcome {
            changed: true,
            activated: true,
        })
    }
    pub(super) fn scope_allows_id(&self, id: &str) -> bool {
        self.overlays
            .scopes
            .last()
            .is_none_or(|scope| scope.ids.contains(id))
    }
    pub(super) fn overlay_active(&self) -> bool {
        !self.overlays.scopes.is_empty()
    }
    fn inside_overlay_content(&self, x: f32, y: f32) -> bool {
        let Some(scope) = self.overlays.scopes.last() else {
            return false;
        };
        let anchor = scope
            .options
            .anchor
            .as_deref()
            .filter(|_| scope.options.keep_anchor);
        self.boxes
            .iter()
            .filter(|b| b.id == scope.content || Some(b.id.as_str()) == anchor)
            .any(|b| {
                b.clip.contains(x, y)
                    && b.clip_regions.iter().all(|r| r.contains(x, y))
                    && b.content_clip
                        .as_ref()
                        .is_none_or(|clip| clip.contains(x, y))
                    && b.bounds.contains_corners(b.radii(self.scale), x, y)
            })
    }
    /// A backdrop press is consumed even when outside dismissal is disabled.
    pub(super) fn overlay_pointer_down(&mut self, x: f32, y: f32) -> Option<bool> {
        if !self.overlay_active() || self.inside_overlay_content(x, y) {
            self.overlays.outside_capture = None;
            return None;
        }
        self.cancel_input();
        self.overlays.outside_capture = if x.is_finite() && y.is_finite() {
            self.overlays.scopes.last().map(|s| s.id.clone())
        } else {
            None
        };
        Some(true)
    }
    pub(super) fn overlay_pointer_up<C>(
        &mut self,
        x: f32,
        y: f32,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<Option<InputOutcome>, DocumentError> {
        let Some(pressed) = self.overlays.outside_capture.take() else {
            return Ok(None);
        };
        let should_dismiss = self
            .overlays
            .scopes
            .last()
            .is_some_and(|scope| scope.id == pressed && scope.options.backdrop_dismiss)
            && x.is_finite()
            && y.is_finite()
            && !self.inside_overlay_content(x, y);
        if should_dismiss {
            let result = self.dispatch_overlay_dismiss(handlers, context)?;
            return Ok(Some(InputOutcome {
                changed: true,
                ..result
            }));
        }
        Ok(Some(InputOutcome {
            changed: true,
            activated: false,
        }))
    }
    fn dispatch_overlay_dismiss<C>(
        &mut self,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        let action = self
            .overlays
            .scopes
            .last()
            .and_then(|scope| scope.options.dismiss_action.clone());
        self.cancel_input();
        self.overlays.outside_capture = None;
        if let Some(action) = action {
            handlers
                .dispatch(&action, Event::Activate, context)
                .map_err(|e| DocumentError(e.to_string()))?;
            Ok(InputOutcome {
                changed: true,
                activated: true,
            })
        } else {
            Ok(InputOutcome::default())
        }
    }
    /// Escape is dispatched on key press, not on a later activation-key release.
    /// The application must close its controlled overlay in the callback.
    pub fn dismiss_overlay<C>(
        &mut self,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        if !self
            .overlays
            .scopes
            .last()
            .is_some_and(|scope| scope.options.escape_dismiss)
        {
            return Ok(InputOutcome::default());
        }
        self.dispatch_overlay_dismiss(handlers, context)
    }
    /// Popup arrows/Home/End move focus only; activation commits the choice.
    /// Modal cards keep ordinary Tab traversal and child component key handling.
    pub fn overlay_key(&mut self, key: SelectionKey) -> bool {
        if !self
            .overlays
            .scopes
            .last()
            .is_some_and(|scope| scope.options.kind == OverlayKind::Popup)
        {
            return false;
        }
        let scope = self.overlays.scopes.last().unwrap();
        let menu = menu(self.document.root(), scope);
        let ids: Vec<_> = if let Some(menu) = &menu {
            menu.items
                .iter()
                .filter(|item| item.enabled)
                .map(|item| item.id.clone())
                .collect()
        } else {
            self.boxes
                .iter()
                .filter(|b| {
                    self.scope_allows_id(&b.id) && interactive_box(b) && b.enabled && !b.departing
                })
                .map(|b| b.id.clone())
                .collect()
        };
        if ids.is_empty() {
            return false;
        }
        let current = self
            .focused
            .as_ref()
            .and_then(|id| ids.iter().position(|value| value == id));
        let next = match key {
            SelectionKey::First => 0,
            SelectionKey::Last => ids.len() - 1,
            SelectionKey::Next => current.map_or(0, |index| (index + 1) % ids.len()),
            SelectionKey::Previous => {
                current.map_or(ids.len() - 1, |index| (index + ids.len() - 1) % ids.len())
            }
        };
        if let Some(menu) = &menu {
            let target = menu.items.iter().find(|item| item.id == ids[next]).unwrap();
            if let Some(content) = self.boxes.iter().find(|b| b.id == scope.content) {
                let height = content.bounds.bounds()[3] / self.scale;
                let visible = (height - menu.padding[0] - menu.padding[2]).max(0.);
                let mut offset = self
                    .scroll_offsets
                    .get(&scope.content)
                    .copied()
                    .unwrap_or(content.scroll_offset);
                if target.start < offset[1] {
                    offset[1] = target.start;
                }
                if target.start + target.height > offset[1] + visible {
                    offset[1] = target.start + target.height - visible;
                }
                offset[1] = offset[1].clamp(0., content.scroll_max[1]);
                self.scroll_offsets.insert(scope.content.clone(), offset);
            }
            self.overlays.focus_target = Some(ids[next].clone());
            self.invalidate_layout();
        }
        self.cancel_input();
        self.focused = Some(ids[next].clone());
        self.focus_visible = true;
        true
    }
}
