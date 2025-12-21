use alloc::boxed::Box;
use alloc::vec::Vec;
use anyhow::{Context, Result, bail};
use core::ffi::c_void;
use core::ptr;
use log::error;
use uefi::proto::device_path::DevicePath;
use uefi::proto::device_path::build::DevicePathBuilder;
use uefi::proto::device_path::build::media::Vendor;
use uefi::proto::media::load_file::LoadFile2;
use uefi::{Guid, Handle};
use uefi_raw::protocol::device_path::DevicePathProtocol;
use uefi_raw::protocol::media::LoadFile2Protocol;
use uefi_raw::{Boolean, Status};

pub mod constants;

/// The media loader protocol.
#[derive(Debug)]
#[repr(C)]
struct MediaLoaderProtocol {
    /// This is the standard EFI LoadFile2 protocol.
    pub load_file: unsafe extern "efiapi" fn(
        this: *mut MediaLoaderProtocol,
        file_path: *const DevicePathProtocol,
        boot_policy: Boolean,
        buffer_size: *mut usize,
        buffer: *mut c_void,
    ) -> Status,
    /// A pointer to a Box<[u8]> containing the data to load.
    pub address: *mut c_void,
    /// The length of the data to load.
    pub length: usize,
}

/// Represents a media loader which has been registered in the UEFI stack.
/// You MUST call [MediaLoaderHandle::unregister] when ready to unregister.
/// [Drop] is not implemented for this type.
pub struct MediaLoaderHandle {
    /// The handle of the media loader in the UEFI stack.
    handle: Handle,
    /// The protocol interface pointer.
    protocol: *mut MediaLoaderProtocol,
    /// The device path pointer.
    path: *mut DevicePath,
}

impl MediaLoaderHandle {
    /// The behavior of this function is derived from how Linux calls it.
    ///
    /// Linux calls this function by first passing a NULL `buffer`.
    /// We must set the size of the buffer it should allocate in `buffer_size`.
    /// The next call will pass a buffer of the right size, and we should copy
    /// data into that buffer, checking whether it is safe to copy based on
    /// the buffer size.
    ///
    /// SAFETY: `this.address` and `this.length` are set by leaking a Box<[u8]>, so we can
    /// be sure their pointers are valid when this is called. The caller must call this function
    /// while inside UEFI boot services to ensure pointers are valid. Copying to `buffer` is
    /// assumed valid because the caller must ensure `buffer` is valid by function contract.
    unsafe extern "efiapi" fn load_file(
        this: *mut MediaLoaderProtocol,
        file_path: *const DevicePathProtocol,
        boot_policy: Boolean,
        buffer_size: *mut usize,
        buffer: *mut c_void,
    ) -> Status {
        // Check if the pointers are non-null first.
        if this.is_null() || buffer_size.is_null() || file_path.is_null() {
            return Status::INVALID_PARAMETER;
        }

        // Boot policy must not be true, and if it is, that is special behavior that is irrelevant
        // for the media loader concept.
        if boot_policy == Boolean::TRUE {
            return Status::UNSUPPORTED;
        }

        // SAFETY: Validated as safe because this is checked to be non-null. It is the caller's
        // responsibility to ensure that the right pointer is passed for [this].
        unsafe {
            // Check if the length and address are valid.
            if (*this).length == 0 || (*this).address.is_null() {
                return Status::NOT_FOUND;
            }

            // Check if the buffer is large enough.
            // If it is not, we need to set the buffer size to the length of the data.
            // This is the way that Linux calls this function, to check the size to allocate
            // for the buffer that holds the data.
            if buffer.is_null() || *buffer_size < (*this).length {
                *buffer_size = (*this).length;
                return Status::BUFFER_TOO_SMALL;
            }

            // Copy the data into the buffer.
            buffer.copy_from((*this).address, (*this).length);
            // Set the buffer size to the length of the data.
            *buffer_size = (*this).length;
        }

        // We've successfully loaded the data.
        Status::SUCCESS
    }

    /// Creates a new device path for the media loader based on a vendor `guid`.
    fn device_path(guid: Guid) -> Result<Box<DevicePath>> {
        // The buffer for the device path.
        let mut path = Vec::new();
        // Build a device path for the media loader with a vendor-specific guid.
        let path = DevicePathBuilder::with_vec(&mut path)
            .push(&Vendor {
                vendor_guid: guid,
                vendor_defined_data: &[],
            })
            .context("unable to produce device path")?
            .finalize()
            .context("unable to produce device path")?;
        // Convert the device path to a boxed device path.
        // This is safer than dealing with a pooled device path.
        Ok(path.to_boxed())
    }

    /// Checks if the media loader is already registered with the UEFI stack.
    fn already_registered(guid: Guid) -> Result<bool> {
        // Acquire the device path for the media loader.
        let path = Self::device_path(guid)?;

        let mut existing_path = path.as_ref();

        // Locate the LoadFile2 protocol for the media loader based on the device path.
        let result = uefi::boot::locate_device_path::<LoadFile2>(&mut existing_path);

        // If the result is okay, the media loader is already registered.
        if result.is_ok() {
            return Ok(true);
        } else if let Err(error) = result
            && error.status() != Status::NOT_FOUND
        // If the error is not found, that means it's not registered.
        {
            bail!("unable to locate media loader device path: {}", error);
        }
        // The media loader is not registered.
        Ok(false)
    }

