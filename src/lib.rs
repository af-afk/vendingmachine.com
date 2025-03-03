use stylus_sdk::{
    alloy_primitives::*,
    alloy_sol_types::{sol, SolError},
    prelude::*,
    stylus_core,
};

#[macro_use]
pub mod errors;
use errors::*;

mod calldata;

mod chainlink_price_call;
mod chainlink_vrf_call;
mod nft_call;

mod immutables;
pub use immutables::*;

pub mod storage;
pub use storage::*;

extern crate alloc;

use rand_chacha::{
    rand_core::{RngCore, SeedableRng},
    ChaCha8Rng,
};

sol! {
    /* ~~~~~~ EVENTS ~~~~~~ */

    event LockedUpTokens(
        address indexed recipient,
        uint256 indexed amount
    );

    event RandomnessRequested(uint256 indexed ticketNo);
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
        let value = self.vm().msg_value();
        // Someone has tried to supply us with 0 value!
        require!(value > U256::ZERO, ErrNoValue {});
        // Prevent people from using this if there aren't any NFTs to distribute.
        let has_nfts = (0..self.levels.len())
            .find(|i| self.levels.get(*i).unwrap().nfts_distributeable.len() > 0)
            .is_some();
        require!(has_nfts, ErrNoNfts {});
        // Extend the internal queue with our request before proceeding.
        if !self.chainlink_vrf_pending.get() {
            // Since we haven't made the request to Chainlink, let's start to request it.
            let fee = self.estimate_fee()?;
            // Store the request price, which will become the flat fee from now on for everyone.
            self.chainlink_vrf_fee.set(fee);
            // We need to make the request for the random words.
            let ticket_no = chainlink_vrf_call::request_random_words_in_native(
                self.vm(),
                CHAINLINK_VRF_ADDR,
                fee,
                ESTIMATED_CALLBACK_LIMIT,
                CHAINLINK_VRF_CONFIRMATIONS,
                CHAINLINK_NUM_WORDS,
                Bytes::new(),
            )?;
            // We don't do any validation the randomness ticket is accurate. This
            // opens us up to an issue with Chainlink, but in my opinion, that's
            // preferable to a situation where we need an escape hatch (or to even
            // assume Chainlink might misbehave). That's beyond the scope of this
            // example, as obviously the VRF could just not reply in an extraneous state,
            // bringing this platform down.
            stylus_core::log(
                self.vm(),
                RandomnessRequested {
                    ticketNo: ticket_no,
                },
            );
            self.chainlink_vrf_pending.set(true);
        }
        let value = value
            .checked_sub(self.chainlink_vrf_fee.get())
            .ok_or(ErrCheckedSub {}.abi_encode())?;
        // Push the user's request for the NFT into the queue.
        let ticket_no = U256::from(self.queue.len());
        stylus_core::log(
            self.vm(),
            LockedUpTokens {
                recipient,
                amount: value,
            },
        );
        let mut t = self.queue.grow();
        t.submitter.set(recipient);
        t.eth_amt.set(value);
        Ok(ticket_no)
    }

    // Called by the Chainlink VRF coordinator once we've received the
    // random VRF words.
    pub fn raw_fulfill_random_words(
        &mut self,
        ticket: U256,
        words: Vec<U256>,
    ) -> Result<(), Vec<u8>> {
        // At this point, we need to start to refund the users that're in the
        // queue from the balance of the tokens we've received, and start to
        // pick the NFTs that were supplied to this contract. We use the
        // random words to seed a random number generator.
        let mut rng = ChaCha8Rng::from_seed(
            words
                .into_iter()
                .fold(U256::MAX, |acc, v| acc ^ v)
                .to_le_bytes::<32>(),
        );
        let price = chainlink_price_call::get_price(self.vm(), CHAINLINK_PRICE_ADDR)?;
        // This vending machine is slightly dangerous to use, because if there
        // aren't enough NFTs to distribute, it will return the user's
        // investment back.
        'queue: for i_t in 0..self.queue.len() {
            let mut t = self.queue.setter(i_t).unwrap();
            let usd_invested = t
                .eth_amt
                .checked_mul(price)
                .ok_or(ErrCheckedMul {}.abi_encode())?;
            for i_l in (0..self.levels.len()).rev() {
                let l = self.levels.get(i_l).unwrap();
                // We start to work our way back through the levels.
                if usd_invested < self.levels.get(i_l).unwrap().usd_min.get() {
                    continue;
                }
                // The user has enough for this NFT! Use the RNG to pick one!
                if l.nfts_distributeable.len() == 0 {
                    // We need to continue this user's search.
                    continue;
                }
                // Making assumptions here that the usize word is 32 bits.
                let nft_key = l
                    .nfts_distributeable
                    .get((rng.next_u32() % l.nfts_distributeable.len() as u32) as usize)
                    .unwrap();
                let len_nfts_distributeable =
                    self.levels.get(i_l).unwrap().nfts_distributeable.len();
                let mut n = self.levels.get(i_l).unwrap().nfts.setter(nft_key);
                // We should send them their NFT! We need to find which one we want to send though.
                let nft_id =
                // We need this replacement id to swap the vector item when we do deletion.
                // If there's only one item, then this operation will be a waste of time, but harmless.
                let replacement_id = l.nfts_distributeable.get(len_nfts_distributeable - 1).unwrap();
                // If there's none left of this operation, we need to pop this from our NFT list.
                if n.spent_amt.get() == n.available_amt.get() {
                    // We can "delete" this item by replacing the NFT at the last one with
                    // this, then shifting left. It's not perfect randomness, but it's a
                    // constraint hack given the scenario.
                    self.levels
                        .setter(i_l)
                        .unwrap()
                        .nfts_distributeable
                        .setter(i_l)
                        .unwrap()
                        .set(replacement_id);
                    // Prune the rightmost side of the vec so we can fake delete it.
                    unsafe {
                        self.levels
                            .setter(i_l)
                            .unwrap()
                            .nfts_distributeable
                            .set_len(len_nfts_distributeable - 1);
                    }
                }
            }
            // Looks like the user wasn't able to get a NFT. We need to send them
            // back their initial investment minus the fee.
        }
    }

    pub fn estimate_fee(&self) -> Result<U256, Vec<u8>> {
        chainlink_vrf_call::calculate_request_price_native(
            self.vm(),
            CHAINLINK_VRF_ADDR,
            ESTIMATED_CALLBACK_LIMIT,
            CHAINLINK_NUM_WORDS,
        )
    }
}
