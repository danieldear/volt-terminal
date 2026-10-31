//! A fixed-capacity VTE parser. No per-byte wrapper filtering or OSC heap growth.
//!
//! All workspace VTE dependencies must keep `std` disabled: std ignores the
//! const-generic bound. Performer rejects dispatch at capacity, so truncation
//! cannot become a valid shortened clipboard/image request.
// Matches pinned VTE 0.15's parameter ceiling; reaching it is rejected because
// additional parameters cannot be distinguished from truncation at dispatch.
pub const MAX_OSC_PARAMETERS: usize = 16;
pub const OSC_CAPACITY: usize = 64 * 1024 + 16;

pub struct Parser {
    inner: vte::Parser<OSC_CAPACITY>,
}
impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}
impl Parser {
    pub fn new() -> Self {
        Self {
            inner: vte::Parser::new_with_size(),
        }
    }
    #[inline]
    pub fn advance<P: vte::Perform>(&mut self, performer: &mut P, bytes: &[u8]) {
        self.inner.advance(performer, bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::performer::Performer;
    #[test]
    fn unterminated_flood_has_fixed_size_and_recovers_after_terminator() {
        let mut parser = Parser::new();
        let mut p = Performer::new(80, 24);
        parser.advance(&mut p, b"\x1b]52;c;");
        let block = [b'A'; 4096];
        for _ in 0..1024 {
            parser.advance(&mut p, &block);
        }
        assert!(std::mem::size_of::<Parser>() < OSC_CAPACITY + 2048);
        parser.advance(&mut p, b"\x07OK");
        assert!(p.pending_events.is_empty());
        assert!(p.grid.row_text(p.grid.row_cells(0)).starts_with("OK"));
        parser.advance(&mut p, b"\x1b]52;c;aGVsbG8=\x07");
        assert_eq!(p.pending_events.len(), 1);
    }
    #[test]
    fn ignored_extra_parameters_cannot_hide_a_truncated_osc() {
        let mut parser = Parser::new();
        let mut p = Performer::new(80, 24);
        parser.advance(&mut p, b"\x1b]2;safe-title");
        for _ in 0..32 {
            parser.advance(&mut p, b";x");
        }
        parser.advance(&mut p, &vec![b'A'; OSC_CAPACITY + 1024]);
        parser.advance(&mut p, b"\x07OK");
        assert!(p.pending_events.is_empty());
        assert!(p.grid.row_text(p.grid.row_cells(0)).starts_with("OK"));
    }

    #[test]
    fn truncated_title_and_image_are_rejected_not_published() {
        for header in [b"\x1b]2;".as_slice(), b"\x1b]1337;File=inline=1:"] {
            let mut parser = Parser::new();
            let mut p = Performer::new(80, 24);
            parser.advance(&mut p, header);
            parser.advance(&mut p, &vec![b'A'; OSC_CAPACITY + 1024]);
            parser.advance(&mut p, b"\x1b\\after");
            assert!(p.pending_events.is_empty());
            assert!(p.grid.row_text(p.grid.row_cells(0)).starts_with("after"));
        }
    }
}
