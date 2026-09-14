//! MUTF-8 encoding for query text.
//!
//! DEX strings are MUTF-8: NUL is stored as `0xC0 0x80` and supplementary
//! characters as a surrogate pair of three-byte sequences. A query has to be
//! encoded the same way before it can be matched against the string table.

/// Appends a MUTF-8 string to `out` without building an intermediate `String` when the
/// bytes are ASCII.
///
/// Real descriptors, member names and query values are ASCII, and MUTF-8 is byte-for-byte
/// the same as UTF-8 there - only NUL (0xC0 0x80) and the surrogate-pair form for
/// supplementary characters differ, and both are non-ASCII. So the common case is a copy,
/// not a decode, and the non-ASCII case keeps the decoder's exact behaviour.
pub(super) fn push_decoded(out: &mut String, bytes: &[u8]) {
    if bytes.is_ascii() {
        // SAFETY: every byte was just checked to be < 0x80, so the slice is valid UTF-8.
        out.push_str(unsafe { std::str::from_utf8_unchecked(bytes) });
        return;
    }
    out.push_str(&decode_owned(bytes));
}

/// One MUTF-8 string as an owned `String`; see [`push_decoded`] for the ASCII shortcut.
pub(super) fn decode_owned(bytes: &[u8]) -> String {
    if bytes.is_ascii() {
        let mut out = String::with_capacity(bytes.len());
        // SAFETY: see `push_decoded`.
        out.push_str(unsafe { std::str::from_utf8_unchecked(bytes) });
        return out;
    }
    decode_mutf8(bytes).unwrap_or_else(|_| String::from_utf8_lossy(bytes).into_owned())
}

pub(super) fn encode_mutf8(value: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len());
    for unit in value.encode_utf16() {
        match unit {
            0 => out.extend_from_slice(&[0xc0, 0x80]),
            0x0001..=0x007f => out.push(unit as u8),
            0x0080..=0x07ff => {
                out.push((0xc0 | (unit >> 6)) as u8);
                out.push((0x80 | (unit & 0x3f)) as u8);
            }
            _ => {
                out.push((0xe0 | (unit >> 12)) as u8);
                out.push((0x80 | ((unit >> 6) & 0x3f)) as u8);
                out.push((0x80 | (unit & 0x3f)) as u8);
            }
        }
    }
    out
}

/// Decodes one DEX MUTF-8 string (DEX spec §3.3.1).
///
/// Accepted: ASCII `0x01..=0x7F`; 2-byte leads `0xC0..=0xDF`, including the
/// MUTF-8 NUL form `0xC0 0x80`; 3-byte leads `0xE0..=0xEF` for non-surrogate
/// BMP code points; and the 6-byte CESU-8 surrogate pair for supplementary
/// characters. A bare `0x00` ends the string and the remaining bytes are
/// ignored (that byte is the DEX string terminator).
///
/// Rejected: overlong forms other than `0xC0 0x80`, lone or swapped or
/// oversized surrogates, truncated sequences, and bad continuation bytes. `Err`
/// carries the byte offset where the failing sequence starts.
///
/// This used to be `rasc_dex::mutf8::decode_mutf8`; it lives here now that the
/// CLI no longer depends on that crate, with the same contract and rejection
/// matrix.
fn decode_mutf8(bytes: &[u8]) -> Result<String, usize> {
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let lead = bytes[i];
        if lead == 0 {
            break;
        }
        if lead < 0x80 {
            out.push(char::from(lead));
            i += 1;
            continue;
        }
        match lead {
            0xC0..=0xDF => {
                let b1 = continuation(bytes, i + 1, i)?;
                let code = (u32::from(lead & 0x1F) << 6) | u32::from(b1 & 0x3F);
                if code == 0 {
                    out.push('\0');
                } else if code < 0x80 {
                    return Err(i);
                } else {
                    out.push(char::from_u32(code).ok_or(i)?);
                }
                i += 2;
            }
            0xE0..=0xEF => {
                let b1 = continuation(bytes, i + 1, i)?;
                let b2 = continuation(bytes, i + 2, i)?;
                let code = (u32::from(lead & 0x0F) << 12)
                    | (u32::from(b1 & 0x3F) << 6)
                    | u32::from(b2 & 0x3F);
                if (0xD800..=0xDBFF).contains(&code) {
                    let (low, next) = decode_low_surrogate(bytes, i + 3, i)?;
                    let combined = 0x1_0000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                    out.push(char::from_u32(combined).ok_or(i)?);
                    i = next;
                } else if (0xDC00..=0xDFFF).contains(&code) {
                    return Err(i); // lone low surrogate
                } else if code < 0x800 {
                    return Err(i); // overlong
                } else {
                    out.push(char::from_u32(code).ok_or(i)?);
                    i += 3;
                }
            }
            _ => return Err(i),
        }
    }
    Ok(out)
}

