use crate::boot_counter::BootCounterTarget;
use crate::context::SproutContext;
use crate::entries::BootableEntry;
use alloc::{
    format,
    rc::Rc,
    string::{String, ToString},
    vec::Vec,
};
use anyhow::{Context, Result};
use core::{cmp::Ordering, str::FromStr};
use edera_sprout_bls::{
    BlsEntry, BootCounter, is_reserved_entry_name, profile_id_suffix, sort_bls, strip_extension,
};
use edera_sprout_config::generators::bls::BlsConfiguration;
use log::{info, warn};
use uefi::{
    cstr16,
    fs::{FileSystem, PathBuf},
    proto::device_path::text::{AllowShortcuts, DisplayOnly},
    proto::media::fs::SimpleFileSystem,
};

mod uki;

/// The PE machine type of the images that can boot on this machine.
#[cfg(target_arch = "x86_64")]
const PE_MACHINE: u16 = edera_sprout_bls::PE_MACHINE_X86_64;
/// The PE machine type of the images that can boot on this machine.
#[cfg(target_arch = "aarch64")]
const PE_MACHINE: u16 = edera_sprout_bls::PE_MACHINE_AARCH64;

/// The name of the architecture of this machine in BLS entries.
#[cfg(target_arch = "x86_64")]
const ARCHITECTURE: &str = "x64";
/// The name of the architecture of this machine in BLS entries.
#[cfg(target_arch = "aarch64")]
const ARCHITECTURE: &str = "aa64";

/// The number of initrd slots set on each BLS entry.
/// Entries set `initrd-0` through `initrd-7`, and slots without an initrd are empty.
pub const BLS_INITRD_SLOTS: usize = 8;

// TODO(azenla): remove this once variable substitution is implemented.
/// This function is used to remove the `tuned_initrd` variable from the initrd paths.
/// Fedora uses tuned which adds an initrd that shouldn't be used.
fn quirk_initrd_remove_tuned(paths: Vec<String>) -> Vec<String> {
    paths
        .into_iter()
        .filter(|path| path != "$tuned_initrd")
        .collect()
}

/// Sorts two entries according to the BLS sort system.
/// Reference: <https://uapi-group.org/specifications/specs/boot_loader_specification/#sorting>
fn sort_entries(a: &(BlsEntry, BootableEntry), b: &(BlsEntry, BootableEntry)) -> Ordering {
    let (a_bls, a_boot) = a;
    let (b_bls, b_boot) = b;
    sort_bls(a_bls, a_boot.sort_name(), b_bls, b_boot.sort_name())
}

/// Produces the bootable entry for the BLS `entry` with the id `name` and the `initrds`.
fn bootable_entry(
    context: &Rc<SproutContext>,
    bls: &BlsConfiguration,
    name: &str,
    entry: &BlsEntry,
    initrds: &[String],
) -> BootableEntry {
    // Produce a new sprout context for the entry with the extracted values.
    let mut context = context.fork();

    let title_base = entry.title().unwrap_or_else(|| name.to_string());
    let chainload = entry.chainload_path().unwrap_or_default();
    let options = entry.chainload_options().unwrap_or_default();
    let version = entry.version().unwrap_or_default();
    let machine_id = entry.machine_id().unwrap_or_default();

    // Combine the title with the version if a version is present, except if it already contains it.
    // Sometimes BLS will have a version in the title already, and this makes it unique.
    let title_full = if !version.is_empty() && !title_base.contains(&version) {
        format!("{} {}", title_base, version)
    } else {
        title_base.clone()
    };

    context.set("title-base", title_base);
    context.set("title", title_full);
    context.set("chainload", chainload);
    context.set("options", options);
    // The command line and kernel version embedded in a unified kernel image.
    context.set("cmdline", entry.cmdline.clone().unwrap_or_default());
    context.set("devicetree", entry.devicetree_path().unwrap_or_default());
    context.set("uname", entry.uname.clone().unwrap_or_default());
    // The initrd value keeps the last initrd, which is what it held before
    // multiple initrds were supported.
    context.set("initrd", initrds.last().cloned().unwrap_or_default());

    // Set every initrd slot, even the unused ones. An unset slot would let "$initrd-1"
    // match "$initrd" instead, producing the initrd value followed by "-1".
    for slot in 0..BLS_INITRD_SLOTS {
        context.set(
            format!("initrd-{}", slot),
            initrds.get(slot).cloned().unwrap_or_default(),
        );
    }

    context.set("version", version);
    context.set("machine-id", machine_id);

    // Produce a new bootable entry.
    let mut boot = BootableEntry::new(
        name.to_string(),
        bls.entry.title.clone(),
        context.freeze(),
        bls.entry.clone(),
    );

    // Pin the entry name to prevent prefixing.
    // This is needed as the bootloader interface requires the name to be
    // the same as the entry file name, minus the .conf extension.
    if bls.pin_names {
        boot.mark_pin_name();
    }

    boot
}

