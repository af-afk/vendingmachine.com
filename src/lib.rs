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

pub mod chainlink_price_call;
mod chainlink_vrf_call;
mod eth;
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

#[public]
impl StorageVendingMachine {
    pub fn setup(
        &mut self,
        submitters: Vec<Address>,
        mut usd_levels: Vec<U256>,
    ) -> Result<(), Vec<u8>> {
        require!(self.version.is_zero(), ErrAlreadySetup {});
        self.version.set(U256::from(1));
        for s in submitters {
            self.submitters.setter(s).set(true);
        }
        usd_levels.sort();
        usd_levels.dedup();
        for u in usd_levels {
            let mut l = self.levels.grow();
            l.usd_min.set(u);
        }
        Ok(())
    }

    // Add a NFT to the list by first searching for the level to add it to,
    // then add it to the list of tokens for that NFT at that usd_min amount.
    pub fn add_nft(&mut self, addr: Address, usd_min_ask: U256, id: U256) -> Result<U256, Vec<u8>> {
        require!(
            self.submitters.get(self.vm().msg_sender()),
            ErrNotSubmitter {}
        );
        // Add a NFT at the best price point given relatively.
        let level_i = self
            .pick_level(usd_min_ask, false)
            .ok_or(ErrNoLevel {}.abi_encode())?;
        let mut level = self.levels.setter(level_i).unwrap();
        let chosen_usd_min = level.usd_min.get();
        level.nfts_distributeable.push(addr);
        self.nft_ids_to_send.setter(addr).setter(level_i).push(id);
        let msg_sender = self.vm().msg_sender();
        let contract = self.vm().contract_address();
        nft_call::transfer_from(self.vm(), addr, msg_sender, contract, id)?;
        Ok(chosen_usd_min)
    }

    pub fn add_nfts(&mut self, nfts: Vec<(Address, U256, U256)>) -> Result<Vec<U256>, Vec<u8>> {
        nfts.into_iter()
            .map(|(addr, usd_min, id)| self.add_nft(addr, usd_min, id))
            .collect::<Result<Vec<_>, _>>()
    }

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

    #[allow(non_snake_case)]
    pub fn on_E_R_C_721_Received(
        &self,
        _operator: Address,
        _from: Address,
        _token_id: U256,
        _calldata: stylus_sdk::abi::Bytes,
    ) -> Result<FixedBytes<4>, Vec<u8>> {
        Ok(FixedBytes::<4>::from([0x15, 0x0b, 0x7a, 0x02]))
    }

