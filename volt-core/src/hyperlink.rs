//! OSC 8 metadata. Terminals may receive arbitrary schemes; opening is a UI policy.
use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub const MAX_URI_BYTES: usize = 2048;
pub const MAX_ID_BYTES: usize = 128;
pub(crate) const MAX_LINKS: usize = 4096;
pub(crate) const MAX_LINKED_TEXTS: usize = 65_536;
pub(crate) const MAX_LINKED_TEXT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hyperlink {
    uri: Arc<str>,
    id: Arc<str>,
    fingerprint: u64,
}

impl Hash for Hyperlink {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.fingerprint.hash(state);
    }
}

impl Hyperlink {
    pub fn uri(&self) -> &str {
        &self.uri
    }
    pub fn id(&self) -> &str {
        &self.id
    }

    /// VTE splits all semicolons, including ones in the URI; rejoin those only.
    /// Malformed/oversized/empty commands close the current hyperlink.
    pub fn from_osc(params: &[&[u8]]) -> Option<Arc<Self>> {
        // VTE 0.15 caps OSC parameters at 16 and may truncate beyond that.
        // Never turn a truncated destination into a different valid URL.
        if params.len() < 3 || params.len() >= 16 || params[0] != b"8" || params[1].len() > 1024 {
            return None;
        }
        let size = params[2..]
            .iter()
            .try_fold(0usize, |n, p| n.checked_add(p.len()))?
            .checked_add(params.len() - 3)?;
        if size == 0 || size > MAX_URI_BYTES {
            return None;
        }
        let mut uri = String::with_capacity(size);
        for (i, part) in params[2..].iter().enumerate() {
            if i > 0 {
                uri.push(';');
            }
            uri.push_str(std::str::from_utf8(part).ok()?);
        }
        if uri.chars().any(|c| c.is_control()) {
            return None;
        }
        let options = std::str::from_utf8(params[1]).ok()?;
        let id = options
            .split(':')
            .find_map(|p| p.strip_prefix("id="))
            .unwrap_or("");
        if id.len() > MAX_ID_BYTES || id.chars().any(|c| c.is_control()) {
            return None;
        }
        // Hash a potentially long URL once per OSC, not once per printed cell.
        // Equality still compares the full immutable strings on collisions.
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        uri.hash(&mut hasher);
        id.hash(&mut hasher);
        Some(Arc::new(Self {
            uri: Arc::from(uri),
            id: Arc::from(id),
            fingerprint: hasher.finish(),
        }))
    }
}
