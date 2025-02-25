use stylus_sdk::{
    alloy_primitives::*,
    alloy_sol_types::{sol, SolError},
    prelude::*,
    storage::*,
    stylus_core::{self, calls::context::Call},
};

mod chainlink_call;

extern crate alloc;

/// Any outstanding requests to purchase a NFT using the vending machine.
#[storage]
struct StorageQueue {
    submitter: StorageAddress,
    eth_amt: StorageU256,
}

/// A NFT that can be distributed, including the amount of the NFT outstanding.
#[storage]
struct StorageNFTDistributable {
    address: StorageAddress,
    amt: StorageU256,
}

#[storage]
struct StorageLevel {
    // The USD amount that's the minimum to participate in owning a NFT
    // at this level.
    usd_min: StorageU256,

    // NFTs ready to be distributed, and what's left.
    nfts_distributeable: StorageVec<StorageNFTDistributable>,
}

#[entrypoint]
#[storage]
struct StorageVendingMachine {
    // Version of the contract. If set to 0, this won't function, so this
    // needs setup first.
    version: StorageU256,

    // Allowed submitters to bundle new NFTs in each tranche.
    submitters: StorageMap<Address, StorageBool>,

    // The Chainlink price feed address.
    chainlink_price_feed: StorageAddress,

    // The Chanlink VRF request address.
    chainlink_vrf_request: StorageAddress,

    // Is a request to the Chainlink VRF pending?
    chainlink_vrf_pending: StorageBool,

    // Outstanding requests to distribute NFTs.
    queue: StorageVec<StorageQueue>,

    // Levels that can be used to distribute with. The lowest amount first.
    // Finding the tranche to distribute with is done with binary search.
    levels: StorageVec<StorageLevel>,
}

sol! {
    /* ~~~~~~ EVENTS ~~~~~~ */

    event LockedUpTokens(
        address indexed recipient,
        uint256 indexed amount
    );

    /* ~~~~~~ ERRORS ~~~~~~ */

    error ErrNotSetup();

    error ErrInvalidRecipient(address sender);
}

macro_rules! require {
    ($cond:expr, $err:expr) => {
        if !($cond) {
            Err($err.abi_encode())?;
        }
    };
}

#[public]
impl StorageVendingMachine {
    // Lock up some tokens using ETH. Gets the amount from the amount
    // payable. This function is usable by anyone and begins the deposit user
    // story. Returns the ticket for the redeeming taken.
    pub fn lockup(&mut self, recipient: Address) -> Result<U256, Vec<u8>> {
        require!(!self.version.is_zero(), ErrNotSetup {});
        // Make sure someone doesn't supply a zero recipient address.
        require!(
            !recipient.is_zero(),
            ErrInvalidRecipient {
                sender: self.vm().msg_sender()
            }
        );
        let amt = self.vm().msg_value();
        // Extend the internal queue with our request before proceeding.
        let ticket_no = U256::from(self.queue.len());
        let mut ticket = self.queue.grow();
        ticket.submitter.set(recipient);
        ticket.eth_amt.set(amt);
        stylus_core::log(
            self.vm(),
            LockedUpTokens {
                recipient,
                amount: amt,
            },
        );
        if !self.chainlink_vrf_pending.get() {
            // Since we haven't made the request to Chainlink, let's start to request it.
            self.vm()
                .static_call(&Call::new(), self.chainlink_vrf_request.get(), &vec![])?;
        }
        Ok(ticket_no)
    }
}

// Reproduction of the equivalent function in Chainlink's
// VRFV2PlusWrapperConsumerBase.
pub fn request_randomness(
    callback_gas_limit: u32,
    request_confirmations: u16,
    num_words: u32,
    extra_args: Vec<u8>
) -> Result<(U256, U256), Vec<u8>> {
    Ok((U256::ZERO, U256::ZERO))
}
