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

mod entry;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct BlsConfiguration {
    pub entry: EntryDeclaration,
    #[serde(default = "default_bls_path")]
    pub path: String,
}

fn default_bls_path() -> String {
    "\\loader\\entries".to_string()
}

pub fn generate(
    context: Rc<SproutContext>,
    bls: &BlsConfiguration,
) -> Result<Vec<(Rc<SproutContext>, EntryDeclaration)>> {
    let mut entries = Vec::new();

    let path = context.stamp(&bls.path);
    let resolved = utils::resolve_path(context.root().loaded_image_path()?, &path)
        .context("unable to resolve bls path")?;
    let fs = uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(resolved.filesystem_handle)
        .context("unable to open bls filesystem")?;
    let mut fs = FileSystem::new(fs);
    let sub_text_path = resolved
        .sub_path
        .to_string(DisplayOnly(false), AllowShortcuts(false))
        .context("unable to convert subpath to string")?;
    let entries_path = Path::new(&sub_text_path);

    let entries_iter = fs
        .read_dir(entries_path)
        .context("unable to read bls entries")?;

    for entry in entries_iter {
        let entry = entry?;
        if !entry.is_regular_file() {
            continue;
        }
        let name = entry.file_name().to_string();
        if !name.ends_with(".conf") {
            continue;
        }

        let full_entry_path = CString16::try_from(format!("{}\\{}", sub_text_path, name).as_str())
            .context("unable to construct full entry path")?;
        let full_entry_path = Path::new(&full_entry_path);
        let content = fs
            .read(full_entry_path)
            .context("unable to read bls file")?;
        let content = String::from_utf8(content).context("unable to read bls entry as utf8")?;
        let entry = BlsEntry::from_str(&content).context("unable to parse bls entry")?;

        if !entry.is_valid() {
            continue;
        }

        let mut context = context.fork();
        context.set("title", entry.title().unwrap_or(name));
        context.set("chainload", entry.chainload_path().unwrap_or_default());
        context.set("options", entry.options().unwrap_or_default());
        context.set("initrd", entry.initrd_path().unwrap_or_default());

        entries.push((context.freeze(), bls.entry.clone()));
    }

    Ok(entries)
}
