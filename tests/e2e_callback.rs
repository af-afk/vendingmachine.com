#![cfg(not(target_arch = "wasm32"))]

use libvendingmachine::*;

use stylus_sdk::{
    alloy_primitives::{Address, U256},
    prelude::*,
    testing::vm::TestVM,
};

use proptest::prelude::*;

proptest! {
    #[test]
    fn e2e_callback_1(
        rng_word in any::<[u64; 4]>(),
        levels in proptest::collection::vec((
            any::<[u8; 20]>(),
            any::<[u64; 4]>(),
            any::<[u64; 4]>()
        ), 0..10),
        _ in any::<u64>()
     ) {
        // Test the contract by taking an investment from Saruul and Annie after
        // setting up with three NFTs. Then, do the callback to start
        // to send them their amounts.
        let nfts =
            levels
                .into_iter()
                .map(|(addr, amt, id)| (
                    Address::from(addr),
                    U256::from_limbs(amt),
                    U256::from_limbs(id)
                ))
                .collect::<Vec<_>>();
        let rng_word = U256::from_limbs(rng_word);
        let vm = TestVM::new();
        // Mock the decimals function to be like Chainlink's.
        vm.mock_static_call(
            CHAINLINK_PRICE_ADDR,
            const_hex::decode("313ce567").unwrap(),
            //decimals from 0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419 on mainnet
            Ok(const_hex::decode("0000000000000000000000000000000000000000000000000000000000000008").unwrap())
        );
        // Mock the call to the price function to be like Chainlink's.
        vm.mock_static_call(
            CHAINLINK_PRICE_ADDR,
            const_hex::decode("feaf968c").unwrap(),
            //latestRoundData from 0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419 on mainnet
            Ok(const_hex::decode("000000000000000000000000000000000000000000000007000000000000243a0000000000000000000000000000000000000000000000000000002c003479800000000000000000000000000000000000000000000000000000000067d948790000000000000000000000000000000000000000000000000000000067d94887000000000000000000000000000000000000000000000007000000000000243a").unwrap())
        );
        let mut c = StorageVendingMachine::from(&vm);
        let level_amts = nfts.iter().map(|(_, amt, _)| *amt).collect::<Vec<_>>();
        let msg_sender = c.vm().msg_sender();
        c.setup(vec![msg_sender], level_amts.clone()).unwrap();
        // This should break!
        panic_guard! {
            let _ = c.setup(vec![msg_sender], level_amts).unwrap_err();
        }
        c.add_nfts(nfts).unwrap();
        vm.set_sender(CHAINLINK_VRF_ADDR);
        c.raw_fulfill_random_words(U256::ZERO, vec![rng_word]).unwrap();
        dbg!(vm.get_emitted_logs());
    }
}
