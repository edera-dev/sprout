use crate::context::SproutContext;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use anyhow::{Context, Result};
use edera_sprout_config::drivers::DriverDeclaration;
use eficore::shim::{ShimInput, ShimSupport};
use log::info;
use uefi::boot::SearchType;

/// Loads the driver specified by the `driver` declaration.
fn load_driver(context: Rc<SproutContext>, driver: &DriverDeclaration) -> Result<()> {
    // Acquire the handle and device path of the loaded image.
    let sprout_image = uefi::boot::image_handle();

    // Resolve the path to the driver image.
    let resolved = eficore::path::resolve_path(
        Some(context.root().loaded_image_path()?),
        &context.stamp(&driver.path),
    )
    .context("unable to resolve path to driver")?;

    // Load the driver image using the shim support integration.
    // It will determine if the image needs to be loaded via the shim or can be loaded directly.
    let image = ShimSupport::load(sprout_image, ShimInput::ResolvedPath(&resolved))?;

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

    info!("loading drivers");

    // Load all the drivers in no particular order.
    for (name, driver) in drivers {
        load_driver(context.clone(), driver).context(format!("unable to load driver: {}", name))?;
    }

    // Reconnect all the controllers to all handles.
    reconnect().context("unable to reconnect drivers")?;
    info!("loaded drivers");

    // We've now loaded all the drivers, so we can return.
    Ok(())
}
