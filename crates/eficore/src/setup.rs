use crate::logger;
use anyhow::{Context, Result};

/// Initializes the UEFI environment.
pub fn init() -> Result<()> {
    // Initialize the logger for Sprout.
    // NOTE: This cannot use a result type as errors need to be printed
    // using the logger, which is not initialized until this returns.
    logger::init();

    // Initialize further UEFI internals.
    uefi::helpers::init().context("unable to initialize uefi environment")?;
    Ok(())
}
