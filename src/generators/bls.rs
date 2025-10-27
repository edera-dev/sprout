use crate::context::SproutContext;
use crate::entries::{BootableEntry, EntryDeclaration};
use crate::generators::bls::entry::BlsEntry;
use crate::utils;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use std::str::FromStr;
use uefi::fs::{FileSystem, Path, PathBuf};
use uefi::proto::device_path::text::{AllowShortcuts, DisplayOnly};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::{CString16, cstr16};

/// BLS entry parser.
mod entry;

/// The default path to the BLS directory.
const BLS_TEMPLATE_PATH: &str = "\\loader";

/// The configuration of the BLS generator.
/// The BLS uses the Bootloader Specification to produce
/// entries from an input template.
#[derive(Serialize, Deserialize, Default, Clone)]
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

    // Convert the path to a UEFI PathBuf.
    let path = PathBuf::from(
        CString16::try_from(path.as_str()).context("unable to convert bls path to CString16")?,
    );

    // Construct the path to the BLS entries directory.
    let mut entries_path = path.clone();
    entries_path.push(cstr16!("entries"));

    // Resolve the path to the BLS entries directory.
    let entries_resolved = utils::resolve_path(
        context.root().loaded_image_path()?,
        &path.to_cstr16().to_string(),
    )
    .context("unable to resolve bls path")?;

    // Open exclusive access to the BLS filesystem.
    let fs =
        uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(entries_resolved.filesystem_handle)
            .context("unable to open bls filesystem")?;
    let mut fs = FileSystem::new(fs);

    // Convert the subpath to the BLS entries directory to a string.
    let sub_text_path = entries_resolved
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

        // Add the entry to the list with a frozen context.
        entries.push(BootableEntry::new(
            name,
            bls.entry.title.clone(),
            context.freeze(),
            bls.entry.clone(),
        ));
    }

    Ok(entries)
}
