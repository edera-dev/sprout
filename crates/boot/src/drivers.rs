use crate::context::SproutContext;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use anyhow::{Context, Result};
use edera_sprout_config::drivers::{DriverDeclaration, DriverProvides};
use eficore::loader::source::ImageSource;
use eficore::loader::{ImageLoadRequest, ImageLoader};
use log::info;
use uefi::boot::{OpenProtocolAttributes, OpenProtocolParams, SearchType};
use uefi::proto::media::block::BlockIO;
use uefi::proto::media::disk::{DiskIo, DiskIo2};
use uefi::{Handle, Identify};

/// Loads the driver specified by the `driver` declaration.
fn load_driver(context: Rc<SproutContext>, driver: &DriverDeclaration) -> Result<Handle> {
    // Acquire the handle and device path of the loaded image.
    let sprout_image = uefi::boot::image_handle();

    // Resolve the path to the driver image.
    let resolved = eficore::path::resolve_path(
        Some(context.root().loaded_image_path()?),
        context.stamp(&driver.path),
    )
    .context("unable to resolve path to driver")?;

    // Create an image load request with the current image and the resolved path.
    let request = ImageLoadRequest::new(sprout_image, ImageSource::ResolvedPath(&resolved));

    // Load the driver image using the image loader support module.
    // It will determine if the image needs to be loaded via the shim or can be loaded directly.
    let image = ImageLoader::load(request)?;

    // Start the driver image, this is expected to return control to sprout.
    // There is no guarantee that the driver will actually return control as it is
    // just a standard EFI image.
    info!("starting driver image");
    uefi::boot::start_image(*image.handle()).context("unable to start driver image")?;
    info!("driver image returned");

    Ok(*image.handle())
}

/// Checks whether the handle is a logical partition rather than a whole disk.
fn is_partition(handle: Handle) -> bool {
    let params = OpenProtocolParams {
        handle,
        agent: uefi::boot::image_handle(),
        controller: None,
    };
    // Opening with GetProtocol does not disturb the existing protocol users.
    let block = unsafe {
        uefi::boot::open_protocol::<BlockIO>(params, OpenProtocolAttributes::GetProtocol)
    };
    block.is_ok_and(|block| block.media().is_logical_partition())
}

/// Locates the handles that are disks or partitions.
/// Filesystem drivers bind to partitions, so whole disks are only returned when
/// there are no partitions. Reconnecting a whole disk makes the firmware rebuild
/// its partition handles, which hangs on some NVMe firmware.
fn locate_storage_handles() -> Result<Vec<Handle>> {
    let mut storage = Vec::new();
    for guid in [&BlockIO::GUID, &DiskIo::GUID, &DiskIo2::GUID] {
        // Not finding any handles for a protocol is not an error.
        let Ok(handles) = uefi::boot::locate_handle_buffer(SearchType::ByProtocol(guid)) else {
            continue;
        };
        for handle in handles.iter() {
            if !storage.contains(handle) {
                storage.push(*handle);
            }
        }
    }
    let partitions: Vec<Handle> = storage
        .iter()
        .copied()
        .filter(|handle| is_partition(*handle))
        .collect();
    if partitions.is_empty() {
        return Ok(storage);
    }
    Ok(partitions)
}

/// Describes a handle by its device path, for diagnosing hangs.
fn describe(handle: Handle) -> String {
    use uefi::proto::device_path::DevicePath;

    let params = OpenProtocolParams {
        handle,
        agent: uefi::boot::image_handle(),
        controller: None,
    };
    match unsafe {
        uefi::boot::open_protocol::<DevicePath>(params, OpenProtocolAttributes::GetProtocol)
    } {
        Ok(path) => path.to_string(),
        Err(_) => String::from("<no device path>"),
    }
}

/// Reconnects handles to their controllers.
/// This is effectively a UEFI stack reload in a sense.
/// After we load all the drivers, we need to reconnect their handles
/// so that filesystems are recognized again.
/// When `provides` is [`DriverProvides::Filesystems`], only disk and partition handles are reconnected.
fn reconnect(provides: DriverProvides, drivers: &[Handle]) -> Result<()> {
    // Locate the handles to reconnect in the UEFI stack.
    let handles = match provides {
        DriverProvides::Any => uefi::boot::locate_handle_buffer(SearchType::AllHandles)
            .context("unable to locate handles buffer")?
            .iter()
            .copied()
            .collect(),
        DriverProvides::Filesystems => {
            // Probe in reverse so the first partition is the last to be tried.
            let mut storage = locate_storage_handles()?;
            storage.reverse();
            storage
        }
    };

    // The driver list passed to the firmware must end with a null entry.
    let driver_list: Vec<Option<Handle>> = drivers
        .iter()
        .copied()
        .map(Some)
        .chain(core::iter::once(None))
        .collect();

    let total = handles.len();
    for (index, handle) in handles.into_iter().enumerate() {
        info!("reconnecting {}/{}: {}", index + 1, total, describe(handle));
        // Ignore the result as there is nothing we can do if reconnecting a controller fails.
        // This is also likely to fail in some cases but should fail safely.
        // Filesystem drivers are connected directly so the firmware stack is not rebuilt.
        let _ = match provides {
            DriverProvides::Any => uefi::boot::connect_controller(handle, &[], None, true),
            DriverProvides::Filesystems => {
                uefi::boot::connect_controller(handle, &driver_list, None, false)
            }
        };
        info!("reconnected {}/{}", index + 1, total);
    }

    Ok(())
}

/// Load all the drivers specified in `drivers`.
/// There is no driver order currently. This will reconnect the controllers
/// if at least one driver was loaded. Every controller is reconnected unless all
/// the drivers only provide filesystems, in which case only storage is reconnected.
pub fn load(
    context: Rc<SproutContext>,
    drivers: &BTreeMap<String, DriverDeclaration>,
) -> Result<()> {
    // If there are no drivers, we don't need to do anything.
    if drivers.is_empty() {
        return Ok(());
    }

    info!("loading drivers");

    // Load all the drivers in no particular order.
    let mut loaded = Vec::new();
    for (name, driver) in drivers {
        loaded.push(
            load_driver(context.clone(), driver)
                .context(format!("unable to load driver: {}", name))?,
        );
    }

    // Reconnect everything if any driver can provide anything, otherwise only storage.
    let provides = if drivers
        .values()
        .all(|driver| driver.provides == DriverProvides::Filesystems)
    {
        DriverProvides::Filesystems
    } else {
        DriverProvides::Any
    };
    reconnect(provides, &loaded).context("unable to reconnect drivers")?;
    info!("loaded drivers");

    // We've now loaded all the drivers, so we can return.
    Ok(())
}
