mod common;
mod convert;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

use self::convert::generate_auto_try_from;

#[proc_macro_derive(AutoTryFrom, attributes(auto_try_from))]
pub fn auto_try_from_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match generate_auto_try_from(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

// proc_macroのテストは統合テストで行う
