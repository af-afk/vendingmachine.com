
use stylus_sdk::{alloy_sol_types::sol, stylus_core::calls::errors::Error};

pub(crate) fn unpack_err(x: Error) -> Vec<u8> {
    match x {
        Error::Revert(x) => x,
        _ => unimplemented!()
    }
}

#[macro_export]
macro_rules! unpack_on_err {
    ($rd:expr, $conv:ident) => {{
        use stylus_sdk::alloy_sol_types::SolError;
        $rd.map_err(|x| $conv{_0: $crate::errors::unpack_err(x).into()}.abi_encode())
    }};
}

#[macro_export]
macro_rules! revert {
    ($err:ident) => { return Err($err{}.abi_encode()); }
}

sol!("./src/IErrors.sol");

pub use IErrors::*;
