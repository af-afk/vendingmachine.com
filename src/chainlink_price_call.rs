use stylus_sdk::{
    alloy_primitives::{Address, I256, U256},
    alloy_sol_types::{sol, SolCall, SolError},
    stylus_core::calls::{context::Call, CallAccess},
};

use crate::{calldata::unpack_u8, errors::*, immutables::*};

sol!("./src/AggregatorV3Interface.sol");

pub fn decimals(access: &dyn CallAccess, addr: Address) -> Result<u8, Vec<u8>> {
    unpack_u8(&unpack_on_err!(
        access.static_call(
            &Call::new(),
            addr,
            &AggregatorV3Interface::decimalsCall {}.abi_encode()
        ),
        ErrChainlinkDecimals
    )?)
    .ok_or(ErrUnpackU8 {}.abi_encode())
}

pub fn latest_round_data_price(access: &dyn CallAccess, addr: Address) -> Result<I256, Vec<u8>> {
    let d = unpack_on_err!(
        access.static_call(
            &Call::new(),
            addr,
            &AggregatorV3Interface::latestRoundDataCall {}.abi_encode()
        ),
        ErrChainlinkRound
    )?;
    if d.len() != 32 * 5 {
        return Err(ErrChainlinkRoundUnpack { _0: d.into() }.abi_encode());
    }
    Ok(I256::from_le_bytes::<32>(d[32..64].try_into().map_err(
        |_| ErrChainlinkRoundUnpack { _0: d.into() }.abi_encode(),
    )?))
}

// Get the latest round price data by combining the decimals, and the
// difference in our targeted decimals amount, with the return data.
pub fn get_price(access: &dyn CallAccess, addr: Address) -> Result<U256, Vec<u8>> {
    let d = decimals(access, addr)?;
    let r = latest_round_data_price(access, addr)?;
    if r.is_negative() {
        revert!(ErrChainlinkPriceNegative);
    }
    let r = r.into_raw();
    let d = if USD_DECIMALS > d {
        USD_DECIMALS - d
    } else {
        // We don't have a good way of handling situations where the decimals are
        // lower than 6. So we're oging to assume this is 1 for now. In practice,
        // this will be 8 decimals, so we'll get two decimals to trim in the real world.
        // We won't test situations where that isn't the case.
        1
    };
    // Get the price being price / (10 ** decimals).
    Ok(r.checked_div(U256::from(10).pow(U256::from(d)))
        .ok_or(ErrCheckedDiv {}.abi_encode())?)
}
