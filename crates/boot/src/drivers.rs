use crate::context::SproutContext;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use anyhow::{Context, Result};
use edera_sprout_config::drivers::{DriverDeclaration, DriverProvides};
use eficore::loader::source::ImageSource;
use eficore::loader::{ImageLoadRequest, ImageLoader};
use log::info;
use uefi::boot::SearchType;
use uefi::proto::media::block::BlockIO;
use uefi::proto::media::disk::{DiskIo, DiskIo2};
use uefi::{Handle, Identify};

/// Loads the driver specified by the `driver` declaration.
fn load_driver(context: Rc<SproutContext>, driver: &DriverDeclaration) -> Result<()> {
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
    uefi::boot::start_image(*image.handle()).context("unable to start driver image")?;

    Ok(())
}

/// Locates the handles that are disks or partitions.
/// These are the only controllers that filesystem drivers can bind to.
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
    Ok(storage)
}

/// Reconnects handles to their controllers.
/// This is effectively a UEFI stack reload in a sense.
/// After we load all the drivers, we need to reconnect their handles
/// so that filesystems are recognized again.
/// When `provides` is [`DriverProvides::Filesystems`], only disk and partition handles are reconnected.
fn reconnect(provides: DriverProvides) -> Result<()> {
    // Locate the handles to reconnect in the UEFI stack.
    let handles = match provides {
        DriverProvides::Any => uefi::boot::locate_handle_buffer(SearchType::AllHandles)
            .context("unable to locate handles buffer")?
            .iter()
            .copied()
            .collect(),
        DriverProvides::Filesystems => locate_storage_handles()?,
    };

    for handle in handles {
        // Ignore the result as there is nothing we can do if reconnecting a controller fails.
        // This is also likely to fail in some cases but should fail safely.
        let _ = uefi::boot::connect_controller(handle, &[], None, true);
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
    for (name, driver) in drivers {
        load_driver(context.clone(), driver).context(format!("unable to load driver: {}", name))?;
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
    reconnect(provides).context("unable to reconnect drivers")?;
    info!("loaded drivers");

    // We've now loaded all the drivers, so we can return.
    Ok(())
}
