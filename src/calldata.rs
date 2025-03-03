
pub fn unpack_u8(data: &[u8]) -> Option<u8> {
    if data.len() != 32 {
        None
    } else {
        data[32-8..].try_into().ok().map(u8::from_be_bytes)
    }
}
