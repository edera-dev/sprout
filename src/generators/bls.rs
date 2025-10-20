use crate::context::SproutContext;
use crate::entries::EntryDeclaration;
use crate::generators::bls::entry::BlsEntry;
use crate::utils;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use std::str::FromStr;
use uefi::CString16;
use uefi::fs::{FileSystem, Path};
use uefi::proto::device_path::text::{AllowShortcuts, DisplayOnly};
use uefi::proto::media::fs::SimpleFileSystem;

/// BLS entry parser.
mod entry;

/// The default path to the BLS entries directory.
const BLS_TEMPLATE_PATH: &str = "\\loader\\entries";

/// The configuration of the BLS generator.
/// The BLS uses the Bootloader Specification to produce
/// entries from an input template.
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct BlsConfiguration {
    /// The entry to use for as a template.
    pub entry: EntryDeclaration,
    /// The path to the BLS entries directory.
    #[serde(default = "default_bls_path")]
    pub path: String,
}

fn default_bls_path() -> String {
    BLS_TEMPLATE_PATH.to_string()
}

/// Generates entries from the BLS entries directory using the specified `bls` configuration and
/// `context`. The BLS conversion is best-effort and will ignore any unsupported entries.
pub fn generate(
    context: Rc<SproutContext>,
    bls: &BlsConfiguration,
) -> Result<Vec<(Rc<SproutContext>, EntryDeclaration)>> {
    let mut entries = Vec::new();

    // Stamp the path to the BLS entries directory.
    let path = context.stamp(&bls.path);

    // Resolve the path to the BLS entries directory.
    let resolved = utils::resolve_path(context.root().loaded_image_path()?, &path)
        .context("unable to resolve bls path")?;

    // Open exclusive access to the BLS filesystem.
    let fs = uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(resolved.filesystem_handle)
        .context("unable to open bls filesystem")?;
    let mut fs = FileSystem::new(fs);

    // Convert the subpath to the BLS entries directory to a string.
    let sub_text_path = resolved
        .sub_path
        .to_string(DisplayOnly(false), AllowShortcuts(false))
        .context("unable to convert subpath to string")?;

    // Produce a path to the BLS entries directory.
    let entries_path = Path::new(&sub_text_path);

    // Read the BLS entries directory.
    let entries_iter = fs
        .read_dir(entries_path)
        .context("unable to read bls entries")?;

    // For each entry in the BLS entries directory, parse the entry and add it to the list.
    for entry in entries_iter {
        // Unwrap the entry file info.
        let entry = entry.context("unable to read bls item entry")?;

        // Skip items that are not regular files.
        if !entry.is_regular_file() {
            continue;
        }

        // Get the file name of the filesystem item.
        let name = entry.file_name().to_string();

        // Ignore files that are not .conf files.
        if !name.ends_with(".conf") {
            continue;
        }

        // Produce the full path to the entry file.
        let full_entry_path = CString16::try_from(format!("{}\\{}", sub_text_path, name).as_str())
            .context("unable to construct full entry path")?;
        let full_entry_path = Path::new(&full_entry_path);

        // Read the entry file.
        let content = fs
            .read(full_entry_path)
            .context("unable to read bls file")?;

        // Parse the entry file as a UTF-8 string.
        let content = String::from_utf8(content).context("unable to read bls entry as utf8")?;

        // Parse the entry file as a BLS entry.
        let entry = BlsEntry::from_str(&content).context("unable to parse bls entry")?;

        // Ignore entries that are not valid for Sprout.
        if !entry.is_valid() {
            continue;
        }

        // Produce a new sprout context for the entry with the extracted values.
        let mut context = context.fork();
        context.set("title", entry.title().unwrap_or(name));
        context.set("chainload", entry.chainload_path().unwrap_or_default());
        context.set("options", entry.options().unwrap_or_default());
        context.set("initrd", entry.initrd_path().unwrap_or_default());

        // Add the entry to the list with a frozen context.
        entries.push((context.freeze(), bls.entry.clone()));
    }

    Ok(entries)
}
