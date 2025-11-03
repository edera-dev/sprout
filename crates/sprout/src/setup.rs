use anyhow::{Context, Result};

/// Initializes the UEFI environment.
pub fn init() -> Result<()> {
    // Initialize the uefi internals.
    uefi::helpers::init().context("unable to initialize uefi")?;
    Ok(())
}