/// One `10xxxxxx` continuation byte, or `Err(start)`.
fn continuation(bytes: &[u8], at: usize, start: usize) -> Result<u8, usize> {
    match bytes.get(at) {
        Some(&byte) if byte & 0xC0 == 0x80 => Ok(byte),
        _ => Err(start),
    }
}

/// The low half of a CESU-8 pair (`0xED 0xB0..=0xBF 10xxxxxx`) starting at `at`;
/// returns the code unit and the offset after it. `start` is the pair's offset.
fn decode_low_surrogate(bytes: &[u8], at: usize, start: usize) -> Result<(u32, usize), usize> {
    if bytes.get(at) != Some(&0xED) {
        return Err(start);
    }
    let b1 = continuation(bytes, at + 1, start)?;
    if !(0xB0..=0xBF).contains(&b1) {
        return Err(start);
    }
    let b2 = continuation(bytes, at + 2, start)?;
    let low = (u32::from(0xEDu8 & 0x0F) << 12) | (u32::from(b1 & 0x3F) << 6) | u32::from(b2 & 0x3F);
    if !(0xDC00..=0xDFFF).contains(&low) {
        return Err(start);
    }
    Ok((low, at + 3))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ascii_shortcut_agrees_with_the_decoder() {
        for (bytes, expected) in [
            (&b"Lcom/example/Main;"[..], "Lcom/example/Main;"),
            (b"<init>", "<init>"),
            (&b""[..], ""),
            (b"a\x7fb", "a\x7fb"),
        ] {
            let mut pushed = String::new();
            push_decoded(&mut pushed, bytes);
            assert_eq!(pushed, expected);
            assert_eq!(decode_owned(bytes), expected);
        }
        // Non-ASCII still goes through the MUTF-8 decoder, including the two encodings
        // that are not UTF-8.
        for value in ["a\u{7f}b", "\u{e9}t\u{e9}", "\u{4e2d}\u{6587}", "A\u{1F600}Z", "n\0ul"] {
            let encoded = encode_mutf8(value);
            let mut pushed = String::new();
            push_decoded(&mut pushed, &encoded);
            assert_eq!(pushed, value, "{value:?}");
            assert_eq!(decode_owned(&encoded), value, "{value:?}");
        }
    }

    #[test]
    fn mutf8_encodes_nul_and_supplementary_characters() {
        assert_eq!(encode_mutf8("a\0b"), b"a\xc0\x80b");
        assert_eq!(encode_mutf8("😀"), [0xed, 0xa0, 0xbd, 0xed, 0xb8, 0x80]);
        assert_eq!(decode_mutf8(&encode_mutf8("A😀\0Z")).unwrap(), "A😀\0Z");
    }

    /// The surrogate-pair rejection matrix the decoder inherited from rasc-dex:
    /// lone high, lone low, swapped, oversized, and the truncation shapes.
    #[test]
    fn malformed_surrogates_and_truncation_are_rejected() {
        // High surrogate U+D83D followed by a private-use 3-byte sequence.
        assert_eq!(decode_mutf8(&[0xED, 0xA0, 0xBD, 0xEE, 0x88, 0x9F]), Err(0));
        // Low surrogate U+DE00 where a codepoint should start.
        assert_eq!(decode_mutf8(&[0xED, 0xB8, 0x80]), Err(0));
        // Low before high.
        assert_eq!(decode_mutf8(&[0xED, 0xB8, 0x80, 0xED, 0xA0, 0xBD]), Err(0));
        // High surrogate followed by U+FFFF.
        assert_eq!(decode_mutf8(&[0xED, 0xA0, 0xBD, 0xEF, 0xBF, 0xBF]), Err(0));
        // Truncated and bad-continuation shapes must not panic.
        assert!(decode_mutf8(&[0xC3]).is_err());
        assert!(decode_mutf8(&[0xE4, 0xB8]).is_err());
        assert!(decode_mutf8(&[0xED, 0xA0, 0xBD, 0xED]).is_err());
        assert!(decode_mutf8(&[0xED, 0xA0, 0xBD, 0xED, 0x00, 0x00]).is_err());
        // Overlong forms other than the encoded NUL.
        assert!(decode_mutf8(&[0xC1, 0x81]).is_err());
    }

    #[test]
    fn the_terminator_and_the_encoded_nul_are_both_supported() {
        assert_eq!(decode_mutf8(&[0xC0, 0x80]).unwrap(), "\0");
        // A bare NUL ends the string; the tail is ignored.
        assert_eq!(decode_mutf8(b"ab\0cd").unwrap(), "ab");
    }
}
