
pub fn unpack_u8(data: &[u8]) -> Option<u8> {
    if data.len() != 32 {
        None
    } else {
        Some(data[31] as u8)
    }
}

#[test]
fn test_unpack_u8() {
    assert_eq!(
        8,
        unpack_u8(&const_hex::decode("0000000000000000000000000000000000000000000000000000000000000008").unwrap()).unwrap()
    );
}
