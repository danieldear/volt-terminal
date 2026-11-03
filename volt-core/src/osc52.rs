//! Clipboard-write decoding only. Queries never read the host clipboard.
pub const MAX_TEXT_BYTES: usize = 48 * 1024;

pub fn decode_write(params: &[&[u8]]) -> Option<Vec<u8>> {
    if params.len() != 3 || params[0] != b"52" || !matches!(params[1], b"" | b"c") {
        return None;
    }
    let encoded = params[2];
    if encoded == b"?" || encoded.len() > MAX_TEXT_BYTES.div_ceil(3) * 4 {
        return None;
    }
    if !encoded.len().is_multiple_of(4) {
        return None;
    }
    let value = |b: u8| -> Option<u8> {
        match b {
            b'A'..=b'Z' => Some(b - b'A'),
            b'a'..=b'z' => Some(b - b'a' + 26),
            b'0'..=b'9' => Some(b - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let mut out = Vec::with_capacity(encoded.len() / 4 * 3);
    for (i, part) in encoded.as_chunks::<4>().0.iter().enumerate() {
        let a = value(part[0])?;
        let b = value(part[1])?;
        let last = (i + 1) * 4 == encoded.len();
        out.push(a << 2 | b >> 4);
        if part[2] == b'=' {
            if !last || part[3] != b'=' || b & 15 != 0 {
                return None;
            }
        } else {
            let c = value(part[2])?;
            out.push(b << 4 | c >> 2);
            if part[3] == b'=' {
                if !last || c & 3 != 0 {
                    return None;
                }
            } else {
                out.push(c << 6 | value(part[3])?);
            }
        }
    }
    if out.len() > MAX_TEXT_BYTES || std::str::from_utf8(&out).is_err() {
        return None;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queries_invalid_selectors_and_malformed_base64_are_denied() {
        for s in [&b"?"[..], b"YQ=", b"YR==", b"YQ==AAAA", b"YQ=A", b"!!!!"] {
            assert!(decode_write(&[b"52", b"c", s]).is_none());
        }
        assert!(decode_write(&[b"52", b"p", b"YQ=="]).is_none());
        assert!(decode_write(&[b"52", b"c", b"/w=="]).is_none());
        assert!(decode_write(&[b"52", b"c", &vec![b'A'; 65540]]).is_none());
    }
    #[test]
    fn valid_utf8_empty_and_padding_decode_exactly() {
        assert_eq!(decode_write(&[b"52", b"c", b"YQ=="]), Some(b"a".to_vec()));
        assert_eq!(decode_write(&[b"52", b"", b"YWJj"]), Some(b"abc".to_vec()));
        assert_eq!(
            decode_write(&[b"52", b"c", b"4pyT"]),
            Some("✓".as_bytes().to_vec())
        );
        assert_eq!(decode_write(&[b"52", b"c", b""]), Some(vec![]));
    }
}
