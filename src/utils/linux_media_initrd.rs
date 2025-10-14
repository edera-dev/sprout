use anyhow::{Context, Result, bail};
use log::info;
use std::ffi::c_void;
use uefi::proto::device_path::DevicePath;
use uefi::proto::device_path::build::DevicePathBuilder;
use uefi::proto::device_path::build::media::Vendor;
use uefi::proto::media::load_file::LoadFile2;
use uefi::proto::unsafe_protocol;
use uefi::{Guid, Handle, guid};
use uefi_raw::protocol::device_path::DevicePathProtocol;
use uefi_raw::protocol::media::LoadFile2Protocol;
use uefi_raw::{Boolean, Status};

#[derive(Debug)]
#[repr(C)]
pub struct LinuxMediaInitrdProtocol {
    pub load_file: unsafe extern "efiapi" fn(
        this: *mut LinuxMediaInitrdProtocol,
        file_path: *const DevicePathProtocol,
        boot_policy: Boolean,
        buffer_size: *mut usize,
        buffer: *mut c_void,
    ) -> Status,
    pub address: *mut c_void,
    pub length: usize,
}

impl LinuxMediaInitrdProtocol {
    pub const GUID: Guid = guid!("5568e427-68fc-4f3d-ac74-ca555231cc68");

    pub fn device_path() -> Box<DevicePath> {
        let mut path = Vec::new();
        let path = DevicePathBuilder::with_vec(&mut path)
            .push(&Vendor {
                vendor_guid: LinuxMediaInitrdProtocol::GUID,
                vendor_defined_data: &[],
            })
            .unwrap()
            .finalize()
            .unwrap();
        path.to_boxed()
    }
}

#[repr(transparent)]
#[unsafe_protocol(LinuxMediaInitrdProtocol::GUID)]
pub struct LinuxMediaInitrd(LinuxMediaInitrdProtocol);

pub struct LinuxMediaInitrdHandle {
    pub handle: Handle,
    pub protocol: *mut LinuxMediaInitrdProtocol,
    pub path: *mut DevicePath,
}

unsafe extern "efiapi" fn load_initrd_file(
    this: *mut LinuxMediaInitrdProtocol,
    file_path: *const DevicePathProtocol,
    boot_policy: Boolean,
    buffer_size: *mut usize,
    buffer: *mut c_void,
) -> Status {
    if this.is_null() || buffer_size.is_null() || file_path.is_null() {
        return Status::INVALID_PARAMETER;
    }

    if boot_policy == Boolean::TRUE {
        return Status::UNSUPPORTED;
    }

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

fn already_registered() -> Result<bool> {
    let path = LinuxMediaInitrdProtocol::device_path();

    let mut existing_path = path.as_ref();
    let result = uefi::boot::locate_device_path::<LoadFile2>(&mut existing_path);

    if result.is_ok() {
        return Ok(true);
    } else if let Err(error) = result
        && error.status() != Status::NOT_FOUND
    {
        bail!("unable to locate initrd device path: {}", error);
    }
    Ok(false)
}

/// Registers the provided [data] with the UEFI stack as a Linux initrd.
/// This uses a special device path that Linux EFI stub will look at
/// to load the initrd from.
pub fn register_linux_initrd(data: Box<[u8]>) -> Result<LinuxMediaInitrdHandle> {
    let path = LinuxMediaInitrdProtocol::device_path();
    let path = Box::leak(path);

    if already_registered()? {
        bail!("linux initrd already registered");
    }

    let mut handle = unsafe {
        uefi::boot::install_protocol_interface(
            None,
            &DevicePathProtocol::GUID,
            path.as_ffi_ptr() as *mut c_void,
        )
    }
    .context("unable to install linux initrd device path handle")?;

    let data = Box::leak(data);

    let protocol = Box::new(LinuxMediaInitrdProtocol {
        load_file: load_initrd_file,
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
    .context("unable to install linux initrd load file handle")?;

    if !already_registered()? {
        bail!("linux initrd not registered when expected to be registered");
    }

    info!("linux initrd registered");

    Ok(LinuxMediaInitrdHandle {
        handle,
        protocol,
        path,
    })
}

/// Unregisters a Linux initrd from the UEFI stack.
/// This will free the memory allocated by the initrd.
pub fn unregister_linux_initrd(handle: LinuxMediaInitrdHandle) -> Result<()> {
    if !already_registered()? {
        return Ok(());
    }

    unsafe {
        uefi::boot::uninstall_protocol_interface(
            handle.handle,
            &DevicePathProtocol::GUID,
            handle.path as *mut c_void,
        )
        .context("unable to uninstall linux initrd device path handle")?;

        uefi::boot::uninstall_protocol_interface(
            handle.handle,
            &LoadFile2Protocol::GUID,
            handle.protocol as *mut _ as *mut c_void,
        )
        .context("unable to uninstall linux initrd load file handle")?;

        let _path = Box::from_raw(handle.path);
        let _protocol = Box::from_raw(handle.protocol);
    }

    Ok(())
}
