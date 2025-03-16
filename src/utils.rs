use stylus_sdk::alloy_primitives::{aliases::U96, U256};

pub fn u96_to_u256(x: U96) -> U256 {
    let mut b = [0u8; 32];
    b[32 - 12..].copy_from_slice(&x.to_be_bytes::<12>());
    U256::from_be_bytes(b)
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod test {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_u96_to_u256(x in any::<[u8; 12]>()) {
            let x = U96::from_be_bytes(x);
            assert_eq!(x.to_string(), u96_to_u256(x).to_string());
        }
    }
}
