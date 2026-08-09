//! RFC 4231 test vectors for HMAC-SHA-256.
//!
//! RFC 4231 §4 defines seven test cases for the HMAC-SHA-2 family. The keys, data and expected
//! HMAC-SHA-256 outputs below are the values that specification publishes; they are reproduced here
//! rather than generated from this crate's own implementation, which would make the test a tautology.
//!
//! Case 5 is the only case where the RFC specifies a truncated output (`HMAC-SHA-256-128`, the
//! leading 128 bits). It is asserted as a prefix, because that is what the specification states, and
//! inventing the remaining 128 bits to make the test look like the others would be fabricating a
//! vector.
//!
//! Cases 6 and 7 use a 131-byte key, longer than SHA-256's 64-byte block, and therefore exercise
//! RFC 2104 §2's rule that such a key is replaced by its own hash before padding. Case 7 additionally
//! uses a message longer than the block size.

use bioprism_bundle::{KeyIdentity, SecretKey};

fn tag_hex(key: &[u8], data: &[u8]) -> String {
    let key = SecretKey::new(KeyIdentity::new("rfc4231"), key.to_vec());
    let tag = key.authenticate(data);
    tag.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn rfc4231_case_1_twenty_byte_key_and_a_short_message() {
    assert_eq!(
        tag_hex(&[0x0b; 20], b"Hi There"),
        "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
    );
}

#[test]
fn rfc4231_case_2_a_four_byte_key_exercises_zero_padding() {
    assert_eq!(
        tag_hex(b"Jefe", b"what do ya want for nothing?"),
        "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
    );
}

#[test]
fn rfc4231_case_3_fifty_bytes_of_data_under_a_twenty_byte_key() {
    assert_eq!(
        tag_hex(&[0xaa; 20], &[0xdd; 50]),
        "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe"
    );
}

#[test]
fn rfc4231_case_4_a_twenty_five_byte_counting_key() {
    let key: Vec<u8> = (0x01u8..=0x19u8).collect();
    assert_eq!(key.len(), 25);
    assert_eq!(
        tag_hex(&key, &[0xcd; 50]),
        "82558a389a443c0ea4cc819899f2083a85f0faa3e578f8077a2e3ff46729665b"
    );
}

#[test]
fn rfc4231_case_5_specifies_only_the_leading_128_bits() {
    let full = tag_hex(&[0x0c; 20], b"Test With Truncation");
    assert_eq!(&full[..32], "a3b6167473100ee06e0c796c2955552b");
}

#[test]
fn rfc4231_case_6_a_key_longer_than_the_block_is_hashed_first() {
    assert_eq!(
        tag_hex(
            &[0xaa; 131],
            b"Test Using Larger Than Block-Size Key - Hash Key First"
        ),
        "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
    );
}

#[test]
fn rfc4231_case_7_an_oversized_key_and_an_oversized_message() {
    let data = b"This is a test using a larger than block-size key and a larger \
than block-size data. The key needs to be hashed before being used by the HMAC algorithm.";
    assert_eq!(data.len(), 152);
    assert_eq!(
        tag_hex(&[0xaa; 131], data),
        "9b09ffa71b942fcb27635fbcd5b0e944bfdc63644f0713938a7f51535c3a35e2"
    );
}
