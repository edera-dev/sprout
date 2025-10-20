use crate::context::SproutContext;
use crate::utils;
use anyhow::{Context, Result};
use log::info;
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

fn load_driver(context: Rc<SproutContext>, driver: &DriverDeclaration) -> Result<()> {
    let sprout_image = uefi::boot::image_handle();
    let image_device_path_protocol =
        uefi::boot::open_protocol_exclusive::<LoadedImageDevicePath>(sprout_image)
            .context("unable to open loaded image device path protocol")?;

    let mut full_path = utils::device_path_root(&image_device_path_protocol)?;

    full_path.push_str(&context.stamp(&driver.path));

    info!("driver path: {}", full_path);

    let device_path = utils::text_to_device_path(&full_path)?;

    let image = uefi::boot::load_image(
        sprout_image,
        uefi::boot::LoadImageSource::FromDevicePath {
            device_path: &device_path,
            boot_policy: uefi::proto::BootPolicy::ExactMatch,
        },
    )
    .context("unable to load image")?;

    uefi::boot::start_image(image).context("unable to start driver image")?;

    Ok(())
}

fn reconnect() -> Result<()> {
    let handles = uefi::boot::locate_handle_buffer(SearchType::AllHandles)
        .context("unable to locate handles buffer")?;

    for handle in handles.iter() {
        // ignore result as there is nothing we can do if it doesn't work.
        let _ = uefi::boot::connect_controller(*handle, None, None, true);
    }

    Ok(())
}

pub fn load(
    context: Rc<SproutContext>,
    drivers: &BTreeMap<String, DriverDeclaration>,
) -> Result<()> {
    if drivers.is_empty() {
        return Ok(());
    }

    info!("loading drivers");
    for (name, driver) in drivers {
        load_driver(context.clone(), driver).context(format!("unable to load driver: {}", name))?;
    }

    reconnect().context("unable to reconnect drivers")?;
    info!("loaded drivers");
    Ok(())
}
