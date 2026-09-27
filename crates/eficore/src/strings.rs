use alloc::vec::Vec;
use anyhow::{Context, Result, bail};
use uefi::CString16;

/// Convert a byte slice into a CString16.
pub fn utf16_bytes_to_cstring16(bytes: &[u8]) -> Result<CString16> {
    // Validate the input bytes are the right length.
    if !bytes.len().is_multiple_of(2) {
        bail!("utf16 bytes must be a multiple of 2");
    }

    // Convert the bytes to UTF-16 data.
    let mut data = bytes
        // Chunk everything into two bytes.
        .as_chunks::<2>()
        .0
        .iter()
        // Reinterpret the bytes as u16 little-endian.
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        // Stop at the first null terminator, if any.
        // This ignores any trailing data, such as extra null padding.
        .take_while(|c| *c != 0)
        // Collect the result into a vector.
        .collect::<Vec<_>>();

    // Add the null terminator, as the input is not guaranteed to be null-terminated.
    data.push(0);

    CString16::try_from(data).context("unable to convert utf16 bytes to CString16")
}
