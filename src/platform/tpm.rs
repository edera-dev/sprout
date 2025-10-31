use crate::utils;
use anyhow::{Context, Result};
use uefi::boot::ScopedProtocol;
use uefi::proto::tcg::PcrIndex;
use uefi::proto::tcg::v2::{PcrEventInputs, Tcg};
use uefi_raw::protocol::tcg::EventType;
use uefi_raw::protocol::tcg::v2::{Tcg2HashLogExtendEventFlags, Tcg2Protocol, Tcg2Version};

/// Represents the platform TPM.
pub struct PlatformTpm;

/// Represents an open TPM handle.
pub struct TpmProtocolHandle {
    /// The version of the TPM protocol.
    version: Tcg2Version,
    /// The protocol itself.
    protocol: ScopedProtocol<Tcg>,
}

impl TpmProtocolHandle {
    /// Construct a new [TpmProtocolHandle] from the `version` and `protocol`.
    pub fn new(version: Tcg2Version, protocol: ScopedProtocol<Tcg>) -> Self {
        Self { version, protocol }
    }

    /// Access the version provided by the tcg2 protocol.
    pub fn version(&self) -> Tcg2Version {
        self.version
    }

    /// Access the protocol interface for tcg2.
    pub fn protocol(&mut self) -> &mut ScopedProtocol<Tcg> {
        &mut self.protocol
    }
}

impl PlatformTpm {
    /// The PCR for measuring the bootloader configuration into.
    pub const PCR_BOOT_LOADER_CONFIG: PcrIndex = PcrIndex(5);

    /// Acquire access to the TPM protocol handle, if possible.
    /// Returns None if TPM is not available.
    fn protocol() -> Result<Option<TpmProtocolHandle>> {
        // Attempt to acquire the TCG2 protocol handle. If it's not available, return None.
        let Some(handle) =
            utils::find_handle(&Tcg2Protocol::GUID).context("unable to determine tpm presence")?
        else {
            return Ok(None);
        };

        // If we reach here, we've already validated that the handle
        // implements the TCG2 protocol.
        let mut protocol = uefi::boot::open_protocol_exclusive::<Tcg>(handle)
            .context("unable to open tcg2 protocol")?;

        // Acquire the capabilities of the TPM.
        let capability = protocol
            .get_capability()
            .context("unable to get tcg2 boot service capability")?;

        // If the TPM is not present, return None.
        if !capability.tpm_present() {
            return Ok(None);
        }

        // If the TPM is present, we need to determine the version of the TPM.
        let version = capability.protocol_version;

        // We have a TPM, so return the protocol version and the protocol handle.
        Ok(Some(TpmProtocolHandle::new(version, protocol)))
    }

    /// Determines whether the platform TPM is present.
    pub fn present() -> Result<bool> {
        Ok(PlatformTpm::protocol()?.is_some())
    }

    /// Determine the number of active PCR banks on the TPM.
    /// If no TPM is available, this will return zero.
    pub fn active_pcr_banks() -> Result<u32> {
        // Acquire access to the TPM protocol handle.
        let Some(mut handle) = PlatformTpm::protocol()? else {
            return Ok(0);
        };

        // Check if the TPM supports `GetActivePcrBanks`, and if it doesn't return zero.
        if handle.version().major < 1 || handle.version().major == 1 && handle.version().minor < 1 {
            return Ok(0);
        }

        // The safe wrapper for this function will decode the bitmap.
        // Strictly speaking, it's not future-proof to re-encode that, but in practice it will work.
        let banks = handle
            .protocol()
            .get_active_pcr_banks()
            .context("unable to get active pcr banks")?;

        // Return the number of active PCR banks.
        Ok(banks.bits())
    }

    /// Log an event into the TPM pcr `pcr_index` with `buffer` as data. The `description`
    /// is used to describe what the event is.
    ///
    /// If a TPM is not available, this will do nothing.
    pub fn log_event(pcr_index: PcrIndex, buffer: &[u8], description: &str) -> Result<()> {
        // Acquire access to the TPM protocol handle.
        let Some(mut handle) = PlatformTpm::protocol()? else {
            return Ok(());
        };

        // Encode the description as a UTF-16 little endian string.
        let description = description
            .encode_utf16()
            .flat_map(|c| c.to_le_bytes())
            .collect::<Vec<u8>>();

        // Construct an event input for the TPM.
        let event = PcrEventInputs::new_in_box(pcr_index, EventType::IPL, &description)
            .context("unable to construct pcr event inputs")?;

        // Log the event into the TPM.
        handle
            .protocol()
            .hash_log_extend_event(Tcg2HashLogExtendEventFlags::empty(), buffer, &event)
            .context("unable to log event to tpm")?;
        Ok(())
    }
}
