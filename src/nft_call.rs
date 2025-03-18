use stylus_sdk::{
    alloy_primitives::{Address, U256},
    alloy_sol_types::{sol, SolCall},
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
        access.call(
            &Call::new(),
            addr,
            &transferCall { recipient, id }.abi_encode()
        ),
        ErrNFTTransfer
    )?;
    Ok(())
}

pub fn transfer_from(
    access: &dyn CallAccess,
    addr: Address,
    sender: Address,
    recipient: Address,
    id: U256,
) -> Result<(), Vec<u8>> {
    unpack_on_err!(
        access.call(
            &Call::new(),
            addr,
            &transferFromCall { sender, recipient, id }.abi_encode()
        ),
        ErrNFTTransfer
    )?;
    Ok(())
}