/// Generates the BLS Type #1 entries from the entries directory under `path`.
/// The conversion is best-effort and will ignore any unsupported entries.
fn generate_type1(
    context: &Rc<SproutContext>,
    bls: &BlsConfiguration,
    path: &str,
) -> Result<Vec<(BlsEntry, BootableEntry)>> {
    let mut entries = Vec::new();

    // Resolve the path to the BLS directory.
    let bls_resolved = eficore::path::resolve_path(Some(context.root().loaded_image_path()?), path)
        .context("unable to resolve bls path")?;

    // Construct a filesystem path to the BLS entries directory.
    let mut entries_path = PathBuf::from(
        bls_resolved
            .sub_path
            .to_string16(DisplayOnly(false), AllowShortcuts(false))
            .context("unable to convert bls path to string")?,
    );
    entries_path.push(cstr16!("entries"));

    // Open exclusive access to the BLS filesystem.
    let fs =
        uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(bls_resolved.filesystem_handle)
            .context("unable to open bls filesystem")?;
    let mut fs = FileSystem::new(fs);

    // The entries directory is optional, as a BLS directory may only contain a loader.conf file.
    // If it does not exist, there are no entries to generate.
    if !fs
        .try_exists(&entries_path)
        .context("unable to check for bls entries directory")?
    {
        return Ok(Vec::new());
    }

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
        let file_name = entry.file_name().to_string();

        // Ignore hidden files, such as the AppleDouble files that macOS writes, and files that
        // are not .conf files. Files named just ".conf" are not valid entry files.
        let Some((stem, extension)) = strip_extension(&file_name, ".conf") else {
            continue;
        };
        if file_name.starts_with('.') || stem.is_empty() {
            continue;
        }
        // Names that start with auto- are reserved for entries the boot loader makes itself.
        if is_reserved_entry_name(&file_name) {
            continue;
        }
        let extension = extension.to_string();

        // Split the boot counter off the file name. The entry id never includes the counter, so
        // that it stays the same as tries are consumed.
        let (id, boot_counter) = {
            let (id, counter) = BootCounter::parse(stem);
            (id.to_string(), counter)
        };
        let name = id;

        // Create a mutable path so we can append the file name to produce the full path.
        let mut full_entry_path = entries_path.to_path_buf();
        full_entry_path.push(entry.file_name());

        // Read the entry file.
        // Entries that can't be read or parsed are skipped, as one broken entry
        // should not prevent booting any of the other entries.
        let content = match fs.read(full_entry_path) {
            Ok(content) => content,
            Err(error) => {
                warn!("unable to read bls entry {}: {}", name, error);
                continue;
            }
        };

        // Parse the entry file as a UTF-8 string.
        let content = match String::from_utf8(content) {
            Ok(content) => content,
            Err(error) => {
                warn!("unable to read bls entry {} as utf8: {}", name, error);
                continue;
            }
        };

        // Parse the entry file as a BLS entry.
        let mut entry = match BlsEntry::from_str(&content) {
            Ok(entry) => entry,
            Err(error) => {
                warn!("unable to parse bls entry {}: {}", name, error);
                continue;
            }
        };
        entry.boot_counter = boot_counter;

        // Ignore entries that are not valid for Sprout.
        if !entry.is_valid() {
            continue;
        }

        // Ignore entries that are for another architecture.
        if !entry.matches_architecture(ARCHITECTURE) {
            continue;
        }

        // Put the initrds through a quirk modifier to support Fedora.
        let initrds = quirk_initrd_remove_tuned(entry.initrd_paths());

        // Skip entries with more initrds than there are slots, as they can't be fully loaded.
        if initrds.len() > BLS_INITRD_SLOTS {
            warn!(
                "bls entry {} has more than {} initrds, skipping",
                name, BLS_INITRD_SLOTS
            );
            continue;
        }

        let mut boot = bootable_entry(context, bls, &name, &entry, &initrds);
        boot.set_id_suffix(&extension);

        // Record where the boot counter lives so a try can be consumed when this entry boots.
        if let Some(counter) = boot_counter {
            boot.set_boot_counter(BootCounterTarget {
                counter,
                filesystem: bls_resolved.filesystem_handle,
                directory: entries_path.clone(),
                id: name.clone(),
                file_name,
                extension,
            });
        }

        // Add the BLS entry to the list, along with the bootable entry.
        entries.push((entry, boot));
    }

    Ok(entries)
}

