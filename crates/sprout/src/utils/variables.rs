use crate::utils;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use anyhow::{Context, Result};
use log::warn;
use uefi::{CString16, guid};
use uefi_raw::Status;
use uefi_raw::table::runtime::{VariableAttributes, VariableVendor};

/// The classification of a variable.
/// This is an abstraction over various variable attributes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VariableClass {
    /// The variable is available in Boot Services and Runtime Services and is not persistent.
    BootAndRuntimeTemporary,
}

impl VariableClass {
    /// The [VariableAttributes] for this classification.
    fn attributes(&self) -> VariableAttributes {
        match self {
            VariableClass::BootAndRuntimeTemporary => {
                VariableAttributes::BOOTSERVICE_ACCESS | VariableAttributes::RUNTIME_ACCESS
            }
        }
    }
}

/// Provides access to a particular set of vendor variables.
pub struct VariableController {
    /// The GUID of the vendor.
    vendor: VariableVendor,
}

impl VariableController {
    /// Global variables.
    pub const GLOBAL: VariableController = VariableController::new(VariableVendor(guid!(
        "8be4df61-93ca-11d2-aa0d-00e098032b8c"
    )));

    /// Create a new [VariableController] for the `vendor`.
    pub const fn new(vendor: VariableVendor) -> Self {
        Self { vendor }
    }

    /// Convert `key` to a variable name as a CString16.
    fn name(key: &str) -> Result<CString16> {
        CString16::try_from(key).context("unable to convert variable name to CString16")
    }

    /// Retrieve the cstr16 value specified by the `key`.
    /// Returns None if the value isn't set.
    /// If the value is not decodable, we will return None and log a warning.
    pub fn get_cstr16(&self, key: &str) -> Result<Option<String>> {
        let name = Self::name(key)?;

        // Retrieve the variable data, handling variable not existing as None.
        match uefi::runtime::get_variable_boxed(&name, &self.vendor) {
            Ok((data, _)) => {
                // Try to decode UTF-16 bytes to a CString16.
                match utils::utf16_bytes_to_cstring16(&data) {
                    Ok(value) => {
                        // We have a value, so return the UTF-8 value.
                        Ok(Some(value.to_string()))
                    }

                    Err(error) => {
                        // We encountered an error, so warn and return None.
                        warn!("efi variable '{}' is not valid UTF-16: {}", key, error);
                        Ok(None)
                    }
                }
            }

            Err(error) => {
                // If the variable does not exist, we will return None.
                if error.status() == Status::NOT_FOUND {
                    Ok(None)
                } else {
                    Err(error).with_context(|| format!("unable to get efi variable {}", key))
                }
            }
        }
    }

    /// Retrieve a boolean value specified by the `key`.
    pub fn get_bool(&self, key: &str) -> Result<bool> {
        let name = Self::name(key)?;

        // Retrieve the variable data, handling variable not existing as false.
        match uefi::runtime::get_variable_boxed(&name, &self.vendor) {
            Ok((data, _)) => {
                // If the variable is zero-length, we treat it as false.
                if data.is_empty() {
                    Ok(false)
                } else {
                    // We treat the variable as true if the first byte is non-zero.
                    Ok(data[0] > 0)
                }
            }

            Err(error) => {
                // If the variable does not exist, we treat it as false.
                if error.status() == Status::NOT_FOUND {
                    Ok(false)
                } else {
                    Err(error).with_context(|| format!("unable to get efi variable {}", key))
                }
            }
        }
    }

    /// Set a variable specified by `key` to `value`.
    /// The variable `class` controls the attributes for the variable.
    pub fn set(&self, key: &str, value: &[u8], class: VariableClass) -> Result<()> {
        let name = Self::name(key)?;
        uefi::runtime::set_variable(&name, &self.vendor, class.attributes(), value)
            .with_context(|| format!("unable to set efi variable {}", key))?;
        Ok(())
    }

    /// Set a variable specified by `key` to `value`, converting the value to
    /// a [CString16]. The variable `class` controls the attributes for the variable.
    pub fn set_cstr16(&self, key: &str, value: &str, class: VariableClass) -> Result<()> {
        // Encode the value as a CString16 little endian.
        let mut encoded = value
            .encode_utf16()
            .flat_map(|c| c.to_le_bytes())
            .collect::<Vec<u8>>();
        // Add a null terminator to the end of the value.
        encoded.extend_from_slice(&[0, 0]);
        self.set(key, &encoded, class)
    }

    /// Set a boolean variable specified by `key` to `value`, converting the value.
    /// The variable `class` controls the attributes for the variable.
    pub fn set_bool(&self, key: &str, value: bool, class: VariableClass) -> Result<()> {
        self.set(key, &[value as u8], class)
    }

    /// Set the u64 little-endian variable specified by `key` to `value`.
    /// The variable `class` controls the attributes for the variable.
    pub fn set_u64le(&self, key: &str, value: u64, class: VariableClass) -> Result<()> {
        self.set(key, &value.to_le_bytes(), class)
    }

    /// Remove the variable specified by `key`.
    /// This can fail if the variable is not set.
    pub fn remove(&self, key: &str) -> Result<()> {
        let name = Self::name(key)?;

        // Delete the variable from UEFI.
        uefi::runtime::delete_variable(&name, &self.vendor)
            .with_context(|| format!("unable to remove efi variable {}", key))
    }
}
