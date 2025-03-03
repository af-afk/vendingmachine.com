use stylus_sdk::{storage::*, prelude::*, alloy_primitives::*};

/// Any outstanding requests to purchase a NFT using the vending machine.
pub struct StorageQueue {
    pub submitter: StorageAddress,
    pub eth_amt: StorageU96,
}

/// A NFT that can be distributed, including the amount of the NFT outstanding.
#[storage]
pub struct StorageNFTDistributable {
    pub address: StorageAddress,
    pub to_send_ids: StorageVec<U256>
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
    pub nfts: StorageMap<U32, StorageNFTDistributable>
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

    // Outstanding requests to distribute NFTs.
    pub queue: StorageVec<StorageQueue>,

    // Levels that can be used to distribute with. The lowest amount first.
    // Finding the tranche to distribute with is done with binary search.
    pub levels: StorageVec<StorageLevel>,
}
