//! Native file drag and drop onto keyed drop zones.
use super::*;
use std::path::PathBuf;

fn find_node<'a>(node: &'a Node, id: &str) -> Option<&'a Node> {
    if node.id == id {
        Some(node)
    } else {
        node.children.iter().find_map(|child| find_node(child, id))
    }
}
impl Runtime {
    fn drop_action(&self, id: &str) -> Option<String> {
        // Effective enablement includes bindings. Revalidate scope
        // and mounting here too: a drag can span a modal or document change.
        self.boxes
            .iter()
            .find(|b| b.id == id && b.enabled && !b.departing && self.scope_allows_id(id))?;
        find_node(self.document.root(), id)
            .filter(|n| n.enabled)
            .and_then(|n| n.on_files_dropped.clone())
    }
    /// The drop zone under `at` (its nearest accepting ancestor), else the
    /// only drop zone in the active scope: platforms may not report the
    /// pointer while a drag from another application is in progress.
    fn drop_target(&self, at: Option<[f32; 2]>) -> Option<(String, String)> {
        let in_scope = |id: &str| self.scope_allows_id(id);
        if let Some([x, y]) = at.filter(|p| p.iter().all(|v| v.is_finite())) {
            if let Some(mut index) = self.boxes.iter().rposition(|b| {
                in_scope(&b.id)
                    && b.clip.contains(x, y)
                    && b.clip_regions.iter().all(|clip| clip.contains(x, y))
                    && b.bounds.contains_corners(b.radii(self.scale), x, y)
            }) {
                loop {
                    let b = &self.boxes[index];
                    if let Some(action) = self.drop_action(&b.id) {
                        return Some((b.id.clone(), action));
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
        }
        let mut zones = self
            .boxes
            .iter()
            .filter(|b| in_scope(&b.id) && !b.departing)
            .filter_map(|b| Some((b.id.clone(), self.drop_action(&b.id)?)));
        let only = zones.next()?;
        zones.next().is_none().then_some(only)
    }
    /// A file drag is over the window. The target zone shows its hover style.
    /// Returns whether the highlighted zone changed.
    pub fn file_hover(&mut self, at: Option<[f32; 2]>) -> bool {
        let target = self.drop_target(at).map(|(id, _)| id);
        if target == self.file_hover {
            return false;
        }
        self.file_hover_end();
        self.file_hover = target.clone();
        if target.is_some() {
            self.hovered = target;
        }
        true
    }
    /// The drag left the window or was cancelled.
    pub fn file_hover_end(&mut self) -> bool {
        let Some(id) = self.file_hover.take() else {
            return false;
        };
        if self.hovered.as_deref() == Some(id.as_str()) {
            self.hovered = None;
        }
        true
    }
    /// The drop zone currently highlighted by a file drag.
    pub fn file_hover_target(&self) -> Option<&str> {
        self.file_hover.as_deref()
    }
    /// Deliver dropped paths to the zone under `at` (see `drop_target`).
    pub fn drop_files<C>(
        &mut self,
        at: Option<[f32; 2]>,
        paths: Vec<PathBuf>,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        let target = self
            .file_hover
            .clone()
            .and_then(|id| Some((id.clone(), self.drop_action(&id)?)))
            .or_else(|| self.drop_target(at));
        self.file_hover_end();
        let Some((_, action)) = target.filter(|_| !paths.is_empty()) else {
            return Ok(InputOutcome::default());
        };
        handlers
            .dispatch(&action, Event::FilesDropped(paths), context)
            .map_err(|e| DocumentError(e.to_string()))?;
        Ok(InputOutcome {
            changed: true,
            activated: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::prelude::*;
    struct Metrics;
    impl TextMetrics for Metrics {
        fn measure(&mut self, _: &str, size: f32, max: f32) -> [f32; 2] {
            [max.min(100.), size * 1.2]
        }
    }
    #[test]
    fn binding_disabled_drop_zones_cannot_receive_files() {
        let e: Element<usize> = drop_zone()
            .key("drop")
            .on_drop(|n: &mut usize, _| *n += 1)
            .into();
        let (mut doc, mut handlers) = e.build().unwrap().into_parts();
        doc.node_mut("drop").unwrap().bind_enabled = Some("allowed".into());
        let mut values = Bindings::default();
        values.set_bool("allowed", false);
        let mut runtime = Runtime::new(doc, &handlers, &values).unwrap();
        runtime
            .layout(
                Viewport::new(400., 400., 1.).unwrap(),
                &values,
                &mut Metrics,
            )
            .unwrap();
        let mut calls = 0;
        assert!(
            !runtime
                .drop_files(None, vec!["private.txt".into()], &mut handlers, &mut calls)
                .unwrap()
                .activated
        );
        assert_eq!(calls, 0);
    }
}
