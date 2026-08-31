//! Lowercase hex rendering, kept in one place.
//!
//! Both the HMAC path in [`crate::mac`] and the Ed25519 path in [`crate::signature`] render raw
//! bytes into the textual form that appears inside a tag or a signature string. Those strings are
//! compared byte-for-byte by verifiers, so two independent encoders are a standing hazard: a
//! divergence in either one — uppercase digits, a different padding rule — would not fail to
//! compile and would not fail either module's own tests, it would simply make tags written by one
//! release unverifiable by another. One encoder cannot drift from itself.

/// Renders `bytes` as lowercase hex, two characters per byte, with no separator or prefix.
pub(crate) fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
