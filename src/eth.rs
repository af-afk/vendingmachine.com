use stylus_sdk::{
    alloy_primitives::{Address, U256},
    stylus_core::calls::ValueTransfer,
};

pub fn transfer(
    transfer: &dyn ValueTransfer,
    recipient: Address,
    amt: U256,
) -> Result<(), Vec<u8>> {
    transfer.transfer_eth(recipient, amt)
}
