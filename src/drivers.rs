use crate::context::SproutContext;
use crate::utils;
use anyhow::{Context, Result};
use log::debug;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::rc::Rc;
use uefi::boot::SearchType;
use uefi::proto::device_path::LoadedImageDevicePath;

/// Declares a driver configuration.
/// Drivers allow extending the functionality of Sprout.
/// Drivers are loaded at runtime and can provide extra functionality like filesystem support.
/// Drivers are loaded by their name, which is used to reference them in other concepts.
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct DriverDeclaration {
    /// The filesystem path to the driver.
    /// This file should be an EFI executable that can be located and executed.
    pub path: String,
}

/// Loads the driver specified by the `driver` declaration.
fn load_driver(context: Rc<SproutContext>, driver: &DriverDeclaration) -> Result<()> {
    // Acquire the handle and device path of the loaded image.
    let sprout_image = uefi::boot::image_handle();
    let image_device_path_protocol =
        uefi::boot::open_protocol_exclusive::<LoadedImageDevicePath>(sprout_image)
            .context("unable to open loaded image device path protocol")?;

    // Get the device path root of the sprout image.
    let mut full_path = utils::device_path_root(&image_device_path_protocol)?;

    // Push the path of the driver from the root.
    full_path.push_str(&context.stamp(&driver.path));

    debug!("driver path: {}", full_path);

    // Convert the path to a device path.
    let device_path = utils::text_to_device_path(&full_path)?;

    // Load the driver image.
    let image = uefi::boot::load_image(
        sprout_image,
        uefi::boot::LoadImageSource::FromDevicePath {
            device_path: &device_path,
            boot_policy: uefi::proto::BootPolicy::ExactMatch,
        },
    )
    .context("unable to load image")?;

    // Start the driver image, this is expected to return control to sprout.
    // There is no guarantee that the driver will actually return control as it is
    // just a standard EFI image.
    uefi::boot::start_image(image).context("unable to start driver image")?;

    Ok(())
}

/// Reconnects all handles to their controllers.
/// This is effectively a UEFI stack reload in a sense.
/// After we load all the drivers, we need to reconnect all of their handles
/// so that filesystems are recognized again.
fn reconnect() -> Result<()> {
    // Locate all of the handles in the UEFI stack.
    let handles = uefi::boot::locate_handle_buffer(SearchType::AllHandles)
        .context("unable to locate handles buffer")?;

    for handle in handles.iter() {
        // Ignore the result as there is nothing we can do if reconnecting a controller fails.
        // This is also likely to fail in some cases but should fail safely.
        let _ = uefi::boot::connect_controller(*handle, None, None, true);
    }

    Ok(())
}

/// Load all the drivers specified in `drivers`.
/// There is no driver order currently. This will reconnect all the controllers
/// to all handles if at least one driver was loaded.
pub fn load(
    context: Rc<SproutContext>,
    drivers: &BTreeMap<String, DriverDeclaration>,
) -> Result<()> {
    // If there are no drivers, we don't need to do anything.
    if drivers.is_empty() {
        return Ok(());
    }

    debug!("loading drivers");

    // Load all the drivers in no particular order.
    for (name, driver) in drivers {
        load_driver(context.clone(), driver).context(format!("unable to load driver: {}", name))?;
    }

    // Reconnect all the controllers to all handles.
    reconnect().context("unable to reconnect drivers")?;
    debug!("loaded drivers");

    // We've now loaded all the drivers, so we can return.
    Ok(())
}
