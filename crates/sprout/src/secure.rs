use crate::utils::variables::VariableController;
use anyhow::Result;

/// Secure boot services.
pub struct SecureBoot;

impl SecureBoot {
    /// Checks if Secure Boot is enabled on the system.
    /// This might fail if retrieving the variable fails in an irrecoverable way.
    pub fn enabled() -> Result<bool> {
        // The SecureBoot variable will tell us whether Secure Boot is enabled at all.
        VariableController::GLOBAL.get_bool("SecureBoot")
    }
}
