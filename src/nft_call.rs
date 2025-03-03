use stylus_sdk::{
    alloy_primitives::{Address, I256, U256},
    alloy_sol_types::{sol, SolCall, SolError},
    stylus_core::calls::{context::Call, CallAccess},
};

use crate::errors::*;

sol! {
    function transfer(address recipient, uint256 id) external;
    function transferFrom(address sender, address recipient, uint256 id) external;
}

pub fn transfer(
    access: &dyn CallAccess,
    addr: Address,
    recipient: Address,
    id: U256,
) -> Result<(), Vec<u8>> {
    unpack_on_err!(
        access.static_call(
            &Call::new(),
            addr,
            transferCall { recipient, id }.abi_encode()
        ),
        ErrNFTTransfer
    )?;
    Ok(())
}
