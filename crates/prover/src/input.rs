use std::fmt::{self, Debug, Formatter};

use raziel_authorization_core::{
    AuthorizationGuestInputV1, AuthorizationPublicValuesV1, authorize,
};

use crate::ProverError;

pub struct PreparedGuestInput {
    input: AuthorizationGuestInputV1,
    expected_public_values: AuthorizationPublicValuesV1,
    expected_public_values_bytes: Vec<u8>,
}

impl PreparedGuestInput {
    pub const fn input(&self) -> &AuthorizationGuestInputV1 {
        &self.input
    }

    pub const fn expected_public_values(&self) -> &AuthorizationPublicValuesV1 {
        &self.expected_public_values
    }

    pub fn expected_public_values_bytes(&self) -> &[u8] {
        &self.expected_public_values_bytes
    }
}

impl Debug for PreparedGuestInput {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedGuestInput")
            .field("private_fields", &"<redacted>")
            .finish()
    }
}

pub fn prepare_guest_input(
    input: AuthorizationGuestInputV1,
) -> Result<PreparedGuestInput, ProverError> {
    let result = authorize(&input)?;
    Ok(PreparedGuestInput {
        input,
        expected_public_values: result.public_values,
        expected_public_values_bytes: result.public_values_bytes,
    })
}
