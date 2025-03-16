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

mod utils;
use utils::*;

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

use core::cmp::Ordering;

sol!("./src/IEvents.sol");
use IEvents::*;

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
    #[payable]
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
        require!(value <= MAX_U256_VALUE, ErrTooMuchValue {});
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
        self.queue.grow().set(pack_queue_item(value, recipient));
        Ok(ticket_no)
    }

    // Called by the Chainlink VRF coordinator once we've received the
    // random VRF words.
    pub fn raw_fulfill_random_words(
        &mut self,
        ticket: U256,
        words: Vec<U256>,
    ) -> Result<(), Vec<u8>> {
        let vm = self.vm();
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
        // Using this price function will do the decimal place conversion for us.
        let price = chainlink_price_call::get_price(self.vm(), CHAINLINK_PRICE_ADDR)?;
        // This vending machine is slightly dangerous to use, because if there
        // aren't enough NFTs to distribute, it will return the user's
        // investment back.
        'queue: for i_t in 0..self.queue.len() {
            let (tic_eth_amt, tic_addr) = unpack_queue_item(self.queue.get(i_t).unwrap());
            let usd_invested = price * u96_to_u256(tic_eth_amt);
            // To reduce the gas profile here, this code does a simple binary search.
            // Once it finds a level item that could be used as the target, it scans
            // right once, and if that item can't be purchased (or doesn't exist),
            // then it assumes that. If not, it continues searching.
            if let Some(level_i) = self.pick_level(usd_invested) {
                let level = self.levels.getter(level_i).unwrap();
                // Randomly pick a NFT to distribute. We know there will be one here due
                // to the pick function. We need the position of the NFT in the vector so we
                // can optionally pop it later.
                let nft_addr_i = rng.next_u32() as usize % level.nfts_distributeable.len();
                let nft_addr = level
                    .nfts_distributeable
                    .get(nft_addr_i)
                    .unwrap();
                // Pick the NFT id from the other vec, so that we can start to pop from this if we're done.
                let nft_id = self
                    .nft_ids_to_send
                    .getter(nft_addr)
                    .get(rng.next_u32() as usize % self.nft_ids_to_send.getter(nft_addr).len())
                    .unwrap();
                if let Ok(()) = nft_call::transfer(vm, nft_addr, tic_addr, nft_id) {
                    // We're good. The NFT was sent correctly. Let's send them their rebate
                    // if more than one user contributed to the queue by taking the fee amount,
                    // then reducing it by the number of users multiplied by the amount.
                    let fee_rebate = self.calc_fee_rebate()?;
                    if !fee_rebate.is_zero() {
                        // It's an issue if the contract does not have enough ETH to send this,
                        // which means we have a bug. But what could also happen here is that the
                        // user has a payable function, and they break on receiving the amount.
                        // If this is the case, we just continue as-is.
                        //let _ = eth::send(tic_addr, fee_rebate);
                    }
                    // We need to pop that we spent this NFT id!
                    {
                        let mut ids = self.nft_ids_to_send.get(nft_addr);
                        let last_id = self.nft_ids_to_send.get(nft_addr).get(self.nft_ids_to_send.len()-1).unwrap();
                        let mut ids = self.nft_ids_to_send.setter(nft_addr);
                        ids.setter(nft_id).unwrap().set(last_id);
                        ids.pop();
                    }
                    // Was that the last NFT that we spent id that we spent from this NFT?
                    // We need to pop from it.
                    if self.nft_ids_to_send.get(nft_addr).is_empty() {
                        let level_nfts_len = self.levels.get(level_i).unwrap().nfts_distributeable.len();
                        let last_nft_addr = self.levels.getter(level_i).unwrap().nfts_distributeable.get(level_nfts_len - 1).unwrap();
                        let mut level = self.levels.setter(level_i).unwrap();
                        level.nfts_distributeable.setter(nft_addr_i).unwrap().set(last_nft_addr);
                        level.nfts_distributeable.pop();
                    }
                }
            }
            // Looks like the user wasn't able to get a NFT. We need to send them
            // back their initial investment minus the fee.
        }
        Ok(())
    }

    pub fn calc_fee_rebate(&self) -> Result<U256, Vec<u8>> {
        let vrf_fee = self.chainlink_vrf_fee.get() * SCALING_FACTOR_U256;
        Ok((vrf_fee
            .checked_sub(vrf_fee / U256::from(self.queue.len()))
            .ok_or(ErrCheckedSub {}.abi_encode()))?
            / SCALING_FACTOR_U256)
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

impl StorageVendingMachine {
    // Pick an initial NFT level using a very simple binary search where the
    // rightmost element following the current element that meets the minimum
    // must be unable to be sent to be successful.
    pub fn pick_level(&self, usd_amt: U256) -> Option<usize> {
        let nft = (0..self.levels.len())
            .collect::<Vec<_>>()
            .binary_search_by(|i| {
                let l = self.levels.get(*i).unwrap();
                if l.usd_min.get() <= usd_amt {
                    // Check if the next element can be bought, if so, continue search.
                    if let Some(lx) = self.levels.get(*i + 1) {
                        if lx.usd_min.get() > usd_amt {
                            Ordering::Equal
                        } else {
                            Ordering::Less
                        }
                    } else {
                        Ordering::Equal
                    }
                } else {
                    Ordering::Greater
                }
            })
            .ok()?;
        (0..=nft)
            .rev()
            .find(|i| self.levels.get(*i).unwrap().nfts_distributeable.len() > 0)
    }

    // Pick an initial NFT level using a very simple binary search where the
    // rightmost element following the current element that meets the minimum
    // must be unable to be sent to be successful. It's possible to probably
    // optimise this further since we know the rightmost element is not
    // appropriate for us, though with Stylus caching, we can afford to be
    // succinct here and avoid supporting that case. Though, I wonder to what
    // extent that's true.
    pub fn pick_level_manual(&self, usd_amt: U256) -> Option<usize> {
        let mut left = 0;
        let mut right = self.levels.len() as isize - 1;
        let mut nft = None;
        while left <= right {
            let mid = left + (right - left) / 2;
            let next_level_amt = self.levels.get(mid as usize + 1).map(|x| x.usd_min.get());
            let l = self.levels.get(mid as usize)?;
            if l.usd_min.get() <= usd_amt {
                if let Some(next_level_amt) = next_level_amt {
                    if next_level_amt > usd_amt {
                        nft = Some(mid as usize);
                        break;
                    }
                } else {
                    nft = Some(mid as usize);
                    break;
                }
                left = mid + 1;
            } else {
                right = mid - 1;
            }
        }
        (0..=nft?)
            .rev()
            .find(|i| self.levels.get(*i).unwrap().nfts_distributeable.len() > 0)
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod test {
    use super::*;
    use proptest::prelude::*;
    use stylus_sdk::{host::VM, testing::vm::TestVM};

    proptest! {
        #[test]
        fn test_nft_picking(
            nft_i in 0usize..1000,
            mut items in proptest::collection::vec((any::<u64>(), 0u64..10), 1000)
        ) {
            items.sort_by(|(x, _), (y, _)| x.cmp(y));
            let host = VM{host:Box::new(TestVM::new())};
            let mut c = unsafe { StorageVendingMachine::new(U256::ZERO, 0, host) };
            let addr = Address::from([1u8; 20]);
            for i in 0..10 {
                c.nft_ids_to_send.setter(addr).setter(i).unwrap().set(U256::from(i));
            }
            for (usd_min, nfts_distributeable) in items.iter() {
                let mut l = c.levels.grow();
                l.usd_min.set(U256::from(*usd_min));
                for _ in 0..*nfts_distributeable {
                    l.nfts_distributeable.grow().set(addr);
                }
            }
            let usd_amt = U256::from(items.get(nft_i).unwrap().0);
            let nft_i =
                (0..=nft_i).rev().find(|i| c.levels.get(*i).unwrap().nfts_distributeable.len() > 0);
            assert_eq!(nft_i, c.pick_level(usd_amt));
        }

        #[test]
        fn test_fee_rebate_calc(_deposits in any::<u64>()) {
            let host = VM{host:Box::new(TestVM::new())};
            let mut c = unsafe { StorageVendingMachine::new(U256::ZERO, 0, host) };
            unimplemented!()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn native_keccak256(bytes: *const u8, len: usize, output: *mut u8) {
    use core::slice;
    use tiny_keccak::{Hasher, Keccak};
    let mut hasher = Keccak::v256();
    let data = unsafe { slice::from_raw_parts(bytes, len) };
    hasher.update(data);
    let output = unsafe { slice::from_raw_parts_mut(output, 32) };
    hasher.finalize(output);
}