    // Called by the Chainlink VRF coordinator once we've received the
    // random VRF words.
    pub fn raw_fulfill_random_words(
        &mut self,
        ticket_no: U256,
        words: Vec<U256>,
    ) -> Result<(), Vec<u8>> {
        require!(
            CHAINLINK_PRICE_ADDR == self.vm().msg_sender(),
            ErrNotChainlink {}
        );
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
        for i_t in 0..self.queue.len() {
            let (tic_eth_amt, tic_addr) = unpack_queue_item(self.queue.get(i_t).unwrap());
            let usd_invested = price * u96_to_u256(tic_eth_amt);
            // This code does a simple binary search. Once it finds a level item that
            // could be used as the target, it scans right once, and if that item
            // can't be purchased (or doesn't exist), then it assumes that. If not,
            // it continues searching.
            let level_i = self.pick_level(usd_invested, true);
            if level_i.is_none() {
                // We couldn't find a suitable level for this user! We need to refund their amount.
                unimplemented!()
            }
            let level_i = level_i.unwrap();
            // Randomly pick a NFT to distribute. We know there will be one here due
            // to the pick function. We need the position of the NFT in the vector so we
            // can optionally pop it later.
            let nft_addr_i = rng.next_u32() as usize
                % self
                    .levels
                    .getter(level_i)
                    .unwrap()
                    .nfts_distributeable
                    .len();
            let nft_addr = self
                .levels
                .getter(level_i)
                .unwrap()
                .nfts_distributeable
                .get(nft_addr_i)
                .unwrap();
            // Pick the NFT id from the other vec, so that we can start to pop from this if we're done.
            let nft_id = self
                .nft_ids_to_send
                .getter(nft_addr)
                .get(
                    rng.next_u32() as usize
                        % self.nft_ids_to_send.getter(nft_addr).getter(level_i).len(),
                )
                .get(level_i)
                .unwrap();
            if let Err(_) = nft_call::transfer(self.vm(), nft_addr, tic_addr, nft_id) {
                // Looks like the user wasn't able to get a NFT. We need to send them
                // back their initial investment minus the fee.
                unimplemented!()
            }
            // We're good. The NFT was sent correctly. Let's send them their rebate
            // if more than one user contributed to the queue by taking the fee amount,
            // then reducing it by the number of users multiplied by the amount.
            let fee_rebate = self.calc_fee_rebate()?;
            if !fee_rebate.is_zero() {
                // It's an issue if the contract does not have enough ETH to send this,
                // which means we have a bug. But what could also happen here is that the
                // user has a payable function, and they break on receiving the amount.
                // If this is the case, we just continue as-is.
                let _ = eth::transfer(self.vm(), tic_addr, fee_rebate);
            }
            // We need to pop that we spent this NFT id!
            {
                // Get the last item in the ids.
                let last_id = self
                    .nft_ids_to_send
                    .get(nft_addr)
                    .get(self.nft_ids_to_send.get(nft_addr).get(level_i).len() - 1)
                    .get(level_i)
                    .unwrap();
                let mut nfts = self.nft_ids_to_send.setter(nft_addr);
                let mut ids = nfts.setter(level_i);
                ids.setter(nft_id).unwrap().set(last_id);
                ids.pop();
            }
            // Was that the last NFT that we spent id that we spent from this NFT?
            // We need to pop from it.
            if self.nft_ids_to_send.get(nft_addr).get(level_i).is_empty() {
                let level_nfts_len = self.levels.get(level_i).unwrap().nfts_distributeable.len();
                let last_nft_addr = self
                    .levels
                    .getter(level_i)
                    .unwrap()
                    .nfts_distributeable
                    .get(level_nfts_len - 1)
                    .unwrap();
                let mut level = self.levels.setter(level_i).unwrap();
                level
                    .nfts_distributeable
                    .setter(nft_addr_i)
                    .unwrap()
                    .set(last_nft_addr);
                level.nfts_distributeable.pop();
            }
            stylus_core::log(
                self.vm(),
                UserReceivedNFT {
                    nft: nft_addr,
                    id: nft_id,
                    usdAmount: usd_invested,
                    refunded: fee_rebate,
                    recipient: tic_addr,
                },
            );
        }
        // We sent the queue! Time to zero out the queue from before.
        unsafe { self.queue.set_len(0) }
        self.chainlink_vrf_pending.set(false);
        stylus_core::log(
            self.vm(),
            RandomnessResolved {
                ticketNo: ticket_no,
            },
        );
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
    pub fn pick_level(&self, usd_amt: U256, find_spendable: bool) -> Option<usize> {
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
        if find_spendable {
            (0..=nft)
                .rev()
                .find(|i| self.levels.get(*i).unwrap().nfts_distributeable.len() > 0)
        } else {
            Some(nft)
        }
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

    proptest! {
        #[test]
        fn test_nft_picking(
            nft_i in 0usize..1000,
            mut items in proptest::collection::vec((any::<u64>(), 0u64..10), 1000)
        ) {
            items.sort_by(|(x, _), (y, _)| x.cmp(y));
            let mut c = StorageVendingMachine::default();
            let addr = Address::from([1u8; 20]);
            for (i, (usd_min, nfts_distributeable)) in items.iter().enumerate() {
                let mut l = c.levels.grow();
            for x in 0..10 {
                c.nft_ids_to_send.setter(addr).setter(i).grow().set(U256::from(x));
            }
                l.usd_min.set(U256::from(*usd_min));
                for _ in 0..*nfts_distributeable {
                    l.nfts_distributeable.grow().set(addr);
                }
            }
            let usd_amt = U256::from(items.get(nft_i).unwrap().0);
            let nft_i =
                (0..=nft_i).rev().find(|i| c.levels.get(*i).unwrap().nfts_distributeable.len() > 0);
            assert_eq!(nft_i, c.pick_level(usd_amt, true));
        }
    }
}

#[test]
fn test_fee_rebate_calc() {
    let mut c = StorageVendingMachine::default();
    c.chainlink_vrf_fee.set(U256::from(1e18 as u64));
    for _ in 0..500 {
        c.queue
            .push(pack_queue_item(U256::from(100), Address::ZERO));
    }
    assert_eq!(U256::from(998e15), c.calc_fee_rebate().unwrap());
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