/// Generates the BLS Type #2 entries from the unified kernel images in the `uki_path` directory.
/// An image with the same id as one of the Type #1 `entries` is skipped.
fn generate_type2(
    context: &Rc<SproutContext>,
    bls: &BlsConfiguration,
    uki_path: &str,
    entries: &[(BlsEntry, BootableEntry)],
) -> Result<Vec<(BlsEntry, BootableEntry)>> {
    let resolved = eficore::path::resolve_path(Some(context.root().loaded_image_path()?), uki_path)
        .context("unable to resolve uki path")?;
    let directory = resolved
        .sub_path
        .to_string16(DisplayOnly(false), AllowShortcuts(false))
        .context("unable to convert uki path to string")?;
    let directory_name = directory.to_string();

    let mut found: Vec<(BlsEntry, BootableEntry)> = Vec::new();
    for image in uki::scan(resolved.filesystem_handle, &directory_name)? {
        // Only unified kernel images that are built for this machine are entries.
        if image.machine != PE_MACHINE {
            info!(
                "skipping {} as it is built for another architecture",
                image.file_name
            );
            continue;
        }

        // The id is the file name without the extension and the boot counter.
        let file_name = image.file_name;
        if is_reserved_entry_name(&file_name) {
            continue;
        }
        let Some((stem, extension)) = strip_extension(&file_name, ".efi") else {
            continue;
        };
        let extension = extension.to_string();
        let (id, boot_counter) = BootCounter::parse(stem);
        if id.is_empty() {
            continue;
        }
        let id = id.to_string();

        // Every profile of the image is an entry. They share the file, and so the boot counter.
        let mut suffixes: Vec<String> = Vec::new();
        for profile in image.profiles {
            // A profile without an os-release section is not a unified kernel image, such as
            // other EFI programs.
            if !profile.has_osrel {
                info!(
                    "skipping {} as it has no .osrel section, so it is not a unified kernel image",
                    file_name
                );
                continue;
            }

            // The first profile has the id of the file, and the others name their profile.
            // A profile that repeats the identifier of an earlier one is told apart by its number.
            let index = profile.index.unwrap_or(0);
            let mut suffix = profile_id_suffix(index, &profile.info);
            if let Some(ref name) = suffix
                && suffixes.contains(name)
            {
                suffix = Some(index.to_string());
            }
            if let Some(ref name) = suffix {
                suffixes.push(name.clone());
            }
            let name = match suffix {
                Some(ref suffix) => format!("{}@{}", id, suffix),
                None => id.clone(),
            };

            let mut entry = profile.entry;
            entry.boot_counter = boot_counter;

            let mut boot = bootable_entry(context, bls, &name, &entry, &[]);
            boot.set_id_suffix(&extension);
            if let Some(ref suffix) = suffix {
                boot.set_id_profile(suffix);
            }

            // An image with the same id as another entry, such as a leftover copy with another
            // boot counter, would be ambiguous. Type 1 and type 2 entries have different ids.
            if entries
                .iter()
                .chain(found.iter())
                .any(|(_, other)| other.id() == boot.id())
            {
                warn!(
                    "unified kernel image {} has the same id as another entry, skipping",
                    file_name
                );
                continue;
            }

            if let Some(counter) = boot_counter {
                boot.set_boot_counter(BootCounterTarget {
                    counter,
                    filesystem: resolved.filesystem_handle,
                    directory: PathBuf::from(directory.clone()),
                    id: id.clone(),
                    file_name: file_name.clone(),
                    extension: extension.clone(),
                });
            }
            found.push((entry, boot));
        }
    }
    Ok(found)
}

/// Generates entries from the BLS entries directory and the unified kernel image directory
/// using the specified `bls` configuration and `context`. The BLS conversion is best-effort
/// and will ignore any unsupported entries.
pub fn generate(context: Rc<SproutContext>, bls: &BlsConfiguration) -> Result<Vec<BootableEntry>> {
    // Stamp the path to the BLS directory.
    let path = context.stamp(&bls.path);

    let uki_path = bls.uki_path_for(&path);

    // A problem reading the Type #1 entries only stops the unified kernel images when there
    // are none to add, so that either kind of entry can still boot.
    let mut entries = match generate_type1(&context, bls, &path) {
        Ok(entries) => entries,
        Err(error) if uki_path.is_some() => {
            warn!("unable to generate bls entries: {:#}", error);
            Vec::new()
        }
        Err(error) => return Err(error),
    };

    // Add the unified kernel images, unless that is disabled. A problem scanning them
    // should not prevent booting the other entries.
    if let Some(uki_path) = uki_path {
        let uki_path = context.stamp(&uki_path);
        match generate_type2(&context, bls, &uki_path, &entries) {
            Ok(found) => entries.extend(found),
            Err(error) => warn!(
                "unable to generate unified kernel image entries: {:#}",
                error
            ),
        }
    }

    // Sort all the entries according to the BLS sort system.
    entries.sort_by(sort_entries);

    // Grab the number of entries that we have, so we can calculate a reverse index.
    let entry_count = entries.len();

    // Set the sort keys of all the bootable entries to a semi-unique prefix + the BLS sort order.
    // The final comparison happens using version comparison, so this will sort
    // things properly.
    for (idx, (_bls, boot)) in entries.iter_mut().enumerate() {
        boot.set_sort_key(format!("bls-{}-{}", path, entry_count - idx - 1));
    }

    // Collect all the bootable entries and return them.
    Ok(entries.into_iter().map(|(_, boot)| boot).collect())
}
