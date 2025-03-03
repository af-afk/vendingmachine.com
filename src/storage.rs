use stylus_sdk::{
    alloy_primitives::{aliases::*, *},
    prelude::*,
    storage::*,
};

/// A NFT that can be distributed, including the amount of the NFT outstanding.
#[storage]
pub struct StorageNFTDistributable {
    pub address: StorageAddress,
    pub to_send_ids: StorageVec<StorageU256>,
}

#[storage]
pub struct StorageLevel {
    // The USD amount that's the minimum to participate in owning a NFT
    // at this level. Stored as a 1e6 number.
    pub usd_min: StorageU256,

    // NFTs ready to be distributed, and what's left.
    pub nfts_distributeable: StorageVec<StorageU32>,

    // These are NFTs keyed to the contract, able to be moved around if the
    // vector has its direction changed.
    pub nfts: StorageMap<U32, StorageNFTDistributable>,
}

#[entrypoint]
#[storage]
pub struct StorageVendingMachine {
    // Version of the contract. If set to 0, this won't function, so this
    // needs setup first.
    pub version: StorageU256,

    // Allowed submitters to bundle new NFTs in each tranche.
    pub submitters: StorageMap<Address, StorageBool>,

    // The Chainlink price feed address.
    pub chainlink_price_feed: StorageAddress,

    // The fee that's needing to be paid by every user for the current request.
    pub chainlink_vrf_fee: StorageU256,

    // Is a request to the Chainlink VRF pending?
    pub chainlink_vrf_pending: StorageBool,

    // Outstanding requests to distribute NFTs. The word here is unpacked to
    // StorageQueue, which contains (address . uint96), submitter and eth amount.
    pub queue: StorageVec<StorageU256>,

    // Levels that can be used to distribute with. The lowest amount first.
    // Finding the tranche to distribute with is done with binary search.
    pub levels: StorageVec<StorageLevel>,
}

pub fn unpack_queue_item(x: U256) -> (U96, Address) {
    let b: [u8; 32] = x.to_be_bytes();
    let addr = Address::from_slice(&b[12..32]);
    let amt = U96::from_be_bytes::<12>(b[0..12].try_into().unwrap());
    (amt, addr)
}

pub fn pack_queue_item(amt: U256, addr: Address) -> U256 {
    let mut b = [0u8; 32];
    b[0..12].copy_from_slice(&amt.to_be_bytes::<32>()[20..]);
    b[12..32].copy_from_slice(addr.as_slice());
    U256::from_be_bytes(b)
}

#[cfg(all(not(target_arch = "wasm32"), test))]
mod test {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn pack_unpack_queue_item(amt in any::<[u8; 12]>(), addr in any::<[u8; 20]>()) {
            let mut amt_u = [0u8; 32];
            amt_u[20..].copy_from_slice(&amt);
            let amt_u = U256::from_be_bytes(amt_u);
            let amt = U96::from_be_bytes(amt);
            let addr = Address::from(addr);
            assert_eq!(
                (amt, addr),
                unpack_queue_item(pack_queue_item(amt_u, addr)
            ));
        }
    }
}
