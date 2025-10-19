use anyhow::{Context, Result, bail};
use std::ffi::c_void;
use uefi::proto::device_path::DevicePath;
use uefi::proto::device_path::build::DevicePathBuilder;
use uefi::proto::device_path::build::media::Vendor;
use uefi::proto::media::load_file::LoadFile2;
use uefi::{Guid, Handle};
use uefi_raw::protocol::device_path::DevicePathProtocol;
use uefi_raw::protocol::media::LoadFile2Protocol;
use uefi_raw::{Boolean, Status};

pub mod constants;

#[derive(Debug)]
#[repr(C)]
struct MediaLoaderProtocol {
    pub load_file: unsafe extern "efiapi" fn(
        this: *mut MediaLoaderProtocol,
        file_path: *const DevicePathProtocol,
        boot_policy: Boolean,
        buffer_size: *mut usize,
        buffer: *mut c_void,
    ) -> Status,
    pub address: *mut c_void,
    pub length: usize,
}

/// Represents a media loader which has been registered in the UEFI stack.
/// You MUST call [MediaLoaderHandle::unregister] when ready to unregister.
/// [Drop] is not implemented for this type.
pub struct MediaLoaderHandle {
    guid: Guid,
    handle: Handle,
    protocol: *mut MediaLoaderProtocol,
    path: *mut DevicePath,
}

impl MediaLoaderHandle {
    /// The behavior of this function is derived from how Linux calls it.
    ///
    /// Linux calls this function by first passing a NULL [buffer].
    /// We must set the size of the buffer it should allocate in [buffer_size].
    /// The next call will pass a buffer of the right size, and we should copy
    /// data into that buffer, checking whether it is safe to copy based on
    /// the buffer size.
    unsafe extern "efiapi" fn load_file(
        this: *mut MediaLoaderProtocol,
        file_path: *const DevicePathProtocol,
        boot_policy: Boolean,
        buffer_size: *mut usize,
        buffer: *mut c_void,
    ) -> Status {
        if this.is_null() || buffer_size.is_null() || file_path.is_null() {
            return Status::INVALID_PARAMETER;
        }

        // Boot policy must not be true, as that's special behavior that is irrelevant
        // for the media loader concept.
        if boot_policy == Boolean::TRUE {
            return Status::UNSUPPORTED;
        }

        // SAFETY: Validated as safe because this is checked to be non-null. It is the caller's
        // responsibility to ensure that the right pointer is passed for [this].
        unsafe {
            if (*this).length == 0 || (*this).address.is_null() {
                return Status::NOT_FOUND;
            }

            if buffer.is_null() || *buffer_size < (*this).length {
                *buffer_size = (*this).length;
                return Status::BUFFER_TOO_SMALL;
            }

            buffer.copy_from((*this).address, (*this).length);
            *buffer_size = (*this).length;
        }

        Status::SUCCESS
    }

    fn device_path(guid: Guid) -> Box<DevicePath> {
        let mut path = Vec::new();
        let path = DevicePathBuilder::with_vec(&mut path)
            .push(&Vendor {
                vendor_guid: guid,
                vendor_defined_data: &[],
            })
            .unwrap()
            .finalize()
            .unwrap();
        path.to_boxed()
    }

    fn already_registered(guid: Guid) -> Result<bool> {
        let path = Self::device_path(guid);

        let mut existing_path = path.as_ref();
        let result = uefi::boot::locate_device_path::<LoadFile2>(&mut existing_path);

        if result.is_ok() {
            return Ok(true);
        } else if let Err(error) = result
            && error.status() != Status::NOT_FOUND
        {
            bail!("unable to locate media loader device path: {}", error);
        }
        Ok(false)
    }

    /// Registers the provided [data] with the UEFI stack as media loader.
    /// This uses a special device path that other EFI programs will look at
    /// to load the data from.
    pub fn register(guid: Guid, data: Box<[u8]>) -> Result<MediaLoaderHandle> {
        let path = Self::device_path(guid);
        let path = Box::leak(path);

        if Self::already_registered(guid)? {
            bail!("media loader already registered");
        }

        let mut handle = unsafe {
            uefi::boot::install_protocol_interface(
                None,
                &DevicePathProtocol::GUID,
                path.as_ffi_ptr() as *mut c_void,
            )
        }
        .context("unable to install media loader device path handle")?;

        let data = Box::leak(data);

        let protocol = Box::new(MediaLoaderProtocol {
            load_file: Self::load_file,
            address: data.as_ptr() as *mut _,
            length: data.len(),
        });

        let protocol = Box::leak(protocol);

        handle = unsafe {
            uefi::boot::install_protocol_interface(
                Some(handle),
                &LoadFile2Protocol::GUID,
                protocol as *mut _ as *mut c_void,
            )
        }
        .context("unable to install media loader load file handle")?;

        if !Self::already_registered(guid)? {
            bail!("media loader not registered when expected to be registered");
        }

        Ok(Self {
            guid,
            handle,
            protocol,
            path,
        })
    }

    /// Unregisters a media loader from the UEFI stack.
    /// This will free the memory allocated by the passed data.
    pub fn unregister(self) -> Result<()> {
        if !Self::already_registered(self.guid)? {
            return Ok(());
        }

        unsafe {
            uefi::boot::uninstall_protocol_interface(
                self.handle,
                &DevicePathProtocol::GUID,
                self.path as *mut c_void,
            )
            .context("unable to uninstall media loader device path handle")?;

            uefi::boot::uninstall_protocol_interface(
                self.handle,
                &LoadFile2Protocol::GUID,
                self.protocol as *mut _ as *mut c_void,
            )
            .context("unable to uninstall media loader load file handle")?;

            let path = Box::from_raw(self.path);
            let protocol = Box::from_raw(self.protocol);
            let slice =
                std::ptr::slice_from_raw_parts_mut(protocol.address as *mut u8, protocol.length);
            let data = Box::from_raw(slice);

            drop(path);
            drop(protocol);
            drop(data);
        }

        Ok(())
    }
}
