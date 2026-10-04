use crate::handle::find_handle;
use anyhow::{Context, Result, anyhow, bail};
use core::ffi::c_void;
use core::ptr::{self, NonNull};
use log::{info, warn};
use uefi::boot::{AllocateType, MemoryType, PAGE_SIZE};
use uefi::proto::unsafe_protocol;
use uefi_raw::{Guid, Status, guid};

/// The configuration table that holds the flattened devicetree.
const DTB_TABLE_GUID: Guid = guid!("b1b621d5-f19c-41a5-830b-d9152c69aae0");

/// The protocol that firmware such as U-Boot provides to patch a devicetree for the machine.
const DT_FIXUP_GUID: Guid = guid!("e617d64c-fe08-46da-f4dc-bbd5870c7300");

/// The magic number at the start of a flattened devicetree.
const FDT_MAGIC: u32 = 0xd00d_feed;

/// The size of the fixed header of a flattened devicetree.
const FDT_HEADER_SIZE: usize = 7 * 4;

/// The largest devicetree that is accepted. The Linux stub copies it into a smaller buffer.
const MAX_DTB_SIZE: usize = 32 * 1024 * 1024;

/// Apply the fixups for the machine to the devicetree.
const DT_APPLY_FIXUPS: u32 = 0x1;

/// Reserve the memory that the devicetree says to reserve.
const DT_RESERVE_MEMORY: u32 = 0x2;

/// The protocol that patches a devicetree for the machine.
#[unsafe_protocol(DT_FIXUP_GUID)]
#[repr(C)]
struct DtFixupProtocol {
    revision: u64,
    fixup: unsafe extern "efiapi" fn(
        this: *mut DtFixupProtocol,
        fdt: *mut c_void,
        buffer_size: *mut usize,
        flags: u32,
    ) -> Status,
}

/// A devicetree that is installed for the image that is about to start. When this is dropped,
/// which is when the image returned or failed to start, the devicetree of the firmware is
/// put back and the memory is freed.
pub struct DeviceTree {
    /// The pages that hold the devicetree.
    pages: NonNull<u8>,
    /// The number of pages.
    count: usize,
    /// The devicetree table of the firmware, which is null if there was none.
    original: *const c_void,
    /// Whether the table points to the pages.
    installed: bool,
}

impl DeviceTree {
    /// Check that `dtb` looks like a flattened devicetree that can be installed.
    fn validate(dtb: &[u8]) -> Result<()> {
        if dtb.len() < FDT_HEADER_SIZE || dtb.len() > MAX_DTB_SIZE {
            bail!("the devicetree is {} bytes, which is not valid", dtb.len());
        }
        // The header is big endian: the magic number, then the total size.
        let magic = u32::from_be_bytes([dtb[0], dtb[1], dtb[2], dtb[3]]);
        let total = u32::from_be_bytes([dtb[4], dtb[5], dtb[6], dtb[7]]) as usize;
        if magic != FDT_MAGIC {
            bail!("the file is not a flattened devicetree");
        }
        if total > dtb.len() {
            bail!("the devicetree is truncated");
        }
        Ok(())
    }

    /// Allocate `count` pages of memory that belongs to the firmware tables, as a devicetree
    /// has to be in memory of the type that ACPI tables are in.
    fn allocate(count: usize) -> Result<NonNull<u8>> {
        uefi::boot::allocate_pages(AllocateType::AnyPages, MemoryType::ACPI_RECLAIM, count)
            .context("unable to allocate memory for the devicetree")
    }

    /// Install `dtb` as the devicetree of the machine until the returned value is dropped.
    pub fn install(dtb: &[u8]) -> Result<Self> {
        Self::validate(dtb)?;

        // Remember the table of the firmware so it can be put back.
        let original = uefi::system::with_config_table(|tables| {
            tables
                .iter()
                .find(|entry| entry.guid == DTB_TABLE_GUID)
                .map(|entry| entry.address)
                .unwrap_or(ptr::null())
        });

        let count = dtb.len().div_ceil(PAGE_SIZE);
        let pages = Self::allocate(count)?;
        // SAFETY: The pages are at least as large as the devicetree, and don't overlap it.
        unsafe { ptr::copy_nonoverlapping(dtb.as_ptr(), pages.as_ptr(), dtb.len()) };
        let mut tree = Self {
            pages,
            count,
            original,
            installed: false,
        };

        tree.fixup(dtb)?;

        // SAFETY: The table points to pages that stay allocated until this is dropped, which
        // puts the original table back before the pages are freed.
        unsafe {
            uefi::boot::install_configuration_table(&DTB_TABLE_GUID, tree.pages.as_ptr().cast())
        }
        .context("unable to install the devicetree")?;
        tree.installed = true;

        info!(
            "installed the devicetree at {:p} ({} bytes)",
            tree.pages.as_ptr(),
            dtb.len()
        );
        Ok(tree)
    }

    /// Let the firmware patch the devicetree for the machine, if it can.
    fn fixup(&mut self, dtb: &[u8]) -> Result<()> {
        let Some(handle) = find_handle(&DT_FIXUP_GUID)? else {
            return Ok(());
        };
        let mut protocol = uefi::boot::open_protocol_exclusive::<DtFixupProtocol>(handle)
            .context("unable to open the devicetree fixup protocol")?;
        let fixup = protocol.fixup;
        let this: *mut DtFixupProtocol = &mut *protocol;
        let flags = DT_APPLY_FIXUPS | DT_RESERVE_MEMORY;

        // The firmware gets all the memory that was allocated. If that is not enough, it says
        // how much it needs, and it is given a copy of the original in a larger allocation.
        let mut size = self.count * PAGE_SIZE;
        // SAFETY: The pages are valid for the size that is passed.
        let mut status = unsafe { fixup(this, self.pages.as_ptr().cast(), &mut size, flags) };
        if status == Status::BUFFER_TOO_SMALL {
            let count = size.div_ceil(PAGE_SIZE);
            let pages = Self::allocate(count)?;
            // SAFETY: The new pages are larger than the devicetree, and don't overlap it.
            unsafe { ptr::copy_nonoverlapping(dtb.as_ptr(), pages.as_ptr(), dtb.len()) };
            // SAFETY: The old pages came from the same allocator and nothing refers to them yet.
            if let Err(error) = unsafe { uefi::boot::free_pages(self.pages, self.count) } {
                warn!("unable to free the devicetree memory: {}", error);
            }
            self.pages = pages;
            self.count = count;
            size = count * PAGE_SIZE;
            // SAFETY: The new pages are valid for the size that is passed.
            status = unsafe { fixup(this, self.pages.as_ptr().cast(), &mut size, flags) };
        }
        if status != Status::SUCCESS {
            return Err(anyhow!("the devicetree fixup failed: {:?}", status));
        }
        Ok(())
    }
}

impl Drop for DeviceTree {
    fn drop(&mut self) {
        if self.installed {
            // SAFETY: This puts back the table that the firmware had, which is null if it had
            // none, and which removes the table.
            let result =
                unsafe { uefi::boot::install_configuration_table(&DTB_TABLE_GUID, self.original) };
            if let Err(error) = result {
                // The table can still point to the pages, so they must not be freed.
                if !(error.status() == Status::NOT_FOUND && self.original.is_null()) {
                    warn!("unable to restore the devicetree: {}", error);
                    return;
                }
            }
        }
        // SAFETY: Nothing refers to the pages any more.
        if let Err(error) = unsafe { uefi::boot::free_pages(self.pages, self.count) } {
            warn!("unable to free the devicetree memory: {}", error);
        }
    }
}
