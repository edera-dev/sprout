use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use anyhow::{Result, bail};

/// The largest section that will be read. UKI metadata sections are small, and a limit
/// keeps a corrupt image from causing a large allocation.
pub const MAX_SECTION_SIZE: u64 = 64 * 1024;

/// The most sections a PE image can have.
const MAX_SECTIONS: usize = 96;

/// The size of a section table entry.
const SECTION_ENTRY_SIZE: usize = 40;

/// A source of bytes that can be read at an offset, such as a file.
pub trait ReadAt {
    /// Fills `buf` with the bytes at `offset`, failing if they can't all be read.
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<()>;
}

/// Reads from a slice of bytes, which is useful for images that are already in memory.
impl ReadAt for &[u8] {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<()> {
        let start = usize::try_from(offset)?;
        let end = start.checked_add(buf.len());
        match end.and_then(|end| self.get(start..end)) {
            Some(bytes) => {
                buf.copy_from_slice(bytes);
                Ok(())
            }
            None => bail!("read past the end of the image"),
        }
    }
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// Reads the sections named in `wanted` from the PE image in `reader`.
/// Only the headers and the wanted sections are read, so this is cheap on large images.
/// A wanted section that is missing, or larger than [MAX_SECTION_SIZE], is left out.
/// An image that isn't a valid PE file is an error.
pub fn read_sections<R: ReadAt>(
    reader: &mut R,
    wanted: &[&str],
) -> Result<BTreeMap<String, Vec<u8>>> {
    // The DOS header starts with "MZ" and holds the offset of the PE header at 0x3c.
    let mut dos = [0u8; 0x40];
    reader.read_at(0, &mut dos)?;
    if &dos[..2] != b"MZ" {
        bail!("not a PE image: missing the MZ signature");
    }
    let pe_offset = u64::from(u32_at(&dos, 0x3c));

    // The PE header is the "PE\0\0" signature followed by the COFF header.
    let mut coff = [0u8; 24];
    reader.read_at(pe_offset, &mut coff)?;
    if &coff[..4] != b"PE\0\0" {
        bail!("not a PE image: missing the PE signature");
    }
    let section_count = usize::from(u16_at(&coff, 6));
    let optional_header_size = u64::from(u16_at(&coff, 20));
    if section_count > MAX_SECTIONS {
        bail!("PE image has too many sections: {}", section_count);
    }

    // The section table follows the optional header.
    let mut table = vec![0u8; section_count * SECTION_ENTRY_SIZE];
    reader.read_at(pe_offset + 24 + optional_header_size, &mut table)?;

    let mut sections = BTreeMap::new();
    for entry in table.as_chunks::<SECTION_ENTRY_SIZE>().0 {
        // The name is up to eight bytes, padded with NUL.
        let name_len = entry[..8].iter().position(|b| *b == 0).unwrap_or(8);
        let Ok(name) = core::str::from_utf8(&entry[..name_len]) else {
            continue;
        };
        if !wanted.contains(&name) || sections.contains_key(name) {
            continue;
        }

        // The real size is the virtual size, as the raw size is padded to the file alignment.
        let virtual_size = u64::from(u32_at(entry, 8));
        let raw_size = u64::from(u32_at(entry, 16));
        let raw_offset = u64::from(u32_at(entry, 20));
        let size = if virtual_size == 0 {
            raw_size
        } else {
            virtual_size.min(raw_size)
        };
        if size > MAX_SECTION_SIZE {
            continue;
        }

        let mut data = vec![0u8; size as usize];
        reader.read_at(raw_offset, &mut data)?;
        sections.insert(name.to_string(), data);
    }

    Ok(sections)
}
