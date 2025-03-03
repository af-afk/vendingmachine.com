use stylus_sdk::alloy_primitives::{address, Address, U256};

pub static OWNER_ADDR: Address = address!("6221a9c005f6e47eb398fd867784cacfdcfff4e7");

pub static CHAINLINK_VRF_ADDR: Address = address!("0x6221a9c005f6e47eb398fd867784cacfdcfff4e7");

pub static CHAINLINK_PRICE_ADDR: Address = address!("0x6221a9c005f6e47eb398fd867784cacfdcfff4e7");

pub static CHAINLINK_NUM_WORDS: u32 = 1;

pub static ESTIMATED_CALLBACK_LIMIT: u32 = 100; // TODO

pub static CHAINLINK_VRF_CONFIRMATIONS: u16 = 1;

pub static USD_DECIMALS: u8 = 6;

pub static MAX_U256_VALUE: U256 = U256::from_limbs([18446744073709551615, 4294967295, 0, 0]);
