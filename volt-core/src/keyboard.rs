//! Negotiated per-screen keyboard state. Legacy input remains the default.
const SUPPORTED: u16 = 31;
const MAX_DEPTH: usize = 32;
#[derive(Default)]
pub struct KeyboardModes {
    flags: u16,
    stack: Vec<u16>,
}
impl KeyboardModes {
    pub fn flags(&self) -> u16 {
        self.flags
    }
    pub fn set(&mut self, flags: u16, mode: u16) {
        let flags = flags & SUPPORTED;
        match mode {
            0 | 1 => self.flags = flags,
            2 => self.flags |= flags,
            3 => self.flags &= !flags,
            _ => {}
        }
    }
    pub fn push(&mut self, flags: u16) {
        if self.stack.len() == MAX_DEPTH {
            self.stack.remove(0);
        }
        self.stack.push(self.flags);
        self.flags = flags & SUPPORTED;
    }
    pub fn pop(&mut self, count: u16) {
        for _ in 0..usize::from(count.max(1)).min(MAX_DEPTH + 1) {
            self.flags = self.stack.pop().unwrap_or(0);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_stack_set_modes_and_underflow() {
        let mut k = KeyboardModes::default();
        k.set(1, 1);
        k.set(2, 2);
        assert_eq!(k.flags(), 3);
        k.set(1, 3);
        assert_eq!(k.flags(), 2);
        k.set(31, 9);
        assert_eq!(k.flags(), 2);
        for _ in 0..100 {
            k.push(65535);
        }
        assert_eq!(k.stack.len(), MAX_DEPTH);
        assert_eq!(k.flags(), 31);
        k.pop(65535);
        assert_eq!(k.flags(), 0);
    }
}
