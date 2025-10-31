use crate::context::SproutContext;
use crate::entries::{BootableEntry, EntryDeclaration};
use crate::generators::bls::entry::BlsEntry;
use crate::utils;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use std::str::FromStr;
use uefi::cstr16;
use uefi::fs::{FileSystem, PathBuf};
use uefi::proto::device_path::text::{AllowShortcuts, DisplayOnly};
use uefi::proto::media::fs::SimpleFileSystem;

/// BLS entry parser.
mod entry;

/// The default path to the BLS directory.
const BLS_TEMPLATE_PATH: &str = "\\loader";

/// The configuration of the BLS generator.
/// The BLS uses the Bootloader Specification to produce
/// entries from an input template.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct BlsConfiguration {
    /// The entry to use for as a template.
    pub entry: EntryDeclaration,
    /// The path to the BLS directory.
    #[serde(default = "default_bls_path")]
    pub path: String,
}

fn default_bls_path() -> String {
    BLS_TEMPLATE_PATH.to_string()
}

// TODO(azenla): remove this once variable substitution is implemented.
/// This function is used to remove the `tuned_initrd` variable from entry values.
/// Fedora uses tuned which adds an initrd that shouldn't be used.
fn quirk_initrd_remove_tuned(input: String) -> String {
    input.replace("$tuned_initrd", "").trim().to_string()
}

/// Generates entries from the BLS entries directory using the specified `bls` configuration and
/// `context`. The BLS conversion is best-effort and will ignore any unsupported entries.
pub fn generate(context: Rc<SproutContext>, bls: &BlsConfiguration) -> Result<Vec<BootableEntry>> {
    let mut entries = Vec::new();

    // Stamp the path to the BLS directory.
    let path = context.stamp(&bls.path);

    // Resolve the path to the BLS directory.
    let bls_resolved = utils::resolve_path(Some(context.root().loaded_image_path()?), &path)
        .context("unable to resolve bls path")?;

    // Construct a filesystem path to the BLS entries directory.
    let mut entries_path = PathBuf::from(
        bls_resolved
            .sub_path
            .to_string(DisplayOnly(false), AllowShortcuts(false))
            .context("unable to convert bls path to string")?,
    );
    entries_path.push(cstr16!("entries"));

    // Open exclusive access to the BLS filesystem.
    let fs =
        uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(bls_resolved.filesystem_handle)
            .context("unable to open bls filesystem")?;
    let mut fs = FileSystem::new(fs);

    // Read the BLS entries directory.
    let entries_iter = fs
        .read_dir(&entries_path)
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
        let mut name = entry.file_name().to_string();

        // Ignore files that are not .conf files.
        if !name.to_lowercase().ends_with(".conf") {
            continue;
        }

        // Remove the .conf extension.
        name.truncate(name.len() - 5);

        // Create a mutable path so we can append the file name to produce the full path.
        let mut full_entry_path = entries_path.to_path_buf();
        full_entry_path.push(entry.file_name());

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

        let title = entry.title().unwrap_or_else(|| name.clone());
        let chainload = entry.chainload_path().unwrap_or_default();
        let options = entry.options().unwrap_or_default();

        // Put the initrd through a quirk modifier to support Fedora.
        let initrd = quirk_initrd_remove_tuned(entry.initrd_path().unwrap_or_default());

        context.set("title", title);
        context.set("chainload", chainload);
        context.set("options", options);
        context.set("initrd", initrd);

        // Produce a new bootable entry.
        let mut entry = BootableEntry::new(
            name,
            bls.entry.title.clone(),
            context.freeze(),
            bls.entry.clone(),
        );

        // Pin the entry name to prevent prefixing.
        // This is needed as the bootloader interface requires the name to be
        // the same as the entry file name, minus the .conf extension.
        entry.mark_pin_name();

        // Add the entry to the list with a frozen context.
        entries.push(entry);
    }

    Ok(entries)
}