    /// Registers the provided `data` with the UEFI stack as media loader.
    /// This uses a special device path that other EFI programs will look at
    /// to load the data from.
    pub fn register(guid: Guid, data: Box<[u8]>) -> Result<MediaLoaderHandle> {
        // Acquire the vendor device path for the media loader.
        let path = Self::device_path(guid)?;

        // Check if the media loader is already registered.
        // If it is, we can't register it again safely.
        if Self::already_registered(guid)? {
            bail!("media loader already registered");
        }

        // Leak the device path to pass it to the UEFI stack.
        let path = Box::leak(path);

        // Install a protocol interface for the device path.
        // This ensures it can be located by other EFI programs.
        let primary_handle = match unsafe {
            uefi::boot::install_protocol_interface(
                None,
                &DevicePathProtocol::GUID,
                path.as_ffi_ptr() as *mut c_void,
            )
        }
        .context("unable to install media loader device path handle")
        {
            // Acquiring the primary handle succeeded, so we can return the handle.
            Ok(handle) => handle,
            // If acquiring the primary handle failed, we free the device path and return the error.
            Err(error) => {
                // SAFETY: We know that the device path is leaked,
                // so we can safely take a reference to it again.
                // The UEFI stack failed to install the protocol interface
                // if we reach here, so the path is no longer in use.
                let path = unsafe { Box::from_raw(path) };
                // Explicitly drop the path to clarify the lifetime.
                drop(path);

                // Return the original error.
                return Err(error);
            }
        };

        // Leak the data we need to pass to the UEFI stack.
        let data = Box::leak(data);

        // Allocate a new box for the protocol interface.
        let protocol = Box::new(MediaLoaderProtocol {
            load_file: Self::load_file,
            address: data.as_ptr() as *mut _,
            length: data.len(),
        });

        // Leak the protocol interface to pass it to the UEFI stack.
        let protocol = Box::leak(protocol);

        // Install a protocol interface for the load file protocol for the media loader protocol.
        let secondary_handle = unsafe {
            uefi::boot::install_protocol_interface(
                Some(primary_handle),
                &LoadFile2Protocol::GUID,
                // The UEFI API expects an opaque pointer here.
                protocol as *mut MediaLoaderProtocol as *mut c_void,
            )
        };

        // If installing the second protocol interface failed, we need to clean up after ourselves.
        if secondary_handle.is_err() {
            // Uninstall the protocol interface for the device path protocol.
            // SAFETY: If we have reached this point, we know that the protocol is registered.
            // If this fails, we have no choice but to leak memory. The error will be shown
            // to the user, so at least they can see it. In most cases, catching this error
            // will exit, so leaking is safe.
            unsafe {
                uefi::boot::uninstall_protocol_interface(
                    primary_handle,
                    &DevicePathProtocol::GUID,
                    path.as_ffi_ptr() as *mut c_void,
                )
                .context(
                    "unable to uninstall media loader device path handle, this will leak memory",
                )?;
            }

            // SAFETY: We know that the protocol is leaked, so we can safely take a reference to it.
            let protocol = unsafe { Box::from_raw(protocol) };
            // SAFETY: We know that the data is leaked, so we can safely take a reference to it.
            let data = unsafe { Box::from_raw(data) };
            // SAFETY: We know that the path is leaked, so we can safely take a reference to it.
            let path = unsafe { Box::from_raw(path) };

            // Drop all the allocations explicitly to clarify the lifetime.
            drop(protocol);
            drop(data);
            drop(path);
        }

        // If installing the second protocol interface failed, this will return the error.
        // We should have already cleaned up after ourselves, so this is safe.
        secondary_handle.context("unable to install media loader load file handle")?;

        // Return a handle to the media loader.
        Ok(Self {
            handle: primary_handle,
            protocol,
            path,
        })
    }

    /// Unregisters a media loader from the UEFI stack.
    /// This will free the memory allocated by the passed data.
    fn unregister(&self) -> Result<()> {
        // SAFETY: We know that the media loader is registered if the handle is valid,
        // so we can safely uninstall it.
        // We should have allocated the pointers involved, so we can safely free them.
        unsafe {
            // Uninstall the protocol interface for the device path protocol.
            uefi::boot::uninstall_protocol_interface(
                self.handle,
                &DevicePathProtocol::GUID,
                self.path as *mut c_void,
            )
            .context("unable to uninstall media loader device path handle")?;

            // Uninstall the protocol interface for the load file protocol.
            uefi::boot::uninstall_protocol_interface(
                self.handle,
                &LoadFile2Protocol::GUID,
                self.protocol as *mut _ as *mut c_void,
            )
            .context("unable to uninstall media loader load file handle")?;

            // Retrieve a box for the device path and protocols.
            let path = Box::from_raw(self.path);
            let protocol = Box::from_raw(self.protocol);

            // Retrieve a box for the data we passed in.
            let slice = ptr::slice_from_raw_parts_mut(protocol.address as *mut u8, protocol.length);
            let data = Box::from_raw(slice);

            // Drop all the allocations explicitly, as we don't want to leak them.
            drop(path);
            drop(protocol);
            drop(data);
        }

        Ok(())
    }
}

/// Implement drop for the handle to automatically unregister the media loader.
impl Drop for MediaLoaderHandle {
    fn drop(&mut self) {
        // If unregister fails, print an error to the log.
        // This may leak stuff, but the only other option is to panic.
        if let Err(error) = self.unregister() {
            error!("unable to unregister media loader: {}", error);
        }
    }
}
