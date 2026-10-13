//! A small, fast non-cryptographic hasher for internal caches keyed by node
//! ids, text and sizes (the FxHash scheme used by rustc). Not resistant to
//! hash flooding; never use it for maps keyed by untrusted network input.
use std::hash::{BuildHasherDefault, Hasher};

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

#[derive(Clone, Copy, Default)]
pub(crate) struct FxHasher {
    hash: u64,
}
impl FxHasher {
    #[inline]
    fn add(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}
impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let (chunks, rest) = bytes.as_chunks::<8>();
        for c in chunks {
            self.add(u64::from_le_bytes(*c));
        }
        if !rest.is_empty() {
            // Assemble the tail in a register (no buffer copy).
            let mut word = (rest.len() as u64) << 56;
            for (i, b) in rest.iter().enumerate() {
                word |= u64::from(*b) << (8 * i);
            }
            self.add(word);
        }
    }
    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add(u64::from(i));
    }
    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.add(u64::from(i));
    }
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add(u64::from(i));
    }
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add(i);
    }
    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }
    #[inline]
    fn finish(&self) -> u64 {
        // The multiply leaves aligned inputs (addresses, small integers) with
        // weak low bits, which hash tables use to pick buckets; rotate the
        // well-mixed high bits down (as rustc-hash 2 does).
        self.hash.rotate_left(26)
    }
}
pub(crate) type FxBuild = BuildHasherDefault<FxHasher>;
pub(crate) type FxHashMap<K, V> = std::collections::HashMap<K, V, FxBuild>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::{Hash, Hasher};
    fn h(v: impl Hash) -> u64 {
        let mut s = FxHasher::default();
        v.hash(&mut s);
        s.finish()
    }
    #[test]
    fn distinguishes_similar_keys_and_lengths() {
        assert_ne!(h("auto-root-0-1"), h("auto-root-0-2"));
        assert_ne!(h("abc"), h("abc\0"));
        assert_ne!(h(("a", 1u32)), h(("a", 2u32)));
        assert_eq!(h("same"), h("same"));
        let mut m: FxHashMap<String, u32> = FxHashMap::default();
        for i in 0..1000 {
            m.insert(format!("node-{i}"), i);
        }
        assert_eq!(m.len(), 1000);
        assert_eq!(m["node-500"], 500);
        // Aligned addresses must spread across buckets.
        let low: std::collections::BTreeSet<u64> =
            (0..64u64).map(|i| h(0x1000 + i * 576) & 63).collect();
        assert!(
            low.len() > 32,
            "only {} of 64 low-bit buckets used",
            low.len()
        );
    }
}
