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
    let data = bytes
        // Chunk everything into two bytes.
        .chunks_exact(2)
        // Reinterpret the bytes as u16 little-endian.
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        // Collect the result into a vector.
        .collect::<Vec<_>>();

    CString16::try_from(data).context("unable to convert utf16 bytes to CString16")
}
