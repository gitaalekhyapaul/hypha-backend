//! SP1 authorization guest entrypoint for Raziel.

#![no_main]

use raziel_authorization_core::{AuthorizationGuestInputV1, authorize};

sp1_zkvm::entrypoint!(main);

pub fn main() {
    let input = sp1_zkvm::io::read::<AuthorizationGuestInputV1>();
    let result = authorize(&input).unwrap_or_else(|_| panic!("authorization guest rejected input"));
    sp1_zkvm::io::commit_slice(&result.public_values_bytes);
}
